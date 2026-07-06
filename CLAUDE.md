# CLAUDE.md — read this first, every session

You are helping build **LOOM**, a live-coding music language with an AI brain.
The owner is a musician/composer/teacher learning to code. Be a patient guide:
explain choices in plain language, keep changes small and testable, and never
break a working state without saying so.

## The vision (the compass — not a single task)
LOOM should let anyone make music by writing a tiny, readable language OR by
describing music in plain words and letting an AI write the LOOM code. The long
dream: rich enough to express music theory and traditions from around the world
(maqam, raga, gamelan, modes, jazz harmony…), song structure, and generative
variation. We get there in LAYERS, not all at once. Knowledge lives in the AI
model + good prompting + a growing language — we do NOT hardcode "all of music".

## Current state
- `web/index.html` — the working browser prototype (JS engine). Plays chip-tune
  drums + synth voices, has pattern verbs, presets, FX, and a "VIBE" AI box.
- `loom-core/` — Rust port of the language (parser, verbs, scheduler) WITH tests.
- `loom-wasm/` — wasm-bindgen wrapper (browser/phone).
- `loom-tui/` — terminal client skeleton (native audio TODO).

## The language today
- `name: pattern | verb | verb`
- drums (kick snare hat clap): `x` hit, `.` rest
- melody (any other name): note names `c2 eb3 g4`
- verbs: `rev`, `fast N`, `slow N`, `every N <verb>`
- timbre: `saw sine tri pulse square`, `gain N`

## Roadmap (build in this order, each as its own small PR)
1. **Music theory layer**: `key: c minor`, `scale: dorian`, `chord: Cm7`,
   world scales (maqam, raga, pentatonic, gamelan). Notes can be scale degrees.
2. **Structure**: sections, arrangement (intro/verse/chorus/drop), song timeline.
3. **Variation / generative**: euclidean rhythms, probability, mutation, seeds.
4. **Polyrhythm/polymeter, tempo & meter changes.**
5. **Effects, sample playback, MIDI out.**
6. **Port the JS engine to loom-core (Rust→WASM) so all surfaces share one engine.**

## Session cycle (tasks/)
- Start of session: read `tasks/todo.md` (current work) and `tasks/lessons.md` (past corrections).
- Before implementing anything non-trivial: write the plan as checkable items in `tasks/todo.md`, then check them off as you go; add a short "Review" note at the end.
- After ANY correction from the owner: add one lesson to `tasks/lessons.md` (one lesson per entry).

## Conventions
- Keep `loom-core` dependency-free and fully tested. Run `cargo test -p loom-core`.
- Every new language feature: update the parser, add tests, then expose it in
  `web/index.html` and update the AI system prompt so the brain can use it.
- Small commits, clear messages. Explain tradeoffs to the owner.

## The AI brain
Use the strongest available Claude model (Fable 5, formerly Opus) for composition
quality. The owner has a Max subscription — authenticate Claude Code with that
(do NOT set ANTHROPIC_API_KEY, or it bills the API instead of the subscription).

## First task suggestion
Run `cargo test -p loom-core` to confirm the core builds, then start the music
theory layer (item 1): add `key:` and `scale:` so melodic voices can use scale
degrees (1 2 3 4 5 6 7) that resolve against the key. Add tests first.
