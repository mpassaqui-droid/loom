//! P6-2 — the full mixing + mastering pipeline (mono, deterministic, std-only).
//!
//! Builds on the P6-1 note engine (`render::note_sample`, shared): each note's
//! dry sample is deposited into the buffers, then a **phase 2** pass runs the
//! envois + master chain. The topologies are the mono versions of the native
//! `loom-audio` algorithms (`master.rs`, `master/{dynamics,reverb}.rs`,
//! `inserts/{prims,color,space}.rs`) — same constants, same op order — so the
//! JS mirror (`renderFX` in `web/index.html`) can be `Math.fround`-identical
//! and `tools/parity_test.js` proves DSP parity sample by sample.
//!
//! ```text
//! phase 1 (deposits, per event):  dry[ix] += v   (v = note_sample)
//!   | flange (replace) fx += out−v · | phaser fx += chainAP(v)
//!   | echo   fx += repeats (per-note tempo line)
//!   | reverb  rev[ix] += v   ·   | delay  ech[ix] += v·N
//! phase 2 (mix+master):
//!   bus = dry+fx ; rev += bus ; ech += bus
//!   mix = bus·duck + ping(ech)·0.15 + reverb(rev)·0.18
//!   → sat(warmth) → limiter(0.97) → clamp(−1,1)
//! ```
//!
//! Out of scope (documented in `tasks/todo.md`): glue compressor / master EQ,
//! stereo, `send:` named returns, buses — P6-2 keeps the native wiring
//! (the main bus feeds both envois at full level, voice `| reverb`/`| delay`
//! sends on top), with `SPACE_SEND`/`ECHO_SEND` at their native values.

use crate::render::{adsr, note_sample, wrap, NoteCtx, NoteParam, DEFAULT_ENV};
use crate::{Event, Program};
use std::f32::consts::FRAC_1_SQRT_2;

/// Seconds of tail kept after the last note so the reverb/echo rings are heard.
const FX_TAIL_SEC: f32 = 2.0;
/// Native `SPACE_SEND` (`space/100*0.9`, browser default 20).
const SPACE_SEND: f32 = 0.20 * 0.9;
/// Native `ECHO_SEND` (the browser ECHO slider's default 15).
const ECHO_SEND: f32 = 0.15;
/// One WebAudio render quantum — the minimum delay in any feedback loop.
const QUANTUM: usize = 128;
/// The master limiter's ceiling (native `Limiter::new(sr, 0.97)`).
const LIMIT_CEILING: f32 = 0.97;
/// The lookahead of `LoomLim` (native).
const LOOKAHEAD: usize = 128;

/// Render `bars` bars of `src` through the full P6-2 mono pipeline.
/// Deterministic: same input ⇒ bit-identical output. Length =
/// `Σ round(bar_seconds(bar)·sr)` + `FX_TAIL_SEC·sr` tail frames.
pub fn render_full(src: &str, bars: usize, sr_raw: f32) -> Vec<f32> {
    let prog = Program::parse(src);
    let sr = if sr_raw > 0.0 { sr_raw } else { 44100.0 };
    let mut bar_secs: Vec<f32> = Vec::with_capacity(bars);
    let mut bar_frames: Vec<usize> = Vec::with_capacity(bars);
    let mut dry_frames = 0usize;
    for b in 0..bars {
        let sec = prog.bar_seconds(b as i64) as f32;
        let frames = (sec * sr).round() as usize;
        bar_secs.push(sec);
        bar_frames.push(frames);
        dry_frames += frames;
    }
    if dry_frames == 0 {
        return Vec::new();
    }
    let tail = (FX_TAIL_SEC * sr).round() as usize;
    let len = dry_frames + tail;
    let mut dry = vec![0.0f32; len];
    let mut fx = vec![0.0f32; len];
    let mut rev = vec![0.0f32; len];
    let mut ech = vec![0.0f32; len];
    let mut base = 0usize;
    for b in 0..bars {
        let sec = bar_secs[b];
        for e in prog.schedule_bar(b as i64) {
            deposit_event(&mut dry, &mut fx, &mut rev, &mut ech, base, sec, e, sr);
        }
        base += bar_frames[b];
    }
    master_pass(&prog, &dry, &fx, &mut rev, &mut ech, bar_secs, bar_frames, sr, len)
}

// ---------------------------------------------------------------------------
// phase 1 — per-event deposits
// ---------------------------------------------------------------------------

/// Add one event's dry sample + note inserts (flange/phaser/echo) + sends.
#[allow(clippy::too_many_arguments)]
fn deposit_event(dry: &mut [f32], fx: &mut [f32], rev: &mut [f32], ech: &mut [f32], base: usize, sec: f32, e: Event, sr: f32) {
    if e.is_drum {
        return;
    }
    let Some(f) = e.freq else { return };
    if !f.is_finite() || f <= 0.0 {
        return;
    }
    let start = base + (e.phase * sec * sr) as usize;
    let s = &e.synth;
    let dur = sec * e.dur;
    let amp = e.gain * e.vel;
    if amp <= 0.0 || amp.is_nan() {
        return;
    }
    let gate = (dur * s.gate.unwrap_or(1.0)).max(0.0);
    let env = s.env.unwrap_or(DEFAULT_ENV);
    let rel = env.r.max(0.0);
    let n = ((gate + rel) * sr).ceil() as usize;
    let p = NoteParam::of(s);
    let mut c = NoteCtx::seeded(&p, start);
    // per-note FX
    let mut echo = s.echo.map(|ec| NoteEcho::new(ec, sec, sr));
    let mut flange = s.flange.map(|fl| Flange::new(fl.rate, fl.depth, sr));
    let mut phaser = s.phaser.map(|level| Phaser::new(level.max(0.0), sr));
    let rev_lvl = s.reverb.unwrap_or(0.0).clamp(0.0, 1.0);
    let delay_lvl = s.delay.unwrap_or(0.0).clamp(0.0, 1.0);
    let echo_tail = echo.as_ref().map(NoteEcho::tail).unwrap_or(0.0);
    // the echo rings beyond the note; the buffer bounds the tail
    let n_max = ((gate + rel + echo_tail).max(0.0) * sr).ceil() as usize;
    for i in 0..n_max {
        let ix = start + i;
        if ix >= dry.len() {
            break;
        }
        let active = i < n;
        let v = if active {
            let a = amp * adsr(i as f32 / sr, gate, env);
            a * note_sample(&mut c, &p, i, sr, f, dur, e.wave)
        } else {
            0.0
        };
        if active {
            dry[ix] += v;
            if rev_lvl > 0.0 {
                rev[ix] += v;
            }
            if delay_lvl > 0.0 {
                ech[ix] += v * delay_lvl;
            }
        }
        // note inserts (native pre-chain order: flange → phaser → echo)
        let mut pre = v;
        if let Some(fl) = &mut flange {
            if active {
                let y = fl.process(pre);
                fx[ix] += y - pre; // flange replaces the dry
                pre = y;
            }
        }
        if let Some(ph) = &mut phaser {
            if active {
                let ch = ph.chain(pre);
                fx[ix] += ch; // phaser adds its chain above the dry
                pre += ch;
            }
        }
        if let Some(ec) = &mut echo {
            let y = ec.process(pre);
            fx[ix] += y;
        }
    }
}

/// Per-note tempo-synced feedback delay (`| echo DIV FB`), a mono port of the
/// native note echo (`inserts/space.rs`).
struct NoteEcho {
    ring: Ring,
    fb: f32,
    delay_s: f32,
}

impl NoteEcho {
    fn new(ec: crate::Echo, bar_sec: f32, sr: f32) -> NoteEcho {
        let delay = (bar_sec / ec.div.max(0.01)).clamp(0.01, 2.0);
        let ring = Ring::new(((delay * sr).round() as usize).max(1));
        let delay_s = ring.len() as f32 / sr;
        NoteEcho { ring, fb: ec.fb.clamp(0.0, 0.95), delay_s }
    }

    fn process(&mut self, x: f32) -> f32 {
        let fb = self.fb;
        let y = self.ring.out();
        self.ring.push(undenormal(0.45 * x + fb * y));
        y
    }

    /// Seconds the repeats keep ringing (60 dB down, capped 30 s).
    fn tail(&self) -> f32 {
        let repeats = if self.fb > 0.0 { (1e-3f32.ln() / self.fb.ln()).ceil() } else { 0.0 };
        (self.delay_s * (repeats + 1.0)).min(30.0)
    }
}

/// `| flange RATE DEPTH` — a 2..62-sample swept delay with 0.5 feedback,
/// `(x + dl)·0.6` (mono port of native `color::Flange`).
struct Flange {
    buf: [f32; 128],
    pos: usize,
    lfo_ph: f32,
    rate: f32,
    depth: f32,
    sr: f32,
}

impl Flange {
    fn new(rate: f32, depth: f32, sr: f32) -> Flange {
        Flange { buf: [0.0; 128], pos: 0, lfo_ph: 0.0, rate: rate.max(0.0), depth: depth.clamp(0.0, 1.0), sr }
    }

    fn process(&mut self, x: f32) -> f32 {
        let lfo = crate::render::sin_pi(self.lfo_ph);
        self.lfo_ph = wrap(self.lfo_ph + self.rate / self.sr);
        let ds = 2.0 + (lfo * 0.5 + 0.5) * self.depth * 60.0;
        let pos = self.pos;
        let ri = pos as f32 - ds;
        let r0 = ri.floor();
        let fr = ri - r0;
        let i0 = (r0 as isize).rem_euclid(128) as usize;
        let i1 = (i0 + 1) % 128;
        let dl = self.buf[i0] + (self.buf[i1] - self.buf[i0]) * fr;
        self.buf[pos] = undenormal(x + dl * 0.5);
        self.pos = (pos + 1) % 128;
        (x + dl) * 0.6
    }
}

/// `| phaser N` — dry + 4 allpasses at 400/750/1100/1450 Hz swept ±900 Hz by a
/// sine at `0.25 + 1.4N` Hz, coefficients refreshed every 32 samples (mono port
/// of native `color::Phaser` — returns the allpass chain of `x`).
struct Phaser {
    aps: Vec<Biquad>,
    lfo_ph: f32,
    count: u32,
    rate: f32,
    sr: f32,
}

impl Phaser {
    fn new(level: f32, sr: f32) -> Phaser {
        let aps = (0..4).map(|k| Biquad::new(BiquadKind::Allpass, sr, 400.0 + k as f32 * 350.0, 0.7)).collect();
        Phaser { aps, lfo_ph: 0.0, count: 0, rate: 0.25 + level * 1.4, sr }
    }

    /// Advance the LFO, refresh the filters every 32 samples, chain the input.
    fn chain(&mut self, x: f32) -> f32 {
        let l = crate::render::sin_pi(self.lfo_ph);
        self.lfo_ph = wrap(self.lfo_ph + self.rate / self.sr);
        if self.count.is_multiple_of(32) {
            for (k, ap) in self.aps.iter_mut().enumerate() {
                ap.set(BiquadKind::Allpass, self.sr, 400.0 + k as f32 * 350.0 + 900.0 * l, 0.7);
            }
        }
        self.count = self.count.wrapping_add(1);
        let mut v = x;
        for ap in self.aps.iter_mut() {
            v = ap.process(v);
        }
        v
    }
}

// ---------------------------------------------------------------------------
// phase 2 — envois (reverb, ping-pong) + duck + mastering
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn master_pass(prog: &Program, dry: &[f32], fx: &[f32], rev: &mut [f32], ech: &mut [f32], bar_secs: Vec<f32>, bar_frames: Vec<usize>, sr: f32, len: usize) -> Vec<f32> {
    let mix = &prog.mix;
    let room = &mix.room;
    let mut verb = Reverb::new(sr, room);
    let mut ping = PingEcho::new(sr);
    let mut sat = Saturator { prev: 0.0 };
    let mut lim = Limiter::new(sr, LIMIT_CEILING);
    let mut duck_gain = 1.0f32;
    let mut coef = 0.0f32;
    // duck trigger positions (any drum hit — kick idiom; drum sounds are P6-3)
    let mut trig: Vec<usize> = Vec::new();
    if mix.duck > 0.0 {
        let mut base = 0usize;
        for b in 0..bar_frames.len() {
            let sec = bar_secs[b];
            for e in prog.schedule_bar(b as i64) {
                if e.is_drum {
                    trig.push(base + (e.phase * sec * sr) as usize);
                }
            }
            base += bar_frames[b];
        }
        trig.sort_unstable();
    }
    let mut tr = 0usize;
    // duck glide coefficient, set per bar (τ = min(0.12, bar/8))
    let mut bar_idx = 0usize;
    let mut bar_start = 0usize;
    if mix.duck > 0.0 {
        coef = duck_coef(bar_secs[0], sr);
    }
    let mut out = vec![0.0f32; len];
    for i in 0..len {
        while tr < trig.len() && trig[tr] <= i {
            duck_gain = (1.0 - mix.duck).max(0.05);
            tr += 1;
        }
        if bar_idx + 1 < bar_frames.len() && i >= bar_start + bar_frames[bar_idx] {
            bar_start += bar_frames[bar_idx];
            bar_idx += 1;
            if mix.duck > 0.0 {
                coef = duck_coef(bar_secs[bar_idx], sr);
            }
        }
        duck_gain += (1.0 - duck_gain) * coef;
        let b = dry[i] + fx[i];
        let mut m = b * duck_gain;
        m += ECHO_SEND * ping.process(ech[i] + b);
        m += SPACE_SEND * verb.process(rev[i] + b);
        // mastering: LoomSat → LoomLim → clamp
        m = sat.process(m, mix.warmth);
        m = lim.process(m);
        out[i] = m.clamp(-1.0, 1.0);
    }
    out
}

fn duck_coef(bar_sec: f32, sr: f32) -> f32 {
    let tau = (bar_sec / 8.0).min(0.12);
    1.0 - (-1.0 / (tau * sr)).exp()
}

/// The master reverb (native `Reverb::new`): Schroeder by default, FDN with
/// `room: fdn DECAY DAMP`.
enum Reverb {
    Schroeder(Schroeder),
    Fdn(Fdn),
}

impl Reverb {
    fn new(sr: f32, room: &crate::Room) -> Reverb {
        if room.kind.as_deref() == Some("fdn") {
            let decay = room.decay.unwrap_or(0.84).clamp(0.0, 0.97);
            let damp = room.damp.unwrap_or(4200.0);
            Reverb::Fdn(Fdn::new(sr, decay, damp))
        } else {
            Reverb::Schroeder(Schroeder::new(sr))
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        match self {
            Reverb::Schroeder(ch) => ch.process(x),
            Reverb::Fdn(ch) => ch.process(x),
        }
    }
}

fn loop_len(sr: f32, secs: f32) -> usize {
    ((secs * sr).round() as usize).max(QUANTUM)
}

fn loop_lowpass(sr: f32, freq: f32) -> Biquad {
    Biquad::new(BiquadKind::Lowpass, sr, freq, FRAC_1_SQRT_2)
}

/// Schroeder/Moorer: 4 damped feedback combs in parallel → 2 allpasses (mono).
struct Schroeder {
    combs: Vec<(Ring, Biquad)>,
    aps: [Allpass; 2],
}

impl Schroeder {
    fn new(sr: f32) -> Schroeder {
        let combs =
            [0.0297, 0.0371, 0.0411, 0.0437].iter().map(|&t| (Ring::new(loop_len(sr, t)), loop_lowpass(sr, 3800.0))).collect();
        Schroeder { combs, aps: [Allpass::new(loop_len(sr, 0.005), 0.5), Allpass::new(loop_len(sr, 0.0017), 0.5)] }
    }

    fn process(&mut self, x: f32) -> f32 {
        let mut sum = 0.0;
        for (ring, lp) in &mut self.combs {
            let d = ring.out();
            sum += d;
            ring.push(undenormal(x + 0.8 * lp.process(d)));
        }
        let y = self.aps[0].process(sum);
        self.aps[1].process(y)
    }
}

const FDN_TIMES: [f32; 8] = [0.0431, 0.0530, 0.0617, 0.0727, 0.0843, 0.0937, 0.1051, 0.1153];

/// Jot feedback delay network: 8 lines mixed by a Hadamard matrix scaled by
/// `decay`, each line damped by a lowpass, then 2 allpasses (mono).
struct Fdn {
    lines: Vec<Ring>,
    lps: Vec<Biquad>,
    decay: f32,
    aps: [Allpass; 2],
}

impl Fdn {
    fn new(sr: f32, decay: f32, damp: f32) -> Fdn {
        Fdn {
            lines: FDN_TIMES.iter().map(|&t| Ring::new(loop_len(sr, t))).collect(),
            lps: (0..8).map(|_| loop_lowpass(sr, damp)).collect(),
            decay,
            aps: [Allpass::new(loop_len(sr, 0.0047), 0.5), Allpass::new(loop_len(sr, 0.0019), 0.5)],
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let norm = 1.0 / 8f32.sqrt();
        let mut out = 0.0;
        let mut v = [0.0f32; 8];
        for (i, value) in v.iter_mut().enumerate() {
            let d = self.lines[i].out();
            out += d;
            *value = self.lps[i].process(d);
        }
        // fast Walsh–Hadamard transform = multiplying by the ±1 Hadamard matrix
        let mut h = 1;
        while h < 8 {
            for pair in v.chunks_exact_mut(h * 2) {
                let (a, b) = pair.split_at_mut(h);
                for (x, y) in a.iter_mut().zip(b.iter_mut()) {
                    let (av, bv) = (*x, *y);
                    *x = av + bv;
                    *y = av - bv;
                }
            }
            h *= 2;
        }
        for (i, line) in self.lines.iter_mut().enumerate() {
            line.push(undenormal(norm * x + v[i] * norm * self.decay));
        }
        let y = self.aps[0].process(out);
        self.aps[1].process(y)
    }
}

/// The master ping-pong delay, mono: one 0.2 s feedback line (native
/// `reverb::PingPong` runs two cross-fed lines for the stereo L/R).
struct PingEcho {
    line: Ring,
}

impl PingEcho {
    fn new(sr: f32) -> PingEcho {
        PingEcho { line: Ring::new(loop_len(sr, 0.2)) }
    }

    fn process(&mut self, x: f32) -> f32 {
        let d = self.line.out();
        self.line.push(undenormal(x + 0.32 * d));
        d
    }
}

/// The `LoomSat` warmth saturator (mono): tanh with a cheap 2× oversample trick
/// (average of the shaped half-sample midpoint and the shaped sample).
struct Saturator {
    prev: f32,
}

impl Saturator {
    fn process(&mut self, x: f32, drive: f32) -> f32 {
        let g = 1.0 + drive * 4.0;
        let comp = 1.0 / (1.0 + drive * 0.6);
        let mid = (self.prev + x) * 0.5;
        self.prev = x;
        (((mid * g).tanh() + (x * g).tanh()) * 0.5) * comp
    }
}

/// The `LoomLim` lookahead brick-wall limiter (mono): 128-sample lookahead,
/// instant attack, slow release, holding the gain at the minimum any buffered
/// sample still needs so the output never exceeds the ceiling.
struct Limiter {
    ceiling: f32,
    buf: [f32; LOOKAHEAD],
    need: [f32; LOOKAHEAD],
    pos: usize,
    g: f32,
    release: f32,
    min: f32,
    min_pos: usize,
}

impl Limiter {
    fn new(sr: f32, ceiling: f32) -> Limiter {
        let release = 1.0 - (1.0 - 0.0009f32).powf(48000.0 / sr);
        Limiter { ceiling, buf: [0.0; LOOKAHEAD], need: [1.0; LOOKAHEAD], pos: 0, g: 1.0, release, min: 1.0, min_pos: 0 }
    }

    fn process(&mut self, x: f32) -> f32 {
        let peak = x.abs();
        let tg = if peak > self.ceiling { self.ceiling / peak } else { 1.0 };
        if tg < self.g {
            self.g = tg;
        } else {
            self.g += (1.0 - self.g) * self.release;
        }
        let p = self.pos;
        let g = self.g.min(self.min).min(tg);
        let out = self.buf[p] * g;
        self.buf[p] = x;
        self.need[p] = tg;
        if tg <= self.min {
            self.min = tg;
            self.min_pos = p;
        } else if self.min_pos == p {
            let (i, m) = self.need.iter().enumerate().fold((p, tg), |best, (i, &v)| if v < best.1 { (i, v) } else { best });
            self.min = m;
            self.min_pos = i;
        }
        self.pos = (p + 1) % LOOKAHEAD;
        out
    }
}

// ---------------------------------------------------------------------------
// primitives (mono ports of native `inserts/prims.rs` + `dsp.rs::Biquad`)
// ---------------------------------------------------------------------------

/// A fixed delay of `len` samples at the write pointer (read `out()` then
/// `push()`).
struct Ring {
    buf: Vec<f32>,
    pos: usize,
}

impl Ring {
    fn new(len: usize) -> Ring {
        Ring { buf: vec![0.0; len.max(1)], pos: 0 }
    }

    fn out(&self) -> f32 {
        self.buf[self.pos]
    }

    fn len(&self) -> usize {
        self.buf.len()
    }

    fn push(&mut self, x: f32) {
        self.buf[self.pos] = x;
        self.pos += 1;
        if self.pos == self.buf.len() {
            self.pos = 0;
        }
    }
}

struct Allpass {
    ring: Ring,
    g: f32,
}

impl Allpass {
    fn new(len: usize, g: f32) -> Allpass {
        Allpass { ring: Ring::new(len), g }
    }

    fn process(&mut self, x: f32) -> f32 {
        let d = self.ring.out();
        self.ring.push(undenormal(x + self.g * d));
        d - self.g * x
    }
}

/// Kill denormals (tiny numbers that make feedback loops slow on x86) —
/// must match the JS mirror so the samples stay bit-identical.
fn undenormal(x: f32) -> f32 {
    if x.abs() < 1e-18 {
        0.0
    } else {
        x
    }
}

#[derive(Clone, Copy, PartialEq)]
enum BiquadKind {
    Lowpass,
    Allpass,
}

/// RBJ biquad in direct form 1, with the native denormal guard on `y1`.
#[derive(Clone)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    fn new(kind: BiquadKind, sr: f32, freq: f32, q: f32) -> Biquad {
        let mut b = Biquad { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0, x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 };
        b.set(kind, sr, freq, q);
        b
    }

    fn set(&mut self, kind: BiquadKind, sr: f32, freq: f32, q: f32) {
        let freq = freq.clamp(10.0, sr * 0.49);
        let q = q.max(1e-4);
        let w = 2.0 * std::f32::consts::PI * freq / sr;
        let (cw, sw) = (w.cos(), w.sin());
        let alpha = sw / (2.0 * q);
        let (b0, b1, b2, a0, a1, a2) = match kind {
            BiquadKind::Lowpass => (
                (1.0 - cw) / 2.0,
                1.0 - cw,
                (1.0 - cw) / 2.0,
                1.0 + alpha,
                -2.0 * cw,
                1.0 - alpha,
            ),
            BiquadKind::Allpass => (1.0 - alpha, -2.0 * cw, 1.0 + alpha, 1.0 + alpha, -2.0 * cw, 1.0 - alpha),
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = if y.abs() < 1e-20 { 0.0 } else { y };
        self.y1
    }
}

// ---------------------------------------------------------------------------
// WAV export
// ---------------------------------------------------------------------------

/// Render through the full pipeline, normalize the peak to 0.97 (if non-silent)
/// and encode as a 16-bit mono WAV (44-byte header + PCM). Deterministic.
pub fn render_wav(src: &str, bars: usize, sr_raw: f32) -> Vec<u8> {
    let pcm = render_full(src, bars, sr_raw);
    let sr = if sr_raw > 0.0 { sr_raw } else { 44100.0 };
    let sr_u = sr.round() as u32;
    let peak = pcm.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    let norm = if peak > 1e-9 { 0.97 / peak } else { 1.0 };
    let n = pcm.len();
    let mut out = Vec::with_capacity(44 + n * 2);
    out.extend(b"RIFF");
    out.extend(((36 + n * 2) as u32).to_le_bytes());
    out.extend(b"WAVE");
    out.extend(b"fmt ");
    out.extend((16u32).to_le_bytes());
    out.extend((1u16).to_le_bytes()); // PCM
    out.extend((1u16).to_le_bytes()); // mono
    out.extend(sr_u.to_le_bytes());
    out.extend((sr_u * 2).to_le_bytes()); // byte rate
    out.extend((2u16).to_le_bytes()); // block align
    out.extend((16u16).to_le_bytes()); // 16 bit
    out.extend(b"data");
    out.extend(((n * 2) as u32).to_le_bytes());
    for &x in &pcm {
        let v = (x * norm).clamp(-1.0, 1.0);
        let s = ((v * 32767.0).round() as i32).clamp(-32768, 32767) as i16;
        out.extend(s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    fn r(src: &str, bars: usize) -> Vec<f32> {
        render_full(src, bars, SR)
    }

    fn energy(v: &[f32]) -> f64 {
        v.iter().map(|x| (*x as f64) * (*x as f64)).sum()
    }

    fn peak(v: &[f32]) -> f32 {
        v.iter().fold(0.0f32, |m, x| m.max(x.abs()))
    }

    #[test]
    fn full_deterministic_and_length() {
        let src = "key: c major\nlead: 1 3 5 1 | saw | lp 800 q 4 | env 0.01 0.1 0.7 0.05";
        let a = r(src, 2);
        let b = r(src, 2);
        assert_eq!(a, b, "same input must render bit-identically");
        // 2 bars at 120 bpm 4/4 = 4 s + 2 s of tail = 6 s.
        assert_eq!(a.len(), (6.0 * SR) as usize);
    }

    #[test]
    fn drums_only_stays_silent() {
        let out = r("kick: x . x .\nhat: x x x x", 2);
        assert!(out.iter().all(|x| *x == 0.0), "no note ⇒ no sound, even with the master chain");
    }

    #[test]
    fn echo_leaves_audible_repeats_that_decay() {
        // `| echo 2 .45` beats back every half bar; the repeats ring into the tail.
        let src = "key: c major\nlead: 1 | sine | echo 2 0.45 | env 0.01 0.05 0.9 0.02";
        let out = r(src, 1);
        let note = &out[0..(2.0 * SR) as usize / 2];
        let tail = &out[(2.0 * SR) as usize / 2..(4.0 * SR) as usize];
        assert!(energy(tail) > energy(note) * 1e-4, "echo repeats must ring past the note: {} vs {}", energy(tail), energy(note));
        let head = &tail[0..(1.0 * SR) as usize];
        let far = &tail[(1.0 * SR) as usize..(2.0 * SR) as usize];
        assert!(energy(far) < energy(head), "and the repeats decay: {} vs {}", energy(far), energy(head));
    }

    #[test]
    fn reverb_send_adds_a_decaying_tail() {
        let plain = r("key: c major\nlead: 1 3 5 1 | saw | env 0.01 0.05 0.9 0.02", 1);
        let wet = r("key: c major\nlead: 1 3 5 1 | saw | reverb 0.8 | env 0.01 0.05 0.9 0.02", 1);
        assert_ne!(wet, plain, "the reverb send must change the render");
        let dry_frames = (2.0 * SR) as usize;
        let head = &wet[dry_frames..dry_frames + (1.0 * SR) as usize];
        let far = &wet[dry_frames + (1.0 * SR) as usize..];
        assert!(energy(head) > 0.0, "reverb tail must be audible");
        assert!(energy(far) < energy(head) * 0.9, "and it decays: {} vs {}", energy(far), energy(head));
    }

    #[test]
    fn duck_pumps_after_each_kick_and_recovers() {
        let src = "key: c major\nlead: 1 1 1 1 | saw | env 0.005 0.05 0.9 0.02";
        let no = r(src, 1);
        let dup = r(&format!("duck: 0.7\nkick: x x x x\n{}", src), 1);
        assert_ne!(no, dup);
        // right after a kick the mix is pumped down...
        let h = (0.02 * SR) as usize; // just after the 1st beat kick
        let a = &no[h..h + (0.08 * SR) as usize];
        let b = &dup[h..h + (0.08 * SR) as usize];
        assert!(energy(b) < energy(a) * 0.6, "duck must lower early energy: {} vs {}", energy(b), energy(a));
        // ...and recovers well before the next kick (0.5 s later)
        let h2 = (0.36 * SR) as usize;
        let a2 = &no[h2..h2 + (0.08 * SR) as usize];
        let b2 = &dup[h2..h2 + (0.08 * SR) as usize];
        assert!(energy(b2) > energy(a2) * 0.5, "duck must have recovered: {} vs {}", energy(b2), energy(a2));
    }

    #[test]
    fn warmth_drives_saturation() {
        let cool = r("warmth: 0\nkey: c minor\nlead: 1 3 1 5 | saw | gain 1.2 | env 0.01 0.1 0.9 0.05", 1);
        let warm = r("warmth: 0.9\nkey: c minor\nlead: 1 3 1 5 | saw | gain 1.2 | env 0.01 0.1 0.9 0.05", 1);
        assert_ne!(cool, warm, "more drive must change the tone");
        // the tanh soft-clip keeps per-sample output bounded
        assert!(peak(&warm) <= 1.0 + 1e-6, "saturator must not exceed +1: {}", peak(&warm));
        assert!(warm.iter().all(|x| x.is_finite()), "no NaN from the master chain");
    }

    #[test]
    fn limiter_holds_the_ceiling_on_loud_mixes() {
        // overlapping loud saws at gain 2: the lookahead limiter must catch them.
        let loud = r("warmth: 1\nkey: c major\nlead: 1 1 1 1 | saw | gain 2 | env 0.2 0.3 1 0.2", 2);
        assert!(loud.iter().all(|x| x.is_finite()));
        assert!(peak(&loud) <= 0.97 + 1e-3, "limiter must hold the ceiling: {}", peak(&loud));
        assert!(loud.iter().all(|x| x.abs() <= 0.97 + 1e-3), "and clamp the samples");
    }

    #[test]
    fn flange_replaces_and_phaser_adds() {
        let base = "key: c major\nlead: 1 3 5 1 | saw | env 0.01 0.05 0.8 0.02";
        let fl = r(&format!("{base} | flange 0.5 0.8"), 1);
        let ph = r(&format!("{base} | phaser 0.7"), 1);
        let dry = r(base, 1);
        assert_ne!(fl, dry, "flange must change the tone");
        assert_ne!(ph, dry, "phaser must change the tone");
        assert!(fl.iter().all(|x| x.is_finite()) && ph.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn delay_send_feeds_the_ping_pong() {
        let plain = r("key: c major\nlead: 1 | sine | env 0.01 0.05 0.9 0.02", 1);
        let echoed = r("key: c major\nlead: 1 | sine | delay 0.5 | env 0.01 0.05 0.9 0.02", 1);
        assert_ne!(plain, echoed, "| delay must add echoes");
        // 0.2 s later (the ping-pong period) there is clearly audible energy
        let f = (0.2 * SR) as usize;
        let w = &echoed[f..f + (0.1 * SR) as usize];
        assert!(energy(w) > 0.0, "the 0.2 s echo must ring");
    }

    #[test]
    fn wav_header_and_decode() {
        let src = "key: a minor\nlead: 1 3 5 3 | saw | reverb 0.3 | env 0.01 0.1 0.8 0.05";
        let bars = 1;
        let wav = render_wav(src, bars, SR);
        assert_eq!(wav.len(), 44 + 2 * r(src, bars).len(), "header + 16-bit data");
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(u16::from_le_bytes(wav[20..22].try_into().unwrap()), 1, "PCM");
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1, "mono");
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), SR as u32, "sample rate @24");
        assert_eq!(u16::from_le_bytes(wav[32..34].try_into().unwrap()), 2, "block align");
        assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16, "16 bit");
        assert_eq!(&wav[36..40], b"data");
        let data_bytes = u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize;
        assert_eq!(data_bytes, wav.len() - 44, "data size = payload bytes");
        let n = data_bytes / 2;
        // decode: no clipping beyond the normalized ceiling
        let mut peak_i = 0u16;
        for k in 0..n {
            let s = i16::from_le_bytes([wav[44 + 2 * k], wav[45 + 2 * k]]);
            peak_i = peak_i.max(s.unsigned_abs());
        }
        assert!(peak_i <= 31784, "normalized peak ≤ round(0.97*32767) = {}, got {}", 31784, peak_i);
    }
}
