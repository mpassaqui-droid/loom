//! Tone-shaping inserts: eq, vowel, drive, fold, comp, crush, redux, reson,
//! flange, width, ring, am, phaser. Numbers are the browser's.

use super::prims::{per_channel, undenormal, Osc};
use super::Stage;
use crate::clock::Clock;
use crate::dsp::{Biquad, BiquadKind};
use crate::master::dynamics::Compressor;
use crate::note::Stereo;
use loom_core::{Eq3, Param};
use std::f32::consts::FRAC_PI_2;

/// `| eq L M H`: low shelf 180 Hz, peak 1200 Hz (Q 1), high shelf 5000 Hz.
pub(super) struct Eq { ch: [[Biquad; 3]; 2] }

impl Eq {
    pub fn new(eq: Eq3, sr: f32) -> Eq {
        let bands = [
            Biquad::new(BiquadKind::Lowshelf, sr, 180.0, 1.0, eq.l),
            Biquad::new(BiquadKind::Peaking, sr, 1200.0, 1.0, eq.m),
            Biquad::new(BiquadKind::Highshelf, sr, 5000.0, 1.0, eq.h),
        ];
        Eq { ch: [bands.clone(), bands] }
    }
}

impl Stage for Eq {
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        per_channel(&mut self.ch, x, mono, |f, s| f.iter_mut().fold(s, |s, b| b.process(s)))
    }
}

/// `| vowel a|e|i|o|u`: three parallel band-passes at the vowel's formants.
pub(super) struct Vowel { ch: [[Biquad; 3]; 2] }

const FORMANT_AMPS: [f32; 3] = [1.0, 0.55, 0.3];

impl Vowel {
    pub fn new(v: char, sr: f32) -> Vowel {
        let f = match v {
            'e' => [400.0, 1700.0, 2600.0],
            'i' => [300.0, 2300.0, 3000.0],
            'o' => [450.0, 800.0, 2830.0],
            'u' => [325.0, 700.0, 2530.0],
            _ => [800.0, 1150.0, 2900.0],
        };
        let bands = [0, 1, 2].map(|k| Biquad::bandpass(sr, f[k], 9.0 - k as f32 * 2.5));
        Vowel { ch: [bands.clone(), bands] }
    }
}

impl Stage for Vowel {
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        per_channel(&mut self.ch, x, mono, |f, s| f.iter_mut().zip(FORMANT_AMPS).map(|(b, a)| b.process(s) * a).sum())
    }
    fn tail(&self) -> f32 { 0.05 }
}

/// The JS `WaveShaperNode` with `oversample="2x"`, approximated like the
/// `LoomSat` worklet: average of the curve at the half-sample midpoint and at
/// the sample (tames aliasing without a real resampler).
#[inline]
fn shape2x(prev: &mut f32, x: f32, curve: impl Fn(f32) -> f32) -> f32 {
    let mid = (*prev + x) * 0.5;
    *prev = x;
    (curve(mid.clamp(-1.0, 1.0)) + curve(x.clamp(-1.0, 1.0))) * 0.5
}

/// `| drive N`: the JS `driveCurve` `(1+k)x / (1+k|x|)`, k = 120·drive, then `1/(1+drive)`.
/// A sweep/signal is read once at note start.
pub(super) struct Drive { param: Param, over: Option<f32>, salt: u32, k: f32, post: f32, prev: [f32; 2] }

impl Drive {
    pub fn new(param: Param, over: Option<f32>, salt: u32) -> Drive {
        Drive { param, over, salt, k: 0.0, post: 1.0, prev: [0.0; 2] }
    }
}

impl Stage for Drive {
    fn start(&mut self, clock: &Clock) {
        let dr = clock.param(&self.param, self.over, self.salt);
        let dr = if dr.is_finite() { dr.clamp(0.0, 1.0) } else { 0.0 };
        self.k = dr * 120.0;
        self.post = 1.0 / (1.0 + dr);
    }
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        let (k, post) = (self.k, self.post);
        per_channel(&mut self.prev, x, mono, |p, s| shape2x(p, s, |v| (1.0 + k) * v / (1.0 + k * v.abs())) * post)
    }
}

/// `| fold N`: wavefolder `sin(x·(1+6N)·π/2) / (1+0.4N)`.
pub(super) struct Fold { amount: f32, gain: f32, prev: [f32; 2] }

impl Fold {
    pub fn new(fold: f32) -> Fold { Fold { amount: (1.0 + fold * 6.0) * FRAC_PI_2, gain: 1.0 / (1.0 + fold * 0.4), prev: [0.0; 2] } }
}

impl Stage for Fold {
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        let (a, g) = (self.amount, self.gain);
        per_channel(&mut self.prev, x, mono, |p, s| shape2x(p, s, |v| (v * a).sin() * g))
    }
}

/// `| comp N`: DynamicsCompressor at -24 dB (knee 30), ratio 2+9N, 3 ms / 150 ms, then ×(1+0.6N).
pub(super) struct Comp { comp: Compressor, makeup: f32 }

impl Comp {
    pub fn new(c: f32, sr: f32) -> Comp {
        Comp { comp: Compressor::new(sr, -24.0, 30.0, 2.0 + c * 9.0, 0.003, 0.15), makeup: 1.0 + c * 0.6 }
    }
}

impl Stage for Comp {
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo { self.comp.process(x, mono).scale(self.makeup) }
}

/// `| crush BITS`: quantise to `2^(bits-1)` steps per unit (JS `Math.round`).
pub(super) struct Crush { levels: f32 }

impl Crush {
    pub fn new(bits: u32) -> Crush { Crush { levels: 2f32.powi(bits.clamp(1, 8) as i32 - 1) } }
}

impl Stage for Crush {
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        let lv = self.levels;
        let q = |s: f32| (s.clamp(-1.0, 1.0) * lv + 0.5).floor() / lv;
        if mono { Stereo::mono(q(x.l)) } else { Stereo { l: q(x.l), r: q(x.r) } }
    }
}

/// `| redux N`: the `LoomRedux` sample-and-hold, holding every `1 + 30N` samples.
pub(super) struct Redux { every: f32, phase: f32, held: Stereo }

impl Redux {
    pub fn new(r: f32) -> Redux { Redux { every: 1.0 + r * 30.0, phase: 0.0, held: Stereo::ZERO } }
}

impl Stage for Redux {
    fn process(&mut self, x: Stereo, _mono: bool) -> Stereo {
        self.phase += 1.0;
        if self.phase >= self.every {
            self.phase -= self.every;
            self.held = x;
        }
        self.held
    }
}

/// `| reson FREQ DECAY`: the `LoomReson` comb — one period of delay, a two-tap
/// average in the loop, output halved.
pub(super) struct Reson { ch: [ResonLine; 2], decay: f32, period: f32 }

#[derive(Clone)]
struct ResonLine { ring: super::prims::Ring, lp: f32 }

impl Reson {
    pub fn new(r: loom_core::Reson, sr: f32) -> Reson {
        let per = (sr / r.freq.max(1.0)).round().clamp(2.0, 4095.0) as usize;
        let line = ResonLine { ring: super::prims::Ring::new(per), lp: 0.0 };
        Reson { ch: [line.clone(), line], decay: r.decay.clamp(0.0, 0.995), period: per as f32 / sr }
    }
}

impl Stage for Reson {
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        let dec = self.decay;
        per_channel(&mut self.ch, x, mono, |c, s| {
            let fb = c.ring.out();
            c.lp = undenormal(c.lp * 0.5 + fb * 0.5);
            let y = s + c.lp * dec;
            c.ring.push(y);
            y * 0.5
        })
    }
    /// Until the loop has lost 60 dB (capped at 8 s).
    fn tail(&self) -> f32 {
        if self.decay <= 0.0 { return self.period; }
        (self.period * (1e-3f32.ln() / self.decay.ln())).min(8.0) + self.period
    }
    fn gap(&self) -> f32 { self.period }
}

/// `| flange RATE DEPTH`: the `LoomFlange` worklet — a 2..62-sample swept delay
/// with 0.5 feedback, `(x + delayed)·0.6`.
pub(super) struct Flange { ch: [[f32; FLANGE_LEN]; 2], pos: usize, lfo: Osc, depth: f32 }

const FLANGE_LEN: usize = 128;

impl Flange {
    pub fn new(f: loom_core::Flange, sr: f32) -> Flange {
        Flange { ch: [[0.0; FLANGE_LEN]; 2], pos: 0, lfo: Osc::sine(f.rate, sr), depth: f.depth }
    }
}

impl Stage for Flange {
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        let lfo = self.lfo.next();
        let ds = 2.0 + (lfo * 0.5 + 0.5) * self.depth * 60.0;
        let pos = self.pos;
        let ri = pos as f32 - ds;
        let r0 = ri.floor();
        let fr = ri - r0;
        let i0 = (r0 as isize).rem_euclid(FLANGE_LEN as isize) as usize;
        let i1 = (i0 + 1) % FLANGE_LEN;
        let y = per_channel(&mut self.ch, x, mono, |buf, s| {
            let dl = buf[i0] + (buf[i1] - buf[i0]) * fr;
            buf[pos] = undenormal(s + dl * 0.5);
            (s + dl) * 0.6
        });
        self.pos = (pos + 1) % FLANGE_LEN;
        y
    }
    fn tail(&self) -> f32 { 0.02 }
    fn gap(&self) -> f32 { 0.002 }
}

/// `| stereo N`: the `LoomWidth` mid/side widener. A mono signal passes through.
pub(super) struct Width(pub f32);

impl Stage for Width {
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        if mono { return x; }
        let m = (x.l + x.r) * 0.5;
        let s = (x.l - x.r) * 0.5 * self.0;
        Stereo { l: m + s, r: m - s }
    }
}

/// `| ring HZ`: multiply by a sine.
pub(super) struct RingMod { osc: Osc }

impl RingMod {
    pub fn new(hz: f32, sr: f32) -> RingMod { RingMod { osc: Osc::sine(hz.clamp(1.0, 8000.0), sr) } }
}

impl Stage for RingMod {
    fn process(&mut self, x: Stereo, _mono: bool) -> Stereo { x.scale(self.osc.next()) }
}

/// `| am HZ`: gain `0.55 + 0.45·sine`.
pub(super) struct Am { osc: Osc }

impl Am {
    pub fn new(hz: f32, sr: f32) -> Am { Am { osc: Osc::sine(hz.clamp(0.1, 8000.0), sr) } }
}

impl Stage for Am {
    fn process(&mut self, x: Stereo, _mono: bool) -> Stereo { x.scale(0.55 + 0.45 * self.osc.next()) }
}

/// `| phaser N`: dry + 4 allpasses at 400/750/1100/1450 Hz, all swept ±900 Hz
/// by a sine at `0.25 + 1.4N` Hz. Coefficients are refreshed every 32 samples.
pub(super) struct Phaser { ch: [[Biquad; 4]; 2], lfo: Osc, sr: f32, count: u32 }

const PHASER_EVERY: u32 = 32;

impl Phaser {
    pub fn new(p: f32, sr: f32) -> Phaser {
        let aps = [0, 1, 2, 3].map(|k| Biquad::new(BiquadKind::Allpass, sr, 400.0 + k as f32 * 350.0, 0.7, 0.0));
        Phaser { ch: [aps.clone(), aps], lfo: Osc::sine(0.25 + p * 1.4, sr), sr, count: 0 }
    }
}

impl Stage for Phaser {
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        // advance the LFO every sample, but only recompute the filters now and then
        let l = self.lfo.next();
        if self.count.is_multiple_of(PHASER_EVERY) {
            for chan in &mut self.ch {
                for (k, ap) in chan.iter_mut().enumerate() {
                    ap.set(BiquadKind::Allpass, self.sr, 400.0 + k as f32 * 350.0 + 900.0 * l, 0.7, 0.0);
                }
            }
        }
        self.count = self.count.wrapping_add(1);
        per_channel(&mut self.ch, x, mono, |aps, s| s + aps.iter_mut().fold(s, |v, ap| ap.process(v)))
    }
}
