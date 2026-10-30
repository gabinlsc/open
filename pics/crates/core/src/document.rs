use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::adjust::Adjustment;
use crate::blend::BlendMode;
use crate::error::CoreError;
use crate::geom::Size;
use crate::history::{Command, History};
use crate::styles::LayerStyle;
use crate::text::TextModel;
use crate::tile::TileGrid;
use crate::vector::VectorModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LayerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Repère magnétique ; `position` en pixels de document.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Guide {
    pub id: u64,
    pub axis: Axis,
    pub position: f64,
}

/// Verrous d'un calque. `all` interdit toute modification ; `pixels` interdit de peindre ;
/// `position` interdit de déplacer ou transformer (appliqué par les outils de transformation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Locks {
    pub pixels: bool,
    pub position: bool,
    pub all: bool,
}

impl Locks {
    pub fn blocks_painting(&self) -> bool {
        self.pixels || self.all
    }

    pub fn blocks_moving(&self) -> bool {
        self.position || self.all
    }
}

/// Masque de calque : la valeur est le canal rouge (0 = caché, 255 = visible). Une tuile absente
/// vaut donc « caché » ; un masque « tout révéler » est fait de tuiles uniformes blanches.
#[derive(Debug, Clone)]
pub struct Mask {
    pub pixels: TileGrid,
    pub enabled: bool,
}

/// Propriétés d'un calque hors pixels : c'est ce qu'enregistre `SetLayerProps` pour l'historique.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerProps {
    pub name: String,
    pub visible: bool,
    /// 0..=1. Multiplie l'alpha du calque *et* de ses styles.
    pub opacity: f32,
    /// 0..=1. Multiplie l'alpha du contenu seulement : les styles (ombre, contour…) n'en dépendent pas.
    pub fill: f32,
    pub blend: BlendMode,
    pub locks: Locks,
    /// Les calques d'un même groupe de liaison se déplacent ensemble.
    pub link: Option<u64>,
    /// Écrêtage : le calque n'apparaît que là où le calque non écrêté situé dessous est opaque.
    pub clip_to_below: bool,
    /// Groupes seulement : `true` = groupe isolé, `false` = « traverser » (les enfants se fusionnent
    /// directement avec ce qui est dessous). Sans effet sur un calque raster.
    pub isolated: bool,
}

/// Nature d'un calque : pixels, groupe qui en contient d'autres, ou réglage appliqué à ce qui est dessous.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum LayerKind {
    #[default]
    Raster,
    Group {
        isolated: bool,
    },
    Adjustment(Box<Adjustment>),
    /// Texte : le modèle est la source ; les pixels du calque en sont le rendu en cache.
    Text(Box<TextModel>),
    /// Forme vectorielle : idem, les pixels sont le rendu du modèle.
    Shape(Box<VectorModel>),
}

/// Calque raster ou groupe (un groupe n'a pas de pixels propres : sa grille reste vide). Les réglages, textes
/// et objets dynamiques arrivent ensuite en généralisant le contenu, sans toucher à `TileGrid` ni à l'historique.
#[derive(Debug, Clone)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub fill: f32,
    pub blend: BlendMode,
    pub locks: Locks,
    pub link: Option<u64>,
    pub clip_to_below: bool,
    pub kind: LayerKind,
    /// Groupe parent ; `None` = niveau supérieur.
    pub group: Option<LayerId>,
    pub mask: Option<Mask>,
    /// Styles de calque, du premier appliqué au dernier ; vide par défaut.
    pub styles: Vec<LayerStyle>,
    pub pixels: TileGrid,
}

impl Layer {
    pub fn new(id: LayerId, name: impl Into<String>, pixels: TileGrid) -> Self {
        Self {
            id,
            name: name.into(),
            visible: true,
            opacity: 1.0,
            fill: 1.0,
            blend: BlendMode::Normal,
            locks: Locks::default(),
            link: None,
            clip_to_below: false,
            kind: LayerKind::Raster,
            group: None,
            mask: None,
            styles: Vec::new(),
            pixels,
        }
    }

    /// Groupe vide : il n'a pas de pixels, seulement des enfants (isolé par défaut).
    pub fn new_group(id: LayerId, name: impl Into<String>, pixels: TileGrid) -> Self {
        Self { kind: LayerKind::Group { isolated: true }, ..Layer::new(id, name, pixels) }
    }

    pub fn is_group(&self) -> bool {
        matches!(self.kind, LayerKind::Group { .. })
    }

    /// Calque de pixels : le seul type que l'on peut peindre, transformer ou filtrer.
    pub fn is_raster(&self) -> bool {
        matches!(self.kind, LayerKind::Raster)
    }

    /// Vrai si le calque porte des pixels (raster, texte, forme) ; faux pour un groupe ou un réglage.
    pub fn has_pixels(&self) -> bool {
        matches!(self.kind, LayerKind::Raster | LayerKind::Text(_) | LayerKind::Shape(_))
    }

    pub fn text(&self) -> Option<&TextModel> {
        match &self.kind {
            LayerKind::Text(t) => Some(t),
            _ => None,
        }
    }

    pub fn shape(&self) -> Option<&VectorModel> {
        match &self.kind {
            LayerKind::Shape(v) => Some(v),
            _ => None,
        }
    }

    pub fn adjustment(&self) -> Option<&Adjustment> {
        match &self.kind {
            LayerKind::Adjustment(a) => Some(a),
            _ => None,
        }
    }

    /// Calque de réglage : sans pixels, il applique `adjustment` à ce qui est dessous.
    pub fn new_adjustment(id: LayerId, name: impl Into<String>, pixels: TileGrid, adjustment: Adjustment) -> Self {
        Self { kind: LayerKind::Adjustment(Box::new(adjustment.sanitized())), ..Layer::new(id, name, pixels) }
    }

    pub fn props(&self) -> LayerProps {
        LayerProps {
            name: self.name.clone(),
            visible: self.visible,
            opacity: self.opacity,
            fill: self.fill,
            blend: self.blend,
            locks: self.locks,
            link: self.link,
            clip_to_below: self.clip_to_below,
            isolated: !matches!(&self.kind, LayerKind::Group { isolated: false }),
        }
    }

    /// Applique des propriétés en bornant opacité et remplissage : une valeur hors de 0..=1 (ou NaN)
    /// ne doit jamais produire un alpha invalide au rendu.
    pub fn set_props(&mut self, p: &LayerProps) {
        self.name = p.name.clone();
        self.visible = p.visible;
        self.opacity = clamp_unit(p.opacity);
        self.fill = clamp_unit(p.fill);
        self.blend = p.blend;
        self.locks = p.locks;
        self.link = p.link;
        self.clip_to_below = p.clip_to_below;
        if let LayerKind::Group { isolated } = &mut self.kind {
            *isolated = p.isolated;
        }
    }
}

fn clamp_unit(v: f32) -> f32 {
    if v.is_nan() { 1.0 } else { v.clamp(0.0, 1.0) }
}

#[derive(Debug)]
pub struct Document {
    pub id: DocId,
    pub name: String,
    pub path: Option<PathBuf>,
    size: Size,
    pub layers: Vec<Layer>,
    pub guides: Vec<Guide>,
    /// Sélection active (masque 0..255) ; `None` = tout est modifiable.
    pub selection: Option<TileGrid>,
    history: History,
    revision: u64,
    next_id: u64,
}

impl Document {
    /// Document vide : un fond transparent.
    pub fn new(id: DocId, name: impl Into<String>, size: Size) -> Result<Self, CoreError> {
        let background = TileGrid::new(size.width, size.height)?;
        let mut doc = Self {
            id,
            name: name.into(),
            path: None,
            size,
            layers: Vec::new(),
            guides: Vec::new(),
            selection: None,
            history: History::default(),
            revision: 0,
            next_id: 1,
        };
        doc.add_layer("Arrière-plan", background);
        Ok(doc)
    }

    /// Taille du canevas, indépendante des calques : supprimer tous les calques ne la change pas.
    pub fn size(&self) -> Size {
        self.size
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_layer(&mut self, name: impl Into<String>, pixels: TileGrid) -> LayerId {
        let id = LayerId(self.alloc_id());
        self.layers.push(Layer::new(id, name, pixels));
        id
    }

    /// Réserve les identifiants déjà utilisés par un document relu depuis un fichier.
    pub fn reserve_ids_up_to(&mut self, max_used: u64) {
        self.next_id = self.next_id.max(max_used + 1);
    }

    /// Vrai si `ancestor` est `id` lui-même ou l'un de ses groupes parents (sert à refuser les cycles).
    pub fn is_ancestor_or_self(&self, ancestor: LayerId, id: LayerId) -> bool {
        let mut cur = Some(id);
        for _ in 0..=self.layers.len() {
            match cur {
                Some(c) if c == ancestor => return true,
                Some(c) => cur = self.layers.iter().find(|l| l.id == c).and_then(|l| l.group),
                None => return false,
            }
        }
        true // boucle déjà présente : on la traite comme un cycle
    }

    /// Imbrication d'un calque (0 = niveau supérieur).
    pub fn depth(&self, id: LayerId) -> usize {
        let mut depth = 0;
        let mut cur = self.layers.iter().find(|l| l.id == id).and_then(|l| l.group);
        while let Some(g) = cur {
            depth += 1;
            if depth > self.layers.len() {
                break;
            }
            cur = self.layers.iter().find(|l| l.id == g).and_then(|l| l.group);
        }
        depth
    }

    /// Ordre d'affichage d'un panneau : du haut vers le bas, chaque groupe suivi de ses enfants.
    pub fn tree_top_first(&self) -> Vec<(LayerId, usize)> {
        fn walk(d: &Document, parent: Option<LayerId>, depth: usize, out: &mut Vec<(LayerId, usize)>) {
            for l in d.layers.iter().rev().filter(|l| l.group == parent) {
                out.push((l.id, depth));
                if l.is_group() && depth < 64 {
                    walk(d, Some(l.id), depth + 1, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(self, None, 0, &mut out);
        out
    }

    pub fn layer(&self, id: LayerId) -> Result<&Layer, CoreError> {
        self.layers.iter().find(|l| l.id == id).ok_or(CoreError::LayerNotFound(id.0))
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Result<&mut Layer, CoreError> {
        self.layers.iter_mut().find(|l| l.id == id).ok_or(CoreError::LayerNotFound(id.0))
    }

    /// Copie sans historique pour le rendu : cloner une `TileGrid` ne copie que des `Arc`, donc le
    /// thread de rendu travaille sur un instantané sans garder le verrou du document pendant le vsync.
    pub fn render_snapshot(&self) -> Document {
        Document {
            id: self.id,
            name: self.name.clone(),
            path: None,
            size: self.size,
            layers: self.layers.clone(),
            guides: self.guides.clone(),
            selection: self.selection.clone(),
            history: History::default(),
            revision: self.revision,
            next_id: self.next_id,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Une révision par modification (y compris annuler/rétablir) : sert à savoir si le contenu a
    /// changé, indépendamment de l'état « enregistré ».
    pub fn touch(&mut self) {
        self.revision += 1;
    }

    pub fn is_dirty(&self) -> bool {
        !self.history.is_at_saved_state()
    }

    /// Pour un document restauré d'une récupération : il n'existe sur aucun fichier de l'utilisateur.
    pub fn mark_unsaved(&mut self) {
        self.history.mark_unsaved();
    }

    pub fn mark_saved(&mut self) {
        self.history.mark_saved();
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn execute(&mut self, mut command: Box<dyn Command>) -> Result<(), CoreError> {
        command.apply(self)?;
        self.history.record(command);
        self.touch();
        Ok(())
    }

    /// `Ok(false)` s'il n'y a rien à annuler. Une commande qui échoue au `revert` reste en pile.
    pub fn undo(&mut self) -> Result<bool, CoreError> {
        let Some(mut cmd) = self.history.take_undo() else {
            return Ok(false);
        };
        match cmd.revert(self) {
            Ok(()) => {
                self.history.push_redo(cmd);
                self.touch();
                Ok(true)
            }
            Err(e) => {
                self.history.restore_undo(cmd);
                Err(e)
            }
        }
    }

    pub fn redo(&mut self) -> Result<bool, CoreError> {
        let Some(mut cmd) = self.history.take_redo() else {
            return Ok(false);
        };
        match cmd.apply(self) {
            Ok(()) => {
                self.history.restore_undo(cmd);
                self.touch();
                Ok(true)
            }
            Err(e) => {
                self.history.push_redo(cmd);
                Err(e)
            }
        }
    }

    /// Aplatit le document (modes de fusion, opacité, masques, écrêtage) en RGBA 8 bits non prémultiplié.
    pub fn flatten_rgba8(&self) -> Vec<u8> {
        crate::composite::flatten_rgba8(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::AddGuide;

    fn doc() -> Document {
        Document::new(DocId(1), "t", Size::new(10, 10)).unwrap()
    }

    #[test]
    fn execute_undo_redo_and_dirty_flag() {
        let mut d = doc();
        assert!(!d.is_dirty());
        d.execute(Box::new(AddGuide::new(Axis::Vertical, 5.0))).unwrap();
        assert_eq!(d.guides.len(), 1);
        assert!(d.is_dirty());
        assert!(d.undo().unwrap());
        assert!(d.guides.is_empty());
        assert!(d.redo().unwrap());
        assert_eq!(d.guides.len(), 1);
        d.mark_saved();
        assert!(!d.is_dirty());
        assert!(d.undo().unwrap());
        assert!(d.is_dirty());
        // Revenir à l'état enregistré (par redo) n'est plus une modification.
        assert!(d.redo().unwrap());
        assert!(!d.is_dirty());
    }

    #[test]
    fn saved_state_lost_when_its_redo_branch_is_discarded() {
        let mut d = doc();
        d.execute(Box::new(AddGuide::new(Axis::Vertical, 1.0))).unwrap();
        d.mark_saved();
        d.undo().unwrap();
        // Nouvelle action : l'état enregistré (1 guide « Vertical 1 ») n'existe plus nulle part.
        d.execute(Box::new(AddGuide::new(Axis::Horizontal, 2.0))).unwrap();
        assert!(d.is_dirty());
        d.undo().unwrap();
        assert!(d.is_dirty());
    }

    #[test]
    fn new_command_clears_redo_stack() {
        let mut d = doc();
        d.execute(Box::new(AddGuide::new(Axis::Vertical, 1.0))).unwrap();
        d.undo().unwrap();
        assert!(d.history().can_redo());
        d.execute(Box::new(AddGuide::new(Axis::Horizontal, 2.0))).unwrap();
        assert!(!d.history().can_redo());
        assert!(!d.redo().unwrap());
    }

    #[test]
    fn render_snapshot_shares_tiles_and_drops_history() {
        let mut d = doc();
        d.layers[0].pixels.put_pixel(1, 1, [1, 2, 3, 255]);
        d.execute(Box::new(AddGuide::new(Axis::Vertical, 5.0))).unwrap();
        let snap = d.render_snapshot();
        assert_eq!(snap.guides, d.guides);
        assert_eq!(snap.layers[0].pixels.get_pixel(1, 1), [1, 2, 3, 255]);
        assert!(!snap.history().can_undo());
        assert_eq!(snap.revision(), d.revision());
    }

    #[test]
    fn flatten_composites_over() {
        let mut d = Document::new(DocId(1), "t", Size::new(1, 1)).unwrap();
        d.layers[0].pixels.put_pixel(0, 0, [255, 0, 0, 255]);
        let mut top = TileGrid::new(1, 1).unwrap();
        top.put_pixel(0, 0, [0, 0, 255, 128]);
        d.add_layer("haut", top);
        let flat = d.flatten_rgba8();
        assert_eq!(flat[3], 255);
        assert!((i32::from(flat[2]) - 128).abs() <= 1);
        assert!((i32::from(flat[0]) - 127).abs() <= 1);
    }
}
