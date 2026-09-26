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
 * Tempo in BPM at a given bar (after any `accel:` ramp / per-section `tempo:`).
 */
export function bar_bpm(src: string, bar: number): number;

/**
 * Duration of a given bar in seconds, honouring `meter:` and `accel:`/`tempo:`.
 * The JS transport multiplies each event's `phase` (0..1) by this.
 */
export function bar_seconds(src: string, bar: number): number;

/**
 * Top-level program info (tempo, meter, arrangement length) as JSON.
 */
export function program_info_json(src: string): string;

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
 * Schedule one bar and return its events as JSON for the JS audio layer.
 */
export function schedule_bar_json(src: string, bar: number): string;
