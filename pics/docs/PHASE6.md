# Phase 6 — Texte et vectoriel

## Ce qui est livré
- **Calques de texte** : le texte reste la source (police, corps, interligne, couleur, gras/italique, alignement, bloc de paragraphe avec retour à la ligne, texte sur tracé). Le rendu raster est recalculé à chaque modification et l'opération est annulable.
- **Formes vectorielles** : rectangle, rectangle arrondi, ellipse, ligne, plume (courbes de Bézier). Remplissage, contour, règle pair-impair.
- **Opérations booléennes** : union, intersection, soustraction, exclusion.
- **Tracé → sélection**, **rastérisation** d'un calque texte/forme, **export SVG**, **import SVG** (rastérisé).
- Sérialisation `.ocv` : les modèles texte et forme sont conservés (le document rouvert reste éditable).

## Limites assumées
- Édition de nœuds (ajoutée ensuite) : outil **Sélection directe** (A) pour déplacer points et poignées d'une forme, poignées symétriques (Alt pour les rendre indépendantes), Alt + clic pour un point anguleux. Pas encore d'ajout ni de suppression de point.
- Texte sur tracé : première ligne uniquement.
- Pas de jonctions de contour à angle vif (miter) : jonctions rondes.
- Import SVG : rastérisation uniquement, pas de conversion en formes éditables.
- Résultats booléens polygonaux (courbes aplaties).
- Pas d'interface pour les fonctionnalités OpenType (ligatures, variantes).

## Tests
Les tests de texte nécessitent des polices système : `OPENPICS_REQUIRE_FONTS=1` fait échouer la suite si aucune n'est trouvée au lieu de les ignorer silencieusement.

## Dépendances
`cargo deny check advisories` signale des avis sur des dépendances transitives (zip, quick-xml, ttf-parser…). Le CI ne vérifie que licences, bans et sources ; le code n'utilise pas `ZipArchive::extract` (les entrées sont lues une à une, noms validés). À surveiller lors des mises à jour.
