#!/bin/zsh
# Publie LOOM (web/) sur munaypassaqui.com/loom/ : copie dans le dist/ du portfolio,
# puis déploiement Cloudflare Pages (projet munay-passaqui). À relancer après chaque
# changement du web. --sans-deploiement : copie seulement (appelé par construire.sh).
set -e
LOOM="${0:A:h:h}"
SITE="$HOME/Dev/portfolio-munay"
[ -d "$SITE/dist" ] || { echo "pas de $SITE/dist : lance d'abord $SITE/construire.sh"; exit 1; }
rm -rf "$SITE/dist/loom"
mkdir -p "$SITE/dist/loom"
# exactement le dernier commit (rien de local ni de secret), sans les samples tiers
git -C "$LOOM" archive HEAD web | tar -x -C "$SITE/dist/loom" --strip-components 1 --exclude 'web/samples'
echo "dist/loom prêt : $(du -sh "$SITE/dist/loom" | cut -f1), $(git -C "$LOOM" rev-parse --short HEAD)"
[ "$1" = "--sans-deploiement" ] && exit 0
cd "$SITE" && npx wrangler pages deploy dist --project-name munay-passaqui --branch main --commit-dirty=true
