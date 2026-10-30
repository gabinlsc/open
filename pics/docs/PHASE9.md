# Phase 9 — Finitions

## Livré
- **Taille de l'image** : tous les calques, masques, repères et la sélection sont rééchantillonnés (Lanczos‑3 prémultiplié, noyau élargi en réduction), en une seule étape d'historique. Les textes et formes deviennent des calques de pixels ; les styles de calque (ombres…) ne sont pas mis à l'échelle.
- **Export multi‑résolutions** : `nom@1x.png`, `nom@2x.png`, `nom@0.5x.png`… depuis l'image aplatie, en PNG / JPEG / WebP.
- **Comparaison avec l'original** (Ctrl+Alt+C, menu Affichage) : le document revient à son état d'ouverture et est **gelé** (toute modification est refusée) ; la quitter rétablit exactement l'historique.
- **Explorateur de dossier** : liste les images d'un dossier, ouverture en un clic ou en bloc.
- **Scripts** (Rhai) et **enregistreur d'actions** : un script *décrit* des étapes (`filter`, `op`, `select_*`, `remove_background`, `inpaint`, `resize`, `new_layer`) exécutées ensuite par la session ; pas de modules, pas de fichiers, pas de réseau, nombre d'opérations/étapes/taille plafonné, `eval` désactivé. L'enregistreur produit ce même texte (relisible, éditable, rejouable) ; les actions se sauvegardent par nom.
- **Modèles ONNX réellement exécutés** (suppression d'arrière‑plan / sélection du sujet, type U²‑Net : image → carte de probabilité) via ONNX Runtime chargé dynamiquement (aucun téléchargement à la compilation, **télémétrie désactivée**). Testé de bout en bout avec ONNX Runtime réel et un modèle jouet généré par `crates/ai/tests/make_toy_model.py` (CI : `OPENPICS_REQUIRE_ORT=1`).
- **Publication Windows** : `release.yml` (étiquette `vX.Y.Z`) construit NSIS, MSI et portable, calcule les SHA‑256, génère `releases.json` et crée la release (voir `RELEASE.md`).
- **Correctif** : `ci.yml` était un YAML invalide depuis la phase 1 (deux `name:` contenant `: ` non guillemetés) ; corrigé.

## Non livré
- **Aucun vrai modèle n'a été essayé** (U²‑Net, ISNet, RMBG…) : seul un modèle jouet a été exécuté. Le prétraitement (ImageNet, taille d'entrée lue dans le modèle ou 320) et la normalisation de la sortie couvrent la famille U²‑Net ; d'autres architectures peuvent demander des réglages.
- **Remplissage génératif, super‑résolution neuronale, colorisation, débruitage neuronal** : non faits (architectures et pré/post‑traitements propres à chaque modèle, impossibles à valider sans eux). Le redimensionnement Lanczos remplace la super‑résolution classique.
- **ONNX Runtime n'est pas livré** : `onnxruntime.dll` est à fournir à côté de l'exécutable ou dans le dossier des modèles. `ort` exige Rust ≥ 1.92 quand la fonctionnalité `onnx` est active.
- **Plans de travail multiples (artboards)** et **édition de nœuds** sur un tracé : non faits — ils demandent un changement du modèle de document / un outil d'édition de courbes complet, que je préfère ne pas improviser.
- **Traduction** : les boîtes de dialogue et messages restent en français.
- **Le workflow de publication et l'application Tauri n'ont jamais été exécutés sur Windows** depuis cet environnement (vérifiés par `clippy` en compilation croisée seulement).
