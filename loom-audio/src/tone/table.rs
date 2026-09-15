//! Single-cycle wavetables for `| wt NAME` and `| partials …` — the port of the
//! browser's `waveCoeffs` / `getPartialsWave` (a WebAudio `PeriodicWave`).

use std::f32::consts::TAU;
use std::sync::OnceLock;

/// Samples in one cycle (a power of two, so harmonic k at sample j is `SINE[(k*j) % LEN]`).
const LEN: usize = 1024;

/// One cycle of a sine, computed once per process. `at` is a fast sine:
/// linear interpolation in the table (error below -100 dB, several times
/// cheaper than `f32::sin`). Fetch it once with [`sine`] and keep the reference.
pub struct SineTable([f32; LEN]);

impl SineTable {
    /// `sin(2π·phase)` for phase 0..1.
    #[inline]
    pub fn at(&self, phase: f32) -> f32 {
        let x = phase * LEN as f32;
        let i = x as i32; // (an i32 cast is cheaper than a usize one)
        let f = x - i as f32;
        let i = i as usize;
        let a = self.0[i & (LEN - 1)];
        a + (self.0[(i + 1) & (LEN - 1)] - a) * f
    }
}

pub fn sine() -> &'static SineTable {
    static SINE: OnceLock<SineTable> = OnceLock::new();
    SINE.get_or_init(|| SineTable(std::array::from_fn(|j| (TAU * j as f32 / LEN as f32).sin())))
}

/// Harmonic amplitudes of a named table, index 0 = fundamental (JS `waveCoeffs`).
pub fn named_harmonics(name: &str) -> Vec<f32> {
    // the JS fills `im[k]` for harmonic k (im[0] is DC and stays 0)
    let mut im = [0.0f32; 32];
    match name {
        "organ" => im[..11].copy_from_slice(&[0.0, 1.0, 0.6, 0.0, 0.45, 0.0, 0.3, 0.0, 0.2, 0.0, 0.15]),
        "warm" => im[..7].copy_from_slice(&[0.0, 1.0, 0.5, 0.33, 0.2, 0.13, 0.08]),
        "soft" => (1..32).step_by(2).for_each(|k| im[k] = 1.0 / (k * k) as f32),
        _ => (1..32).for_each(|k| im[k] = 1.0 / k as f32), // "rich" and anything unknown
    }
    im[1..].to_vec()
}

/// A band-limited single-cycle waveform, read with linear interpolation.
pub struct WaveTable { data: Vec<f32> }

impl WaveTable {
    /// `harm[k]` = amplitude of harmonic k+1. Harmonics above `max_harm` are left
    /// out (they would alias at this note's pitch). Like WebAudio, the scale is
    /// chosen so the FULL table peaks at 1, whatever gets left out.
    pub fn new(harm: &[f32], max_harm: usize) -> WaveTable {
        let full = sum_harmonics(harm);
        let peak = full.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let scale = if peak > 0.0 { 1.0 / peak } else { 0.0 };
        let mut data = if max_harm >= harm.len() { full } else { sum_harmonics(&harm[..max_harm.max(1)]) };
        data.iter_mut().for_each(|x| *x *= scale);
        data.push(data[0]); // guard sample for interpolation
        WaveTable { data }
    }

    /// The waveform at `phase` (0..1).
    #[inline]
    pub fn read(&self, phase: f32) -> f32 {
        let x = phase * LEN as f32;
        let i = (x as i32).clamp(0, LEN as i32 - 1);
        let f = x - i as f32;
        let i = i as usize;
        let a = self.data[i];
        a + (self.data[i + 1] - a) * f
    }
}

fn sum_harmonics(harm: &[f32]) -> Vec<f32> {
    let sin = &sine().0;
    let mut out = vec![0.0f32; LEN];
    for (k, &amp) in harm.iter().enumerate().filter(|(_, a)| **a != 0.0) {
        let h = k + 1;
        for (j, o) in out.iter_mut().enumerate() {
            *o += amp * sin[(h * j) & (LEN - 1)];
        }
    }
    out
}
