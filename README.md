# LOOM

A minimal live-coding music language. One voice per line, a tiny grammar, and
verbs to reshape a pattern in place.

```
kick:  x . . . x . . .
snare: . . x . . . x .
hat:   x x x x x x x x | every 4 fast 2
bass:  c2 . eb2 . g2 . eb2 . | saw | every 8 rev
```

## The language in 30 seconds
- **One voice per line:** `name: pattern | verb | verb`
- **Drums** (`kick snare hat clap`): `x` = hit, `.` = rest
- **Melody** (any other name): note names like `c2 eb3 g4`, `.` = rest
- **Steps fill one bar evenly:** 4 tokens = quarters, 8 = eighths, 16 = sixteenths
- **Verbs:** `rev` · `fast N` · `slow N` · `every N <verb>`
- **Timbre:** `saw` `sine` `tri` `pulse` `square` · `gain N`
- `#` starts a comment

## Native apps (Linux / Omarchy)
```bash
./install.sh            # builds and installs `loom` + `loom-gui` for your user (no sudo)
./install.sh --uninstall
```
- **`loom-gui`**: a window with the editor, a live step grid, transport, volume, and the song library.
- **`loom`**: the same thing in the terminal. Also `loom play song.loom` (headless) and
  `loom render song.loom out.wav [bars]` (offline bounce).
- Both play sound natively through PipeWire/ALSA. Code edits hot-swap at the next bar
  line, so the music never stops while you type.
- On [Omarchy](https://omarchy.org) the apps appear in the Super+Space menu, take their
  colours from the active theme, and recolour live on `omarchy theme set`. `.loom` files
  open in LOOM.

The native engine is a port of the browser engine: synth drums and samples
(`| sample bd`, warp, freeze, chop, grain), oscillators with unison, wavetables,
FM, plucked/bowed/reed strings, granular, filters and envelopes, the per-note
effects (`drive chorus phaser crush echo ir vowel`…), the master chain (reverb,
ping-pong delay, `duck:`, glue compressor, EQ, `warmth:`, limiter), `bus:`
submixes, `morph:` crossfades, MIDI in/out (a virtual "LOOM" port, clock sync,
live keyboard), REC to WAV, a metronome, a mixer, the VIBE AI box (through the
`claude` CLI) and the visual stage (SCÈNE).

Around it, the apps carry the browser's studio and teacher tools: mixer with
master knobs, stems export (`loom render song.loom dir --stems`), tap tempo,
LUFS meter, MIDI learn, piano roll, editable song timeline, demo tour, clips,
share codes compatible with the web (`LOOM1:`), GÉNÉRER / DÉVELOPPE, the AI
teacher (EXPLIQUE, `?question`), ear training, HUM (sing a melody, LOOM writes
it) and 57 lessons + the language reference. Press F1 in either app for the keys.

## What's in this repo
| path | what it is | runs where |
|------|------------|-----------|
| `loom-core` | Parser, verbs, per-bar scheduler. Zero dependencies. | anywhere |
| `loom-audio` | Native engine: sequencer, instruments, samples, effects, master, MIDI, live output (cpal), WAV rendering | desktop |
| `loom-app` | Shared logic for the native apps: Omarchy theme, song library, step-grid model, VIBE, web-compatible share codes | desktop |
| `loom-tui` | Terminal app `loom` (ratatui) | terminal |
| `loom-gui` | Window app `loom-gui` (egui) | desktop |
| `loom-wasm` | `wasm-bindgen` wrapper around `loom-core` | browser |
| `web/index.html` | Standalone browser prototype, JS engine. Includes "VIBE": type a plain-language request and an AI model writes the LOOM code. | browser |

`loom-core` has no external dependencies and is the one crate meant to run
identically everywhere else. Everything else builds on top of it.

## Build & test
```bash
# core logic (no deps, instant)
cargo test -p loom-core

# WASM module for the browser
cargo install wasm-pack
wasm-pack build loom-wasm --target web

# native apps
cargo run -p loom-tui --release          # terminal
cargo run -p loom-gui --release          # window
cargo test -p loom-audio -p loom-app     # native sound + shared app logic
```

`loom-core` has 90 tests and all of them pass (`cargo test -p loom-core`).

## Status
This is a repo in transition, not a finished product. The browser prototype
(`web/index.html`) still runs its own hand-written JS engine, built before
`loom-core` existed. `loom-core` is the Rust rewrite of that same language:
parser, pattern verbs, and scheduler, with tests. The two are not wired
together yet. The near-term work is compiling `loom-core` to WASM and swapping
it into the browser prototype in place of the JS engine, so all surfaces
(browser, terminal, and any future one) share one engine instead of two
implementations of the same language drifting apart.

A separate, more polished web demo, `loom-studio`, exists as a companion
project and is not part of this repo.

## License
MIT.
