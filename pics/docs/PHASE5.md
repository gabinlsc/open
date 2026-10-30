# Phase 5 — Filtres

## Fait (référence CPU, `ops/src/filters.rs`, `ops/src/liquify.rs`)

- **Flou** : gaussien, de surface (préserve les contours), directionnel, radial (rotation et zoom), de l'objectif (disque, éclat des hautes lumières).
- **Netteté** : masque flou (quantité, rayon, seuil), accentuation optimisée (atténuée dans les zones uniformes, plafonnée aux contours).
- **Bruit** : ajout (uniforme ou gaussien, monochrome, graine reproductible), réduction (médiane).
- **Déformation** : vague, pincement, sphérisation ; **fluidité** interactive (pousser, gonfler, pincer, tourbillon, reconstruire) avec champ de déplacement : le calque d'origine n'est jamais dégradé, on peut reconstruire.
- **Stylisation** : relief, contours (Sobel), isohélie, seuil, solarisation. **Pixellisation** : mosaïque, cristallisation (Voronoï). **Rendu** : nuages (bruit fractal), éclairage (lumière ponctuelle).
- **Galerie** : le menu Filtre est généré depuis un registre unique (identifiant, catégorie, paramètres bornés) ; **aperçu en direct** dans un panneau ancré (le canvas reste visible), un seul état d'historique à la validation, annulation propre, respect de la **sélection** et des **verrous**.
- Tous les calculs sont en alpha prémultiplié : un bord transparent ne ternit pas la couleur.

## Vérifié

Un test par famille : invariants (identité à rayon nul, conservation de la moyenne, flou de surface qui garde le contour mieux qu'un gaussien, flou directionnel qui n'étale que dans son axe, affûtage qui ne touche pas un aplat, bruit reproductible et centré, médiane qui supprime les points isolés, déformations identité à 0 avec centre et bord fixes, cellules uniformes de la mosaïque, nuages reproductibles), et un test commun qui exécute **chaque filtre du registre** avec ses défauts, des valeurs NaN et des valeurs énormes. Session : aperçu / changement de filtre / annulation / validation, sélection, verrous, filtre sans effet = pas d'historique, fluidité annulable.

## Pas fait / limites

- **Filtres intelligents (non destructifs) : non faits.** Les filtres modifient les pixels (annulables) ; un filtre rejouable sur un calque exigerait un cache d'image filtrée à invalider à chaque modification du calque, que je n'ai pas réalisé de façon fiable. Contournement : dupliquer le calque avant de filtrer.
- **Pas de shaders WGSL** : tous les filtres sont en CPU (rayon). L'objectif « gaussien 24 Mpx < 150 ms sur GPU » n'est donc ni atteint ni mesuré ; le flou gaussien CPU travaille sur une image flottante entière (un 24 Mpx demande ≈ 400 Mo de tampon). Le filtrage par tuiles avec halo et le portage GPU sont à faire.
- **Pas de tests d'images de référence (golden)** par filtre : les tests vérifient des propriétés, pas des images figées.
- L'aperçu recalcule le calque entier à chaque changement de curseur (agréable jusqu'à quelques mégapixels, lent au-delà).
- Fluidité : pas de masque de gel, pas de visage, pas de pression du stylet.
- Pas de filtre « à partir d'un fichier » ni de modules externes (voir phase 8).
