/* tslint:disable */
/* eslint-disable */

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
