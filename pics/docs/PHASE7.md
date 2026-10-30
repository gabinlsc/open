# Phase 7 — IA locale

Ce que cette phase **livre** et ce qu'elle **ne livre pas**, sans enjoliver.

## Livré
- **Catalogue de modèles** (`crates/ai`, `catalog.rs`) : fichier `models.json` (identifiant, tâche, licence, source, SHA-256, taille). Un modèle s'**importe depuis un fichier** : l'empreinte est vérifiée avant tout rangement, l'écriture est atomique, les identifiants qui tenteraient de sortir du dossier sont refusés. Boîte de dialogue « Modèles d'IA… ».
- **Aucun réseau** : le logiciel ne télécharge rien. La champ « source » est une information pour l'utilisateur.
- **Méthodes classiques sans modèle** (`classic.rs`), déterministes et instantanées :
  - *Sélectionner le sujet* : le fond est estimé par k-means sur les bords, puis ce qui lui ressemble **et** touche le bord est du fond (les trous entourés par le sujet restent sujet). Transition adoucie.
  - *Supprimer l'arrière-plan* : même méthode, résultat posé dans un **masque** (réversible).
  - *Combler la sélection* : diffusion depuis les abords (couche par couche, puis relaxation). Retire poussières, fils, petits objets.
- Le menu **IA** expose ces trois fonctions ; le débruitage « classique » existe déjà dans Filtre › Bruit.

## Non livré (et pourquoi)
- **Exécution de modèles ONNX** (suppression de fond neuronale, remplissage génératif, super-résolution, colorisation) : non implémentée. `ort` télécharge des binaires ONNX Runtime à la compilation et n'a pas pu être validé ici, et aucun modèle n'a d'empreinte ni de licence vérifiées dans cet environnement. Livrer du code d'inférence jamais exécuté serait mentir sur son état. L'infrastructure (catalogue, vérification, stockage) est prête à l'accueillir.
- Les méthodes classiques **ne remplacent pas** un réseau de neurones : sur une scène complexe (cheveux, fonds texturés) la sélection du sujet sera médiocre ; la diffusion ne crée aucune texture.
- Pas de super-résolution ni de colorisation : sans modèle, il n'existe pas de méthode classique honnête.

## Tests
Empreinte correcte/incorrecte, import atomique, catalogue absent/corrompu/traversée de chemin, sujet sur fond uni/dégradé/transparent, trous, diffusion dans un dégradé, annulation du masque.
