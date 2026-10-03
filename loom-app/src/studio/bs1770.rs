//! The mastering meter, offline and by the book (ITU BS.1770-4, EBU R128):
//! integrated loudness with its two gates, short-term and momentary maxima,
//! loudness range (LRA) and true peak. Used after an export or a REC, and to
//! bring a whole song to a streaming target.

use super::loudness::Biquad;

/// What a mastering engineer reads off a finished track.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// Integrated loudness, LUFS (gated: silence and quiet passages don't drag it down).
    pub integrated: Option<f64>,
    /// Loudest 3-second window, LUFS.
    pub short_term_max: Option<f64>,
    /// Loudest 400 ms window, LUFS.
    pub momentary_max: Option<f64>,
    /// Loudness range, LU (how much the loudness moves: film 15+, pop 5-8).
    pub lra: Option<f64>,
    /// True peak, dBTP (inter-sample).
    pub true_peak: f64,
}

/// Where the big platforms turn a track down (or up) to.
pub const TARGETS: [(&str, f64, f64); 5] = [
    // (name, LUFS, max true peak dBTP)
    ("spotify", -14.0, -1.0),
    ("youtube", -14.0, -1.0),
    ("apple", -16.0, -1.0),
    ("deezer", -15.0, -1.0),
    ("club", -9.0, -0.3),
];

pub fn target(name: &str) -> Option<(f64, f64)> {
    TARGETS.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(name)).map(|(_, l, t)| (*l, *t))
}

fn lufs(power: f64) -> f64 { -0.691 + 10.0 * power.max(1e-20).log10() }

/// Mean K-weighted power (summed over L and R) of each `block`-sample window,
/// stepping by `step` samples.
fn block_powers(stereo: &[f32], sr: f32, block: usize, step: usize) -> Vec<f64> {
    let n = stereo.len() / 2;
    let mut sq = vec![0.0f64; n];
    for ch in 0..2 {
        let (mut shelf, mut hp) = (Biquad::k_shelf(sr), Biquad::k_high_pass(sr));
        for i in 0..n {
            let y = hp.process(shelf.process(stereo[i * 2 + ch] as f64));
            sq[i] += y * y;
        }
    }
    // running sums for O(n)
    let mut cum = vec![0.0f64; n + 1];
    for i in 0..n { cum[i + 1] = cum[i] + sq[i]; }
    if n < block || block == 0 { return Vec::new(); }
    (0..=(n - block)).step_by(step.max(1)).map(|s| (cum[s + block] - cum[s]) / block as f64).collect()
}

/// Measure interleaved stereo `samples` at `sample_rate`.
pub fn measure(samples: &[f32], sample_rate: u32) -> Report {
    let sr = sample_rate as f32;
    let momentary = block_powers(samples, sr, (0.4 * sr) as usize, (0.1 * sr) as usize);
    let short = block_powers(samples, sr, (3.0 * sr) as usize, sr as usize);
    // integrated: absolute gate -70 LUFS, then relative gate 10 LU under the mean
    let above: Vec<f64> = momentary.iter().copied().filter(|p| lufs(*p) > -70.0).collect();
    let integrated = (!above.is_empty()).then(|| {
        let rel = lufs(above.iter().sum::<f64>() / above.len() as f64) - 10.0;
        let kept: Vec<f64> = above.iter().copied().filter(|p| lufs(*p) > rel).collect();
        lufs(kept.iter().sum::<f64>() / kept.len().max(1) as f64)
    });
    // LRA: short-term values, gated at -70 and 20 LU under their mean, 10th to 95th percentile
    let st: Vec<f64> = short.iter().copied().filter(|p| lufs(*p) > -70.0).collect();
    let lra = (st.len() >= 2).then(|| {
        let rel = lufs(st.iter().sum::<f64>() / st.len() as f64) - 20.0;
        let mut v: Vec<f64> = st.iter().map(|p| lufs(*p)).filter(|l| *l > rel).collect();
        v.sort_by(|a, b| a.total_cmp(b));
        let at = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
        at(0.95) - at(0.10)
    });
    let left: Vec<f32> = samples.iter().step_by(2).copied().collect();
    let right: Vec<f32> = samples.iter().skip(1).step_by(2).copied().collect();
    let tp = loom_audio::truepeak::true_peak(&left).max(loom_audio::truepeak::true_peak(&right));
    let max = |v: &[f64]| v.iter().copied().fold(None, |m: Option<f64>, p| Some(m.map_or(p, |m| m.max(p)))).map(lufs).filter(|l| *l > -70.0);
    Report {
        integrated,
        short_term_max: max(&short),
        momentary_max: max(&momentary),
        lra,
        true_peak: 20.0 * (tp.max(1e-10) as f64).log10(),
    }
}

impl Report {
    /// `-13.8 LUFS · LRA 6.2 LU · -1.2 dBTP`
    pub fn text(&self) -> String {
        let f = |v: Option<f64>| v.map_or("−∞".into(), |v| format!("{v:.1}"));
        format!("{} LUFS intégré · LRA {} LU · crête vraie {:.1} dBTP · court max {} · moment max {}",
            f(self.integrated), f(self.lra), self.true_peak, f(self.short_term_max), f(self.momentary_max))
    }

    /// Advice against a platform target: how far, and whether the peaks are safe.
    pub fn against(&self, name: &str) -> Option<String> {
        let (lufs, tp) = target(name)?;
        let i = self.integrated?;
        let diff = i - lufs;
        let level = if diff.abs() < 0.5 { "pile sur la cible".to_string() }
            else if diff > 0.0 { format!("{diff:.1} dB trop fort : la plateforme baissera") }
            else { format!("{:.1} dB trop doux", -diff) };
        let peaks = if self.true_peak > tp { format!(", crête vraie {:.1} > {tp} dBTP : risque de saturation à l'encodage", self.true_peak) } else { String::new() };
        Some(format!("{name} ({lufs} LUFS) : {level}{peaks}"))
    }
}

/// The gain (linear) that brings `report` to `lufs`, held so the true peak stays under `max_tp` dBTP.
pub fn normalize_gain(report: &Report, lufs: f64, max_tp: f64) -> Option<f64> {
    let i = report.integrated?;
    let want = 10f64.powf((lufs - i) / 20.0);
    let room = 10f64.powf((max_tp - report.true_peak) / 20.0);
    Some(want.min(room))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stereo_sine(freq: f32, amp: f32, sr: u32, seconds: f32) -> Vec<f32> {
        (0..(sr as f32 * seconds) as usize)
            .flat_map(|i| { let v = amp * (std::f32::consts::TAU * freq * i as f32 / sr as f32).sin(); [v, v] })
            .collect()
    }

    #[test]
    fn reference_ebu() {
        // EBU Tech 3341 case 1 and 2: a 1 kHz stereo sine at -23 dBFS reads -23 LUFS
        let r = measure(&stereo_sine(1000.0, 10f32.powf(-23.0 / 20.0), 48_000, 20.0), 48_000);
        assert!((r.integrated.unwrap() + 23.0).abs() < 0.1, "{r:?}");
        assert!(r.lra.unwrap() < 0.2, "steady tone, no range");
        assert!((r.true_peak + 23.0).abs() < 0.2);
        let r = measure(&stereo_sine(1000.0, 10f32.powf(-33.0 / 20.0), 44_100, 20.0), 44_100);
        assert!((r.integrated.unwrap() + 33.0).abs() < 0.1, "{r:?}");
    }

    #[test]
    fn le_silence_ne_tire_pas_vers_le_bas() {
        // 10 s at -23 then 10 s of silence: still -23 thanks to the gates
        let mut s = stereo_sine(1000.0, 10f32.powf(-23.0 / 20.0), 48_000, 10.0);
        s.extend(std::iter::repeat_n(0.0, 48_000 * 20));
        let r = measure(&s, 48_000);
        assert!((r.integrated.unwrap() + 23.0).abs() < 0.1, "{r:?}");
    }

    #[test]
    fn plage_de_loudness() {
        // EBU 3342 style: 20 s at -20 then 20 s at -30 LUFS gives about 10 LU of range
        let mut s = stereo_sine(1000.0, 10f32.powf(-20.0 / 20.0), 48_000, 20.0);
        s.extend(stereo_sine(1000.0, 10f32.powf(-30.0 / 20.0), 48_000, 20.0));
        let lra = measure(&s, 48_000).lra.unwrap();
        assert!((lra - 10.0).abs() < 1.0, "LRA {lra}");
    }

    #[test]
    fn cibles_et_normalisation() {
        let r = measure(&stereo_sine(1000.0, 10f32.powf(-23.0 / 20.0), 48_000, 10.0), 48_000);
        let g = normalize_gain(&r, -14.0, -1.0).unwrap();
        assert!((20.0 * g.log10() - 9.0).abs() < 0.2, "{g}");
        assert!(r.against("spotify").unwrap().contains("trop doux"));
        assert!(r.against("inconnu").is_none());
        // a loud peak limits the gain
        let mut hot = r.clone();
        hot.true_peak = -3.0;
        assert!((20.0 * normalize_gain(&hot, -14.0, -1.0).unwrap().log10() - 2.0).abs() < 0.01);
    }
}
