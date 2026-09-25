# To-do — LOOM

> Plan écrit AVANT d'implémenter (items cochables), coché au fur et à mesure,
> section « Review » ajoutée en fin de session. La roadmap long-terme vit dans
> `CLAUDE.md` ; ici = le travail en cours, session par session.

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
