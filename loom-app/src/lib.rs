//! Shared logic for LOOM's native apps. The terminal (`loom`) and window
//! (`loom-gui`) front-ends both build on this, so they look and behave alike.

pub mod develop;
pub mod dictee;
pub mod ear;
pub mod export;
pub mod generate;
pub mod grid;
pub mod hum;
pub mod lessons;
pub mod prof;
pub mod samples;
pub mod library;
pub mod lint;
pub mod share;
pub mod studio;
pub mod teacher;
pub mod theme;
pub mod themes_web;
pub mod vibe;

pub use grid::{grid, Cell, Row};
pub use library::Library;
pub use theme::{Rgb, Theme, ThemeWatcher};
pub use vibe::{VibeAnswer, VibeJob};

/// The pattern a brand-new session opens with.
pub const WELCOME: &str = "\
# LOOM — une voix par ligne.  name: motif | verbe | verbe
bpm: 118
key: c minor

kick:  x . . . x . . . x . . . x . x .
snare: . . . . x . . . . . . . x . . .
hat:   x x x x x x x x | every 4 fast 2
bass:  1 . 1 . 3 . 5 . 1 . 1 . 7 . 5 . | saw | oct 2
lead:  5 . . 3 . . 1 . | tri | every 2 rev
";

#[cfg(test)]
mod tests {
    #[test]
    fn welcome_song_is_valid() {
        let p = loom_core::Program::parse(super::WELCOME);
        let names: Vec<&str> = p.voices.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["kick", "snare", "hat", "bass", "lead"]);
        assert!(p.key.is_some() && (p.bpm - 118.0).abs() < 1e-9);
        let g = super::grid(&p, 0, 16);
        assert!(g[3].notes.iter().flatten().all(|n| n.ends_with('2')), "bass in octave 2: {:?}", g[3].notes);
    }
}
