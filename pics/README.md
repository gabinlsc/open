# OpenPics

Éditeur d'images libre (GPL-3.0-only), 100 % local : pas de compte, pas de télémétrie. Windows 10/11 x64.

## Ce qu'il fait
- **Documents** : calques (raster, groupes, réglage, texte, formes), masques, 27 modes de fusion, styles, historique illimité, onglets, repères, règles, grille, aimantation, récupération après plantage.
- **Sélections et peinture** : rectangle, ellipse, lasso, baguette magique, pinceau, gomme, dégradé, pot de peinture, tampon, correcteurs, pièce, doigt, éponge, fluidité.
- **Réglages** (non destructifs) : 15 types dont courbes, niveaux, LUT 3D `.cube`, correspondance de couleurs. **Filtres** : 21, avec aperçu en direct.
- **Texte et vectoriel** : texte éditable (paragraphe, sur tracé), formes, plume, opérations booléennes, SVG (import raster, export vectoriel).
- **IA locale** : sélection du sujet et suppression d'arrière-plan par modèle ONNX (à fournir) ou méthode classique ; comblement par diffusion.
- **Productivité** : palette Ctrl+K, scripts (Rhai) et enregistreur d'actions, traitement par lot, taille d'image (Lanczos), export multi-échelles, comparaison avec l'original, explorateur de dossier, interface FR/EN (menus et outils).
- **Formats** : PNG, JPEG, WebP, TIFF, BMP, GIF, et `.ocv` natif (calques, masques, texte et formes éditables).

## Limites connues (honnêtes)
Voir `docs/PHASE9.md`. En bref : pas de plans de travail multiples, pas d'édition de nœuds sur un tracé existant, pas de modèles génératifs (remplissage, super-résolution, colorisation), dialogues non traduits en anglais, ONNX Runtime à fournir soi-même, application jamais exécutée sur un Windows réel pendant le développement (compilée et éditée de liens seulement).

## Construire
```bash
# Cœur, opérations, E/S, rendu, IA (Linux/macOS/Windows)
cargo test --workspace --exclude openpics-app
# Logique d'application sans la coque Tauri
cargo test -p openpics-app --no-default-features
# Application (Windows, WebView2) : voir docs/RELEASE.md
cd ui && npm ci && npm run build && cd ../crates/app && ../../ui/node_modules/.bin/tauri build
```
Documentation : `docs/ARCHITECTURE.md`, une page par phase (`docs/PHASE*.md`), `docs/MODELS.md` (modèles ONNX), `docs/RELEASE.md` (publication), `tools/README.md` (captures et vidéo).
