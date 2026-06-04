# LOOM — ROADMAP vers « meilleur qu'Ableton / Tidal / Strudel »

Objectif : un instrument de live-coding qui **sonne pro**, qui soit **plus beau** que tout
ce qui existe, et qui ait un **avantage unique** (IA + voix + simplicité) que les autres
n'ont pas. Effort : S (heures) · M (1 session) · L (plusieurs sessions) · XL (chantier).

---

## 🔎 Diagnostic honnête
Le frein principal au « ça sonne nul » N'EST PAS le manque de features — on en a énormément.
C'est :
1. **Synthèse basique** (oscillateurs Web Audio). → besoin d'un vrai moteur DSP.
2. **Samples lo-fi** (Dirt-Samples vieux). → besoin de sons propres / multi-échantillonnés.
3. **IA gratuite faible + je n'entends pas.** → clé Claude pour les prompts ; retours d'oreille de l'owner.

Donc la priorité absolue = **QUALITÉ AUDIO**, puis puissance du langage, puis beauté, puis IA.

---

## ✅ Ce qu'on a déjà (rappel)
Langage (degrés, ~50 gammes + microtonal, accords étendus + renversements, romains) ·
génératif (euclidien, automates cellulaires, Thue-Morse, Fibonacci, proba+seed, ratchets,
variation A/B, verbes Tidal, vraie polymétrie/phasing) · structure longue (song + @section) ·
son (ADSR, filtres, super-saw, drive, LFO, automation, reverb/delay par voix, pan, sidechain,
groove) · samples intégrés + perso · REC/RENDER WAV · MIDI out · IA VIBE + voix · UI (séquenceur
cliquable, oscillo/spectre, SCÈNE plein écran, 48 thèmes, COURS+RÉFÉRENCE) · cœur Rust testé.

---

## ❌ Ce qui nous manque vs eux
- **Ableton** : enregistrement/import audio, instruments FM/wavetable/sampler/granulaire, effets
  pro (EQ, multibande, reverb algorithmique, chorus…), mixette par piste + sends + groupes,
  automation dessinée, piano-roll, session/clips, plugins VST.
- **Tidal/Strudel** : algèbre de patterns (stack, jux, off, sometimes, chunk, striate, signaux
  continus, patterns de patterns), mini-notation riche, énorme librairie de samples + synthés
  SuperCollider, effets par événement.

---

## 🛣️ PHASES

### PHASE 0 — Qualité audio (LE plus gros levier)  ⭐ commencer ici
- [ ] 0.1 **Moteur synthé AudioWorklet** : FM (2-4 opérateurs), wavetable, sub osc, meilleures
      enveloppes, anti-alias. Remplace/augmente `tone()`. → des sons riches/pro.  **XL**
- [ ] 0.2 **Drums synthétisés pro** : kick pitch-drop + clic + corps, snare layered (corps+bruit),
      hats métalliques (FM), clap multi-tap. → ne plus dépendre des samples lo-fi.  **L**
- [ ] 0.3 **Meilleurs samples** : banque propre vérifiée (ou soundfonts/instruments
      multi-échantillonnés), + import dossier.  **M**
- [ ] 0.4 **Effets pro** : reverb algorithmique (FDN) + meilleurs IR, EQ par voix (3 bandes),
      saturation/warmth, chorus, stéréo widener.  **L**
- [ ] 0.5 **Mastering** : EQ master, compresseur multibande, limiteur soigné (déjà un début).  **M**

### PHASE 1 — Algèbre de patterns (puissance façon Tidal)
- [ ] 1.1 **Signaux continus** : `sine`/`saw`/`tri`/`rand`/`perlin` utilisables comme valeur de
      paramètre qui varie (ex. `| lp sine 0.5 200 4000`).  **L**
- [ ] 1.2 **Combinateurs** : `stack`, `jux` (sépare en stéréo + transforme un côté), `off N <verbe>`
      (couche décalée), `sometimes`/`someBy P`, `chunk N`.  **L**
- [ ] 1.3 **Mini-notation** : groupements `[a b]`, sous-séquences `[a b]/3`, alternances déjà via
      `<a/b>`. Étendre.  **L**
- [ ] 1.4 **D2 enfin** : changements de tempo/mesure par section.  **M**

### PHASE 2 — Mixette + FX par piste + automation visuelle
- [ ] 2.1 **Mixette** : volume/pan/mute/solo par voix, faders style terminal, metering.  **L**
- [ ] 2.2 **Sends partagés** : bus reverb/delay communs + niveau d'envoi par voix.  **M**
- [ ] 2.3 **Lanes d'automation visuelles** (dessiner une courbe par paramètre).  **L**

### PHASE 3 — Nouvelles vues / beauté (au-delà du terminal)
- [ ] 3.1 **Visuels WebGL/shaders** (bloom, feedback, raymarching) pour SCÈNE + oscillo.  **L**
- [ ] 3.2 **Vue modulaire à câbles** (patchbay) — magnifique et adapté.  **XL**
- [ ] 3.3 **Vue tracker** (colonnes verticales façon Renoise/ProTracker).  **L**
- [ ] 3.4 **Piano-roll** cliquable pour les mélodies.  **L**
- [ ] 3.5 **Skins d'OS** : Mac System 7 / OS 9, Windows 95/98/XP, Amiga Workbench, BeOS, NeXT…  **M**
- [ ] 3.6 **Pochette générative** par morceau + système de design (typo, micro-animations).  **M**

### PHASE 4 — IA co-producteur (NOTRE avantage décisif)
- [ ] 4.1 **Itération conversationnelle** : « plus sombre », « ajoute un drop à 2 min »,
      « dans le style de X » → édite le code existant.  **L**
- [ ] 4.2 **Génération d'arrangements longs entiers** (few-shot riche, plan de morceau).  **M**
- [ ] 4.3 **Hum-to-melody** : chante/siffle → transcription en notes.  **XL**
- [ ] 4.4 Intégration **Claude** soignée (clé) + presets de prompts par genre.  **S**

### PHASE 5 — Sortie pro / interop
- [ ] 5.1 **Export stems** (chaque voix en WAV séparé).  **M**
- [ ] 5.2 **MIDI avancé** (CC, clock, clips) + **MIDI in** (jouer au clavier).  **L**
- [ ] 5.3 **Import audio** (charger un loop, le découper).  **L**
- [ ] 5.4 **Partage cloud / galerie** de morceaux.  **M**

---

## 🎯 Recommandation d'ordre
1. **Phase 0.1 + 0.2** (moteur synthé + drums pro) — le vrai déblocage du son.
2. **Phase 1.1 + 1.2** (signaux + combinateurs) — la puissance qui dépasse Strudel.
3. **Phase 3.1 ou 3.2** (WebGL ou modulaire) — la beauté qui marque.
4. **Phase 4** (IA) en continu — l'avantage que personne n'a.

> Note : matcher la qualité audio d'un DAW est un long chemin ; mais sur l'IA, la voix,
> la beauté réactive et la simplicité, LOOM peut être **devant** dès maintenant.
