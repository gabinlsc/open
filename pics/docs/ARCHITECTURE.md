# OpenPics — Architecture (v0, à valider avant tout code)

Statut : proposition. Aucun code applicatif n'existe. Les signatures ci-dessous décrivent le design, pas l'implémentation.

## 0. Licence

**Recommandation : GPLv3.**
1. Le copyleft empêche qu'un fork fermé s'approprie l'éditeur, ce qui correspond à l'esprit « suite libre ».
2. Elle est compatible avec nos dépendances LGPL (rawler, libheif/libde265) et MIT/Apache (wgpu, lcms2, ort, wasmtime, image).
3. Contrepartie : pas d'intégration dans un produit propriétaire. Si ce point compte, MIT reste possible, mais les dépendances LGPL imposeraient alors un lien dynamique.

Un audit des licences (`cargo deny`) est prévu en CI. Les licences de rawler et de libheif sont à revérifier à l'ajout de la dépendance.

## 1. Crates

```
                    ┌───────────────────────────┐
                    │ app  (Tauri 2 + React)    │  IPC, fenêtre native du canvas,
                    └──┬───────┬────────┬───────┘  entrées stylet, plugins, ONNX
                       │       │        │
              ┌────────▼──┐ ┌──▼───┐ ┌──▼───┐
              │ gpu       │ │ ops  │ │ io   │
              │ wgpu/WGSL │ │ rayon│ │ codecs, ICC, PSD, .ocv
              └────────┬──┘ └──┬───┘ └──┬───┘
                       └───────▼────────┘
                          ┌───▼────┐
                          │ core   │  aucun GPU, aucune E/S fichier
                          └────────┘
```

| Crate | Responsabilité | Dépend de |
|---|---|---|
| `core` | Modèle du document, tuiles, historique, sélections, espaces/profils de couleur (types), géométrie. Pur, déterministe, 100 % testable sans GPU. | — |
| `ops` | Opérations CPU (rayon) : pinceau, remplissage, sélections, transformations, filtres de repli, moteur de brosses, hôte WASM (wasmtime). | `core` |
| `io` | Lecture/écriture des formats, conversions ICC (lcms2), `.ocv`, PSD, métadonnées EXIF/XMP. | `core` |
| `gpu` | Contexte wgpu, cache de textures de tuiles, graphe de rendu, shaders WGSL (fusion, réglages, filtres), chargeur de filtres WGSL externes. | `core` |
| `app` | Binaire Tauri : commandes IPC, fenêtre du canvas, événements, raccourcis, préférences, ONNX Runtime (`ort`, phase 7), scripts (phase 8). | tout |

Les crates `plugin` et `ai` ne sont **pas** créées d'avance : l'hôte WASM vit dans `ops`, les filtres WGSL dans `gpu`, l'IA dans `app`. On extrait si la taille le justifie. Seuls `xtask` (build, installeur, golden images) et un crate de test partagé `testkit` s'ajoutent hors des cinq imposées.

Règle de dépendance : `core` ne dépend jamais de `gpu`, `io` ni `app`. Les erreurs sont typées par crate (`thiserror`), sans `unwrap`/`panic` hors tests (lints `clippy::unwrap_used`, `expect_used`, `panic` en `deny`).

## 2. Modèle de données du document

```rust
struct Document { id: DocId, size: Size, color: ColorSpec, layers: LayerTree,
                  selection: Selection, channels: Vec<AlphaChannel>, guides: Guides,
                  artboards: Vec<Artboard>, history: History, meta: Metadata }

struct ColorSpec { model: Rgb|Gray|Cmyk, depth: U8|U16, profile: IccProfileRef } // sRGB, P3, AdobeRGB, ICC
```

- **Espace de travail** : l'édition se fait dans le profil du document, en 8 ou 16 bits. La composition GPU se fait en linéaire `rgba16float` prémultiplié, puis conversion vers le profil de l'écran à l'affichage. CMYK et LAB : conversion à l'entrée/sortie et pour certains outils, pas d'édition CMYK native (voir §10).
- **Arbre de calques** : arène (`slotmap`) avec identifiants stables ; un nœud référence son parent et ses enfants. Les commandes d'historique manipulent des identifiants, jamais des pointeurs.

```rust
struct Layer { id: LayerId, name: String, visible: bool, opacity: f32, fill: f32,
               blend: BlendMode, locks: Locks, linked: Option<LinkGroup>,
               mask: Option<Mask>, clip_to_below: bool,
               styles: Vec<LayerStyle>, transform: Affine2, kind: LayerKind }

enum LayerKind {
  Raster(TileGrid),
  Text(TextModel),                 // runs, police, OpenType, tracé, paragraphe ; raster en cache
  Shape(VectorModel),              // chemins, remplissage, contour, booléens ; raster en cache
  Group { children: Vec<LayerId>, isolated: bool },   // isolé ou « pass-through »
  Adjustment(AdjustmentKind),      // niveaux, courbes, teinte/sat., LUT 3D, etc. (paramètres, pas de pixels)
  Smart(SmartObject),              // source embarquée + filtres intelligents + transformation
}
struct Mask { grid: TileGrid /*Gray*/, density: f32, feather: f32, enabled: bool, linked: bool }
```

- **27 modes de fusion** : normal, dissolution, obscurcir, produit, densité couleur +, densité linéaire +, couleur plus sombre, éclaircir, écran, densité couleur −, densité linéaire − (ajout), couleur plus claire, superposition, lumière tamisée, lumière crue, lumière vive, lumière linéaire, lumière ponctuelle, mélange maître, différence, exclusion, soustraction, division, teinte, saturation, couleur, luminosité. Formules W3C Compositing/PDF, documentées en commentaire du shader.
- **Masque d'écrêtage** : un calque `clip_to_below` est composé dans la base formée par le calque non écrêté situé dessous (alpha de la base × son propre alpha).
- **Styles de calque** : `LayerStyle` = ombre portée/interne, lueur ext./int., contour, incrustation couleur/dégradé/motif. Calculés depuis l'alpha du calque (après transformation), mis en cache comme textures d'effet.
- **Sélection** : masque 8 bits (ou 16) en `TileGrid`, plus un chemin vectoriel optionnel pour les formes simples ; `AlphaChannel { name, grid, color, opacity }` pour les canaux enregistrés ; le mode masque rapide est une vue d'édition de la sélection comme calque temporaire.
- **Objet dynamique** : `SmartObject { source: Arc<EmbeddedImage>, transform, smart_filters: Vec<FilterInstance> }`. Les filtres sont rejoués depuis la source ; le résultat est mis en cache par hash des paramètres.
- **Format natif `.ocv`** : zip contenant `document.json` (version de schéma, arbre, paramètres), `layers/<id>.png` (ou `.tif` 16 bits), `masks/`, `channels/`, `icc/`, `thumb.png`. Écriture atomique (fichier temporaire puis renommage) ; migrations de schéma numérotées.

## 3. Historique

- **Commandes** : `trait Command { apply(&mut Document) -> Result<()>; revert(...); merge(&mut self, next:&Self)->bool }`. Une commande n'enregistre que ce qui change (paramètres d'un calque, arbre) ; l'historique est une pile linéaire plus des **branches d'états nommés** (instantanés).
- **Snapshots par tuiles, copy-on-write** : une `TileGrid` est une table `(tx,ty) → Arc<Tile>`. Avant de modifier une tuile (`Arc::make_mut`), l'ancienne `Arc` est conservée dans le `TileDelta { layer, before: Map<TileKey, Arc<Tile>>, after }` de la commande. Seules les tuiles touchées coûtent de la mémoire ; deux états partagent les tuiles inchangées.
- **Trait pour gros traits de pinceau** : les commandes de peinture sont fusionnées par trait (`merge`) et le delta est finalisé au relâchement.
- **Budget mémoire** : « illimité » = borné par le disque. Un `TileStore` applique un budget RAM (par défaut 50 % de la RAM libre) ; les tuiles de l'historique froides sont compressées (LZ4 ou zstd) puis écrites dans un fichier temporaire mmap ; on les relit à la demande lors d'un undo.
- **Récupération après crash** : journal des commandes + tuiles spillées dans `%LOCALAPPDATA%\OpenPics\recovery`, rejoué au redémarrage (phase 8).

## 4. Moteur de tuiles

- **Tuile** : 256×256 pixels, format du document (RGBA8, RGBA16, Gray, masque). 256×256×4×2 octets = 512 Kio à 16 bits.
- **Creux** : une tuile absente est transparente ; une tuile uniforme est stockée comme `Uniform(Color)` (aucun buffer). Un 100 Mpx 8 bits plein = ~400 Mo, 16 bits ~800 Mo par calque raster dense : d'où le creux, le spill disque et l'absence de copie complète.
- **Pyramide** : niveaux de détail (mip) 1/2, 1/4… calculés paresseusement et invalidés par tuile sale ; le zoom arrière lit un niveau réduit, jamais le plein.
- **Rendu progressif** : le viewport demande les tuiles visibles au LOD voulu. Ordre : LOD grossier d'abord (déjà en cache), puis raffinement du centre vers les bords. Les tuiles sont préparées par un pool de threads (rayon), téléversées vers le GPU dans un atlas de textures avec un cache LRU.
- **Invalidation** : toute modification marque `dirty(layer, tile)` ; le rendu recompose uniquement les tuiles d'écran dont la région recouvre une tuile sale (propagée via l'arbre : masque, groupe, réglages).
- **Ouverture d'un 50 Mpx < 2 s** : décodage en flux par bandes parallèles vers les tuiles, affichage du LOD réduit dès les premières bandes. L'objectif dépend du codec (JPEG/PNG OK ; TIFF/RAW variables), à mesurer dès la phase 1.

## 5. Pipeline de rendu GPU

1. **Entrée** : `RenderRequest { viewport, zoom, lod, dirty_regions }`.
2. **Aplatissement** de l'arbre en une liste d'opérations par tuile d'écran (calques visibles, masques, écrêtage, groupes).
3. **Passes** par tuile d'écran, en `rgba16float` linéaire prémultiplié :
   - raster : échantillonnage de la tuile source avec transformation (bilinéaire/bicubique) ;
   - masque et styles : multiplication par le masque, effets pré-calculés ;
   - fusion : un shader par mode (génération par `naga_oil`/macros WGSL) sur `backdrop` et `source` ;
   - **réglages non destructifs** : chaque calque de réglage est une passe plein-tuile qui lit le fond et applique la fonction (LUT 1D/3D en textures, courbes en LUT 1D 4096 entrées) avant composition par son masque ;
   - groupes isolés : rendu dans une texture intermédiaire, puis fusion ; groupes « pass-through » : les enfants fusionnent directement dans le fond.
4. **Sortie** : conversion profil document → profil écran (matrice + courbe en shader, ou LUT 3D lcms2), dessin du damier, de la grille, des règles/guides et de la sélection (« fourmis »), le tout par le GPU.
5. **Repli CPU** : `ops` fournit la même interface (`TileProcessor`) pour les filtres non portés ; le planificateur choisit selon le support (limites d'adaptateur, taille de tuile + halo).
6. **Filtres à halo** : une tuile + `halo` pixels de chaque côté (flou, netteté) ; les filtres globaux (liquify, correspondance de couleur) travaillent par passes plein-calque.

Cible : gaussien 24 Mpx < 150 ms sur GPU dédié (séparable, mémoire partagée). Sur GPU intégré, l'objectif est indicatif ; à benchmarker.

## 6. Plugins

- **Filtres WGSL** : un dossier avec `manifest.toml` (id, nom, version, licence, paramètres typés avec min/max/défaut, `halo`, nombre de passes, textures auxiliaires) et `filter.wgsl` (point d'entrée standardisé `fn apply(...)`). Le cœur génère l'UI des paramètres depuis le manifeste et valide le WGSL (naga) avant chargement ; limites de temps GPU et de taille de buffers.
- **Scripts WASM** : exécutés dans `wasmtime`, sans WASI par défaut, sans accès fichier ni réseau. L'API hôte est une liste explicite de fonctions (lire/écrire une tuile, paramètres, progression). Limites : mémoire, carburant (fuel), temps ; arrêt coopératif. Les capacités sont déclarées dans le manifeste et acceptées par l'utilisateur à l'installation.
- Intégrité : hachage du paquet ; signature optionnelle plus tard. Aucun plugin ne parle au réseau.

## 7. API de commandes IPC

- **Transport** : commandes Tauri (`invoke`) pour les requêtes, événements Tauri pour les notifications. Types partagés **générés** (`specta` ou `ts-rs`) : le TypeScript n'est jamais écrit à la main pour les DTO.
- **Principe** : le frontend n'a jamais les pixels. Il envoie des intentions ; le cœur répond par des identifiants et des métadonnées (miniatures PNG de petite taille pour les panneaux).
- **Familles** (toutes retournent `Result<T, ApiError>` avec code stable) :
  - `doc.*` : `new`, `open`, `save`, `export`, `close`, `list`, `info` ;
  - `view.*` : `set_viewport`, `zoom`, `pan`, `fit`, `set_grid/guides/rulers/snap` ;
  - `layer.*` : `add`, `remove`, `reorder`, `set_props`, `set_blend`, `mask.add/remove`, `style.set`, `transform.begin/update/commit` ;
  - `select.*` : `rect`, `ellipse`, `lasso`, `wand`, `modify`, `invert`, `to_channel` ;
  - `tool.*` : `activate`, `set_option`, `stroke.begin/point/end` (entrées stylet), `cancel` ;
  - `adjust.*`, `filter.*` : `preview.begin/update/commit/cancel` ;
  - `history.*` : `undo`, `redo`, `list`, `goto`, `snapshot.create` ;
  - `plugin.*`, `prefs.*`, `shortcuts.*`.
- **Événements** : `doc:changed {doc, dirty_layers}`, `history:changed`, `render:progress`, `job:progress`, `error:raised`.
- **Entrées de peinture** : les points sont transmis en lots avec pression, inclinaison, horodatage. Si le canvas est natif (cf. question §11), ils ne passent pas par le webview.
- **Travaux longs** : une commande retourne un `JobId` ; progression par événements ; annulation par `job.cancel`.

## 8. Qualité et CI

- Tests unitaires (`core`, `ops`), tests d'images de référence (golden, tolérance par canal documentée) pour chaque filtre, benchmarks `criterion`, tests de propriété (`proptest`) sur l'historique (appliquer/annuler = identité).
- CI GitHub Actions : `fmt`, `clippy -D warnings`, `cargo test`, `cargo deny`, build de l'installeur Windows avec étape de signature en variable factice. GPU en CI : `wgpu` avec adaptateur logiciel (lavapipe/WARP) pour les golden images.
- Accessibilité : navigation clavier complète, contrastes AA, rôles ARIA sur les panneaux. Télémétrie : aucune, et aucune requête réseau hors téléchargement de modèles IA explicitement demandé.

## 9. Sites et déploiement

`site/` contient la suite (`open.glsc.fr`) et `site/pics/` la page de téléchargement du module ; `deploy/deploy-site.sh` (rsync/SSH) publie le site, `deploy/publish-release.sh` publie les binaires et `releases.json` (empreintes SHA-256). La page n'affiche aucun lien tant qu'aucune version n'existe.

## 10. Réalisme : points durs, dits franchement

- **Canvas wgpu dans Tauri** : un webview ne peut pas héberger une surface wgpu dans le DOM (voir §11).
- **CMYK** : conversion fidèle (lcms2) oui ; édition CMYK native de bout en bout, non avant longtemps.
- **PSD** : lecture des calques raster/groupes/masques/modes réaliste ; écriture basique (raster + masques) oui ; styles, texte éditable et objets dynamiques non fidèles au départ.
- **HEIC/AVIF** : lecture via libheif/libavif ; les brevets HEVC restent à examiner pour la distribution.
- **Camera Raw, sélection par sujet, inpainting, super-résolution** : réalisables avec des modèles ONNX libres, mais la qualité visera « utile », pas la parité avec Photoshop.
- **Pinceau < 16 ms** : atteignable seulement si l'entrée stylet et le tracé évitent le webview (voir §11).
- **Ordre des dépendances** : la phase 1 doit prouver, avant d'aller plus loin, le canvas natif, l'historique par tuiles et le zoom 60 fps sur un 100 Mpx.

## 11. Question unique (avant de coder)

**Comment le canvas wgpu cohabite-t-il avec l'interface web ?**

- **A — Fenêtre enfant native (recommandé)** : le cœur crée un HWND enfant avec sa swapchain wgpu, positionné sur la zone du canvas ; le webview affiche le reste. Latence minimale, entrées souris/stylet reçues nativement (`WM_POINTER`, pression Windows Ink). Contrainte : aucun élément HTML ne peut recouvrir la zone du canvas (menus déroulants, infobulles) ; le curseur d'outil, la sélection, les guides et les poignées sont dessinés par le GPU.
- **B — Rendu vers texture puis flux de frames vers le webview** : plus simple pour la superposition HTML, mais copie CPU ou `SharedArrayBuffer` à chaque image, latence plus élevée, et les entrées stylet passent par le webview : l'objectif < 16 ms du pinceau devient incertain.

Ma recommandation par défaut est **A**. Sans autre indication, je pars sur A.

## 12. Prochaine étape

Après votre validation : Phase 1 (workspace Cargo, `core` avec tuiles et historique et leurs tests, prototype du canvas natif, ouverture/enregistrement PNG/JPEG, UI de base), avec ses livrables et critères d'acceptation.
