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

## Tranche 2 — peinture et retouche

**Fait** (moteur dans `ops/src/{brush,fill,retouch}.rs`, outils dans `app/src/session/paint.rs`)
- **Pinceau** : taille, dureté, espacement, opacité (plafond du trait entier), flux (s'accumule dab après dab), lissage, taille et opacité selon la pression ; **crayon** (bords nets) ; **gomme**.
- **Dégradés** : linéaire, radial, angulaire, réfléchi, losange ; du premier plan vers l'arrière-plan ou vers la transparence ; aperçu en direct sur les calques jusqu'à 4 Mpx.
- **Pot de peinture** (tolérance, contigu, calque ou image aplatie).
- **Retouche** : flou, netteté, doigt, densité −/+ (ombres / tons moyens / hautes lumières), éponge (saturer / désaturer), **tampon de duplication** (Alt + clic pour la source, décalage aligné), **correcteur** (texture de la source, éclairage de la destination), **correcteur localisé** (choisit seul la zone source), **pièce** (sélection glissée vers la source).
- Un trait = **un seul état d'historique** ; Échap en cours de trait restitue le calque ; verrous, groupes et sélection respectés ; couleurs de premier/arrière-plan avec permutation (`X`).
- Correction de fond du noyau : un traitement qui ne change aucun pixel ne crée plus d'état d'historique (comparaison du contenu des tuiles, plus seulement de leur identité). Cela retire une limite notée en phase 2.

**Vérifié** : tests unitaires du moteur (disque plein, chute douce, absence de trous, plafond d'opacité vs accumulation du flux, pression, lissage, sélection, valeurs hostiles), de chaque dégradé, de chaque outil de retouche, et de la session (historique, verrous, annulation en cours de trait). L'interface a été relue en capture.

**Pas encore fait**
- **Pression du stylet (Windows Ink)** : les événements natifs transmettent une pression fixe de 1 ; la lecture de `WM_POINTER` reste à écrire (nécessite une machine Windows pour être vérifiée). La taille et l'opacité selon la pression sont prêtes côté moteur.
- **Dynamique de forme avancée** (angle, rondeur, texture, diffusion) ; pas de pointes de pinceau personnalisées.
- Pas de curseur de pinceau affichant le diamètre ; pas d'aperçu du tampon.
- Tous les retouches s'appliquent calque par calque (pas d'« échantillonner tous les calques »).
- Pictogrammes de la barre d'outils : des symboles texte pour les outils de retouche (dessins dédiés à faire).
- Performance non mesurée : les dabs sont calculés en CPU, une opération par pixel ; les très grosses brosses (> 500 px) peuvent être lentes.
