#!/usr/bin/env bash
# Installe / met à jour open.glsc.fr SUR LE SERVEUR, depuis le dépôt public.
#
#   curl -fsSL https://raw.githubusercontent.com/gabinlsc/openpics/main/deploy/install.sh | sudo bash
#   curl -fsSL .../install.sh | sudo bash -s -- --dir /var/www/open.glsc.fr --release --nginx
#
# Options (variables d'environnement équivalentes entre parenthèses) :
#   --dir CHEMIN   dossier web (DIR, défaut /var/www/open.glsc.fr)
#   --ref REF      branche ou étiquette à déployer (REF, défaut main)
#   --release      télécharge aussi la dernière version publiée (installeurs + releases.json), SHA-256 vérifiés
#   --nginx        installe la config nginx fournie (teste avec nginx -t avant de recharger)
#   --dry-run      n'écrit rien, montre ce qui serait fait
# Relancer le script = mettre à jour. Les téléchargements déjà publiés ne sont jamais supprimés.
set -euo pipefail

REPO="${REPO:-gabinlsc/openpics}"
DIR="${DIR:-/var/www/open.glsc.fr}"
REF="${REF:-main}"
RELEASE=0 NGINX=0 DRY=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dir) DIR="$2"; shift 2 ;;
    --ref) REF="$2"; shift 2 ;;
    --release) RELEASE=1; shift ;;
    --nginx) NGINX=1; shift ;;
    --dry-run) DRY=1; shift ;;
    -h|--help) sed -n '2,13p' "$0"; exit 0 ;;
    *) echo "option inconnue : $1" >&2; exit 2 ;;
  esac
done

for c in curl tar sha256sum; do command -v "$c" >/dev/null || { echo "$c est requis" >&2; exit 1; }; done
run() { if [[ $DRY -eq 1 ]]; then echo "[simulation] $*"; else "$@"; fi; }

WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT

# 1. Le site, depuis le dépôt (TARBALL_URL permet de tester avec un fichier local).
URL="${TARBALL_URL:-https://codeload.github.com/$REPO/tar.gz/$REF}"
echo "→ téléchargement du site ($REPO @ $REF)"
curl -fsSL "$URL" -o "$WORK/src.tgz"
tar -xzf "$WORK/src.tgz" -C "$WORK"
SRC="$(find "$WORK" -mindepth 2 -maxdepth 2 -type d -name site | head -n1)"
[[ -f "$SRC/index.html" ]] || { echo "site/ introuvable dans l'archive" >&2; exit 1; }
REPO_DIR="$(dirname "$SRC")"

# 2. Copie dans le dossier web. Avec rsync : les fichiers retirés du dépôt disparaissent aussi (sauf les téléchargements).
run mkdir -p "$DIR"
if command -v rsync >/dev/null; then
  O=(-a --delete --exclude 'pics/download/' --exclude 'pics/releases.json'); [[ $DRY -eq 1 ]] && O+=(--dry-run --itemize-changes)
  rsync "${O[@]}" "$SRC/" "$DIR/"
else
  run cp -a "$SRC/." "$DIR/"
fi
echo "✓ site copié dans $DIR"

# 3. Dernière version publiée : releases.json + fichiers listés, empreintes vérifiées avant mise en ligne.
if [[ $RELEASE -eq 1 ]]; then
  BASE="${RELEASE_BASE:-https://github.com/$REPO/releases/latest/download}"
  if curl -fsSL "$BASE/releases.json" -o "$WORK/releases.json"; then
    run mkdir -p "$DIR/pics/download"
    FLAT="$(tr -d '\n ' < "$WORK/releases.json")"
    mapfile -t NAMES < <(grep -o '"name":"[^"]*"' <<<"$FLAT" | sed 's/"name":"//; s/"$//')
    for N in "${NAMES[@]}"; do
      [[ "$N" =~ ^[A-Za-z0-9._-]+$ ]] || { echo "nom de fichier suspect : $N" >&2; exit 1; }
      WANT="$(grep -o "\"name\":\"$N\"[^}]*" <<<"$FLAT" | grep -o '"sha256":"[0-9a-f]*"' | head -n1 | sed 's/"sha256":"//; s/"$//')"
      echo "→ $N"
      curl -fsSL "$BASE/$N" -o "$WORK/$N"
      GOT="$(sha256sum "$WORK/$N" | cut -d' ' -f1)"
      [[ -n "$WANT" && "$GOT" == "$WANT" ]] || { echo "empreinte incorrecte pour $N : attendue '$WANT', obtenue $GOT" >&2; exit 1; }
      run install -m 0644 "$WORK/$N" "$DIR/pics/download/$N"
    done
    run install -m 0644 "$WORK/releases.json" "$DIR/pics/releases.json"
    echo "✓ version publiée installée (${#NAMES[@]} fichier(s))"
  else
    echo "ℹ aucune release publiée pour l'instant : la page de téléchargement affichera « Bientôt »."
  fi
fi

# 4. nginx (optionnel). Config HTTP seule ; le HTTPS est ajouté par certbot. Une config existante n'est jamais écrasée
#    (certbot l'a modifiée), et une config qui ne passe pas `nginx -t` est retirée pour ne pas bloquer les autres sites.
if [[ $NGINX -eq 1 ]]; then
  [[ $EUID -eq 0 || $DRY -eq 1 ]] || { echo "--nginx demande les droits root (sudo)" >&2; exit 1; }
  CONF=/etc/nginx/sites-available/open.glsc.fr
  LINK=/etc/nginx/sites-enabled/open.glsc.fr
  if [[ -e "$CONF" ]]; then
    echo "ℹ $CONF existe déjà : conservée (certbot y a peut-être ajouté le HTTPS)."
  else
    sed "s#root /var/www/open.glsc.fr;#root $DIR;#" "$REPO_DIR/deploy/nginx.conf" > "$WORK/nginx.conf"
    run install -m 0644 "$WORK/nginx.conf" "$CONF"
    run ln -sfn "$CONF" "$LINK"
    if [[ $DRY -eq 0 ]]; then
      if nginx -t 2>/dev/null; then
        systemctl reload nginx && echo "✓ nginx rechargé (HTTP). Pour le HTTPS : certbot --nginx -d open.glsc.fr"
      else
        rm -f "$LINK"
        echo "✗ nginx -t échoue avec cette config : elle a été désactivée (aucun autre site n'est touché). Détail : nginx -t" >&2
        exit 1
      fi
    fi
  fi
fi

echo "Terminé. Vérifiez : curl -fsSI https://open.glsc.fr/pics/"
