//! Space inserts: the tempo-synced `| echo`, the `| chorus` and the `| ir` reverb.

use super::prims::{per_channel, undenormal, Allpass, Osc, Panner, Ring};
use super::Stage;
use crate::dsp::{Biquad, BiquadKind, OnePole};
use crate::note::Stereo;

/// `| echo DIV FB`: a feedback delay of `bar/DIV` seconds (0.01..2), fed at 0.45.
/// Only the repeats come out (they go to the bus, not the sends).
pub(crate) struct Echo { ch: [Ring; 2], fb: f32, delay: f32 }

impl Echo {
    pub fn new(e: loom_core::Echo, bar_sec: f32, sr: f32) -> Echo {
        let delay = (bar_sec / e.div.max(0.01)).clamp(0.01, 2.0);
        let ring = Ring::secs(sr, delay, 1);
        let delay = ring.len() as f32 / sr;
        Echo { ch: [ring.clone(), ring], fb: e.fb.clamp(0.0, 0.95), delay }
    }

    /// Takes the chain signal at the tap point, returns the repeats.
    pub fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        let fb = self.fb;
        per_channel(&mut self.ch, x, mono, |ring, s| {
            let y = ring.out();
            ring.push(undenormal(0.45 * s + fb * y));
            y
        })
    }

    /// Repeats until they are 60 dB down (capped at 30 s).
    pub fn tail(&self) -> f32 {
        let repeats = if self.fb > 0.0 { (1e-3f32.ln() / self.fb.ln()).ceil() } else { 0.0 };
        (self.delay * (repeats + 1.0)).min(30.0)
    }
    pub fn gap(&self) -> f32 { self.delay }
}

/// A delay line read at a fractional, moving position (power-of-two size, so wrapping is a mask).
#[derive(Clone)]
struct SweptLine { buf: Vec<f32>, mask: usize, pos: usize }

impl SweptLine {
    fn new(max_samples: usize) -> SweptLine {
        let n = (max_samples + 2).next_power_of_two();
        SweptLine { buf: vec![0.0; n], mask: n - 1, pos: 0 }
    }
    /// Read `delay` samples back (≥ 1, fractional), then write `x`.
    #[inline]
    fn tick(&mut self, x: f32, delay: f32) -> f32 {
        let i = delay as usize;
        let frac = delay - i as f32;
        let a = self.buf[self.pos.wrapping_sub(i) & self.mask];
        let b = self.buf[self.pos.wrapping_sub(i + 1) & self.mask];
        self.pos = (self.pos + 1) & self.mask;
        self.buf[self.pos] = x;
        a + (b - a) * frac
    }
}

/// One chorus voice: a delay swept ±3 ms by a sine, then panned.
struct ChorusVoice { lines: [SweptLine; 2], lfo: Osc, base: f32, depth: f32, pan: Panner, delay: f32, step: f32 }

/// Samples between chorus LFO updates (the delay glides linearly in between).
const CHORUS_BLOCK: u32 = 16;

/// `| chorus N`: two modulated delays (15 ms @0.6 Hz panned -0.7, 22 ms @0.45 Hz
/// panned +0.7), each at `N·0.5`. Tapped from the end of the chain to the bus.
pub(crate) struct Chorus { voices: [ChorusVoice; 2], wet: f32, count: u32 }

impl Chorus {
    pub fn new(wet: f32, sr: f32) -> Chorus {
        let voice = |dt: f32, rate: f32, pan: f32| {
            let line = SweptLine::new((0.026 * sr) as usize);
            ChorusVoice { lines: [line.clone(), line], lfo: Osc::sine(rate, sr), base: dt * sr, depth: 0.003 * sr, pan: Panner::new(pan, false), delay: dt * sr, step: 0.0 }
        };
        Chorus { voices: [voice(0.015, 0.6, -0.7), voice(0.022, 0.45, 0.7)], wet: wet.clamp(0.0, 1.0) * 0.5, count: 0 }
    }

    pub fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        let update = self.count == 0;
        self.count = if update { CHORUS_BLOCK - 1 } else { self.count - 1 };
        let mut out = Stereo::ZERO;
        for v in &mut self.voices {
            if update {
                let target = v.base + v.depth * v.lfo.skip(CHORUS_BLOCK);
                v.step = (target - v.delay) / CHORUS_BLOCK as f32;
            }
            let d = v.delay;
            v.delay += v.step;
            let y = per_channel(&mut v.lines, x, mono, |line, s| line.tick(s, d));
            out.add(v.pan.apply(y, mono));
        }
        out.scale(self.wet)
    }

    pub fn tail(&self) -> f32 { 0.026 }
    pub fn gap(&self) -> f32 { 0.026 }
}

/// The `irBuffer()` spaces: IR length, polynomial decay power, brightness (Hz),
/// spring chirp — plus this port's network size and level calibration.
struct IrPreset { sec: f32, decay: f32, bright: f32, spring: bool, size: f32, norm: f32 }

fn ir_preset(kind: &str) -> IrPreset {
    match kind {
        "room" => IrPreset { sec: 0.5, decay: 3.6, bright: 4200.0, spring: false, size: 0.55, norm: 0.078 },
        "plate" => IrPreset { sec: 1.9, decay: 2.1, bright: 7200.0, spring: false, size: 0.8, norm: 0.093 },
        "spring" => IrPreset { sec: 1.7, decay: 2.6, bright: 2800.0, spring: true, size: 0.9, norm: 0.194 },
        "cathedral" => IrPreset { sec: 5.2, decay: 1.5, bright: 2400.0, spring: false, size: 1.9, norm: 0.219 },
        _ => IrPreset { sec: 2.8, decay: 2.0, bright: 3400.0, spring: false, size: 1.25, norm: 0.149 }, // hall
    }
}

/// Reverb time (-60 dB) that matches an IR whose amplitude falls as `(1-x)^decay`
/// over `sec`: a T30 fit (-5 → -35 dB) of its energy decay curve `(1-x)^(2·decay+1)`.
fn t60_of(sec: f32, decay: f32) -> f32 {
    let n = 10.0 * (2.0 * decay + 1.0);
    sec * 2.0 * (10f32.powf(-5.0 / n) - 10f32.powf(-35.0 / n))
}

const IR_LINES: [f32; 4] = [1116.0, 1277.0, 1422.0, 1617.0];
const IR_DIFFUSERS: [f32; 3] = [142.0, 379.0, 107.0];
/// An allpass with g = 0.6 loses ~90 dB in 40 round trips.
const IR_IDLE_TRIPS: f32 = 40.0;

/// `| ir hall|room|plate|spring|cathedral [MIX]`.
///
/// The browser convolves each voice with a multi-second generated noise IR —
/// far too costly per note natively. This is a small algorithmic reverb that
/// follows the same presets: a 4-line feedback delay network (Householder
/// mixing) whose reverb time matches the IR's decay curve, a lowpass at the
/// IR's brightness, spring = dispersive allpasses + the IR's flutter, and a
/// level calibrated to the normalised convolver (`ConvolverNode.normalize`).
/// Input L/R are summed (the convolver keeps them apart).
///
/// Every preset is darker than a quarter of the sample rate, so the network
/// runs at HALF rate (one step every two samples, output interpolated and then
/// smoothed by the brightness lowpass): half the cost per note.
pub(super) struct IrVerb {
    dry: f32,
    wet: f32,
    diffusers: [Allpass; 3],
    disperse: Option<[Biquad; 4]>,
    lines: [Ring; 4],
    gains: [f32; 4],
    /// In-loop damping: one-pole lowpass states sharing one coefficient.
    damp: [f32; 4],
    damp_coef: f32,
    tone: [OnePole; 2],
    flutter: Option<Osc>,
    norm: f32,
    t60: f32,
    /// Half-rate bookkeeping: the input summed over the pair, the last two
    /// network outputs, and which sample of the pair this is.
    acc: f32,
    prev: (f32, f32),
    next: (f32, f32),
    odd: bool,
    /// Silent network steps in a row, and after how many the input diffusers
    /// have rung out and can be skipped.
    idle: u32,
    idle_limit: u32,
}

impl IrVerb {
    pub fn new(kind: &str, mix: f32, sr: f32) -> IrVerb {
        let p = ir_preset(kind);
        let t60 = t60_of(p.sec, p.decay);
        let hr = sr * 0.5; // the network's rate
        let scale = hr / 44100.0;
        let lines = IR_LINES.map(|n| Ring::new((n * p.size * scale).round().max(1.0) as usize));
        let gains = [0, 1, 2, 3].map(|i| 10f32.powf(-3.0 * lines[i].len() as f32 / (hr * t60)));
        let wet = mix.clamp(0.0, 1.0);
        IrVerb {
            dry: 1.0 - wet,
            wet,
            diffusers: IR_DIFFUSERS.map(|n| Allpass::new((n * scale).round() as usize, 0.6)),
            disperse: p.spring.then(|| [1200.0, 2400.0, 3600.0, 4800.0].map(|f| Biquad::new(BiquadKind::Allpass, hr, f, 0.5, 0.0))),
            lines,
            gains,
            damp: [0.0; 4],
            damp_coef: 1.0 - (-std::f32::consts::TAU * (p.bright * 2.5).min(hr * 0.45) / hr).exp(),
            tone: std::array::from_fn(|_| OnePole::new(sr, p.bright.min(hr * 0.45))),
            flutter: p.spring.then(|| Osc::sine(120.0 / (std::f32::consts::TAU * p.sec), hr)),
            // the normalised IR's energy gain scales with 1/√sr; `norm` is measured at 48 kHz
            norm: p.norm * (48000.0 / sr).sqrt(),
            t60,
            acc: 0.0,
            prev: (0.0, 0.0),
            next: (0.0, 0.0),
            odd: false,
            idle: 0,
            // the slowest diffuser (379) plus the spring's allpasses, 40 trips each
            idle_limit: (IR_IDLE_TRIPS * 379.0 * scale) as u32 + if p.spring { 1000 } else { 0 },
        }
    }

    /// One step of the half-rate network: input → (left, right) reverb.
    fn step(&mut self, mut s: f32) -> (f32, f32) {
        self.idle = if s == 0.0 { self.idle.saturating_add(1) } else { 0 };
        if self.idle < self.idle_limit {
            if let Some(aps) = &mut self.disperse { s = aps.iter_mut().fold(s, |v, ap| ap.process(v)); }
            s = self.diffusers.iter_mut().fold(s, |v, ap| ap.process(v));
        }
        let o = [self.lines[0].out(), self.lines[1].out(), self.lines[2].out(), self.lines[3].out()];
        let mut v = [0.0f32; 4];
        for i in 0..4 {
            self.damp[i] += (o[i] - self.damp[i]) * self.damp_coef;
            v[i] = self.damp[i] * self.gains[i];
        }
        // Householder reflection: every line gets its own value minus half the total.
        // (No denormal guard needed: the note ends long before values get that small.)
        let half = (v[0] + v[1] + v[2] + v[3]) * 0.5;
        for (line, vi) in self.lines.iter_mut().zip(v) { line.push(undenormal(s + vi - half)); }
        let mut g = self.norm;
        if let Some(f) = &mut self.flutter { g *= 0.7 + 0.3 * f.next(); }
        ((o[0] - o[2]) * g, (o[1] - o[3]) * g)
    }
}

impl Stage for IrVerb {
    fn process(&mut self, x: Stereo, _mono: bool) -> Stereo {
        self.acc += (x.l + x.r) * 0.5;
        // run the network once per pair of samples; in between, output the midpoint
        let (wl, wr) = if self.odd {
            self.prev = self.next;
            let input = std::mem::take(&mut self.acc) * 0.5;
            self.next = self.step(input);
            ((self.prev.0 + self.next.0) * 0.5, (self.prev.1 + self.next.1) * 0.5)
        } else {
            self.next
        };
        self.odd = !self.odd;
        let wl = self.tone[0].process(wl);
        let wr = self.tone[1].process(wr);
        Stereo { l: x.l * self.dry + wl * self.wet, r: x.r * self.dry + wr * self.wet }
    }
    fn makes_stereo(&self) -> bool { true }
    fn tail(&self) -> f32 { self.t60 }
    fn gap(&self) -> f32 { 0.05 }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Energy gain (√Σh²) of the wet path, per output side.
    fn energy(kind: &str, sr: f32) -> (f32, f32) {
        let mut v = IrVerb::new(kind, 1.0, sr);
        let (mut el, mut er) = (0.0f64, 0.0f64);
        for i in 0..(8.0 * sr) as usize {
            let y = v.process(Stereo::mono(if i == 0 { 1.0 } else { 0.0 }), true);
            el += (y.l * y.l) as f64;
            er += (y.r * y.r) as f64;
        }
        (el.sqrt() as f32, er.sqrt() as f32)
    }

    #[test]
    fn ir_levels_match_the_normalised_convolver() {
        let mut bad = false;
        for kind in ["room", "hall", "plate", "spring", "cathedral"] {
            let p = ir_preset(kind);
            for sr in [44100.0, 48000.0] {
                // Chromium: IR RMS normalised to -58 dB at 44.1 kHz → Σh² = 44100²·1.5625e-6·sec/sr
                let target = 55.125 * (p.sec / sr).sqrt();
                let (l, r) = energy(kind, sr);
                let db = |e: f32| 20.0 * (e / target).log10();
                println!("{kind} @{sr}: L {:+.2} dB R {:+.2} dB (norm now {}, ideal {:.3})", db(l), db(r), p.norm, p.norm * target / ((l + r) * 0.5));
                bad |= db(l).abs() > 1.5 || db(r).abs() > 1.5;
            }
        }
        assert!(!bad, "an `ir` preset is off its calibrated level");
    }

    #[test]
    fn reverb_times_follow_the_presets() {
        assert!(t60_of(0.5, 3.6) < 0.7 && t60_of(5.2, 1.5) > 5.0);
        assert!(t60_of(0.5, 3.6) < t60_of(1.9, 2.1) && t60_of(1.9, 2.1) < t60_of(2.8, 2.0));
    }
}
