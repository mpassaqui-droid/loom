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
- [ ] Raccourci Hyprland (proposé, pas imposé)

### Plus tard (sessions suivantes)
- [ ] Porter FX, filtres/ADSR, samples, MIDI, VIBE (IA) vers le moteur natif

## Review

**2026-09-14 — apps natives livrées et installées (`./install.sh`).**
- `loom-audio` (nouveau) : synthé natif + séquenceur + cpal + export WAV. `loom-app` (nouveau) : thème Omarchy live, bibliothèque, grille.
- `loom` (TUI) et `loom-gui` (egui) : éditeur coloré, grille animée, transport, mute, volume, export, dernier fichier retenu.
- Bugs corrigés dans `loom-core` : `Program::schedule_bar` ignorait `prog:` (les voix `^` étaient muettes, en natif ET dans l'A/B WASM du web). `Event` porte maintenant `dur`.
- Tests : core 90 · audio 7 · app 4 · tui 3 · gui 12.
- Limites connues : le natif n'a pas encore FX / filtres / env / unison / samples / MIDI / VIBE → `h_concrete` (100 % samples) est muet, les autres sonnent « nus ». Le parser ignore silencieusement les lignes invalides.
- Idées suivantes : Status « changement en attente » pour la grille ; erreurs de parse avec n° de ligne ; porter `lp`/`env`/`unison` puis les samples (`web/samples`).

