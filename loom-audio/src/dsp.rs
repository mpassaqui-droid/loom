//! Small DSP building blocks shared by every instrument and effect.
//!
//! These mirror the WebAudio nodes the browser engine is built from
//! (`BiquadFilterNode`, `DelayNode`, `OscillatorNode`, ramps…), written as
//! plain per-sample structs.

use loom_core::Wave;
use std::f32::consts::TAU;

/// WebAudio `exponentialRampToValueAtTime`: `v0` at `t0` → `v1` at `t1`, held outside.
pub fn exp_ramp(v0: f32, v1: f32, t0: f32, t1: f32, t: f32) -> f32 {
    if t <= t0 { return v0; }
    if t >= t1 || t1 <= t0 { return v1; }
    let v0 = if v0 == 0.0 { 1e-6 } else { v0 };
    v0 * (v1 / v0).powf((t - t0) / (t1 - t0))
}

/// WebAudio `linearRampToValueAtTime`: `v0` at `t0` → `v1` at `t1`, held outside.
pub fn lin_ramp(v0: f32, v1: f32, t0: f32, t1: f32, t: f32) -> f32 {
    if t <= t0 { return v0; }
    if t >= t1 || t1 <= t0 { return v1; }
    v0 + (v1 - v0) * (t - t0) / (t1 - t0)
}

pub fn db_to_gain(db: f32) -> f32 { 10f32.powf(db / 20.0) }

/// Cheap white noise (xorshift32), -1..1.
#[derive(Clone, Debug)]
pub struct Noise(u32);

impl Noise {
    pub fn new(seed: u32) -> Self { Noise(seed.wrapping_mul(2654435761).max(1)) }
    pub fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
    /// Uniform 0..1.
    pub fn unit(&mut self) -> f32 { self.next() * 0.5 + 0.5 }
}

/// The filter shapes of WebAudio's `BiquadFilterNode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiquadKind { Lowpass, Highpass, Bandpass, Notch, Allpass, Peaking, Lowshelf, Highshelf }

/// RBJ-cookbook biquad — the same maths as WebAudio's `BiquadFilterNode`.
/// Call `set` again at any time to modulate it (state is kept).
#[derive(Clone, Debug)]
pub struct Biquad {
    b0: f32, b1: f32, b2: f32, a1: f32, a2: f32,
    x1: f32, x2: f32, y1: f32, y2: f32,
}

impl Biquad {
    /// `q` is the resonance (WebAudio `Q`); `gain_db` only matters for peaking/shelves.
    pub fn new(kind: BiquadKind, sr: f32, freq: f32, q: f32, gain_db: f32) -> Self {
        let mut f = Biquad { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0, x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 };
        f.set(kind, sr, freq, q, gain_db);
        f
    }
    pub fn lowpass(sr: f32, freq: f32, q: f32) -> Self { Self::new(BiquadKind::Lowpass, sr, freq, q, 0.0) }
    pub fn highpass(sr: f32, freq: f32, q: f32) -> Self { Self::new(BiquadKind::Highpass, sr, freq, q, 0.0) }
    pub fn bandpass(sr: f32, freq: f32, q: f32) -> Self { Self::new(BiquadKind::Bandpass, sr, freq, q, 0.0) }

    pub fn set(&mut self, kind: BiquadKind, sr: f32, freq: f32, q: f32, gain_db: f32) {
        let freq = freq.clamp(10.0, sr * 0.49);
        let q = q.max(1e-4);
        let w = TAU * freq / sr;
        let (cw, sw) = (w.cos(), w.sin());
        let a = 10f32.powf(gain_db / 40.0);
        let (b0, b1, b2, a0, a1, a2) = match kind {
            BiquadKind::Lowpass => {
                let alpha = sw / (2.0 * q);
                ((1.0 - cw) / 2.0, 1.0 - cw, (1.0 - cw) / 2.0, 1.0 + alpha, -2.0 * cw, 1.0 - alpha)
            }
            BiquadKind::Highpass => {
                let alpha = sw / (2.0 * q);
                ((1.0 + cw) / 2.0, -(1.0 + cw), (1.0 + cw) / 2.0, 1.0 + alpha, -2.0 * cw, 1.0 - alpha)
            }
            BiquadKind::Bandpass => {
                let alpha = sw / (2.0 * q);
                (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cw, 1.0 - alpha)
            }
            BiquadKind::Notch => {
                let alpha = sw / (2.0 * q);
                (1.0, -2.0 * cw, 1.0, 1.0 + alpha, -2.0 * cw, 1.0 - alpha)
            }
            BiquadKind::Allpass => {
                let alpha = sw / (2.0 * q);
                (1.0 - alpha, -2.0 * cw, 1.0 + alpha, 1.0 + alpha, -2.0 * cw, 1.0 - alpha)
            }
            BiquadKind::Peaking => {
                let alpha = sw / (2.0 * q);
                (1.0 + alpha * a, -2.0 * cw, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cw, 1.0 - alpha / a)
            }
            BiquadKind::Lowshelf | BiquadKind::Highshelf => {
                // WebAudio shelves use a fixed slope S = 1
                let alpha = sw / 2.0 * std::f32::consts::SQRT_2;
                let k = 2.0 * a.sqrt() * alpha;
                if kind == BiquadKind::Lowshelf {
                    (a * ((a + 1.0) - (a - 1.0) * cw + k), 2.0 * a * ((a - 1.0) - (a + 1.0) * cw), a * ((a + 1.0) - (a - 1.0) * cw - k),
                     (a + 1.0) + (a - 1.0) * cw + k, -2.0 * ((a - 1.0) + (a + 1.0) * cw), (a + 1.0) + (a - 1.0) * cw - k)
                } else {
                    (a * ((a + 1.0) + (a - 1.0) * cw + k), -2.0 * a * ((a - 1.0) + (a + 1.0) * cw), a * ((a + 1.0) + (a - 1.0) * cw - k),
                     (a + 1.0) - (a - 1.0) * cw + k, 2.0 * ((a - 1.0) - (a + 1.0) * cw), (a + 1.0) - (a - 1.0) * cw - k)
                }
            }
        };
        self.b0 = b0 / a0; self.b1 = b1 / a0; self.b2 = b2 / a0;
        self.a1 = a1 / a0; self.a2 = a2 / a0;
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1; self.x1 = x;
        // denormal guard: very quiet tails would otherwise slow the CPU down
        self.y2 = self.y1; self.y1 = if y.abs() < 1e-20 { 0.0 } else { y };
        self.y1
    }
}

/// One-pole smoothing lowpass (e.g. for envelopes, damping, parameter smoothing).
#[derive(Clone, Debug, Default)]
pub struct OnePole { coeff: f32, z: f32 }

impl OnePole {
    pub fn new(sr: f32, cutoff: f32) -> Self {
        OnePole { coeff: 1.0 - (-TAU * cutoff / sr).exp(), z: 0.0 }
    }
    pub fn process(&mut self, x: f32) -> f32 {
        self.z += (x - self.z) * self.coeff;
        self.z
    }
}

/// A circular delay line with fractional (linear-interpolated) reads, like `DelayNode`.
#[derive(Clone, Debug)]
pub struct DelayLine { buf: Vec<f32>, pos: usize }

impl DelayLine {
    /// Room for `max_seconds` of delay.
    pub fn new(sr: f32, max_seconds: f32) -> Self {
        DelayLine { buf: vec![0.0; ((sr * max_seconds) as usize).max(2) + 2], pos: 0 }
    }
    /// Read `delay` samples back (fractional allowed), BEFORE writing this sample.
    pub fn read(&self, delay: f32) -> f32 {
        let n = self.buf.len();
        let d = delay.clamp(0.0, (n - 2) as f32);
        let i = d.floor() as usize;
        let frac = d - i as f32;
        let a = self.buf[(self.pos + n - i) % n];
        let b = self.buf[(self.pos + n - i - 1) % n];
        a + (b - a) * frac
    }
    /// Push one sample and advance.
    pub fn write(&mut self, x: f32) {
        self.pos = (self.pos + 1) % self.buf.len();
        self.buf[self.pos] = x;
    }
    pub fn capacity(&self) -> usize { self.buf.len() - 2 }
}

/// Band-limiting correction for saw/square edges (fewer aliasing whistles).
pub fn poly_blep(t: f32, dt: f32) -> f32 {
    if dt <= 0.0 { return 0.0; }
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// One sample of a basic waveform at `phase` (0..1). `dt` = freq / sample rate.
pub fn osc(wave: Wave, phase: f32, dt: f32) -> f32 {
    match wave {
        Wave::Sine => (phase * TAU).sin(),
        Wave::Saw => (2.0 * phase - 1.0) - poly_blep(phase, dt),
        Wave::Square => {
            let mut s = if phase < 0.5 { 1.0 } else { -1.0 };
            s += poly_blep(phase, dt);
            s -= poly_blep((phase + 0.5) % 1.0, dt);
            s
        }
        Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
    }
}

/// A low-frequency oscillator. `shape` uses LOOM's words: sine, tri/triangle,
/// saw/sawtooth, square/sqr, rand/random. Output -1..1.
#[derive(Clone, Debug)]
pub struct Lfo { phase: f32, rate: f32, shape: LfoShape, noise: Noise, held: f32 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LfoShape { Sine, Tri, Saw, Square, Rand }

impl LfoShape {
    pub fn from_word(w: &str) -> LfoShape {
        match w {
            "tri" | "triangle" => LfoShape::Tri,
            "saw" | "sawtooth" => LfoShape::Saw,
            "square" | "sqr" => LfoShape::Square,
            "rand" | "random" => LfoShape::Rand,
            _ => LfoShape::Sine,
        }
    }
}

impl Lfo {
    pub fn new(rate_hz: f32, shape: LfoShape, seed: u32) -> Self {
        Lfo { phase: 0.0, rate: rate_hz.max(0.0), shape, noise: Noise::new(seed), held: 0.0 }
    }
    pub fn next(&mut self, sr: f32) -> f32 {
        let p = self.phase;
        let out = match self.shape {
            LfoShape::Sine => (p * TAU).sin(),
            LfoShape::Tri => 1.0 - 4.0 * (p - 0.5).abs(),
            LfoShape::Saw => 2.0 * p - 1.0,
            LfoShape::Square => if p < 0.5 { 1.0 } else { -1.0 },
            LfoShape::Rand => self.held,
        };
        self.phase += self.rate / sr;
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
            self.held = self.noise.next();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms_through(mut f: Biquad, freq: f32, sr: f32) -> f32 {
        let mut acc = 0.0;
        let n = (sr * 0.5) as usize;
        for i in 0..n {
            let y = f.process((TAU * freq * i as f32 / sr).sin());
            if i > n / 2 { acc += y * y; }
        }
        (acc / (n / 2) as f32).sqrt()
    }

    #[test]
    fn biquads_shape_the_spectrum() {
        let sr = 48000.0;
        let lp = || Biquad::lowpass(sr, 500.0, 0.707);
        assert!(rms_through(lp(), 100.0, sr) > 0.6 && rms_through(lp(), 8000.0, sr) < 0.02);
        let hp = || Biquad::highpass(sr, 2000.0, 0.707);
        assert!(rms_through(hp(), 100.0, sr) < 0.01 && rms_through(hp(), 10000.0, sr) > 0.6);
        let shelf = Biquad::new(BiquadKind::Lowshelf, sr, 200.0, 1.0, 12.0);
        assert!(rms_through(shelf, 50.0, sr) > 2.5, "a +12 dB shelf roughly quadruples lows");
    }

    #[test]
    fn delay_line_and_ramps() {
        let mut d = DelayLine::new(1000.0, 0.1);
        for i in 0..20 { d.write(i as f32); }
        assert_eq!(d.read(0.0), 19.0);
        assert_eq!(d.read(5.0), 14.0);
        assert!((d.read(2.5) - 16.5).abs() < 1e-5);
        assert!((exp_ramp(1.0, 0.001, 0.0, 1.0, 0.5) - 0.0316).abs() < 1e-3);
        assert_eq!(lin_ramp(0.0, 1.0, 0.0, 2.0, 1.0), 0.5);
    }
}
