# AGENTS.md — LOOM

**Source de vérité = `CLAUDE.md`** (dans ce dossier). Lis-le en premier, à chaque session.

LOOM = un langage de **live-coding musical** avec un cerveau IA. Propriétaire =
musicien/compositeur/prof qui apprend à coder : sois un guide patient, changements
petits et testables, ne casse pas un état qui marche sans le dire.

## Repères
- `web/index.html` — prototype navigateur qui marche (moteur JS).
- `loom-core/` (Rust, testé) · `loom-wasm/` · `loom-tui/`.
- Roadmap par couches : théorie musicale → structure → variation/génératif → polyrythmie → FX/MIDI.
- `cargo test -p loom-core` doit rester vert. Petits commits, tradeoffs expliqués.
- Auth Claude = abonnement Max → **ne pas** définir `ANTHROPIC_API_KEY`.

> Note pour agents non-Claude (Codex/Cursor…) : ouvrez `CLAUDE.md` pour la vision,
> l'état actuel, la grammaire du langage et la roadmap complète.
