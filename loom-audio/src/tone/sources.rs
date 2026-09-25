//! The special sound sources of `tone()`: the buffer instruments (`pluck`,
//! `bow`, `reed`, `sync`), `pwm`, `bell` and `grain`.
//!
//! The browser renders the buffer instruments into an `AudioBuffer` up front.
//! Here each buffer is generated sample by sample while it plays: the maths is
//! identical, but only one string period of memory is needed instead of seconds.

use super::table::SineTable;
use crate::dsp::{osc, Noise};
use loom_core::Wave;
use std::f32::consts::FRAC_PI_2;

/// How the samples of a buffer instrument are made.
enum Gen {
    /// Karplus-Strong family: noise burst for one period, then
    /// `d[i] = (d[i-p] + d[i-p-1]) · gain + noise · inject`.
    /// `ring` holds the last p+1 samples.
    String { ring: Vec<f32>, p: usize, gain: f32, inject: f32, noise: Noise },
    /// Hard sync: a slave saw reset by the master, its ratio sweeping up.
    Sync { mp: f32, sp: f32, mi: f32, freq_sr: f32, max_ratio: f32 },
}

/// An `AudioBufferSourceNode` playing a generated buffer, with fractional
/// playback rate (for `wow` and pitch LFOs, which move its `detune`).
pub struct BufferPlayer {
    gen: Gen,
    /// Index of the next sample to generate, and the buffer length.
    i: usize,
    len: usize,
    /// The two samples around the read position, and the position between them.
    a: f32,
    b: f32,
    frac: f32,
}

/// JS: `N = min(floor(secs·sr), cap·sr)`, at least 1.
fn buffer_len(sr: f32, secs: f32, cap_secs: f32) -> usize {
    ((secs * sr).floor().min(sr * cap_secs) as usize).max(1)
}

impl BufferPlayer {
    fn start(gen: Gen, len: usize) -> BufferPlayer {
        let mut p = BufferPlayer { gen, i: 0, len, a: 0.0, b: 0.0, frac: 0.0 };
        p.a = p.generate();
        p.b = p.generate();
        p
    }

    /// `pluck`: JS `ksBuffer` — damping 0.985..0.9995 from the brightness.
    pub fn pluck(sr: f32, freq: f32, dur: f32, bright: f32, seed: u32) -> BufferPlayer {
        let damp = 0.985 + 0.0145 * bright.clamp(0.0, 1.0);
        Self::string(sr, freq, buffer_len(sr, dur + 0.6, 4.0), 0.5 * damp, 0.0, seed)
    }

    /// `bow`: JS `bowBuffer` — friction noise keeps re-exciting the string.
    pub fn bow(sr: f32, freq: f32, dur: f32, seed: u32) -> BufferPlayer {
        Self::string(sr, freq, buffer_len(sr, dur + 0.3, 4.0), 0.5 * 0.999, 0.04, seed)
    }

    /// `reed`: JS `reedBuffer` — inverted reflection (stopped pipe, odd harmonics).
    pub fn reed(sr: f32, freq: f32, dur: f32, seed: u32) -> BufferPlayer {
        Self::string(sr, freq, buffer_len(sr, dur + 0.3, 4.0), -0.5 * 0.997, 0.03, seed)
    }

    fn string(sr: f32, freq: f32, len: usize, gain: f32, inject: f32, seed: u32) -> BufferPlayer {
        let p = ((sr / freq.max(20.0)).round() as usize).max(2);
        let gen = Gen::String { ring: vec![0.0; p + 1], p, gain, inject, noise: Noise::new(seed) };
        Self::start(gen, len)
    }

    /// `sync`: JS `syncBuffer` — ratio 1 → max over the first 62% of the buffer.
    pub fn sync(sr: f32, freq: f32, dur: f32, max_ratio: f32) -> BufferPlayer {
        let gen = Gen::Sync { mp: 0.0, sp: 0.0, mi: freq / sr, freq_sr: freq / sr, max_ratio };
        Self::start(gen, buffer_len(sr, dur + 0.1, 3.0))
    }

    /// Buffer sample `i` (0 after the end of the buffer).
    fn generate(&mut self) -> f32 {
        let (i, len) = (self.i, self.len);
        if i >= len { return 0.0; }
        self.i += 1;
        match &mut self.gen {
            Gen::String { ring, p, gain, inject, noise } => {
                let n = *p + 1;
                let slot = i % n; // holds d[i-p-1]; d[i-p] is in the next slot
                let x = if i < *p {
                    noise.sample()
                } else {
                    let a = ring[(slot + 1) % n];
                    let b = if i == *p { a } else { ring[slot] };
                    let y = (a + b) * *gain;
                    if *inject != 0.0 { y + noise.sample() * *inject } else { y }
                };
                ring[slot] = x;
                x
            }
            Gen::Sync { mp, sp, mi, freq_sr, max_ratio } => {
                let ratio = 1.0 + (*max_ratio - 1.0) * (i as f32 / len as f32 * 1.6).min(1.0);
                *mp += *mi;
                if *mp >= 1.0 { *mp -= 1.0; *sp = 0.0; }
                *sp += *freq_sr * ratio;
                if *sp >= 1.0 { *sp -= 1.0; }
                (2.0 * *sp - 1.0) * 0.5
            }
        }
    }

    /// Next output sample; `rate` = playback speed (1 = as recorded).
    #[inline]
    pub fn next(&mut self, rate: f32) -> f32 {
        let y = self.a + (self.b - self.a) * self.frac;
        self.frac += rate;
        while self.frac >= 1.0 {
            self.frac -= 1.0;
            self.a = self.b;
            self.b = self.generate();
        }
        y
    }
}

/// `pwm`: a saw plus an inverted copy delayed by half a period, the delay
/// swept by a sine LFO — a pulse whose width breathes.
pub struct Pwm {
    phase: f32,
    dt: f32,
    /// The saw's recent past: a power-of-two ring (masking is cheaper than `%`).
    ring: Vec<f32>,
    pos: usize,
    lfo_phase: f32,
    lfo_dt: f32,
    /// Delay centre and LFO swing, in samples, and the longest delay allowed.
    centre: f32,
    swing: f32,
    max_delay: f32,
}

impl Pwm {
    pub fn new(sr: f32, freq: f32, amount: f32) -> Pwm {
        // WebAudio DelayNode(0.05): the delay time can't exceed 50 ms
        let max_delay = ((0.5 + 0.4 * amount) / freq).min(0.05) * sr;
        Pwm {
            phase: 0.0,
            dt: freq / sr,
            ring: vec![0.0; (max_delay as usize + 2).next_power_of_two()],
            pos: 0,
            lfo_phase: 0.0,
            lfo_dt: (0.3 + amount * 4.0) / sr,
            centre: 0.5 / freq * sr,
            swing: 0.4 / freq * amount * sr,
            max_delay,
        }
    }

    /// `pitch` = detune multiplier (wow, pitch LFO) on the saw.
    #[inline]
    pub fn next(&mut self, pitch: f32, sine: &SineTable) -> f32 {
        let dt = self.dt * pitch;
        let saw = if dt < 0.5 { osc(Wave::Saw, self.phase, dt) } else { 0.0 };
        advance(&mut self.phase, dt);
        let d = (self.centre + self.swing * sine.at(self.lfo_phase)).clamp(0.0, self.max_delay);
        advance(&mut self.lfo_phase, self.lfo_dt);
        // write, then read `d` samples back with linear interpolation (like DelayNode)
        let mask = self.ring.len() - 1;
        self.pos = (self.pos + 1) & mask;
        self.ring[self.pos] = saw;
        let i = d as usize;
        let f = d - i as f32;
        let a = self.ring[(self.pos.wrapping_sub(i)) & mask];
        let b = self.ring[(self.pos.wrapping_sub(i + 1)) & mask];
        saw - (a + (b - a) * f)
    }
}

/// `bell`: modal synthesis — six inharmonic sine partials, each decaying on its own.
pub struct Bell { parts: Vec<Partial> }

/// `k` = per-sample decay factor; the gain stops at 0.0008 (WebAudio holds a ramp's end value).
struct Partial { phase: f32, dt: f32, gain: f32, k: f32 }

impl Bell {
    pub fn new(sr: f32, freq: f32, dur: f32) -> Bell {
        const RATIOS: [f32; 6] = [1.0, 2.0, 2.4, 3.0, 4.5, 5.33];
        const AMPS: [f32; 6] = [1.0, 0.55, 0.45, 0.38, 0.25, 0.18];
        let parts = (0..6)
            // a WebAudio sine above Nyquist is silent
            .filter(|&k| freq * RATIOS[k] < sr * 0.5)
            .map(|k| {
                let dec = (dur * (1.7 - 0.2 * k as f32)).max(0.12);
                let g0 = AMPS[k] * 0.5;
                // exponential ramp g0 → 0.0008 over `dec`, as a per-sample factor
                Partial { phase: 0.0, dt: freq * RATIOS[k] / sr, gain: g0, k: (0.0008 / g0).powf(1.0 / (dec * sr)) }
            })
            .collect();
        Bell { parts }
    }

    /// Add a block into `out`; `pitch[i]` = detune multiplier per sample.
    pub fn render(&mut self, pitch: &[f32], out: &mut [f32], sine: &SineTable) {
        for p in &mut self.parts {
            let (mut ph, mut g) = (p.phase, p.gain);
            for (o, &m) in out.iter_mut().zip(pitch) {
                *o += sine.at(ph) * g;
                advance(&mut ph, p.dt * m);
                g = (g * p.k).max(0.0008);
            }
            (p.phase, p.gain) = (ph, g);
        }
    }
}

/// `grain`: a cloud of short windowed grains spread over the note, each with
/// its own random detune and stereo position.
pub struct Grains { grains: Vec<Grain>, wave: Wave, first: usize }

struct Grain { start: u32, peak_at: u32, fall_end: u32, end: u32, phase: f32, dt: f32, gain: f32, up: f32, down: f32, gl: f32, gr: f32 }

impl Grains {
    pub fn new(sr: f32, freq: f32, dur: f32, amount: f32, wave: Wave, noise: &mut Noise) -> Grains {
        let count = (6.0 + amount * 42.0).round().clamp(6.0, 48.0) as usize;
        let glen = (dur / 3.0 + 0.02).min(0.13);
        let top = 0.6 / (count as f32).sqrt();
        let (rise, fall) = (glen * 0.4, glen * 0.6);
        let grains = (0..count)
            .map(|k| {
                let gt = k as f32 / count as f32 * dur + noise.unit() * dur / count as f32 * 0.7;
                let cents = noise.sample() * amount * 50.0;
                let (gl, gr) = equal_power(noise.sample() * 0.85);
                let start = (gt * sr) as u32;
                Grain {
                    start,
                    peak_at: start + (rise * sr) as u32,
                    fall_end: start + (glen * sr) as u32,
                    end: start + ((glen + 0.02) * sr) as u32, // JS: o.stop(gt + glen + 0.02)
                    phase: 0.0,
                    dt: freq * 2f32.powf(cents / 1200.0) / sr,
                    gain: 0.0001,
                    up: (top / 0.0001).powf(1.0 / (rise * sr).max(1.0)),
                    down: (0.0001 / top).powf(1.0 / (fall * sr).max(1.0)),
                    gl,
                    gr,
                }
            })
            .collect();
        Grains { grains, wave, first: 0 }
    }

    /// Stereo output at sample `n` of the note.
    #[inline]
    pub fn next(&mut self, n: u32) -> (f32, f32) {
        let (mut l, mut r) = (0.0, 0.0);
        // grains are sorted by start (and so by end): skip the finished ones
        while self.first < self.grains.len() && n >= self.grains[self.first].end { self.first += 1; }
        for g in &mut self.grains[self.first..] {
            if n < g.start { break; }
            if n >= g.end { continue; }
            let s = if g.dt < 0.5 { osc(self.wave, g.phase, g.dt) * g.gain } else { 0.0 };
            l += s * g.gl;
            r += s * g.gr;
            advance(&mut g.phase, g.dt);
            if n < g.peak_at { g.gain *= g.up } else if n < g.fall_end { g.gain *= g.down } else { g.gain = 0.0001 }
        }
        (l, r)
    }
}

/// WebAudio `StereoPannerNode` on a mono input: equal-power gains for `pan` -1..1.
pub fn equal_power(pan: f32) -> (f32, f32) {
    let x = (pan.clamp(-1.0, 1.0) + 1.0) * 0.5;
    ((x * FRAC_PI_2).cos(), (x * FRAC_PI_2).sin())
}

/// Move an oscillator phase by `dt`, wrapping into 0..1 (also for negative `dt`).
#[inline]
pub fn advance(phase: &mut f32, dt: f32) {
    *phase += dt;
    if !(0.0..1.0).contains(phase) {
        *phase -= phase.floor();
        if *phase >= 1.0 || !phase.is_finite() { *phase = 0.0; }
    }
}
