//! The teacher's offline knowledge base and analyser — the "why it sounds the
//! way it does" report shown before (and without) the AI brain.
//!
//! Moved here from `loom-app` as the keystone of the unified engine: the same
//! std-only code now runs in the browser (via `loom-wasm`) and in the native
//! apps (re-exported through `loom-app::prof`). Everything is a faithful port
//! of the browser's `analyzePiece` + `noveltyReport` + `chordRoughness`, so all
//! UIs give exactly the same knowledge-base answer.
//!
//! - [`lore`] — the static knowledge base (`LORE`, `SCALE_LORE`, `TRADITION_LORE`).
//! - [`analyze`] — instant offline analysis (tonality, microtuning, Plomp–Levelt
//!   acoustics, roman-numeral harmony + cadence, form, novelty score).
//! - [`novelty_report`] — how much the music actually changes bar to bar.
//!
//! No AI — pure parsing + measurement — so it works without internet.

pub mod analyze;
pub mod lore;

pub use analyze::{analyze, novelty_report};
pub use lore::{LORE, MOOD, SCALE_LORE, TRADITION_LORE};
