# Phase 2 — Calques (en cours : tranche 1)

La phase est découpée en tranches vérifiables. Cette page dit ce qui est fait, ce qui reste, et où en sont les critères d'acceptation fixés dans `PHASE1.md` §7.

## Tranche 1 — modèle de calques, compositeur de référence, panneau (livrée)

- **27 modes de fusion** en CPU (`core/src/blend.rs`), formules W3C pour ceux qu'elle définit ; dissolution déterministe (même document = même image).
- **Propriétés de calque** : visibilité, opacité, remplissage, mode, verrous (pixels / position / tout), liaison (modèle seulement), écrêtage.
- **Masques de calque** (tout révéler / tout masquer, activer/désactiver, supprimer), stockés en tuiles comme les pixels.
- **Compositeur de référence** tuile par tuile (`core/src/composite.rs`) : groupes d'écrêtage, masques, opacité ; mémoire bornée à une tuile.
- **Historique** : propriétés, ajout/suppression/ordre des calques, masques ; libellés lisibles (« Mode de fusion : Produit »).
- **Verrous appliqués** : une opération de pixels sur un calque verrouillé est refusée avec un message clair.
- **`.ocv` schéma 2** (propriétés et masques) ; les fichiers du schéma 1 restent lisibles ; valeurs hors bornes ramenées dans 0..1 à la lecture.
- **Rendu** : les documents à calques passent par le compositeur de référence (repli CPU), mis en cache par tuile et révision ; vérifié sur GPU (Vulkan logiciel).
- **Interface** : panneau Calques (pile, œil, sélection, renommer, mode, opacité, remplissage, verrous, écrêtage, masques, monter/descendre), menu Calque, `Ctrl+Maj+N`.

Vérifié : 136 tests Rust (core 63, ops 6, io 14, gpu 8, app 45), 16 tests de l'interface, fmt, clippy (Linux et `x86_64-pc-windows-gnu`), `cargo deny`. Les opérations s'appliquent maintenant au **calque actif**.

## Reste à faire pour clore la phase

| Élément | État |
|---|---|
| Groupes de calques (arbre, groupes isolés / « traverser ») | à faire : `Layer` reste une liste plate |
| Transformation libre : échelle, rotation, perspective, déformation, warp en grille | à faire |
| Styles de calque : ombre, lueur, contour, dégradé, incrustation | à faire |
| Shaders WGSL des 27 modes + test de comparaison GPU ↔ CPU (≤ 1/255) | à faire : le GPU utilise pour l'instant le repli CPU |
| Pyramide de niveaux de détail (dette de la phase 1) | à faire |
| Liaison de calques : interface | à faire (le modèle et le format la portent déjà) |
| Test de propriété (`proptest`) « annuler/rétablir = identité » sur les calques | à faire (tests unitaires par commande en place) |
| Parcours manuel sous Windows | non exécuté (aucune machine Windows ici) |

## Limites connues de la tranche 1

- **Performance** : le compositeur est mono-thread et recalcule les tuiles visibles à chaque modification du document ; non mesuré sur de grands documents à nombreux calques.
- **Remplissage** : tant que les styles n'existent pas, remplissage et opacité ont le même effet visuel.
- **Le masque ne se peint pas encore** : aucun outil de peinture avant la phase 3 ; la commande d'historique `ReplaceTiles::between_mask` est prête.
- **Calques créés vides et transparents** ; pas d'import d'une image comme nouveau calque.
- **Pas de miniatures** de calques.
