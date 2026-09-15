//! The master SPACE reverb and ECHO ping-pong delay, as wired in `buildGraph()`.
//! Each channel runs its own copy of the network, like the stereo WebAudio graph.

use crate::dsp::Biquad;
use crate::inserts::prims::{undenormal, Allpass, Panner, Ring, QUANTUM};
use crate::note::Stereo;
use loom_core::Room;
use std::f32::consts::FRAC_1_SQRT_2;

/// Delay-time → samples, clamped to one render quantum like any WebAudio delay in a loop.
fn loop_len(sr: f32, secs: f32) -> usize { ((secs * sr).round() as usize).max(QUANTUM) }

/// Lowpass inside a feedback loop. The JS uses WebAudio's default `Q` (1 dB),
/// which peaks ~2 dB near the cutoff and puts the loop gain at or above 1 there
/// (a marginally unstable ring). A flat Butterworth Q keeps the same damping
/// without the risk.
fn loop_lowpass(sr: f32, freq: f32) -> Biquad { Biquad::lowpass(sr, freq, FRAC_1_SQRT_2) }

/// Schroeder/Moorer: 4 damped feedback combs in parallel → 2 allpasses.
#[derive(Clone)]
pub(crate) struct SchroederChannel { combs: Vec<(Ring, Biquad)>, aps: [Allpass; 2] }

impl SchroederChannel {
    fn new(sr: f32) -> Self {
        let combs = [0.0297, 0.0371, 0.0411, 0.0437].iter().map(|&t| (Ring::new(loop_len(sr, t)), loop_lowpass(sr, 3800.0))).collect();
        SchroederChannel { combs, aps: [Allpass::new(loop_len(sr, 0.005), 0.5), Allpass::new(loop_len(sr, 0.0017), 0.5)] }
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let mut sum = 0.0;
        for (ring, lp) in &mut self.combs {
            let d = ring.out();
            sum += d;
            ring.push(undenormal(x + 0.8 * lp.process(d)));
        }
        let y = self.aps[0].process(sum);
        self.aps[1].process(y)
    }
}

/// Jot feedback delay network: 8 lines mixed by an orthogonal Hadamard matrix
/// scaled by `decay`, each line damped by a lowpass, then 2 allpasses.
#[derive(Clone)]
pub(crate) struct FdnChannel { lines: Vec<Ring>, lps: Vec<Biquad>, decay: f32, aps: [Allpass; 2] }

const FDN_TIMES: [f32; 8] = [0.0431, 0.0530, 0.0617, 0.0727, 0.0843, 0.0937, 0.1051, 0.1153];

impl FdnChannel {
    fn new(sr: f32, decay: f32, damp: f32) -> Self {
        FdnChannel {
            lines: FDN_TIMES.iter().map(|&t| Ring::new(loop_len(sr, t))).collect(),
            lps: (0..8).map(|_| loop_lowpass(sr, damp)).collect(),
            decay,
            aps: [Allpass::new(loop_len(sr, 0.0047), 0.5), Allpass::new(loop_len(sr, 0.0019), 0.5)],
        }
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let norm = 1.0 / 8f32.sqrt();
        let mut out = 0.0;
        let mut v = [0.0f32; 8];
        for i in 0..8 {
            let d = self.lines[i].out();
            out += d;
            v[i] = self.lps[i].process(d);
        }
        // fast Walsh–Hadamard transform = multiplying by the ±1 Hadamard matrix
        let mut h = 1;
        while h < 8 {
            for i in (0..8).step_by(h * 2) {
                for j in i..i + h {
                    let (a, b) = (v[j], v[j + h]);
                    v[j] = a + b;
                    v[j + h] = a - b;
                }
            }
            h *= 2;
        }
        for (i, line) in self.lines.iter_mut().enumerate() {
            line.push(undenormal(norm * x + v[i] * norm * self.decay));
        }
        let y = self.aps[0].process(out);
        self.aps[1].process(y)
    }
}

/// The master reverb, chosen by `room:`.
#[derive(Clone)]
pub(crate) enum Reverb {
    Schroeder([SchroederChannel; 2]),
    Fdn([FdnChannel; 2]),
}

impl Reverb {
    pub fn new(sr: f32, room: &Room) -> Reverb {
        if room.kind.as_deref() == Some("fdn") {
            let decay = room.decay.unwrap_or(0.84).clamp(0.0, 0.97);
            let damp = room.damp.unwrap_or(4200.0);
            let ch = FdnChannel::new(sr, decay, damp);
            Reverb::Fdn([ch.clone(), ch])
        } else {
            let ch = SchroederChannel::new(sr);
            Reverb::Schroeder([ch.clone(), ch])
        }
    }

    #[inline]
    pub fn process(&mut self, x: Stereo) -> Stereo {
        match self {
            Reverb::Schroeder([l, r]) => Stereo { l: l.process(x.l), r: r.process(x.r) },
            Reverb::Fdn([l, r]) => Stereo { l: l.process(x.l), r: r.process(x.r) },
        }
    }
}

/// The ping-pong delay: two 0.2 s lines cross-fed at 0.32, left line panned
/// -0.9 and right line +0.9 (WebAudio's stereo panner law).
#[derive(Clone)]
pub(crate) struct PingPong { left: [Ring; 2], right: [Ring; 2], pan_l: Panner, pan_r: Panner }

impl PingPong {
    pub fn new(sr: f32) -> PingPong {
        let line = || Ring::new(loop_len(sr, 0.2));
        PingPong { left: [line(), line()], right: [line(), line()], pan_l: Panner::new(-0.9, false), pan_r: Panner::new(0.9, false) }
    }

    #[inline]
    pub fn process(&mut self, x: Stereo) -> Stereo {
        let dl = Stereo { l: self.left[0].out(), r: self.left[1].out() };
        let dr = Stereo { l: self.right[0].out(), r: self.right[1].out() };
        // L line ← input + 0.32·R line ; R line ← 0.32·L line
        self.left[0].push(undenormal(x.l + 0.32 * dr.l));
        self.left[1].push(undenormal(x.r + 0.32 * dr.r));
        self.right[0].push(undenormal(0.32 * dl.l));
        self.right[1].push(undenormal(0.32 * dl.r));
        let mut out = self.pan_l.apply(dl, false);
        out.add(self.pan_r.apply(dr, false));
        out
    }
}
