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
- [x] A3. Accords romains diatoniques `I ii iii IV V vi vii` (+`7`) contre la key (s'adaptent à la gamme). Arpégeables. Preset PROG.
- [x] A4. Arpèges `| arp up/down/updown/downup` — égrène les accords sur le pas. Preset ARP.

### B. Génératif & rythme
- [x] B1. Rythmes euclidiens `e(k,n)` et `e(k,n,rot)` — distribution Bjorklund + rotation, mixable avec tokens normaux
- [x] B2. Probabilité par pas (`x?` ~50%, `x?70` 70%) — sur n'importe quel token. Preset CHANCE.
- [x] B3. Seed déterministe (`seed: 42`) — RNG SplitMix64/(seed,bar,voix,pas), rejouable à l'identique.
- [x] B4. Ratchets / rolls (`x*3` = N frappes dans un pas), combinable avec `?`. Preset ROLLS. (mutations = plus tard si temps)

### C. Structure (morceaux longs)
- [x] C1. Structure : `song: intro*4 verse*8 chorus*8 …` + tag de voix `| in verse chorus`. Struct `Program` dans loom-core. Preset SONG. La console affiche la section courante.
- [ ] C2. Variation A/B, longueur de morceau

### D. Polyrythmie & temps
- [ ] D1. Polymètre (voix de longueurs différentes qui se déphasent)
- [ ] D2. Changements de tempo/mesure

### E. Sound design (web — beaux sons)
- [x] E1. Filtre par voix `| lp 800` / `| hp 2000` (+ `q R` résonance). Refactor `tone(t,f,dur,v[,opt])`.
- [x] E2. Enveloppe ADSR par voix `| env A D S R` (sustain 0-1). Son par défaut inchangé si absent. Preset PAD.
- [x] E3. LFO `| lfo cutoff|pitch|gain RATE DEPTH` (mouvement, vibrato, tremolo).
- [x] E4. Unison/detune super-saw `| unison N` (+ `| detune C`). Preset SUPERSAW.
- [x] E5. Distorsion `| drive X` (0-1, WaveShaper). (bitcrush = plus tard si temps)
- [x] E6. Sidechain — ligne `duck: 70` : tout le mix duck à chaque kick (pompage techno/house). Preset PUMP.
- [x] E7. Presets de genre soignés : CHIP, TECHNO, HOUSE, DNB, AMBIENT, SYNTHWAVE (utilisent filtres/env/unison/drive/duck/euclid).

### F. Interface
- [ ] F1. Galerie de thèmes terminal (CRT vert, ambre, C64, DOS, Mac, moderne) + sélecteur
- [ ] F2. Bouton COURS : tutoriel explicatif intégré (apprendre LOOM pas à pas, exemples cliquables)

## Journal (le plus récent en haut)
_(je remplis ici à chaque étape : ce que j'ai fait, fichiers touchés, tests)_

- **[B1]** Rythmes euclidiens `e(k,n[,rot])` : core (2 tests) + miroir JS + prompt IA. Essaie `kick: e(4,16)` ou `hat: e(7,16,2)`. 23 tests verts.
- **[A1]** 19 gammes ajoutées (maqam/raga/gamelan/blues/harmonic-minor/whole-tone…) : core (1 test) + JS + prompt IA + presets HIJAZ & RAGA. 21 tests verts.
- **[A3] Accords en chiffres romains.** Avec une `key:`, écris `I ii iii IV V vi vii` (+`7` pour la septième : `V7`, `ii7`) → l'accord diatonique construit sur ce degré, dont la qualité (majeur/mineur/dim) vient automatiquement de la gamme (en mineur `i` est mineur, etc.). Parfait pour les progressions (`I V vi IV`). Arpégeables avec `| arp up`. Essaie le preset **PROG**. (loom-core +2 tests, miroir JS dans trigger+arp, SYS, preset)
- **[E7] Presets de genre soignés.** Les boutons du haut sont maintenant de vrais points de départ par style, exploitant tout le nouveau sound design : **CHIP** (8-bit), **TECHNO** (pump + stabs), **HOUSE** (off-beat hats, stabs orgue), **DNB** (break + reese bass disto), **AMBIENT** (nappes lentes + LFO), **SYNTHWAVE** (arp + pad gated + basse grasse). Clique-les et regarde le code pour apprendre les combinaisons. (web only, 38 tests verts)
- **[E6] Sidechain / pump.** Ajoute une ligne `duck: 70` (0-100) : tout le mix « duck » (baisse puis remonte) à chaque coup de kick — le pompage caractéristique de la techno/house. Implémenté via un gain `scGain` entre le bus et le master, déclenché dans `kick()` (recovery exponentielle). Essaie le preset **PUMP**. (web only ; `duck` ajouté aux directives core pour ne pas créer de voix — 38 tests verts)
- **[E3+E4+E5] LFO, super-saw, distorsion.** Sur une voix mélodique : `| unison 5 | detune 18` empile 5 oscillateurs désaccordés = gros son super-saw ; `| drive 0.5` ajoute de la chaleur/saturation ; `| lfo cutoff 0.3 0.6` fait bouger le filtre (cible `cutoff`, `pitch` = vibrato, `gain` = tremolo ; RATE en Hz, DEPTH 0-1). `tone()` refactoré en chaîne unison→filtre→ADSR→drive→tremolo→bus, avec clamps (pas de clic/saturation incontrôlée). Essaie le preset **SUPERSAW**. (web only, core inchangé/vert)
- **[E1+E2] Filtre & enveloppe ADSR (sound design).** Sur une voix mélodique : `| lp 800` (passe-bas, plus sombre), `| hp 2000` (passe-haut), + `q R` pour la résonance (ex. `lp 600 q 8`). `| env A D S R` = enveloppe (attaque, déclin, niveau de sustain 0-1, release, en secondes) : `env 0.5 0.4 0.7 1.4` pour des nappes lentes, `env 0.005 0.08 0 0.05` pour un pluck sec. Le `tone()` a été refactoré pour porter ces réglages ; **le son par défaut est inchangé** si tu ne mets ni filtre ni env. Essaie le preset **PAD**. (web only, miroir core inchangé/vert)
- **[C1] Structure de morceau (morceaux longs).** Pose un arrangement avec `song: intro*2 verse*4 chorus*4 verse*4 chorus*4 outro*2` (nom*barres, l'arrangement boucle). Puis tague chaque voix avec `| in verse chorus` pour choisir où elle joue. Les voix sans tag jouent partout. On crée les montées/drops en ajoutant des voix dans les sections plus intenses. La console affiche la section courante en live. Nouvelle struct `Program` dans loom-core (front door : `Program::parse(src).schedule_bar(bar)`). Essaie le preset **SONG**. (loom-core +3 tests, wasm bascule sur Program, miroir JS, SYS, preset)
- **[A4] Arpèges.** Ajoute `| arp up` (ou `down`, `updown`, `downup`) à une voix d'accords : chaque accord est égrené note par note sur la durée du pas — la séquence arpégée classique (techno/synthwave). Essaie le preset **ARP**. (loom-core +2 tests, miroir JS, SYS, preset)
- **[B4] Ratchets / rolls.** Suffixe `*N` = N frappes rapides dans un seul pas : `snare: . . x*4 .` (un roll), `hat: x*2 x x*3 x`. Combinable : `x*4?60` (roll qui sort 60% du temps). Parfait pour les montées/fills. (loom-core +2 tests + refactor `token_mods` robuste qui gère `?` et `*` dans n'importe quel ordre, miroir JS, SYS, preset ROLLS)
- **[B2+B3] Probabilité & seed.** Ajoute `?` à un token pour qu'il joue *parfois* : `x?` ~50%, `x?70` = 70%, `c3?25`, `Cm7?` — marche sur tout (drums, notes, degrés, accords). `seed: N` rend les choix aléatoires **reproductibles** (rejoue à l'identique) ; change le seed pour une variation. Essaie le preset **CHANCE**. (loom-core +3 tests, RNG déterministe, miroir JS, wasm, SYS, preset)
- **[A2] Accords.** Un token à **initiale MAJUSCULE** joue un accord entier : `Cm7`, `F`, `G7`, `Am`, `Dsus4`, `Gmaj7`… Minuscule reste une note simple (`c3`) → aucune ambiguïté. ~30 qualités. Mets-les sur une voix `keys:`/`pad:`. Essaie le preset **CHORDS**. (loom-core +4 tests, miroir JS multi-notes, SYS, preset)
- **[B1] Rythmes euclidiens.** `e(k,n)` = k frappes réparties uniformément sur n pas ; `e(k,n,rot)` tourne le motif. Ex : `kick: e(4,16)`, `hat: e(7,16,2)`. Mixable avec des tokens normaux. Essaie le preset **EUCLID**. (loom-core +2 tests, miroir JS, SYS, preset)
- **[A1] Gammes du monde.** 19 gammes ajoutées (hijaz, hijazkar/byzantine, nahawand, kurd, ajam, bhairav, yaman, todi, slendro, pelog, hirajoshi, insen, iwato, egyptian, blues, harmonic/melodic minor, whole-tone, chromatic). Presets **HIJAZ**, **RAGA**. Note : les quarts de ton (maqam bayati/rast/saba) sont approximés en 12-TET — couche microtonale = chantier futur.
- **[init]** git initialisé, baseline commitée sur `main`, branche `night-build` créée. Plan posé.
