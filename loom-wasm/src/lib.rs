//! Browser/phone entry point. Compile with `wasm-pack build --target web`.
//! The JS audio layer calls `schedule_bar_json(src, bar)` once per bar and
//! plays the returned events with Web Audio — replacing the JS parser in the
//! prototype with this shared Rust engine.

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
    freq: Option<f32>,
}

#[wasm_bindgen]
pub fn schedule_bar_json(src: &str, bar: i32) -> String {
    let voices = loom_core::parse(src);
    let key = loom_core::parse_key(src);
    let events = loom_core::schedule_bar_keyed(&voices, key.as_ref(), bar as i64);
    let js: Vec<JsEvent> = events.into_iter().map(|e| {
        let wave = match e.wave {
            loom_core::Wave::Sine => "sine",
            loom_core::Wave::Square => "square",
            loom_core::Wave::Saw => "sawtooth",
            loom_core::Wave::Triangle => "triangle",
        }.to_string();
        let freq = if e.is_drum { None } else { loom_core::note_to_freq(&e.token) };
        JsEvent { phase: e.phase, voice: e.voice, token: e.token, wave, gain: e.gain, is_drum: e.is_drum, freq }
    }).collect();
    serde_json::to_string(&js).unwrap_or_else(|_| "[]".to_string())
}
