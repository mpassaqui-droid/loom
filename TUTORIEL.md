# 🎵 LOOM — Le tutoriel complet

Tout ce que tu peux écrire dans LOOM, du premier `x` jusqu'au microtonal
génératif. Chaque section a un exemple que tu peux **coller dans l'éditeur et
RUN** tout de suite. Garde cette page à côté de toi.

> **La règle d'or, une seule :**
> ```
> nom: motif | verbe | timbre | effet | ...
> ```
> Une **ligne = une voix**. Le **nom** à gauche, le **motif** au milieu, puis
> autant de **`| réglages`** que tu veux. Le motif remplit **une mesure**.
> `#` en début de ligne = commentaire (ignoré).

---

## 1. La batterie

Quatre noms sont des percussions : **`kick snare hat clap`**.
`x` = une frappe, `.` = un silence.

```
kick:  x . . . x . . .
snare: . . x . . . x .
hat:   x x x x x x x x
clap:  . . . . x . . .
```

---

## 2. Les mélodies

**Tout autre nom** (lead, bass, pad…) joue des notes.

- **Notes absolues** : lettre `a`–`g`, `#`/`b`, octave `1`–`5` → `c3 eb2 g4`
- **`. `** = silence (comme en batterie)

```
lead: c3 e3 g3 c4 g3 e3 c3 .
```

---

## 3. Tonalité & degrés

Déclare une **`key:`** puis écris en **degrés 1–7** (1 = tonique). La gamme
choisit les bonnes notes pour toi.

- **`'`** monte d'une octave, **`,`** descend → `1'`, `5,`
- **Altération en SUFFIXE** : `7#` (hausser), `3b 5b` (baisser).
  ⚠️ jamais en préfixe (`b3` serait la **note** Si).
- **Bend en cents** : `3+50` (50 cents plus aigu), `c3-30`.

```
key:  c minor
bass: 1 . 3 . 5 . 3 . | oct 2
lead: 1 5, 1' 7# 5 3 1 .
```

---

## 4. Les gammes

Change la gamme, la même mélodie change de couleur.

- **Occidentales** : `major minor dorian phrygian lydian mixolydian locrian`
  `pentatonic minpent harmonic_minor melodic_minor whole_tone blues chromatic`
- **Du monde** : `hijaz hijazkar nahawand kurd ajam bhairav yaman todi marwa
  purvi slendro pelog hirajoshi insen iwato egyptian`
- **Microtonales (vrais quarts de ton)** : `bayati rast saba sikah huzam jiharkah`

```
key:  e phrygian
lead: 1 2 3 4 5 4 3 2 | pulse
```

---

## 5. Les accords

Une **MAJUSCULE** joue un accord entier (minuscule = note simple).

- **Qualités** : `5 maj m dim aug sus2 sus4 6 m6 6/9 7 maj7 m7 mmaj7 m7b5 dim7
  7b5 7#5 7b9 7#9 7#11 add9 add11 9 maj9 m9 11 m11 13 maj13 m13`
- **Slash / renversement** : `Cm7/G`, `C/E` (la note après `/` va à la basse)
- **Chiffres romains** (avec une key) : `I ii iii IV V vi vii` (+`7`)
- **Arpège** : `| arp up` (`down` / `updown` / `downup`)
- **Harmoniseur** : `| chord m7` → chaque note devient un accord

```
key:  c major
keys: I . vi . IV . V7 . | sine | gain 0.25
arp:  Am . F . C . G . | arp up | saw | gain 0.2
```

### Progression + accord-suivi (`^`)
`prog:` pose une suite d'accords (un par mesure). Le token **`^`** = l'accord
courant ; **`^N`** = sa Nᵉ note (`^1` = fondamentale, idéal pour la basse).

```
key:  a minor
prog: Am F C G
bass: ^1 . . . ^1 . 5, . | sine | oct 2 | gain 0.3
pad:  ^ . . . | saw | unison 4 | lp 1200 | gain 0.15
```

---

## 6. Le rythme avancé

| Écriture | Effet |
|---|---|
| `[a b]` | **sous-séquence** : subdivise un pas (`x [x x]` = 2ᵉ pas 2× plus vite) |
| `a b ; c d` | **stack** : plusieurs lignes en même temps sur UNE voix |
| `x!  x!!` | **accent** (vélocité plus forte) |
| `x*3` | **ratchet** : 3 frappes dans le pas |
| `x*8>` / `x*8<` | **roll** crescendo / decrescendo |
| `x?` / `x?70` | **probabilité** (~50 % / 70 %) |
| `{a b c}` | **aléatoire** : tire une alternative (reproductible via `seed`) |
| `<a/b/c>` | **alternance** : une version par mesure |

### Générateurs mathématiques
| Token | Rythme |
|---|---|
| `e(k,n)` `e(k,n,rot)` | **euclidien** (k frappes réparties sur n pas) |
| `ca(rule,n)` | **automate de Wolfram** (évolue chaque mesure ; rules 90 30 110 18 150) |
| `morse(n)` `fib(n)` | suites de **Thue-Morse / Fibonacci** (auto-similaires) |

```
seed:  5
kick:  e(5,16)
hat:   ca(90,16) | gain 0.16
snare: . . . . x . . . . . . . x . x*3 .
perc:  x?80 x?40 x x?60 x?80 x?40 x x?50 | sample perc | gain 0.16
```

---

## 7. Les verbes (transforment le motif)

Après un `|`, on enchaîne des verbes :

| Verbe | Effet |
|---|---|
| `rev` | inverse le motif |
| `fast N` / `slow N` | accélère N× / une mesure sur N |
| `every N <verbe>` | applique un verbe une mesure sur N |
| `jux <verbe>` | original à gauche, version transformée à droite (stéréo) |
| `chunk N <verbe>` | applique le verbe à 1/N du motif, en tournant (variation) |
| `off N <verbe>` | couche décalée de N pas, plus douce (canon/écho) |
| `add N` | transpose les degrés de N pas de gamme (`jux add 2` = tierce à droite) |
| `palindrome` | endroit une mesure, envers la suivante |
| `sometimes <v>` | applique au hasard (`often` 75 % · `rarely` 25 % · `someby P`) |
| `rot N` / `iter N` | rotation fixe / décalage d'un pas par mesure |
| `poly N` | **vraie polymétrie** : tourne à N pas/mesure et dérive (phasing) |

```
key:  a minor
bass: 1 5, 1' 5 4 1 5 3 | saw | oct 2 | every 4 rev | gain 0.15
arp:  1 2 3 5 | jux add 2 | pluck 0.7 | gain 0.12
```

---

## 8. La synthèse (le timbre)

### Formes d'onde de base
`| saw` `| sine` `| tri` `| pulse` `| square`

### Oscillateurs riches
| Réglage | Son |
|---|---|
| `| unison N` `| detune C` `| spread N` | super-saw (oscillateurs désaccordés, étalés en stéréo) |
| `| supersaw` | macro instantanée (saw + unison 7 + detune + spread) |
| `| fm RATIO INDEX` | **FM** : cloches, e-piano, basses grasses (index 1–6) |
| `| wt organ\|warm\|rich\|soft` | **table d'ondes** |
| `| pwm N` | largeur d'impulsion variable |
| `| sync N` | **hard-sync** qui balaye (lead rave/acid) |
| `| fold N` | **wavefolding** (West-coast, harmoniques mouvants) |
| `| ring FREQ` | modulation en **anneau** (métallique/robot) |
| `| am FREQ` | modulation d'amplitude (trémolo/bandes, plus doux) |

### Modèles physiques & spéciaux
| Réglage | Son |
|---|---|
| `| pluck N` | corde pincée **Karplus-Strong** (guitare/harpe/koto ; N = brillance) |
| `| bow` | corde **frottée** (violon/violoncelle/drone) |
| `| reed` | **anche** (clarinette/hautbois, creux et boisé) |
| `| bell` | synthèse **modale** (cloche/métal/gamelan) |
| `| grain N` | **granulaire** (nuages de micro-grains, textures) |
| `| vowel a/e/i/o/u` | filtre de **formants** (voix/chœur « aah/eeh ») |
| `| vocode SAMPLE` | **vocodeur** : la voix « parle » avec `vox`/`yeah`/`breath` |

### Couches & corps
`| sub N` (sub-octave, basses) · `| double N` (octave au-dessus, brillance) ·
`| noise N` (souffle/air) · `| semi N` (transpose la voix de N demi-tons)

```
key:  d minor
epiano: 1 3 5 3 | sine | fm 1 2 | gain 0.12
harp:   1 2 3 5 3 2 | pluck 0.7 | reverb 0.4 | gain 0.12
choir:  1 . . . | saw | unison 4 | vowel a | gain 0.1
```

---

## 9. Filtre & enveloppe

| Réglage | Effet |
|---|---|
| `| lp N` `| hp N` (`q R`) | passe-bas / passe-haut (+ résonance) |
| `| bp N` `| notch N` (`q R`) | passe-bande / réjecteur (voix nasales, wah) |
| `| fenv N` | enveloppe de **filtre** (la coupure suit une ADSR : acid/plucky) |
| `| env A D S R` | enveloppe d'**ampli** (attaque, déclin, sustain 0-1, release) |
| `| gate N` | longueur des notes (0.05–4 : <1 staccato, >1 tenu) |

```
key: c minor
pad: Cm7 . . . Ab . . . | saw | lp 800 q 6 | env 0.4 0.3 0.7 1.2 | gain 0.16
```

---

## 10. Le mouvement (modulation & automation)

| Écriture | Effet |
|---|---|
| `| lfo cutoff\|pitch\|gain RATE DEPTH` | LFO (mouvement, vibrato, trémolo) |
| `| lp 300..6000` (`| over N`) | **rampe** : un param glisse de A à B (filtre qui s'ouvre, riser) |
| `| pan -1..1` `| drive 0..0.6` | n'importe quel param `A..B` évolue sur la section |
| `| lp sine 0.5 300 4000` | **signal** : param qui ondule — WAVE(`sine saw tri square rand`) RATE MIN MAX |
| `| slide N` | portamento (glisse depuis la note précédente, son 303) |
| `| bend N [T]` | pitch-bend (démarre N demi-tons plus loin, glisse en T s : kick drop, 808) |
| `| wow N` | wow & flutter (pleurage de bande, lo-fi vintage) |
| `| orbit RATE` | la voix tourne autour de l'auditeur (mouvement stéréo) |

```
key:  f minor
riser: 1 1 1 1 1 1 1 1 | saw | unison 5 | lp 300..5000 | over 8 | gain 0.12
acid:  1 5, 1' 5 4 1 5 3 | saw | oct 2 | slide | lp 500 q 9 | fenv 0.7 | gain 0.14
```

---

## 11. Effets & mix (par voix)

| Réglage | Effet |
|---|---|
| `| gain N` | volume (0–1) |
| `| pan P` | panoramique (-1 gauche … 1 droite) |
| `| reverb N` `| delay N` | sends d'écho/réverbe (0–1) |
| `| eq L M H` | EQ 3 bandes en dB (grave/médium/aigu) |
| `| chorus N` `| phaser N` | élargissement / balayage psyché |
| `| comp N` | compression (punch & glue) |
| `| drive X` | distorsion / chaleur |
| `| crush BITS` | bitcrush 1–8 (lo-fi/IDM) |
| `| redux N` | réduction d'échantillonnage (digital crunch, ≠ crush) |
| `| flange RATE [DEPTH]` | flanger (swoosh) |
| `| reson FREQ [DECAY]` | résonateur en peigne accordé (métal/Karplus) |
| `| stereo N` | widener stéréo M/S (0–2) |
| `| swing N` `| shift N` | swing par voix / microtiming (retard ±) |

### Réglages globaux (directives)
`duck: N` (sidechain : tout pompe sur le kick) · `feel: N` (humanise le timing) ·
`warmth: 0.5` (saturation chaleur du master)

```
duck: 60
key:  a minor
kick: x . . . x . . . x . . . x . . .
pad:  Am7 . . . F . . . | saw | lp 1100 | crush 6 | gain 0.18
```

---

## 12. La structure d'un morceau

- **`bpm: 128`** : tempo · **`tempo: intro 120 drop 140`** : tempo par section
- **`song: intro*4 drop*8 break*4`** : l'arrangement (nom\*mesures, en boucle)
- **`| in verse chorus`** : la voix ne joue que dans ces sections
- **`nom@section: motif`** : un motif **différent** par section → morceaux longs

```
song: intro*2 drop*4 outro*2
key:  a minor
kick: x . . . x . . . | in drop outro
arp:  Am . F . C . G . | arp up | saw | gain 0.18 | in drop
arp@outro: Am . . . | saw | gain 0.12
pad:  Am . . . | sine | gain 0.2
```

---

## 13. L'accordage (microtonal)

**`tune:`** change l'intonation de toutes les hauteurs :
- `just` (intonation juste, ratios purs) · `pyth` (Pythagore, quintes pures)
- `Nedo` (N divisions égales : `19edo`, `31edo`…) · `equal` (tempérament égal, défaut)

```
key:  c major
tune: just
keys: I . IV . V . I . | sine | gain 0.2
```

---

## 14. Les samples

- **Batterie** : `bd sd hh cp kick2 kick909 clubkick clap2 oh rim cow perc tom
  tom2 tom3 ride tabla`
- **Mélodiques** (repitchés sur les degrés) : `bass arpy gtr casio sax sitar
  juno hoover stab bleep sid voodoo`
- **Loops / voix** : `amen break amencut` · `vox yeah breath wood metal glass bottle`
- **`| root NOTE`** : accorde un sample mélodique à l'oreille
- **Tes fichiers** : panneau « mes sons » → dépose un audio → `| sample <ton_nom>`

```
key:   c minor
kick:  x . . . x . . . | sample bd
break: 1 . 2 . 3 . 1 . | sample amen | gain 0.3
sitar: 1 . 2 3 . 4 5 . | sample sitar | gain 0.4
```

---

## 15. Le visuel

- **`scene:`** choisit le visualiseur plein écran (touche **N** pour changer).
  72 scènes : `radial tunnel starfield kaleido aurora julia plasma mandelbrot
  lissajous fire matrix harmono clifford lorenz reaction tracker metatron …`
- **`| viz burst|ring|beam|flash|shape`** et **`| hue N`** : apparence/couleur
  d'une voix dans la scène.

```
scene: lorenz
key:   a minor
lead:  1 5 1' 3' 5 1' | pluck 0.7 | hue 200 | viz ring | gain 0.15
```

---

## 16. Exporter & partager

- **REC** : enregistre en live · **RENDER** : bounce tout le morceau en **WAV**
- **FILM** : exporte une **vidéo** ; `film: vertical|square|wide` (format) ;
  `title: Mon morceau` (carte-titre en fondu)
- **SAVE / LOAD / DEL** : ta bibliothèque locale + un code de partage `LOOM1:…`
- **`| midi CH`** : envoie la voix en MIDI (la batterie part sur le canal 10)

---

## 17. Le cerveau IA

- **VIBE** (ou le micro 🎤) : décris la musique en mots, l'IA écrit le code LOOM.
- **look ▸** / 🎲 : 48 thèmes de terminal pour l'apparence (mémorisés).

---

## 18. Un morceau complet, commenté

Colle-le, RUN, puis change une chose à la fois pour voir l'effet.

```
# ---- réglages globaux ----
title: Demo
bpm:   120
key:   a minor
prog:  Am F C G          # l'harmonie ; ^ la suit
duck:  45                # sidechain
feel:  18                # ça respire
scene: harmono
song:  intro*2 main*8 break*4 main*8 outro*2

# ---- batterie ----
kick:  x . . . x . . . x . . . x . . . | sample bd | gain 0.5 | comp 0.4 | in main outro
snare: . . . . x . . . . . . . x . x*3 . | sample sd | gain 0.4 | in main outro
hat:   ca(90,16) | sample hh | gain 0.12 | swing 56 | in main break outro

# ---- basse (suit la prog) ----
bass:  ^1 . . . 5, . ^1 . ^1 . . . 6, . ^1 . | saw | oct 2 | sub 0.5 | lp 700 | slide | gain 0.15 | in main outro

# ---- harmonie & mélodie ----
keys:  ^ . . . ^ . . . | wt warm | env 0.02 0.2 0.6 0.4 | reverb 0.4 | gain 0.1 | in main
arp:   ^1 ^2 ^3 ^2 | arp up | pluck 0.7 | delay 0.3 | gain 0.1 | in main break
lead@break: 1' . 7 . 6 . 5 . | reed | slide | reverb 0.5 | gain 0.12

# ---- nappe (partout) ----
pad:   ^ . . . | saw | unison 4 | vowel a | lp 1300 | reverb 0.7 | gate 1.8 | gain 0.06
```

---

## 🎛️ Pour explorer plus loin

Charge les **presets ★ Vitrines** (chacun expose une facette) :
**CLAVIERS** (synthèses), **BASSES** (designs de basse), **DESIGN** (sound
design), **EXPOSE** (IDM génératif), **AUTECHRE** (polymétrie/phasing),
**TAQSIM** (maqâm microtonal), **GAMELAN** (slendro), **MIXTAPE** (la suite qui
traverse tout). Bouton **⏭** pour les enchaîner.

*Tout est aussi dans le panneau **APPRENDRE** de l'app (leçons cliquables) et
dans la **recherche de référence** (la barre cherche n'importe quel mot-clé).*
