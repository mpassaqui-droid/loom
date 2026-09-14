//! The sequencer: turns a parsed LOOM `Program` into sound, one sample at a time.
//!
//! It keeps a sample clock. When the clock crosses a bar line it asks
//! `loom-core` for that bar's events and turns each into a synth `Voice` queued
//! at its exact sample. Nothing here touches a sound card, so the same code
//! drives live output (cpal) and offline WAV rendering — and is unit-testable.

use crate::synth::Voice;
use loom_core::Program;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};

/// Hard ceiling on simultaneous voices, so a very dense pattern can't choke
/// the audio thread (the browser engine has the same guard).
const MAX_VOICES: usize = 192;
/// Notes are released slightly before the next step, like the JS engine's 0.92 gate.
const GATE: f32 = 0.92;
/// Master compressor: above this level the mix is turned down by `COMP_RATIO`,
/// like the browser engine's DynamicsCompressor (keeps dense songs from smearing).
const COMP_THRESHOLD: f32 = 0.5;
const COMP_RATIO: f32 = 4.0;

/// Live transport state, written by the audio thread and read by the UIs
/// without locking.
#[derive(Default)]
pub struct Status {
    playing: AtomicBool,
    bar: AtomicI64,
    song_bar: AtomicI64,
    phase: AtomicU32,
    peak: AtomicU32,
    voices: AtomicU32,
}

impl Status {
    pub fn playing(&self) -> bool { self.playing.load(Ordering::Relaxed) }
    /// Bars elapsed since RUN.
    pub fn bar(&self) -> i64 { self.bar.load(Ordering::Relaxed) }
    /// Position inside the `song:` arrangement (wraps at its end). Equals `bar` without a song.
    pub fn song_bar(&self) -> i64 { self.song_bar.load(Ordering::Relaxed) }
    /// Position inside the current bar, 0..1.
    pub fn phase(&self) -> f32 { f32::from_bits(self.phase.load(Ordering::Relaxed)) }
    /// Output peak of the last audio block, 0..1.
    pub fn peak(&self) -> f32 { f32::from_bits(self.peak.load(Ordering::Relaxed)) }
    /// Number of voices currently sounding.
    pub fn voices(&self) -> u32 { self.voices.load(Ordering::Relaxed) }
}

pub struct Sequencer {
    sr: f32,
    program: Program,
    /// An edit waiting for the next bar line (live-coding never cuts mid-bar).
    pending: Option<Program>,
    playing: bool,
    bar: i64,
    song_bar: i64,
    pos: u64,
    bar_start: u64,
    bar_len: u64,
    /// Scheduled voices, sorted so the soonest start is at the END (cheap pop).
    queue: Vec<(u64, Voice)>,
    active: Vec<Voice>,
    volume: f32,
    muted: Vec<String>,
    seed: u32,
    peak: f32,
    /// Compressor envelope follower and its per-sample smoothing coefficients.
    env: f32,
    comp_attack: f32,
    comp_release: f32,
}

impl Sequencer {
    pub fn new(sample_rate: f32) -> Sequencer {
        Sequencer {
            sr: sample_rate, program: Program::parse(""), pending: None, playing: false,
            bar: 0, song_bar: 0, pos: 0, bar_start: 0, bar_len: 0,
            queue: Vec::new(), active: Vec::new(), volume: 0.8, muted: Vec::new(), seed: 1, peak: 0.0,
            env: 0.0,
            comp_attack: 1.0 - (-1.0 / (0.005 * sample_rate)).exp(),
            comp_release: 1.0 - (-1.0 / (0.15 * sample_rate)).exp(),
        }
    }

    pub fn sample_rate(&self) -> f32 { self.sr }
    pub fn is_playing(&self) -> bool { self.playing }

    /// Load new code. While playing, it takes effect at the next bar line.
    pub fn set_program(&mut self, program: Program) {
        if self.playing { self.pending = Some(program); } else { self.program = program; }
    }

    /// Start from bar 0.
    pub fn play(&mut self) {
        if let Some(p) = self.pending.take() { self.program = p; }
        self.playing = true;
        self.bar = 0;
        self.pos = 0;
        self.queue.clear();
        self.start_bar();
    }

    /// Stop the transport. Sounding notes ring out naturally.
    pub fn stop(&mut self) {
        self.playing = false;
        self.queue.clear();
        if let Some(p) = self.pending.take() { self.program = p; }
    }

    pub fn set_volume(&mut self, v: f32) { self.volume = v.clamp(0.0, 1.5); }

    pub fn set_muted(&mut self, voice: &str, muted: bool) {
        self.muted.retain(|m| m != voice);
        if muted { self.muted.push(voice.to_string()); }
    }

    fn start_bar(&mut self) {
        if let Some(p) = self.pending.take() { self.program = p; }
        let len = self.program.len_bars() as i64;
        self.song_bar = if len > 0 { self.bar.rem_euclid(len) } else { self.bar };
        let bar_sec = self.program.bar_seconds(self.song_bar) as f32;
        self.bar_start = self.pos;
        self.bar_len = ((bar_sec * self.sr) as u64).max(1);
        for ev in self.program.schedule_bar(self.song_bar) {
            if self.muted.iter().any(|m| *m == ev.voice) { continue; }
            let start = self.bar_start + (ev.phase.clamp(0.0, 0.9999) * self.bar_len as f32) as u64;
            self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let voice = if ev.is_drum {
                // The browser's synth drums ignore `| gain` (it's for samples, often set to
                // 0.4-0.6). Natively it may only turn a drum DOWN from the 0.22 default,
                // so songs never come out hotter than in the browser.
                Voice::drum(&ev.voice, self.sr, ev.vel * (ev.gain / 0.22).min(1.0), ev.pan, self.seed)
            } else {
                ev.freq.map(|f| Voice::tone(&ev.voice, ev.wave, f, ev.dur * bar_sec * GATE, ev.gain, ev.vel, ev.pan, self.sr))
            };
            if let Some(v) = voice { self.queue.push((start, v)); }
        }
        self.queue.sort_by(|a, b| b.0.cmp(&a.0));
    }

    /// Fill an interleaved buffer with `channels` channels (1 = mono, 2+ = L/R then silence).
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let mut peak = 0.0f32;
        for frame in out.chunks_mut(channels) {
            if self.playing && self.pos >= self.bar_start + self.bar_len {
                self.bar += 1;
                self.start_bar();
            }
            while self.queue.last().is_some_and(|(s, _)| *s <= self.pos) {
                let (_, v) = self.queue.pop().unwrap();
                if self.active.len() >= MAX_VOICES { self.active.remove(0); }
                self.active.push(v);
            }
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for v in &mut self.active { v.render(&mut l, &mut r); }
            self.active.retain(|v| !v.done());
            // master: volume → glue compressor → soft saturator (never hard digital clipping)
            l *= self.volume;
            r *= self.volume;
            let level = l.abs().max(r.abs());
            let coeff = if level > self.env { self.comp_attack } else { self.comp_release };
            self.env += (level - self.env) * coeff;
            if self.env > COMP_THRESHOLD {
                let over = self.env / COMP_THRESHOLD;
                let g = over.powf(1.0 / COMP_RATIO - 1.0);
                l *= g;
                r *= g;
            }
            l = l.tanh();
            r = r.tanh();
            peak = peak.max(l.abs()).max(r.abs());
            match frame {
                [m] => *m = (l + r) * 0.5,
                [a, b, rest @ ..] => { *a = l; *b = r; rest.fill(0.0); }
                [] => {}
            }
            self.pos += 1;
        }
        self.peak = peak;
    }

    /// Copy the transport state into a lock-free `Status` for the UI.
    pub fn publish(&self, s: &Status) {
        let phase = if self.playing && self.bar_len > 0 {
            (self.pos.saturating_sub(self.bar_start) as f32 / self.bar_len as f32).min(1.0)
        } else { 0.0 };
        s.playing.store(self.playing, Ordering::Relaxed);
        s.bar.store(self.bar, Ordering::Relaxed);
        s.song_bar.store(self.song_bar, Ordering::Relaxed);
        s.phase.store(phase.to_bits(), Ordering::Relaxed);
        s.peak.store(self.peak.to_bits(), Ordering::Relaxed);
        s.voices.store(self.active.len() as u32, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48000.0;

    fn bars(seq: &mut Sequencer, n: f32) -> Vec<f32> {
        let mut buf = vec![0.0; (n * 2.0 * SR) as usize * 2];
        seq.render(&mut buf, 2);
        buf
    }

    #[test]
    fn silent_until_play() {
        let mut s = Sequencer::new(SR);
        s.set_program(Program::parse("bpm: 120\nkick: x x x x"));
        assert!(bars(&mut s, 0.5).iter().all(|x| *x == 0.0));
    }

    #[test]
    fn kick_lands_on_the_beat() {
        let mut s = Sequencer::new(SR);
        s.set_program(Program::parse("bpm: 120\nkick: x . . ."));
        s.play();
        let buf = bars(&mut s, 1.0); // 120 bpm, 4 beats = 2 s
        let loud = |from: f32, to: f32| {
            let (a, b) = ((from * SR) as usize * 2, (to * SR) as usize * 2);
            buf[a..b].iter().fold(0.0f32, |m, x| m.max(x.abs()))
        };
        assert!(loud(0.0, 0.1) > 0.3, "kick at beat 1");
        assert!(loud(0.6, 1.9) < 0.001, "silence on beats 2-4");
    }

    #[test]
    fn edits_wait_for_the_bar_line() {
        let mut s = Sequencer::new(SR);
        s.set_program(Program::parse("bpm: 120\nkick: x . . ."));
        s.play();
        bars(&mut s, 0.25);
        s.set_program(Program::parse("bpm: 120\nhat: x x x x x x x x"));
        let st = Status::default();
        s.publish(&st);
        assert_eq!(st.bar(), 0);
        bars(&mut s, 1.0);
        s.publish(&st);
        assert_eq!(st.bar(), 1, "new code starts on bar 1");
        assert!(st.playing() && st.phase() > 0.2 && st.phase() < 0.3);
    }

    #[test]
    fn song_wraps_and_mute_silences() {
        let mut s = Sequencer::new(SR);
        s.set_program(Program::parse("bpm: 120\nsong: a*1 b*1\nbass: c2 . . . | saw"));
        s.play();
        bars(&mut s, 2.5);
        let st = Status::default();
        s.publish(&st);
        assert_eq!((st.bar(), st.song_bar()), (2, 0));
        s.set_muted("bass", true);
        s.stop();
        s.play();
        assert!(bars(&mut s, 1.0).iter().all(|x| x.abs() < 1e-6));
    }
}
