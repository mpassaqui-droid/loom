//! The sequencer: turns a parsed LOOM `Program` into sound, one sample at a time.
//!
//! It keeps a sample clock. When the clock crosses a bar line it asks
//! `loom-core` for that bar's events and turns each into a [`Note`] (synth
//! drum, sample, or tone with its insert chain) queued at its exact sample —
//! the port of the browser's `trigger()`. Notes are mixed into buses and the
//! [`Master`]. Nothing here touches a sound card, so the same code drives live
//! output (cpal) and offline WAV rendering, and is unit-testable.

use crate::clock::{name_salt, Clock};
use crate::drums::Drum;
use crate::master::{Master, MixInput};
use crate::note::{Note, NoteSpec, Route, Stereo};
use crate::sample::{SampleBank, SampleVoice};
use loom_core::{note_to_freq, rng01, Event, Program};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Hard ceiling on simultaneous notes, so a very dense pattern can't choke the
/// audio thread (the browser engine has the same guard).
const MAX_NOTES: usize = 256;

/// One note starting, for visuals (the browser's `vizQ` bursts).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Seconds since RUN.
    pub at: f64,
    /// `clock::name_salt` of the voice name — match it against the program's voices.
    pub voice: u32,
    /// Colour, 0..360 (`| hue`, else derived from the voice name like the browser).
    pub hue: f32,
    /// Loudness 0..1 (the JS `min(1, gain*2)`).
    pub gain: f32,
    pub drum: bool,
}

/// A mixer channel's live setting for one voice.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub name: String,
    /// Fader, 0..2 (1 = as written).
    pub gain: f32,
    /// Extra pan, -1..1.
    pub pan: f32,
    pub solo: bool,
}

/// Samples of the output kept for oscilloscopes and spectrum analysers.
pub const SCOPE_LEN: usize = 2048;

/// What the audio thread shares with visualisers. Written with `try_lock` only,
/// so a busy UI can never make the sound stutter.
pub struct VisualFeed {
    hits: VecDeque<Hit>,
    /// MIDI controller moves (cc, value) not yet read by the UI.
    controls: VecDeque<(u8, u8)>,
    /// Peak level per voice (`name_salt`, level) over the last block.
    levels: Vec<(u32, f32)>,
    scope: Vec<f32>,
    write: usize,
}

impl Default for VisualFeed {
    fn default() -> Self { VisualFeed { hits: VecDeque::new(), controls: VecDeque::new(), levels: Vec::new(), scope: vec![0.0; SCOPE_LEN], write: 0 } }
}

/// Live transport state, written by the audio thread and read by the UIs
/// without locking.
#[derive(Default)]
pub struct Status {
    visual: Mutex<VisualFeed>,
    playing: AtomicBool,
    bar: AtomicI64,
    song_bar: AtomicI64,
    phase: AtomicU32,
    peak: AtomicU32,
    voices: AtomicU32,
    pending: AtomicBool,
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
    /// Number of notes currently sounding.
    pub fn voices(&self) -> u32 { self.voices.load(Ordering::Relaxed) }
    /// True while new code waits for the next bar line.
    pub fn pending(&self) -> bool { self.pending.load(Ordering::Relaxed) }

    /// Notes that started since the last call (oldest first).
    pub fn take_hits(&self) -> Vec<Hit> {
        self.visual.lock().map(|mut v| v.hits.drain(..).collect()).unwrap_or_default()
    }

    /// Copy the last `SCOPE_LEN` output samples (mono, oldest first) into `out`.
    pub fn scope(&self, out: &mut [f32]) {
        let Ok(v) = self.visual.lock() else { return };
        for (i, o) in out.iter_mut().take(SCOPE_LEN).enumerate() {
            *o = v.scope[(v.write + i) % SCOPE_LEN];
        }
    }

    /// MIDI knob/fader moves `(cc, value 0..127)` since the last call (for MIDI learn).
    pub fn take_controls(&self) -> Vec<(u8, u8)> {
        self.visual.lock().map(|mut v| v.controls.drain(..).collect()).unwrap_or_default()
    }

    /// MIDI thread: record a controller move.
    pub fn push_control(&self, cc: u8, value: u8) {
        if let Ok(mut v) = self.visual.lock() {
            if v.controls.len() >= 256 { v.controls.pop_front(); }
            v.controls.push_back((cc, value));
        }
    }

    /// Peak level of each voice (by `name_salt`) in the latest block, for mixer meters.
    pub fn voice_levels(&self) -> Vec<(u32, f32)> {
        self.visual.lock().map(|v| v.levels.clone()).unwrap_or_default()
    }

    /// Audio thread: hand over this block's hits, voice levels and output
    /// (skipped if the UI holds the lock).
    pub fn feed_visuals(&self, hits: &mut Vec<Hit>, levels: &[(u32, f32)], block: &[f32], channels: usize) {
        let Ok(mut v) = self.visual.try_lock() else { return };
        v.levels.clear();
        v.levels.extend_from_slice(levels);
        for h in hits.drain(..) {
            if v.hits.len() >= 512 { v.hits.pop_front(); }
            v.hits.push_back(h);
        }
        for frame in block.chunks(channels.max(1)) {
            let m = frame.iter().sum::<f32>() / frame.len() as f32;
            let w = v.write;
            v.scope[w] = m;
            v.write = (w + 1) % SCOPE_LEN;
        }
    }
}

/// A note for an external instrument (`| midi N`), timed in transport seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MidiNote {
    /// Seconds since RUN when the note starts.
    pub at: f64,
    /// Channel 1..16.
    pub channel: u8,
    pub note: u8,
    pub velocity: u8,
    pub duration: f32,
}

/// Something to send to the MIDI port, timed in transport seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MidiEvent {
    Note(MidiNote),
    /// One MIDI clock pulse (24 per quarter note).
    Clock { at: f64 },
    Start { at: f64 },
    Stop,
}

/// `morph: N` in progress: voices that changed fade from the old code to the
/// new one over N bars (equal-power), instead of switching abruptly.
struct Morph {
    /// The previous program, reduced to the voices that changed or vanished.
    old: Program,
    /// `name` / `name@section` keys of the voices that are new or changed.
    changed: Vec<String>,
    start_bar: i64,
    bars: u32,
}

fn voice_key(v: &loom_core::Voice) -> String {
    match &v.section { Some(s) => format!("{}@{}", v.name, s), None => v.name.clone() }
}

/// A note waiting for its start sample.
struct Queued {
    start: u64,
    note: Note,
}

/// JS `voiceHue`: a stable colour per voice name.
fn voice_hue(name: &str) -> f32 {
    // the browser's 32-bit FNV-1a of the name, mod 360
    (name.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619)) % 360) as f32
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
    /// Scheduled notes, sorted so the soonest start is at the END (cheap pop).
    queue: Vec<Queued>,
    notes: Vec<Note>,
    master: Master,
    mix: MixInput,
    clock: Clock,
    bank: Arc<SampleBank>,
    muted: Vec<String>,
    seed: u32,
    peak: f32,
    /// Last pitch per voice (for `| slide`) and running hit count (for `| chop`).
    last_freq: HashMap<String, f32>,
    chop_count: HashMap<String, u32>,
    /// MIDI produced by the last `render` call (`| midi` notes, clock). The live
    /// engine drains this and forwards it to the MIDI port.
    pub midi_out: Vec<MidiEvent>,
    /// Send MIDI clock + start/stop (sync a DAW or hardware to LOOM).
    midi_clock: bool,
    /// Tempo imposed by an external MIDI clock, replacing the song's own tempo.
    bpm_override: Option<f64>,
    /// Notes played live from a MIDI keyboard, by note number.
    live: Vec<(u8, LiveVoice)>,
    morph: Option<Morph>,
    metronome: bool,
    /// Notes that started during the last `render` call, for visuals.
    pub hits: Vec<Hit>,
    /// Mixer settings per voice (only voices that were touched).
    channels: Vec<Channel>,
    /// Peak level per voice over the last `render` call.
    pub levels: Vec<(u32, f32)>,
    /// Global swing 0..0.9 applied to voices without their own `| swing` (the SWING knob).
    swing: f32,
    /// Stems export: only this voice sounds.
    stem: Option<String>,
}

impl Sequencer {
    pub fn new(sample_rate: f32, bank: Arc<SampleBank>) -> Sequencer {
        let mut master = Master::new(sample_rate);
        let program = Program::parse("");
        master.configure(&program.mix);
        let mix = MixInput { buses: vec![Default::default(); master.bus_count()], ..Default::default() };
        Sequencer {
            sr: sample_rate, program, pending: None, playing: false,
            bar: 0, song_bar: 0, pos: 0, bar_start: 0, bar_len: 0,
            queue: Vec::new(), notes: Vec::new(), master, mix, clock: Clock::new(sample_rate), bank,
            muted: Vec::new(), seed: 1, peak: 0.0,
            last_freq: HashMap::new(), chop_count: HashMap::new(), midi_out: Vec::new(),
            midi_clock: false, bpm_override: None, live: Vec::new(),
            morph: None, metronome: false, hits: Vec::new(),
            channels: Vec::new(), levels: Vec::new(), swing: 0.0, stem: None,
        }
    }

    pub fn sample_rate(&self) -> f32 { self.sr }
    pub fn is_playing(&self) -> bool { self.playing }

    /// Load new code. While playing, it takes effect at the next bar line.
    pub fn set_program(&mut self, program: Program) {
        if self.playing { self.pending = Some(program); } else { self.install(program); }
    }

    /// Swap in new code at a bar line while playing, starting a `morph:` crossfade if asked.
    fn install_live(&mut self, program: Program) {
        let bars = program.mix.morph;
        if bars > 0 {
            let old_keys: Vec<(String, &loom_core::Voice)> = self.program.voices.iter().map(|v| (voice_key(v), v)).collect();
            let new_keys: Vec<(String, &loom_core::Voice)> = program.voices.iter().map(|v| (voice_key(v), v)).collect();
            let changed: Vec<String> = new_keys.iter()
                .filter(|(k, v)| old_keys.iter().find(|(ok, _)| ok == k).is_none_or(|(_, ov)| *ov != *v))
                .map(|(k, _)| k.clone()).collect();
            let mut old = self.program.clone();
            old.voices.retain(|v| {
                let k = voice_key(v);
                changed.contains(&k) || !new_keys.iter().any(|(nk, _)| *nk == k)
            });
            self.morph = (!old.voices.is_empty() || !changed.is_empty())
                .then_some(Morph { old, changed, start_bar: self.bar, bars });
        } else {
            self.morph = None;
        }
        self.install(program);
    }

    /// Mixer fader (0..2) for a voice; applies immediately, even to sounding notes.
    pub fn set_voice_gain(&mut self, voice: &str, gain: f32) { self.channel(voice).gain = gain.clamp(0.0, 2.0); }
    /// Mixer pan (-1..1) for a voice.
    pub fn set_voice_pan(&mut self, voice: &str, pan: f32) { self.channel(voice).pan = pan.clamp(-1.0, 1.0); }
    /// Mixer solo: while any voice is soloed, only soloed voices play (with `| solo` too).
    pub fn set_solo(&mut self, voice: &str, solo: bool) { self.channel(voice).solo = solo; }
    /// The SWING knob: delays every odd step of voices that don't set `| swing`.
    pub fn set_swing(&mut self, swing: f32) { self.swing = swing.clamp(0.0, 0.9); }
    /// Only let `voice` sound (stems export), or everything with `None`.
    pub fn set_stem(&mut self, voice: Option<&str>) { self.stem = voice.map(str::to_string); }

    fn channel(&mut self, voice: &str) -> &mut Channel {
        if let Some(i) = self.channels.iter().position(|c| c.name == voice) { return &mut self.channels[i]; }
        self.channels.push(Channel { name: voice.to_string(), gain: 1.0, pan: 0.0, solo: false });
        self.channels.last_mut().unwrap()
    }

    fn trim_for(&self, voice: &str) -> (f32, f32) {
        match self.channels.iter().find(|c| c.name == voice) {
            Some(c) => (c.gain * (1.0 - c.pan).min(1.0), c.gain * (1.0 + c.pan).min(1.0)),
            None => (1.0, 1.0),
        }
    }

    /// Tick on every beat (not recorded into `| midi`, not sent to effects).
    pub fn set_metronome(&mut self, on: bool) { self.metronome = on; }

    fn install(&mut self, program: Program) {
        self.master.configure(&program.mix);
        self.mix.buses.resize(self.master.bus_count(), Default::default());
        self.program = program;
    }

    /// Start from bar 0.
    pub fn play(&mut self) {
        if let Some(p) = self.pending.take() { self.install(p); }
        self.playing = true;
        self.bar = 0;
        self.pos = 0;
        self.queue.clear();
        self.chop_count.clear();
        if self.midi_clock { self.midi_out.push(MidiEvent::Start { at: 0.0 }); }
        self.start_bar();
    }

    /// Stop the transport. Sounding notes ring out naturally.
    pub fn stop(&mut self) {
        if self.playing && self.midi_clock { self.midi_out.push(MidiEvent::Stop); }
        self.playing = false;
        self.queue.clear();
        if let Some(p) = self.pending.take() { self.install(p); }
    }

    pub fn set_volume(&mut self, v: f32) { self.master.set_volume(v); }
    /// SPACE knob 0..100 (reverb amount for the whole mix).
    pub fn set_space(&mut self, knob: f32) { self.master.set_space(knob); }
    /// ECHO knob 0..100 (ping-pong delay amount for the whole mix).
    pub fn set_echo(&mut self, knob: f32) { self.master.set_echo(knob); }
    /// Master EQ in dB: low, mid, high.
    pub fn set_eq(&mut self, low: f32, mid: f32, high: f32) { self.master.set_eq(low, mid, high); }

    /// Seconds since RUN at the current sample position.
    pub fn transport_seconds(&self) -> f64 { self.pos as f64 / self.sr as f64 }

    pub fn set_midi_clock(&mut self, on: bool) { self.midi_clock = on; }

    /// Follow an external tempo (from MIDI clock), or `None` to use the song's.
    pub fn set_bpm_override(&mut self, bpm: Option<f64>) { self.bpm_override = bpm.filter(|b| (30.0..300.0).contains(b)); }

    /// A key pressed on a MIDI keyboard: play it with the `lead` voice's timbre
    /// (or the first melodic voice, or a plain saw) — the browser's `liveNoteOn`.
    pub fn live_note_on(&mut self, note: u8, velocity: u8) {
        self.live_note_off(note);
        let voice = self.program.voices.iter().find(|v| v.name == "lead" && !v.is_drum)
            .or_else(|| self.program.voices.iter().find(|v| !v.is_drum));
        let (wave, synth) = match voice {
            Some(v) => (v.wave, v.synth.clone()),
            None => (loom_core::Wave::Saw, Arc::new(loom_core::Synth::default())),
        };
        let route = Route {
            bus: self.master.bus_index(synth.bus.as_deref()),
            reverb: synth.reverb.unwrap_or(0.0).min(1.0),
            delay: synth.delay.unwrap_or(0.0).min(1.0),
        };
        self.live.push((note, LiveVoice::new(self.sr, wave, &synth, note, velocity, route)));
    }

    pub fn live_note_off(&mut self, note: u8) {
        for (n, v) in &mut self.live { if *n == note { v.release(); } }
    }

    pub fn set_muted(&mut self, voice: &str, muted: bool) {
        self.muted.retain(|m| m != voice);
        if muted { self.muted.push(voice.to_string()); }
    }

    /// Which bar of the arrangement plays at transport bar `bar` (honours `loop:`).
    fn song_bar_for(&self, bar: i64) -> i64 {
        let song = &self.program.song;
        let len = self.program.len_bars() as i64;
        if len == 0 { return bar; }
        if let Some(sec) = &self.program.mix.loop_section {
            let mut acc = 0i64;
            for (name, n) in song {
                if name == sec {
                    return acc + (bar - acc).rem_euclid(*n as i64);
                }
                acc += *n as i64;
            }
        }
        bar.rem_euclid(len)
    }

    fn start_bar(&mut self) {
        if let Some(p) = self.pending.take() { self.install_live(p); }
        self.song_bar = self.song_bar_for(self.bar);
        let bar_sec = match self.bpm_override {
            Some(bpm) => (60.0 / bpm * self.program.meter_beats) as f32,
            None => self.program.bar_seconds(self.song_bar) as f32,
        };
        self.bar_start = self.pos;
        self.bar_len = ((bar_sec * self.sr) as u64).max(1);

        // where the current section sits in the song (for `A..B` sweeps)
        self.clock.bar_sec = bar_sec;
        self.clock.song_bars = self.program.len_bars();
        let (mut acc, mut sec) = (0u32, (0.0, 0.0));
        for (_, n) in &self.program.song {
            if (self.song_bar as u32) < acc + n { sec = (acc as f64, *n as f64); break; }
            acc += n;
        }
        (self.clock.section_start, self.clock.section_len) = sec;

        // MIDI clock: 24 pulses per quarter note across this bar
        if self.midi_clock {
            let pulses = (24.0 * self.program.meter_beats).round().max(1.0) as usize;
            let t0 = self.bar_start as f64 / self.sr as f64;
            for i in 0..pulses {
                self.midi_out.push(MidiEvent::Clock { at: t0 + i as f64 / pulses as f64 * bar_sec as f64 });
            }
        }

        if self.metronome {
            let beats = self.program.meter_beats.round().max(1.0) as usize;
            for b in 0..beats {
                let start = self.bar_start + (b as f64 / beats as f64 * self.bar_len as f64) as u64;
                let click = Drum::click(self.sr, b == 0);
                self.queue.push(Queued { start, note: Note::drum(click, "metronome", None, Route::default()) });
            }
        }

        // morph: equal-power crossfade of the voices that changed
        let mut fade_new = 1.0f32;
        let mut old_events = Vec::new();
        let mut changed: Vec<String> = Vec::new();
        if let Some(m) = &self.morph {
            let off = (self.bar - m.start_bar).max(0);
            if off >= m.bars as i64 {
                self.morph = None;
            } else {
                let x = (off + 1) as f32 / (m.bars + 1) as f32;
                fade_new = (x * std::f32::consts::FRAC_PI_2).sin();
                let fade_old = (x * std::f32::consts::FRAC_PI_2).cos();
                old_events = m.old.schedule_bar(self.song_bar);
                old_events.iter_mut().for_each(|e| e.vel *= fade_old);
                changed = m.changed.clone();
            }
        }

        let mut events = self.program.schedule_bar(self.song_bar);
        if !changed.is_empty() {
            // which new-code voices are fading in (by name; section variants share the name)
            for e in events.iter_mut() {
                if changed.iter().any(|k| k == &e.voice || k.split('@').next() == Some(e.voice.as_str())) { e.vel *= fade_new; }
            }
        }
        events.extend(old_events);
        // by time, then voice, so a chord's notes sit next to each other
        events.sort_by(|a, b| a.phase.total_cmp(&b.phase).then_with(|| a.voice.cmp(&b.voice)));
        let mixer_solo = self.channels.iter().any(|c| c.solo);
        let any_solo = mixer_solo || events.iter().any(|e| e.synth.solo);
        // the SWING knob: push odd steps late on voices without their own swing
        if self.swing > 0.0 {
            for e in events.iter_mut() {
                let steps = (1.0 / e.dur.max(1e-4)).round();
                let idx = (e.phase * steps).round() as i64;
                let own = self.program.voices.iter().any(|v| v.name == e.voice && v.swing > 0.0);
                if !own && idx % 2 == 1 && steps >= 2.0 { e.phase += self.swing * e.dur; }
            }
        }
        let mut i = 0;
        while i < events.len() {
            // simultaneous notes of one voice form a chord (voicing, harmonizer, strum)
            let mut j = i + 1;
            while j < events.len() && events[j].voice == events[i].voice && (events[j].phase - events[i].phase).abs() < 1e-6 { j += 1; }
            let group = events[i..j].to_vec();
            i = j;
            let ev = &group[0];
            let soloed = ev.synth.solo || self.channels.iter().any(|c| c.solo && c.name == ev.voice);
            if ev.synth.mute || (any_solo && !soloed) || self.muted.contains(&ev.voice) { continue; }
            if self.stem.as_ref().is_some_and(|s| *s != ev.voice) { continue; }
            self.trigger(&group, bar_sec);
        }
        self.queue.sort_by_key(|a| std::cmp::Reverse(a.start));
    }

    /// The port of the browser's `trigger()` for one step of one voice.
    fn trigger(&mut self, group: &[Event], bar_sec: f32) {
        let before = self.queue.len();
        self.trigger_notes(group, bar_sec);
        let ev = &group[0];
        let hue = ev.synth.hue.unwrap_or_else(|| voice_hue(&ev.voice));
        let gain = (ev.gain * 2.0 * ev.vel).min(1.0);
        for q in &mut self.queue[before..] { q.note.visual = Some((hue, gain, ev.is_drum)); }
    }

    fn trigger_notes(&mut self, group: &[Event], bar_sec: f32) {
        let ev = &group[0];
        let s = ev.synth.clone();
        let step = (ev.phase * 1000.0) as usize;
        let seed = self.program.seed;

        // feel / humanize: small seeded nudges of timing and velocity
        let mut offset = 0.0f32;
        let mut vel = ev.vel;
        let feel = s.humanize.unwrap_or(self.program.mix.feel);
        if feel > 0.0 {
            offset += (rng01(seed, self.song_bar, &ev.voice, step * 3 + 7) - 0.5) * feel * 0.016;
            vel *= 1.0 + (rng01(seed, self.song_bar, &ev.voice, step * 3 + 11) - 0.5) * feel * 0.5;
        }
        // cresc / dim: ramp velocity across the bar
        if let Some(d) = s.dynamics {
            let ph = ev.phase.clamp(0.0, 1.0);
            vel *= 0.15 + 0.85 * if d > 0 { ph } else { 1.0 - ph };
        }
        let at = |this: &Self, extra: f32| -> u64 {
            let sec = ev.phase.clamp(0.0, 0.9999) * bar_sec + offset + extra;
            this.bar_start + (sec.max(0.0) * this.sr) as u64
        };
        let route = Route {
            bus: self.master.bus_index(s.bus.as_deref()),
            reverb: s.reverb.unwrap_or(0.0).min(1.0),
            delay: s.delay.unwrap_or(0.0).min(1.0),
        };
        let step_sec = ev.dur * bar_sec;
        // flam: a soft grace note just before the hit (drums and notes alike)
        let hits: &[(f32, f32)] = if s.flam { &[(-0.026, 0.45), (0.0, 1.0)] } else { &[(0.0, 1.0)] };

        if ev.is_drum {
            for &(dt, vm) in hits {
                let vm = vel * vm;
                let start = at(self, dt);
                if let Some(ch) = s.midi {
                    let gm = match ev.voice.as_str() { "kick" | "bd" => 36, "snare" | "sd" => 38, "hat" | "hh" => 42, "clap" | "cp" => 39, _ => 37 };
                    self.push_midi(start, ch, gm as f32, (110.0 * vm).round(), 0.1);
                    continue;
                }
                self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let spec = self.spec(ev, 0.0, step_sec, vm, bar_sec);
                if s.sample.is_some() {
                    let gain = (ev.gain * 4.0 * vm).min(1.0);
                    let chop = self.next_chop(&ev.voice);
                    if let Some(v) = SampleVoice::new(&self.bank, &spec, gain, 1.0, chop) {
                        self.queue.push(Queued { start, note: Note::sample(v, &spec, route) });
                        continue;
                    }
                }
                // The browser's synth drums ignore `| gain` (meant for samples);
                // natively it may only turn a drum DOWN from the 0.22 default.
                if let Some(d) = Drum::new(&ev.voice, self.sr, vm * (ev.gain / 0.22).min(1.0), self.seed) {
                    self.queue.push(Queued { start, note: Note::drum(d, &ev.voice, ev.pan, Route { reverb: 0.0, delay: 0.0, ..route }) });
                }
            }
            return;
        }

        // pitched: resolve the chord, then voicing → harmonizer → transpose → tuning
        let mut midis: Vec<f32> = group.iter()
            // JS `melFreqs`: on a melodic voice a plain `x` hit plays c3
            .filter_map(|e| e.freq.or_else(|| note_to_freq(if e.token == "x" { "c3" } else { &e.token })))
            .map(|f| 69.0 + 12.0 * (f / 440.0).log2())
            .collect();
        if midis.is_empty() { return; }
        if let Some(v) = &s.voicing { midis = voice_chord(midis, v); }
        let mut freqs: Vec<f32> = midis.iter().map(|m| 440.0 * 2f32.powf((m - 69.0) / 12.0)).collect();
        if let Some(ch) = &s.chord {
            freqs = freqs.iter().flat_map(|f| ch.iter().map(move |iv| f * 2f32.powf(*iv as f32 / 12.0))).collect();
        }
        if let Some(semi) = s.semi { freqs.iter_mut().for_each(|f| *f *= 2f32.powf(semi as f32 / 12.0)); }
        if self.program.mix.tune == "adaptive" && freqs.len() > 1 { freqs = adaptive_just(&freqs); }

        let dur = step_sec * s.gate.unwrap_or(1.0);
        for &(dt, vm) in hits {
            let vm = vel * vm;
            if let Some(ch) = s.midi {
                for f in &freqs {
                    let note = (69.0 + 12.0 * (f / 440.0).log2()).round().clamp(0.0, 127.0);
                    let start = at(self, dt);
                    self.push_midi(start, ch, note, (96.0 * vm).round(), dur);
                }
                continue;
            }
            if s.sample.is_some() && (s.warp || s.freeze) {
                // JS `playSampleWarp`: one stretched hit per step at gain × velocity,
                // whatever the chord or pitch
                let spec = self.spec(ev, freqs[0], dur, ev.gain * vm, bar_sec);
                if let Some(v) = SampleVoice::new(&self.bank, &spec, ev.gain * vm, 1.0, 0) {
                    let start = at(self, dt);
                    self.queue.push(Queued { start, note: Note::sample(v, &spec, route) });
                }
                continue;
            }
            if s.sample.is_some() {
                let root = s.root.as_deref().and_then(note_to_freq).or_else(|| note_to_freq("c3")).unwrap_or(130.81);
                let gain = (ev.gain * 3.0 * vm).min(1.0);
                for f in &freqs {
                    let spec = self.spec(ev, *f, dur, gain, bar_sec);
                    let chop = self.next_chop(&ev.voice);
                    if let Some(v) = SampleVoice::new(&self.bank, &spec, gain, f / root, chop) {
                        let start = at(self, dt);
                        self.queue.push(Queued { start, note: Note::sample(v, &spec, route) });
                    }
                }
                continue;
            }
            let peak = if freqs.len() > 1 { ev.gain * 0.7 } else { ev.gain } * vm;
            for (k, f) in freqs.iter().enumerate() {
                let strum = s.strum.map_or(0.0, |st| k as f32 * st);
                let spec = self.spec(ev, *f, dur, peak, bar_sec);
                let note = Note::tone(&spec, &self.bank, route);
                // JS: `| pluck` voices don't feed the slide memory
                if s.pluck.is_none() { self.last_freq.insert(ev.voice.clone(), *f); }
                let start = at(self, dt + strum);
                self.queue.push(Queued { start, note });
            }
        }
    }

    fn spec<'a>(&mut self, ev: &'a Event, freq: f32, dur: f32, peak: f32, bar_sec: f32) -> NoteSpec<'a> {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        NoteSpec {
            name: &ev.voice,
            synth: &ev.synth,
            wave: ev.wave,
            freq,
            dur,
            peak,
            pan: ev.pan,
            prev_freq: self.last_freq.get(&ev.voice).copied(),
            sr: self.sr,
            bar_sec,
            seed: self.seed ^ name_salt(&ev.voice),
        }
    }

    fn next_chop(&mut self, voice: &str) -> u32 {
        let c = self.chop_count.entry(voice.to_string()).or_insert(0);
        *c += 1;
        *c
    }

    fn push_midi(&mut self, start: u64, channel: u8, note: f32, velocity: f32, duration: f32) {
        self.midi_out.push(MidiEvent::Note(MidiNote {
            at: start as f64 / self.sr as f64,
            channel,
            note: note.clamp(0.0, 127.0) as u8,
            velocity: velocity.clamp(1.0, 127.0) as u8,
            duration,
        }));
    }

    /// Fill an interleaved buffer with `channels` channels (1 = mono, 2+ = L/R then silence).
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let mut peak = 0.0f32;
        // refresh mixer trims once per block so faders act on sounding notes too
        if !self.channels.is_empty() {
            for i in 0..self.notes.len() {
                let t = self.trim_for(&self.notes[i].name);
                self.notes[i].trim = t;
            }
        }
        self.levels.clear();
        for frame in out.chunks_mut(channels) {
            if self.playing && self.pos >= self.bar_start + self.bar_len {
                self.bar += 1;
                self.start_bar();
            }
            while self.queue.last().is_some_and(|q| q.start <= self.pos) {
                let q = self.queue.pop().unwrap();
                if q.note.is_kick() && self.program.mix.duck > 0.0 { self.master.duck(self.clock.bar_sec); }
                if let Some((hue, gain, drum)) = q.note.visual {
                    if self.hits.len() < 256 {
                        self.hits.push(Hit { at: self.pos as f64 / self.sr as f64, voice: name_salt(&q.note.name), hue, gain, drum });
                    }
                }
                if self.notes.len() >= MAX_NOTES { self.notes.remove(0); }
                let mut note = q.note;
                note.trim = self.trim_for(&note.name);
                self.notes.push(note);
            }
            self.clock.seconds = self.pos as f64 / self.sr as f64;
            self.clock.bars = self.bar as f64 + self.pos.saturating_sub(self.bar_start) as f64 / self.bar_len.max(1) as f64;

            self.mix.clear();
            for n in &mut self.notes {
                let mut o = n.render(&self.clock);
                let (tl, tr) = n.trim;
                if tl != 1.0 || tr != 1.0 {
                    o.main = Stereo { l: o.main.l * tl, r: o.main.r * tr };
                    o.bus_only = Stereo { l: o.bus_only.l * tl, r: o.bus_only.r * tr };
                }
                // per-voice meter
                let lvl = o.main.l.abs().max(o.main.r.abs());
                if lvl > 1e-4 {
                    let id = name_salt(&n.name);
                    match self.levels.iter_mut().find(|(v, _)| *v == id) {
                        Some((_, m)) => *m = m.max(lvl),
                        None => if self.levels.len() < 64 { self.levels.push((id, lvl)); },
                    }
                }
                let bus = n.route.bus.min(self.mix.buses.len() - 1);
                self.mix.buses[bus].add(o.main);
                self.mix.buses[bus].add(o.bus_only);
                if n.route.reverb > 0.0 { self.mix.reverb.add(o.main.scale(n.route.reverb)); }
                if n.route.delay > 0.0 { self.mix.delay.add(o.main.scale(n.route.delay)); }
            }
            self.notes.retain(|n| !n.done());
            for (_, v) in &mut self.live {
                let o = v.render();
                let bus = v.route.bus.min(self.mix.buses.len() - 1);
                self.mix.buses[bus].add(o);
                if v.route.reverb > 0.0 { self.mix.reverb.add(o.scale(v.route.reverb)); }
                if v.route.delay > 0.0 { self.mix.delay.add(o.scale(v.route.delay)); }
            }
            self.live.retain(|(_, v)| !v.done());
            let s = self.master.process(&self.mix);
            peak = peak.max(s.l.abs()).max(s.r.abs());
            match frame {
                [m] => *m = (s.l + s.r) * 0.5,
                [a, b, rest @ ..] => { *a = s.l; *b = s.r; rest.fill(0.0); }
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
        s.voices.store(self.notes.len() as u32, Ordering::Relaxed);
        s.pending.store(self.pending.is_some(), Ordering::Relaxed);
    }
}

/// A note held on a MIDI keyboard — the browser's `liveNoteOn`: two detuned
/// oscillators through the voice's filter, fast attack, 180 ms release on key up.
struct LiveVoice {
    wave: loom_core::Wave,
    phases: [f32; 2],
    incs: [f32; 2],
    filter: crate::dsp::Biquad,
    level: f32,
    target: f32,
    attack: f32,
    release: f32,
    released: bool,
    route: Route,
}

impl LiveVoice {
    fn new(sr: f32, wave: loom_core::Wave, synth: &loom_core::Synth, note: u8, velocity: u8, route: Route) -> LiveVoice {
        let freq = 440.0 * 2f32.powf((note as f32 - 69.0) / 12.0);
        let cutoff = match synth.cutoff { Some(loom_core::Param::Const(c)) => c, _ => 3200.0 };
        let kind = match synth.filter_type {
            Some(loom_core::FilterType::Highpass) => crate::dsp::BiquadKind::Highpass,
            Some(loom_core::FilterType::Bandpass) => crate::dsp::BiquadKind::Bandpass,
            Some(loom_core::FilterType::Notch) => crate::dsp::BiquadKind::Notch,
            _ => crate::dsp::BiquadKind::Lowpass,
        };
        LiveVoice {
            wave,
            phases: [0.0, 0.0],
            incs: [freq / sr, freq * 2f32.powf(8.0 / 1200.0) / sr],
            filter: crate::dsp::Biquad::new(kind, sr, cutoff, synth.q.unwrap_or(1.0), 0.0),
            level: 0.0001,
            target: velocity as f32 / 127.0 * 0.32,
            // exponential moves: ~8 ms up, ~180 ms down
            attack: (-1.0 / (0.008 / 5.0 * sr)).exp(),
            release: (-1.0 / (0.18 / 5.0 * sr)).exp(),
            released: false,
            route,
        }
    }
    fn release(&mut self) { self.released = true; }
    fn done(&self) -> bool { self.released && self.level < 0.0002 }
    fn render(&mut self) -> crate::note::Stereo {
        let mut s = 0.0;
        for k in 0..2 {
            s += crate::dsp::osc(self.wave, self.phases[k], self.incs[k]);
            self.phases[k] = (self.phases[k] + self.incs[k]) % 1.0;
        }
        let (goal, coeff) = if self.released { (0.0, self.release) } else { (self.target, self.attack) };
        self.level = goal + (self.level - goal) * coeff;
        crate::note::Stereo::mono(self.filter.process(s) * self.level)
    }
}

/// JS `voiceChord`: re-arrange chord notes (MIDI numbers) for a voicing style.
fn voice_chord(mut c: Vec<f32>, kind: &str) -> Vec<f32> {
    if c.len() < 2 { return c; }
    let sorted = |mut v: Vec<f32>| { v.sort_by(|a, b| a.total_cmp(b)); v };
    match kind {
        "open" | "spread" => { for i in (1..c.len()).step_by(2) { c[i] += 12.0; } c }
        "drop2" => { let mut s = sorted(c); let n = s.len(); s[n - 2] -= 12.0; s }
        "drop3" => { let mut s = sorted(c); let n = s.len(); if n >= 3 { s[n - 3] -= 12.0; } s }
        "shell" => {
            let root = c.iter().cloned().fold(f32::MAX, f32::min);
            let keep: Vec<f32> = c.iter().cloned().filter(|x| ((x - root).round() as i32).rem_euclid(12) != 7).collect();
            if keep.is_empty() { c } else { keep }
        }
        "quartal" => {
            let mut s = sorted(c);
            for i in 1..s.len() { while s[i] - s[i - 1] < 5.0 { s[i] += 12.0; } }
            s
        }
        "wide" => { for (i, x) in c.iter_mut().enumerate() { *x += 12.0 * i as f32; } c }
        _ => c,
    }
}

/// JS adaptive just intonation: retune a chord to pure ratios above its lowest note.
fn adaptive_just(freqs: &[f32]) -> Vec<f32> {
    const JR: [f32; 12] = [1.0, 16.0 / 15.0, 9.0 / 8.0, 6.0 / 5.0, 5.0 / 4.0, 4.0 / 3.0, 45.0 / 32.0, 1.5, 8.0 / 5.0, 5.0 / 3.0, 9.0 / 5.0, 15.0 / 8.0];
    let lo = freqs.iter().cloned().fold(f32::MAX, f32::min);
    freqs.iter().map(|f| {
        let iv = (12.0 * (f / lo).log2()).round() as i32;
        lo * JR[iv.rem_euclid(12) as usize] * 2f32.powi(iv.div_euclid(12))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48000.0;

    /// A dry sequencer: no global SPACE/ECHO, so timing tests see clean silences.
    fn seq() -> Sequencer {
        let mut s = Sequencer::new(SR, Arc::new(SampleBank::empty()));
        s.set_space(0.0);
        s.set_echo(0.0);
        s
    }

    fn bars(seq: &mut Sequencer, n: f32) -> Vec<f32> {
        let mut buf = vec![0.0; (n * 2.0 * SR) as usize * 2];
        seq.render(&mut buf, 2);
        buf
    }

    #[test]
    fn silent_until_play() {
        let mut s = seq();
        s.set_program(Program::parse("bpm: 120\nkick: x x x x"));
        assert!(bars(&mut s, 0.5).iter().all(|x| *x == 0.0));
    }

    #[test]
    fn kick_lands_on_the_beat() {
        let mut s = seq();
        s.set_program(Program::parse("bpm: 120\nkick: x . . ."));
        s.play();
        let buf = bars(&mut s, 1.0); // 120 bpm, 4 beats = 2 s
        let loud = |from: f32, to: f32| {
            let (a, b) = ((from * SR) as usize * 2, (to * SR) as usize * 2);
            buf[a..b].iter().fold(0.0f32, |m, x| m.max(x.abs()))
        };
        assert!(loud(0.0, 0.1) > 0.2, "kick at beat 1");
        assert!(loud(0.8, 1.9) < 0.01, "silence on beats 2-4");
    }

    #[test]
    fn edits_wait_for_the_bar_line() {
        let mut s = seq();
        s.set_program(Program::parse("bpm: 120\nkick: x . . ."));
        s.play();
        bars(&mut s, 0.25);
        s.set_program(Program::parse("bpm: 120\nhat: x x x x x x x x"));
        let st = Status::default();
        s.publish(&st);
        assert_eq!(st.bar(), 0);
        assert!(st.pending());
        bars(&mut s, 1.0);
        s.publish(&st);
        assert_eq!(st.bar(), 1, "new code starts on bar 1");
        assert!(!st.pending());
        assert!(st.playing() && st.phase() > 0.2 && st.phase() < 0.3);
    }

    #[test]
    fn song_wraps_loops_and_mute_silences() {
        let mut s = seq();
        s.set_program(Program::parse("bpm: 120\nsong: a*1 b*1\nbass: c2 . . . | saw"));
        s.play();
        bars(&mut s, 2.5);
        let st = Status::default();
        s.publish(&st);
        assert_eq!((st.bar(), st.song_bar()), (2, 0));
        s.set_muted("bass", true);
        s.stop();
        bars(&mut s, 1.0); // let the last note ring out
        s.play();
        assert!(bars(&mut s, 1.0).iter().all(|x| x.abs() < 1e-6));

        let mut s = seq();
        s.set_program(Program::parse("bpm: 120\nsong: a*2 b*2\nloop: b\nkick: x"));
        s.play();
        bars(&mut s, 5.5);
        s.publish(&st);
        assert_eq!(st.song_bar(), 3, "loop: b keeps cycling bars 2-3");
    }

    #[test]
    fn midi_voices_emit_notes_instead_of_sound() {
        let mut s = seq();
        s.set_program(Program::parse("bpm: 120\nkeys: c4 . e4 . | midi 2"));
        s.play();
        let buf = bars(&mut s, 1.0);
        assert!(buf.iter().all(|x| x.abs() < 1e-6));
        let notes: Vec<MidiNote> = s.midi_out.iter().filter_map(|m| match m { MidiEvent::Note(n) => Some(*n), _ => None }).collect();
        assert_eq!(notes.iter().map(|m| (m.channel, m.note)).collect::<Vec<_>>(), vec![(2, 60), (2, 64)]);
        assert!((notes[1].at - 1.0).abs() < 1e-3);
    }

    #[test]
    fn midi_clock_tempo_follow_and_live_keys() {
        let mut s = seq();
        s.set_midi_clock(true);
        s.set_bpm_override(Some(60.0));
        s.set_program(Program::parse("bpm: 120\nlead: c4 | saw"));
        s.play();
        bars(&mut s, 0.1);
        let clocks: Vec<f64> = s.midi_out.iter().filter_map(|m| match m { MidiEvent::Clock { at } => Some(*at), _ => None }).collect();
        assert_eq!(s.midi_out[0], MidiEvent::Start { at: 0.0 });
        assert_eq!(clocks.len(), 96);
        assert!((clocks[24] - 1.0).abs() < 1e-6, "60 bpm from the external clock: a beat lasts 1 s");

        let mut s = seq();
        s.set_program(Program::parse("lead: c4 | saw | lp 900"));
        s.live_note_on(60, 100);
        let held = bars(&mut s, 0.1);
        assert!(held.iter().any(|x| x.abs() > 0.05), "a held key sounds while stopped");
        s.live_note_off(60);
        bars(&mut s, 0.25);
        assert!(bars(&mut s, 0.05).iter().all(|x| x.abs() < 1e-3), "and fades after release");
    }

    #[test]
    fn morph_crossfades_changed_voices_and_metronome_ticks() {
        let level = |s: &mut Sequencer| bars(s, 1.0).iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let mut s = seq();
        s.set_program(Program::parse("bpm: 120\nmorph: 3\nkick: x . . ."));
        s.play();
        bars(&mut s, 0.5);
        s.set_program(Program::parse("bpm: 120\nmorph: 3\nkick: x . . .\nhat: x x x x x x x x"));
        bars(&mut s, 0.6); // cross the bar line
        assert!(s.morph.is_some(), "a morph starts at the bar line");
        assert_eq!(s.morph.as_ref().unwrap().changed, vec!["hat".to_string()]);
        for _ in 0..4 { level(&mut s); }
        assert!(s.morph.is_none(), "and ends after N bars");

        let mut s = seq();
        s.set_metronome(true);
        s.set_program(Program::parse("bpm: 120\nlead: . | saw"));
        s.play();
        let buf = bars(&mut s, 1.0);
        let at = |sec: f32| buf[(sec * SR) as usize * 2..(sec * SR + 400.0) as usize * 2].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(at(0.0) > 0.05 && at(0.5) > 0.03 && at(0.25) < 0.01, "ticks on the beats only");
    }

    #[test]
    fn hits_and_scope_feed_the_visuals() {
        let mut s = seq();
        s.set_program(Program::parse("bpm: 120\nkick: x . x .\nlead: c4 . . . | saw | hue 200"));
        s.play();
        let buf = bars(&mut s, 1.0);
        let st = Status::default();
        st.feed_visuals(&mut s.hits, &s.levels.clone(), &buf[..SCOPE_LEN * 2], 2); // the kick's first 40 ms
        let hits = st.take_hits();
        assert_eq!(hits.len(), 3);
        let lead = hits.iter().find(|h| !h.drum).unwrap();
        assert_eq!((lead.hue, lead.voice), (200.0, name_salt("lead")));
        assert_eq!(hits.iter().find(|h| h.drum).unwrap().hue, voice_hue("kick"));
        assert!(st.take_hits().is_empty(), "hits are handed over once");
        let mut scope = vec![0.0; SCOPE_LEN];
        st.scope(&mut scope);
        assert!(scope.iter().any(|x| *x != 0.0));
    }

    #[test]
    fn mixer_faders_solo_stems_and_swing() {
        let peak = |buf: &[f32]| buf.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let song = "bpm: 120\nkick: x . . .\nlead: . . c4 . | saw";
        let mut s = seq();
        s.set_program(Program::parse(song));
        s.play();
        let full = bars(&mut s, 1.0);
        assert!(s.levels.iter().any(|(v, l)| *v == name_salt("kick") && *l > 0.1), "kick meter moves");

        let mut s = seq();
        s.set_program(Program::parse(song));
        s.set_solo("lead", true);
        s.play();
        let solo = bars(&mut s, 1.0);
        assert!(peak(&solo[..(0.4 * SR) as usize * 2]) < 1e-4, "kick silenced by the lead solo");

        let mut s = seq();
        s.set_program(Program::parse(song));
        s.set_voice_gain("kick", 0.0);
        s.set_stem(Some("kick"));
        s.play();
        assert!(peak(&bars(&mut s, 1.0)) < 1e-4, "stem of a faded-out voice is silent");
        assert!(peak(&full) > 0.1);

        let mut s = seq();
        s.set_swing(0.5);
        s.set_program(Program::parse("bpm: 120\nhat: x x x x"));
        s.play();
        let buf = bars(&mut s, 1.0);
        let at = |sec: f32| peak(&buf[(sec * SR) as usize * 2..((sec + 0.03) * SR) as usize * 2]);
        assert!(at(0.5) < 0.01 && at(0.75) > 0.02, "the 2nd step moves half a step late");
    }

    #[test]
    fn voicings_and_just_tuning() {
        assert_eq!(voice_chord(vec![60.0, 64.0, 67.0], "open"), vec![60.0, 76.0, 67.0]);
        assert_eq!(voice_chord(vec![60.0, 64.0, 67.0], "drop2"), vec![60.0, 52.0, 67.0]);
        assert_eq!(voice_chord(vec![60.0, 64.0, 67.0, 71.0], "shell"), vec![60.0, 64.0, 71.0]);
        let j = adaptive_just(&[261.63, 329.63, 392.0]);
        assert!((j[1] / j[0] - 1.25).abs() < 1e-4 && (j[2] / j[0] - 1.5).abs() < 1e-4);
    }
}
