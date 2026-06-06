//! Browser/phone entry point. Compile with `wasm-pack build --target web`.
//! The JS audio layer calls `schedule_bar_json(src, bar)` once per bar and
//! plays the returned events with Web Audio — replacing the JS parser in the
//! prototype with this shared Rust engine. Tempo (bpm/meter/accel) comes from
//! `program_info_json` / `bar_seconds`, so both engines share one timeline.

use wasm_bindgen::prelude::*;
use serde::Serialize;

#[derive(Serialize)]
struct JsEvent {
    phase: f32,
    voice: String,
    token: String,
    wave: String,
    gain: f32,
    is_drum: bool,
    /// Velocity multiplier (accents `!`, crescendo/decrescendo rolls).
    vel: f32,
    /// Exact frequency in Hz for pitched notes (None for drums / unresolved).
    freq: Option<f32>,
}

#[derive(Serialize)]
struct ProgramInfo {
    bpm: f64,
    meter_beats: f64,
    total_bars: u32,
    has_accel: bool,
    has_groove: bool,
    /// Section names in arrangement order (empty if no `song:`).
    sections: Vec<String>,
}

fn wave_name(w: loom_core::Wave) -> &'static str {
    match w {
        loom_core::Wave::Sine => "sine",
        loom_core::Wave::Square => "square",
        loom_core::Wave::Saw => "sawtooth",
        loom_core::Wave::Triangle => "triangle",
    }
}

/// Schedule one bar and return its events as JSON for the JS audio layer.
#[wasm_bindgen]
pub fn schedule_bar_json(src: &str, bar: i32) -> String {
    let prog = loom_core::Program::parse(src);
    let events = prog.schedule_bar(bar as i64);
    let js: Vec<JsEvent> = events.into_iter().map(|e| {
        // e.freq carries exact (microtonal) pitch; fall back to the note name only
        // if the core didn't resolve one (older paths / safety).
        let freq = e.freq.or_else(|| if e.is_drum { None } else { loom_core::note_to_freq(&e.token) });
        JsEvent {
            phase: e.phase, voice: e.voice, wave: wave_name(e.wave).to_string(),
            gain: e.gain, is_drum: e.is_drum, vel: e.vel, freq, token: e.token,
        }
    }).collect();
    serde_json::to_string(&js).unwrap_or_else(|_| "[]".to_string())
}

/// Duration of a given bar in seconds, honouring `meter:` and `accel:`/`tempo:`.
/// The JS transport multiplies each event's `phase` (0..1) by this.
#[wasm_bindgen]
pub fn bar_seconds(src: &str, bar: i32) -> f64 {
    loom_core::Program::parse(src).bar_seconds(bar as i64)
}

/// Tempo in BPM at a given bar (after any `accel:` ramp / per-section `tempo:`).
#[wasm_bindgen]
pub fn bar_bpm(src: &str, bar: i32) -> f64 {
    loom_core::Program::parse(src).bar_bpm(bar as i64)
}

/// Top-level program info (tempo, meter, arrangement length) as JSON.
#[wasm_bindgen]
pub fn program_info_json(src: &str) -> String {
    let prog = loom_core::Program::parse(src);
    let sections: Vec<String> = prog.song.iter().map(|(n, _)| n.clone()).collect();
    let info = ProgramInfo {
        bpm: prog.bpm,
        meter_beats: prog.meter_beats,
        total_bars: prog.len_bars(),
        has_accel: prog.accel.is_some(),
        has_groove: !prog.groove.is_empty(),
        sections,
    };
    serde_json::to_string(&info).unwrap_or_else(|_| "{}".to_string())
}
