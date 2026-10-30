#!/usr/bin/env bash
# Déploie le site statique (open.glsc.fr) par rsync sur SSH.
# Usage : DEPLOY_HOST=user@serveur DEPLOY_PATH=/var/www/open.glsc.fr deploy/deploy-site.sh [--dry-run]
# Les binaires (pics/download/) et pics/releases.json sont exclus de --delete : ils sont
# publiés par deploy/publish-release.sh, pas par le site.
set -euo pipefail

: "${DEPLOY_HOST:?DEPLOY_HOST requis (ex. user@serveur)}"
: "${DEPLOY_PATH:?DEPLOY_PATH requis (ex. /var/www/open.glsc.fr)}"

root="$(cd "$(dirname "$0")/.." && pwd)"
extra=()
[[ "${1:-}" == "--dry-run" ]] && extra+=(--dry-run)

rsync -az --delete --itemize-changes "${extra[@]}" \
  --exclude 'pics/download/' --exclude 'pics/releases.json' \
  "$root/site/" "$DEPLOY_HOST:$DEPLOY_PATH/"
echo "Site déployé sur $DEPLOY_HOST:$DEPLOY_PATH"
