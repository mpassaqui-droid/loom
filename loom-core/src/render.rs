//! Déterministic sample-accurate renderer — the "unified engine" stone P6-1.
//!
//! This turns a piece string into mono 32-bit PCM floats with **no external
//! dependency** (pure std-only arithmetic), consuming **exactly the events the
//! Rust scheduler already produces** (`Program::parse` + `schedule_bar`), so the
//! same key / microtonality / `key@section` resolution drives the sound as the
//! native apps and the web's Rust-default scheduler.
//!
//! Coverage (v1): oscillators sine/square/saw/triangle, `| unison N` + `| detune`,
//! `| sub` (sine one octave below), `| double` (octave-up layer), a state-variable
//! filter (lp/hp/bp/notch) that is static or linear-swept (`A..B` over the note),
//! `| env ADSR` scaled by `| gate`, amplitude = `vel × gain`. Drums and tokens that
//! don't resolve to a frequency are silent.
//!
//! Out of scope (ignored, documented in `tasks/todo.md` · P6-2/P6-3): wavetables,
//! partials, FM, noise, pluck/bow/reed, bell, grain, sync/pwm, slide/bend, wow,
//! LFOs, inserts (drive/fold/crush/redux), pan (this is mono), sends/buses and
//! the master chain. `Math.fround`-style f32 discipline here mirrors the JS
//! `renderPCM` in `web/index.html` — `tools/parity_test.js` proves numeric DSP
//! parity between the two on a dedicated corpus.

use crate::{Env, Event, FilterType, Param, Program, Synth, Wave};
use std::f32::consts::PI;

/// Default envelope used when a voice has no `| env` (short attack/decay, no
/// clicks at the note gates). Must match the JS mirror.
const DEFAULT_ENV: Env = Env { a: 0.003, d: 0.015, s: 0.92, r: 0.05 };
/// Fallback low-pass cutoff when a filter type is set but no `A..B`/value was
/// given (the web's synth parser also defaults to 800 Hz). Only used when a
/// filter is present.
const DEFAULT_CUTOFF: f32 = 800.0;

/// Render `bars` bars of `src` into mono PCM (f32, -1..1), at sample rate `sr`.
/// The total length is the sum of `round(bar_seconds(bar) * sr)` over the bars —
/// identical to the JS mirror, which also consumes the Rust `bar_seconds`.
pub fn render_program(src: &str, bars: usize, sr_raw: f32) -> Vec<f32> {
    let prog = Program::parse(src);
    let sr = if sr_raw > 0.0 { sr_raw } else { 44100.0 };
    let mut bar_secs: Vec<f32> = Vec::with_capacity(bars);
    let mut bar_frames: Vec<usize> = Vec::with_capacity(bars);
    let mut total = 0usize;
    for b in 0..bars {
        let sec = prog.bar_seconds(b as i64) as f32;
        let frames = (sec * sr).round() as usize;
        bar_secs.push(sec);
        bar_frames.push(frames);
        total += frames;
    }
    let mut buf: Vec<f32> = vec![0.0; total];
    let mut base = 0usize;
    for b in 0..bars {
        let sec = bar_secs[b];
        for e in prog.schedule_bar(b as i64) {
            render_event(&mut buf, base, sec, e, sr);
        }
        base += bar_frames[b];
    }
    buf
}

/// Add one scheduled event to the buffer. Drums, rests and unresolved pitches
/// produce silence.
fn render_event(buf: &mut [f32], base: usize, sec: f32, e: Event, sr: f32) {
    if e.is_drum {
        return;
    }
    let Some(f) = e.freq else { return };
    if !f.is_finite() || f <= 0.0 {
        return;
    }
    let start = base + (e.phase * sec * sr) as usize;
    render_note(buf, start, f, sec * e.dur, e.gain * e.vel, e.wave, &e.synth, sr);
}

/// Raw band-limited(*) oscillator shapes over a 0..1 phase. (*) square/saw/tri
/// are naive (aliasing is fine for a deterministic compute engine).
fn wave(w: Wave, p: f32) -> f32 {
    match w {
        Wave::Sine => sin_pi(p),
        Wave::Square => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Wave::Saw => 2.0 * p - 1.0,
        Wave::Triangle => {
            if p < 0.5 {
                4.0 * p - 1.0
            } else {
                3.0 - 4.0 * p
            }
        }
    }
}

fn sin_pi(p: f32) -> f32 {
    (p * 2.0 * PI).sin()
}

/// Wrap a phase into 0..1.
fn wrap(p: f32) -> f32 {
    p - p.floor()
}

/// ADSR envelope: attack [0,a] -> decay [a,a+d] 1→s -> sustain -> release after
/// the gate [gate, gate+r] (started from the current level, so it is continuous).
fn adsr(t: f32, gate: f32, e: Env) -> f32 {
    let a = e.a.max(0.0);
    let d = e.d.max(0.0);
    let r = e.r.max(0.0);
    let s = e.s.clamp(0.0, 1.0);
    let body = if t < a {
        if a <= 0.0 {
            1.0
        } else {
            t / a
        }
    } else if d <= 0.0 {
        s
    } else if t < a + d {
        1.0 - (1.0 - s) * ((t - a) / d)
    } else {
        s
    };
    if t < gate {
        body
    } else {
        let rn = t - gate;
        if rn < r {
            body * (1.0 - rn / r)
        } else {
            0.0
        }
    }
}

/// The filter cutoff range: `(c0, c1)` with `c0 == c1` meaning a constant cutoff
/// and `c0 != c1` a linear sweep across the note. LFO-type signals are treated
/// as their base level (v1 — LFOs are P6-3).
fn cutoff_span(c: Option<Param>) -> (f32, f32) {
    match c {
        Some(Param::Const(x)) => (x, x),
        Some(Param::Sweep { a, b }) => (a, b),
        Some(Param::Signal { a, .. }) => (a, a),
        None => (DEFAULT_CUTOFF, DEFAULT_CUTOFF),
    }
}

/// Current cutoff at note time `t` (seconds), sweeping linearly over the note.
fn sweep(c0: f32, c1: f32, t: f32, dur: f32) -> f32 {
    if c1 == c0 {
        c0
    } else {
        c0 + (c1 - c0) * (t / dur).min(1.0)
    }
}

/// 2-pole state-variable filter (svf) — one sample. `ic1/ic2` are the integrator
/// states of this note (per-event, reset at the note start).
fn svf(x: f32, fc: f32, q: f32, kind: FilterType, sr: f32, ic1: &mut f32, ic2: &mut f32) -> f32 {
    let fc = fc.clamp(5.0, sr * 0.48);
    let g = (PI * fc / sr).tan();
    let k = 1.0 / q;
    let a1 = 1.0 / (1.0 + g * (g + k));
    let a2 = g * a1;
    let a3 = g * a2;
    let v3 = x - *ic2;
    let v1 = a1 * *ic1 + a2 * v3;
    let v2 = *ic2 + a2 * *ic1 + a3 * v3;
    *ic1 = 2.0 * v1 - *ic1;
    *ic2 = 2.0 * v2 - *ic2;
    let high = x - k * v1 - v2;
    match kind {
        FilterType::Lowpass => v2,
        FilterType::Highpass => high,
        FilterType::Bandpass => v1,
        FilterType::Notch => v2 - high,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_note(buf: &mut [f32], start: usize, freq: f32, dur: f32, amp: f32, w: Wave, s: &Synth, sr: f32) {
    if amp <= 0.0 || amp.is_nan() {
        return;
    }
    let gate = (dur * s.gate.unwrap_or(1.0)).max(0.0);
    let env = s.env.unwrap_or(DEFAULT_ENV);
    let rel = env.r.max(0.0);
    let n = ((gate + rel) * sr).ceil() as usize;
    // --- oscillators ---
    let n_uni = s.unison.unwrap_or(0).max(1) as usize;
    let detune = s.detune.unwrap_or(0.0).max(0.0);
    let sub = s.sub.unwrap_or(0.0).clamp(0.0, 1.0);
    let dub = s.double.unwrap_or(0.0).clamp(0.0, 1.0);
    // --- filter ---
    let ft = s.filter_type;
    let q = s.q.unwrap_or(1.0).clamp(0.1, 20.0);
    let (c0, c1) = cutoff_span(s.cutoff);
    // --- per-note state ---
    let mut uni_ph: Vec<f32> = (0..n_uni).map(|i| i as f32 / (n_uni as f32 + 1.0)).collect();
    let mut sub_ph = 0.0f32;
    let mut dub_ph = 0.0f32;
    let (mut ic1, mut ic2) = (0.0f32, 0.0f32);
    for i in 0..n {
        let ix = start + i;
        if ix >= buf.len() {
            break;
        }
        let t = i as f32 / sr;
        let a = amp * adsr(t, gate, env);
        let mut out = 0.0f32;
        for (k, ph) in uni_ph.iter_mut().enumerate() {
            let ratio = if n_uni > 1 {
                let pos = k as f32 * 2.0 / (n_uni as f32 - 1.0) - 1.0;
                2.0f32.powf(pos * (detune / 1200.0))
            } else {
                1.0
            };
            let fk = freq * ratio;
            out += wave(w, *ph);
            *ph = wrap(*ph + fk / sr);
        }
        out /= n_uni as f32;
        if sub > 0.0 {
            out += sub * sin_pi(sub_ph);
            sub_ph = wrap(sub_ph + (freq / 2.0) / sr);
        }
        if dub > 0.0 {
            out += dub * wave(w, dub_ph);
            dub_ph = wrap(dub_ph + (freq * 2.0) / sr);
        }
        let fout = match ft {
            None => out,
            Some(kind) => svf(out, sweep(c0, c1, t, dur), q, kind, sr, &mut ic1, &mut ic2),
        };
        buf[ix] += a * fout;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    fn r(src: &str, bars: usize) -> Vec<f32> {
        render_program(src, bars, SR)
    }

    /// Energy (sum of squares) of a buffer region.
    fn energy(v: &[f32]) -> f64 {
        v.iter().map(|x| (*x as f64) * (*x as f64)).sum()
    }

    /// Crude "brightness": sum of |d/dn| — high-frequency content makes it grow.
    fn brightness(v: &[f32]) -> f32 {
        v.windows(2).map(|w| (w[1] - w[0]).abs()).sum()
    }

    #[test]
    fn deterministic_and_length() {
        let src = "key: c major\nlead: 1 3 5 1' | saw | lp 800 q 4 | env 0.01 0.1 0.7 0.05";
        let a = r(src, 2);
        let b = r(src, 2);
        assert_eq!(a, b, "same input must render bit-identically");
        // 2 bars at 120 bpm in 4/4 = 4 s.
        assert_eq!(a.len(), (4.0 * SR) as usize);
    }

    #[test]
    fn drums_rests_and_unresolved_are_silent() {
        assert!(r("kick: x . x .\nhat: x x x x", 2).iter().all(|x| *x == 0.0));
        assert!(r("key: c major\nlead: . . . .", 1).iter().all(|x| *x == 0.0));
    }

    #[test]
    fn sine_frequency_is_exact() {
        // One note spanning a whole bar (120 bpm 4/4 = 2 s), flat env (sustain 1)
        // so the waveform is a plain sine of the scheduled frequency.
        let src = "key: c major\nlead: 1 | sine | env 0.001 0.001 1 0.001";
        let prog = Program::parse(src);
        let evs = prog.schedule_bar(0);
        let freqs: Vec<f32> = evs.iter().filter_map(|e| e.freq).collect();
        assert!(!freqs.is_empty(), "melody must resolve a pitch");
        let freq = freqs[0];
        let a = r(src, 1);
        let secs = prog.bar_seconds(0);
        let crossings = a[1..].iter().zip(&a).filter(|(y, x)| **x * **y < 0.0).count();
        let expected = (freq as f64 * secs * 2.0) as i64; // 2 crossings per cycle
        let tol = expected / 10;
        assert_eq!(a.len(), (secs * SR as f64).round() as usize);
        assert!(
            (crossings as i64 - expected).abs() < tol,
            "sine crossings {crossings} vs expected {expected} (freq {freq} Hz)"
        );
    }

    #[test]
    fn attack_starts_quiet_then_reaches_sustain() {
        let src = "key: c major\nlead: 1 | sine | env 0.05 0.05 0.8 0.1";
        let a = r(src, 1);
        let head = a[..200].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let later = a[4000..14000].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(
            later > head * 10.0,
            "attack must start near 0 (head {head}, sustain peak {later})"
        );
    }

    #[test]
    fn lowpass_darkens_a_saw() {
        let bright = r("key: c major\nlead: 1 1 1 1 | saw | lp 15000 q 1", 1);
        let dark = r("key: c major\nlead: 1 1 1 1 | saw | lp 300 q 1", 1);
        assert!(
            brightness(&bright) > brightness(&dark) * 1.5,
            "a low cutoff must remove high harmonics"
        );
    }

    #[test]
    fn sweep_filter_ends_differently_than_it_starts() {
        // Swept upward: starts dark (fc 100), ends bright (fc 15k) -> early energy
        // small, late energy big, versus a constant low cutoff.
        let swept = r("key: c major\nlead: 1 1 | saw | lp 100..15000 q 2 | env 0 0.001 1 0.001", 1);
        let const_dark = r("key: c major\nlead: 1 1 | saw | lp 100 q 2 | env 0 0.001 1 0.001", 1);
        let head_s = energy(&swept[..2000]);
        let tail_s = energy(&swept[swept.len() - 4000..]);
        assert!(
            tail_s > head_s * 1.3, "sweep must open over the note"
        );
        assert!(
            brightness(&swept) > brightness(&const_dark) * 1.8,
            "the swept note must end much brighter than a constant low cutoff"
        );
    }

    #[test]
    fn sub_and_double_add_energy() {
        let plain = r("key: c major\nlead: 1 1 1 1 | sine", 1);
        let sub = r("key: c major\nlead: 1 1 1 1 | sine | sub 0.7", 1);
        let dub = r("key: c major\nlead: 1 1 1 1 | sine | double 0.7", 1);
        assert!(energy(&sub) > energy(&plain) * 1.3, "sub adds a layer");
        assert!(energy(&dub) > energy(&plain) * 1.3, "double adds a layer");
    }

    #[test]
    fn unison_one_equals_no_unison() {
        // `| unison 1` degenerates to the plain oscillator — bit-identical output.
        let plain = r("key: c major\nlead: 1 3 5 | saw", 1);
        let uni1 = r("key: c major\nlead: 1 3 5 | saw | unison 1", 1);
        assert_eq!(plain, uni1);
    }

    #[test]
    fn supersaw_changes_timbre() {
        let plain = r("key: e minor\nlead: 1 3 5 1 | saw", 1);
        let sg = r("key: e minor\nlead: 1 3 5 1 | supersaw", 1);
        assert_ne!(sg, plain, "detuned unison must change the tone");
        assert!(energy(&sg) > 0.0, "the supersaw must not be silent");
        // averaging 7 detuned voices (sum/7) spreads and lowers the energy,
        // exactly like a real supersaw patch — timbre, not level, is the point.
        assert!(energy(&plain) > energy(&sg) * 2.0);
    }

    #[test]
    fn gain_scales_amplitude() {
        let lo = r("key: c major\nlead: 1 1 | sine | gain 0.1", 1);
        let hi = r("key: c major\nlead: 1 1 | sine | gain 0.9", 1);
        let m = |v: &[f32]| v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(
            m(&hi) > m(&lo) * 5.0,
            "higher gain must be louder (max hi {} vs lo {})",
            m(&hi),
            m(&lo)
        );
    }
}
