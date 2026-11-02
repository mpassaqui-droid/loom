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
 * Une prise au clavier → la ligne LOOM. `notes` : `on,off,midi` séparés par `;`.
 */
export function capture_line(name: string, notes: string, steps_per_bar: number): string;

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
 * La boucle d'une prise au micro rangée mesure par mesure, l'ancienne dessous
 * (`old` vide = pas d'ancienne). Le dernier échantillon rendu est `L`, le nombre de mesures.
 */
export function layout_take(_new: Float32Array, new_bars: number, first_bar: number, old: Float32Array): Float32Array;

/**
 * Le lien Live du programme (`go:` et automations `@voix.Param:`) sur la même
 * longueur que `bake_json` : `{"go":…, "auto":[{voice, param, points:[[temps, durée, 0..1]]}]}`.
 */
export function live_json(src: string, bars: number): string;

/**
 * The voices whose `| sample` is not in the bank, so they play on their synth, as a JSON array.
 */
export function missing_samples_json(src: string): string;

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
 * The whole native engine in the browser: drums, tone models, inserts, buses and
 * mastering, stereo, 24 bits, with the tail. The web build ships no samples: `| sample`
 * voices fall back to their synth, see `missing_samples_json`.
 */
export function render_engine_wav(src: string, bars: number, sr: number): Uint8Array;

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
 * Same engine, cut to exactly the bars with the tail folded onto the start, for looped playback.
 */
export function render_loop_wav(src: string, bars: number, sr: number): Uint8Array;

/**
 * One voice alone, through the same master chain.
 */
export function render_stem_wav(src: string, name: string, bars: number, sr: number): Uint8Array;

/**
 * Render through the full pipeline and encode as a 16-bit mono WAV (normalized
 * to 0.97, 44-byte header). Returns the raw bytes — `tools/parity_test.js`
 * verifies the header + decodes the PCM against the JS mirror's `wavBytes`.
 */
export function render_wav(src: string, bars: number, sr: number): Uint8Array;

/**
 * Le fichier d'un instrument multi-échantillon que jouerait le moteur natif :
 * `names` séparés par des retours à la ligne, `midi` NaN pour une batterie.
 * Rend `nom racine` (racine `-` sans note), ou une chaîne vide.
 */
export function sample_zone(names: string, inst: string, midi: number, vel: number, counter: number): string;

/**
 * Un fichier `.scl` (Scala) → la ligne `key:` LOOM (`ERR:` + raison si illisible).
 */
export function scala_key_line(scl: string, tonic: string): string;

/**
 * Schedule one bar and return its events as JSON for the JS audio layer.
 */
export function schedule_bar_json(src: string, bar: number): string;

/**
 * The voice names `render_stem_wav` takes, in order, as a JSON array.
 */
export function stem_names_json(src: string): string;

/**
 * `| stretch` : `left` (et `right`, vide en mono) étirés à `out_len` trames, hauteur
 * gardée. Rend les canaux bout à bout (`out_len` trames chacun).
 */
export function stretch_pv(left: Float32Array, right: Float32Array, out_len: number): Float32Array;

/**
 * Les erreurs des lignes `vis:` de `code`, une par ligne (vide si tout est lisible).
 */
export function vis_errors(code: string): string;

/**
 * Les lignes `vis:` de `code` dessinées en `w × h` RGBA (vide s'il n'y en a pas).
 */
export function vis_render(code: string, w: number, h: number, t: number, low: number, mid: number, high: number, level: number, beat: number, bar: number): Uint8Array;
