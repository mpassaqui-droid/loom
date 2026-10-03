/* tslint:disable */
/* eslint-disable */

/**
 * The offline teacher's analysis of a piece (tonality, microtuning, tonic-triad
 * acoustics, roman-numeral harmony + cadence, "why", form, novelty score) —
 * the browser's `analyzePiece`. Pure knowledge base, no AI: works offline in
 * the browser exactly as in the native apps (shared `loom-core` code). Returns
 * the lines of the report (JS joins them).
 */
export function analyze_json(src: string): string[];

/**
 * The program as MIDI notes per voice (`loom bake`'s JSON): the native
 * sequencer run offline, so chords, arpeggios and humanize match what you hear.
 * `bars = 0` means the whole `song:` (or 8 bars). Used by the Max for Live device.
 */
export function bake_json(src: string, bars: number): string;

/**
 * Tempo in BPM at a given bar (after any `accel:` ramp / per-section `tempo:`).
 */
export function bar_bpm(src: string, bar: number): number;

/**
 * Duration of a given bar in seconds, honouring `meter:` and `accel:`/`tempo:`.
 * The JS transport multiplies each event's `phase` (0..1) by this.
 */
export function bar_seconds(src: string, bar: number): number;

/**
 * Bytebeat : `len` échantillons de la formule `expr` (variable `t`) à partir de
 * l'échantillon absolu `start`, à `sr` Hz (`loom_core::expr::bytebeat_fill`).
 * Expression invalide : tableau vide. Sert à la parité avec le miroir JS.
 */
export function bytebeat_pcm(expr: string, start: number, len: number, sr: number): Float32Array;

/**
 * Un clip MIDI de Live (notes `[[hauteur, début, durée, vélocité], …]` en noires)
 * devenu une ligne de code LOOM.
 */
export function clip_to_loom(nom: string, notes_json: string, longueur: number, meter_beats: number, batterie: boolean): string;

/**
 * Le correcteur de contrepoint (quintes/octaves parallèles, croisements, sauts).
 */
export function counterpoint_json(src: string): string[];

/**
 * Évalue une expression entière (`vars` = les lettres des variables, dans
 * l'ordre de `vals`). Rend la valeur en texte, ou `ERR` si l'expression est
 * invalide. Sert à la parité avec le miroir JS `exprCompile`/`exprEval`.
 */
export function expr_eval(src: string, vars: string, vals: Int32Array): string;

/**
 * Le lien Live du programme (`go:` et automations `@voix.Param:`) sur la même
 * longueur que `bake_json` : `{"go":…, "auto":[{voice, param, points:[[temps, durée, 0..1]]}]}`.
 */
export function live_json(src: string, bars: number): string;

/**
 * Serialize one piece's resolved mix settings (P6-2). The JS `renderFX` mirror
 * consumes these on top of the `schedule_bar_json` events — same Rust defaults.
 */
export function mix_json(src: string): string;

/**
 * Top-level program info (tempo, meter, arrangement length) as JSON.
 */
export function program_info_json(src: string): string;

/**
 * Réharmonise la ligne `prog:` (`jazz`, `tritone`, `iiv`). Renvoie le morceau
 * réécrit, ou `ERR:` suivi de la raison.
 */
export function reharm(src: string, how: string): string;

/**
 * Render `bars` bars of `src` through the **full P6-2 mono pipeline**: the
 * P6-1 note engine (dry) + note inserts (`| echo/flange/phaser`) + sends
 * (`| reverb`/`| delay`) into the master reverb + ping-pong + duck + warmth
 * saturator + lookahead limiter (`loom-core::render_fx`, deterministic). The
 * JS mirror (`renderFX` in `web/index.html`) consumes the same `schedule_bar_json`
 * events + `mix_json`, so the parity harness proves DSP identity.
 */
export function render_fx_json(src: string, bars: number, sr: number): Float32Array;

/**
 * Render `bars` bars of `src` into mono f32 PCM samples via the unified
 * std-only renderer (`loom-core::render`, P6-1) — waveform osc + unison/sub/
 * double layers, svf filter (static or swept), ADSR×gate, at sample rate `sr`.
 * Deterministic: same input ⇒ bit-identical buffer. The JS DSP mirror
 * (`renderPCM` in `web/index.html`) consumes the SAME events via
 * `schedule_bar_json` + `bar_seconds`, so `tools/parity_test.js` proves
 * numeric DSP parity. Returns a `Float32Array`.
 */
export function render_json(src: string, bars: number, sr: number): Float32Array;

/**
 * Render through the full pipeline and encode as a 16-bit mono WAV (normalized
 * to 0.97, 44-byte header). Returns the raw bytes — `tools/parity_test.js`
 * verifies the header + decodes the PCM against the JS mirror's `wavBytes`.
 */
export function render_wav(src: string, bars: number, sr: number): Uint8Array;

/**
 * Schedule one bar and return its events as JSON for the JS audio layer.
 */
export function schedule_bar_json(src: string, bar: number): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly analyze_json: (a: number, b: number) => [number, number];
    readonly bake_json: (a: number, b: number, c: number) => [number, number];
    readonly bar_bpm: (a: number, b: number, c: number) => number;
    readonly bar_seconds: (a: number, b: number, c: number) => number;
    readonly bytebeat_pcm: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly clip_to_loom: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly counterpoint_json: (a: number, b: number) => [number, number];
    readonly expr_eval: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly live_json: (a: number, b: number, c: number) => [number, number];
    readonly mix_json: (a: number, b: number) => [number, number];
    readonly program_info_json: (a: number, b: number) => [number, number];
    readonly reharm: (a: number, b: number, c: number, d: number) => [number, number];
    readonly render_fx_json: (a: number, b: number, c: number, d: number) => [number, number];
    readonly render_json: (a: number, b: number, c: number, d: number) => [number, number];
    readonly render_wav: (a: number, b: number, c: number, d: number) => [number, number];
    readonly schedule_bar_json: (a: number, b: number, c: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_drop_slice: (a: number, b: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
