# LOOM pour Live

Écris ta musique en LOOM, Live la joue. Le device embarque le moteur de LOOM
(le même que l'appli, compilé en WebAssembly) : Live joue les mêmes notes que
LOOM, au même moment, à l'échantillon près. Le son, ce sont tes instruments Live.

## Installer

1. Glisse `LOOM pour Live.amxd` dans ta User Library (ou directement sur une piste).
2. Il faut Live 12 Suite (ou Standard avec Max for Live), Live 12.2 ou plus récent.

## Jouer

1. Crée une piste MIDI avec un instrument (Wavetable, Drift, un Drum Rack…).
2. Nomme la piste comme une voix de ton morceau : `kick`, `bass`, `pad`…
   (`LOOM kick` marche aussi).
3. Pose le device **LOOM pour Live** sur la piste, avant l'instrument.
4. Glisse ton fichier `.loom` sur le device.

Le device écrit un clip « LOOM · kick » dans la piste, de la longueur du morceau
(`song:`), en boucle. Lance-le comme n'importe quel clip.

## Coder en direct

Chaque fois que tu enregistres le fichier `.loom`, le device recuit le clip
(une demi-seconde environ). Live continue de jouer pendant ce temps.

- **Éditeur web LOOM** (Chrome ou Edge) : bouton **🎛 Live**, choisis le fichier,
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

- Les voix « audio » de LOOM (synthèse, effets, samples) ne passent pas dans Live :
  le device écrit les notes, tes instruments Live font le son.
- `tune: 19 edo` et les accordages à plus de 12 notes ne sont pas encore portés.
- Le tempo de Live est réglé une fois, au premier fichier. Les tempos par section
  passent par les scènes (`bake_live.py`).

---

# LOOM for Live (English)

Write music in LOOM, Live plays it. The device embeds LOOM's engine (the same as the
app, compiled to WebAssembly), so Live plays the same notes at the same time, sample-accurate (your Live instruments make the sound).

1. Make a MIDI track with an instrument, name it after one of your voices (`kick`, `bass`…).
2. Put **LOOM pour Live** before the instrument and drop your `.loom` file on it.
3. It writes a looping clip of the whole song. Save the file: the clip is rebuilt in
   about half a second while Live keeps playing. A code error keeps the previous clip.

Whole song at once (tracks, clips, one scene per section with its tempo), with
AbletonOSC: `loom bake song.loom song.json && python3 bake_live.py song.json`.
