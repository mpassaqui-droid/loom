//! Synth voices — sample-by-sample ports of the browser engine's instruments
//! (`kick`, `snare`, `hat`, `clap`, `tone` in `web/index.html`).
//!
//! WebAudio describes sounds as node graphs with `exponentialRampToValueAtTime`
//! automation. Here each voice is a tiny state machine: it knows its own age in
//! seconds and computes the same ramps directly, so no graph is needed.

use loom_core::Wave;
use std::f32::consts::TAU;

/// WebAudio's exponential ramp: `v0` at `t0` → `v1` at `t1`, held after.
fn exp_ramp(v0: f32, v1: f32, t0: f32, t1: f32, t: f32) -> f32 {
    if t <= t0 { return v0; }
    if t >= t1 { return v1; }
    v0 * (v1 / v0).powf((t - t0) / (t1 - t0))
}

/// Cheap white noise (xorshift32), -1..1.
#[derive(Clone)]
pub struct Noise(u32);

impl Noise {
    pub fn new(seed: u32) -> Self { Noise(seed.max(1)) }
    pub fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// RBJ biquad, the same filter family as WebAudio's `BiquadFilterNode`.
#[derive(Clone, Default)]
pub struct Biquad { b0: f32, b1: f32, b2: f32, a1: f32, a2: f32, x1: f32, x2: f32, y1: f32, y2: f32 }

impl Biquad {
    fn from_coeffs(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        Biquad { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0, ..Default::default() }
    }
    fn parts(sr: f32, freq: f32, q: f32) -> (f32, f32) {
        let w = TAU * freq.min(sr * 0.49) / sr;
        (w.cos(), w.sin() / (2.0 * q))
    }
    pub fn highpass(sr: f32, freq: f32) -> Self {
        let (c, alpha) = Self::parts(sr, freq, std::f32::consts::FRAC_1_SQRT_2);
        Self::from_coeffs((1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0, 1.0 + alpha, -2.0 * c, 1.0 - alpha)
    }
    pub fn bandpass(sr: f32, freq: f32, q: f32) -> Self {
        let (c, alpha) = Self::parts(sr, freq, q);
        Self::from_coeffs(alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * c, 1.0 - alpha)
    }
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1; self.x1 = x;
        self.y2 = self.y1; self.y1 = y;
        y
    }
}

/// PolyBLEP correction — softens the hard edges of saw/square waves so high
/// notes don't alias into harsh inharmonic whistles.
fn poly_blep(t: f32, dt: f32) -> f32 {
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

fn osc(wave: Wave, phase: f32, dt: f32) -> f32 {
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

/// Which instrument a voice is playing.
#[derive(Clone)]
enum Kind {
    Kick { phase: f32, click_hp: Biquad },
    Snare { p1: f32, p2: f32, hp: Biquad },
    Hat { phases: [f32; 6], hp: Biquad },
    Clap { bp: Biquad },
    Tone { wave: Wave, freq: f32, phase: f32, len: f32 },
}

const HAT_FREQS: [f32; 6] = [2003.0, 3674.0, 4985.0, 6291.0, 7531.0, 9270.0];

/// One sounding note. Created by the sequencer, rendered until `done()`.
#[derive(Clone)]
pub struct Voice {
    kind: Kind,
    /// Voice name (`kick`, `bass`…) — used for mute/solo and meters.
    pub name: String,
    sr: f32,
    dt: f32,
    t: f32,
    vel: f32,
    gain_l: f32,
    gain_r: f32,
    noise: Noise,
}

impl Voice {
    fn new(kind: Kind, name: &str, sr: f32, vel: f32, pan: Option<f32>, seed: u32) -> Voice {
        let p = pan.unwrap_or(0.0).clamp(-1.0, 1.0);
        Voice {
            kind, name: name.to_string(), sr, dt: 1.0 / sr, t: 0.0, vel,
            gain_l: (1.0 - p).min(1.0), gain_r: (1.0 + p).min(1.0),
            noise: Noise::new(seed),
        }
    }

    /// A drum hit (`kick`/`bd`, `snare`/`sd`, `hat`/`hh`, `clap`/`cp`).
    /// Returns `None` for an unknown drum name.
    pub fn drum(name: &str, sr: f32, vel: f32, pan: Option<f32>, seed: u32) -> Option<Voice> {
        let kind = match name {
            "kick" | "bd" => Kind::Kick { phase: 0.0, click_hp: Biquad::highpass(sr, 1200.0) },
            "snare" | "sd" => Kind::Snare { p1: 0.0, p2: 0.0, hp: Biquad::highpass(sr, 1700.0) },
            "hat" | "hh" => Kind::Hat { phases: [0.0; 6], hp: Biquad::highpass(sr, 8200.0) },
            "clap" | "cp" => Kind::Clap { bp: Biquad::bandpass(sr, 1500.0, 1.1) },
            _ => return None,
        };
        Some(Voice::new(kind, name, sr, vel, pan, seed))
    }

    /// A pitched note: `freq` Hz for `len` seconds (the gate), then a short release.
    /// `gain` is the voice gain (`| gain N`, default 0.22), `vel` the accent.
    pub fn tone(name: &str, wave: Wave, freq: f32, len: f32, gain: f32, vel: f32, pan: Option<f32>, sr: f32) -> Voice {
        Voice::new(Kind::Tone { wave, freq, phase: 0.0, len: len.max(0.01) }, name, sr, gain * vel, pan, 1)
    }

    /// True once the sound has fully decayed.
    pub fn done(&self) -> bool {
        let end = match &self.kind {
            Kind::Kick { .. } => 0.36,
            Kind::Snare { .. } => 0.17,
            Kind::Hat { .. } => 0.06,
            Kind::Clap { .. } => 0.18,
            Kind::Tone { len, .. } => len + TONE_RELEASE,
        };
        self.t >= end
    }

    /// Render one stereo sample and add it into `(l, r)`.
    pub fn render(&mut self, l: &mut f32, r: &mut f32) {
        let t = self.t;
        let v = self.vel;
        let sr = self.sr;
        let s = match &mut self.kind {
            Kind::Kick { phase, click_hp } => {
                // sine body with a pitch drop 180→45 Hz + a short high-passed noise click
                let f = exp_ramp(180.0, 45.0, 0.0, 0.09, t);
                *phase = (*phase + f / sr) % 1.0;
                let body = if t < 0.36 { (*phase * TAU).sin() * exp_ramp(v, 0.001, 0.0, 0.34, t) } else { 0.0 };
                let n = self.noise.next();
                let click = click_hp.process(n);
                body + if t < 0.03 { click * exp_ramp(0.5 * v, 0.001, 0.0, 0.02, t) } else { 0.0 }
            }
            Kind::Snare { p1, p2, hp } => {
                // two triangle partials (185/295 Hz) + high-passed noise
                *p1 = (*p1 + 185.0 / sr) % 1.0;
                *p2 = (*p2 + 295.0 / sr) % 1.0;
                let mut out = 0.0;
                if t < 0.1 {
                    out += osc(Wave::Triangle, *p1, 0.0) * exp_ramp(0.28 * v, 0.001, 0.0, 0.09, t);
                    out += osc(Wave::Triangle, *p2, 0.0) * exp_ramp(0.16 * v, 0.001, 0.0, 0.09, t);
                }
                let n = hp.process(self.noise.next());
                out + if t < 0.17 { n * exp_ramp(0.5 * v, 0.001, 0.0, 0.15, t) } else { 0.0 }
            }
            Kind::Hat { phases, hp } => {
                // 808 metallic hat: six inharmonic squares through a high-pass
                let mut mix = 0.0;
                if t < 0.06 {
                    for (ph, f) in phases.iter_mut().zip(HAT_FREQS) {
                        *ph = (*ph + f / sr) % 1.0;
                        mix += if *ph < 0.5 { 1.0 } else { -1.0 };
                    }
                }
                hp.process(mix * 0.1) * exp_ramp(0.4 * v, 0.001, 0.0, 0.045, t)
            }
            Kind::Clap { bp } => {
                // three quick noise bursts + a longer tail, all through one bandpass
                let n = self.noise.next();
                let mut env = 0.0;
                for (i, d) in [0.0, 0.008, 0.016, 0.028].iter().enumerate() {
                    let tt = t - d;
                    let (peak, dec) = if i < 3 { (0.45 * v, 0.02) } else { (0.55 * v, 0.13) };
                    if tt >= 0.0 && tt < dec + 0.02 {
                        env += if tt < 0.001 { exp_ramp(0.0001, peak, 0.0, 0.001, tt) } else { exp_ramp(peak, 0.001, 0.001, dec, tt) };
                    }
                }
                bp.process(n * env)
            }
            Kind::Tone { wave, freq, phase, len } => {
                let dt = *freq / sr;
                let s = osc(*wave, *phase, dt);
                *phase = (*phase + dt) % 1.0;
                s * tone_env(t, *len) * v
            }
        };
        *l += s * self.gain_l;
        *r += s * self.gain_r;
        self.t += self.dt;
    }
}

const TONE_ATTACK: f32 = 0.005;
const TONE_DECAY: f32 = 0.12;
const TONE_SUSTAIN: f32 = 0.7;
const TONE_RELEASE: f32 = 0.06;

/// Amplitude envelope of a tone: fast attack, gentle decay to sustain, then a
/// short release after the gate closes (no clicks at note boundaries).
fn tone_env(t: f32, len: f32) -> f32 {
    let held = |t: f32| {
        if t < TONE_ATTACK { t / TONE_ATTACK }
        else if t < TONE_ATTACK + TONE_DECAY { 1.0 - (1.0 - TONE_SUSTAIN) * (t - TONE_ATTACK) / TONE_DECAY }
        else { TONE_SUSTAIN }
    };
    if t < len { held(t) } else {
        let k = 1.0 - (t - len) / TONE_RELEASE;
        if k <= 0.0 { 0.0 } else { held(len) * k * k }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(mut v: Voice) -> (usize, f32) {
        let (mut n, mut peak) = (0, 0.0f32);
        while !v.done() {
            let (mut l, mut r) = (0.0, 0.0);
            v.render(&mut l, &mut r);
            assert!(l.is_finite() && r.is_finite());
            peak = peak.max(l.abs()).max(r.abs());
            n += 1;
        }
        (n, peak)
    }

    #[test]
    fn drums_are_audible_and_finish() {
        for name in ["kick", "snare", "hat", "clap"] {
            let (n, peak) = run(Voice::drum(name, 48000.0, 1.0, None, 7).unwrap());
            assert!(n > 1000 && n < 48000, "{name} lasted {n} samples");
            assert!(peak > 0.02 && peak < 2.0, "{name} peak {peak}");
        }
        assert!(Voice::drum("banjo", 48000.0, 1.0, None, 7).is_none());
    }

    #[test]
    fn tone_follows_gate_and_pan() {
        let v = Voice::tone("bass", Wave::Saw, 110.0, 0.25, 0.22, 1.0, Some(-1.0), 48000.0);
        let mut w = v.clone();
        let (mut l, mut r) = (0.0, 0.0);
        for _ in 0..2000 { w.render(&mut l, &mut r); }
        assert_eq!(r, 0.0, "hard-left pan must silence the right channel");
        let (n, peak) = run(v);
        let expected = ((0.25 + TONE_RELEASE) * 48000.0f32) as i64;
        assert!((n as i64 - expected).abs() <= 2, "{n} vs {expected}");
        assert!(peak > 0.1 && peak <= 0.3);
    }
}
