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
