//! The contract between the sequencer and the sound modules.
//!
//! When the sequencer triggers a note it fills a [`NoteSpec`] and builds a
//! [`Note`]: a sound source (synth drum, tone, or sample) followed, for tones,
//! by the per-note insert chain, then routed to a mix bus plus the master
//! reverb / delay sends.
//!
//! Signal flow (mirrors the browser's per-note WebAudio graph):
//!
//! ```text
//! Drum ─────────────┐
//! Sample ───────────┤ (fixed pan)
//! Tone ── Inserts ──┴──► bus[n] ──► Master (buses → reverb/delay → glue → out)
//!             │   └── sends: reverb, delay
//!             └── bus_only (chorus / echo taps: bus, not sends)
//! ```

use crate::clock::Clock;
use crate::drums::Drum;
use crate::inserts::prims::Panner;
use crate::inserts::Inserts;
use crate::sample::{SampleBank, SampleVoice};
use crate::tone::Tone;
use loom_core::{Synth, Wave};

/// A stereo sample.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stereo { pub l: f32, pub r: f32 }

impl Stereo {
    pub const ZERO: Stereo = Stereo { l: 0.0, r: 0.0 };
    pub fn mono(x: f32) -> Stereo { Stereo { l: x, r: x } }
    pub fn scale(self, g: f32) -> Stereo { Stereo { l: self.l * g, r: self.r * g } }
    pub fn add(&mut self, o: Stereo) { self.l += o.l; self.r += o.r; }
}

/// Everything a sound module needs to start a note.
#[derive(Clone, Copy)]
pub struct NoteSpec<'a> {
    /// Voice name (`bass`, `kick`…).
    pub name: &'a str,
    pub synth: &'a Synth,
    pub wave: Wave,
    /// Pitch in Hz (for samples: the target pitch; for drums: unused).
    pub freq: f32,
    /// Gate length in seconds (step length × `| gate`). The release comes after.
    pub dur: f32,
    /// Peak amplitude: voice gain × velocity (× chord scaling). Drums: velocity.
    pub peak: f32,
    /// Fixed stereo position of this layer (`| pan N`, or the jux/off split).
    /// A swept/signal `| pan` lives in `synth.pan` and is applied by the inserts.
    pub pan: Option<f32>,
    /// The previous pitch of this voice, for `| slide` portamento.
    pub prev_freq: Option<f32>,
    pub sr: f32,
    /// Current bar length in seconds (tempo-synced effects: `echo`, `tgate`).
    pub bar_sec: f32,
    /// Per-note random seed (noise, grains, humanisation).
    pub seed: u32,
}

/// Where a voice's sound goes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Route {
    /// Index of the mix bus (0 = main), from `Master::bus_index`.
    pub bus: usize,
    /// `| reverb N` send amount.
    pub reverb: f32,
    /// `| delay N` send amount.
    pub delay: f32,
}

/// What a note produces for one sample.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoteOut {
    /// Main signal: goes to the bus AND feeds the reverb/delay sends.
    pub main: Stereo,
    /// Extra taps that go to the bus only (chorus wet, per-voice echo).
    pub bus_only: Stereo,
}

// A note is created on the audio path; boxing Tone would allocate for every note.
#[allow(clippy::large_enum_variant)]
enum Source {
    Drum(Drum),
    Tone(Tone),
    Sample(SampleVoice),
}

/// One sounding note: a source, its insert chain, and its routing.
pub struct Note {
    source: Source,
    inserts: Option<Inserts>,
    /// Fixed pan for drums and samples (tones pan inside their inserts).
    pan: Option<Panner>,
    pub route: Route,
    /// Voice name, for mute/solo and meters.
    pub name: String,
    /// Colour, loudness and drum-ness for visuals; `None` for the metronome.
    pub visual: Option<(f32, f32, bool)>,
    /// Mixer fader × pan gains for this voice, refreshed by the sequencer.
    pub trim: (f32, f32),
    /// Seconds of silence rendered after the source finished (effect tails).
    tail_elapsed: f32,
}

/// The same pan law as the tone inserts (WebAudio equal-power, lifted so a
/// centred mono sound stays at full level), so `| pan` sounds alike on every source.
fn panner(pan: Option<f32>) -> Option<Panner> {
    pan.filter(|p| *p != 0.0).map(|p| Panner::new(p, true))
}

impl Note {
    pub fn drum(drum: Drum, name: &str, pan: Option<f32>, route: Route) -> Note {
        Note { source: Source::Drum(drum), inserts: None, pan: panner(pan), route, name: name.to_string(), visual: None, trim: (1.0, 1.0), tail_elapsed: 0.0 }
    }

    pub fn sample(voice: SampleVoice, spec: &NoteSpec, route: Route) -> Note {
        Note { source: Source::Sample(voice), inserts: None, pan: panner(spec.pan), route, name: spec.name.to_string(), visual: None, trim: (1.0, 1.0), tail_elapsed: 0.0 }
    }

    /// A synthesized note with its full per-note insert chain.
    pub fn tone(spec: &NoteSpec, bank: &SampleBank, route: Route) -> Note {
        Note {
            source: Source::Tone(Tone::new(spec)),
            inserts: Some(Inserts::new(spec, bank)),
            pan: None, // the inserts own panning for tones
            route,
            name: spec.name.to_string(),
            visual: None,
            trim: (1.0, 1.0),
            tail_elapsed: 0.0,
        }
    }

    pub fn is_kick(&self) -> bool { matches!(&self.source, Source::Drum(d) if d.is_kick()) }

    fn source_done(&self) -> bool {
        match &self.source {
            Source::Drum(d) => d.done(),
            Source::Tone(t) => t.done(),
            Source::Sample(s) => s.done(),
        }
    }

    pub fn done(&self) -> bool {
        self.source_done() && self.tail_elapsed >= self.inserts.as_ref().map_or(0.0, |i| i.tail())
    }

    pub fn render(&mut self, clock: &Clock) -> NoteOut {
        let finished = self.source_done();
        let dry = if finished {
            self.tail_elapsed += 1.0 / clock.sr;
            Stereo::ZERO
        } else {
            match &mut self.source {
                Source::Drum(d) => {
                    let m = Stereo::mono(d.render());
                    self.pan.map_or(m, |p| p.apply(m, true))
                }
                Source::Sample(s) => {
                    let x = s.render(clock);
                    self.pan.map_or(x, |p| p.apply(x, x.l == x.r))
                }
                Source::Tone(t) => t.render(clock),
            }
        };
        match &mut self.inserts {
            Some(ins) => ins.process(dry, clock),
            None => NoteOut { main: dry, bus_only: Stereo::ZERO },
        }
    }
}
