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

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly bar_bpm: (a: number, b: number, c: number) => number;
    readonly bar_seconds: (a: number, b: number, c: number) => number;
    readonly program_info_json: (a: number, b: number) => [number, number];
    readonly schedule_bar_json: (a: number, b: number, c: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
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
