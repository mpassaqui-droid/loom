# 🌌 LOOM — Le plan (vision + ce qui manque + ce qu'on peut ajouter)

> **La vision** : un instrument unique qui réunit le **DAW** (studio), le **code**
> (live-coding), le **visuel**, les **maths**, la **philosophie** et **toute la
> connaissance** — mais entièrement **au service de la musique**.
> On y va en COUCHES, jamais tout d'un coup ; la connaissance vit dans le modèle
> IA + le prompt + une langue qui grandit (on ne code pas « toute la musique » en dur).

Légende : ✅ fait · 🔨 prochaine étape réaliste · 🌠 grand chantier / ambition

---

## 0. Où on en est (la base, déjà solide)
- **Langage** : notes, degrés, ~70 gammes (+ perso numériques), microtonal
  (just/pyth/N-edo/maqâm en quarts de ton/Werckmeister/Kirnberger/méantone),
  accords (qualités, romains, slash, `prog`, `^`, 8 arpèges, voicings,
  voice-leading, `harmonize`, harmoniseur), rythme (euclide, automates `ca()`,
  `morse/fib`, ratchets, rolls, proba, aléatoire `{}`, alternance `<>`,
  sous-séquences `[]`, stacks `;`, accents, liaisons `_`), verbes (rev, every,
  jux, chunk, off, add, semi, palindrome, sometimes, rot, iter, poly, markov,
  invert, sequence, mutate, stutter, cresc, dim), structure (`song`, `@section`,
  `in`, tempo par section, `meter`, modulation `key@section`, `accel`, `groove`, `chance`).
- **Synthèse** : soustractive, FM, table d'ondes, **additive**, granulaire,
  modèles physiques (pluck/bow/reed), modale (bell), hard-sync, wavefold,
  ring/AM, PWM, vocodeur, formants.
- **Effets** : filtres + fenv, ADSR, **2 LFO** (5 cibles × 4 formes), automation
  `A..B`, signaux, drive, crush, redux, chorus, phaser, comp, eq, reverb, delay,
  **echo sync**, **convolution `ir`**, flange, reson, stereo, haas, autopan,
  tgate, slide, bend, wow, orbit, sub/double/noise, `tape`, swing/shift/humanize,
  gate, strum, flam, sidechain, warmth.
- **Sampler** : lecture, `chop`, `reverse`, `begin/end`, `root`, fichiers perso, granulaire.
- **Visuel** : **95 scènes** (Canvas2D + 4 shaders **GPU**), viz/couleur par voix,
  code à l'écran, **fondus** entre scènes, **mode VJ**, export **vidéo (FILM)**.
- **Export / E-S** : WAV, stems, **MIDI (fichier + out + in clavier)**,
  **partage URL**, save/load, **sauvegarde auto**.
- **App** : **✨ GÉNÉRER**, **IA (VIBE) gratuite sans clé** + chips, **linter**,
  **autocomplétion** (Tab), raccourcis, **menu de langues** (11 + 100+),
  thèmes, **mobile**, 56 leçons, référence, tutoriel.
- **Rust** : `loom-core` (parseur/scheduler testé) — **en retard** sur le moteur JS.

---

## 1. 🎚️ DAW — devenir un vrai studio  *(le plus gros écart)*
- 🔨 **Mixeur visuel** : faders, pan, VU-mètres (M/S existent).
- 🔨 **Bus & sends** : groupes + returns d'effets partagés.
- 🔨 **Métrage pro** : LUFS / crête / corrélation, cible de loudness.
- 🔨 **Mastering** : EQ + multibande + limiteur soigné.
- 🔨 **Sampler instrument** : charger des fichiers, slice/loop à la souris,
  multisample, ADSR visuel, **time-stretch**.
- 🌠 **Timeline / arrangement** visuel (clips qu'on glisse, automation dessinée).
- 🌠 **Enregistrement audio** (micro/ligne, overdub).
- 🌠 **Clip launcher / vue Session** (live, lancement quantisé, tap-tempo).
- 🌠 **MIDI complet** : clock sync, MPE, mapping CC, enregistrer en pattern.
- 🌠 **Plugins** (Web Audio Modules) / LOOM exporté en plugin.
- 🌠 **Projets** multi-morceaux + versions + cloud.

## 2. ⌨️ CODE / LANGAGE — la langue qui grandit
- 🔨 **Fonctions / macros / variables** (motifs et verbes définis par toi).
- 🔨 **Polyrythmie explicite** (`3:2`), métrique composée, modulation métrique.
- 🔨 **Undo/redo** des chargements (preset / IA / génération).
- 🌠 **Théorie du monde en profondeur** : raga (aroha/avaroha, pakad, vadi),
  maqâm (ajnas, sayr), gamelan (pathet), talas carnatiques, timelines africaines.
- 🌠 **Harmonie fonctionnelle** : dominantes secondaires, emprunts, réharmonisation.
- 🌠 **Contrepoint** : espèces, canon/fugue, harmoniser une mélodie.
- 🌠 **Paroles / chant** : texte → rythme, synthèse de voix chantée.
- 🌠 **Accordages** : import Scala (.scl), juste intonation adaptative.

## 3. 🎬 VISUEL — le saut GPU continue
- 🔨 **Post-traitement GPU** : bloom, feedback, glitch.
- 🔨 **Plus de shaders** : raymarching 3D, fractales, néons.
- 🔨 **Vue partition + piano-roll** (lire/éditer en notation, pas qu'en code).
- 🌠 **Éditeur de shader** intégré (écris ton GLSL réactif au son).
- 🌠 **Export vidéo pro** (MP4/4K, GIF, transparent).
- 🌠 **Sortie scène** (Syphon/Spout, visuels pilotés par MIDI).
- 🌠 **Générateur de pochette** par morceau.

## 4. 🔢 MATHS — la musique comme structure
- ✅ euclide, automates, morse/fib, Markov, chaos, microtonal.
- 🔨 **Markov d'ordre N**, chaînes apprises d'un morceau.
- 🌠 **Composition algorithmique** : L-systèmes, grammaires, stochastique (Xenakis),
  génétique/évolutionnaire, RNN/IA mélodique.
- 🌠 **Résolution sous contraintes** (« une mélodie qui respecte ces règles »).
- 🌠 **Maths de l'accordage** : lattices, comma pumps, explorateur N-EDO, spectres → accords.
- 🌠 **Atelier DSP** (construire son synthé depuis les maths).
- 🌠 **Théorie de l'information** : entropie/complexité, auto-similarité, surprise.

## 5. 🏛️ PHILOSOPHIE / CONNAISSANCE — « tout, pour la musique »  *(le plus singulier)*
- 🔨 **Base de connaissance intégrée** : chaque gamme/tradition avec son **histoire,
  son contexte et son sens** (rasa indien, ethos des modes grecs, humeurs des
  maqâmat, wuxing chinois ↔ notes).
- 🔨 **L'explicateur « pourquoi ça sonne »** : consonance, tension/détente, attente
  (Huron/Meyer), psychoacoustique — l'IA + la théorie.
- 🌠 **Musique & nombre / cosmos** (Pythagore, harmonie des sphères, ratios justes).
- 🌠 **Musique & émotion & langage**.
- 🌠 **Une « université de la musique » dans l'app** : parcours interactifs reliant
  théorie, histoire, acoustique, maths et philosophie — sources citées.

## 6. 🧠 LE CERVEAU IA — le tissu qui relie tout
- ✅ composer depuis une description (gratuit, sans clé), chips d'action, voix, prompt à jour.
- 🔨 **Explique mon code**, **corrige**, **continue**, **réharmonise**, **« à la manière de X »**, **critique**.
- 🌠 **Prof interactif** qui connaît TOUTE la base (théorie + histoire + philosophie)
  et l'**applique** : il explique ET modifie ton morceau pour te le faire entendre.
- 🌠 **Conversation multi-tours**, mémoire du projet, modèle local optionnel.

## 7. 🛠️ PLATEFORME / FONDATIONS
- 🌠 **Moteur unifié Rust→WASM** : web + terminal + téléphone sur UN moteur
  (sample-accurate, latence basse) — la grande dette technique.
- 🔨 **Accessibilité** (lecteur d'écran, daltoniens, grandes polices).
- 🔨 **PWA** (installer comme app).
- 🌠 **Communauté** : galerie, packs, **collaboration temps réel**.
- 🔨 **CI / tests / docs**.

---

## 🎯 Ordre conseillé (impact / effort)
1. **Vue partition / piano-roll** + **mixeur visuel** → ça *ressemble* enfin à un DAW.
2. **Base de connaissance + explicateur IA** → le cœur de ta vision (relier maths/philo/théorie à l'écoute).
3. **Post-traitement GPU (bloom) + shaders** → le « waouh » visuel continue.
4. **Sampler instrument (slice/multisample/stretch)** → vrai studio.
5. **Fonctions / macros** → la langue devient extensible par toi.
6. **Moteur Rust unifié** (fond) → durable partout.

> Rien n'est bloqué par une clé : l'IA marche déjà gratuitement (Puter/Pollinations).
> Chaque ligne ci-dessus peut devenir une « vague » concrète, testée et commitée.
