//! The loudness meter: `LUFS~ -14 · pk -3dB`, next to the peak meter.
//!
//! Streaming services aim for about -14 LUFS. Real LUFS (ITU BS.1770) filters
//! the sound with a "K-weighting" curve (a gentle treble boost and a cut of
//! the deepest bass, roughly how loud our ears find each frequency), then
//! averages its power. "Short-term" loudness averages the last 3 seconds.
//!
//! We only get a mono copy of the output (the engine's scope), refreshed
//! every frame, so this is an approximation (hence the `~`): each snapshot is
//! K-weighted and measured, and the last 3 seconds of measures are averaged.
//! The mono copy counts as two identical channels, as a centred mix would.

use std::collections::VecDeque;

/// Short-term loudness window, in seconds.
const WINDOW: f64 = 3.0;
/// The held peak falls by this factor each second (-12 dB/s), so the peak
/// of the last beat stays readable.
const PEAK_FALL_PER_SECOND: f32 = 0.25;
/// Samples skipped at the start of a snapshot while the filters settle.
const SETTLE: usize = 256;

/// Colour hint for the meter, like the browser's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Normal,
    /// Louder than -9 LUFS (the browser turns amber).
    Loud,
    /// The peak touched full scale: clipping (red).
    Clipping,
}

pub struct Loudness {
    sample_rate: f32,
    /// `(time in seconds, mean square)` of the recent snapshots.
    measures: VecDeque<(f64, f32)>,
    /// The slowly falling peak-hold (0..1).
    peak_hold: f32,
    /// When `feed` was last called, in seconds.
    last_feed: Option<f64>,
}

impl Loudness {
    pub fn new(sample_rate: u32) -> Loudness {
        Loudness { sample_rate: sample_rate.max(1) as f32, measures: VecDeque::new(), peak_hold: 0.0, last_feed: None }
    }

    /// Measure a snapshot of the output (mono, oldest sample first), taken at
    /// `now` seconds, with the output's latest block `peak` (0..1). The peak
    /// shown is the loudest of that and the snapshot, like the browser's.
    pub fn feed(&mut self, snapshot: &[f32], peak: f32, now: f64) {
        let peak = snapshot.iter().fold(peak, |m, x| m.max(x.abs()));
        let ms = k_weighted_mean_square(snapshot, self.sample_rate);
        self.measures.push_back((now, ms));
        while self.measures.front().is_some_and(|(t, _)| now - t > WINDOW) {
            self.measures.pop_front();
        }
        let dt = self.last_feed.map_or(0.0, |t| (now - t).max(0.0));
        self.last_feed = Some(now);
        self.peak_hold = peak.max(self.peak_hold * PEAK_FALL_PER_SECOND.powf(dt as f32));
    }

    /// Forget everything (the music stopped).
    pub fn reset(&mut self) {
        self.measures.clear();
        self.peak_hold = 0.0;
        self.last_feed = None;
    }

    /// Short-term loudness in LUFS, or `None` for silence.
    pub fn lufs(&self) -> Option<f32> {
        if self.measures.is_empty() {
            return None;
        }
        let average = self.measures.iter().map(|(_, ms)| ms).sum::<f32>() / self.measures.len() as f32;
        lufs_from_mean_square(average)
    }

    /// Recent peak in dBFS (held, falling back slowly so short hits stay
    /// readable), or `None` for silence.
    pub fn peak_db(&self) -> Option<f32> {
        (self.peak_hold > 1e-4).then(|| 20.0 * self.peak_hold.log10())
    }

    pub fn level(&self) -> Level {
        if self.peak_hold > 0.99 {
            Level::Clipping
        } else if self.lufs().is_some_and(|l| l > -9.0) {
            Level::Loud
        } else {
            Level::Normal
        }
    }

    /// `LUFS~ -14 · pk -3dB`, with `−∞` for silence.
    pub fn text(&self) -> String {
        let show = |v: Option<f32>| v.map_or("−∞".to_string(), |v| format!("{:.0}", v.round()));
        format!("LUFS~ {} · pk {}dB", show(self.lufs()), show(self.peak_db()))
    }
}

/// Loudness of a mean square power: `-0.691 + 10·log10(2·ms)` (two channels).
pub fn lufs_from_mean_square(ms: f32) -> Option<f32> {
    let power = 2.0 * ms;
    (power > 1e-9).then(|| -0.691 + 10.0 * power.log10()).filter(|l| *l > -90.0)
}

/// Mean square of `samples` after the K-weighting filters.
pub fn k_weighted_mean_square(samples: &[f32], sample_rate: f32) -> f32 {
    let mut shelf = Biquad::k_shelf(sample_rate);
    let mut high_pass = Biquad::k_high_pass(sample_rate);
    let settle = if samples.len() > SETTLE * 2 { SETTLE } else { 0 };
    let mut sum = 0.0f64;
    for (i, &x) in samples.iter().enumerate() {
        let y = high_pass.process(shelf.process(x as f64));
        if i >= settle {
            sum += y * y;
        }
    }
    let counted = samples.len() - settle;
    if counted == 0 { 0.0 } else { (sum / counted as f64) as f32 }
}

/// A second-order filter (the formulas are the standard BS.1770 ones,
/// recomputed for any sample rate).
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [f64; 2],
}

impl Biquad {
    /// Stage 1: high shelf, +4 dB above ~1.7 kHz (the head's acoustic effect).
    fn k_shelf(sample_rate: f32) -> Biquad {
        let (f0, gain_db, q) = (1681.974450955533, 3.999843853973347, 0.7071752369554196);
        let k = (std::f64::consts::PI * f0 / sample_rate as f64).tan();
        let vh = 10f64.powf(gain_db / 20.0);
        let vb = vh.powf(0.4996667741545416);
        let a0 = 1.0 + k / q + k * k;
        Biquad {
            b: [(vh + vb * k / q + k * k) / a0, 2.0 * (k * k - vh) / a0, (vh - vb * k / q + k * k) / a0],
            a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
            z: [0.0; 2],
        }
    }

    /// Stage 2: high-pass at ~38 Hz (the "RLB" curve).
    fn k_high_pass(sample_rate: f32) -> Biquad {
        let (f0, q) = (38.13547087602444, 0.5003270373238773);
        let k = (std::f64::consts::PI * f0 / sample_rate as f64).tan();
        let a0 = 1.0 + k / q + k * k;
        Biquad { b: [1.0, -2.0, 1.0], a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0], z: [0.0; 2] }
    }

    fn process(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, amplitude: f32, sample_rate: f32, len: usize) -> Vec<f32> {
        (0..len).map(|i| amplitude * (std::f32::consts::TAU * freq * i as f32 / sample_rate).sin()).collect()
    }

    #[test]
    fn full_scale_1khz_sine_is_about_0_lufs() {
        // the BS.1770 reference: a 997 Hz sine at 0 dBFS in both channels reads 0 LUFS
        let ms = k_weighted_mean_square(&sine(997.0, 1.0, 48_000.0, 48_000), 48_000.0);
        let lufs = lufs_from_mean_square(ms).unwrap();
        assert!(lufs.abs() < 0.3, "{lufs}");
        // 20 dB quieter reads -20
        let quiet = lufs_from_mean_square(k_weighted_mean_square(&sine(997.0, 0.1, 44_100.0, 44_100), 44_100.0)).unwrap();
        assert!((quiet + 20.0).abs() < 0.3, "{quiet}");
    }

    #[test]
    fn mono_approximation_is_close_to_real_stereo_loudness() {
        let src = "bpm: 120\nkey: c minor\nkick: x . . . x . . . x . . . x . x .\nhat: x x x x x x x x\nbass: 1 . 1 . 3 . 5 . | saw | oct 2\nlead: 5 . . 3 . . 1 . | tri";
        let sr = 48_000;
        let stereo = loom_audio::render(src, 4, sr);
        let left: Vec<f32> = stereo.chunks(2).map(|f| f[0]).collect();
        let right: Vec<f32> = stereo.chunks(2).map(|f| f[1]).collect();
        let mono: Vec<f32> = stereo.chunks(2).map(|f| (f[0] + f[1]) / 2.0).collect();
        let real = -0.691 + 10.0 * (k_weighted_mean_square(&left, sr as f32) + k_weighted_mean_square(&right, sr as f32)).log10();
        let ours = lufs_from_mean_square(k_weighted_mean_square(&mono, sr as f32)).unwrap();
        assert!((real - ours).abs() < 1.5, "real {real} ours {ours}");
    }

    #[test]
    fn deep_bass_counts_less_than_treble() {
        let bass = k_weighted_mean_square(&sine(30.0, 0.5, 48_000.0, 48_000), 48_000.0);
        let treble = k_weighted_mean_square(&sine(4000.0, 0.5, 48_000.0, 48_000), 48_000.0);
        assert!(treble > bass * 2.0, "bass {bass} treble {treble}");
    }

    #[test]
    fn short_term_window_and_text() {
        let mut meter = Loudness::new(48_000);
        assert_eq!(meter.text(), "LUFS~ −∞ · pk −∞dB");
        let loud = sine(997.0, 0.5, 48_000.0, 2048);
        meter.feed(&loud, 0.5, 0.0);
        let lufs = meter.lufs().unwrap();
        assert!((lufs + 6.0).abs() < 0.6, "{lufs}");
        assert_eq!(meter.text(), format!("LUFS~ {:.0} · pk -6dB", lufs.round()));
        // 4 seconds of silence later, the loud snapshot has left the window
        meter.feed(&[0.0; 2048], 0.0, 4.0);
        assert_eq!(meter.lufs(), None);
        assert_eq!(meter.level(), Level::Normal);
        // the held peak (-6 dB) fell 12 dB per second for 4 s
        assert!((meter.peak_db().unwrap() + 54.2).abs() < 0.5, "{:?}", meter.peak_db());
        meter.feed(&loud, 1.0, 4.1);
        assert_eq!(meter.level(), Level::Clipping);
    }
}
