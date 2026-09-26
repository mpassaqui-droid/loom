//! Browser/phone entry point. Compile with `wasm-pack build --target web`.
//! The JS audio layer calls `schedule_bar_json(src, bar)` once per bar and
//! plays the returned events with Web Audio — replacing the JS parser in the
//! prototype with this shared Rust engine. Tempo (bpm/meter/accel) comes from
//! `program_info_json` / `bar_seconds`, so both engines share one timeline.
//! The offline prof analysis (`analyze_json`) comes from the same std-only
//! `loom-core::prof` code the native apps use — one teacher everywhere.

use wasm_bindgen::prelude::*;
use serde::Serialize;

/// The v1 sound-design subset the unified renderer understands (see
/// `loom-core::render`). Serialized on every event so the JS DSP mirror consumes
/// **exactly the same** resolved synth params as the Rust renderer — the mirror
/// never interprets `|` verbs itself, it only reads this resolved form.
#[derive(Serialize)]
struct SynthV1 {
    /// Filter cutoff `(a, b)` Hz; `a == b` = static, `a != b` = linear sweep
    /// across the note. LFO signals are flattened to their base level (v1).
    cutoff: Option<CutoffV1>,
    /// Filter type: 0 lowpass, 1 highpass, 2 bandpass, 3 notch (None = none).
    ft: Option<u8>,
    /// Resonance 0.1..20 (resolved default 1).
    q: f32,
    /// ADSR `[a, d, s, r]` seconds/level (None = default env).
    env: Option<[f32; 4]>,
    /// Note-gate multiplier.
    gate: f32,
    /// Unison voice count (0 = none).
    unison: u32,
    /// Detune in cents between unison voices.
    detune: f32,
    /// `| sub N` — sine one octave below, 0..1.
    sub: f32,
    /// `| double N` — octave-up layer, 0..1.
    double: f32,
    /// `| echo DIV FB` — per-note tempo feedback delay `[div, fb]` (None = none).
    echo: Option<[f32; 2]>,
    /// `| flange RATE DEPTH` — swept delay `[rate, depth]` (None = none).
    flange: Option<[f32; 2]>,
    /// `| phaser N` — level 0..1 of the 4-allpass phaser (None = none).
    phaser: Option<f32>,
    /// `| reverb N` — level 0..1 of the send to the master reverb (None = none).
    reverb: Option<f32>,
    /// `| delay N` — level 0..1 of the send to the master ping-pong (None = none).
    delay: Option<f32>,
}

#[derive(Serialize)]
struct CutoffV1 {
    a: f32,
    b: f32,
}

fn synth_v1(s: &loom_core::Synth) -> SynthV1 {
    use loom_core::{FilterType, Param};
    let cutoff = match s.cutoff {
        Some(Param::Const(x)) => Some(CutoffV1 { a: x, b: x }),
        Some(Param::Sweep { a, b }) => Some(CutoffV1 { a, b }),
        Some(Param::Signal { a, .. }) => Some(CutoffV1 { a, b: a }), // v1: LFO -> base
        None if s.filter_type.is_some() => Some(CutoffV1 { a: 800.0, b: 800.0 }),
        None => None,
    };
    let ft = s.filter_type.map(|f| match f {
        FilterType::Lowpass => 0u8,
        FilterType::Highpass => 1,
        FilterType::Bandpass => 2,
        FilterType::Notch => 3,
    });
    SynthV1 {
        cutoff,
        ft,
        q: s.q.unwrap_or(1.0),
        env: s.env.map(|e| [e.a, e.d, e.s, e.r]),
        gate: s.gate.unwrap_or(1.0),
        unison: s.unison.unwrap_or(0),
        detune: s.detune.unwrap_or(0.0),
        sub: s.sub.unwrap_or(0.0),
        double: s.double.unwrap_or(0.0),
        echo: s.echo.map(|e| [e.div, e.fb]),
        flange: s.flange.map(|f| [f.rate, f.depth]),
        phaser: s.phaser,
        reverb: s.reverb,
        delay: s.delay,
    }
}

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
    /// Stereo pan (-1..1) for this event's layer (jux/off split, `| pan N`).
    pan: Option<f32>,
    /// Step length (fraction of the bar) this event occupies.
    dur: f32,
    /// v1 renderer sound design (see [`SynthV1`]).
    synth: SynthV1,
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
            gain: e.gain, is_drum: e.is_drum, vel: e.vel, freq, pan: e.pan, token: e.token,
            dur: e.dur, synth: synth_v1(&e.synth),
        }
    }).collect();
    serde_json::to_string(&js).unwrap_or_else(|_| "[]".to_string())
}

/// Render `bars` bars of `src` into mono f32 PCM samples via the unified
/// std-only renderer (`loom-core::render`, P6-1) — waveform osc + unison/sub/
/// double layers, svf filter (static or swept), ADSR×gate, at sample rate `sr`.
/// Deterministic: same input ⇒ bit-identical buffer. The JS DSP mirror
/// (`renderPCM` in `web/index.html`) consumes the SAME events via
/// `schedule_bar_json` + `bar_seconds`, so `tools/parity_test.js` proves
/// numeric DSP parity. Returns a `Float32Array`.
#[wasm_bindgen]
pub fn render_json(src: &str, bars: u32, sr: f32) -> Vec<f32> {
    loom_core::render::render_program(src, bars as usize, sr)
}

/// The resolved mix directives both engines share for a piece (P6-2): duck
/// depth, master warmth drive and the master reverb algorithm. Defaults are
/// applied Rust-side so the JS `renderFX` mirror reads plain numbers.
#[derive(Serialize)]
struct MixV1 {
    /// `duck: N` 0..0.95 sidechain pump depth (0 = off).
    duck: f32,
    /// `warmth: N` 0..1 master saturator drive (resolved default 0.3).
    warmth: f32,
    /// `"fdn"` = feedback delay network; anything else = Schroeder.
    room_kind: String,
    /// FDN decay 0..0.97 (Schroeder keeps its fixed 0.8-ish gain).
    room_decay: f32,
    /// FDN damping cutoff in Hz.
    room_damp: f32,
}

/// Serialize one piece's resolved mix settings (P6-2). The JS `renderFX` mirror
/// consumes these on top of the `schedule_bar_json` events — same Rust defaults.
#[wasm_bindgen]
pub fn mix_json(src: &str) -> String {
    let mix = &loom_core::Program::parse(src).mix;
    let room = &mix.room;
    let out = MixV1 {
        duck: mix.duck,
        warmth: mix.warmth,
        room_kind: room.kind.clone().unwrap_or_default(),
        room_decay: room.decay.unwrap_or(0.84),
        room_damp: room.damp.unwrap_or(4200.0),
    };
    serde_json::to_string(&out).unwrap_or_else(|_| "{}".to_string())
}

/// Render `bars` bars of `src` through the **full P6-2 mono pipeline**: the
/// P6-1 note engine (dry) + note inserts (`| echo/flange/phaser`) + sends
/// (`| reverb`/`| delay`) into the master reverb + ping-pong + duck + warmth
/// saturator + lookahead limiter (`loom-core::render_fx`, deterministic). The
/// JS mirror (`renderFX` in `web/index.html`) consumes the same `schedule_bar_json`
/// events + `mix_json`, so the parity harness proves DSP identity.
#[wasm_bindgen]
pub fn render_fx_json(src: &str, bars: u32, sr: f32) -> Vec<f32> {
    loom_core::render_fx::render_full(src, bars as usize, sr)
}

/// Render through the full pipeline and encode as a 16-bit mono WAV (normalized
/// to 0.97, 44-byte header). Returns the raw bytes — `tools/parity_test.js`
/// verifies the header + decodes the PCM against the JS mirror's `wavBytes`.
#[wasm_bindgen]
pub fn render_wav(src: &str, bars: u32, sr: f32) -> Vec<u8> {
    loom_core::render_fx::render_wav(src, bars as usize, sr)
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

/// The offline teacher's analysis of a piece (tonality, microtuning, tonic-triad
/// acoustics, roman-numeral harmony + cadence, "why", form, novelty score) —
/// the browser's `analyzePiece`. Pure knowledge base, no AI: works offline in
/// the browser exactly as in the native apps (shared `loom-core` code). Returns
/// the lines of the report (JS joins them).
#[wasm_bindgen]
pub fn analyze_json(src: &str) -> Vec<String> {
    loom_core::prof::analyze(src)
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
