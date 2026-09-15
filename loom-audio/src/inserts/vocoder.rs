//! `| vocode SAMPLE`: the browser's 12-band channel vocoder. The voice (carrier)
//! and a looping sample (modulator) are split into the same band-passes; each
//! modulator band's rectified, 20 Hz-smoothed level ×7 sets the gain of the
//! matching carrier band — the synth "talks".

use super::prims::{per_channel, q_from_db};
use super::Stage;
use crate::dsp::Biquad;
use crate::note::Stereo;
use crate::sample::SampleData;
use std::sync::Arc;

const BANDS: [f32; 12] = [140.0, 200.0, 290.0, 400.0, 560.0, 800.0, 1150.0, 1600.0, 2300.0, 3300.0, 4700.0, 6800.0];

struct Band {
    carrier: [Biquad; 2],
    modulator: Biquad,
    envelope: Biquad,
}

pub(super) struct Vocoder {
    data: Arc<SampleData>,
    /// Read position in the sample (frames) and the step per output sample.
    pos: f64,
    step: f64,
    bands: Vec<Band>,
    gains: [f32; 12],
}

impl Vocoder {
    /// `data` must hold at least one channel of two or more frames.
    pub fn new(data: Arc<SampleData>, sr: f32) -> Vocoder {
        let bands = BANDS.iter().map(|&f| {
            let bp = Biquad::bandpass(sr, f, 7.0);
            Band { carrier: [bp.clone(), bp.clone()], modulator: bp, envelope: Biquad::lowpass(sr, 20.0, q_from_db(1.0)) }
        }).collect();
        let step = (data.sample_rate as f64 / sr as f64).clamp(0.0, 64.0);
        let step = if step.is_finite() { step } else { 1.0 };
        Vocoder { data, pos: 0.0, step, bands, gains: [0.0; 12] }
    }

    /// The looping modulator, played at its own rate (mono mix of its channels —
    /// the browser rectifies each channel before mixing; close enough for an envelope).
    fn modulator(&mut self) -> f32 {
        let len = self.data.channels[0].len();
        let i = self.pos as usize % len;
        let frac = (self.pos - self.pos.floor()) as f32;
        let j = (i + 1) % len;
        let chans = &self.data.channels;
        let mut s = 0.0;
        for c in chans {
            let (a, b) = (c.get(i).copied().unwrap_or(0.0), c.get(j).copied().unwrap_or(0.0));
            s += a + (b - a) * frac;
        }
        self.pos += self.step;
        if self.pos >= len as f64 { self.pos -= len as f64; }
        s / chans.len() as f32
    }
}

impl Stage for Vocoder {
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        let m = self.modulator();
        for (band, g) in self.bands.iter_mut().zip(&mut self.gains) {
            *g = band.envelope.process(band.modulator.process(m).abs()) * 7.0;
        }
        let gains = self.gains;
        let mut out = Stereo::ZERO;
        for (band, g) in self.bands.iter_mut().zip(gains) {
            out.add(per_channel(&mut band.carrier, x, mono, |bp, s| bp.process(s)).scale(g));
        }
        out
    }
    fn tail(&self) -> f32 { 0.1 }
}
