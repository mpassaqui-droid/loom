//! MIX: one channel strip per voice — fader, pan, solo and a live meter.
//!
//! Mute stays where it already was in the apps (their `muted` list, also used
//! by the grid), so the mixer and the grid always agree.
//!
//! The settings live here while the song is open: voices that disappear from
//! the code keep theirs (in case they come back), and everything is sent to
//! the engine again after each RUN. Opening another song resets the mixer.

use loom_audio::clock::name_salt;
use loom_audio::Engine;
use loom_core::Program;

/// How fast a meter falls: ×0.85 per 60th of a second (the browser's per-frame falloff).
const FALLOFF_PER_FRAME: f32 = 0.85;

#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    pub name: String,
    /// Fader, 0..2 (1 = as written in the code).
    pub gain: f32,
    /// Pan, -1 (left) .. 1 (right).
    pub pan: f32,
    pub solo: bool,
    /// The meter, 0.. (above 1 = very loud), falling off smoothly.
    pub meter: f32,
    /// Is the voice in the current code? (Strips of removed voices are kept, hidden.)
    pub present: bool,
}

impl Strip {
    fn new(name: &str) -> Strip {
        Strip { name: name.to_string(), gain: 1.0, pan: 0.0, solo: false, meter: 0.0, present: true }
    }

    /// How full to draw this strip's meter (see [`meter_fraction`]).
    pub fn meter_fraction(&self) -> f32 {
        meter_fraction(self.meter)
    }

    /// "80 %" for the fader.
    pub fn gain_text(&self) -> String {
        format!("{:.0} %", self.gain * 100.0)
    }

    /// "C", "G 40", "D 100" for the pan (gauche / droite).
    pub fn pan_text(&self) -> String {
        let amount = (self.pan.abs() * 100.0).round();
        if amount == 0.0 { "C".into() } else if self.pan < 0.0 { format!("G {amount:.0}") } else { format!("D {amount:.0}") }
    }
}

/// How full to draw a meter for a level (0..1 = full scale), on a decibel
/// scale from -48 dB to +6 dB: quiet voices (a hi-hat) still move it, like on
/// a mixing desk.
pub fn meter_fraction(level: f32) -> f32 {
    if level <= 0.0 {
        return 0.0;
    }
    ((20.0 * level.log10() + 48.0) / 54.0).clamp(0.0, 1.0)
}

/// A mixer parameter, for keyboard control.
#[allow(dead_code)] // used by the terminal app
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Param {
    Gain,
    Pan,
}

#[derive(Default)]
pub struct Mixer {
    /// The current voices first (in the code's order), then the hidden ones.
    pub strips: Vec<Strip>,
}

impl Mixer {
    /// Match the strips to the program's voices, keeping known settings.
    pub fn sync(&mut self, program: &Program) {
        let mut names: Vec<&str> = Vec::new();
        for v in &program.voices {
            if !names.contains(&v.name.as_str()) {
                names.push(&v.name);
            }
        }
        let mut old = std::mem::take(&mut self.strips);
        for name in &names {
            let strip = match old.iter().position(|s| s.name == *name) {
                Some(i) => Strip { present: true, ..old.remove(i) },
                None => Strip::new(name),
            };
            self.strips.push(strip);
        }
        self.strips.extend(old.into_iter().map(|s| Strip { present: false, meter: 0.0, ..s }));
    }

    /// The strips of the voices in the code.
    pub fn visible(&self) -> &[Strip] {
        let count = self.strips.iter().take_while(|s| s.present).count();
        &self.strips[..count]
    }

    /// Send every setting to the engine (after RUN). Untouched strips are sent
    /// too, so an engine that kept an old setting for that name is corrected.
    pub fn apply(&self, engine: &Engine) {
        for s in &self.strips {
            engine.set_voice_gain(&s.name, s.gain);
            engine.set_voice_pan(&s.name, s.pan);
            engine.set_solo(&s.name, s.solo);
        }
    }

    /// Back to neutral (another song was opened), in the engine too.
    pub fn reset(&mut self, engine: Option<&Engine>) {
        for s in &mut self.strips {
            *s = Strip { present: s.present, ..Strip::new(&s.name) };
        }
        if let Some(engine) = engine {
            self.apply(engine);
        }
        self.strips.retain(|s| s.present);
    }

    pub fn set_gain(&mut self, index: usize, gain: f32, engine: Option<&Engine>) {
        let Some(s) = self.strips.get_mut(index) else { return };
        s.gain = gain.clamp(0.0, 2.0);
        if let Some(e) = engine {
            e.set_voice_gain(&s.name, s.gain);
        }
    }

    pub fn set_pan(&mut self, index: usize, pan: f32, engine: Option<&Engine>) {
        let Some(s) = self.strips.get_mut(index) else { return };
        s.pan = pan.clamp(-1.0, 1.0);
        if let Some(e) = engine {
            e.set_voice_pan(&s.name, s.pan);
        }
    }

    pub fn toggle_solo(&mut self, index: usize, engine: Option<&Engine>) {
        let Some(s) = self.strips.get_mut(index) else { return };
        s.solo = !s.solo;
        if let Some(e) = engine {
            e.set_solo(&s.name, s.solo);
        }
    }

    /// Keyboard: move a parameter by `steps` (fader 5 %, pan 0.1 per step).
    #[allow(dead_code)] // used by the terminal app
    pub fn nudge(&mut self, index: usize, param: Param, steps: f32, engine: Option<&Engine>) {
        let Some(s) = self.strips.get(index) else { return };
        match param {
            Param::Gain => {
                let g = ((s.gain + steps * 0.05) * 100.0).round() / 100.0;
                self.set_gain(index, g, engine);
            }
            Param::Pan => {
                let p = ((s.pan + steps * 0.1) * 10.0).round() / 10.0;
                self.set_pan(index, p, engine);
            }
        }
    }

    /// Is any voice soloed? (Then the others are silent.)
    pub fn any_solo(&self) -> bool {
        self.visible().iter().any(|s| s.solo)
    }

    /// Feed the meters with the engine's `voice_levels`, `dt` seconds after the
    /// last call. A meter jumps up to a new peak and falls off slowly.
    pub fn update_meters(&mut self, levels: &[(u32, f32)], dt: f32) {
        let fall = FALLOFF_PER_FRAME.powf((dt * 60.0).clamp(0.0, 60.0));
        for s in &mut self.strips {
            let salt = name_salt(&s.name);
            let level = levels.iter().filter(|(id, _)| *id == salt).map(|(_, l)| *l).fold(0.0, f32::max);
            s.meter = (s.meter * fall).max(level);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_follow_the_code_and_keep_their_settings() {
        let mut mixer = Mixer::default();
        mixer.sync(&Program::parse("kick: x . x .\nbass: c2 .\nbass@drop: c3"));
        assert_eq!(mixer.visible().iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["kick", "bass"]);
        mixer.set_gain(1, 1.4, None);
        mixer.toggle_solo(1, None);
        // bass removed, lead added: bass is kept hidden
        mixer.sync(&Program::parse("lead: c4\nkick: x"));
        assert_eq!(mixer.visible().iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["lead", "kick"]);
        assert!(!mixer.any_solo());
        // bass back: its fader is still at 140 %
        mixer.sync(&Program::parse("bass: c2\nkick: x"));
        assert_eq!((mixer.strips[0].name.as_str(), mixer.strips[0].gain, mixer.strips[0].solo), ("bass", 1.4, true));
        mixer.reset(None);
        assert_eq!(mixer.strips.len(), 2);
        assert!(mixer.strips.iter().all(|s| s.gain == 1.0 && !s.solo));
    }

    #[test]
    fn keyboard_nudges_stay_in_range() {
        let mut mixer = Mixer::default();
        mixer.sync(&Program::parse("hat: x x"));
        for _ in 0..30 {
            mixer.nudge(0, Param::Gain, 1.0, None);
            mixer.nudge(0, Param::Pan, -1.0, None);
        }
        assert_eq!((mixer.strips[0].gain, mixer.strips[0].pan), (2.0, -1.0));
        assert_eq!((mixer.strips[0].gain_text(), mixer.strips[0].pan_text()), ("200 %".to_string(), "G 100".to_string()));
        mixer.nudge(0, Param::Pan, 13.0, None);
        assert_eq!(mixer.strips[0].pan_text(), "D 30");
    }

    #[test]
    fn meters_jump_up_and_fall_off() {
        let mut mixer = Mixer::default();
        mixer.sync(&Program::parse("kick: x\nlead: c4"));
        mixer.update_meters(&[(name_salt("kick"), 0.8)], 1.0 / 60.0);
        assert_eq!((mixer.strips[0].meter, mixer.strips[1].meter), (0.8, 0.0));
        mixer.update_meters(&[], 1.0 / 60.0);
        assert!((mixer.strips[0].meter - 0.68).abs() < 1e-4);
        mixer.update_meters(&[], 1.0);
        assert!(mixer.strips[0].meter < 0.01);
        // the drawn meter is in decibels: 0 dB fills 48/54, -24 dB half of that
        let at = |meter: f32| Strip { meter, ..Strip::new("x") }.meter_fraction();
        assert!((at(1.0) - 48.0 / 54.0).abs() < 1e-4);
        assert!((at(0.063) - 24.0 / 54.0).abs() < 0.01);
        assert_eq!((at(0.0), at(0.001)), (0.0, 0.0));
    }
}
