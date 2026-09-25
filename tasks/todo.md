# To-do — LOOM

> Plan écrit AVANT d'implémenter (items cochables), coché au fur et à mesure,
> section « Review » ajoutée en fin de session. La roadmap long-terme vit dans
> `CLAUDE.md` ; ici = le travail en cours, session par session.

## Les quatre vagues — terminées (prof interactif · moteur unifié web · vue partition · sampler)

> Décision du propriétaire (2026-09-25) : « ok for all ». Chaque vague = un incrément
> petit, testé, commité. L'ordre = impact/effort : d'abord le prof interactif hors-ligne
> (pure logique testable), puis le moteur unifié côté web, puis la vue partition (GUI),
> puis l'instrument sampler (GUI). Les quatre vagues sont livrées (voir Review en bas).

### Vague 1 — Prof interactif « explique ET modifie » (hors-ligne, sans API)
- [x] `loom-app/src/teacher/transform.rs` : `transform(src, kind) -> Result<Transformed, String>`
      — transposer (semi), sombre/clair (réécrit le mode de `key:`), dense (unison),
      sec/espace (reverb), groove (swing), varie (develop) — chaque ligne touchée garde
      commentaire ; re-parse vérifié. Tests par transformation.
- [x] Chips « TRANSFORME » dans la GUI **et** la TUI (prof applique + explique, undo partagé).

### Vague 2 — Moteur unifié côté web : le Rust pilote le rythme du web
- [x] `web/index.html` : défaut du moteur Rust « ON » quand WASM est chargé (truismé par
      parité 238/238 bit-à-bit), badge 🦀 visible + toggle pour revenir à JS à l'instant.
- [x] `tools/parity_test.js` : cas explicites `meter: 2+2+3/8` (composé) + `poly 3:2`
      (triolet) — parité remontée de 202/202 à **238/238 PERFECT**.
- [x] README/ROADMAP §7 : status « le moteur live (rythme/notes) est Rust quand chargé ;
      le son reste JS » + prochaine pierre (analyzer partagé via WASM).

### Vague 3 — Vue partition (GUI)
- [x] `loom-app/src/studio/notation.rs` : partition du roll (grande portée, têtes par voix,
      `staff()`), + `transpose_voice(src, voice, semi)` — 5 tests (portée standard, clé de fa,
      lignes supplémentaires, transposition avec commentaire conservé, lignes vides).
- [x] `loom-gui/src/studio_ui/notation.rs` : panneau « PARTITION » (portées clé de sol/fa +
      têtes colorées + lignes supplémentaires + clic = aller à la voix + armer, ◀▶ demi-ton
      et « réinit » pour transposer la voix armée, undo partagé via History).

### Vague 4 — Instrument sampler (GUI)
- [x] `loom-app/src/studio/sampletool.rs` : liste des samples + spec (sample/root/begin/end/
      chop/reverse) → réécrit `| sample …` de la voix choisie — tests.
- [x] `loom-gui/src/studio_ui/sampler.rs` : panneau « SAMPLER » (liste + curseurs + mini
      waveform du sample sélectionné).

## En cours — LOOM app native complète pour Omarchy (Rust)

Décisions du propriétaire (2026-09-14) : refonte complète, TUI **et** GUI native
**egui** (pas une web app), Rust installé via mise.

Architecture : `loom-core` (langage, inchangé) → `loom-audio` (NOUVEAU : synthé
+ séquenceur natif, partagé) → `loom-tui` (terminal) et `loom-gui` (fenêtre egui).

### Phase 1 — moteur audio natif `loom-audio`
- [x] Synthé DSP : kick / snare / hat / clap (portés de web/index.html), tone saw/sine/tri/square, pan, vel
- [x] Séquenceur à horloge d'échantillons : `Program::schedule_bar` → notes, tempo/meter/song respectés
- [x] Hot-swap du code quantisé à la mesure suivante (live-coding sans coupure)
- [x] Rendu hors-ligne + export WAV (`loom render x.loom out.wav`) — testable sans carte son
- [x] Sortie cpal (PipeWire/ALSA) + tests

### Phase 2 — `loom-tui`
- [x] Éditeur de code + grille de pas animée + transport (RUN/STOP, bar, section, BPM)
- [x] Ouvrir / sauver des `.loom`, rechargement à chaud, couleurs du thème Omarchy

### Phase 3 — `loom-gui` (egui)
- [x] Fenêtre : éditeur, grille, transport, volume, navigateur des `.loom`
- [x] Couleurs du thème Omarchy

### Phase 4 — intégration Omarchy
- [x] `.desktop` (menu Super+Espace) pour TUI et GUI, icône
- [x] Script `install.sh` (cargo install + desktop entries)
- [x] Raccourci Hyprland : Super+Shift+L (GUI), Super+Shift+Alt+L (TUI)

### Vague 2 — « tout porter » (commencée 2026-09-14, terminée 2026-09-15)
- [x] loom-core : ~95 verbes de son (`synth.rs`), directives mix (duck/feel/warmth/room/bus/morph/loop/tune), macros `def`, arp modes, `stutter`
- [x] loom-audio : dsp/clock/note, instrument (`tone/`), effets par note (`inserts/`), master (`master/`), samples WAV (`sample/`), MIDI in/out (`midi.rs`), micro (`input.rs`), REC, métronome, morph, mixeur (gain/pan/solo/niveaux), swing, SPACE/ECHO/EQ, stems, flux visuel (hits/scope/CC)
- [x] VIBE (F2) dans TUI + GUI (claude CLI, Fable 5 par défaut) · SCÈNE : 24 scènes GUI (F11), 5 TUI (F12)
- [x] partage compatible web (`loom-app/src/share.rs`)
- [x] Studio (GUI + TUI) : mixeur, knobs master, stems, tap, LUFS, MIDI learn, partage, piano-roll, timeline éditable, démo, clips
- [x] Prof & composition (logique, loom-app) : generate, develop, ear, hum, prof (explique/?question/lookup), 57 leçons + 163 références
- [x] Prof & composition branchés dans GUI + TUI : GÉNÉRER, DÉVELOPPE, annuler/rétablir partagé, EXPLIQUE, ?question, OREILLE, HUM, LEÇONS (`loom-gui/src/teacher/` partagé, `teacher_ui/`, `app_teacher.rs` ×2, `loom-tui/src/teacher_view.rs`)
- [x] `loom render morceau.loom dossier --stems [N]` en CLI (+ `loom render --help`)
- [x] Audit : 62/62 morceaux rendus, aucun silence, crêtes ≤ 0.96, médiane 34× temps réel
- [x] Fix core : `seed: N # commentaire` était lu comme 0 (aurora/showcase/expose)
- [x] Tests workspace (361 ok), README/CLAUDE.md, `./install.sh`, commit + push

Mesures : showcase en live ≈ 20 % d'un cœur, 22 Mo ; rendu hors-ligne 11–40× temps réel.

## Review

**2026-09-14 — apps natives livrées et installées (`./install.sh`).**
- `loom-audio` (nouveau) : synthé natif + séquenceur + cpal + export WAV. `loom-app` (nouveau) : thème Omarchy live, bibliothèque, grille.
- `loom` (TUI) et `loom-gui` (egui) : éditeur coloré, grille animée, transport, mute, volume, export, dernier fichier retenu.
- Bugs corrigés dans `loom-core` : `Program::schedule_bar` ignorait `prog:` (les voix `^` étaient muettes, en natif ET dans l'A/B WASM du web). `Event` porte maintenant `dur`.
- Tests : core 90 · audio 7 · app 4 · tui 3 · gui 12.
- Limites connues : le natif n'a pas encore FX / filtres / env / unison / samples / MIDI / VIBE → `h_concrete` (100 % samples) est muet, les autres sonnent « nus ». Le parser ignore silencieusement les lignes invalides.
- Idées suivantes : Status « changement en attente » pour la grille ; erreurs de parse avec n° de ligne ; porter `lp`/`env`/`unison` puis les samples (`web/samples`).

**2026-09-15 — vague 2 : le moteur web entier porté en natif.**
- Moteur : instruments, samples, effets par note, master, MIDI, REC, morph, mixeur ; 62/62 morceaux rendus, live ≈ 20 % d'un cœur.
- Apps : VIBE, SCÈNE (24 GUI / 5 TUI), Studio (11 outils), Prof & composition, leçons.
- Bugs core corrigés : `prog:` ignoré par Program, `seed: N # commentaire` lu 0, `x` mélodique muet.
- Tests : 361 ok (core 96 · audio 70 · app 47 · gui 75 · tui 73).
- Non vérifié : écoute réelle sur enceintes, clics souris dans la GUI (logique testée), MIDI learn avec un vrai contrôleur, micro réel pour HUM.
- Pistes : partager un `ir` par voix (CPU), déplacer `studio/` et `teacher/` de loom-gui vers loom-app, analyse hors-ligne `analyzePiece` du prof pas portée.

**2026-09-16 — analyse hors-ligne du prof portée (`analyzePiece`).**
- `loom-app/src/prof/analyze.rs` (neuf) : port du prof hors-ligne du web — tonalité + lore,
  tradition (raga/maqam/gamelan/modes/blues…), microtonalité, accordage, consonance de
  l'accord de tonique (Plomp–Levelt), harmonie en chiffrage romain + cadences, « pourquoi ça
  sonne », forme (`song:`), et rapport de nouveauté (variété/hauteurs/rythme) via
  `Program::schedule_bar`. 10 tests, `pub use prof::analyze`.
- EXPLIQUE dans la GUI et la TUI : l'analyse hors-ligne s'affiche immédiatement, la réponse
  IA s'ajoute ensuite (comme sur le web).
- Tests : 371 ok (core 96 · audio 70 · app 57 · gui 75 · tui 73).

**2026-09-16 — studio/ et teacher/ déplacés dans loom-app.**
- `loom-app/src/studio/` et `loom-app/src/teacher/` (logique pure) : plus de `#[path]`
  croisé entre crates — la GUI et la TUI utilisent `loom_app::studio` /
  `loom_app::teacher` ; `loom-app` dépend maintenant de `loom-audio`.
- Idem EXPLIQUE (voir ci-dessus). Tests uniques : 322 ok (app 106 · audio 70 · core 96 · gui 26 · tui 24 — les tests studio/teacher ne sont plus dupliqués GUI+TUI).

**2026-09-16 — `| ir` partagé (cache de géométrie, comme `ctx._irCache[type]`).**
- `loom-audio/src/inserts/space.rs` : la géométrie du réseau `| ir` (longueurs de lignes,
  gains, diffuseurs, coeff de damping, niveau, t60, idle_limit) est calculée une seule fois
  par (espace, fréquence) et partagée en `Arc` entre toutes les notes — au lieu d'être
  re-dérivée à chaque note. 3 tests : partage du cache, géométrie = calcul vierge, deux
  notes sonnent identiques. Tests : 325 ok au total (audio 73).


**2026-09-16 — langage : polyrythmie explicite `poly m:n` + métrique composée.**
- `loom-core` : nouveau `| poly m:n` (triolet — la mesure est coupée en n fenêtres de
  m pas espacés : 3:2 = trois contre deux), distinct de l'ancien `| poly N` (phasing).
  Scheduler + 2 tests (regroupement 3-dans-2, glissement de phrase, même grille que
  `poly m*n`). `meter: 2+2+3/8` (métrique composée) accepté : temps = somme des groupés,
  groupes exposés par `parse_meter_groups` ; `parse_meter` garde le comportement.
- `loom-app` l'analyse offline nomme la métrique composée (`3+2+2 (3,5 temps)`).
  Tests : 328 ok au total (app 107 · core 98).

**2026-09-18 — CI + docs (wave 5).**
- `.github/workflows/ci.yml` : job `rust` = build + `cargo test --workspace`
  (331 tests) + clippy conseil ; job `web` = `node tools/check_web.js` (scripts
  index.html + manifest PWA + sw.js) + `node tools/parity_test.js` (202/202).
  Déclenché sur main + pull requests ; cpal/midir → apt libasound2-dev.
  `cargo fmt` pas encore une garde (base jamais rustfmt-tée — durcissement
  progressif, documenté dans le workflow).
- `tools/check_web.js` : validation structurelle du web/ (parse des scripts
  inline, JSON du manifest, icônes présentes, service worker) — partagé CI/local.
- README.md à jour : `cargo test --workspace` (331), `./build-wasm.sh` +
  wasm-bindgen 0.2.128 (remplace l'ancienne voie wasm-pack), outils de vérif,
  statut moteur unifié (WASM chargé non-furtif → `window.__loomRust`, parité
  garantie, moteur live encore côté JS), PWA. ROADMAP.md §7 passé en ✅.

**2026-09-18 — PWA (web/ s'installe comme une app, joue hors-ligne).**
- `web/manifest.webmanifest` : nom/icônes 192+512+maskable (générées ImageMagick,
  monogramme LOOM vert sur noir), standalone, thème `#0b0f0b`, langue fr.
- `web/sw.js` : coquille préchargée (index.html, pkg/loom_wasm.js+wasm, icônes),
  navigation réseau d'abord → repli coquille hors-ligne, statique (samples/*.wav,
  pkg/*) en cache d'abord → un premier passage rend les samples joués jouables
  hors-ligne. Activation/claims immédiats, purge des vieilles caches.
- `web/index.html` : metas manifest/theme-color/apple-touch + enregistrement SW
  (https ou localhost uniquement) + classe `body.standalone` (padding réduit en
  app installée). ROADMAP.md §7 passé en ✅.

**2026-09-18 — maths : Markov appris + solveur à contraintes + parité 202/202.**
- `loom-core` + `web` : `| markov [N] learn: <phrase>` (chaînes apprises d'un morceau) —
  transition table apprise depuis la phrase, ordre N, longueur propre de la voix conservée,
  clé RNG inchangée (rien ne régresse). Tests : `markov_learns_from_a_piece`,
  `markov_order_n_learns_from_a_piece`.
- `loom-core` + `web` : **solveur à contraintes `solve(n,target:T,ladder:L,range:lo-hi,arc)`** —
  remontée stochastique depuis la cible imposée (dernier pas = target), pas ≤ ladder,
  ambitus [lo,hi], option `arc` (montée → pic → descente sur la cible), seedée/déterministe.
  Test natif `constraint_solver_honors_the_rules` ; fix E0606 dans le draw pondéré.
- Chaos mort (`hash_u64`, `fnv`) supprimés de `loom-core` (0 warning).
- `web` : aide + suggestion bot + analyzeur à jour (solve/markov learn) ; ROADMAP.md §4 ✅.
- `tools/parity_test.js` : cas markov m1-m4 + `solve` m1-m4 ajoutés → **202/202 voix×mesure
  ✅ PERFECT** (le solveur est bit-pour-bit identique JS/Rust via WASM).
- Toolchain WASM : `rustup target add wasm32-unknown-unknown` + `cargo install
  wasm-bindgen-cli 0.2.128` (aligné sur Cargo.lock) ; `./build-wasm.sh` re-runs web/pkg + tools/pkg-node.

**2026-09-17 — langage (parité web) + undo/redo vérifié.**
- `web/index.html` : `poly 3:2` (triolet, même grille que la native) + `meter: 2+2+3/8`
  (métrique composée) portés côté JS ; l'aide affiche le triolet. Aides à l'éditeur
  mises à jour. `node` : script principal sans erreur de syntaxe ; tests isolés de
  `parseMeter` (7/8=3.5, 3+2+2/8=3.5, 5/4=5, 3=3).
- `tools/parity_test.js` (JS↔Rust sur les onsets) : 170/170 voix×mesure ✅ PERFECT.
- Undo/redo des chargements déjà complet et testé dans les apps natives (VIBE/IA,
  GÉNÉRER, DÉVELOPPE, HUM, leçon — 30 étapes, F3/Maj-F3/Ctrl+Y), `History` dans
  `loom-app/teacher/history.rs` ; ROADMAP.md §2 passé en ✅.

**2026-09-25 — fin de la vague 5 (CI engagé) + état tout-vert confirmé.**
- `.github/workflows/ci.yml` était décrit dans « wave 5 » et ROADMAP §7 mais jamais
  commité (répertoire inédit → github/ jamais ajouté). Commit `5e7e9e8` : le
  workflow rentre enfin dans l'historique. Gates : build+test workspace (stable),
  clippy conseil (24 warnings non fatals — exit 0 → la porte `test ${PIPESTATUS[0]}
  -eq 0` passe), `node tools/check_web.js` + `node tools/parity_test.js`.
- Vérification locale complète : `cargo test --workspace` 331 ok · parité JS↔Rust
  202/202 ✅ PERFECT · check_web 4/4 ✓.
**2026-09-25 — Vague 1 « Prof interactif » : explique ET modifie (`c504d28`).**
- `loom-app/src/teacher/transform.rs` (neuf) : `transform(src, kind, seed)` réécrit une
  voix choisie (rythme `| semi N`, mode `key:`, `| unison 3`, `| reverb 0.4`, `| swing 0.25`,
  `develop()`) tout en gardant commentaires + autres effets, sans double-dose
  (familles déjà couvertes refusées poliment). 6 tests.
- GUI : chips « TRANSFORME » + `TeacherAction::Transform(Kind)` + `replace_song`
  (undo partagé). TUI : popup Alt+W (Alt+T = TAP du studio).
- Tests : loom-app 107 → 113.

**2026-09-25 — Vague 2 : le moteur Rust→WASM pilote vraiment le web (`312ffe7`).**
- `web/index.html` : `window.__loomUseRust = true` par défaut, case à cocher cochée,
  badge « Rust actif » / « mode JS (repli) » (repli JS instantané, try/catch),
  parité de rendu garantie.
- `tools/parity_test.js` : 238 cas (202 précédents + 36 voix×mesure, métriques
  composées 2+2+3/8 et poly 3:2) → **238/238 PERFECT**.
- README/ROADMAP/§7 + CI à jour, honnêtement. Prochaine pierre documentée :
  déplacer `analyze.rs` + `lore.rs` dans `loom-core` (le WASM ne dépend que de core).

**2026-09-25 — Vague 3 : vue partition en solfège (GUI) (`15f35ec`).**
- `loom-app/src/studio/notation.rs` (neuf) : `staff(roll)` → portée grand format
  (clé de sol + clé de fa, degrés, dièses/bémols, lignes supplémentaires) +
  `transpose_voice` (réécrit le dernier `| semi`, commentaires préservés, variantes
  `bass@drop` transposées chacune sur leur propre ligne). 5 tests.
- `loom-gui/src/studio_ui/notation.rs` : panneau « PARTITION » — clic une note = aller à
  sa voix + l'armer, boutons ◀ ▶ réinit transposent d'un demi-ton (annulable).
  Pas de 𝄞/𝄢 (glyphes absents des fonts egui) : libellés « clé de sol / clé de fa ».
- Tests : 342 ok (app 118 · audio 73 · core 101 · gui 26 · tui 24).

**2026-09-25 — Vague 4 : instrument sampler (GUI).**
- `loom-app/src/studio/sampletool.rs` (neuf) : spec d'un sample (sample/root/begin/end/
  chop/reverse), `read` depuis le Synth (ce que l'oreille du moteur entend), `apply_verb`
  / `apply_spec` réécrivent la ligne de la voix armée (dernier segment remplacé ou
  ajouté, alias `s` géré, commentaires préservés, `bass@drop` visé par son nom complet,
  voix inconnue refusée) — 6 tests, clippy propre.
- `loom-gui/src/studio_ui/sampler.rs` : panneau « SAMPLER » — chips par voix (armée/
  déjà-échantillonnée), liste réelle du `default_bank()` partagé avec le moteur
  (noms + durée), racine par texte, curseurs begin/end/chop, case reverse, mini
  waveform du sample choisi avec marqueurs begin/end, bouton APPLIQUER (annulable).
  Bouton « SAMPLER » dans la barre du studio (entre PARTITION et CLIPS).
- Tests : **348 ok** (app 124 · audio 73 · core 101 · gui 26 · tui 24) ; build 0 erreur.
- Piste suivante documentée : `analyze.rs` + `lore.rs` vers `loom-core` pour
  l'analyseur dans le WASM du web (clé de voûte du moteur unifié).
