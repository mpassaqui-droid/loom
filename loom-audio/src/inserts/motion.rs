//! Level and position: LFO tremolo and pan, `| pan`, `orbit`, `tgate`, `haas`, `autopan`.

use super::prims::{MovingPan, Osc, Panner, Ring, Shape, PAN_BLOCK};
use super::Stage;
use crate::clock::Clock;
use crate::note::Stereo;
use loom_core::{Lfo, Param};

/// The JS `mkLFO` oscillator: rate ≥ 0.01 Hz, shape from `SHP`, scaled by `depth`.
fn lfo_osc(l: &Lfo, sr: f32, max_depth: f32) -> (Osc, f32) {
    (Osc::new(l.rate.max(0.01), sr, Shape::from_word(&l.shape)), l.depth.min(max_depth))
}

/// `| lfo gain|vol RATE DEPTH`: gain `1 + Σ lfo·min(0.9, depth)`.
pub(super) struct Tremolo { lfos: Vec<(Osc, f32)> }

impl Tremolo {
    pub fn new(lfos: &[&Lfo], sr: f32) -> Tremolo { Tremolo { lfos: lfos.iter().map(|l| lfo_osc(l, sr, 0.9)).collect() } }
}

impl Stage for Tremolo {
    fn process(&mut self, x: Stereo, _mono: bool) -> Stereo {
        let g = self.lfos.iter_mut().fold(1.0, |g, (o, d)| g + o.next() * *d);
        x.scale(g)
    }
}

/// `| lfo pan RATE DEPTH`: a panner swept by `Σ lfo·min(1, depth)`.
pub(super) struct LfoPan { lfos: Vec<(Osc, f32)>, pan: MovingPan }

impl LfoPan {
    pub fn new(lfos: &[&Lfo], sr: f32) -> LfoPan {
        LfoPan { lfos: lfos.iter().map(|l| lfo_osc(l, sr, 1.0)).collect(), pan: MovingPan::default() }
    }
}

impl Stage for LfoPan {
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        let lfos = &mut self.lfos;
        self.pan.apply(x, mono, || lfos.iter_mut().fold(0.0, |p, (o, d)| p + o.skip(PAN_BLOCK) * *d))
    }
    fn makes_stereo(&self) -> bool { true }
}

/// `| pan N` (or the layer pan of `jux`/`off`). A sweep/signal is read once at note start.
pub(super) struct Pan { param: Param, over: Option<f32>, salt: u32, pan: Panner }

impl Pan {
    pub fn new(param: Param, over: Option<f32>, salt: u32) -> Pan { Pan { param, over, salt, pan: Panner::new(0.0, true) } }
}

impl Stage for Pan {
    fn start(&mut self, clock: &Clock) {
        let p = clock.param(&self.param, self.over, self.salt);
        self.pan = Panner::new(if p.is_finite() { p } else { 0.0 }, true);
    }
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo { self.pan.apply(x, mono) }
    fn makes_stereo(&self) -> bool { true }
}

/// `| orbit RATE`: a sine pans ±0.95 while the level wobbles `0.7 + 0.3·sine` (distance).
pub(super) struct Orbit { osc: Osc, pan: MovingPan, gain: f32, gain_step: f32 }

impl Orbit {
    pub fn new(rate: f32, sr: f32) -> Orbit {
        Orbit { osc: Osc::sine(rate.clamp(0.02, 20.0), sr), pan: MovingPan::default(), gain: 0.7, gain_step: 0.0 }
    }
}

impl Stage for Orbit {
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        let (osc, gain, gain_step) = (&mut self.osc, self.gain, &mut self.gain_step);
        let y = self.pan.apply(x, mono, || {
            let s = osc.skip(PAN_BLOCK);
            // the distance wobble glides along with the position
            *gain_step = (0.7 + 0.3 * s - gain) / PAN_BLOCK as f32;
            0.95 * s
        });
        let out = y.scale(self.gain);
        self.gain += self.gain_step;
        out
    }
    fn makes_stereo(&self) -> bool { true }
}

/// `| tgate N`: a square LFO at `N / bar` Hz switches the sound fully on and off.
pub(super) struct TGate { osc: Osc, half_period: f32 }

impl TGate {
    pub fn new(n: u32, bar_sec: f32, sr: f32) -> TGate {
        let hz = n as f32 / bar_sec.max(0.1);
        TGate { osc: Osc::new(hz, sr, Shape::Square), half_period: 0.5 / hz }
    }
}

impl Stage for TGate {
    fn process(&mut self, x: Stereo, _mono: bool) -> Stereo { x.scale(0.5 + 0.5 * self.osc.next()) }
    fn gap(&self) -> f32 { self.half_period }
}

/// `| haas N`: the dry signal panned -0.75 plus a copy delayed 8..30 ms panned +0.75.
pub(super) struct Haas { ch: [Ring; 2], delay: f32, left: Panner, right: Panner }

impl Haas {
    pub fn new(h: f32, sr: f32) -> Haas {
        let delay = 0.008 + 0.022 * h;
        let ring = Ring::secs(sr, delay, 1);
        Haas { ch: [ring.clone(), ring], delay, left: Panner::new(-0.75, false), right: Panner::new(0.75, false) }
    }
}

impl Stage for Haas {
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        let d = Stereo { l: self.ch[0].out(), r: self.ch[1].out() };
        self.ch[0].push(x.l);
        self.ch[1].push(x.r);
        let mut y = self.left.apply(x, mono);
        y.add(self.right.apply(d, mono));
        y
    }
    fn makes_stereo(&self) -> bool { true }
    fn tail(&self) -> f32 { self.delay }
    fn gap(&self) -> f32 { self.delay }
}

/// `| autopan RATE`: a sine sweeps the pan ±0.92.
pub(super) struct AutoPan { osc: Osc, pan: MovingPan }

impl AutoPan {
    pub fn new(rate: f32, sr: f32) -> AutoPan { AutoPan { osc: Osc::sine(rate, sr), pan: MovingPan::default() } }
}

impl Stage for AutoPan {
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo {
        let osc = &mut self.osc;
        self.pan.apply(x, mono, || 0.92 * osc.skip(PAN_BLOCK))
    }
    fn makes_stereo(&self) -> bool { true }
}
