#!/usr/bin/env bash
# Publie une version d'OpenPics : copie les binaires, calcule les SHA-256, écrit releases.json.
# Usage : DEPLOY_HOST=... DEPLOY_PATH=... deploy/publish-release.sh <version> <installeur.msi|exe> <portable.zip>
set -euo pipefail

: "${DEPLOY_HOST:?DEPLOY_HOST requis}"
: "${DEPLOY_PATH:?DEPLOY_PATH requis}"
[[ $# -eq 3 ]] || { echo "Usage : $0 <version> <installeur> <portable.zip>" >&2; exit 2; }
version="$1"; installer="$2"; portable="$3"
for f in "$installer" "$portable"; do [[ -f "$f" ]] || { echo "Introuvable : $f" >&2; exit 1; }; done

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
entry() { # label fichier
  printf '{"label":"%s","name":"%s","size":%s,"sha256":"%s"}' \
    "$1" "$(basename "$2")" "$(stat -c %s "$2")" "$(sha256sum "$2" | cut -d' ' -f1)"
}
printf '{"version":"%s","files":[%s,%s]}\n' "$version" \
  "$(entry 'Installeur Windows (x64)' "$installer")" \
  "$(entry 'Version portable (x64)' "$portable")" > "$tmp/releases.json"

# Binaires d'abord, manifeste en dernier : la page n'annonce jamais un fichier absent.
ssh "$DEPLOY_HOST" "mkdir -p '$DEPLOY_PATH/pics/download'"
rsync -az "$installer" "$portable" "$DEPLOY_HOST:$DEPLOY_PATH/pics/download/"
rsync -az "$tmp/releases.json" "$DEPLOY_HOST:$DEPLOY_PATH/pics/releases.json"
echo "Version $version publiée."
