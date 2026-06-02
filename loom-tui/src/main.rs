//! LOOM terminal client — SKELETON.
//!
//! This is the Phase 4 starting point, not a finished app. The hard part that
//! Claude Code should build out is the native audio backend (cpal): synthesize
//! the same kick/snare/hat/tone voices the browser makes, driven by
//! `loom_core::schedule_bar`. The TUI half (ratatui) renders the green step
//! grid and a text editor pane, mirroring the browser look in the terminal.
//!
//! Suggested architecture:
//!   - main thread: ratatui draw loop + crossterm key events (edit source, RUN/STOP)
//!   - audio thread: cpal output stream; a sample-clock scheduler that, each bar,
//!     calls loom_core::schedule_bar(&voices, bar) and queues voice envelopes.
//!   - share the parsed `Vec<Voice>` via Arc<Mutex<..>> so edits hot-swap live.

use loom_core::{parse, parse_key, schedule_bar_keyed};

fn main() {
    // Smoke test of the shared engine — prove the core works before wiring audio.
    // Uses the music-theory layer: a key + scale degrees (1=tonic) that the
    // scheduler resolves into absolute note names.
    let src = "key: c minor\nkick: x . . . x . . .\nbass: 1 . 3 . 5 . 3 . | saw";
    let voices = parse(src);
    let key = parse_key(src);
    println!("LOOM terminal — parsed {} voices, key: {:?}", voices.len(), key);
    for ev in schedule_bar_keyed(&voices, key.as_ref(), 0) {
        println!("  bar0 @ {:>5.2}  {:<6} {}", ev.phase, ev.voice, ev.token);
    }
    println!("\nTODO (Claude Code): cpal audio thread + ratatui editor/grid UI.");
    // See the module docs above for the recommended structure.
}
