//! LE PROF & COMPOSITION in the apps: the state behind GÉNÉRER, DÉVELOPPE,
//! EXPLIQUE, `?question`, OREILLE, HUM and LEÇONS, with no drawing.
//!
//! The musical logic itself lives in `loom-app` (`generate`, `develop`,
//! `prof`, `ear`, `hum`, `lessons`); this folder only holds what the two
//! front-ends need around it:
//!
//! - [`history`]: the shared undo / redo for everything that rewrites the editor.
//! - [`route`]: what the VIBE box does with the text (`?question` or VIBE).
//! - [`demo`]: playing a one-bar example through the engine, then giving the song back.
//! - [`ear`]: the ear-training panel (level, exercise, score, verdict).
//! - [`hum`]: the HUM take (countdown, recording, analysis).
//! - [`browse`]: the lessons / reference list with its search.
//!
//! Nothing here draws, so both apps share it: `loom-tui` compiles this
//! folder too (see the `#[path]` line in its `main.rs`).

pub mod browse;
pub mod demo;
pub mod ear;
pub mod history;
pub mod hum;
pub mod route;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// A new seed for the musical dice (GÉNÉRER, DÉVELOPPE, OREILLE): the clock,
/// mixed with a counter so two clicks in the same instant still differ.
pub fn fresh_seed() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    nanos ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

#[cfg(test)]
mod tests {
    #[test]
    fn seeds_differ() {
        let a = super::fresh_seed();
        let b = super::fresh_seed();
        assert_ne!(a, b);
    }
}
