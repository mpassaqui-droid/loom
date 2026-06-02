# NIGHT BUILD LOG — LOOM

Bonjour ☀️ Tu dormais, j'ai construit. Lis ce fichier en premier.

## Comment essayer (au réveil)
1. Le serveur web tourne peut-être encore : http://localhost:8753/index.html
   - Sinon, relance-le : `cd web && python3 -m http.server 8753` puis ouvre le lien.
2. Clique **RUN** (ou Ctrl/Cmd+Entrée). Essaie les nouveaux presets.
3. Cœur Rust : `. "$HOME/.cargo/env" && cargo test -p loom-core` doit être tout vert.
4. Tout est sur la branche git **night-build** (l'état d'avant est sur **main**).
   - Voir l'historique : `git log --oneline`
   - Revenir à l'état d'avant si besoin : `git checkout main`
   - Comparer : `git diff main night-build -- web/index.html`

## Règles que je me suis fixées
- Mode SÛR : je teste à chaque étape, je ne commite QUE ce qui marche.
- `cargo test -p loom-core` vert + app jouable à tout moment.
- Le moteur Rust (`loom-core`) reste la source de vérité du langage (logique + tests).
  Le SON vit dans `web/index.html` (Web Audio). Je ne peux pas l'entendre — je garantis
  que ça tourne, tu juges la beauté. Chaque feature : core+tests → miroir JS → prompt IA → preset.

## Plan (coché au fur et à mesure)

### A. Théorie musicale
- [ ] A1. Gammes du monde (maqam hijaz/bayati, raga, gamelan slendro/pelog, blues, harmonic/melodic minor, whole-tone…)
- [ ] A2. Accords `chord: Cm7` — un token joue plusieurs notes (triades, 7th, sus, add)
- [ ] A3. Progressions d'accords + degrés d'accords romains (I IV V vi…)
- [ ] A4. Arpèges (`arp up/down/updown` sur un accord)

### B. Génératif & rythme
- [ ] B1. Rythmes euclidiens `e(3,8)`, `e(5,8,2)` (pulses, steps, rotation)
- [ ] B2. Probabilité par pas (`x?` joue parfois, `x?70` 70%)
- [ ] B3. Seed déterministe (`seed: 42`) pour que l'aléatoire soit reproductible
- [ ] B4. Ratchets / répétitions (`x*3`), mutations

### C. Structure (morceaux longs)
- [ ] C1. Sections nommées + arrangement/timeline (intro/verse/chorus/drop, ordre, répétitions)
- [ ] C2. Variation A/B, longueur de morceau

### D. Polyrythmie & temps
- [ ] D1. Polymètre (voix de longueurs différentes qui se déphasent)
- [ ] D2. Changements de tempo/mesure

### E. Sound design (web — beaux sons)
- [ ] E1. Filtre par voix (lowpass/highpass, cutoff, résonance)
- [ ] E2. Enveloppe ADSR par voix
- [ ] E3. LFO (sur cutoff / pitch / volume)
- [ ] E4. Detune / unison (super-saw), plus de richesse
- [ ] E5. Distorsion / drive, bitcrush
- [ ] E6. Sidechain (pompage sur le kick) — essentiel techno/house
- [ ] E7. Presets de genre : Techno/House, DnB/Jungle, Ambient/Drone, Synthwave/Chiptune

### F. Interface
- [ ] F1. Galerie de thèmes terminal (CRT vert, ambre, C64, DOS, Mac, moderne) + sélecteur

## Journal (le plus récent en haut)
_(je remplis ici à chaque étape : ce que j'ai fait, fichiers touchés, tests)_

- **[init]** git initialisé, baseline commitée sur `main`, branche `night-build` créée. 20 tests core verts. Plan posé.
