# To-do — LOOM

> Plan écrit AVANT d'implémenter (items cochables), coché au fur et à mesure,
> section « Review » ajoutée en fin de session. La roadmap long-terme vit dans
> `CLAUDE.md` ; ici = le travail en cours, session par session.

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
- [ ] **P2 · Accessibilité** (web + GUI) — navigation 100 % clavier, contrastes
      des thèmes (WCAG AA), grandes polices / zoom, focus visible.
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
- [ ] **P4 · Composition algorithmique dans la langue** — la langue qui grandit :
      verbes `l-sys` (L-systèmes), grammaires stochastiques (Xenakis), `evolve`
      (génétique) ; `loom-core` + tests + parité 2 moteurs + doc.
- [ ] **P5 · Base de connaissance & explicatriceur** — étendre `LORE`/`MOOD` :
      ethos des modes grecs, wuxing↔notes, talas, histoire par tradition ;
      explicatriceur « pourquoi ça sonne » + prof interactif ; miroir JS + parité.
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
