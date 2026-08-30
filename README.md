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

## What's in this repo
| path | what it is | runs where |
|------|------------|-----------|
| `loom-core` | Parser, verbs, per-bar scheduler. Zero dependencies. | anywhere |
| `loom-wasm` | `wasm-bindgen` wrapper around `loom-core` | browser |
| `loom-tui` | Terminal client (ratatui for the grid UI, cpal for audio) | terminal |
| `web/index.html` | Standalone browser prototype, JS engine. Includes "VIBE": type a plain-language request and an AI model writes the LOOM code. | browser |

`loom-core` has no external dependencies and is the one crate meant to run
identically everywhere else. `loom-wasm` and `loom-tui` build on top of it.

## Build & test
```bash
# core logic (no deps, instant)
cargo test -p loom-core

# WASM module for the browser
cargo install wasm-pack
wasm-pack build loom-wasm --target web

# terminal client
cargo run -p loom-tui --release
```

`loom-core` has 88 tests and all of them pass (`cargo test -p loom-core`).

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
