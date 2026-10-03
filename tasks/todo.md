# To-do — LOOM

> Plan écrit AVANT d'implémenter (items cochables), coché au fur et à mesure,
> section « Review » ajoutée en fin de session. La roadmap long-terme vit dans
> `CLAUDE.md` ; ici = le travail en cours, session par session.

## Session 2026-10-03 · verbes Tidal (branche verbes-tidal)

- [x] Inventaire : nudge, stut (alias de stutter), strum, chop existent déjà.
- [x] loom-core : `ply N`, `linger F`, `segment N`, `range A B` (+ tests), `loopAt N` (synth.rs).
- [x] loom-audio : `loopAt N` étire le warp sur N mesures (drums et mélodie, + test).
- [x] web : miroir JS (applyOne, parse, trigger), prompt VIBE, aide, mots-clés.
- [x] parity_test : 5 pièces Tidal, parité stricte onset + hauteur.
- [x] Exemple `tidal.loom`.

Review : écartés `stut N FB T` (nom déjà pris par stutter ; l'écho existe via
`| echo DIV FB` et `| off N`), `rolled` (= `| strum`), `nudge` (existe),
`chop` (existe), `striate` (tranchage entrelacé qui demande la phase de
l'événement dans les 3 moteurs audio ; reporté).

## Session 2026-09-25 — vague 6 : GPU de plus · bus&sends · algocompo · accessibilité · connaissance

> Décision du propriétaire : « ok pour tout » — on lance la vague 6 dans l'ordre
> d'effort croissant : P1 → P3 → P4 → P2 → P5. Chaque piste = incrément testé +
> committé + poussé ; portes : tests workspace, clippy -D, check_web, parité,
> diff --check. Rust touchant la sortie analysée → rebuild WASM. P6 (moteur
> unifié synthèse/FX WASM) reste un chantier séparé, pas dans cette vague.

- [x] **P1 · Plus de scènes GPU (web)** — suite de T5 : raymarching 3D (SDF
      sphères/tore), néons (tubes le long du beat), hyperspace (warp stellaire) ;
      même mécanisme `uMode` + post déjà en place ; GLSL ES 1.00 validé
      (`glslangValidator`) + `check_web.js` 4/4 + parse JS. commit ceae1ee
- [x] **P2 · Accessibilité** (web + GUI) — navigation 100 % clavier, contrastes
      des thèmes (WCAG AA), grandes polices / zoom, focus visible.
  - [x] Audit WCAG Python des 62 thèmes (luminance relative) : `dim` était le grand
        fautif ; AG = carte statique `THEME_AA` (62 entrées [fg,dim,glow,amber,cyan],
        même teinte, pas 0.005 en HLS) → **fails: 0/62**, min = 4.5 (vt100/dim).
  - [x] Web : `THEME_AA` inséré, `_aaMode` **défaut ON**, sélecteur « Contraste AA /
        d'origine » persistant (`loom-aa`) ; `paintTheme` AA-aware (k[1..5] sous AA, bg
        intact, `_themeFlat` du thème de base).
  - [x] Web : sélecteur « Taille normale / Grande (accessibilité) » persistant
        (`loom-size`) ; `body.size-lg` (fonts ×1.2 posé après font-*) + zoom navigateur.
  - [x] Web : focus visible — `:focus-visible` (lèvre `--bg` + anneau `--glow` AA
        partout), cellules grille + `select` ; `@media (prefers-reduced-motion:reduce)`
        tue le clignotement UI (`.blink`, `#fpBtn`, transitions playhead).
  - [x] Web : clavier grille — cellules `tabindex=0` `role=button` `aria-label`,
        ←/→ focus rangée, ↑/↓ cycle hauteur (ou toggle drums), Entrée/Espace toggle ;
        `aria-label` sur themePrev/Next/randTheme et Mute/Solo (grille + mixeur).
  - [x] Web portes : moteur parse (`node --check`), check_web 4/4, parité 278/278 +
        23/23 inchangées (couleurs ≠ timing), audit AA fails 0/62.
  - [x] GUI (egui) : toggle « grande police » — bouton A/A+ (top-bar) + Ctrl+Shift+A,
        persistant (`Theme::ui_large`/`set_ui_large` dans `$STATE/omarchy/current`),
        scale ×1.25 sur tous les styles egui + éditeur (`style::palette(theme, large)`) ;
        Tab/Entrée natifs egui, hover/focus soulignés accent (déjà en place).
  - [x] Portes Rust : `cargo test --workspace` verts (114/77/123/26/24), clippy -D 0,
        diff --check ; pas de changement `loom-core` → pas de rebuild WASM.
- [x] **P3 · Bus & sends** — le vrai trou DAW : groupes de voix (`bus:`), N sends
      vers des returns d'effets partagés (réverb/delay/echo globaux), ducking lié ;
      logique `loom-audio` + bouche dans le mixeur GUI.
      — `send:` accorde N returns nommés {echo, flange, reverb}, `| send NOM R`
      route une voix vers un retour (wet seule, dry intact, settles > retour silencieux
      jamais gelé) ; lo-es routés par indice pré-résolu à la création de la note ;
      écho retour tempo-synchro (re-dimensionné à la barre) avec LP de feedback 3200 Hz,
      flange LFO, réverb room par return ; fader wet vivant (`set_send_level` master →
      séquenceur → moteur → mixeur GUI, strips « RETOURS ») ; JS : `buildSendReturn`
      (graphe par retour dans `buildGraph`, routage dans `connectFx`) + `parseSends`
      exporté (harness node + parité) ; corpus parité `send returns (no-op onsets)`
      valide 2 moteurs ; portes 361✅ clippy 0✅ parité 23/23+254/254✅ check_web 4/4✅.
- [x] **P4 · Composition algorithmique dans la langue** — la langue qui grandit :
      verbes `l-sys` (L-systèmes personnalisables), `xys` (marche de Markov à la
      Xenakis), `evolve` (génétique) ; `loom-core` + tests + parité 2 moteurs + doc.
  - [x] Rust : `expand_lsys_token` (custom `l-sys(n,axiome,sym>prod.…)` + `lsys(n)`
        rétrocompatible), `expand_xys_token` (états absorbants, transitions à poids
        `sym>a:p|b:q`), `expand_evolve_token` (GA : fitness entière contour+tonique/quinte+
        variété, élite, crossover 1-pt, mutation 20 %, tie = dernier max) ; `rng_seq`
        compteur strictement croissant (parité par construction) ; câblés dans `effective_of`.
  - [x] Tests : `lsys_is_backwards_compatible_and_custom`, `xys_is_seeded_and_weighted`,
        `evolve_is_seeded_and_improves` (123 tests loom-core, verts).
  - [x] JS miroir : `expandLsys`/`expandXys`/`expandEvolve` + chaîne d'expansion +
        exports `window.__loom` ; `expandSolve` restauré verbatim depuis git HEAD.
  - [x] Parité : 3 pièces ajoutées (`l-sys custom`, `xys xenakis`, `evolve`) → 278/278 ✅
        (+ analyse 23/23), check_web 4/4, clippy 0, WASM rebuildé.
  - [x] Doc : tables web/lessons (Génératif maths), aide #2, VIBE (RHYTHM/AVOID REPS),
        develop.rs `VARYING_GENERATORS` élargi (11).
- [x] **P5 · Base de connaissance & explicatriceur** — étendre `LORE`/`MOOD` :  commit `22c8bef`
      ethos des modes grecs, wuxing↔notes, talas, histoire par tradition ;
      explicatriceur « pourquoi ça sonne » + prof interactif ; miroir JS + parité.
  - [x] Connaissance (source = `web/index.html`, miroir Rust `lore.rs` maintenu à la main) :
        `LORE` + 6 concepts — `ethos` (le caractère moral des modes grecs), `tala` (cycles
        indiens : tintal 16 / ektaal 12 / jhaptal 10 / rupak 7 / keherwa 8 / dadra 6 ; sam &
        khali), `rasa` (les 9 saveurs du raga), `sam`, `khali`, `pakad` ;
        `TRADITION_LORE` + `tala` (4 lignes : origine Natyashastra/tabla · mécanique sam/khali
        · pourquoi ça sonne (points d'appui réguliers-irréguliers) · En LOOM sur 16 pas) ;
        enrichir `modes` (ethos des 7 modes d'après les Grecs) et `wuxing` (notes ↔ éléments :
        gong=Terre, shang=Métal, jue=Bois, zhi=Feu, yu=Eau) ;
        `MOOD` + ethos des 7 modes (major=ionien, minor=éolien, dorian, phrygian, lydian,
        mixolydian, locrian) → l'analyse affiche `humeur : …` aussi sur les pièces modales.
  - [x] Explicatriceur : clause `[ pourquoi ça sonne ]` « c'est un MODE : l'ambiance vient de
        sa couleur (ethos), pas d'une cadence V→I » pour dorian/phrygian/lydian/mixolydian/
        locrian — MÊME libellé JS (web) et Rust (analyze.rs) ; parité en ligne comme garantie.
  - [x] Prof interactif (web) : 4 talas ajoutés à `RHYTHMS`/`EAR_RH` (tintal16, jhaptal10,
        keherwa8, rupak7) → l'EAR-TRAINING sait désormais « devine le rythme » indien ;
        `?tala` répond via TRADITION_LORE (aucun code d'acheminement à changer).
  - [x] Miroir : `loom-core/src/prof/lore.rs` (std-only) + tests `lookup` offline dans
        `loom-app/src/prof.rs` (?tala, ?ethos, ?rasa, ?pakad, ?sam) + test analyse « humeur »
        d'une pièce modale (dorian) dans analyze.rs.
  - [x] Parité : + pièce corpus « mode lydien (ethos) » → ANALYSE 24/24 ; vérifier 278/278
        timing + conséquence : REBUILD WASM (`./build-wasm.sh`) obligatoire (lore.rs → analyse).
  - [x] Docs : invite `?tala`/`?wuxing` dans le message du prof (web + miroir `HELP`),
        leçon 10 (tala), ROADMAP §« Base élargie (P5) » ; develop.rs inchangé (pas de verbe neuf).
  - [x] Portes : cargo test --workspace (114/77/124/26/24) · clippy -D 0 · check_web 4/4 ·
        parité **analyse 24/24** + timing **278/278** · diff --check · WASM rebuildé (`_bg.wasm`
        web + node) · COMMIT + PUSH FR.
- [ ] (P6 hors vague) moteur unifié synthèse/FX WASM — chantier séparé à cadrer.
- [ ] Relecture GL live de T5 (gbloom/gglitch/gfeedback) sur le Mac de CI.

## Session 2026-09-25 — « tout » : consolidation du moteur, explicateur prof, studio (samples · mixeur · GPU)

> Décision du propriétaire : « je veux tout fini, fait tout » — on exécute le menu
> dans l'ordre d'effort croissant ; chaque piste = incrément testé + commité.

- [x] **T0 · Docs en vérité** — ROADMAP/README/CLAUDE cohérents avec la réalité
      (analyse portée ✅, clippy strict ✅, parité 238/238 + analyse 14/14, CI = Mac via Claude,
      pistes futures relabellées).
- [x] **T1 · Consolidation du moteur unifié**
  - [x] B1 : `tasks/ci-mac.md` — note de passage pour la CI du Mac (commandes exactes :
        `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
        `node tools/check_web.js`, `node tools/parity_test.js`, `git diff --check`).
  - [x] B2 : repli JS — retirer la fuite d'état `tuning` dans `analyzePiece` (l'acoustique doit
        dépendre du texte, pas du global ; vérifié par la parité).
  - [x] B3 : corpus de parité analyse enrichi (gamelan pelog/slendro, clave/samba, 17-EDO,
        raga aroha/avaroha…) → battery vert (20/20).
- [x] **T2 · Explicateur / prof enrichi** (`loom-core::prof` + repli JS synchronisé + parité)
  - [x] tension/détente & cadence (fonction tonale : repose/instable) + test.
        → ligne `[ tension ]` après `[ harmonie ]` (même scan déterministe des deux côtés).
  - [x] attente : note de tête, demi-pas résolutif vers la tonique + test.
        → ligne `[ attente ]` (7 suspendu qui appelle la tonique / 7→1 résolu) via `key.scale`.
  - [x] conseils de compositeur actionnables (ambitus étroit, dominante manquante, suspens sur V)
        + test → lignes `[ conseil ]` après le « pourquoi ».
  - [x] culture : humeur maqâm / rasa raga / ethos des modes (lore) + test.
        → `MOOD` (33 entrées) + ligne `   humeur : …` sous la tradition.
  - Tout ce qui précède est MIRROIR à l'identique dans `web/index.html` (2 moteurs == 1
    professeur) et verrouillé par la parité : ANALYSE 23/23 + onsets 238/238.
- [x] **T3 · Sampler instrument — ses propres fichiers** — banque mutable en session
      (`loom-audio` : `load_user_sample`, `reload_bank`, `merged_bank`) + navette de fichiers
      dans le panneau SAMPLER (« MES FICHIERS WAV (disque) » : chemin éditable, `< up`,
      dossiers cliquables, `.wav` cliquable = chargé + armé) ; la boucle = fenêtre
      begin/end + `| chop`/`| grain` déjà en place ; test `user_wav_enters_the_merged_bank`
      (wav PCM16 écrit à la main, fichier cassé refusé, conservation après `reload_bank`),
      vérifié en fenêtre (OCR : `ma_boucle`, `mon_chant`, `< up`, `recharger (N session)`).
- [x] **T4 · Mixeur — VU-mètres + grips couleurs** — marqueur de crête (peak-hold) par piste
      (`Strip.hold`, chute plus lente que le mètre, testé) ; grips fader/pan teintés de la
      couleur de la voix (`colored_grip`, scoped `Visuals` restaurés), grip MASTER en accent ;
      mètres déjà live (`update_meters` ← `engine.voice_levels()`). Vérifié en fenêtre par
      analyse de pixels (blocs de teinte saturée sur le fader de chaque piste, ≥5 teintes).
- [x] **T5 · Post-traitement GPU (web)** — pipeline WebGL multi-passes ajoutée au
      visualiseur fragment `uMode` : 3 nouvelles scènes `gbloom`/`gglitch`/`gfeedback`
      (nouveaux shaders 10/11/12) avec post FX : bloom (extraction des brillances + flou
      gaussien quart-résolution réinjecté), glitch (tranches décalées + séparation RGB +)
      parasite scan), feedback (traînées ≪ précédente trame, ping-pong 2 textures pour
      éviter la lecture/écriture simultanée). Les 3 shaders compilent en GLSL ES 1.00
      (glslangValidator) ; `check_web.js` 4/4 ; parse JS OK ; parité intacte ;
      retombée sans post si WebGL limité.
- [x] Toutes les portes finales (tests workspace 356, clippy -D, check_web 4/4, parité
      23/23 + 238/238, diff --check) — vertes ; commits poussés ; Review ci-dessous.
  Limite documentée (non vérifiable ici, mono-écran sans carte GL affichable) : le rendu
  live des 3 scènes GPU post-traitées à relire une fois sur la machine de CI (Mac) —
  plein écran + N jusqu'à gbloom/gglitch/gfeedback.

## Review — « tout fini » : les 6 pistes (T0→T5) embarquées en une session

**Menu complet : consolidation (T1), prof enrichi (T2), sampler (T3), mixeur (T4),
post-traitement GPU web (T5), docs (T0) — chaque incrément testé + commité + poussé.**

- **T1 · consolidation** (`8c1eb85`) : note CI Mac (`tasks/ci-mac.md`) avec les 5 commandes
  de garde ; repli JS `analyzePiece` sans fuite d'état (`tuning` sauvé/restauré) ; corpus de
  parité +7 pièces → ANALYSE 20/20 + onsets 238/238.
- **T2 · prof enrichi** (`1d91183`) : `MOOD` (33), `tension_summary`/`last_voice_degree_tokens`/
  `degree_semitone`, 4 lignes neuves (humeur/tension/attente/conseil) miroitées dans le JS →
  parité ANALYSE 23/23 (2 moteurs == 1 professeur).
- **T3 · sampler** (`07aeb26`) : `SessionBank` (frais + session) derrière `OnceLock<Mutex>` dans
  `loom-audio` (`load_user_sample`, `reload_bank` garde la session, `merged_bank`) ; navette de
  fichiers dans le panneau SAMPLER (« MES FICHIERS WAV », chemin éditable, `< up`, recharger) ;
  WAV utilisateur joué dès le prochain coup ; vérifié en fenêtre par OCR.
- **T4 · mixeur** (`fa5e4f4`) : crête par piste (`Strip.hold`, chute 0.925 vs mètre 0.85, testée) ;
  grips fader/pan teintés de la couleur de la voix (`colored_grip`, Visuals scoped restaurés) ;
  vérifié par analyse de pixels (≥5 teintes saturées dans la bande MIX).
- **T5 · post GPU web** (`d0d3da2`) : pipeline WebGL multi-passes (scène→FBO→post→écran) ; 3
  scènes neuves `gbloom`/`gglitch`/`gfeedback` (shaders 10/11/12) ; post bloom (brillance +
  flou gaussien ¼ de résolution), glitch (tranches + RGB split + scan), feedback (ping-pong
  2 textures, pas de lecture/écriture simultanée) ; FBO toujours sous try/catch avec retombée
  sur le rendu direct si WebGL limité ; shaders compilés en GLSL ES 1.00 (glslangValidator).
- **Portes finales** : workspace 356 ok · clippy `-D warnings` 0 · check_web 4/4 · parité
  23/23 + 238/238 PERFECT · `git diff --check` propre · branches de backup conservées
  (`backup-avant-workflow-drop`), `.github/workflows/` toujours absent (scope workflow refusé).

## Session 2026-09-25 — analyseur du prof dans loom-core (clé de voûte du moteur unifié)

> Décision du propriétaire (2026-09-25) : « y » — on lance la vague.
> Objectif : le prof hors-ligne (`analyze.rs` + `lore.rs`) vit dans `loom-core`
> (zéro dépendance, std-only), est ré-exporté par `loom-app` (TUI/GUI inchangées)
> et exposé au web via `loom-wasm` (repli JS instantané comme le séquenceur).

- [x] Déplacer `lore` + `analyze` dans `loom-core` (module `prof`), `cargo test -p loom-core` vert, std-only.
- [x] Ré-exporter depuis `loom-app` (`pub use loom_core::prof::analyze`) : TUI/GUI identiques.
- [x] Exposer `analyze_json` dans `loom-wasm` (wasm-bindgen), reconstruire le WASM.
- [x] Brancher `web/index.html` sur l'analyse WASM (repli JS instantané, même schéma que le séquenceur).
- [x] Parité : cas JS↔Rust de l'analyse (tonalité, harmonie, novelty) ajoutés à `tools/parity_test.js`.
- [x] Docs (CLAUDE/README/todo) à jour + toutes les portes (tests, clippy -D, check_web, parité).
- [x] Review finale des changements et limites.

### Review
- **Le port** : `loom-app/src/prof/{analyze,lore}.rs` → `loom-core/src/prof/` (module `prof`, std-only,
  zéro dépendance) via `git mv` ; `loom-core` ré-exporte `analyze`, `novelty_report`,
  `LORE`, `SCALE_LORE`, `TRADITION_LORE`. `loom-app/src/prof.rs` fait `pub use loom_core::prof::analyze;`
  (et `lore`, `SCALE_LORE`, `TRADITION_LORE`) → TUI/GUI/exercices inchangés : `cargo test` app 114 vert.
  `loom-wasm` expose `#[wasm_bindgen] analyze_json(src) -> Vec<String>` (même patron que `schedule_bar_json`),
  reconstruit par `./build-wasm.sh` (web/pkg + tools/pkg-node).
- **Web branché** : `window.__loomRust.analyze` = `w.analyze_json` ; `analyzePiece()` prend l'analyse Rust par
  défaut (badge 🦀), repli silencieux instantané sur le moteur JS — <u>exactement</u> le schéma du séquenceur.
- **Portes** : `cargo test --workspace` 348+12 verts (core 113 dont +12 tests analyseur, app 114, audio 73, gui 26,
  tui 24), `cargo clippy --workspace --all-targets -- -D warnings` 0 warning, `check_web.js` 4/4 ✓,
  `parity_test.js` : analyse 14/14 ✅ + onsets 238/238 PERFECT, `git diff --check` propre.
- **3 vraies divergences corrrigées** (port) : (1) nouveauté sans `song:` — le web prend 8 mesures
  nominales, Rust `len_bars().max(1)` échantillonnait deux fois la même mesure → repli à 8 dans
  `novelty_report` (+ test régression : `kick: x x x x x x x` est bien « boucle à l'identique », `bell` non) ;
  (2) la ligne « pourquoi » « le tempo accélère/respire … » (`parse_accel`) manquait à Rust → ajoutée
  (ordre : drône→bend→générateurs→euclid→harmonie→swing→métrique→accel) (+ test) ; (3) écriture des
  métriques — Rust décline « métrique composée 2+2+3 (3,5 temps) » vs le « inhabituelle » du web → le
  repli JS reprend l'écriture Rust (`parseMeterGroups` + `beautifyBeats`, virgule française) pour coller au texte.
- **2 normalisations documentées** dans le test de parité (pas des bugs) : le % de consonance
  Plomp–Levelt (JS flotte en f64 avec `tuning` global stateful vs Rust stateless — 59/60 possible) → le
  nombre est normalisé, l'étiquette reste stricte ; et sur les pièces à `key@section`, Rust empreinte
  des notes RÉSOLUES (il entend la modulation vers le chorus), JS des jetons d'étapes abstraits → la
  section nouveauté n'y est pas comparable ligne à ligne par conception : le test compare les verrous
  déterministes (tonalité, accordage, étiquette acoustique, harmonie + cadence, forme, « pourquoi »)
  strictement, et ne garde de la nouveauté que ce qui est stable.
- **CI : décision du propriétaire (2026-09-25)** — la CI se fait sur son autre Mac via Claude (pas sur
  GitHub Actions). Le workflow `.github/workflows/ci.yml` a donc été <u>retiré de la branche</u> (2 commits
  CI ôtés de l'historique, sauvegardés dans `backup-avant-workflow-drop`) pour que la push passe sans le
  scope `workflow` (GitHub refuse tout push qui crée/modifie `.github/workflows/` sans ce scope). Les
  gardes restent documentées ici et locales ; la chaîne WASM est reconstruite en local par
  `./build-wasm.sh`. Ne pas recréer de workflow GitHub sans accord explicite du propriétaire.
- **Limites notées** : (1) le repli JS de l'analyse reste l'ancien texte « analyseur » pour le cas où le
  WASM n'est pas chargé — aligné sur Rust pour les métriques, mais sa nouveauté garde l'empreinte par
  jetons (différence voulue, cf. ci-dessus) ; (2) `analyzePiece` JS reste *stateful* (fuite de `tuning` via
  `noveltyReport`→`scheduleBarCapture`) — sans conséquence maintenant que Rust est la valeur par défaut,
  et à réviser si le repli JS devait redevenir principal ; (3) la parité analyse couvre un corpus de
  14 pièces (majeur/pythagorien/II-V-I, maqâm, microtonal, métriques simple et composée, accel,
  raga+forme, 19-EDO, romanes+modulation fusionnée, blues, générateur walk, modulations) — étendre le
  corpus si un cas d'école apparaît ailleurs.

## Session 2026-09-25 — qualité et vérification de l'interface
- [x] Nettoyer les avertissements Clippy du workspace sans changer le comportement musical.
- [x] Vérifier le sampler GUI dans une fenêtre, puis corriger les problèmes observés.
- [x] Relancer build, tests, Clippy strict et parité web ; noter les résultats.
- [x] Faire une Review finale des changements et limites.

### Review
- **Clippy 0 warning** sur tout le workspace (`cargo clippy --workspace --all-targets -- -D warnings`),
  y compris les 8 qui bloquaient `loom-gui` et les ~16 dans `loom-audio`/`loom-app`/`loom-core`/`loom-tui`.
  Toujours sans changer le comportement : la grande majorité sont du style (
  `map_or(→is_none_or/is_some_and`, `then(→then_some`, `.sort_by`→`sort_by_key(Reverse)`, `Noise::next`→`Noise::sample`
  (renommage pur), `#![allow]` ciblés avec justification en commentaire). `git diff --check` : propre.
- **Sampler vérifié dans une fenêtre** (hyprland, fenêtre flottante 1200×800 = taille par défaut de l'app) :
  le panneau ancré à droite rend toutes les sections — en-tête (+ « voix … » armée + réinit), chips des voix,
  liste du `default_bank()` avec durées, forme d'onde (± marqueurs begin/end), racine, reverse, begin/end/chop,
  APPLIQUER. Aucun crash, l'app reste fluide. Le correctif du problème observé (panneau 170 px du bas qui
  compactait tout) est déjà la version qu'on a : panneau droit défilable.
- **Portes toutes vertes** : `cargo test --workspace` 348 ok (app 124 · audio 73 · core 101 · gui 26 · tui 24,
  ni 4 ignorés), `node tools/check_web.js` 4/4 ✓, `node tools/parity_test.js` 238/238 PERFECT.
- **T3 · navette de fichiers vérifiée en fenêtre** (OCR sur `/tmp/loompick` avec deux WAV
  « ma_boucle.wav » / « mon_chant.wav » vendus via patch temporaire non committé) : le panneau
  SAMPLER affiche « MES FICHIERS WAV (disque) », le chemin du dossier, `< up`, les lignes
  `ma_boucle` / `mon_chant` (⊕ = dossiers en accent), `recharger (N session)` et APPLIQUER ;
  le panneau est défilable donc tout reste joignable à 800 px. Patchs temporaires (taille de
  fenêtre, ordre d'affichage) retirés avant commit.
- **Portes toutes vertes** (après T3) : `cargo test --workspace` 356 ok (audio 74 dont le test
  session-bank), clippy `-D warnings` sans warning, `check_web` 4/4, parité ANALYSE 23/23 + onsets
  238/238 (aucun changement analyser → pas de rebuild WASM), `git diff --check` propre.
- Limites notées : (1) le clic simulé (API hyprland lua, sans souris) ne permettant pas de dérouler jusqu'au
  choix de sample interactif dans la fenêtre, la sélection/APPLIQUER reste validée par les tests
  `sampletool` (réécriture des verbes, undo partagé) ; (2) en fenêtre étroite (« tuilée » à 627 px), le panneau
  droit à 320 px par défaut mange la largeur du centre — acceptable (panneau défilable), à réévaluer si
  l'app est surtout utilisée en demi-écran.

## Session 2026-09-25 — stabiliser le sampler GUI
- [x] Terminer le panneau sampler latéral déjà commencé : largeur, contrôles et bouton APPLIQUER.
- [x] Vérifier la compilation, les tests workspace et les contrôles web/parité.
- [x] Corriger les indications de statut périmées dans README, AGENTS et CLAUDE.
- [x] Examiner le diff final et noter les limites dans une Review.

### Review
- Le panneau latéral reprend le brouillon en cours : liste d'échantillons, forme d'onde,
  racine, reverse, begin/end/chop et APPLIQUER. Défilement du panneau complet pour
  garder les contrôles accessibles dans une petite fenêtre.
- `cargo test --workspace` : 348 réussis, 4 ignorés ; `cargo test -p loom-gui` :
  26 réussis ; `node tools/check_web.js` : 4 contrôles réussis ; parité : 238/238.
  `git diff --check` : propre.
- Limites : affichage en fenêtre non vérifié manuellement ; `cargo clippy -p loom-gui
  -- -D warnings` s'arrête sur 8 avertissements déjà présents dans `loom-core`.

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

## Session 2026-09-25 — P6 (cadré) : moteur unifié synthèse/FX WASM — P6-1 livré

> « ok pour tout » sur P6, chantier séparé à cadrer. Vue d'architecture après
> reconnaissance : `loom-core/src/synth.rs` n'est qu'un **descripteur** (parse des
> `| verbes`), le rendu vit dans `loom-audio` (`tone.rs`, `master.rs`, `dsp.rs`…)
> côté natif et dans les nœuds WebAudio/`tone()` côté web. P6 converge vers **un
> moteur de rendu PCM déterministe en `loom-core` (std-only), exposé par
> `loom-wasm`**, avec miroir JS + parité numérique — exactement le patron déjà
> éprouvé pour l'analyseur et le scheduling. Découpage en 4 pierres :

- [x] **P6-2** · master FX (warmth/duck/limiter) + envois (echo/reverb
  fdn/flanger/phaser) + normalize/limit + export WAV — portés depuis `loom-audio`.
- [ ] **P6-3 (futur)** · sources riches (wt/partials/fm/pluck/bow/reed/bell/grain/
  sync/pwm/noise), LFO/LFO2 (cutoff/pitch/pan/gain), inserts (drive/fold/crush/
  redux), drums synthétisés (kick/hat/snare/perc), stéréo + pan law, `slide`/
  `bend`/`glide` — vérifiés pierre par pierre (parité numérique croissante).
- [ ] **P6-4 (futur)** · brancher le moteur WASM derrière un AudioWorklet pour le
  playback **live** (remplacer la chaîne `tone()`/`buildGraph`) + A/B toggle.

- [x] **P6-1 · Noyau de rendu PCM déterministe (voix mélodiques, mono)** — cette
  session. `loom-core/src/render.rs` (neuf, std-only, sans dépendance) rend
  `render_program(src, bars, sr) -> Vec<f32>` : il consomme **exactement les
  événements du scheduler Rust** (`Program::parse` + `schedule_bar`, donc la même
  clé/microtonalité/`key@section` que le moteur) et tire chaque note comme un
  instrument. V1 couvre : oscillateurs sine/square/saw/triangle, `| unison N` +
  `| detune`, `| sub`, `| double`, filtre statique ou balayé (lp/hp/bp/notch, q),
  `| env ADSR` × `| gate`, amplitude = vel × gain. Hors v1 (ignorés,
  documentés P6-2/P6-3) : wt/partials/fm/noise/pluck/bow/reed/bell/grain/sync/pwm/
  slide/bend/wow/lfo/inserts/drums/pan(FX stéréo)/sends. Fidélité : `schedule_bar_json`
  gagne `dur` + `synth` (sous-ensemble v1) → **le miroir JS consomme le même flux
  d'événements Rust** = parité de DSP pure (scheduling déjà prouvé bit à bit).
  Miroir JS `renderPCM` dans `web/index.html` (Float32Array + `Math.fround` sur
  les états) ; `tools/parity_test.js` gagne une section RENDER (max diff + taux de
  samples dans une tolérance, corpus dédié 9 pièces × 2 mesures × 44100 Hz).
  Exposé WASM : `render_json(src, bars, sr) -> Float32Array`.

→ **P6-1 FAIT, commit `b15ae95`** : `loom-core/src/render.rs` (10 tests) —
  parité RENDER **9/9 pièces, worst diff 7.75e-7** (3 ordres de grandeur sous le
  seuil) grâce à la discipline `Math.fround` op-pour-op (seules tan/sin/pow font
  1 ulp). Grille : workspace 114/77/134/26/24 · clippy -D 0 · check_web 4/4 ·
  parité 9/9 + 24/24 + 278/278 · diff --check clean. Cette fois la glue
  `loom_wasm.js` **change** (+36 lignes) car `render_json` ajoute un export WASM
  (le cas P5 ne touchait que le `.wasm`). `web/index.html` expose
  `window.__loom.renderPCM` (miroir) + JSynth `dur` dans `schedule_bar_json` ;
  `loom-wasm` sérialise le sous-ensemble v1 du `Synth` résolu sur chaque
  événement → les deux DSP consomment **un seul et même flux d'événements Rust**.

---

## Review — P6-1 (moteur unifié, pierre 1)

- **Fait** : le « son » a maintenant un moteur déterministe std-only dans
  `loom-core` (rendu PCM mono, f32), exporté en WASM (`render_json`) et miroité
  en JS (`renderPCM`) avec **parité numérique vérifiée sur 9 pièces × 2 mesures ×
  44100 Hz** (worst 7.75e-7). Même patron que l'analyseur et le scheduling :
  Rust = autorité, JS = repli/frein, harnais de parité en garde-fou.
- **Levier d'architecture** : `schedule_bar_json` porte maintenant `dur` + le
  sous-ensemble v1 du `Synth` (résolu : défauts appliqués, LFO aplati) — le
  miroir JS ne réinterprète plus jamais les `|` verbes. C'est la fondation des
  pierres P6-2 (master FX/envois/WAV) et P6-3 (wt/fm/pluck/drums/inserts).
- **Honnêteté** : v1 = voix mélodiques mono, 9 features de source/env (pas les
  70 champs du `Synth`) ; drums, percussions, wt/fm/noise/pluck/… documentés
  comme P6-2/P6-3. Aucun changement du playback live (toujours WebAudio JS).
- **Côté CI/Mac** : relecture audio à l'oreille impossible en local (pas de
  sortie audio dans le harnais) — la parité numérique est la preuve de
  cohérence. Un `ffmpeg` peut démontrer le PCM (spectre) si besoin.
- **Rappel** : tout changement de `loom-core` → `./build-wasm.sh` + recommit des
  `_bg.wasm` (et de la glue quand un export change).

## Session 2026-09-26 — P6-2 (cadré) : master FX + envois + normalize/WAV

> La chaîne native (`loom-audio/src/master.rs` + `master/{dynamics,reverb}.rs` +
> `inserts/space.rs` + `inserts/color.rs`) est riche et stéréo/f64. P6-2 l'expose
> en **mono déterministe** dans `loom-core` (std-only) avec les **topologies
> natives exactes** : chaque brique est un port op-pour-op (f32) de la version
> mono des mêmes algorithmes → miroir JS `Math.fround`, parité échantillon par
> échantillon. Le grain de l'entrée est LE note engine de P6-1 (factorisé en
> `NoteCtx`/`note_sample`, partagé entre `render_program` et le nouveau chemin).

### Modèle du pipeline `render_full(src, bars, sr) -> Vec<f32>` (2 phases)

Phase 1 (dépôts, par événement via `schedule_bar`) : pour chaque note, `v =
note_sample(...)` (identique P6-1) ; `dry[ix] += v`, puis inserts note :
- `| echo DIV FB` (retard feedback tempo-sync, ligne par note) : `y=ring.out()`
  `ring.push(undenormal(0.45·v + fb·y))`, wet `fx[ix]+=y` (ligne `ring` par note,
  taille `round(clamp(barSec/div,0.01,2)·sr)`, fb clamp 0..0.95, `n` étendu de la
  queue `Echo::tail`).
- `| flange RATE DEPTH` (128-ring balayé, 0.5 fb, `(x+dl)·0.6`) : remplace le sec
  `fx += out − v`.
- `| phaser N` (4 allpass RBJ 400+350k ±900·sin, Q 0.7, refresh/32) : ajoute le
  chaînage `fx += chainAP(v)` (le sec reste).
- `| reverb N` → `rev_in[ix] += v` (taux plein, comme le câblage natif).
- `| delay N` → `echo_in[ix] += v·N` (les `| reverb`/`| delay` restent au niveau
  du voile sauf retard).

Phase 2 (mix+master) : `bus = dry + fx` ; `rev_in += bus` ; `echo_in += bus`
(le bus alimente TOUJOURS les deux retours, câblage natif) ; passe unique :
1. `space = FDN8.process(rev_in[i])` (lignes FDN_TIMES ×8, lp par ligne damp
   `room: fdn DAMP` défaut 4200, decay défaut 0.84 clamp 0..0.97, FWH·1/√8,
   2 allpass 0.0047/0.0019 @0.5) ; `echo = ping.process(echo_in[i])` (ligne
   mono 0.2 s, fb 0.32, len `max(Q,round(0.2·sr))`).
2. `mix = bus·duck_gain + echo·0.15 + space·0.18` (0.18 = SPACE_SEND, 0.15 =
   ECHO_SEND natifs). `duck:` : chaque événement batterie pompe
   `duck_gain = max(0.05, 1−duck)`, glissade 1-pôle vers 1 avec
   `τ = min(0.12, bar/8)` (coef par bar).
3. mastering : `x = sat(x, warmth)` (LoomSat EXACT : `g=1+4w`, `comp=1/(1+0.6w)`,
   mid=(prev+x)·0.5, `((mid·g)T + (x·g)T)·0.5·comp`) → `x = lim_brick(x)`
   (LoomLim EXACT : lookahead 128, attaque instantanée, release
   `(1−0.0009)^(48000/sr)`, `min(need[])`, plafond 0.97) → `clamp(−1,1)`.
   Glue comp / EQ maître non portés (documenté honnêtement).
4. Queue : buffer étendu de `FX_TAIL = 2.0 s` (queue FDN/echo capturée).

### Exports & miroirs
- loom-core : `render_full` (ci-dessus) + `render_wav(src,bars,sr) -> Vec<u8>`
  (16 bit mono : normalize à 0.97 si crete >0 puis encodage i16, entête WAV 44 B
  RIFF/fmt PCM mono).
- loom-wasm : `render_fx_json(src,bars,sr)`, `mix_json(src)` (`{duck,warmth,
  room:{kind,decay,damp}}` — l'autorité reste Rust), `render_wav(src,bars,sr)` ;
  `SynthV1` étendu (`echo{div,fb}`, `flange{rate,depth}`, `phaser`, `reverb`,
  `delay`).
- web : miroirs `renderFX(evsArr,barSecs,sr,mix)` + `wavBytes(pcm,sr)` dans
  `window.__loom` (le note engine JS est factorisé et partagé avec `renderPCM`).
- parity_test : section RENDER2 (chemin FX complet) sur 9 pièces × 2 mesures ×
  44100, mix Rust autorité ; seuils calibrés sur les diffs réels.

### Portes
`cargo test --workspace` · clippy -D · check_web 4/4 · parity (timing+analyse+
RENDER+RENDER2) · `render_program`/`renderPCM` inchangés (parité P6-1 intacte) ·
WAV vérifié (entête + longueur + décodage) · `./build-wasm.sh` (loom-core touché) ·
`git diff --check` · commit + push.

→ **P6-2 FAIT** : `loom-core/src/render_fx.rs` (12 tests, std-only, 800+ lignes) +
refactor `render.rs` → `NoteCtx`/`NoteParam`/`note_sample` partagé (14 tests) +
exports WASM (`render_fx_json`, `render_wav`, `mix_json`, `SynthV1` étendu).
Miroir JS : bloc DSP partagé (`F/TAU/wave/svf/biquad/allpass/ring/duckCoef`) +
`noteSample` (factorisé de l'ancien renderPCM, parité P6-1 **intacte**) +
`renderFX` (dépôts dry/rev/ech + inserts note + master duck/ping/verb/sat/lim) +
`wavBytes` (en-tête 44 B + norm 0.97 + i16 half-away-from-zero — le bug
`bytes.set` sans offset corrompait l'en-tête, corrigé).
Grille : workspace 114/77/144/26/24 · clippy -D 0 · check_web 4/4 · parité
RENDER 9/9 (worst 7.75e-7) + RENDER2 **10/10 (worst 2.5e-7)** + WAV 10/10
(toléré ≤1 step int16, 1 ulp de tanh/exp) + ANALYSE 24/24 + onsets 278/278 ·
diff --check clean. WAV natif rejoué par ffprobe (pcm_s16le mono 44,1 kHz,
6,0 s = 4+tail).

## Review — P6-2 (maitre FX, pierre 2)
- **Fait** : le pipeline FX complet (envois note + master) est porté en mono
  déterministe dans `loom-core` (rendu `render_full`), exporté WASM
  (`render_fx_json`/`render_wav`) et miroité JS (`renderFX`/`wavBytes`) avec
  parité échantillon par échantillon 10/10 (worst 2.5e-7) et WAV identique à
  ≤1 step int16 près. Topologies natives mono conservées (sauf ping-pong : une
  ligne mono au lieu de 2 croisées L/R — documenté) ; constantes natives
  (`SPACE_SEND=0.18`, `ECHO_SEND=0.15`, Q `1/√2`, lookahead 128, plafond 0.97).
- **Levier** : le note engine JS est désormais UNE seule fonction `noteSample`
  partagée par `renderPCM` et `renderFX` (factorisation op-pour-op vérifiée par
  re-run des 134 tests avant ajout FX, puis parité P6-1 à l'identique).
- **Honnêteté** : glue/EQ maître natifs non portés ; reverb Schroeder par défaut
  vs FDN si `room: fdn DAMP DECAY` (parité sur les deux) ; `room_kind`/`mix_json`
  = l'autorité Rust résolue consommée par le miroir (jamais les `|` verbes).
- **Côté CI/Mac** : relecture audio à l'oreille impossible en local — la parité
  numérique + ffprobe (durée/format valides) sont la preuve.

## Session 2026-09-27 — P6-3 (cadré) : sources riches + LFO + inserts + drums + stéréo

> Le `Synth` (synth.rs) parse DÉJÀ les 70+ verbes de sound-design (héritage du
> moteur browser) ; le moteur déterministe std-only n'en rend qu'un sous-ensemble
> (P6-1 sources sine/sq/saw/tri + uni/det/sub/double + SVF ; P6-2 FX master).
> P6-3 = élargir **le rendu** pierre par pierre, chaque pierre étant un bloc
> `Synth` + serialization `SynthV1` (loom-wasm) + miroir JS `noteSample` +
> corpus de parité. Rust = autorité (algorithme exact défini ici), JS = repli.
> Ordre d'effort croissant, chaque pierre committée/poussée seule.

### [x] Pierre A — sources riches (niveau oscillateur + couches) + inserts note ✅ (03/10/2026)
Remplacé le noyau `wave(w,p)`+sub/double par un **générateur riche** dans
`note_sample` (Rust autorité, miroir JS op-pour-op `Math.fround`) :
- `| fm RATIO INDEX FB` — FM : modulateur `fm_ph` à `ratio·f0`, feedback
  `m = sin_pi(fm_ph) + fb·prev` (fb 0..8), phase porteuse `car += (f0+INDEX·m)/sr`,
  mod avancé en dernier (`wrap`).
- `| partials a1…` / `| organ` — additif `Σ a_k·sin_pi((k+1)·p)` (pas de norm).
- `| wt organ|warm|rich|soft` — tables déterministes (sommes d'harmoniques à
  profils fixes, divisor = Σa pour ~1 de crête).
- `| pwm N` — square à largeur modulée `pw = 0.12+N·(0.5+0.5·sin_pi(pwm_ph))`,
  `pwm_ph += (f0/4)/sr`.
- `| sync RATIO` — hard sync : esclave avance à `f0·ratio`, se resynchronise sur
  le wrap de la porteuse (`slave = mnew` si `mnew < ph`), out = wave(slave).
- `| bell` — modal : 4 modes `r=[1,1.96,2.92,4.2]`, `a=[1,.6,.3,.15]`,
  `τ=[1.5,1.2,1,.8]`, `Σ a·exp(−t/τ)·sin_pi(f0·r·t)`.
- `| noise N` — bruit blanc HP (1-pôle ~6 kHz) via PRNG **xorshift32** par note
  (seed déterministe), `white = (ns>>16)/32768·2−1` (haute précision → parité
  binaire exacte).
- `| am HZ` `unipolaire 0.5·(1+sin_pi(am_ph))` ; `| ring HZ` `sin_pi(ring_ph)`.
- inserts post-filtre : `| drive N` (tanh·g, g=1+4N, comp 1/(1+0.6N)), `| fold N`
  (wavefolder `1−|rem_euclid(x+1,4)−2|`, mix), `| crush BITS` (round·2^(b−1)),
  `| redux N` (sample-hold K=round(1+20N)).
- Sans `| …`, toutes les nouvelles surfaces = 0 → **rétro-compatible bit-à-bit**
  (les 144 tests existants doivent rester verts SANS changement).
- `| pluck` (Karplus-Strong, tampon par note) reporté en Pierre B (taille).
- Sérialisation `SynthV1` étendue : fm/partials/wt/pwm/sync/bell/noise/am/ring/
  drive/fold/crush/redux. Parité : corpus RENDER2" (9+ pièces, une par source).
- Review (03/10/2026) : miroir JS `noteSample` étendu op-pour-op (renderPCM et
  renderFX partagent `richSt`/`noteCtx`), wasm régénéré (wasm-bindgen 0.2.128),
  6 pièces RENDER2 couvrant les 13 sources/inserts. RENDER2 16/16, WAV 16/16.

### Pierre B — drums synthétisés (déterministes, mono)
- `Event.is_drum` + `voice` (kick/bd, snare/sd, hat/hh, clap/cp) → rendu :
  - kick : sin cp fréquence qui dégringole `f = f0·exp(−12t)` + click, env courte.
  - snare : bruit HP + corps ~180 Hz, env 0.15 s.
  - hat : bruit HP (fermé court / ouvert long via `gate`), env 0.05/0.3 s.
  - clap : 3 bruits groupés + bande 1 kHz.
- Les drums alimentent `dry` (et donc le duck, qui pompe sur un vRAI signal).
- `deposit_event` consomme le même rendu (extraire `drum_sample` partagé).
- Miroir JS `drumSample` + parité sur les lignes `kick:/hat:/snare:/clap:`.

### Pierre C — LFO réel (`| lfo` / `| lfo2`)
- Couper `Param::Signal` aplati dans `SynthV1.cutoff` : sérialiser `lfo
  {target,rate,depth,shape}` (rate en cycles par barre → Hz via barSec) ;
- LFO module cutoff (et gain/pitch à la pierre C-bis) — même forme sine/saw/tri/
  square/rand (PRNG partagé), phase par note, pas de wrap discontinu.
- `cutoff_span` rendu en `c0/c1/c_lfo` ; note_sample applique le LFO AVANT le SVF.

### Pierre D — stéréo + pan law (dernière pierre structurelle)
- Buffers 2 canaux L/R, `| pan N` (−1..1, law −3 dB centre), `| spread` des voix
  unison, haas/orbit documentés non portés ; rendu WAV stéréo 2 canaux ; exports
  `render_*` changent de forme → mis à jour miroirs + parity (RENDER2 3D).
- Reporté si la session plafonne (le mono reste la norme v1).

### Portes (chaque pierre)
Rust d'abord (tests unitaires par feature + rétro-compat 144 ok) → `SynthV1` +
`./build-wasm.sh` → miroir JS → parité (RENDER + RENDER2 + RENDER3 + analyse +
onsets, worst seuil 5e-3) → clippy -D · check_web 4/4 · `git diff --check` ·
commit + push · todo.md coché.

## Nuit 2026-10-03 — LOOM Frankenstein vers Ableton (branche `frankenstein`)

Plan : `~/.claude/plans/synthetic-splashing-frog.md`. Silence total (master Live à 0, sortie Mac coupée).

- [x] **A · base** : `p6-3a-wip` fusionnée, miroir JS des 13 sources riches (af2518d). Tests 406/406, check_web 4/4, parité RENDER 9/9, RENDER2 16/16, WAV 16/16, analyse 24/24, onsets 278/278.
- [x] **C1 · spike port MIDI « LOOM »** (`tools/midi_gigue.py`, horodatage CoreMIDI) :
  - le fil MIDI dort jusqu'à 3 ms avant l'échéance puis tourne sur l'horloge : P95 1,0 à 1,5 ms (avant : 2,6 à 3,2 ms) sur les passes saines.
  - boucle IAC corrigée : LOOM écrivait sur le bus IAC ET le relisait (ses propres notes rejouées en touches live, c'est le son entendu cette nuit).
  - `LOOM_AUDIO_DEVICE=BlackHole` choisit la sortie audio (pour nourrir Live).
  - ❌ ouvert : environ 1 passe sur 2, un décalage persistant d'exactement un pas (125,0 ms) apparaît en cours de route. Le séquenceur hors ligne est propre (test `midi_notes_survive_any_block_size`, 5 tailles de bloc), le rendu d'un bloc coûte 0,16 ms au pire. Cause à chercher côté fil audio temps réel / fil MIDI. Deux tentatives d'ancre d'horloge (WallClock) ont empiré : abandonnées.
  - Conclusion produit : le port natif suffit pour jouer, pas pour vendre. Le produit Live passe par l'horloge de Live (cuisson en clips, device M4L).
- [x] **C2 · `loom bake` + `tools/bake_live.py`** : le vrai séquenceur, hors ligne, voix par voix forcée en MIDI (accords, arpèges, humanize identiques à l'écoute), temps convertis en noires mesure par mesure. Live : une piste par voix, une scène par section nommée « section N BPM » (Live règle le tempo au lancement), accordage `.ascl` écrit dans User Library/Tunings. autechre.loom : 28 mesures, 11 pistes, 35 clips, 1614/1614 notes, 35/35 clips identiques en hauteur et position (relus dans Live). 4 tests Rust `bake` + 3 asserts CLI.
- [x] **C3 · device Max for Live « LOOM pour Live »** (`m4l/`, MIDI Effect). Spike : le `[v8]` de Max 9.1.5 (Live 12.4.6) exécute WebAssembly ; il manque TextEncoder/TextDecoder (polyfills dans le device). Moteur choisi : loom-core en WASM, le même que l'appli. `bake_json` exporté par loom-wasm (le séquenceur natif sans carte son), et la cuisson saute l'audio : 0,01 s au lieu de 1,1 s, sortie identique octet pour octet. Le device lit un `.loom`, cuit la voix de sa piste dans un clip (Live joue à l'échantillon près) et recuit à chaque sauvegarde. Vérifié dans Live : 234/234 notes identiques, puis édition du fichier → 117/117 en 39 ms. `node m4l/test_device.js` : 4/4. Un seul fichier .amxd (936 Ko, WASM en base64 dans un `v8.codebox`).
  - Chemin mémorisé avec le set (pattr « blob », stocké seulement) ; pattr vide (0) ignoré, « introuvable » dit une seule fois (test 5/5). Revérifié dans Live : 234 puis 117 notes.
  - ❗ Non vérifié sans interface : le glisser-déposer sur `live.drop` et la relecture du chemin à la réouverture du set. Le navigateur de Live n'indexe plus les nouveaux fichiers cette nuit (fseventsd saturé) : test fait en réutilisant un nom déjà indexé.
- [x] **C4 · lien éditeur web → Live** : bouton « 🎛 Live » (Chrome, Edge) qui lie un fichier `.loom` (File System Access) réécrit à chaque RUN ; le device le surveille et recuit. Aucun serveur. Les éditeurs natifs (`loom`, `loom-gui`) marchent déjà par Ctrl+S. `tools/runtime_test.js` réparé (chemin relatif, stub `setAttribute`) : moteur chargé, 41 ticks, 4052 oscillateurs, 0 erreur ; check_web OK ; parité 5/5 blocs.
  - Reporté : surlignage du token joué dans l'éditeur (il faut un retour device → éditeur, donc un pont réseau).
- [x] **C5 · paquet** : `python3 m4l/package.py` → `dist/LOOM-pour-Live/` (device, LISEZ-MOI FR/EN, `bake_live.py`, 6 exemples) + zip 387 Ko. Rien publié. Reste : un set de démo `.als` (il faut sauver depuis l'interface de Live).
- [x] **B4 · sûreté du live** : vérifié, le TUI (`app.rs:160`), le web (`run()` : « no valid voices ») et le device gardent l'ancienne version quand le code n'a aucune voix ; le GUI ne le faisait pas (RUN d'un code vide coupait la musique) : corrigé. Le fondu à la mesure existe déjà (`morph: N`).
- [x] **Contre-vérification de la cuisson** (sous-agent, à froid) : `skip_to_next_bar` contre `render()` réel, 62 fichiers .loom, 549 voix, 0 différence (hauteur, temps, vélocité, durée). Risque résiduel noté : un futur verbe qui lirait `clock.seconds`/`clock.bars` au moment de planifier ferait diverger la cuisson ; non couverts : horloge MIDI, `bpm_override`, `morph:`.

## Fusion verbes-tidal dans frankenstein (2026-10-03)
- [x] `origin/verbes-tidal` fusionnée (2d3b3da) : `ply`, `linger`, `segment`, `range`, `loopAt`. Sautés : `stut`, `rolled`, `nudge`, `chop` (déjà là), `striate` (reporté).
- [x] Portes après fusion : 414 tests, clippy propre, check_web 4/4, RENDER2 16/16, WAV 16/16, ANALYSE 24/24, TIDAL 88/88, onsets 366/366, runtime 41 ticks, device 5/5.
- [x] Revue sécurité : le port UDP de contrôle (7402) n'est plus dans le device livré, seulement avec `build_device.py --test`.

## Tout finir, produit « Loom » (2026-10-03, demande de Munay)
- [x] Renommage du produit : device `Loom.amxd`, paquet `dist/Loom/` + `Loom.zip`, clips « Loom · voix ».
- [x] B3 · voix bytebeat `t:` (261f7ee). `loom-core/src/expr.rs` : expressions entières 32 bits qui bouclent (comme JS `|0`, `Math.imul`), précédence du C, division par 0 = 0, comparaisons 0/1. `nom: t: EXPR | verbes` : un `|` hors parenthèses suivi d'un mot de 2 lettres ou plus est un verbe, sinon le OU binaire. Pas de notes : loom-audio rend une mesure bout à bout (8 kHz tenu, `t` depuis le début du morceau), routée comme un sample (gain, pan fixe, envois, bus, mute/solo/chance/in), inserts ignorés ici comme sur le web ; `loom bake` saute la voix. Web : AudioBuffer par mesure, même algorithme (`bytebeatFill`). Exemple `bytebeat.loom`. Portes : 426 tests, clippy propre, check_web 4/4, BYTEBEAT 316/316, ANALYSE 25/25, TIDAL 88/88, onsets 374/374, RENDER 9/9, RENDER2 16/16, WAV 16/16, runtime 41 ticks, device ok.
- [x] B5 · `| sync AUTRE` (0f5e0b7). Le jeton du pas global `s` devient `jetons[(s - s_r) mod L]`, `s_r` = premier pas à la dernière reprise du meneur ou après ; cycle du meneur : `poly N` = L/N mesure, `poly m:n` = L/(m·n), voix simple = 1 mesure (K avec `slow K`), bytebeat = 1. Voix inconnue ou soi-même ignorée (linter web), pas de chaîne, `| sync 3` reste le hard sync. Exemple `sync.loom`. Portes : 430 tests, clippy propre, check_web 4/4, SYNC 62/62, onsets 436/436, BYTEBEAT 316/316, TIDAL 88/88, ANALYSE 25/25, RENDER 9/9, RENDER2 16/16, WAV 16/16, runtime 41 ticks, device ok.
- [x] B2 · pas-programmes `=(EXPR)` (b155875). `{}` = choix au hasard, `[]` = sous-pas, `<>` = alternance : `=` dit « ce pas vaut », les parenthèses gardent espaces et `|`. Variables `i` (position du pas dans sa ligne après les verbes ; dans un groupe, dans le groupe ; en poly, l'indice du jeton), `b` (mesure), `n` (pas de la ligne). Valeur : degré compté depuis 0 avec tonalité (0 = `1`, 7 = `1'`, borné ±35), note MIDI sans (0..127), batterie non nul = coup ; modificateurs après la parenthèse. Effets de bord voulus : le tokenizer garde les espaces entre parenthèses, la ligne se coupe sur les `|` hors parenthèses (corrige `xys(…|…)`, avant coupé en deux dans les deux moteurs). Analyse Rust : ordre des voix qui bouclent stable (HashMap → premier vu). Exemple `pas.loom`. Portes : 432 tests, clippy propre (aussi `--all-targets`), check_web 4/4, STEP 54/54, SYNC 62/62, onsets 490/490, ANALYSE 26/26, BYTEBEAT 316/316, TIDAL 88/88, RENDER 9/9, RENDER2 16/16, WAV 16/16, runtime 41 ticks, device ok. `tools/unit_test.js` ne tourne pas (chemin en dur vers ~/Downloads/loom, déjà cassé avant).
- [x] MIDI natif : le « décalage d'un pas » était un artefact du récepteur Python (rtmidi horodate à la réception ; sous charge 6 à 23 il voit 123 trous en 30 s quand l'émetteur en a 0 : 239/240 intervalles à 125 ± 0,1 ms, hauteurs intactes). Gain réel : fil MIDI en QoS « user interactive » + rotation 5 ms avant échéance, retard à l'envoi P50 0,001 ms, P95 0,045 ms, P99 0,65 ms (avant P95 2,0 ms), un pic de 21 ms sur 237 sous charge 8. `LOOM_MIDI_HW=0` : port virtuel seul, pour tester avec Live ouvert sans rien envoyer sur le bus IAC. 414 tests, clippy propre.
- [x] Set de démo `m4l/demo/Loom demo Project/` : `m4l/demo_live.py` construit odyssey (7 pistes, instrument Live + device Loom devant chacune, 6 scènes, 34 clips, 316/316 notes relues), enregistré dans Live par Munay (le panneau d'enregistrement est hors de portée de l'agent). Paquet `dist/Loom.zip` 918 Ko.

## 3 octobre (soir) : Loom complet dans Live (Munay absent, « fais tout »)
- [x] A. Pont au démarrage : LaunchAgent com.munay.loom-pont (/usr/bin/python3), log ~/Library/Logs
- [x] B. Lien Ableton : `@Param: valeurs` (automation dans le clip), `go: scène` (lance la scène), glisser un clip MIDI ou un sample dans la fenêtre
- [x] C. Paquet : web/ + pont + installeur du LaunchAgent dans dist/Loom
- [x] D. Build de vente installé dans Live (sans port UDP)
- [x] E. LISEZ-MOI, lessons, BUS, mémoire, fiche de reprise
- Review : tout fait, portes 440 Rust / device 34/34 / web 4/4 / parité 0 écart. Lien Live stagé, à valider à la main (faux positif du hook). Test souris dans Live à faire par Munay.

## 3 octobre (nuit) : le Loom terminal complet comme le web
- [x] 1. VIBE : les 10 puces d'un chiffre (F2 puis 1..0)
- [x] 2. Export .mid standard (Alt+X, et loom bake x.loom x.mid)
- [ ] 3. Mes sons : importer / vider ses samples depuis l'appli
- [x] 4. Correcteur : loom-app/src/lint.rs (fautes de frappe avec suggestion, degrés sans key:, ^ sans prog:, sections absentes de song:, sample inconnu), lignes soulignées en rouge au RUN ; 5 tests, 0 faux positif sur les 66 exemples (1 vrai : studio.loom l.35, ^1 sans prog:)
- [x] 5. Preset suivant / au hasard, supprimer un morceau du sélecteur
- [x] 6. Lien Live : chaque RUN écrit le .loom lié
- [ ] 7. Scènes : plus de visuels terminal
- [x] 8. Partition à l'écran : Alt+F, grande portée (sol et fa), lignes supplémentaires, ♯/♭, tête de lecture ; vérifiée dans tmux sur ballade.loom
- [x] 9. Thèmes intégrés hors Omarchy : Alt+C fait défiler les 62 thèmes du web (copiés par tools/gen_themes.py, une seule source), choix gardé dans ~/.local/state/loom/theme
- [ ] 10. Dictée vocale dans VIBE (whisper local)
