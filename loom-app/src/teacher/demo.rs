//! Playing a teacher's example (an ear-training question, `?quinte`…)
//! through the engine without touching the editor.
//!
//! An example is a one-bar LOOM program ([`Demo`]) that lasts `seconds`.
//! The engine plays one program at a time, so the choice here is the
//! simplest safe one: **stop** the song, **play** the example from its
//! first beat, **stop** once its bar is over, then **load** the song's code
//! back into the engine, stopped. The song is not resumed by itself (the
//! engine can only restart it from bar 1, which would be a surprise in the
//! middle of an exercise): Ctrl+Entrée / Ctrl+R plays it again. The editor
//! text, the grid and the undo history never change.

use std::time::{Duration, Instant};

use crate::ear::Demo;
use loom_audio::Engine;

/// Stop this much before the bar ends, so the example's second loop never
/// starts (notes already sounding ring out after a stop).
pub const MARGIN: f64 = 0.04;

/// An example playing instead of the song.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoPlay {
    ends: Instant,
    /// The song was playing before the first example (to say how to get it back).
    pub song_was_playing: bool,
}

impl DemoPlay {
    /// How long to let a `seconds`-long example play before stopping.
    pub fn duration(seconds: f64) -> Duration {
        Duration::from_secs_f64((seconds - MARGIN).max(seconds * 0.5).max(0.0))
    }

    /// The timer alone (no sound), for an example started at `now`.
    pub fn timer(seconds: f64, now: Instant, song_was_playing: bool) -> DemoPlay {
        DemoPlay { ends: now + Self::duration(seconds), song_was_playing }
    }

    /// Stop the song and play `demo` from its first beat. `previous` is the
    /// example still playing, if any (the song's state is taken from it).
    pub fn start(engine: &Engine, demo: &Demo, now: Instant, previous: Option<&DemoPlay>) -> DemoPlay {
        let song_was_playing = previous.map_or(engine.status().playing(), |p| p.song_was_playing);
        engine.stop();
        engine.run(&demo.code);
        Self::timer(demo.seconds, now, song_was_playing)
    }

    /// Time to stop the example?
    pub fn is_over(&self, now: Instant) -> bool {
        now >= self.ends
    }

    /// Stop the example and put the song's code back in the engine, stopped.
    pub fn finish(engine: &Engine, song_code: &str) {
        engine.stop();
        engine.load(song_code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_just_before_the_bar_loops() {
        assert_eq!(DemoPlay::duration(1.6), Duration::from_secs_f64(1.56));
        // a very short example still plays at least half of it
        assert_eq!(DemoPlay::duration(0.05), Duration::from_secs_f64(0.025));
        assert_eq!(DemoPlay::duration(0.0), Duration::ZERO);
        let now = Instant::now();
        let play = DemoPlay::timer(1.68, now, true);
        assert!(play.song_was_playing);
        assert!(!play.is_over(now));
        assert!(!play.is_over(now + Duration::from_millis(1600)));
        assert!(play.is_over(now + Duration::from_millis(1640)));
    }

    #[test]
    fn every_ear_example_fits_one_bar() {
        use crate::ear::{Exercise, Level};
        for seed in 0..40 {
            let demo = Exercise::new(Level::Tout, seed).demo();
            let program = loom_core::Program::parse(&demo.code);
            let bar = program.bar_seconds(0);
            assert!((bar - demo.seconds).abs() < 1e-3, "seed {seed}: bar {bar} s, demo {} s", demo.seconds);
            assert!(DemoPlay::duration(demo.seconds).as_secs_f64() < bar);
        }
    }
}
