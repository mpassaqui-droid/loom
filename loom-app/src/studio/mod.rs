//! STUDIO: the browser's studio tools, as plain logic with no drawing.
//!
//! - [`knobs`]: the master knobs VOL, SWING, ECHO, SPACE and the 3-band EQ.
//! - [`mixer`]: one channel strip per voice (fader, pan, solo, meter).
//! - [`stems`]: every voice exported to its own WAV, on a background thread.
//! - [`tap`]: TAP tempo.
//! - [`loudness`]: the LUFS~ loudness meter.
//! - [`learn`]: MIDI LEARN (a hardware knob drives a LOOM knob), saved to disk.
//! - [`share`]: `LOOM1:` share codes with the knob values.
//! - [`songline`]: rewriting the `song:` and `bpm:` lines (TIMELINE and TAP).
//! - [`roll`]: the piano roll's notes.
//! - [`demo`]: the automatic tour of the example songs.
//! - [`clips`]: live variations of one voice (CLIPS).
//!
//! Nothing here draws, so both apps share it: `loom-tui` compiles this
//! folder too (see the `#[path]` line in its `main.rs`).

pub mod clips;
pub mod demo;
pub mod knobs;
pub mod learn;
pub mod loudness;
pub mod mixer;
pub mod roll;
pub mod share;
pub mod songline;
pub mod stems;
pub mod tap;
