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
- [x] A2. Accords `Cm7` `F` `G7` — MAJUSCULE = accord (notes simultanées), minuscule = note. 30+ qualités (maj m dim aug 7 maj7 m7 m7b5 dim7 6 m6 sus2 sus4 add9 9 maj9 m9). Preset CHORDS.
- [x] A1. Gammes du monde (hijaz, hijazkar, nahawand, kurd, ajam, bhairav, yaman, todi, slendro, pelog, hirajoshi, insen, iwato, egyptian, blues, harmonic/melodic minor, whole-tone, chromatic) — presets HIJAZ, RAGA
- [ ] A2. Accords `chord: Cm7` — un token joue plusieurs notes (triades, 7th, sus, add)
- [ ] A3. Progressions d'accords + degrés d'accords romains (I IV V vi…)
- [ ] A4. Arpèges (`arp up/down/updown` sur un accord)

### B. Génératif & rythme
- [x] B1. Rythmes euclidiens `e(k,n)` et `e(k,n,rot)` — distribution Bjorklund + rotation, mixable avec tokens normaux
- [x] B2. Probabilité par pas (`x?` ~50%, `x?70` 70%) — sur n'importe quel token. Preset CHANCE.
- [x] B3. Seed déterministe (`seed: 42`) — RNG SplitMix64/(seed,bar,voix,pas), rejouable à l'identique.
- [x] B4. Ratchets / rolls (`x*3` = N frappes dans un pas), combinable avec `?`. Preset ROLLS. (mutations = plus tard si temps)

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
- [ ] F2. Bouton COURS : tutoriel explicatif intégré (apprendre LOOM pas à pas, exemples cliquables)

## Journal (le plus récent en haut)
_(je remplis ici à chaque étape : ce que j'ai fait, fichiers touchés, tests)_

- **[B1]** Rythmes euclidiens `e(k,n[,rot])` : core (2 tests) + miroir JS + prompt IA. Essaie `kick: e(4,16)` ou `hat: e(7,16,2)`. 23 tests verts.
- **[A1]** 19 gammes ajoutées (maqam/raga/gamelan/blues/harmonic-minor/whole-tone…) : core (1 test) + JS + prompt IA + presets HIJAZ & RAGA. 21 tests verts.
- **[B4] Ratchets / rolls.** Suffixe `*N` = N frappes rapides dans un seul pas : `snare: . . x*4 .` (un roll), `hat: x*2 x x*3 x`. Combinable : `x*4?60` (roll qui sort 60% du temps). Parfait pour les montées/fills. (loom-core +2 tests + refactor `token_mods` robuste qui gère `?` et `*` dans n'importe quel ordre, miroir JS, SYS, preset ROLLS)
- **[B2+B3] Probabilité & seed.** Ajoute `?` à un token pour qu'il joue *parfois* : `x?` ~50%, `x?70` = 70%, `c3?25`, `Cm7?` — marche sur tout (drums, notes, degrés, accords). `seed: N` rend les choix aléatoires **reproductibles** (rejoue à l'identique) ; change le seed pour une variation. Essaie le preset **CHANCE**. (loom-core +3 tests, RNG déterministe, miroir JS, wasm, SYS, preset)
- **[A2] Accords.** Un token à **initiale MAJUSCULE** joue un accord entier : `Cm7`, `F`, `G7`, `Am`, `Dsus4`, `Gmaj7`… Minuscule reste une note simple (`c3`) → aucune ambiguïté. ~30 qualités. Mets-les sur une voix `keys:`/`pad:`. Essaie le preset **CHORDS**. (loom-core +4 tests, miroir JS multi-notes, SYS, preset)
- **[B1] Rythmes euclidiens.** `e(k,n)` = k frappes réparties uniformément sur n pas ; `e(k,n,rot)` tourne le motif. Ex : `kick: e(4,16)`, `hat: e(7,16,2)`. Mixable avec des tokens normaux. Essaie le preset **EUCLID**. (loom-core +2 tests, miroir JS, SYS, preset)
- **[A1] Gammes du monde.** 19 gammes ajoutées (hijaz, hijazkar/byzantine, nahawand, kurd, ajam, bhairav, yaman, todi, slendro, pelog, hirajoshi, insen, iwato, egyptian, blues, harmonic/melodic minor, whole-tone, chromatic). Presets **HIJAZ**, **RAGA**. Note : les quarts de ton (maqam bayati/rast/saba) sont approximés en 12-TET — couche microtonale = chantier futur.
- **[init]** git initialisé, baseline commitée sur `main`, branche `night-build` créée. Plan posé.
