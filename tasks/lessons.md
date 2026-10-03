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

## 2026-10-03 · Le hook anti-secret lit l'index AVANT la commande
`git add X && git commit` dans une seule commande : le hook tourne avant le `git add`,
donc il ne voit pas X. Les commits de m4l/loom_device.js passaient par accident, la ligne
`'x-api-key: ' + k` (en-tête construit avec une variable, pas une clé) ressemble à un secret.
Règle : stager d'abord, committer ensuite. Si le hook bloque sur un faux positif, ne pas
contourner : laisser le commit à Munay avec la commande exacte.

## 2026-10-03 · Vérifier une fenêtre Max soi-même avant de dire « fait »
J'ai annoncé « la page suit la fenêtre » sans l'avoir vue : le `expr $i3-$i1; $i4-$i2`
(syntaxe Pd) ne marche pas dans Max, un expr = une formule. Règle : build de test,
`/loom/fenetre` par UDP, `app_screenshot` de la fenêtre « Loom » (et son bouton plein écran),
PUIS remettre le build de vente. Munay ne doit pas servir de testeur.
