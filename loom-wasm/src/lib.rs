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
    // --- P6-3A rich sources & note inserts ---
    // These carry the SAME resolved forms `render::NoteParam::of` uses, so the JS
    // `noteSample` mirror reads plain numbers (no re-clamping) and can't diverge.
    /// `| fm RATIO INDEX FB` — `[ratio, index, feedback]` (None = no FM).
    fm: Option<[f32; 3]>,
    /// `| partials a1 a2 …` / `| organ` — additive amplitudes (None = none).
    partials: Option<Vec<f32>>,
    /// `| wt organ|warm|rich|soft` — raw table name (JS mirrors `WtKind::from_str`).
    wt: Option<String>,
    /// `| pwm N` — resolved duty depth (0 = off; a present pwm is clamped 0.05..1).
    pwm: f32,
    /// `| sync RATIO` — resolved slave ratio (0 = off; a present sync is `max(1.2)`).
    sync: f32,
    /// `| bell` — modal struck-metal source.
    bell: bool,
    /// `| noise N` — resolved air-layer level 0..1 (0 = off).
    noise: f32,
    /// `| am HZ` — amplitude-modulation rate in Hz (0 = off).
    am: f32,
    /// `| ring HZ` — ring-modulation rate in Hz (0 = off).
    ring: f32,
    /// `| drive N` — resolved saturator amount 0..1 (0 = off; `base()` of a sweep).
    drive: f32,
    /// `| fold N` — resolved wavefolder mix 0..1 (0 = off).
    fold: f32,
    /// `| crush BITS` — raw bit depth (None = off; JS derives scale `2^(bits-1)`).
    crush: Option<u32>,
    /// `| redux N` — raw rate 0..1 (0 = off; JS derives sample-hold `round(1+20N)`).
    redux: f32,
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
        // P6-3A rich sources & inserts — mirror `render::NoteParam::of` exactly.
        fm: s.fm.map(|f| [f.ratio, f.index, f.fb]),
        partials: s.partials.clone(),
        wt: s.wt.clone(),
        pwm: s.pwm.map(|x| x.clamp(0.05, 1.0)).unwrap_or(0.0),
        sync: s.sync.map_or(0.0, |r| r.max(1.2)),
        bell: s.bell,
        noise: s.noise.unwrap_or(0.0).clamp(0.0, 1.0),
        am: s.am.unwrap_or(0.0),
        ring: s.ring.unwrap_or(0.0),
        drive: s.drive.map_or(0.0, |d| d.base().clamp(0.0, 1.0)),
        fold: s.fold.unwrap_or(0.0).clamp(0.0, 1.0),
        crush: s.crush,
        redux: s.redux.unwrap_or(0.0),
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

/// The program as MIDI notes per voice (`loom bake`'s JSON): the native
/// sequencer run offline, so chords, arpeggios and humanize match what you hear.
/// `bars = 0` means the whole `song:` (or 8 bars). Used by the Max for Live device.
#[wasm_bindgen]
pub fn bake_json(src: &str, bars: u32) -> String {
    let bars = if bars == 0 { loom_audio::bake::default_bars(&loom_core::Program::parse(src)) } else { bars };
    loom_audio::bake::bake(src, bars).to_json()
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

/// Bytebeat : `len` échantillons de la formule `expr` (variable `t`) à partir de
/// l'échantillon absolu `start`, à `sr` Hz (`loom_core::expr::bytebeat_fill`).
/// Expression invalide : tableau vide. Sert à la parité avec le miroir JS.
#[wasm_bindgen]
pub fn bytebeat_pcm(expr: &str, start: f64, len: u32, sr: u32) -> Vec<f32> {
    match loom_core::Expr::parse(expr, &['t']) {
        Ok(e) => loom_core::expr::bytebeat_fill(&e, start.max(0.0) as u64, len as usize, sr),
        Err(_) => Vec::new(),
    }
}

/// Évalue une expression entière (`vars` = les lettres des variables, dans
/// l'ordre de `vals`). Rend la valeur en texte, ou `ERR` si l'expression est
/// invalide. Sert à la parité avec le miroir JS `exprCompile`/`exprEval`.
#[wasm_bindgen]
pub fn expr_eval(src: &str, vars: &str, vals: &[i32]) -> String {
    let names: Vec<char> = vars.chars().collect();
    match loom_core::Expr::parse(src, &names) {
        Ok(e) => e.eval(vals).to_string(),
        Err(_) => "ERR".to_string(),
    }
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

/// Le lien Live du programme (`go:` et automations `@voix.Param:`) sur la même
/// longueur que `bake_json` : `{"go":…, "auto":[{voice, param, points:[[temps, durée, 0..1]]}]}`.
#[wasm_bindgen]
pub fn live_json(src: &str, bars: u32) -> String {
    let p = loom_core::Program::parse(src);
    let bars = if bars == 0 { loom_audio::bake::default_bars(&p) } else { bars };
    loom_core::live::live_json(src, bars, p.meter_beats)
}

/// Un clip MIDI de Live (notes `[[hauteur, début, durée, vélocité], …]` en noires)
/// devenu une ligne de code LOOM.
#[wasm_bindgen]
pub fn clip_to_loom(nom: &str, notes_json: &str, longueur: f64, meter_beats: f64, batterie: bool) -> String {
    let notes: Vec<(i32, f64, f64, u8)> = serde_json::from_str::<Vec<(f64, f64, f64, f64)>>(notes_json)
        .unwrap_or_default().into_iter()
        .map(|(p, d, l, v)| (p as i32, d, l, v.clamp(0.0, 127.0) as u8)).collect();
    loom_core::live::clip_to_loom(nom, &notes, longueur, meter_beats, batterie)
}

/// Réharmonise la ligne `prog:` (`jazz`, `tritone`, `iiv`). Renvoie le morceau
/// réécrit, ou `ERR:` suivi de la raison.
#[wasm_bindgen]
pub fn reharm(src: &str, how: &str) -> String {
    use loom_core::harmony::{reharm_source, Reharm};
    match Reharm::from_name(how).ok_or_else(|| format!("réharmonisation inconnue : {how}")).and_then(|h| reharm_source(src, h)) {
        Ok(s) => s,
        Err(e) => format!("ERR:{e}"),
    }
}
