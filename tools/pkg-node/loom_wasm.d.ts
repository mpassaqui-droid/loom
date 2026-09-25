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
 * Schedule one bar and return its events as JSON for the JS audio layer.
 */
export function schedule_bar_json(src: string, bar: number): string;
