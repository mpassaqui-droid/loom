//! Tests for the insert chain: each effect changes the signal the way it should.

use super::*;
use crate::sample::SampleData;
use loom_core::{Synth, Wave};
use std::f32::consts::{SQRT_2, TAU};

const SR: f32 = 48000.0;

fn synth(verbs: &str) -> Synth {
    let mut s = Synth::default();
    let mut w = Wave::Square;
    for seg in verbs.split('|').map(str::trim).filter(|s| !s.is_empty()) {
        assert!(s.apply_segment(seg, &mut w), "not a sound verb: {seg}");
    }
    s
}

fn build(s: &Synth, pan: Option<f32>, bank: &SampleBank) -> Inserts {
    let spec = NoteSpec { name: "lead", synth: s, wave: Wave::Square, freq: 220.0, dur: 0.5, peak: 0.2, pan, prev_freq: None, sr: SR, bar_sec: 2.0, seed: 1 };
    Inserts::new(&spec, bank)
}

fn chain(verbs: &str) -> Inserts { build(&synth(verbs), None, &SampleBank::empty()) }

/// Run `n` samples of `input(i)` through the chain.
fn run(ins: &mut Inserts, n: usize, input: impl Fn(usize) -> Stereo) -> Vec<NoteOut> {
    let clock = Clock::new(SR);
    (0..n).map(|i| ins.process(input(i), &clock)).collect()
}

fn sine(freq: f32, amp: f32) -> impl Fn(usize) -> Stereo {
    move |i| Stereo::mono(amp * (TAU * freq * i as f32 / SR).sin())
}

fn impulse(i: usize) -> Stereo { Stereo::mono(if i == 0 { 1.0 } else { 0.0 }) }

fn peak(out: &[NoteOut], from: usize, to: usize, f: impl Fn(&NoteOut) -> f32) -> f32 {
    out[from..to].iter().map(|o| f(o).abs()).fold(0.0, f32::max)
}

fn rms(v: impl Iterator<Item = f32>) -> f32 {
    let (s, n) = v.fold((0.0, 0), |(s, n), x| (s + x * x, n + 1));
    (s / n.max(1) as f32).sqrt()
}

#[test]
fn a_plain_voice_passes_through_untouched() {
    let mut ins = chain("");
    let out = run(&mut ins, 100, sine(440.0, 0.3));
    assert!(out.iter().enumerate().all(|(i, o)| o.main == sine(440.0, 0.3)(i) && o.bus_only == Stereo::ZERO));
    assert_eq!(ins.tail(), 0.0);
}

#[test]
fn crush_quantises() {
    let mut ins = chain("crush 2");
    let out = run(&mut ins, 2000, sine(50.0, 0.9));
    let levels = [-1.0, -0.5, 0.0, 0.5, 1.0];
    assert!(out.iter().all(|o| levels.contains(&o.main.l)), "only 2-bit steps");
    assert!(out.iter().any(|o| o.main.l == 0.5));
}

#[test]
fn eq_high_shelf_boosts_highs_only() {
    let gain = |f: f32| {
        let mut ins = chain("eq 0 0 12");
        let out = run(&mut ins, 24000, sine(f, 0.1));
        rms(out[12000..].iter().map(|o| o.main.l)) / (0.1 / SQRT_2)
    };
    assert!(gain(12000.0) > 3.0, "+12 dB above 5 kHz: {}", gain(12000.0));
    assert!((gain(100.0) - 1.0).abs() < 0.1, "lows unchanged: {}", gain(100.0));
}

#[test]
fn vowel_drive_fold_and_comp_shape_the_sound() {
    // vowel: a formant band-pass keeps 800 Hz, loses 6 kHz
    let level = |verbs: &str, f: f32| {
        let mut ins = chain(verbs);
        let out = run(&mut ins, 12000, sine(f, 0.1));
        rms(out[6000..].iter().map(|o| o.main.l))
    };
    assert!(level("vowel a", 800.0) > 10.0 * level("vowel a", 6000.0));

    // drive squares a sine off: RMS close to its peak
    let mut ins = chain("drive 1");
    let out = run(&mut ins, 4800, sine(100.0, 0.5));
    let (r, p) = (rms(out.iter().map(|o| o.main.l)), peak(&out, 0, 4800, |o| o.main.l));
    assert!(r / p > 0.9 && p < 0.5, "crest {} peak {p}", r / p);

    // fold: a slow ramp up to 1 comes out folded back (not monotonic)
    let mut ins = chain("fold 1");
    let out = run(&mut ins, 1000, |i| Stereo::mono(i as f32 / 1000.0));
    let rises = out.windows(3).filter(|w| (w[1].main.l - w[0].main.l) * (w[2].main.l - w[1].main.l) < 0.0).count();
    assert!(rises >= 3, "folds back {rises} times");

    // comp: a loud sine (0 dB) gets far less gain than a quiet one (-32 dB)
    let gain = |amp: f32| {
        let mut ins = chain("comp 1");
        let out = run(&mut ins, 24000, sine(200.0, amp));
        rms(out[12000..].iter().map(|o| o.main.l)) / (amp / SQRT_2)
    };
    let (quiet, loud) = (gain(0.025), gain(1.0));
    assert!(quiet / loud > 1.6, "quiet gain {quiet} vs loud gain {loud}");
}

#[test]
fn redux_holds_and_reson_rings_at_its_pitch() {
    let mut ins = chain("redux 0.1"); // hold every 4 samples
    let out = run(&mut ins, 400, sine(300.0, 0.5));
    let changes = out.windows(2).filter(|w| w[0].main.l != w[1].main.l).count();
    assert!((95..=101).contains(&changes), "{changes} changes in 400 samples");

    let mut ins = chain("reson 200 0.9");
    let out = run(&mut ins, 2000, impulse);
    let per = 240; // 48000 / 200
    assert!(out[per].main.l.abs() > 0.1 && out[2 * per + 1].main.l.abs() > 0.05, "rings every period");
    assert!(out[per / 2].main.l.abs() < 1e-6);
    assert!(ins.tail() > 0.05);
}

#[test]
fn flange_width_ring_am_phaser_change_the_signal() {
    for verbs in ["flange 2 1", "ring 1000", "am 30", "phaser 0.8", "vowel o", "eq -12 6 0"] {
        let mut ins = chain(verbs);
        let out = run(&mut ins, 9600, sine(300.0, 0.2));
        let diff = rms(out.iter().enumerate().map(|(i, o)| o.main.l - sine(300.0, 0.2)(i).l));
        assert!(diff > 0.01, "{verbs} changes the sound ({diff})");
        assert!(out.iter().all(|o| o.main.l.is_finite()));
    }
    // ring mod at 1000 Hz moves a 300 Hz sine to 700/1300 Hz: no 300 Hz left
    let mut ins = chain("ring 1000");
    let out = run(&mut ins, 48000, sine(300.0, 0.2));
    let (mut re, mut im) = (0.0, 0.0);
    for (i, o) in out.iter().enumerate() { let ph = TAU * 300.0 * i as f32 / SR; re += o.main.l * ph.cos(); im += o.main.l * ph.sin(); }
    assert!((re * re + im * im).sqrt() / 48000.0 < 1e-3);

    // stereo widener: a partly stereo input gets wider; mono passes through
    let mut ins = chain("stereo 2");
    let o = run(&mut ins, 2, |_| Stereo { l: 0.5, r: 0.3 });
    assert!((o[1].main.l - 0.6).abs() < 1e-6 && (o[1].main.r - 0.2).abs() < 1e-6);
    let mut ins = chain("stereo 2");
    assert_eq!(run(&mut ins, 1, |_| Stereo::mono(0.4))[0].main, Stereo::mono(0.4));
}

#[test]
fn lfo_gain_is_a_tremolo_and_tgate_chops_in_time() {
    let mut ins = chain("lfo gain 4 0.5");
    let out = run(&mut ins, 24000, |_| Stereo::mono(0.5));
    let (lo, hi) = out.iter().fold((9.0f32, 0.0f32), |(a, b), o| (a.min(o.main.l), b.max(o.main.l)));
    assert!((lo - 0.25).abs() < 0.01 && (hi - 0.75).abs() < 0.01, "{lo} {hi}");

    // tgate 4 with 2 s bars = 2 Hz: 0.25 s on, 0.25 s off
    let mut ins = chain("tgate 4");
    let out = run(&mut ins, 48000, |_| Stereo::mono(0.5));
    let at = |t: f32| out[(t * SR) as usize].main.l;
    assert_eq!((at(0.1), at(0.2), at(0.3), at(0.45), at(0.55), at(0.8)), (0.5, 0.5, 0.0, 0.0, 0.5, 0.0));
}

#[test]
fn pan_law_and_movement() {
    let lr = |verbs: &str, pan: Option<f32>| {
        let mut ins = build(&synth(verbs), pan, &SampleBank::empty());
        run(&mut ins, 1, |_| Stereo::mono(0.5))[0].main
    };
    let c = lr("", Some(0.0));
    assert!((c.l - 0.5).abs() < 1e-5 && (c.r - 0.5).abs() < 1e-5, "a centred voice keeps its level: {c:?}");
    let left = lr("", Some(-1.0));
    assert!((left.l - 0.5 * SQRT_2).abs() < 1e-4 && left.r.abs() < 1e-6);
    // the layer pan wins over `| pan`; a sweep is read at note start
    let p = lr("pan 1", Some(-1.0));
    assert!(p.r.abs() < 1e-6);
    let mut clock = Clock::new(SR);
    clock.bars = 8.0; // halfway through the default 16-bar sweep
    let mut ins = build(&synth("pan -1..1"), None, &SampleBank::empty());
    let o = ins.process(Stereo::mono(0.5), &clock).main;
    assert!((o.l - o.r).abs() < 1e-5, "sweep at centre: {o:?}");

    // autopan / orbit / lfo pan move the voice from side to side
    for verbs in ["autopan 2", "orbit 2", "lfo pan 2 1"] {
        let mut ins = chain(verbs);
        let out = run(&mut ins, 24000, |_| Stereo::mono(0.5));
        let left = out.iter().any(|o| o.main.l > o.main.r * 4.0);
        let right = out.iter().any(|o| o.main.r > o.main.l * 4.0);
        assert!(left && right, "{verbs} sweeps both sides");
    }

    // haas: the right side is the left side 8..30 ms later
    let mut ins = chain("haas 0.5");
    let out = run(&mut ins, 2000, impulse);
    let d = (0.019 * SR).round() as usize;
    assert!(out[0].main.l > 0.9 && out[0].main.r < 0.3);
    assert!(out[d].main.r > 0.9 && out[d].main.l < 0.3, "{:?}", out[d].main);
}

#[test]
fn echo_repeats_at_bar_over_div_and_tail_covers_it() {
    let mut ins = chain("echo 4 0.5"); // 2 s bar / 4 = 0.5 s
    let tail = ins.tail();
    let out = run(&mut ins, (12.0 * SR) as usize, impulse);
    let d = (0.5 * SR) as usize;
    assert_eq!(out[0].main.l, 1.0, "the dry signal is untouched");
    assert!(out[1..d].iter().all(|o| o.bus_only == Stereo::ZERO));
    assert!((out[d].bus_only.l - 0.45).abs() < 1e-6 && (out[2 * d].bus_only.l - 0.225).abs() < 1e-6);
    assert!(out.iter().all(|o| o.main.l == 0.0 || o.bus_only == Stereo::ZERO));
    // after `tail` seconds the repeats are 60 dB down
    let last_loud = out.iter().rposition(|o| o.bus_only.l.abs() > 0.45e-3).unwrap() as f32 / SR;
    assert!(tail >= last_loud && tail < 10.0, "tail {tail} vs last audible repeat {last_loud}");
    assert!(ins.tail() == 0.0, "once everything has died away the note can end");
}

#[test]
fn chorus_goes_to_the_bus_and_ir_adds_a_room() {
    let mut ins = chain("chorus 1");
    let out = run(&mut ins, 4800, sine(440.0, 0.3));
    assert!(out.iter().enumerate().all(|(i, o)| o.main == sine(440.0, 0.3)(i)));
    let wet = rms(out[2400..].iter().map(|o| o.bus_only.l));
    assert!(wet > 0.05, "chorus wet {wet}");
    assert!(out.iter().any(|o| (o.bus_only.l - o.bus_only.r).abs() > 0.01), "panned apart");

    let mut ins = chain("ir hall");
    assert!(ins.tail() > 2.0);
    let out = run(&mut ins, (2.0 * SR) as usize, impulse);
    assert!((out[0].main.l - 0.65).abs() < 1e-5, "dry at 1 - 0.35");
    let late = rms(out[(0.5 * SR) as usize..(0.6 * SR) as usize].iter().map(|o| o.main.l));
    assert!(late > 1e-5, "reverb tail {late}");
    let room_tail = chain("ir room").tail();
    assert!(room_tail < 1.0 && chain("ir cathedral").tail() > 4.0);
}

#[test]
fn vocoder_follows_the_modulator() {
    let mut bank = SampleBank::empty();
    // modulator: 1 s of 800 Hz, then 1 s of silence
    let frames: Vec<f32> = (0..96000).map(|i| if i < 48000 { (TAU * 800.0 * i as f32 / SR).sin() } else { 0.0 }).collect();
    bank.insert("vox", SampleData { sample_rate: SR, channels: vec![frames] });
    let s = synth("vocode vox");
    let mut ins = build(&s, None, &bank);
    let saw = |i: usize| Stereo::mono(0.3 * (2.0 * ((i as f32 * 110.0 / SR) % 1.0) - 1.0));
    let out = run(&mut ins, 96000, saw);
    let talking = rms(out[20000..40000].iter().map(|o| o.main.l));
    let silent = rms(out[70000..90000].iter().map(|o| o.main.l));
    assert!(talking > 0.01 && silent < talking * 0.01, "{talking} {silent}");
    // an unknown sample means no vocoder at all
    let mut plain = build(&synth("vocode nope"), None, &bank);
    assert_eq!(run(&mut plain, 1, |_| Stereo::mono(0.2))[0].main, Stereo::mono(0.2));
}

#[test]
fn extreme_settings_never_produce_nan() {
    let all = "vocode vox | eq 18 -18 18 | vowel i | fold 1 | comp 1 | crush 1 | redux 1 | reson 20 0.995 | flange 10 1 \
               | stereo 2 | ring 8000 | am 8000 | phaser 1 | lfo gain 20 5 square | lfo2 pan 20 5 saw | pan -1..1 | orbit 20 \
               | tgate 32 | echo 0.25 0.9 | haas 1 | autopan 20 | ir cathedral 1 | chorus 1 | drive inf..nan";
    let mut bank = SampleBank::empty();
    bank.insert("vox", SampleData { sample_rate: 8000.0, channels: vec![vec![1.0, -1.0, 1e30], vec![0.0]] });
    let s = synth(all);
    for bar_sec in [0.001, 2.0, 1000.0] {
        let spec = NoteSpec { name: "x", synth: &s, wave: Wave::Saw, freq: 20000.0, dur: 0.1, peak: 1.0, pan: None, prev_freq: None, sr: SR, bar_sec, seed: 7 };
        let mut ins = Inserts::new(&spec, &bank);
        let out = run(&mut ins, 48000, |i| if i < 24000 { Stereo { l: n_val(i) * 10.0, r: -n_val(i + 7) * 10.0 } } else { Stereo::ZERO });
        assert!(out.iter().all(|o| o.main.l.is_finite() && o.main.r.is_finite() && o.bus_only.l.is_finite() && o.bus_only.r.is_finite()));
        assert!(ins.tail().is_finite());
    }
}

/// Deterministic pseudo-noise without shared state.
fn n_val(i: usize) -> f32 { ((i as f32 * 12.9898).sin() * 43758.547).fract() * 2.0 - 1.0 }

/// `cargo test --release -p loom-audio --lib -- bench_stages --ignored --nocapture`
#[test]
#[ignore]
fn bench_stages() {
    for verbs in ["", "pan 0.3", "chorus 0.4", "ir hall", "ir cathedral", "echo 4 0.5", "comp 0.4", "drive 0.5", "crush 4", "phaser 0.5", "eq 3 0 3", "vowel a", "orbit 1"] {
        let mut ins = chain(verbs);
        let clock = Clock::new(SR);
        let n = 480000;
        let t = std::time::Instant::now();
        let mut acc = 0.0f32;
        for i in 0..n { let o = ins.process(Stereo::mono(((i % 97) as f32 / 97.0 - 0.5) * 0.2), &clock); acc += o.main.l + o.bus_only.r; }
        println!("{verbs:>14}: {:.1} ns/sample ({acc})", t.elapsed().as_nanos() as f64 / n as f64);
    }
}
