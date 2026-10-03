# Plug-ins et Ableton Link : étude (3 octobre 2026)

## Ce qui marche déjà

- **Audio Units (macOS)** : `loom-audio/src/au.rs`, sans dépendance, par AudioToolbox.
  - `loom plugins` liste les instruments et les effets installés.
  - `loom au "Nom" morceau.loom sortie.wav [voix] [N]` joue les notes des voix mélodiques (ou d'une seule voix) dans un instrument AU. Le rendu est hors ligne, en WAV 24 bits.
  - Testé avec le synthé General MIDI d'Apple (`DLSMusicDevice`) : 2 tests, un accord bien rendu, silence avant la note.
- **Synchro par horloge MIDI** : elle existait déjà, en entrée et en sortie (`loom-audio/src/midi.rs`, messages 0xF8).

## Ce qui manque au prototype AU

1. **Jouer en direct** : rendre l'AU dans le rappel audio de cpal.
   - L'AU s'ouvre hors du fil audio.
   - Les notes passent par une file sans verrou.
   - `AudioUnitRender` s'appelle par blocs de 512 trames au plus.
   - Il faut un verbe `| au NOM` dans `synth.rs`. Le web l'ignorerait, puisqu'un navigateur ne peut pas charger d'AU.
2. **Effets AU** : un rappel d'entrée (`kAudioUnitProperty_SetRenderCallback`) qui fournit le son de la voix.
3. **Fenêtre du plug-in et état sauvegardé** : `kAudioUnitProperty_ClassInfo` pour l'état ; la vue Cocoa de l'AU pour la fenêtre.
4. **Plug-ins tiers** : seuls les AU d'Apple sont testés. Un plug-in tiers peut afficher une fenêtre de licence au chargement.

## VST3

- Le SDK VST3 de Steinberg est sous licence MIT depuis la version 3.8 (2025). Un hôte VST3 n'impose donc plus la GPL.
- Crates Rust :
  - `vst3` (liaisons brutes) ;
  - `vst3-host` (MIT, hôte complet) ;
  - `rack` (MIT/Apache, AU et VST3).
- `rack` compile du C++ avec **cmake**, absent de ce Mac. Il clone aussi le SDK VST3 au moment de la compilation.
- Recommandation : ajouter `vst3-host` derrière une option de compilation `vst3`, désactivée par défaut, quand le jeu en direct AU sera fait. Le chemin AU couvre déjà tous les plug-ins du Mac de Munay, qui existent tous en AU.

## Ableton Link

- Les trois crates Rust sont sous **GPL** :
  - `rusty_link` (GPL-2.0+, enveloppe officielle `abl_link`) ;
  - `ableton-link-rs` (GPL-3.0, réécriture) ;
  - `ableton-link` (0.1, abandonné).
- Link lui-même est sous GPL-2.0+. Ableton accorde une licence propriétaire sur demande, gratuite pour la plupart des applications.
- **Décision à Munay** :
  - LOOM sous GPL : Link peut entrer tel quel ;
  - LOOM sous MIT, ou un produit payant : il faut d'abord demander la licence propriétaire à Ableton (link-devs@ableton.com).
- Technique : `rusty_link` demande **cmake** (`brew install cmake`).
- Recommandation : une fois la licence tranchée, ajouter Link derrière une option `link`, désactivée par défaut.
  - Le séquenceur lit le tempo et la phase de la session (`capture_audio_session_state`) au début de chaque bloc.
  - Il cale `bar` et `phase` sur le temps Link, avec un quantum égal au nombre de temps de la mesure.
- En attendant, l'horloge MIDI synchronise déjà LOOM avec Live.
