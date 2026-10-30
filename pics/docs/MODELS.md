# Utiliser un modèle d'IA (exemple : U²-Net léger)

OpenPics ne télécharge rien : on dépose soi-même le fichier, et il est vérifié par empreinte avant d'être rangé.

1. Récupérer `onnxruntime.dll` (ONNX Runtime ≥ 1.2x, Windows x64) et le placer **à côté de `OpenPics.exe`** ou dans le dossier des modèles.
2. Dans le dossier des modèles (`%APPDATA%\fr.glsc.open.pics\models`, ou `data\models` en mode portable), créer `models.json` :

```json
[
  {
    "id": "u2netp",
    "task": "background_removal",
    "name": "U²-Net léger",
    "licence": "Apache-2.0 (à vérifier auprès du dépôt d'origine avant redistribution)",
    "source": "https://github.com/danielgatis/rembg/releases/download/v0.0.0/u2netp.onnx",
    "sha256": "309c8469258dda742793dce0ebea8e6dd393174f89934733ecc8b14c76f4ddd8",
    "size_bytes": 4574861
  }
]
```

3. Télécharger le fichier à la main, puis menu **IA › Modèles d'IA… › Importer depuis un fichier…** (une empreinte différente est refusée).
4. **IA › Sélectionner le sujet…** ou **Supprimer l'arrière-plan…** : choisir « Modèle : U²-Net léger » dans *Méthode*.

## Ce qui a été vérifié
Avec ce modèle (4,5 Mo) et ONNX Runtime 1.29 sous Linux, sur une photo de tigre dans un feuillage sombre : le masque suit correctement le contour de l'animal (≈ 24 % de l'image), là où la méthode classique (fond estimé par les bords) échouerait. Non vérifié : Windows, d'autres modèles (ISNet, RMBG…), GPU.
