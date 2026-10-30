# Phase 4 — Retouche et réglages non destructifs

## Tranche 1 — calques de réglage

**Fait** (`core/src/adjust.rs`, compositeur, format, panneau)
- **15 réglages** : luminosité/contraste, exposition, niveaux, courbes (interpolation cubique monotone, courbe commune + une par canal), teinte/saturation/clarté, vibrance, balance des couleurs (ombres / tons moyens / hautes lumières, luminosité conservée), noir et blanc (six curseurs de teinte), filtre photo, mélangeur de canaux (matrice 3×3 + monochrome), courbes de dégradé, **LUT 3D** (`.cube`, interpolation trilinéaire, jusqu'à 65³), négatif, tonalité HDR (Reinhard étendu), **correspondance de couleur** (transfert de moyenne et d'écart-type entre deux calques).
- **Non destructifs** : un calque de réglage ne touche aucun pixel ; masqué, il restitue l'original. Il accepte **masque de calque**, **opacité**, **écrêtage** (il ne touche alors que le calque du dessous, dans sa forme) et respecte l'**isolation des groupes**.
- Les paramètres sont décrits par une table (`ParamSpec`) : l'interface génère les curseurs depuis elle, et chaque valeur est bornée à l'application (NaN et infini retombent sur le défaut).
- Historique par réglage (« Exposition : Exposition (IL) ») ; un changement sans effet ne crée pas d'état.
- Format `.ocv` : tous les réglages et leurs données (courbes, arrêts, LUT, statistiques) sont persistés ; un `.cube` illisible fait échouer l'ouverture avec un message précis.
- Interface : Calque › « Réglage : … », panneau avec curseurs, éditeur de courbe (points numériques, aperçu), éditeur d'arrêts de dégradé, chargement de `.cube`, choix des calques source/référence pour la correspondance.

**Vérifié** : valeurs connues et propriétés de chaque famille (identité par défaut, exposition ±1 IL, niveaux, monotonie des courbes, passage par les points de contrôle, conservation de la luminosité, vibrance plus forte sur les couleurs ternes, LUT identité et permutation, correspondance exacte de moyenne), toutes les sorties bornées sur 0..1 pour des entrées hostiles ; composition (non destructif, opacité, masque, pixels transparents, écrêtage, groupes) ; format ; session ; interface.

**Pas encore fait / limites**
- **Camera Raw intégré** : **non fait**. Pas de lecture de fichiers RAW (la bibliothèque `rawler` n'est pas intégrée et je n'ai aucun fichier RAW pour la vérifier), pas de dématriçage, de correction d'objectif ni de réduction de bruit dans un panneau « développement ». Les réglages disponibles (exposition, niveaux, courbes, vibrance, HDR) couvrent une partie de ses fonctions sur des images déjà décodées ; la clarté et la réduction de bruit relèvent des filtres (phase 5).
- **Balance des blancs** par température/teinte : pas de réglage dédié (approximable avec la balance des couleurs ou un filtre photo).
- Correspondance de couleur : statistiques en RGB (pas en espace Lab), sans histogramme ni tolérance sur la zone.
- Le mode de fusion d'un calque de réglage est ignoré.
- Les courbes ne se tracent pas à la souris (édition numérique) ; la courbe par canal n'a pas d'interface (le modèle et le format la portent).
- Calcul CPU, non mesuré sur de grands documents ; le GPU affiche le résultat du compositeur CPU.
