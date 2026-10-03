# Leçons — LOOM (cycle d'auto-amélioration)

> Règle : **une leçon par entrée**, mise à jour après CHAQUE correction du
> propriétaire (erreur corrigée OU approche confirmée). Quand une leçon devient
> une règle permanente, la promouvoir dans `CLAUDE.md` et la retirer d'ici.
> À relire au début de chaque session.

Format :
```
# <sujet court>
lesson: <la règle qui évite de refaire l'erreur>
# ou
confirmed: <l'approche validée>
```

---

# Tests core obligatoires
confirmed: toute feature de langage passe par loom-core d'abord (parser + tests, `cargo test -p loom-core`), puis exposition dans web/index.html + mise à jour du prompt système de l'IA.

# Finition demandée explicitement
lesson: quand le propriétaire redemande de « rendre parfait » après un rapport de limites connues, traiter ces limites comme du travail à finir et vérifier les portes de qualité avant de conclure.

# Silence avant tout test MIDI avec Live ouvert
lesson: avant de lancer `loom play` (même avec des voix `| midi`), Live ouvert = son garanti : le set par défaut arme une piste MIDI en « All Ins » et LOOM envoie aussi vers le bus IAC. Toujours d'abord master Live à 0 et pistes désarmées (AbletonOSC), puis tester. Munay a entendu le test le 03/10 la nuit.
