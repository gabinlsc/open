# Phase 2 — Calques (tranches 1 et 2)

La phase est découpée en tranches vérifiables. Cette page dit ce qui est fait, ce qui reste, et où en sont les critères d'acceptation fixés dans `PHASE1.md` §7.

## Tranche 1 — modèle de calques, compositeur de référence, panneau

- **27 modes de fusion** en CPU (`core/src/blend.rs`), formules W3C pour ceux qu'elle définit ; dissolution déterministe (même document = même image).
- **Propriétés de calque** : visibilité, opacité, remplissage, mode, verrous (pixels / position / tout), liaison (modèle seulement), écrêtage.
- **Masques de calque** (tout révéler / tout masquer, activer/désactiver, supprimer), stockés en tuiles comme les pixels.
- **Compositeur de référence** tuile par tuile (`core/src/composite.rs`) : groupes d'écrêtage, masques, opacité ; mémoire bornée à une tuile.
- **Historique** : propriétés, ajout/suppression/ordre des calques, masques ; libellés lisibles (« Mode de fusion : Produit »).
- **Verrous appliqués** : une opération de pixels sur un calque verrouillé est refusée avec un message clair.
- **`.ocv` schéma 2** (propriétés et masques) ; les fichiers du schéma 1 restent lisibles ; valeurs hors bornes ramenées dans 0..1 à la lecture.
- **Rendu** : les documents à calques passent par le compositeur de référence (repli CPU), mis en cache par tuile et révision ; vérifié sur GPU (Vulkan logiciel).
- **Interface** : panneau Calques (pile, œil, sélection, renommer, mode, opacité, remplissage, verrous, écrêtage, masques, monter/descendre), menu Calque, `Ctrl+Maj+N`.

Les opérations s'appliquent au **calque actif**.

## Tranche 2 — styles, transformation, test de propriété

- **Styles de calque** (`core/src/styles.rs`, compositeur) : ombre portée, lueur externe, contour (extérieur / intérieur / centré), incrustation de couleur et de dégradé avec mode de fusion. Ils partent de la forme du calque (masque compris) ; **le remplissage ne les affecte pas, l'opacité si**. L'ombre déborde correctement dans des tuiles où le calque n'a aucun pixel ; elle n'est pas recouverte par les calques écrêtés.
- Valeurs bornées partout (interface, session, fichier `.ocv`) ; historique « Styles de calque » ; persistés dans le `.ocv`.
- **Transformation libre** (`ops/src/transform.rs`) : matrice 3×3 (translation, rotation, échelle, cisaillement, **perspective** par quadrilatère), rééchantillonnage bilinéaire en alpha prémultiplié (pas de liseré sombre), **déformation par grille** (warp). Annulable ; refusée si la position du calque est verrouillée.
- **Interface** : section Styles (cinq types, couleurs, valeurs numériques validées) et boîte « Transformer… » (échelle avec proportions liées, rotation, déplacement, autour du centre du document).
- **Test de propriété** (`proptest`) : sur des séquences aléatoires d'actions (ajout/suppression/ordre de calques, propriétés, masques, styles, peinture, repères), tout annuler puis tout rétablir restitue exactement chaque état. Il a permis de corriger une faute de mon harnais ; aucun défaut du cœur trouvé.

## Vérifié

168 tests Rust (core 77 + 1 propriété, ops 16, io 15, gpu 8, app 51), 20 tests de l'interface ; fmt ; clippy sur Linux et `x86_64-pc-windows-gnu` ; `cargo deny` ; rendu GPU exécuté sur Vulkan logiciel ; captures de l'interface relues. **Non exécuté sous Windows.**

## Reste à faire pour clore la phase

| Élément | État |
|---|---|
| Groupes de calques (arbre, groupes isolés / « traverser ») | à faire : `Layer` reste une liste plate |
| Shaders WGSL des 27 modes et des styles + comparaison GPU ↔ CPU (≤ 1/255) | à faire : le GPU affiche le résultat du compositeur CPU |
| Pyramide de niveaux de détail (dette de la phase 1) | à faire |
| Interface de la perspective et de la déformation par grille | à faire (le calcul existe et est testé) |
| Liaison de calques : interface (les calques liés ne bougent pas encore ensemble) | à faire |
| Transformation interactive à la souris (poignées) | à faire |
| Parcours manuel sous Windows | non exécuté |

## Limites connues

- **Performance non mesurée** : le compositeur est mono-thread ; les styles utilisent un flou rectangulaire à 3 passes et une dilatation par disque en O(rayon²). Un document à styles recalcule toutes ses tuiles à chaque modification.
- **Styles sur un calque écrêté** : ignorés (seuls ceux du calque de base comptent).
- **Une transformation qui ne change rien** crée quand même un état d'historique (les tuiles sont recréées).
- **Le masque ne se peint pas encore** (outils de peinture en phase 3).
- Les styles ne sont pas encore recalculés par tuile voisine au-delà d'une marge de 3 tuiles (portée maximale 500 px).
- Pas de miniatures de calques ; calques créés vides.
