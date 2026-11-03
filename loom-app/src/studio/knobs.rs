//! The master knobs, with the browser's ranges, steps and defaults.
//!
//! | knob      | range      | default | sent to the engine as        |
//! |-----------|------------|---------|------------------------------|
//! | VOL       | 0..150     | 80      | `set_volume(v / 100)`        |
//! | SWING     | 0..60      | 0       | `set_swing(v / 100)`         |
//! | ECHO      | 0..100     | 0       | `set_echo(v)`                |
//! | SPACE     | 0..100     | 0       | `set_space(v)`               |
//! | EQ ×3     | -12..12 dB | 0, 0, 2 | `set_eq(low, mid, high)`     |
//!
//! (The browser's VOL stops at 100; the native engine allows up to 150 %.)

use loom_audio::Engine;

/// One master knob.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Knob {
    Volume,
    Swing,
    Echo,
    Space,
    EqLow,
    EqMid,
    EqHigh,
}

impl Knob {
    /// Every knob, in the browser's order.
    pub const ALL: [Knob; 7] = [Knob::Swing, Knob::Echo, Knob::Space, Knob::Volume, Knob::EqLow, Knob::EqMid, Knob::EqHigh];

    /// The label shown next to the knob.
    pub fn label(self) -> &'static str {
        match self {
            Knob::Volume => "VOL",
            Knob::Swing => "SWING",
            Knob::Echo => "ECHO",
            Knob::Space => "SPACE",
            Knob::EqLow => "EQ GRAVE",
            Knob::EqMid => "EQ MÉDIUM",
            Knob::EqHigh => "EQ AIGU",
        }
    }

    /// The knob's name in files (the browser's element ids).
    pub fn key(self) -> &'static str {
        match self {
            Knob::Volume => "vol",
            Knob::Swing => "swing",
            Knob::Echo => "echo",
            Knob::Space => "space",
            Knob::EqLow => "eqLow",
            Knob::EqMid => "eqMid",
            Knob::EqHigh => "eqHigh",
        }
    }

    /// The knob named `key` (see [`Knob::key`]).
    pub fn from_key(key: &str) -> Option<Knob> {
        Knob::ALL.into_iter().find(|k| k.key().eq_ignore_ascii_case(key.trim()))
    }

    /// `(min, max, step)`.
    pub fn range(self) -> (f32, f32, f32) {
        match self {
            Knob::Volume => (0.0, 150.0, 1.0),
            Knob::Swing => (0.0, 60.0, 1.0),
            Knob::Echo | Knob::Space => (0.0, 100.0, 1.0),
            Knob::EqLow | Knob::EqMid | Knob::EqHigh => (-12.0, 12.0, 0.5),
        }
    }

    pub fn default_value(self) -> f32 {
        match self {
            Knob::Volume => 80.0,
            Knob::Swing => 0.0,
            Knob::Echo => 0.0,
            Knob::Space => 0.0,
            Knob::EqLow | Knob::EqMid => 0.0,
            Knob::EqHigh => 2.0,
        }
    }

    /// A value brought inside the range and onto a step.
    pub fn snap(self, value: f32) -> f32 {
        let (min, max, step) = self.range();
        let snapped = min + ((value - min) / step).round() * step;
        snapped.clamp(min, max)
    }

    /// The value shown next to the knob, like the browser: `15`, `-1.5`, `2`.
    pub fn format(self, value: f32) -> String {
        if value.fract() == 0.0 { format!("{value:.0}") } else { format!("{value:.1}") }
    }

    /// A MIDI controller value (0..127) turned into this knob's range.
    pub fn from_cc(self, value: u8) -> f32 {
        let (min, max, _) = self.range();
        self.snap(min + value.min(127) as f32 / 127.0 * (max - min))
    }
}

/// The values of all the master knobs.
#[derive(Clone, Debug, PartialEq)]
pub struct Knobs {
    pub volume: f32,
    pub swing: f32,
    pub echo: f32,
    pub space: f32,
    pub eq_low: f32,
    pub eq_mid: f32,
    pub eq_high: f32,
}

impl Default for Knobs {
    fn default() -> Knobs {
        Knobs {
            volume: Knob::Volume.default_value(),
            swing: Knob::Swing.default_value(),
            echo: Knob::Echo.default_value(),
            space: Knob::Space.default_value(),
            eq_low: Knob::EqLow.default_value(),
            eq_mid: Knob::EqMid.default_value(),
            eq_high: Knob::EqHigh.default_value(),
        }
    }
}

impl Knobs {
    pub fn get(&self, knob: Knob) -> f32 {
        match knob {
            Knob::Volume => self.volume,
            Knob::Swing => self.swing,
            Knob::Echo => self.echo,
            Knob::Space => self.space,
            Knob::EqLow => self.eq_low,
            Knob::EqMid => self.eq_mid,
            Knob::EqHigh => self.eq_high,
        }
    }

    /// The knob's storage, to set it or to hand to a slider.
    pub fn value_mut(&mut self, knob: Knob) -> &mut f32 {
        match knob {
            Knob::Volume => &mut self.volume,
            Knob::Swing => &mut self.swing,
            Knob::Echo => &mut self.echo,
            Knob::Space => &mut self.space,
            Knob::EqLow => &mut self.eq_low,
            Knob::EqMid => &mut self.eq_mid,
            Knob::EqHigh => &mut self.eq_high,
        }
    }

    /// Set a knob (kept in range, on a step).
    pub fn set(&mut self, knob: Knob, value: f32) {
        *self.value_mut(knob) = knob.snap(value);
    }

    /// Move a knob by a number of steps (+1 / -1 from the keyboard).
    #[allow(dead_code)] // used by the terminal app
    pub fn nudge(&mut self, knob: Knob, steps: f32) {
        let (_, _, step) = knob.range();
        self.set(knob, self.get(knob) + steps * step);
    }

    /// "VOL 80" — label and value.
    pub fn text(&self, knob: Knob) -> String {
        format!("{} {}", knob.label(), knob.format(self.get(knob)))
    }

    /// Send one knob to the engine.
    pub fn apply(&self, knob: Knob, engine: &Engine) {
        match knob {
            Knob::Volume => engine.set_volume(self.volume / 100.0),
            Knob::Swing => engine.set_swing(self.swing / 100.0),
            Knob::Echo => engine.set_echo(self.echo),
            Knob::Space => engine.set_space(self.space),
            Knob::EqLow | Knob::EqMid | Knob::EqHigh => engine.set_eq(self.eq_low, self.eq_mid, self.eq_high),
        }
    }

    /// Send every knob to the engine (at start-up, after an import).
    pub fn apply_all(&self, engine: &Engine) {
        for knob in [Knob::Volume, Knob::Swing, Knob::Echo, Knob::Space, Knob::EqLow] {
            self.apply(knob, engine);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_ranges_match_the_browser() {
        let k = Knobs::default();
        assert_eq!((k.swing, k.echo, k.space, k.eq_low, k.eq_mid, k.eq_high), (0.0, 0.0, 0.0, 0.0, 0.0, 2.0));
        assert_eq!(Knob::Swing.range(), (0.0, 60.0, 1.0));
        assert_eq!(Knob::EqMid.range(), (-12.0, 12.0, 0.5));
    }

    #[test]
    fn values_snap_to_steps_and_stay_in_range() {
        let mut k = Knobs::default();
        k.set(Knob::EqLow, -1.3);
        assert_eq!(k.eq_low, -1.5);
        k.set(Knob::Swing, 99.0);
        assert_eq!(k.swing, 60.0);
        k.nudge(Knob::Echo, 1.0);
        assert_eq!(k.echo, 1.0);
        assert_eq!(Knob::EqLow.format(-1.5), "-1.5");
        assert_eq!(k.text(Knob::Space), "SPACE 0");
    }

    #[test]
    fn midi_values_cover_the_whole_range() {
        assert_eq!(Knob::Space.from_cc(0), 0.0);
        assert_eq!(Knob::Space.from_cc(127), 100.0);
        assert_eq!(Knob::EqHigh.from_cc(0), -12.0);
        assert_eq!(Knob::EqHigh.from_cc(127), 12.0);
        assert_eq!(Knob::Swing.from_cc(64), 30.0);
    }

    #[test]
    fn keys_round_trip() {
        for knob in Knob::ALL {
            assert_eq!(Knob::from_key(knob.key()), Some(knob));
        }
        assert_eq!(Knob::from_key("bpm"), None);
    }
}
