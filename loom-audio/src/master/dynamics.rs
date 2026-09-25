//! Dynamics: the `DynamicsCompressorNode` approximation (per-voice `| comp`,
//! the master glue and the master limiter), the `LoomSat` warmth saturator and
//! the `LoomLim` lookahead brick-wall limiter.

use crate::note::Stereo;

/// A feed-forward compressor following WebAudio's `DynamicsCompressorNode`:
/// the same static curve (linear below the threshold, an exponential soft knee,
/// then `1/ratio` above) and the automatic make-up gain `(1/curve(1.0))^0.6`.
/// Gain smoothing is a standard attack/release follower in dB (WebAudio's
/// adaptive release is not reproduced). Chromium also delays the audio 6 ms so
/// the detector sees transients coming; we skip that to keep the live latency
/// low (the master's final lookahead limiter catches the few overshoots).
#[derive(Clone, Debug)]
pub(crate) struct Compressor {
    /// Threshold (linear), knee end (linear and dB), knee sharpness `k`.
    thr: f32,
    knee_lin: f32,
    knee_db: f32,
    k: f32,
    /// Output level (dB) at the end of the knee, and the slope above it.
    knee_out_db: f32,
    slope: f32,
    makeup: f32,
    attack: f32,
    release: f32,
    /// Current gain change in dB (≤ 0).
    gr_db: f32,
    /// The detector runs once per [`COMP_BLOCK`] samples on the block's peak;
    /// the linear gain glides from one block to the next.
    peak: f32,
    count: u32,
    gain: f32,
    gain_step: f32,
}

const COMP_BLOCK: u32 = 8;

fn lin2db(x: f64) -> f64 { 20.0 * x.max(1e-30).log10() }

/// The knee curve of the WebAudio spec: `thr + (1 - e^(-k(x - thr))) / k`.
fn knee_curve(x: f64, thr: f64, k: f64) -> f64 {
    if x < thr { x } else { thr + (1.0 - (-k * (x - thr)).exp()) / k }
}

impl Compressor {
    /// Parameters as on the node: dB, dB, ratio, seconds, seconds.
    pub fn new(sr: f32, threshold_db: f32, knee_db: f32, ratio: f32, attack: f32, release: f32) -> Compressor {
        let thr = 10f64.powf(threshold_db as f64 / 20.0);
        let knee_end_db = (threshold_db + knee_db.max(0.0)) as f64;
        let knee_lin = 10f64.powf(knee_end_db / 20.0);
        let slope = 1.0 / ratio.max(1.0) as f64;
        // Find k so the knee's dB slope at its end matches 1/ratio (Chromium's `kAtSlope`).
        let slope_at = |x: f64, k: f64| {
            if x < thr { return 1.0; }
            let x2 = x * 1.001;
            (lin2db(knee_curve(x2, thr, k)) - lin2db(knee_curve(x, thr, k))) / (lin2db(x2) - lin2db(x))
        };
        let (mut lo, mut hi, mut k) = (0.1f64, 10000.0f64, 5.0f64);
        for _ in 0..15 {
            if slope_at(knee_lin, k) < slope { hi = k } else { lo = k }
            k = (lo * hi).sqrt();
        }
        let knee_out_db = lin2db(knee_curve(knee_lin, thr, k));
        let mut c = Compressor {
            thr: thr as f32, knee_lin: knee_lin as f32, knee_db: knee_end_db as f32, k: k as f32,
            knee_out_db: knee_out_db as f32, slope: slope as f32, makeup: 1.0,
            attack: 1.0 - (-(COMP_BLOCK as f32) / (attack.max(1e-4) * sr)).exp(),
            release: 1.0 - (-(COMP_BLOCK as f32) / (release.max(1e-3) * sr)).exp(),
            gr_db: 0.0,
            peak: 0.0,
            count: 0,
            gain: 1.0,
            gain_step: 0.0,
        };
        c.makeup = (1.0 / c.curve(1.0)).powf(0.6);
        c.gain = c.makeup;
        c
    }

    /// The static curve: input level → output level (linear).
    fn curve(&self, x: f32) -> f32 {
        if x < self.thr { return x; }
        if x < self.knee_lin { return knee_curve(x as f64, self.thr as f64, self.k as f64) as f32; }
        let db = self.knee_out_db + self.slope * (20.0 * x.log10() - self.knee_db);
        10f32.powf(db / 20.0)
    }

    /// Feed this input level to the detector; returns the gain to apply now.
    #[inline]
    fn gain(&mut self, level: f32) -> f32 {
        self.peak = self.peak.max(level);
        if self.count == 0 {
            let level = std::mem::take(&mut self.peak);
            let target = if level <= self.thr { 0.0 } else { 20.0 * (self.curve(level) / level).log10() };
            let c = if target < self.gr_db { self.attack } else { self.release };
            self.gr_db += (target - self.gr_db) * c;
            let next = if self.gr_db > -1e-4 { self.makeup } else { 10f32.powf(self.gr_db / 20.0) * self.makeup };
            self.gain_step = (next - self.gain) / COMP_BLOCK as f32;
            self.count = COMP_BLOCK;
        }
        self.count -= 1;
        self.gain += self.gain_step;
        self.gain
    }

    /// One stereo sample (linked detector). `mono`: only the left side is computed.
    #[inline]
    pub fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        let level = if mono { x.l.abs() } else { x.l.abs().max(x.r.abs()) };
        let g = self.gain(level);
        if mono { Stereo::mono(x.l * g) } else { x.scale(g) }
    }
}

/// The `LoomSat` worklet: a tanh saturator with a cheap 2× oversampling trick
/// (average of the shaped half-sample midpoint and the shaped sample).
#[derive(Clone, Debug, Default)]
pub(crate) struct Saturator { prev: [f32; 2] }

impl Saturator {
    #[inline]
    pub fn process(&mut self, x: Stereo, drive: f32) -> Stereo {
        let g = 1.0 + drive * 4.0;
        let comp = 1.0 / (1.0 + drive * 0.6);
        let side = |prev: &mut f32, x: f32| {
            let mid = (*prev + x) * 0.5;
            *prev = x;
            ((mid * g).tanh() + (x * g).tanh()) * 0.5 * comp
        };
        Stereo { l: side(&mut self.prev[0], x.l), r: side(&mut self.prev[1], x.r) }
    }
}

const LOOKAHEAD: usize = 128;

/// The `LoomLim` worklet: 128-sample lookahead, instant attack, slow release.
///
/// One fix over the JS: the JS releases while the loud sample is still in the
/// lookahead buffer, so a big peak can leave slightly above the ceiling. Here
/// the gain is also held at the minimum needed by every sample still in the
/// buffer, so the output never exceeds the ceiling.
#[derive(Clone, Debug)]
pub(crate) struct Limiter {
    ceiling: f32,
    buf: [[f32; LOOKAHEAD]; 2],
    /// Gain each buffered sample needs.
    need: [f32; LOOKAHEAD],
    pos: usize,
    g: f32,
    release: f32,
    /// Smallest `need` in the buffer and where it sits.
    min: f32,
    min_pos: usize,
}

impl Limiter {
    pub fn new(sr: f32, ceiling: f32) -> Limiter {
        // JS: g += (1-g)·0.0009 per sample at the browser's usual 48 kHz
        let release = 1.0 - (1.0 - 0.0009f32).powf(48000.0 / sr);
        Limiter { ceiling, buf: [[0.0; LOOKAHEAD]; 2], need: [1.0; LOOKAHEAD], pos: 0, g: 1.0, release, min: 1.0, min_pos: 0 }
    }

    #[inline]
    pub fn process(&mut self, x: Stereo) -> Stereo {
        let peak = x.l.abs().max(x.r.abs());
        let tg = if peak > self.ceiling { self.ceiling / peak } else { 1.0 };
        if tg < self.g { self.g = tg } else { self.g += (1.0 - self.g) * self.release }
        let p = self.pos;
        // `self.min` still covers the outgoing sample at `p`
        let g = self.g.min(self.min).min(tg);
        let out = Stereo { l: self.buf[0][p] * g, r: self.buf[1][p] * g };
        self.buf[0][p] = x.l;
        self.buf[1][p] = x.r;
        self.need[p] = tg;
        if tg <= self.min {
            self.min = tg;
            self.min_pos = p;
        } else if self.min_pos == p {
            // the smallest need just left the buffer: look for the next one
            let (i, m) = self.need.iter().enumerate().fold((p, tg), |best, (i, &v)| if v < best.1 { (i, v) } else { best });
            self.min = m;
            self.min_pos = i;
        }
        self.pos = (p + 1) % LOOKAHEAD;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressor_curve_matches_webaudio_shape() {
        let c = Compressor::new(48000.0, -14.0, 8.0, 3.5, 0.004, 0.22);
        assert!((c.curve(0.1) - 0.1).abs() < 1e-6, "below threshold: untouched");
        // well above the knee the slope is 1/ratio
        let d = 20.0 * (c.curve(1.0) / c.curve(0.5)).log10();
        assert!((d - 6.02 / 3.5).abs() < 0.05, "slope {d}");
        assert!(c.makeup > 1.5 && c.makeup < 2.5, "auto make-up {}", c.makeup);
        let hard = Compressor::new(48000.0, -2.0, 0.0, 20.0, 0.001, 0.09);
        assert!((20.0 * hard.curve(1.0).log10() + 1.9).abs() < 0.01);
    }

    #[test]
    fn compressor_reduces_loud_signals() {
        let sr = 48000.0;
        let mut c = Compressor::new(sr, -24.0, 30.0, 11.0, 0.003, 0.15);
        let mut quiet = 0.0f32;
        let mut loud = 0.0f32;
        for i in 0..24000 {
            let s = (i as f32 * 0.05).sin();
            quiet = quiet.max(c.process(Stereo::mono(s * 0.01), true).l.abs());
        }
        for i in 0..24000 {
            let s = (i as f32 * 0.05).sin();
            let y = c.process(Stereo::mono(s), true).l.abs();
            if i > 12000 { loud = loud.max(y); }
        }
        // 40 dB more input comes out at least 5 dB less than 40 dB louder (WebAudio's knee is wide)
        assert!(20.0 * (loud / quiet).log10() < 35.0, "{quiet} {loud}");
    }

    #[test]
    fn limiter_holds_the_ceiling() {
        let mut l = Limiter::new(48000.0, 0.97);
        let mut n = crate::dsp::Noise::new(3);
        for i in 0..48000 {
            let amp = if i % 5000 < 50 { 8.0 } else { 0.5 };
            let y = l.process(Stereo { l: n.sample() * amp, r: n.sample() * amp });
            assert!(y.l.abs() <= 0.97 + 1e-6 && y.r.abs() <= 0.97 + 1e-6);
        }
    }
}
