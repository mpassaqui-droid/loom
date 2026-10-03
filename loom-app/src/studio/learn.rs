//! MIDI LEARN: bind a knob of a MIDI controller to a LOOM knob.
//!
//! Turn LEARN on, pick a knob (VOL, SWING, ECHO, SPACE, EQ), then move a
//! controller knob: its CC number is bound to that knob. From then on, moving
//! it sets the LOOM knob (0..127 spread over the knob's range), LEARN on or off.
//!
//! The bindings survive restarts in a small text file,
//! `$XDG_STATE_HOME/loom/midi-learn` (`~/.local/state/loom/midi-learn`):
//!
//! ```text
//! # MIDI LEARN de LOOM : numéro de CC, puis le potard
//! 21 vol
//! 74 space
//! ```

use std::ffi::OsString;
use std::path::PathBuf;

use super::knobs::{Knob, Knobs};

/// Which CC drives which knob.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bindings {
    /// `(cc, knob)`, sorted by CC number. A CC drives one knob; a knob may have several CCs.
    pub pairs: Vec<(u8, Knob)>,
}

impl Bindings {
    /// Bind `cc` to `knob` (replacing what that CC did before).
    pub fn bind(&mut self, cc: u8, knob: Knob) {
        self.pairs.retain(|(c, _)| *c != cc);
        self.pairs.push((cc, knob));
        self.pairs.sort_by_key(|(c, _)| *c);
    }

    pub fn knob_for(&self, cc: u8) -> Option<Knob> {
        self.pairs.iter().find(|(c, _)| *c == cc).map(|(_, k)| *k)
    }

    /// The CCs bound to a knob, e.g. "CC21" (for the knob's tooltip).
    pub fn describe(&self, knob: Knob) -> Option<String> {
        let ccs: Vec<String> = self.pairs.iter().filter(|(_, k)| *k == knob).map(|(c, _)| format!("CC{c}")).collect();
        (!ccs.is_empty()).then(|| ccs.join(", "))
    }

    /// Read the file format; lines that don't make sense are skipped.
    pub fn parse(text: &str) -> Bindings {
        let mut bindings = Bindings::default();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let mut words = line.split_whitespace();
            let cc = words.next().and_then(|w| w.parse::<u8>().ok()).filter(|cc| *cc < 128);
            let knob = words.next().and_then(Knob::from_key);
            if let (Some(cc), Some(knob)) = (cc, knob) {
                bindings.bind(cc, knob);
            }
        }
        bindings
    }

    pub fn to_text(&self) -> String {
        let mut text = String::from("# MIDI LEARN de LOOM : numéro de CC, puis le potard (vol swing echo space eqLow eqMid eqHigh)\n");
        for (cc, knob) in &self.pairs {
            text.push_str(&format!("{cc} {}\n", knob.key()));
        }
        text
    }

    /// Where the bindings are kept.
    pub fn path() -> PathBuf {
        match loom_audio::paths::state_dir() {
            Some(d) => d.join("midi-learn"),
            None => path_from(std::env::var_os("XDG_STATE_HOME"), std::env::var_os("HOME")),
        }
    }

    /// The saved bindings (none if the file doesn't exist yet).
    pub fn load() -> Bindings {
        std::fs::read_to_string(Self::path()).map(|t| Bindings::parse(&t)).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_text())
    }
}

/// `$XDG_STATE_HOME/loom/midi-learn`, else `$HOME/.local/state/loom/midi-learn`.
fn path_from(state_home: Option<OsString>, home: Option<OsString>) -> PathBuf {
    let state = state_home
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(home.unwrap_or_default()).join(".local/state"));
    state.join("loom/midi-learn")
}

/// What happened with the controller moves of one frame.
#[derive(Debug, Default, PartialEq)]
pub struct LearnEvents {
    /// Knobs whose value changed (send them to the engine).
    pub changed: Vec<Knob>,
    /// A new binding was made (tell the user, it's already saved).
    pub bound: Option<(u8, Knob)>,
    /// Saving the bindings failed.
    pub error: Option<String>,
}

/// The LEARN button's state in an app.
#[derive(Default)]
pub struct Learn {
    /// LEARN is on: picking a knob arms it.
    pub on: bool,
    /// The knob waiting for a controller move.
    pub armed: Option<Knob>,
    pub bindings: Bindings,
}

impl Learn {
    /// LEARN with the bindings saved last time.
    pub fn load() -> Learn {
        Learn { on: false, armed: None, bindings: Bindings::load() }
    }

    pub fn toggle(&mut self) {
        self.on = !self.on;
        self.armed = None;
    }

    /// Pick the knob the next controller move binds to (only while LEARN is on).
    pub fn arm(&mut self, knob: Knob) -> bool {
        if self.on {
            self.armed = Some(knob);
        }
        self.on
    }

    /// Handle the `(cc, value)` moves read from the engine: bind the armed
    /// knob, or move the bound knobs. The bindings file is saved on a new binding.
    pub fn handle(&mut self, controls: &[(u8, u8)], knobs: &mut Knobs) -> LearnEvents {
        let mut events = self.handle_without_saving(controls, knobs);
        if events.bound.is_some() {
            events.error = self.bindings.save().err().map(|e| format!("MIDI LEARN non enregistré : {e}"));
        }
        events
    }

    fn handle_without_saving(&mut self, controls: &[(u8, u8)], knobs: &mut Knobs) -> LearnEvents {
        let mut events = LearnEvents::default();
        for &(cc, value) in controls {
            if let Some(knob) = self.armed.take() {
                self.bindings.bind(cc, knob);
                events.bound = Some((cc, knob));
                continue; // like the browser: the binding move doesn't also turn the knob
            }
            if let Some(knob) = self.bindings.knob_for(cc) {
                knobs.set(knob, knob.from_cc(value));
                if !events.changed.contains(&knob) {
                    events.changed.push(knob);
                }
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_format_round_trips_and_skips_nonsense() {
        let mut b = Bindings::default();
        b.bind(74, Knob::Space);
        b.bind(21, Knob::Volume);
        b.bind(74, Knob::EqHigh); // rebinding a CC replaces it
        let text = b.to_text();
        assert!(text.ends_with("21 vol\n74 eqHigh\n"), "{text}");
        assert_eq!(Bindings::parse(&text), b);
        let messy = Bindings::parse("# notes\n\n 7 swing \n300 vol\nabc echo\n9 bpm\n10\n");
        assert_eq!(messy.pairs, vec![(7, Knob::Swing)]);
    }

    #[test]
    fn path_follows_xdg_state_home() {
        assert_eq!(path_from(Some("/s".into()), Some("/home/a".into())), PathBuf::from("/s/loom/midi-learn"));
        assert_eq!(path_from(None, Some("/home/a".into())), PathBuf::from("/home/a/.local/state/loom/midi-learn"));
    }

    #[test]
    fn learn_then_control() {
        let mut learn = Learn::default();
        let mut knobs = Knobs::default();
        assert!(!learn.arm(Knob::Echo), "arming needs LEARN on");
        learn.toggle();
        assert!(learn.arm(Knob::Echo));
        let events = learn.handle_without_saving(&[(20, 64), (20, 127)], &mut knobs);
        assert_eq!(events.bound, Some((20, Knob::Echo)));
        assert_eq!(events.changed, vec![Knob::Echo]);
        assert_eq!(knobs.echo, 100.0);
        assert_eq!(learn.bindings.describe(Knob::Echo).as_deref(), Some("CC20"));
        // an unbound CC does nothing
        assert_eq!(learn.handle_without_saving(&[(5, 0)], &mut knobs), LearnEvents::default());
    }
}
