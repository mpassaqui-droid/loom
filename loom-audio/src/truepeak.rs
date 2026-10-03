//! True peak (ITU BS.1770-4, annex 2): the loudest point of the sound between
//! samples too, found by oversampling 4 times. A sample peak of -0.3 dBFS can
//! hide an inter-sample peak above 0 dBFS that clips a DAC or an MP3 encoder.

/// Taps per oversampling phase (64-tap windowed-sinc prototype).
const TAPS: usize = 16;
const PHASES: usize = 4;
/// How many input samples the estimate lags behind (group delay of the filter).
pub const DELAY: usize = TAPS / 2;

fn coefficients() -> [[f32; TAPS]; PHASES] {
    let n = TAPS * PHASES;
    let centre = (n as f64 - 1.0) / 2.0;
    let mut h = [[0.0f32; TAPS]; PHASES];
    for i in 0..n {
        let t = (i as f64 - centre) / PHASES as f64;
        let sinc = if t.abs() < 1e-9 { 1.0 } else { (std::f64::consts::PI * t).sin() / (std::f64::consts::PI * t) };
        // Blackman window
        let w = 0.42 - 0.5 * (std::f64::consts::TAU * i as f64 / (n - 1) as f64).cos() + 0.08 * (2.0 * std::f64::consts::TAU * i as f64 / (n - 1) as f64).cos();
        h[i % PHASES][i / PHASES] = (sinc * w) as f32;
    }
    // each phase passes DC at unity gain
    for phase in &mut h {
        let sum: f32 = phase.iter().sum();
        for c in phase.iter_mut() { *c /= sum; }
    }
    h
}

/// Streaming true-peak estimate for one channel.
#[derive(Clone, Debug)]
pub struct TruePeak {
    h: [[f32; TAPS]; PHASES],
    hist: [f32; TAPS],
    pos: usize,
}

impl Default for TruePeak {
    fn default() -> Self { TruePeak { h: coefficients(), hist: [0.0; TAPS], pos: 0 } }
}

impl TruePeak {
    /// Feed one sample; returns the largest |value| of the 4 oversampled points
    /// around the sample [`DELAY`] steps back.
    #[inline]
    pub fn push(&mut self, x: f32) -> f32 {
        self.hist[self.pos] = x;
        self.pos = (self.pos + 1) % TAPS;
        let mut peak = 0.0f32;
        for phase in &self.h {
            let mut acc = 0.0;
            for (k, c) in phase.iter().enumerate() {
                // newest sample first
                acc += c * self.hist[(self.pos + TAPS - 1 - k) % TAPS];
            }
            peak = peak.max(acc.abs());
        }
        peak
    }
}

/// True peak of a whole signal (one channel), in linear units.
pub fn true_peak(samples: &[f32]) -> f32 {
    let mut tp = TruePeak::default();
    let mut peak = samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    for &x in samples.iter().chain(std::iter::repeat_n(&0.0, TAPS)) {
        peak = peak.max(tp.push(x));
    }
    peak
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voit_les_pics_entre_les_echantillons() {
        // fs/4 sine phased 45°: every sample is at 0.707, the wave peaks at 1.0
        let s: Vec<f32> = (0..4000).map(|i| (std::f32::consts::FRAC_PI_2 * i as f32 + std::f32::consts::FRAC_PI_4).sin()).collect();
        let sample_peak = s.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!((sample_peak - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.01);
        let tp = true_peak(&s);
        assert!((tp - 1.0).abs() < 0.05, "true peak {tp}");
        // a slow sine: true peak = sample peak
        let slow: Vec<f32> = (0..4000).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        assert!((true_peak(&slow) - 0.5).abs() < 0.01);
    }
}
