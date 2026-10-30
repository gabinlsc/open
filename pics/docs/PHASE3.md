# Phase 3 — Sélections et outils de peinture

La phase est livrée par tranches. Cette page dit ce qui existe, comment il a été vérifié, et ce qui manque.

## Tranche 1 — sélections

**Fait**
- Rectangle, ellipse (anticrénelée), lasso libre, lasso polygonal (clic par clic, Entrée pour fermer, Échap pour annuler), baguette magique (tolérance, pixels contigus ou non, calque actif ou image aplatie), plage de couleurs (`ops::selection::color_range`).
- Modes de combinaison au clavier : Maj = ajouter, Alt = soustraire, Maj+Alt = intersecter. Un simple clic avec un outil de sélection désélectionne.
- Menu Sélection : tout, aucune, inverser, contour progressif, agrandir, contracter.
- Les sélections sont des masques 0–255 en tuiles, dans l'historique (annuler/rétablir) ; **les opérations de pixels n'agissent qu'à travers la sélection**, bords adoucis compris, et les tuiles hors sélection ne coûtent rien à l'historique.
- Contour de sélection dessiné par le GPU (alternance noir/blanc, plafonné à 60 000 segments).

**Vérifié** : tests unitaires des formes (aire, anticrénelage, règle pair-impair), de la baguette, du flou/morphologie, des modes de combinaison ; tests de session simulant les gestes ; rendu du contour sur GPU logiciel ; compilation Windows du code de la fenêtre native.

**Pas encore fait de cette tranche**
- **Sélection rapide** (pinceau de sélection), **affiner le bord**, **mode masque rapide**, **enregistrement en canaux alpha**, **sélection par sujet** (ONNX, phase 7) : à faire.
- Lasso magnétique : à faire (détection de contours).
- Les contours ne « marchent » pas (fourmis fixes).
- La saisie à la souris sous Windows n'a pas été exécutée.
