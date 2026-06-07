# 🌍 PLAN-MONDE — intégrer le meilleur de la musique savante mondiale dans LOOM

> Suite du [ROADMAP](ROADMAP.md). La vision : un instrument unique qui réunit
> **DAW + code + visuel + maths + physique + biologie + philosophie** — tout pour
> la musique. La connaissance NE s'écrit PAS « en dur » : elle vit dans (1) le
> cerveau IA + un prompt enrichi, (2) une base de savoir interne (déjà
> `TRADITION_LORE`), (3) un langage qui grandit (nouveaux verbes/générateurs),
> (4) des modules DSP. On ajoute des OUTILS et du SAVOIR par couches.

Légende : ✅ fait · 🔨 prochaine étape réaliste · 🌠 grand chantier · 🧪 demande de la recherche/DSP

---

## A. LA CARTE — centres & disciplines à puiser

### A1. Centres d'informatique musicale & d'acoustique (le monde)
- **France** — **IRCAM** (Paris) : spectral, Max, Spat (spatialisation), Antescofo
  (suivi de partition), **OpenMusic/OM** (composition assistée), neural audio
  (**RAVE**), AudioSculpt/SuperVP (phase vocoder). **GRAME** (Lyon) : **Faust**
  (langage DSP). **GRM/INA** (musique concrète/acousmatique, **GRM Tools**).
  **LMA-CNRS** (Marseille, acoustique). **Tempo Reale** (Berio, Florence).
- **USA** — **CCRMA Stanford** (FM de **Chowning**, **waveguides/modal** de Julius
  O. Smith, ambisonique, Faust). **CNMAT Berkeley** (**OSC**, additif, SDIF).
  **MIT Media Lab** (hyperinstruments, Machover). **Princeton** (**ChucK**, PLOrk,
  Ge Wang). **UCSD** (Miller **Puckette** = Pd & origine de Max). **Mills College**
  (Center for Contemporary Music). **Dartmouth**, **CCRMA**, **Berklee**.
- **Royaume-Uni** — **C4DM Queen Mary** (MIR/ISMIR, Sonic Visualiser).
  **Huddersfield** (spatial, HISS). **De Montfort** (électroacoustique). **Goldsmiths**.
- **Allemagne/Autriche** — **ZKM** (Karlsruhe). **IEM Graz** (référence ambisonique).
  **SWR Experimentalstudio**. **Native Instruments / Ableton + Cycling'74** (Berlin/SF).
- **Nordiques** — **KTH Stockholm** (acoustique musicale, règles d'interprétation,
  Sundberg). **EMS Stockholm**. **Aalborg** (Sound & Music Computing). **Sibelius Academy**.
- **Pays-Bas** — **STEIM** (live electronics), **Institut de Sonologie** (La Haye).
- **Canada** — **CIRMMT/McGill** (acoustique, perception, MIR).
- **Japon/Corée** — ATR, recherche Yamaha ; KAIST.
- **Communautés/confs** — **ICMC, NIME, ISMIR, DAFx, SMC, TENOR** (notation),
  **Ars Electronica** (Linz).

### A2. Maths
- Théorie des ensembles (Forte), **théorie transformationnelle** néo-riemannienne
  (Lewin, **Tonnetz**, PLR), **géométrie de la musique** (Tymoczko, orbifolds).
- **Xenakis** : stochastique, **cribles (sieves)**, théorie des jeux, **GENDYN**.
- **Canons de Vuza** (tiling rhythmic canons), théorie des nombres, groupes, combinatoire.
- **Spectralisme** (Grisey, Murail) : l'harmonie déduite du spectre.
- Automates cellulaires, **L-systèmes**, fractales/IFS, **chaos** (logistique, Lorenz),
  **théorie de l'information** (entropie, surprise, complexité).
- Markov d'ordre N, marches aléatoires, **bruit 1/f** (Voss-Clarke).

### A3. Physique
- Acoustique : ondes, modes, **guides d'ondes**, **synthèse modale**, FEM/BEM,
  **acoustique des salles** (RT60, premières réflexions → **FDN**).
- **Psychoacoustique** : bandes critiques, masquage, courbes isosoniques
  (Fletcher-Munson), **rugosité Plomp-Levelt** → consonance/dissonance CALCULABLE,
  hauteur virtuelle (Terhardt), illusions **Shepard/Risset**.
- Dynamique non-linéaire, **synchronisation de Kuramoto** (oscillateurs couplés).

### A4. Biologie
- Biomusicologie, **chant des oiseaux** (syntaxe), entraînement rythmique.
- **Algorithmes génétiques/évolutionnaires** (sélection de motifs), **essaims/Boids**
  (voix qui se suivent), **jeu de la vie** (Conway), **L-systèmes** (croissance → forme).
- **Neural audio** : **RAVE** (IRCAM), **DDSP**/NSynth (Magenta), EnCodec — timbres
  appris. **Sonification** (ADN, protéines, données).

### A5. Philosophie / esthétique
- Pythagore & **harmonie des sphères**, Boèce, Rameau (basse fondamentale),
  Schenker (couches), **Cage** (hasard, indétermination), **Eno** (génératif),
  Stockhausen (moment form), **Schaeffer** (objet sonore), **Smalley**
  (spectromorphologie), Adorno, Deleuze (ritournelle), cybernétique (Wiener).
- Cosmologies non-occidentales (rasa, sayr du maqâm, wuxing, pathet du gamelan).

---

## B. Modules concrets dérivés (verbe LOOM proposé)
- **Spectral (IRCAM)** 🧪 : `spectral freeze|stretch|morph`, **phase vocoder**
  (time-stretch/transpose indépendants), `chord from <sample>` (harmonie spectrale).
- **Modèles physiques (Stanford)** 🧪 : `string|tube|membrane|plate` (waveguide/modal
  génériques), au-delà du Karplus actuel.
- **Maths** 🔨🧪 : `sieve(...)`, `vuza`, `markov N`, `gendy`, `1/f`, `tonnetz`/`neo PLR`.
- **Consonance calculée** 🔨 : analyseur **Plomp-Levelt** (rugosité) → l'explicateur
  « pourquoi ça sonne » et l'IA s'en servent.
- **Bio** 🧪 : `boids`, `evolve`, `life`, neural (`rave`/`ddsp` si modèle WASM).
- **Spatialisation (IEM/IRCAM)** 🧪 : ambisonique, `spat azimuth elevation`.
- **Faust embarqué** 🌠 : compiler du DSP utilisateur en WASM.

---

## C. VISUEL ↔ CODE : un lien VIVANT (demande explicite)
**Principe : le visuel est une FONCTION du programme** (déterministe depuis le code
+ seed). Chaque variation de code → variation d'état visuel.
- 🔨 **Mapping data-driven** : chaque voix expose des *features* (hauteur, densité,
  registre, timbre, FX, énergie par bande FFT) → paramètres visuels (couleur, forme,
  vitesse, turbulence, profondeur). On généralise `hue`/`viz` à TOUT.
- 🔨 **Live link** : en tapant, l'état visuel morphe en continu (le recompile existe
  déjà → on le branche sur le rendu, pas seulement le nom de scène).
- 🌠 **Mode interactif/création** : un **graphe visuel** (façon Jitter/TouchDesigner)
  où des nœuds lisent les features audio et pilotent des shaders — un mini-langage
  `vviz:` parallèle au code musical, que la musique pilote directement.
- ✅ Garder **N** (variation de presets) ET ajouter le visuel **généré par le code**.
- 🔨 Réactivité fine : durée de note (sustain), pas seulement l'attaque ; latence réduite.

---

## D. LA NOUVEAUTÉ MUSICALE (le vrai problème : « toujours la même musique »)
**Pourquoi ça sonne pareil** : grilles 16 pas fixes, mêmes patterns de batterie,
progressions tonales pop, formes courtes. **Ce qu'on ajoute :**
- 🔨 **Forme générative macro** : développement motivique (inversion, rétrograde,
  augmentation/diminution), **courbes de tension**, contraste de sections, formes
  (arche, moment-form, processus) — pas juste `song:` qui répète.
- 🔨 **Rythme** : polytempo, **modulation métrique**, tuplets imbriqués, mètres
  additifs, **timelines africaines**, **gati/korvai carnatiques**, canons de Vuza.
- 🌠 **Harmonie hors-clichés** : harmonie **spectrale**, **harmonie négative**,
  conduite de voix par automate, post-tonal, lattices en intonation juste, **néo-riemannien**.
- 🔨 **Process music** : phasing (✅), **L-systèmes** pour des pièces entières,
  **Markov appris d'un corpus**, mélodie **sous contraintes**.
- 🌠 **Construction timbrale** : évolution/morphing spectral, couches.
- 🔨 **Cerveau IA « compositeur »** : un mode qui compose AVEC ces outils + un prompt
  qui **interdit les clichés** (mêmes drums/progressions) et **impose** développement,
  contraste, surprise, forme longue.
- **Tout de suite** : un générateur `develop`/`form` + un mode IA « construit » +
  une bibliothèque de **batteries/rythmes du monde** pour casser la routine.

---

## E. CONNEXION ABLETON LIVE (question « comment ») — du simple au complet
1. **MIDI maintenant (navigateur)** 🔨 : Web MIDI marche déjà. Créer un **port
   virtuel** — **IAC Driver** (macOS : *Audio MIDI Setup → IAC*, activer) ou
   **loopMIDI** (Windows). LOOM → IAC → Ableton (armer une piste MIDI sur ce port).
   Bidirectionnel : Ableton → IAC → entrée Web MIDI de LOOM.
2. **Sync tempo/horloge** 🔨 : émettre/lire le **MIDI Clock** (start/stop, 24 ppqn) —
   LOOM cale Ableton et inversement. Réaliste en Web MIDI.
3. **Sync pro = Ableton Link** (tempo + phase, multi-apps/appareils) 🌠 : pas natif
   au navigateur → **pont local** (petite app Node). **Carabiner** expose Link via
   socket : LOOM ↔ WebSocket ↔ Carabiner ↔ Link ↔ Ableton.
4. **OSC** 🌠 : via le même pont (**AbletonOSC** ou un device Max). Contrôle fin
   bidirectionnel de tous les paramètres.
5. **Audio dans Ableton** 🔨 : router le son via un périphérique audio virtuel —
   **BlackHole** (macOS) / **VB-Cable** (Windows) — pour enregistrer/traiter LOOM.
6. **Max for Live** 🌠 : un device M4L ↔ pont WebSocket/OSC ↔ LOOM = contrôle total.
> Ordre : (a) MIDI notes + clock in/out maintenant ; (b) pont Node optionnel
> (Link + OSC) ; (c) device M4L.

---

## F. MISE EN PAGE / ESPACE / RESPONSIVE (remarque : « désordonné, texte tout en bas »)
- 🔨 **Workspace en panneaux** (CSS grid + breakpoints, panneaux dockables/repliables) :
  - **Ordinateur (large)** : gauche = éditeur ; centre = visuel/scène ; droite =
    mixeur + inspecteur + savoir ; bas = console + timeline → on **exploite les côtés**.
  - **iPad** : 2 panneaux ou onglets ; **rotation gérée** (resize).
  - **Téléphone** : empilé / onglets, gros boutons.
- 🔨 Console = **vrai panneau multi-lignes défilant** (fini le message unique).
- 🔨 Densité adaptative + **sauvegarde de la disposition**.

---

## G. AUTRES FAÇONS DE VOIR LE CODE (au-delà du terminal)
- 🔨 **Piano-roll / partition** éditable (bidirectionnel).
- 🌠 **Vue patch/nœuds** (boîtes façon Max/Pd) — voix = nœuds, FX = chaîne.
- 🌠 **Timeline / arrangement** (clips glissables).
- 🔨 **Vue blocs** (type Scratch/Sonic Pi) pour débuter.
- 🔨 **Tracker** (colonnes verticales, classique).
- 🧪 **Spectrogramme / sonogramme** (voir le son).
- 🔨 **Vue structure** (la forme du morceau en arbre/graphe).
- 🔨 **Thèmes** au-delà du terminal (IDE moderne, papier, néon, clair/sombre, daltonien).

---

## H. ORDRE CONSEILLÉ (pour que ça reste jouable à chaque étape)
1. **Stabilisation** (bugs audio + visuel de l'audit : plafond de voix, dry/wet,
   equal-power pan, devicePixelRatio, resize, fractales adaptatives, console multi-lignes).
2. **Lien visuel↔code vivant** (data-driven) + **workspace en panneaux** (mieux utiliser l'espace).
3. **Moteur de nouveauté** (développement motivique + formes + rythmes du monde + prompt IA anti-cliché).
4. **Pont Ableton** (MIDI/clock now → pont Link/OSC → M4L).
5. **Modules savants par vagues** (spectral, modal, math, bio) — chacun = un verbe + du savoir.
6. **Vues de code alternatives** (piano-roll, patch, timeline) + thèmes.

> Chaque ligne ci-dessus peut devenir une « vague » concrète, testée et commitée —
> exactement comme on a procédé jusqu'ici.

---

## I. IDÉES SUPPLÉMENTAIRES (au-delà du cœur du plan)

### I1. 🧬 Faire ÉVOLUER la musique (remède direct au « toujours pareil »)
- 🔨 **Mètre de surprise** (théorie de l'info) : mesure la prévisibilité d'un morceau →
  un curseur « plus surprenant / plus cohérent ». *(= vague en cours, étape 1)*
- 🌠 **ADN du morceau** : un seed compact = une pièce entière ; **croiser** deux pièces
  (crossover génétique) → des « enfants » ; **sélection par le goût** (« j'aime → 8 variantes »).
- 🌠 **Musique adaptative** (audio de jeu) : réagit à un état (couches verticales, reséquençage).

### I2. 🤲 Le corps & le geste (l'expressivité)
- 🧪 **Webcam → contrôle** (suivi mains/corps, MediaPipe). 🔨 **Téléphone/tablette** comme
  contrôleur (inclinaison, multitouch, accéléromètre). 🧪 **Voix/souffle** comme contrôleur
  continu (chante → pilote des paramètres). 🔨 **Haptique** sur les basses.

### I3. 🎓 L'université vivante (le propriétaire est PROF — sous-exploité)
- 🔨 Cursus interactif : oreille, théorie, rythme, **exercices générés depuis TES morceaux**.
- 🌠 **Tuteur IA socratique** qui écoute ce que tu fais et enseigne en contexte.
- 🔨 Défis/énigmes (« reproduis ce rythme », harmoniser, dictées), chaque concept démontré en son.

### I4. 🎻 Le pont vers les musiciens acoustiques
- 🔨 **Export notation** (MusicXML / Lilypond) → un ensemble réel peut jouer ta pièce.
- 🌠 **Orchestration générative**. 🔨 **Import MIDI/MusicXML** pour analyser & re-générer.

### I5. 🌐 Sonifier le monde
- 🔨 Sonifier données/météo/**EEG-biosignaux**/astro/texte/**image→son**.
- 🧪 **Écoacoustique / paysages sonores** (Schafer, Oliveros) ; chant d'oiseaux comme matériau.

### I6. 🧠 IA profonde (au-delà de composer)
- 🧪 **Audio→code** (chante/joue → transcription LOOM). 🧪 **Séparation de stems** → code.
- 🌠 **Transfert de style** (« à la manière de… »). 🔨 **Continuation/remplissage** de section.
- 🌠 **Compositeur conversationnel** avec mémoire du projet ; 🧪 option **modèle local** hors-ligne.

### I7. 🎚️ Moteurs de composition savante
- 🌠 **Contrepoint** (espèces, canon, **fugue**, harmoniser une mélodie). 🔨 **Réharmoniseur**
  (substitutions, dominantes secondaires). 🌠 **Solveur sous contraintes**.
- 🧪 **Labo microtonal** : import **Scala (.scl)**, lattices, comma pumps, **intonation juste
  adaptative** (accords ré-accordés en ratios purs à la volée — son sublime).

### I8. 🔊 Espace & immersion
- 🧪 **Binaural/HRTF** (3D au casque), 🌠 **ambisonique** (multi-HP). 🔨 **Pièces evergreen**
  (génératives infinies, jamais répétées). 🌠 **Réactives au temps/lieu/météo** (installation).
  🌠 **VR/AR** (composer dans l'espace).

### I9. 🤝 Musique en réseau
- 🌠 **Jam collaboratif temps réel** (multi-utilisateurs, orchestre de laptops).
- 🌠 **Participation du public** (chaque téléphone = une voix). 🔨 **Galerie/communauté** (partage, fork, remix).

### I10. 🎛️ Scène, live & monde physique
- 🔨 **Mode Algorave** (gros visuels, historique, panique, crossfade, **looper**, **pad de morphing**).
- 🧪 **Score following** (façon Antescofo) : tu joues, LOOM te suit. 🌠 **Sortie CV/Eurorack**, capteurs.

### I11. ✨ Le savoir incarné (l'âme, unique à LOOM)
- 🌠 Un morceau qui EST partition + explication + visuel + leçon ; une musique **qui se connaît**.
- 🌠 **Moteur de fusion** des traditions (maqâm + gamelan + jazz, *avec* le savoir pour le faire juste).

### Priorité personnelle conseillée (impact sur le propriétaire)
1. **Évolution génétique + mètre de surprise** (I1) — règle « j'entends toujours la même chose ».
2. **Université vivante** (I3) — il est prof ; personne d'autre n'a ça.
3. **Corps/geste/voix** (I2) — l'expressivité, le souffle humain.
4. **Intonation juste adaptative** (I7) — le son devient magnifique sans rien changer d'autre.
