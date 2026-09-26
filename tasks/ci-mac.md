# CI — passage de relais à Claude (sur le Mac du propriétaire)

La CI ne tourne PAS sur GitHub Actions : elle se fait sur le Mac du propriétaire
via Claude Code (subscription Max). Ce fichier aide la prochaine session Claude
côté Mac à rejouer les mêmes gardes que celles que les vagues exécutent ici.

## Les 4 gardes (ordre habituel, après `cd <repo>` / `git pull`)

```bash
# 1. Tests complets du workspace (stable)
cargo test --workspace

# 2. Linter strict : 0 avertissement exigé, tests/benches/exemples inclus
cargo clippy --workspace --all-targets -- -D warnings

# 3. Web : scripts de index.html + manifest PWA + service worker valides
node tools/check_web.js

# 4. Parité JS <-> Rust : onsets 238/238 + analyse du prof 14/14
node tools/parity_test.js

# 5. Blancs de fin / zones de conflit
git diff --check
```

## Outillage à avoir sur le Mac
- Chaîne Rust stable avec la cible wasm + `wasm-bindgen-cli` **0.2.128**
  (version verrouillée dans Cargo.lock ; le commentaire « 0.2.122 » en tête de
  `build-wasm.sh` est périmé).
- Node ≥ 18 pour `tools/check_web.js` et `tools/parity_test.js`.
- Le module WASM commité (`web/pkg`, `tools/pkg-node`) est utilisé par les tests
  de parité ; si le code Rust change, reconstruire avec `./build-wasm.sh` puis
  recomitter le pkg/ avec la vague.

## Cas particuliers connus
- `cargo fmt --check` n'est PAS une garde (une partie de la base n'est pas
  rustfmt-tée) — durcissement progressif, pas de réforme massive sans accord.
- `loom-audio` exige les headers ALSA (`libasound2-dev`) sur Linux ; sur le Mac,
  cpal passe par CoreAudio (rien à installer).
- Si une pousse GitHub est nécessaire : ne toucher à AUCUN fichier sous
  `.github/workflows/` (le token sans scope `workflow` bloque la pousse) — c'est
  pourquoi le workflow a été retiré de la branche.
