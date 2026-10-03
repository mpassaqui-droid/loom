//! Musical time, as every sounding note sees it, plus evaluation of the
//! time-varying parameters (`200..2000` sweeps, `sine 0.5 200 2000` signals).

use loom_core::{Param, SigShape};
use std::f32::consts::TAU;

/// Where the transport is, for one sample. The sequencer updates it; notes read it.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    pub sr: f32,
    /// Seconds since RUN.
    pub seconds: f64,
    /// Bars since RUN (fractional).
    pub bars: f64,
    /// Length of the current bar in seconds (the JS `cycleLen`).
    pub bar_sec: f32,
    /// Total length of the `song:` arrangement in bars (0 = no song).
    pub song_bars: u32,
    /// Start (in song bars) and length of the section playing now.
    pub section_start: f64,
    pub section_len: f64,
}

impl Clock {
    pub fn new(sr: f32) -> Clock {
        Clock { sr, seconds: 0.0, bars: 0.0, bar_sec: 2.0, song_bars: 0, section_start: 0.0, section_len: 0.0 }
    }

    /// JS `autoProgress`: 0..1 through `| over N` bars, else through the current
    /// section of the song, else through a 16-bar cycle.
    pub fn progress(&self, over: Option<f32>) -> f32 {
        if let Some(o) = over.filter(|o| *o > 0.0) {
            return (self.bars.rem_euclid(o as f64) / o as f64) as f32;
        }
        if self.song_bars > 0 && self.section_len > 0.0 {
            let pos = self.bars.rem_euclid(self.song_bars as f64);
            return ((pos - self.section_start) / self.section_len).clamp(0.0, 1.0) as f32;
        }
        (self.bars.rem_euclid(16.0) / 16.0) as f32
    }

    /// JS `pvVal`: the value of a parameter right now. `salt` makes `rand`
    /// signals differ between voices (use a hash of the voice name).
    pub fn param(&self, p: &Param, over: Option<f32>, salt: u32) -> f32 {
        match *p {
            Param::Const(x) => x,
            Param::Sweep { a, b } => a + (b - a) * self.progress(over),
            Param::Curve { n, ref v, ref s } => loom_core::curve_at(n, v, s, self.progress(over)),
            Param::Signal { shape, rate, a, b } => {
                let ph = self.bars * rate as f64;
                let s = match shape {
                    SigShape::Rand => hash01(ph.floor() as i64 + 1, salt),
                    _ => shape01(shape, ph.rem_euclid(1.0) as f32),
                };
                a + (b - a) * s
            }
        }
    }
}

/// JS `sigShape`, 0..1.
fn shape01(shape: SigShape, ph: f32) -> f32 {
    match shape {
        SigShape::Sine => ((ph * TAU).sin() + 1.0) / 2.0,
        SigShape::Saw => ph,
        SigShape::Tri => if ph < 0.5 { ph * 2.0 } else { 2.0 - ph * 2.0 },
        SigShape::Square => if ph < 0.5 { 1.0 } else { 0.0 },
        SigShape::Rand => 0.0,
    }
}

fn hash01(n: i64, salt: u32) -> f32 {
    let mut x = (n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (salt as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 31;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 29;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

/// Stable small hash of a voice name, for `salt`.
pub fn name_salt(name: &str) -> u32 {
    name.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweeps_follow_section_or_over() {
        let mut c = Clock::new(48000.0);
        c.bars = 4.0;
        assert_eq!(c.param(&Param::Sweep { a: 0.0, b: 16.0 }, None, 0), 4.0, "16-bar default cycle");
        assert_eq!(c.param(&Param::Sweep { a: 0.0, b: 1.0 }, Some(8.0), 0), 0.5);
        c.song_bars = 24; c.section_start = 2.0; c.section_len = 4.0;
        assert_eq!(c.progress(None), 0.5);
        c.bars = 0.25;
        let v = c.param(&Param::Signal { shape: SigShape::Sine, rate: 1.0, a: 100.0, b: 300.0 }, None, 0);
        assert!((v - 300.0).abs() < 1e-3, "sine peaks a quarter cycle in: {v}");
        let r1 = c.param(&Param::Signal { shape: SigShape::Rand, rate: 1.0, a: 0.0, b: 1.0 }, None, 1);
        let r2 = c.param(&Param::Signal { shape: SigShape::Rand, rate: 1.0, a: 0.0, b: 1.0 }, None, 1);
        assert_eq!(r1, r2, "rand is deterministic");
        assert!((0.0..1.0).contains(&r1));
    }
}
