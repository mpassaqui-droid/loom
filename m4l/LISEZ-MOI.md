# Loom pour Ableton Live

Écris ta musique en Loom, Live la joue. Le device embarque le moteur de Loom
(le même que l'appli, compilé en WebAssembly) : Live joue les mêmes notes que
Loom, au même moment, à l'échantillon près. Le son, ce sont tes instruments Live.

## La démo

Ouvre `demo/Loom demo Project/Loom demo.als` : le morceau `exemples/odyssey.loom`
déjà cuit, une piste par voix avec son instrument Live et le device Loom devant,
une scène par section. Lance les scènes de haut en bas. Pour le modifier, glisse
`odyssey.loom` sur le device d'une piste, édite, enregistre.

## Installer

Double-clique `installe.command`. Il pose le device **Loom** dans ta User Library,
et installe la fenêtre Loom (la page de Loom et son petit serveur local, qui
n'écoute que ce Mac) avec un service qui la lance à chaque session.
Il faut Live 12 Suite (ou Standard avec Max for Live), Live 12.2 ou plus récent.

## Jouer

1. Crée une piste MIDI avec un instrument (Wavetable, Drift, un Drum Rack…).
2. Nomme la piste comme une voix de ton morceau : `kick`, `bass`, `pad`…
   (`Loom kick` marche aussi).
3. Pose le device **Loom** sur la piste, avant l'instrument.
4. Clique **Ouvrir Loom** : c'est Loom en entier, comme sur le web (éditeur,
   visuels, VIBE). **RUN** écrit les clips dans Live ; **▶ Live** les lance,
   **■ Live** arrête. **son Loom** : entendre aussi le son de Loom lui-même.

Le code est celui de tout le morceau, partagé par tous les devices Loom du set :
écris-le sur n'importe quelle piste, chaque piste cuit sa voix. Il est sauvé avec
le set. Une erreur garde l'ancien clip : la musique ne s'arrête pas.

Tu préfères ton éditeur ? Glisse un fichier `.loom` sur la zone « fichier » :
le device suit ses enregistrements.

Le device écrit un clip « Loom · kick » dans la piste, de la longueur du morceau
(`song:`), en boucle. Si la piste n'a plus d'emplacement libre, il ajoute une scène en bas.

## VIBE (l'IA)

Décris ta musique, Claude écrit le code, Loom le vérifie avant de le jouer.
Colle ta clé Anthropic dans VIBE (clé), ou, si Claude Code est installé sur ce
Mac, rien à faire : la fenêtre passe par lui.

## Relier Live et Loom

- `@bass.Filter Freq: 0 .25 .5 1 | slow 2` : l'automation du paramètre « Filter Freq »
  d'un device de la piste `bass`, écrite dans le clip, un pas par valeur
  (0 = minimum, 1 = maximum, `.` = garde la valeur). Sans `bass.`, toutes les
  pistes Loom qui ont ce paramètre le suivent.
- `go: drop` : quand tu joues le code, Live lance la scène « drop » (`go: 3` = la 3e).
- **⇣ clip Live** (dans la fenêtre) : le clip MIDI sélectionné dans Live devient
  une ligne de code en bas de l'éditeur.

## Voir

Le device montre le piano-roll de sa voix, avec la tête de lecture du clip : les
notes s'allument quand elles sonnent. Clique dessus : la scène s'ouvre dans une
fenêtre, tout le morceau, chaque voix dans sa couleur tire ses particules quand
elle joue (comme la scène de l'éditeur web).

## Coder en direct

Chaque fois que tu enregistres le fichier `.loom`, le device recuit le clip
(une demi-seconde environ). Live continue de jouer pendant ce temps.

- **Éditeur web Loom** (Chrome ou Edge) : bouton **🎛 Live**, choisis le fichier,
  puis chaque **RUN** l'écrit.
- **Terminal** : `loom morceau.loom`, puis Ctrl+S.
- **N'importe quel éditeur de texte** : enregistrer suffit.

Si ton code a une erreur, le clip d'avant reste en place : la musique ne s'arrête jamais.

## Tout le morceau d'un coup

Pour créer toutes les pistes, les clips et les scènes (une par section, avec son
tempo) en une fois, avec [AbletonOSC](https://github.com/ideoforms/AbletonOSC) installé :

    loom bake morceau.loom morceau.json
    python3 bake_live.py morceau.json

## Accordages

`tune: just`, `pyth`, `werck`, `kirn`, `meantone` : `loom bake` écrit le fichier
d'accordage Live (`.ascl`) dans `User Library/Tunings`. Glisse-le sur le set.

## Limites connues

- Les voix « audio » de Loom (synthèse, effets, samples) ne passent pas dans Live :
  le device écrit les notes, tes instruments Live font le son.
- `tune: 19 edo` et les accordages à plus de 12 notes ne sont pas encore portés.
- Le tempo de Live est réglé une fois, au premier fichier. Les tempos par section
  passent par les scènes (`bake_live.py`).

---

# Loom for Ableton Live (English)

Write music in Loom, Live plays it. The device embeds Loom's engine (the same as the
app, compiled to WebAssembly), so Live plays the same notes at the same time, sample-accurate (your Live instruments make the sound).

1. Make a MIDI track with an instrument, name it after one of your voices (`kick`, `bass`…).
2. Put **Loom** before the instrument, write your code in its editor, press **Tab**.
   The code is the whole song, shared by every Loom device in the set and saved with it.
   You can also link a `.loom` file and use any editor.
3. It writes a looping clip of the whole song. Save the file: the clip is rebuilt in
   about half a second while Live keeps playing. A code error keeps the previous clip.

Whole song at once (tracks, clips, one scene per section with its tempo), with
AbletonOSC: `loom bake song.loom song.json && python3 bake_live.py song.json`.
