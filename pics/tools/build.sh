#!/usr/bin/env bash
# Régénère scènes, captures et vidéo. Usage : build.sh <dossier de travail>
set -euo pipefail
W="$(mkdir -p "$1" && cd "$1" && pwd)"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
python3 tools/showcase/make_art.py >/dev/null
# Taille du canvas mesurée dans l'interface (voir showcase/measure.mjs) ; valeurs pour une fenêtre de 1920×1080.
cargo run -q -p openpics-showcase -- "$W/scenes" 1530 951
(cd tools/showcase && node capture.mjs "$W/scenes" "$W/captures")
mkdir -p tools/promo/assets/art tools/promo/assets/ui
cp "$W"/scenes/art/*.png tools/promo/assets/art/ && cp "$W/captures/editeur-sombre.png" tools/promo/assets/ui/
cp ../site/assets/brand/pics-icone.svg ../site/assets/brand/studioglsc-logo-fond-sombre.svg tools/promo/assets/
mkdir -p tools/promo/assets/fonts && cp ../site/assets/fonts/*.woff2 tools/promo/assets/fonts/
rm -rf "$W/frames" && mkdir -p "$W/frames"
(cd tools/promo
 for k in 0 1 2 3; do node render.mjs "$W/frames" "$k" 1439 4 & done; wait
 python3 audio.py "$W/raw.wav")
ffmpeg -y -loglevel error -i "$W/raw.wav" -af "aecho=0.8:0.6:60|120:0.25|0.15,loudnorm=I=-18:TP=-2:LRA=7" "$W/audio.wav"
ffmpeg -y -loglevel error -framerate 30 -i "$W/frames/f%05d.png" -i "$W/audio.wav" -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p -movflags +faststart -c:a aac -b:a 192k -shortest "$W/openpics-promo.mp4"
echo "vidéo : $W/openpics-promo.mp4"
