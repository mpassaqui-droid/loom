use super::*;
use loom_core::Synth;

const SR: f32 = 48000.0;

fn synth(segs: &[&str]) -> Synth {
    let mut s = Synth::default();
    let mut w = Wave::Square;
    for seg in segs { assert!(s.apply_segment(seg, &mut w), "not a sound verb: {seg}"); }
    s
}

fn spec<'a>(synth: &'a Synth, wave: Wave, freq: f32, dur: f32, prev: Option<f32>) -> NoteSpec<'a> {
    NoteSpec { name: "lead", synth, wave, freq, dur, peak: 0.2, pan: None, prev_freq: prev, sr: SR, bar_sec: 2.0, seed: 7 }
}

/// Render a note until it is done.
fn play(spec: &NoteSpec) -> Vec<Stereo> {
    let clock = Clock::new(SR);
    let mut t = Tone::new(spec);
    let mut out = Vec::new();
    while !t.done() {
        out.push(t.render(&clock));
        assert!(out.len() < (SR * 30.0) as usize, "note never ends");
    }
    out
}

fn rms(xs: impl Iterator<Item = f32>) -> f32 {
    let (mut acc, mut n) = (0.0f64, 0);
    for x in xs { acc += (x * x) as f64; n += 1; }
    (acc / n.max(1) as f64).sqrt() as f32
}

/// RMS of the left channel above `hz` (two cascaded high-passes).
fn rms_above(out: &[Stereo], hz: f32) -> f32 {
    let (mut a, mut b) = (Biquad::highpass(SR, hz, 0.707), Biquad::highpass(SR, hz, 0.707));
    let filtered: Vec<f32> = out.iter().map(|s| b.process(a.process(s.l))).collect();
    rms(filtered[2000..].iter().copied())
}

const HELD: &str = "env 0.005 0.05 1 0.05";

#[test]
fn unison_spread_fills_both_channels() {
    let wide = synth(&["unison 5", "spread 1", HELD]);
    let out = play(&spec(&wide, Wave::Saw, 220.0, 0.5, None));
    let (l, r) = (rms(out.iter().map(|s| s.l)), rms(out.iter().map(|s| s.r)));
    let side = rms(out.iter().map(|s| s.l - s.r));
    assert!(l > 0.02 && r > 0.02, "both channels sound: {l} {r}");
    assert!(side > 0.2 * l, "the detuned voices differ left and right: side {side} vs {l}");

    let mono = synth(&["unison 5", HELD]);
    let out = play(&spec(&mono, Wave::Saw, 220.0, 0.5, None));
    assert!(out.iter().all(|s| s.l == s.r), "without spread the unison stays mono");
}

#[test]
fn lowpass_removes_highs() {
    let bright = synth(&[HELD]);
    let dark = synth(&[HELD, "lp 400"]);
    // a saw well above the cutoff loses most of its level…
    let a = play(&spec(&bright, Wave::Saw, 880.0, 0.5, None));
    let b = play(&spec(&dark, Wave::Saw, 880.0, 0.5, None));
    let (ra, rb) = (rms(a.iter().map(|s| s.l)), rms(b.iter().map(|s| s.l)));
    assert!(rb < ra * 0.4, "overall level drops: {rb} vs {ra}");
    // …and a low saw keeps its fundamental but loses its upper harmonics
    let a = play(&spec(&bright, Wave::Saw, 220.0, 0.5, None));
    let b = play(&spec(&dark, Wave::Saw, 220.0, 0.5, None));
    let (ha, hb) = (rms_above(&a, 3000.0), rms_above(&b, 3000.0));
    assert!(hb < ha * 0.05, "highs are gone: {hb} vs {ha}");
}

#[test]
fn resonance_is_in_decibels_like_webaudio() {
    // Q 20 dB = a ratio of 10: a saw right at the cutoff rings far louder than with Q 0.1
    let flat = synth(&[HELD, "lp 660 q 0.1"]);
    let peaky = synth(&[HELD, "lp 660 q 20"]);
    let a = rms(play(&spec(&flat, Wave::Saw, 220.0, 0.5, None)).iter().map(|s| s.l));
    let b = rms(play(&spec(&peaky, Wave::Saw, 220.0, 0.5, None)).iter().map(|s| s.l));
    assert!(b > a * 1.5, "{b} vs {a}");
}

#[test]
fn pluck_decays() {
    let s = synth(&["pluck 0.6", "env 0.001 0.1 1 0.3"]);
    let out = play(&spec(&s, Wave::Saw, 220.0, 2.0, None));
    let at = |t: f32| rms(out[(t * SR) as usize..((t + 0.1) * SR) as usize].iter().map(|s| s.l));
    assert!(at(0.02) > 0.01, "the string starts loud");
    assert!(at(1.5) < at(0.02) * 0.3, "and dies away: {} → {}", at(0.02), at(1.5));
}

#[test]
fn fm_adds_partials() {
    let plain = synth(&[HELD]);
    let fm = synth(&[HELD, "fm 2 3"]);
    let a = play(&spec(&plain, Wave::Sine, 220.0, 0.5, None));
    let b = play(&spec(&fm, Wave::Sine, 220.0, 0.5, None));
    let (ha, hb) = (rms_above(&a, 600.0), rms_above(&b, 600.0));
    assert!(hb > ha * 10.0 && hb > 0.01, "sidebands appear: {hb} vs {ha}");
}

#[test]
fn slide_starts_at_the_previous_pitch() {
    let s = synth(&["slide 0.2", HELD]);
    let out = play(&spec(&s, Wave::Sine, 440.0, 1.0, Some(110.0)));
    let ups: Vec<usize> = (1..out.len()).filter(|&i| out[i - 1].l < 0.0 && out[i].l >= 0.0).collect();
    let first_period = (ups[1] - ups[0]) as f32;
    assert!((380.0..470.0).contains(&first_period), "starts near 110 Hz: period {first_period}");
    let late = ups.iter().filter(|&&i| i >= (0.5 * SR) as usize && i < (0.6 * SR) as usize).count();
    assert!((42..=46).contains(&late), "arrives at 440 Hz: {late} cycles in 100 ms");
}

#[test]
fn envelope_timing_matches_the_browser() {
    // with env: stop = max(a+d+0.001, dur) + release + 0.02
    let s = synth(&["env 0.01 0.1 0.5 0.3"]);
    let sp = spec(&s, Wave::Sine, 220.0, 0.5, None);
    assert_eq!(play(&sp).len(), (0.82f32 * SR).ceil() as usize);
    let t = Tone::new(&sp);
    assert!((t.amp_at(0.01) - 0.2).abs() < 1e-4, "attack reaches the peak");
    assert!((t.amp_at(0.3) - 0.1).abs() < 1e-4, "sustain level");
    assert!((t.amp_at(0.5 + 0.15) - (0.1f32 * 0.0001).sqrt()).abs() < 1e-3, "exponential release");

    // without env: an 8 ms attack, decay to min(dur·0.9, 0.4), stop 20 ms later
    let plain = Synth::default();
    for (dur, stop) in [(1.0f32, 0.42f32), (0.1, 0.11)] {
        let sp = spec(&plain, Wave::Square, 220.0, dur, None);
        assert_eq!(play(&sp).len(), (stop * SR).ceil() as usize, "dur {dur}");
        assert!((Tone::new(&sp).amp_at(0.008) - 0.2).abs() < 1e-4);
    }
}

#[test]
fn every_source_stays_finite_at_extremes() {
    let configs: &[&[&str]] = &[
        &[],
        &["unison 7", "detune 100", "spread 1", "sub 1", "double 1", "harmonize 24 1", "noise 1"],
        &["wt rich", "unison 7", "detune 100"],
        &["wt organ"], &["wt warm"], &["wt soft"],
        &["partials 1 0.5 0.3 0.2 0.1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1"],
        &["partials 0 0 0"],
        &["fm 16 12 8", "sub 1", "double 1", "lp 20000 q 20", "fenv 1"],
        &["fm 0.1 12 8", "slide 0.5", "bend -48 2"],
        &["pluck 1"], &["pluck 0.01", "wow 1", "lfo2 pitch 20 10"],
        &["bow", "lfo pitch 0.01 4 square"], &["reed", "wow 1"],
        &["sync 8"], &["sync 1.2", "lfo2 pitch 20 4 tri"],
        &["pwm 1", "wow 1"], &["pwm 0.05"],
        &["bell", "lp 20 q 20 fenv 1"], &["grain 1", "spread 1", "hp 18000 q 20"],
        &["grain 0.05", "bp 20 q 0.1"],
        &["lp 100 q 20", "lfo cutoff 20 10 saw", "lfo2 q 20 10 rand"],
        &["notch 1000 q 20", "lfo reso 50 -3", "env 0 0 0 0"],
        &["supersaw", "lp 20..20000", "fenv 1", "env 2 0.5 1 1", "slide 0.005"],
        &["bend 48 0.005", "harmonize -24 1", "sub 1"],
    ];
    let waves = [Wave::Saw, Wave::Square, Wave::Sine, Wave::Triangle];
    for (k, segs) in configs.iter().enumerate() {
        let s = synth(segs);
        for (j, (freq, dur, prev)) in [(20.0, 0.001, None), (20000.0, 1.0, Some(20.0)), (8.0, 0.3, Some(19000.0)), (440.0, 2.5, Some(440.0))].into_iter().enumerate() {
            let wave = waves[(k + j) % 4]; // every wave meets every pitch across the configs
            let out = play(&spec(&s, wave, freq, dur, prev));
            assert!(
                out.iter().all(|x| x.l.is_finite() && x.r.is_finite() && x.l.abs() < 20.0 && x.r.abs() < 20.0),
                "{segs:?} {wave:?} {freq} Hz {dur} s"
            );
        }
    }
}

/// Cost per voice: `cargo test -p loom-audio --release tone_cost -- --ignored --nocapture`.
#[test]
#[ignore]
fn tone_cost() {
    let configs: &[(&str, &[&str])] = &[
        ("plain", &[HELD]),
        ("lp", &[HELD, "lp 1100"]),
        ("pad: unison 4 lp env", &["unison 4", "lp 1100", "env 1.2 0.8 0.7 3"]),
        ("supersaw lp sweep", &["supersaw", "lp 200..6000", HELD]),
        ("acid: lp signal q fenv slide", &["lp sine 0.2 400 1900", "q 7", "fenv 0.6", "slide", HELD]),
        ("fm bells", &["fm 3.5 4", HELD]),
        ("wt rich", &["wt rich", HELD]),
        ("pluck", &["pluck", HELD]),
        ("bell", &["bell", HELD]),
        ("grain", &["grain 1", HELD]),
        ("pwm", &["pwm", HELD]),
        ("lfo pitch+cutoff", &["lp 800", "lfo cutoff 3 0.5", "lfo2 pitch 5 0.3", HELD]),
    ];
    let clock = Clock::new(SR);
    for (label, segs) in configs {
        let s = synth(segs);
        let sp = spec(&s, Wave::Saw, 220.0, 4.0, Some(110.0));
        let n = (SR * 4.0) as usize;
        let t0 = std::time::Instant::now();
        let mut t = Tone::new(&sp);
        let mut acc = 0.0;
        for _ in 0..n { acc += t.render(&clock).l; }
        let ns = t0.elapsed().as_nanos() as f64 / n as f64;
        println!("{label:32} {ns:6.1} ns/sample  ({:.0}× realtime per voice) {acc}", 1e9 / (ns * SR as f64));
    }
}
