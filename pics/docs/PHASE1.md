# Phase 1 — Socle

## 1. Fichiers

```
LICENSE                      GPL-3.0-only
README.md
.github/workflows/ci.yml     fmt, clippy, tests (GPU logiciel), licences, UI, build Windows
pics/
  Cargo.toml  rustfmt.toml  clippy.toml  deny.toml  rust-toolchain.toml  .cargo/config.toml
  crates/core/    modèle du document, tuiles COW, historique, vue, aimantation, raccourcis (+ benches/tiles.rs)
  crates/ops/     retourner H/V, inverser (rayon) (+ benches/ops.rs)
  crates/io/      PNG/JPEG/WebP/TIFF/BMP/GIF, format natif .ocv (+ benches/open.rs)
  crates/gpu/     rendu wgpu : tuiles, damier, grille, repères (shader.wgsl)
  crates/app/     session, DTO, préférences, coque Tauri, canvas natif Win32, icônes
  ui/             React + TypeScript + Zustand (menus, onglets, règles, panneaux, dialogues)
  docs/           ARCHITECTURE.md, PHASE1.md
site/             open.glsc.fr : accueil de la suite et page de téléchargement de Pics
```

## 2. Ce qui est livré

Ouverture, enregistrement (`.ocv`) et export (PNG, JPEG, WebP, TIFF, BMP, GIF) ; canvas natif avec zoom/pan ;
règles, repères (glissés depuis les règles), grille, aimantation ; historique par tuiles copy-on-write
(annuler/rétablir) ; raccourcis clavier personnalisables et persistés ; thèmes clair/sombre/système ;
documents en onglets ; glisser-déposer de fichiers ; mode portable.

## 3. Ce qui a été vérifié, et ce qui ne l'a pas été

**Vérifié ici (Linux)** :

| Vérification | Résultat |
|---|---|
| Tests Rust : `core` 37, `ops` 5, `io` 11, `gpu` 7, `app` (session, préférences, DTO) 34 | tous verts |
| Rendu GPU réel (tuiles, zoom, repères, rechargement d'une tuile modifiée) | exécuté sur Vulkan logiciel (lavapipe), `OPENPICS_REQUIRE_GPU=1` |
| `cargo fmt --check`, `cargo clippy -D warnings` | propres (Linux, et `x86_64-pc-windows-gnu` pour la coque) |
| Coque Tauri et code Win32 | **compilés** pour `x86_64-pc-windows-gnu` |
| `cargo deny check licenses bans sources` | `bans ok, licenses ok, sources ok` |
| Interface : `tsc`, 16 tests vitest, `vite build` | verts |
| Interface rendue dans Chromium avec un faux backend | captures relues (clair, sombre, accueil) |

**Non vérifié — à valider sur une machine Windows** :

- le **lancement réel** de l'application : la fenêtre enfant Win32, son placement sur la zone du canvas, la présentation de la swapchain, la molette, le glisser au clic milieu, le curseur, le dépôt de fichiers sur le canvas ;
- la position de la fenêtre native par rapport au webview (ordre Z, DPI) ;
- la construction des installeurs NSIS/MSI et de la version portable (workflow `windows` de la CI, jamais exécuté) ;
- les performances de zoom/pan à 60 fps et le temps d'ouverture sur du vrai matériel.

Parcours de contrôle manuel (5 minutes) : lancer ; Fichier › Nouveau (1920×1080) ; le document blanc apparaît centré ;
Ctrl+molette zoome sous le curseur ; clic milieu déplace ; glisser depuis la règle du haut crée un repère qui s'aimante ;
Image › Inverser puis Ctrl+Z ; Fichier › Enregistrer puis rouvrir le `.ocv` ; déposer un PNG sur le canvas ;
ouvrir un menu (le canvas se masque le temps de l'ouverture) ; Édition › Raccourcis clavier ; changer de thème.

## 4. Mesures (criterion, sandbox Linux 4 cœurs — indicatif)

| Opération | Temps |
|---|---|
| Ouverture d'un PNG 48 Mpx (décodage + tuilage), image synthétique | ≈ 470 ms |
| Découpage en tuiles 48 Mpx / aplatissement | ≈ 103 ms / ≈ 147 ms |
| Instantané d'historique (clone + comparaison) 48 Mpx | ≈ 34 µs |
| Retourner 24 Mpx (H / V) / inverser | ≈ 65 ms / ≈ 57 ms / ≈ 30 ms |

L'objectif « 50 Mpx en moins de 2 s » est tenu sur ce PNG **synthétique, très compressible** ; un vrai JPEG ou PNG photo
décode plus lentement : à re-mesurer sur des fichiers réels. Les objectifs GPU (60 fps, latence pinceau,
gaussien 24 Mpx < 150 ms) ne sont **pas mesurables** sans carte graphique : pas de filtre ni de pinceau avant les phases 3 et 5.

## 5. Limites connues et dette technique

- **Canvas natif** : toujours au-dessus du webview. Menus déroulants et dialogues **masquent** le canvas pendant qu'ils sont ouverts (visible comme un clignotement) ; les notifications sont dans la colonne des panneaux. Aucun HTML ne peut recouvrir le canvas.
- **Pas de pyramide de niveaux de détail** : zoomé très loin sur une image de 100 Mpx, le rendu dessine jusqu'à ~1 500 tuiles texturées par image. Lissage linéaire seul, donc crénelage possible en dézoomant. À traiter avant la phase 2.
- **Mémoire** : l'historique n'a pas de plafond ni de déversement sur disque ; l'ouverture décode l'image entière en RGBA avant de la tuiler (pic ≈ 2× la taille de l'image). Le décodage par bandes est prévu.
- **Couleur** : sRGB 8 bits uniquement, alpha non prémultiplié, pas de profil ICC, pas de conversion vers le profil de l'écran.
- **Formats** : WebP écrit sans perte seulement, GIF : première image. AVIF, HEIC, PSD, SVG, RAW : phases suivantes.
- **Repères** : créés depuis les règles ; on les supprime par le panneau ; pas encore de déplacement par glisser sur le canvas.
- **Un seul calque** (« Arrière-plan ») ; la structure `Layer` est prête à devenir une énumération (phase 2).
- **Onglets** : pas de réordonnancement par glisser. **Langue** : français seulement (i18n en phase 8).
- **Raccourcis** : `Ctrl+'` et `Ctrl+;` sont peu pratiques sur clavier AZERTY (modifiables dans Édition › Raccourcis clavier).
- **Entrées** : souris uniquement ; le stylet (Windows Ink) arrive avec les outils de peinture.
- **Signature** : la CI contient un emplacement réservé ; les binaires sont non signés (SmartScreen affichera un avertissement).
- **Tests de l'interface** : fonctions pures et registre d'actions ; pas de test de composants ni de bout en bout.

## 6. Publier une version sur le site

Le site est statique. Déposer sur le serveur, dans `pics/download/`, l'installeur, le zip portable, puis `pics/releases.json` :

```json
{
  "version": "0.1.0",
  "files": [
    { "label": "Installeur Windows (x64)", "name": "OpenPics_0.1.0_x64-setup.exe", "size": 12345678, "sha256": "…" },
    { "label": "Version portable (x64)",   "name": "OpenPics-portable-x64.zip",     "size": 9876543,  "sha256": "…" }
  ]
}
```

Le SHA-256 s'obtient avec `Get-FileHash fichier -Algorithm SHA256` (PowerShell) ou `sha256sum fichier`.

## 7. Critères d'acceptation de la phase 2 (calques)

1. `Layer` devient une énumération (raster, groupe) sans changer `TileGrid` ni l'historique ; les tests de la phase 1 passent inchangés.
2. Les 27 modes de fusion ont chacun un test de golden image comparant le shader GPU à une implémentation CPU de référence (tolérance ≤ 1/255 par canal).
3. Opacité, remplissage, masques de calque et d'écrêtage, verrouillage, liaison : annuler/rétablir restitue l'état exact (test de propriété `proptest`).
4. Transformation libre (échelle, rotation, perspective, déformation, warp) avec aperçu à 60 fps sur un document de 24 Mpx, mesuré sur matériel réel et consigné.
5. Styles de calque (ombre, lueur, contour, dégradé, incrustation) : golden images.
6. Pyramide de niveaux de détail : un document de 100 Mpx dézoomé reste fluide (≤ 16 ms par image sur GPU dédié) ; dette « LOD » ci-dessus soldée.
7. Le format `.ocv` passe au schéma 2 avec migration depuis le schéma 1 et test de relecture.
8. Le parcours de contrôle manuel de la phase 1 a été exécuté sur Windows et ses résultats consignés.
