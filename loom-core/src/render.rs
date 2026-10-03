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

use crate::{Env, Event, FilterType, Fm, Param, Program, Synth, Wave};
use std::f32::consts::PI;

/// Default envelope used when a voice has no `| env` (short attack/decay, no
/// clicks at the note gates). Must match the JS mirror.
pub(crate) const DEFAULT_ENV: Env = Env { a: 0.003, d: 0.015, s: 0.92, r: 0.05 };
/// Fallback low-pass cutoff when a filter type is set but no `A..B`/value was
/// given (the web's synth parser also defaults to 800 Hz). Only used when a
/// filter is present.
pub(crate) const DEFAULT_CUTOFF: f32 = 800.0;

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
pub(crate) fn wave(w: Wave, p: f32) -> f32 {
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

pub(crate) fn sin_pi(p: f32) -> f32 {
    (p * 2.0 * PI).sin()
}

/// Wrap a phase into 0..1.
pub(crate) fn wrap(p: f32) -> f32 {
    p - p.floor()
}

/// ADSR envelope: attack [0,a] -> decay [a,a+d] 1→s -> sustain -> release after
/// the gate [gate, gate+r] (started from the current level, so it is continuous).
pub(crate) fn adsr(t: f32, gate: f32, e: Env) -> f32 {
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
pub(crate) fn cutoff_span(c: Option<Param>) -> (f32, f32) {
    match c {
        Some(Param::Const(x)) => (x, x),
        Some(Param::Sweep { a, b }) => (a, b),
        Some(Param::Curve { n, v, .. }) => (v[0], v[(n as usize).clamp(1, crate::synth::CURVE_POINTS) - 1]),
        Some(Param::Signal { a, .. }) => (a, a),
        None => (DEFAULT_CUTOFF, DEFAULT_CUTOFF),
    }
}

/// Current cutoff at note time `t` (seconds), sweeping linearly over the note.
pub(crate) fn sweep(c0: f32, c1: f32, t: f32, dur: f32) -> f32 {
    if c1 == c0 {
        c0
    } else {
        c0 + (c1 - c0) * (t / dur).min(1.0)
    }
}

/// 2-pole state-variable filter (svf) — one sample. `ic1/ic2` are the integrator
/// states of this note (per-event, reset at the note start).
pub(crate) fn svf(x: f32, fc: f32, q: f32, kind: FilterType, sr: f32, ic1: &mut f32, ic2: &mut f32) -> f32 {
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

/// The note's per-sample oscillator/filter parameters, resolved once per event
/// (shared by the raw `render_program` and the full FX pipeline — ONE definition
/// of "what a note sounds like" in Rust).
pub(crate) struct NoteParam {
    pub(crate) n_uni: usize,
    pub(crate) detune: f32,
    pub(crate) sub: f32,
    pub(crate) dub: f32,
    pub(crate) ft: Option<FilterType>,
    pub(crate) q: f32,
    pub(crate) c0: f32,
    pub(crate) c1: f32,
    // --- P6-3A rich sources & note inserts (all defaulted to "off") ---
    pub(crate) fm: Option<Fm>,
    pub(crate) partials: Option<Vec<f32>>,
    pub(crate) wt: Option<WtKind>,
    pub(crate) pwm: f32,
    pub(crate) sync: f32,
    pub(crate) bell: bool,
    pub(crate) noise: f32,
    pub(crate) am: f32,
    pub(crate) ring: f32,
    pub(crate) drive: f32,
    pub(crate) fold: f32,
    /// `| crush BITS` -> the quantization scale `2^(bits-1)`.
    pub(crate) crush: Option<f32>,
    /// `| redux N` -> the sample-hold period `round(1+20N)` (0 = off).
    pub(crate) redux: usize,
}

impl NoteParam {
    pub(crate) fn of(s: &Synth) -> NoteParam {
        let c = s.cutoff;
        let c0 = cutoff_span(c).0;
        let c1 = cutoff_span(c).1;
        NoteParam {
            n_uni: s.unison.unwrap_or(0).max(1) as usize,
            detune: s.detune.unwrap_or(0.0).max(0.0),
            sub: s.sub.unwrap_or(0.0).clamp(0.0, 1.0),
            dub: s.double.unwrap_or(0.0).clamp(0.0, 1.0),
            ft: s.filter_type,
            q: s.q.unwrap_or(1.0).clamp(0.1, 20.0),
            c0,
            c1,
            fm: s.fm,
            partials: s.partials.clone(),
            wt: s.wt.as_deref().and_then(WtKind::from_str),
            pwm: s.pwm.map(|x| x.clamp(0.05, 1.0)).unwrap_or(0.0),
            sync: s.sync.map_or(0.0, |r| r.max(1.2)),
            bell: s.bell,
            noise: s.noise.unwrap_or(0.0).clamp(0.0, 1.0),
            am: s.am.unwrap_or(0.0),
            ring: s.ring.unwrap_or(0.0),
            drive: s.drive.map_or(0.0, |d| d.base().clamp(0.0, 1.0)),
            fold: s.fold.unwrap_or(0.0).clamp(0.0, 1.0),
            crush: s.crush.map(|b| 2.0f32.powi(b.max(1) as i32 - 1)),
            redux: s.redux.map_or(0, |r| (1.0 + 20.0 * r.clamp(0.0, 1.0)).round() as usize),
        }
    }
}

/// Wavetables (`| wt NAME`) — fixed deterministic harmonic stacks, normalised to
/// ~1 peak (the sum of the amplitude profile). Mirrored identically in JS.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WtKind {
    Organ,
    Warm,
    Rich,
    Soft,
}

impl WtKind {
    pub(crate) fn from_str(s: &str) -> Option<WtKind> {
        match s.to_ascii_lowercase().as_str() {
            "organ" => Some(WtKind::Organ),
            "warm" => Some(WtKind::Warm),
            "rich" => Some(WtKind::Rich),
            "soft" => Some(WtKind::Soft),
            _ => None,
        }
    }
    pub(crate) fn amps(self) -> [f32; 9] {
        match self {
            WtKind::Organ => [1.0, 1.0, 1.0, 1.0, 1.0, 0.9, 0.7, 0.5, 0.3],
            WtKind::Warm => [1.0, 0.5, 0.25, 0.15, 0.1, 0.06, 0.03, 0.02, 0.01],
            WtKind::Rich => [1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2],
            WtKind::Soft => [1.0, 0.5, 0.4, 0.3, 0.2, 0.15, 0.1, 0.08, 0.05],
        }
    }
}

/// The note's per-sample state: oscillator phases + filter integrators + the
/// P6-3A rich-source accumulators (FM modulator/feedback, PWM LFO, hard-sync
/// slave, noise PRNG+HP, AM/Ring mod, sample-hold), reset at the note start and
/// advanced by [`note_sample`] each sample.
pub(crate) struct NoteCtx {
    pub(crate) uni_ph: Vec<f32>,
    pub(crate) sub_ph: f32,
    pub(crate) dub_ph: f32,
    pub(crate) ic1: f32,
    pub(crate) ic2: f32,
    pub(crate) fm_ph: f32,
    pub(crate) fm_prev: f32,
    pub(crate) pwm_ph: f32,
    pub(crate) slave_ph: Vec<f32>,
    pub(crate) ns: u32,
    pub(crate) lp: f32,
    pub(crate) am_ph: f32,
    pub(crate) ring_ph: f32,
    pub(crate) held: f32,
}

impl NoteCtx {
    pub(crate) fn new(p: &NoteParam) -> NoteCtx {
        NoteCtx {
            uni_ph: (0..p.n_uni).map(|i| i as f32 / (p.n_uni as f32 + 1.0)).collect(),
            sub_ph: 0.0,
            dub_ph: 0.0,
            ic1: 0.0,
            ic2: 0.0,
            fm_ph: 0.0,
            fm_prev: 0.0,
            pwm_ph: 0.0,
            slave_ph: vec![0.0; p.n_uni],
            ns: crate::render::noise_seed(0),
            lp: 0.0,
            am_ph: 0.0,
            ring_ph: 0.0,
            held: 0.0,
        }
    }

    /// Like [`NoteCtx::new`] but seeds the noise PRNG from the note's start
    /// sample (deterministic per note; mirrored identically in JS).
    pub(crate) fn seeded(p: &NoteParam, start: usize) -> NoteCtx {
        let mut c = NoteCtx::new(p);
        c.ns = noise_seed(start);
        c
    }
}

/// Deterministic per-note PRNG seed: the note's start sample offset.
pub(crate) fn noise_seed(start: usize) -> u32 {
    (start as u32).wrapping_add(0x9E37_79B9)
}

/// xorshift32 white noise -> one-pole high-pass (~6 kHz "air"), full-scale in
/// 16-bit resolution (the `>>16` slice keeps the JS mirror bit-exact).
fn noise_value(c: &mut NoteCtx, sr: f32) -> f32 {
    let mut s = c.ns;
    s ^= s << 13;
    s ^= s >> 17;
    s ^= s << 5;
    c.ns = s;
    let white = (s >> 16) as f32 * (1.0 / 32768.0) - 1.0;
    let alpha = 1.0 - (-2.0 * PI * 6000.0 / sr).exp();
    c.lp += alpha * (white - c.lp);
    white - c.lp
}

/// Modal `| bell`: 4 inharmonic damped sines (struck-metal timbre).
fn bell_value(t: f32, freq: f32) -> f32 {
    const R: [f32; 4] = [1.0, 1.96, 2.92, 4.2];
    const A: [f32; 4] = [1.0, 0.6, 0.3, 0.15];
    const TAU: [f32; 4] = [1.5, 1.2, 1.0, 0.8];
    let mut v = 0.0;
    for k in 0..4 {
        v += A[k] * (-t / TAU[k]).exp() * sin_pi(R[k] * freq * t);
    }
    v
}

/// Additive `| partials`/`| organ` over the main phase.
fn partials_value(p: &NoteParam, ph: f32) -> f32 {
    let mut v = 0.0;
    if let Some(a) = &p.partials {
        for (i, ai) in a.iter().enumerate() {
            v += ai * sin_pi((i as f32 + 1.0) * ph);
        }
    }
    v
}

/// Wavetable `| wt` evaluation (normalised harmonic stack).
fn wt_value(kind: WtKind, p: f32) -> f32 {
    let a = kind.amps();
    let sum: f32 = a.iter().sum();
    let mut v = 0.0;
    for (i, ai) in a.iter().enumerate() {
        v += ai * sin_pi((i as f32 + 1.0) * p);
    }
    v / sum.max(1e-6)
}

/// Detuned frequency of unison voice `k`.
fn unison_fk(p: &NoteParam, k: usize, freq: f32) -> f32 {
    let ratio = if p.n_uni > 1 {
        let pos = k as f32 * 2.0 / (p.n_uni as f32 - 1.0) - 1.0;
        2.0f32.powf(pos * (p.detune / 1200.0))
    } else {
        1.0
    };
    freq * ratio
}

/// One sample of ONE unison voice's oscillator (rich source): advances the
/// phase and returns the raw (pre-sub/double) wave sample. FM modulates the
/// carrier phase increment; hard-sync snaps the slave to the master wrap; wt /
/// partials / pwm / square replace the base `wave`.
#[allow(clippy::too_many_arguments)]
fn osc_value(p: &NoteParam, w: Wave, c: &mut NoteCtx, k: usize, ph: &mut f32, _t: f32, sr: f32, fk: f32) -> f32 {
    if let Some(fm) = &p.fm {
        let m = sin_pi(c.fm_ph) + fm.fb * c.fm_prev;
        c.fm_prev = m;
        *ph = wrap(*ph + (fk + fm.index * m) / sr);
        c.fm_ph = wrap(c.fm_ph + (fm.ratio * fk) / sr);
        return wave(w, *ph);
    }
    let mnew = wrap(*ph + fk / sr);
    let wrapped = mnew < *ph;
    let v = if p.sync > 0.0 {
        let s = if wrapped {
            mnew
        } else {
            wrap(c.slave_ph[k] + (fk * p.sync) / sr)
        };
        c.slave_ph[k] = s;
        wave(w, s)
    } else if let Some(kind) = p.wt {
        wt_value(kind, *ph)
    } else if p.partials.is_some() {
        partials_value(p, *ph)
    } else if p.pwm > 0.0 {
        let pw = 0.12 + p.pwm * (0.5 + 0.5 * sin_pi(c.pwm_ph));
        if *ph < pw { 1.0 } else { -1.0 }
    } else {
        wave(w, *ph)
    };
    *ph = mnew;
    v
}

/// One sample of the note: rich oscillators (unison/detune + sub/double layers)
/// into the SVF (static or swept) then the note inserts (drive/fold/crush/
/// redux), advancing the note's phase/filter state. With no `| …` feature set
/// every new surface is a no-op — bit-identical to the P6-1 core.
pub(crate) fn note_sample(c: &mut NoteCtx, p: &NoteParam, i: usize, sr: f32, freq: f32, dur: f32, w: Wave) -> f32 {
    let t = i as f32 / sr;
    let mut out = 0.0f32;
    if p.bell {
        out = bell_value(t, freq);
        for (k, ph) in c.uni_ph.iter_mut().enumerate() {
            let fk = unison_fk(p, k, freq);
            *ph = wrap(*ph + fk / sr);
        }
    } else {
        // detach the unison phases so `osc_value` can also touch the shared
        // note-level accumulators (fm/pwm/slave/noise/…) without a double borrow
        let mut uni = std::mem::take(&mut c.uni_ph);
        for (k, ph) in uni.iter_mut().enumerate() {
            let fk = unison_fk(p, k, freq);
            out += osc_value(p, w, c, k, ph, t, sr, fk);
        }
        c.uni_ph = uni;
        out /= p.n_uni as f32;
    }
    if p.pwm > 0.0 {
        c.pwm_ph = wrap(c.pwm_ph + (freq / 4.0) / sr);
    }
    if p.noise > 0.0 {
        out += p.noise * noise_value(c, sr);
    }
    if p.sub > 0.0 {
        out += p.sub * sin_pi(c.sub_ph);
        c.sub_ph = wrap(c.sub_ph + (freq / 2.0) / sr);
    }
    if p.dub > 0.0 {
        out += p.dub * wave(w, c.dub_ph);
        c.dub_ph = wrap(c.dub_ph + (freq * 2.0) / sr);
    }
    if p.am > 0.0 {
        out *= 0.5 + 0.5 * sin_pi(c.am_ph);
        c.am_ph = wrap(c.am_ph + p.am / sr);
    }
    if p.ring > 0.0 {
        out *= sin_pi(c.ring_ph);
        c.ring_ph = wrap(c.ring_ph + p.ring / sr);
    }
    let out = match p.ft {
        None => out,
        Some(kind) => svf(out, sweep(p.c0, p.c1, t, dur), p.q, kind, sr, &mut c.ic1, &mut c.ic2),
    };
    apply_inserts(p, c, i, out)
}

/// Note inserts applied after the filter (drive/fold/crush/redux). Each is a
/// no-op at its default level, keeping the P6-1 path sample-identical.
fn apply_inserts(p: &NoteParam, c: &mut NoteCtx, i: usize, x: f32) -> f32 {
    let mut x = x;
    if p.drive > 0.0 {
        let g = 1.0 + 4.0 * p.drive;
        let comp = 1.0 / (1.0 + 0.6 * p.drive);
        x = (x * g).tanh() * comp;
    }
    if p.fold > 0.0 {
        let q = (x + 1.0).rem_euclid(4.0);
        let folded = 1.0 - (q - 2.0).abs();
        x = (1.0 - p.fold) * x + p.fold * folded;
    }
    if let Some(scale) = p.crush {
        x = (x * scale).round() / scale;
    }
    if p.redux > 1 {
        if i.is_multiple_of(p.redux) {
            c.held = x;
        }
        x = c.held;
    }
    x
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
    let p = NoteParam::of(s);
    let mut c = NoteCtx::seeded(&p, start);
    for i in 0..n {
        let ix = start + i;
        if ix >= buf.len() {
            break;
        }
        let a = amp * adsr(i as f32 / sr, gate, env);
        buf[ix] += a * note_sample(&mut c, &p, i, sr, freq, dur, w);
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

#[cfg(test)]
mod tests_a {
    use super::*;

    const SR: f32 = 44100.0;

    fn r(src: &str, bars: usize) -> Vec<f32> {
        render_program(src, bars, SR)
    }
    fn energy(v: &[f32]) -> f64 {
        v.iter().map(|x| (*x as f64) * (*x as f64)).sum()
    }
    fn peak(v: &[f32]) -> f32 {
        v.iter().fold(0.0f32, |m, x| m.max(x.abs()))
    }

    #[test]
    fn fm_changes_timbre_and_is_deterministic() {
        let plain = r("key: c major\nlead: 1 3 5 | saw | env 0 0.001 1 0.001", 1);
        let fm = r("key: c major\nlead: 1 3 5 | saw | fm 2 3 0 | env 0 0.001 1 0.001", 1);
        let fm2 = r("key: c major\nlead: 1 3 5 | saw | fm 2 3 0 | env 0 0.001 1 0.001", 1);
        assert_eq!(fm, fm2, "FM must be deterministic");
        assert_ne!(fm, plain, "FM index must re-shape the tone");
        assert!(energy(&fm) > 0.0);
    }

    #[test]
    fn fm_feedback_changes_again() {
        let fb = r("key: c major\nlead: 1 | saw | fm 2 4 2 | env 0.001 0.001 1 0.001", 1);
        let no = r("key: c major\nlead: 1 | saw | fm 2 4 0 | env 0.001 0.001 1 0.001", 1);
        assert_ne!(fb, no, "FM feedback must affect the spectrum");
    }

    #[test]
    fn partials_are_additive_overtones() {
        let sine = r("key: c major\nlead: 1 | sine | env 0.001 0.001 1 0.001", 1);
        let organ = r("key: c major\nlead: 1 | organ 888000000 | env 0.001 0.001 1 0.001", 1);
        assert_ne!(organ, sine);
        // 6 equal harmonics instead of 1: ~6x the time-averaged energy
        assert!(energy(&organ) > energy(&sine) * 2.5, "9 drawbar slots add energy");
    }

    #[test]
    fn wt_tables_are_distinct_and_tonal() {
        let a = r("key: c major\nlead: 1 | wt organ | env 0.001 0.001 1 0.001", 1);
        let b = r("key: c major\nlead: 1 | wt warm | env 0.001 0.001 1 0.001", 1);
        let c = r("key: c major\nlead: 1 | wt rich | env 0.001 0.001 1 0.001", 1);
        let d = r("key: c major\nlead: 1 | wt soft | env 0.001 0.001 1 0.001", 1);
        assert!(energy(&a) > 0.0 && energy(&b) > 0.0 && energy(&c) > 0.0 && energy(&d) > 0.0);
        assert!(a != b && b != c && c != d && a != c, "4 tables must sound different");
        // normalised: none louder than the plain sine peak (>~1 might alias; bound it)
        assert!(peak(&b) <= 1.0 + 1e-3 && peak(&d) <= 1.0 + 1e-3);
    }

    #[test]
    fn pwm_changes_square_duty() {
        let sq = r("key: c major\nlead: 1 1 1 1 | square | env 0.001 0.001 1 0.001", 1);
        let pwm = r("key: c major\nlead: 1 1 1 1 | square | pwm 0.8 | env 0.001 0.001 1 0.001", 1);
        assert_ne!(pwm, sq, "a modulated duty must differ from a 50% square");
    }

    #[test]
    fn sync_changes_timbre() {
        let plain = r("key: c major\nlead: 1 3 | saw | env 0.001 0.001 1 0.001", 1);
        let sync = r("key: c major\nlead: 1 3 | saw | sync 3 | env 0.001 0.001 1 0.001", 1);
        assert_ne!(sync, plain, "hard sync must re-shape the saw");
        assert!(energy(&sync) > 0.0);
    }

    #[test]
    fn bell_rings_then_decays() {
        let b = r("key: c major\nlead: 1 | bell | env 0.001 0.001 1 0.5", 1);
        let head = energy(&b[..2205]);
        let tail = energy(&b[b.len() - 8820..]);
        // modal partials die away well before the note ends (longest tau 1.5 s)
        assert!(energy(&b[..4410]) > energy(&b[b.len() - 4410..]) * 3.0, "modal partials decay over the bar");
        let _ = (head, tail);
    }

    #[test]
    fn noise_layer_adds_energy() {
        let plain = r("key: c major\nlead: 1 1 | sine | env 0.02 0.05 0.5 0.05", 1);
        let air = r("key: c major\nlead: 1 1 | sine | noise 0.8 | env 0.02 0.05 0.5 0.05", 1);
        assert!(energy(&air) > energy(&plain) * 1.05, "the air layer adds high-freq energy");
    }

    #[test]
    fn noise_is_deterministic() {
        let a = r("key: c major\nlead: 1 1 1 | saw | noise 1 | env 0.01 0.01 1 0.01", 1);
        let b = r("key: c major\nlead: 1 1 1 | saw | noise 1 | env 0.01 0.01 1 0.01", 1);
        assert_eq!(a, b, "the PRNG must be seeded deterministically per note");
    }

    #[test]
    fn am_wobbles_the_amplitude() {
        let plain = r("key: c major\nlead: 1 1 | sine | env 0.001 0.001 1 0.001", 1);
        let am = r("key: c major\nlead: 1 1 | sine | am 400 | env 0.001 0.001 1 0.001", 1);
        assert_ne!(am, plain, "amplitude modulation must move the level");
    }

    #[test]
    fn ring_adds_sideband_energy() {
        let plain = r("key: c major\nlead: 1 | sine | env 0.001 0.001 1 0.001", 1);
        let ring = r("key: c major\nlead: 1 | sine | ring 220 | env 0.001 0.001 1 0.001", 1);
        assert_ne!(ring, plain);
    }

    #[test]
    fn drive_saturates_loud_input() {
        let loud = r("key: c major\nlead: 1 1 | saw | gain 2 | env 0.001 0.001 1 0.001", 1);
        let driven = r("key: c major\nlead: 1 1 | saw | gain 2 | drive 0.9 | env 0.001 0.001 1 0.001", 1);
        assert!(peak(&driven) < peak(&loud), "a saturated hot saw must not exceed the drive amount");
        assert_ne!(driven, loud);
    }

    #[test]
    fn fold_stays_bounded() {
        for src in ["| fold 1", "| fold 0.5"] {
            let v = r(&format!("key: c major\nlead: 1 1 | saw | gain 3 {src} | env 0.001 0.001 1 0.001"), 1);
            assert!(peak(&v) <= 1.0 + 1e-3, "wavefolding must stay in [-1,1]: {src}");
        }
    }

    #[test]
    fn crush_quantizes_the_signal() {
        let plain = r("key: c major\nlead: 1 | saw | env 0.001 0.001 1 0.001", 1);
        let crushed = r("key: c major\nlead: 1 | saw | crush 3 | env 0.001 0.001 1 0.001", 1);
        assert_ne!(crushed, plain);
        // a quantized ramp stays on one 0.25-grid level for many samples
        let max_run = |v: &[f32]| {
            let (mut best, mut run) = (0, 0);
            let mut prev = f32::NAN;
            for &x in v {
                if x == prev && x.abs() > 1e-9 { run += 1; best = best.max(run); }
                else { run = 1; prev = x; }
            }
            best
        };
        let pc = max_run(&crushed);
        let pp = max_run(&plain);
        assert!(pc >= 8, "3-bit crush must hold exact plateaus (got {pc})");
        assert!(pc > pp, "crushed plateaus {pc} must dwarf the un-crushed {pp}");
    }

    #[test]
    fn redux_creates_sample_holds() {
        let plain = r("key: c major\nlead: 1 | saw | env 0.001 0.001 1 0.001", 1);
        let redux = r("key: c major\nlead: 1 | saw | redux 0.9 | env 0.001 0.001 1 0.001", 1);
        assert_ne!(redux, plain);
        // a heavy sample-hold produces long runs of equal nonzero samples
        let mut max_run = 0; let mut run = 0; let mut prev = f32::NAN;
        for &x in &redux {
            if x == prev { run += 1; max_run = max_run.max(run); } else { run = 1; prev = x; }
        }
        assert!(max_run >= 8, "heavy redux must hold samples for a run of {max_run}");
    }

    #[test]
    fn rich_source_with_filter_and_fx_is_deterministic() {
        // one end-to-end piece exercising several sources + inserts at once
        let src = "key: c major\nlead: 1 3 5 1 | fm 2 3 1 | wt warm | drive 0.3 | fold 0.2 | lp 2000 q 2 | env 0.01 0.05 0.9 0.05";
        let a = r(src, 2);
        let b = r(src, 2);
        assert_eq!(a, b);
        assert!(energy(&a) > 0.0);
    }
}
