//! Synth drums — sample-by-sample ports of the browser engine's `kick`, `snare`,
//! `hat` and `clap` (web/index.html). Used when a drum voice has no `| sample`.

use crate::dsp::{exp_ramp, osc, Biquad, Noise};
use loom_core::Wave;
use std::f32::consts::TAU;

#[derive(Clone)]
enum Kind {
    Kick { phase: f32, click_hp: Biquad },
    Snare { p1: f32, p2: f32, hp: Biquad },
    Hat { phases: [f32; 6], hp: Biquad },
    Clap { bp: Biquad },
    /// Metronome tick: a 30 ms square blip (2000 Hz on the downbeat, 1250 Hz otherwise).
    Click { phase: f32, freq: f32, level: f32 },
}

const HAT_FREQS: [f32; 6] = [2003.0, 3674.0, 4985.0, 6291.0, 7531.0, 9270.0];

/// One drum hit, rendered until `done()`. Output is mono.
#[derive(Clone)]
pub struct Drum {
    kind: Kind,
    sr: f32,
    t: f32,
    vel: f32,
    noise: Noise,
}

impl Drum {
    /// `kick`/`bd`, `snare`/`sd`, `hat`/`hh`, `clap`/`cp`; `None` for other names.
    pub fn new(name: &str, sr: f32, vel: f32, seed: u32) -> Option<Drum> {
        let kind = match name {
            "kick" | "bd" => Kind::Kick { phase: 0.0, click_hp: Biquad::highpass(sr, 1200.0, std::f32::consts::FRAC_1_SQRT_2) },
            "snare" | "sd" => Kind::Snare { p1: 0.0, p2: 0.0, hp: Biquad::highpass(sr, 1700.0, std::f32::consts::FRAC_1_SQRT_2) },
            "hat" | "hh" => Kind::Hat { phases: [0.0; 6], hp: Biquad::highpass(sr, 8200.0, std::f32::consts::FRAC_1_SQRT_2) },
            "clap" | "cp" => Kind::Clap { bp: Biquad::bandpass(sr, 1500.0, 1.1) },
            _ => return None,
        };
        Some(Drum { kind, sr, t: 0.0, vel, noise: Noise::new(seed) })
    }

    /// The browser's `clickSound`: accent = first beat of the bar.
    pub fn click(sr: f32, accent: bool) -> Drum {
        let (freq, level) = if accent { (2000.0, 0.22) } else { (1250.0, 0.12) };
        Drum { kind: Kind::Click { phase: 0.0, freq, level }, sr, t: 0.0, vel: 1.0, noise: Noise::new(1) }
    }

    pub fn is_kick(&self) -> bool { matches!(self.kind, Kind::Kick { .. }) }

    pub fn done(&self) -> bool {
        self.t >= match self.kind {
            Kind::Kick { .. } => 0.36,
            Kind::Snare { .. } => 0.17,
            Kind::Hat { .. } => 0.06,
            Kind::Clap { .. } => 0.18,
            Kind::Click { .. } => 0.04,
        }
    }

    pub fn render(&mut self) -> f32 {
        let (t, v, sr) = (self.t, self.vel, self.sr);
        let s = match &mut self.kind {
            Kind::Kick { phase, click_hp } => {
                // sine body with a pitch drop 180→45 Hz + a short high-passed noise click
                let f = exp_ramp(180.0, 45.0, 0.0, 0.09, t);
                *phase = (*phase + f / sr) % 1.0;
                let body = if t < 0.36 { (*phase * TAU).sin() * exp_ramp(v, 0.001, 0.0, 0.34, t) } else { 0.0 };
                let click = click_hp.process(self.noise.sample());
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
                let n = hp.process(self.noise.sample());
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
                let n = self.noise.sample();
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
            Kind::Click { phase, freq, level } => {
                *phase = (*phase + *freq / sr) % 1.0;
                osc(Wave::Square, *phase, *freq / sr) * exp_ramp(*level, 0.001, 0.0, 0.03, t)
            }
        };
        self.t += 1.0 / sr;
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drums_are_audible_and_finish() {
        for name in ["kick", "snare", "hat", "clap"] {
            let mut d = Drum::new(name, 48000.0, 1.0, 7).unwrap();
            let (mut n, mut peak) = (0, 0.0f32);
            while !d.done() {
                let s = d.render();
                assert!(s.is_finite());
                peak = peak.max(s.abs());
                n += 1;
            }
            assert!(n > 1000 && n < 48000, "{name} lasted {n} samples");
            assert!(peak > 0.02 && peak < 2.0, "{name} peak {peak}");
        }
        assert!(Drum::new("banjo", 48000.0, 1.0, 7).is_none());
    }
}
