# LOOM 🎛️

A live-coding music language with an AI brain — runs in the **browser**, on your
**phone**, and (Phase 4) in the **terminal**. One small, regular language; one
shared engine; three surfaces.

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
- **Steps fill one bar evenly** — 4 tokens = quarters, 8 = eighths, 16 = sixteenths
- **Verbs:** `rev` · `fast N` · `slow N` · `every N <verb>`
- **Timbre:** `saw` `sine` `tri` `pulse` `square` · `gain N`
- `#` starts a comment

## Repo layout
| crate | role | runs where |
|-------|------|-----------|
| `loom-core` | parser + verbs + per-bar scheduler. **Zero deps, fully tested.** | everywhere |
| `loom-wasm` | `wasm-bindgen` wrapper around the core | browser / phone |
| `loom-tui`  | terminal client (skeleton) — native audio + grid UI | terminal |

The browser prototype (`loom.html`, separate) already works today with a
JavaScript engine. These Rust crates are the path to replacing that JS engine
with **one shared engine** so all three surfaces behave identically.

## Build & test (in Claude Code, where cargo is available)
```bash
# 1. install Rust if needed
curl https://sh.rustup.rs -sSf | sh

# 2. test the core logic (no deps, instant)
cargo test -p loom-core

# 3. build the WASM module for the web
cargo install wasm-pack
wasm-pack build loom-wasm --target web
#   -> then import schedule_bar_json() in loom.html, replacing the JS parser

# 4. run the terminal client (skeleton)
cargo run -p loom-tui --release
```

## Roadmap
- [x] **Phase 1** — language plays sound (browser, JS)
- [x] **Phase 2** — pattern verbs + AI brain (browser, JS)
- [x] **Phase 2.5** — synth timbres, swing, echo/reverb, presets, share codes
- [ ] **Phase 3** — port engine to `loom-core` (Rust) → WASM; swap into the browser
- [ ] **Phase 4** — `loom-tui`: native audio (cpal) + terminal grid (ratatui)
- [ ] **Later** — MIDI out, sample playback, multiplayer jams, real share URLs

## The AI brain
The browser app sends your plain-language request + the current code to a model
that returns valid LOOM. Because the grammar is tiny and regular, even a small/
cheap model is reliable. For self-hosting later, an open-weight model can fill
the same role. Keep the grammar above as the system prompt.

## License
MIT — do whatever you want. Built as a creative-coding instrument.
