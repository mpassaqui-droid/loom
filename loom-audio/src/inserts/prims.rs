//! Small private building blocks shared by the insert chain and the master:
//! a fixed delay ring, WebAudio-style oscillators, the stereo panner law, and
//! a couple of helpers. (Generic ones live in `crate::dsp`; these are the bits
//! only the effects need.)

use crate::note::Stereo;
use std::f32::consts::{FRAC_PI_2, SQRT_2, TAU};

/// WebAudio's minimum delay inside a feedback loop: one render quantum.
pub(crate) const QUANTUM: usize = 128;

/// Kill denormals (tiny numbers that make feedback loops slow on x86).
#[inline]
pub(crate) fn undenormal(x: f32) -> f32 { if x.abs() < 1e-18 { 0.0 } else { x } }

/// WebAudio's lowpass/highpass `Q` is in dB; `dsp::Biquad` wants a linear Q.
pub(crate) fn q_from_db(q_db: f32) -> f32 { 10f32.powf(q_db.clamp(-30.0, 30.0) / 20.0) }

/// A fixed delay of `len` samples. Read `out()` (the sample pushed `len`
/// pushes ago), then `push()` the new one.
#[derive(Clone, Debug)]
pub(crate) struct Ring { buf: Vec<f32>, pos: usize }

impl Ring {
    pub fn new(len: usize) -> Ring { Ring { buf: vec![0.0; len.max(1)], pos: 0 } }
    /// Seconds → samples, never shorter than `min` samples.
    pub fn secs(sr: f32, secs: f32, min: usize) -> Ring { Ring::new(((secs * sr).round() as usize).max(min)) }
    #[inline] pub fn out(&self) -> f32 { self.buf[self.pos] }
    #[inline]
    pub fn push(&mut self, x: f32) {
        self.buf[self.pos] = x;
        self.pos += 1;
        if self.pos == self.buf.len() { self.pos = 0; }
    }
    pub fn len(&self) -> usize { self.buf.len() }
}

/// A Schroeder allpass (the JS `allpass(time, g)` helper): `d = x + g·d_out`, `y = -g·x + d_out`.
#[derive(Clone, Debug)]
pub(crate) struct Allpass { ring: Ring, g: f32 }

impl Allpass {
    pub fn new(len: usize, g: f32) -> Allpass { Allpass { ring: Ring::new(len), g } }
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let d = self.ring.out();
        self.ring.push(undenormal(x + self.g * d));
        d - self.g * x
    }
}

/// The waveforms of a WebAudio `OscillatorNode`, with WebAudio's phase:
/// every shape starts at its zero crossing (square starts high).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape { Sine, Square, Saw, Tri }

impl Shape {
    /// The JS `SHP` table used by `| lfo`: `rand` has no oscillator type, so it plays a square.
    pub fn from_word(w: &str) -> Shape {
        match w {
            "tri" | "triangle" => Shape::Tri,
            "saw" | "sawtooth" => Shape::Saw,
            "square" | "sqr" | "rand" | "random" => Shape::Square,
            _ => Shape::Sine,
        }
    }
}

/// A free-running oscillator started with the note (like `osc.start(t)`).
#[derive(Clone, Debug)]
pub(crate) struct Osc { phase: f32, inc: f32, shape: Shape }

impl Osc {
    pub fn new(freq: f32, sr: f32, shape: Shape) -> Osc { Osc { phase: 0.0, inc: freq / sr, shape } }
    pub fn sine(freq: f32, sr: f32) -> Osc { Osc::new(freq, sr, Shape::Sine) }

    /// The next sample, -1..1.
    #[inline]
    pub fn next(&mut self) -> f32 { self.skip(1) }

    /// The current sample, then jump `n` samples ahead (for control-rate use).
    #[inline]
    pub fn skip(&mut self, n: u32) -> f32 {
        let p = self.phase;
        self.phase += self.inc * n as f32;
        if self.phase >= 1.0 { self.phase -= self.phase.floor(); }
        match self.shape {
            Shape::Sine => (p * TAU).sin(),
            Shape::Square => if p < 0.5 { 1.0 } else { -1.0 },
            Shape::Saw => { let q = p + 0.5; 2.0 * (q - q.floor()) - 1.0 }
            Shape::Tri => { let q = p + 0.25; 1.0 - 4.0 * (q - q.floor() - 0.5).abs() }
        }
    }
}

/// A `StereoPannerNode` at a fixed position, as six precomputed gains.
///
/// WebAudio's law: a mono input uses equal-power gains (centre = 0.707 per
/// side); a stereo input is "balanced" (centre = unchanged, hard left folds R into L).
///
/// Pan-law choice for a voice's POSITION (`| pan`, `lfo pan`, `orbit`,
/// `autopan`, built with `lift = true`): on a mono voice the browser's
/// equal-power centre is 3 dB quieter than an unpanned voice, which it simply
/// up-mixes to 1.0 per side like drums and samples. We keep the same
/// equal-power curve (constant loudness while it moves) but lift it by √2, so
/// the centre is exactly 1.0 per side: `| pan 0` sounds like no pan, and a
/// hard-panned voice carries the same power as a centred one. Stereo inputs
/// already keep unity at centre and use WebAudio's stereo law unchanged.
/// Internal spreaders (haas, chorus, the master delay) use `lift = false`, the
/// exact WebAudio gains, so their level matches the browser.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Panner {
    /// Mono input: `l = x·ml`, `r = x·mr`.
    ml: f32,
    mr: f32,
    /// Stereo input: `l = L·ll + R·rl`, `r = L·lr + R·rr`.
    ll: f32,
    rl: f32,
    lr: f32,
    rr: f32,
}

impl Panner {
    pub fn new(pan: f32, lift: bool) -> Panner {
        let p = pan.clamp(-1.0, 1.0);
        let m = (p + 1.0) * 0.5 * FRAC_PI_2;
        let g = if lift { SQRT_2 } else { 1.0 };
        let (ml, mr) = (m.cos() * g, m.sin() * g);
        if p <= 0.0 {
            let a = (p + 1.0) * FRAC_PI_2;
            Panner { ml, mr, ll: 1.0, rl: a.cos(), lr: 0.0, rr: a.sin() }
        } else {
            let a = p * FRAC_PI_2;
            Panner { ml, mr, ll: a.cos(), rl: 0.0, lr: a.sin(), rr: 1.0 }
        }
    }

    #[inline]
    pub fn apply(&self, x: Stereo, mono: bool) -> Stereo {
        if mono { Stereo { l: x.l * self.ml, r: x.l * self.mr } }
        else { Stereo { l: x.l * self.ll + x.r * self.rl, r: x.l * self.lr + x.r * self.rr } }
    }

    /// The per-sample change of every gain that glides from `self` to `to` in `n` samples.
    fn lerp_step(&self, to: &Panner, n: f32) -> Panner {
        let d = |a: f32, b: f32| (b - a) / n;
        Panner { ml: d(self.ml, to.ml), mr: d(self.mr, to.mr), ll: d(self.ll, to.ll), rl: d(self.rl, to.rl), lr: d(self.lr, to.lr), rr: d(self.rr, to.rr) }
    }

    fn add(&mut self, s: &Panner) {
        self.ml += s.ml; self.mr += s.mr; self.ll += s.ll; self.rl += s.rl; self.lr += s.lr; self.rr += s.rr;
    }
}

/// Samples between recomputations of a moving pan (gains glide linearly in between).
pub(crate) const PAN_BLOCK: u32 = 16;

/// A pan position that moves (LFO, orbit, autopan): the trigonometry runs once
/// per [`PAN_BLOCK`] samples and the gains glide between block targets.
#[derive(Clone, Debug, Default)]
pub(crate) struct MovingPan { cur: Panner, step: Panner, count: u32 }

impl MovingPan {
    /// `target` is asked for the next position only at block starts.
    #[inline]
    pub fn apply(&mut self, x: Stereo, mono: bool, target: impl FnOnce() -> f32) -> Stereo {
        if self.count == 0 {
            let t = Panner::new(target(), true);
            if self.cur == Panner::default() { self.cur = t; }
            self.step = self.cur.lerp_step(&t, PAN_BLOCK as f32);
            self.count = PAN_BLOCK;
        }
        self.count -= 1;
        let y = self.cur.apply(x, mono);
        self.cur.add(&self.step);
        y
    }
}

/// Run a per-channel process on both sides, or only the left while the
/// signal is still mono (then copy it to the right).
#[inline]
pub(crate) fn per_channel<T>(ch: &mut [T; 2], x: Stereo, mono: bool, mut f: impl FnMut(&mut T, f32) -> f32) -> Stereo {
    let [a, b] = ch;
    let l = f(a, x.l);
    if mono { Stereo::mono(l) } else { Stereo { l, r: f(b, x.r) } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_1_SQRT_2;

    #[test]
    fn oscillators_start_like_webaudio() {
        let sr = 1000.0;
        let first = |s| { let mut o = Osc::new(10.0, sr, s); let a = o.next(); for _ in 0..24 { o.next(); } (a, o.next()) };
        assert_eq!(first(Shape::Sine).0, 0.0);
        assert_eq!(first(Shape::Square).0, 1.0);
        assert!(first(Shape::Saw).0.abs() < 1e-6 && first(Shape::Tri).0.abs() < 1e-6);
        assert!((first(Shape::Tri).1 - 1.0).abs() < 1e-4, "triangle peaks a quarter cycle in");
    }

    #[test]
    fn pan_laws() {
        let c = Panner::new(0.0, false).apply(Stereo::mono(1.0), true);
        assert!((c.l - FRAC_1_SQRT_2).abs() < 1e-5 && (c.r - FRAC_1_SQRT_2).abs() < 1e-5, "WebAudio equal power");
        let v = Panner::new(0.0, true).apply(Stereo::mono(1.0), true);
        assert!((v.l - 1.0).abs() < 1e-5 && (v.r - 1.0).abs() < 1e-5, "centred voice keeps unity");
        let s = Panner::new(0.0, true).apply(Stereo { l: 0.5, r: 0.25 }, false);
        assert!((s.l - 0.5).abs() < 1e-6 && (s.r - 0.25).abs() < 1e-6, "stereo centre is unchanged");
        let hl = Panner::new(-1.0, false).apply(Stereo { l: 0.5, r: 0.25 }, false);
        assert!((hl.l - 0.75).abs() < 1e-6 && hl.r.abs() < 1e-6, "stereo hard left folds R into L");
        let hard = Panner::new(-1.0, true).apply(Stereo::mono(1.0), true);
        assert!((hard.l * hard.l + hard.r * hard.r - 2.0).abs() < 1e-4, "constant power");
    }
}
