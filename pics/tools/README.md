# Outils de démonstration

Ces outils ne font pas partie de l'application : ils servent à produire les captures d'écran et la vidéo de présentation **avec le vrai moteur**.

- `showcase/` — crate Rust. Génère des scènes avec `Session` et le moteur de rendu (`<nom>.json` = état envoyé à l'interface, `<nom>.png` = canvas rendu, `art/*.png` = documents seuls pour la vidéo). `capture.mjs` charge la vraie interface (serveur Vite) avec le cœur simulé par ces scènes et prend les captures ; `make_art.py` génère l'illustration d'exemple (œuvre originale).
- `promo/` — la vidéo (1920×1080, 30 i/s, 48 s) : `index.html` + `timeline.js` (animation déterministe : tout est fonction de l'image courante), `render.mjs` (capture image par image), `audio.py` (bande-son synthétisée, sans échantillon externe).

Reconstruire (Linux, Rust, Node, Playwright, ffmpeg, mesa-vulkan-drivers) : `bash pics/tools/build.sh <dossier de travail>`.

Honnêteté : dans les captures, la zone du canvas natif est remplacée par le rendu du moteur posé à la même place ; la vraie fenêtre Windows n'a pas été capturée. Pendant un dialogue, l'application réelle masque le canvas natif.
