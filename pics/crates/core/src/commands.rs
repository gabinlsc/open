use std::collections::HashMap;

use crate::document::{Axis, Document, Guide, Layer, LayerId, LayerProps, Mask};
use crate::error::CoreError;
use crate::history::Command;
use crate::styles::LayerStyle;
use crate::tile::{TileGrid, TileKey, TileSlot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Pixels,
    Mask,
}

/// Différence de pixels d'un calque (ou de son masque) : n'embarque que les tuiles modifiées, par `Arc`.
pub struct ReplaceTiles {
    name: String,
    layer: LayerId,
    target: Target,
    before: HashMap<TileKey, TileSlot>,
    after: HashMap<TileKey, TileSlot>,
}

impl ReplaceTiles {
    /// Compare `old` (état courant du calque) à `new` (résultat d'une opération). `None` si
    /// l'opération n'a rien changé, pour ne pas polluer l'historique d'états identiques.
    pub fn between(name: impl Into<String>, layer: LayerId, old: &TileGrid, new: &TileGrid) -> Option<Self> {
        Self::build(name, layer, Target::Pixels, old, new)
    }

    /// Idem pour le masque du calque.
    pub fn between_mask(name: impl Into<String>, layer: LayerId, old: &TileGrid, new: &TileGrid) -> Option<Self> {
        Self::build(name, layer, Target::Mask, old, new)
    }

    fn build(name: impl Into<String>, layer: LayerId, target: Target, old: &TileGrid, new: &TileGrid) -> Option<Self> {
        let changed = old.changed_tiles(new);
        if changed.is_empty() {
            return None;
        }
        let mut before = HashMap::with_capacity(changed.len());
        let mut after = HashMap::with_capacity(changed.len());
        for (key, b, a) in changed {
            before.insert(key, b);
            after.insert(key, a);
        }
        Some(Self { name: name.into(), layer, target, before, after })
    }

    fn write(&self, doc: &mut Document, tiles: &HashMap<TileKey, TileSlot>) -> Result<(), CoreError> {
        let layer = doc.layer_mut(self.layer)?;
        let grid = match self.target {
            Target::Pixels => &mut layer.pixels,
            Target::Mask => &mut layer.mask.as_mut().ok_or(CoreError::NoMask(self.layer.0))?.pixels,
        };
        for (key, tile) in tiles {
            grid.set_tile(*key, tile.clone());
        }
        Ok(())
    }
}

impl Command for ReplaceTiles {
    fn name(&self) -> &str {
        &self.name
    }

    fn apply(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        self.write(doc, &self.after)
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        self.write(doc, &self.before)
    }
}

pub struct AddGuide {
    axis: Axis,
    position: f64,
    id: Option<u64>,
}

impl AddGuide {
    pub fn new(axis: Axis, position: f64) -> Self {
        Self { axis, position, id: None }
    }
}

impl Command for AddGuide {
    fn name(&self) -> &str {
        "Ajouter un repère"
    }

    fn apply(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        // L'id est conservé d'un apply à l'autre pour que le redo restaure le même repère.
        let id = match self.id {
            Some(id) => id,
            None => {
                let id = doc.alloc_id();
                self.id = Some(id);
                id
            }
        };
        doc.guides.push(Guide { id, axis: self.axis, position: self.position });
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let id = self.id.ok_or(CoreError::GuideNotFound(0))?;
        doc.guides.retain(|g| g.id != id);
        Ok(())
    }
}

pub struct MoveGuide {
    id: u64,
    to: f64,
    from: Option<f64>,
}

impl MoveGuide {
    pub fn new(id: u64, to: f64) -> Self {
        Self { id, to, from: None }
    }
}

impl Command for MoveGuide {
    fn name(&self) -> &str {
        "Déplacer un repère"
    }

    fn apply(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let g = doc.guides.iter_mut().find(|g| g.id == self.id).ok_or(CoreError::GuideNotFound(self.id))?;
        self.from = Some(g.position);
        g.position = self.to;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let from = self.from.ok_or(CoreError::GuideNotFound(self.id))?;
        let g = doc.guides.iter_mut().find(|g| g.id == self.id).ok_or(CoreError::GuideNotFound(self.id))?;
        g.position = from;
        Ok(())
    }
}

pub struct RemoveGuide {
    id: u64,
    removed: Option<(usize, Guide)>,
}

impl RemoveGuide {
    pub fn new(id: u64) -> Self {
        Self { id, removed: None }
    }
}

impl Command for RemoveGuide {
    fn name(&self) -> &str {
        "Supprimer un repère"
    }

    fn apply(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let idx = doc.guides.iter().position(|g| g.id == self.id).ok_or(CoreError::GuideNotFound(self.id))?;
        self.removed = Some((idx, doc.guides.remove(idx)));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let (idx, guide) = self.removed.ok_or(CoreError::GuideNotFound(self.id))?;
        doc.guides.insert(idx.min(doc.guides.len()), guide);
        Ok(())
    }
}

/// Efface tous les repères en un seul état d'historique.
#[derive(Default)]
pub struct ClearGuides {
    removed: Vec<Guide>,
}

impl ClearGuides {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Command for ClearGuides {
    fn name(&self) -> &str {
        "Effacer les repères"
    }

    fn apply(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        self.removed = std::mem::take(&mut doc.guides);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        doc.guides = std::mem::take(&mut self.removed);
        Ok(())
    }
}

/// Modifie les propriétés d'un calque (nom, visibilité, opacité, fusion, verrous, liaison, écrêtage).
pub struct SetLayerProps {
    name: String,
    layer: LayerId,
    after: LayerProps,
    before: Option<LayerProps>,
}

impl SetLayerProps {
    pub fn new(name: impl Into<String>, layer: LayerId, after: LayerProps) -> Self {
        Self { name: name.into(), layer, after, before: None }
    }
}

impl Command for SetLayerProps {
    fn name(&self) -> &str {
        &self.name
    }

    fn apply(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let layer = doc.layer_mut(self.layer)?;
        self.before = Some(layer.props());
        layer.set_props(&self.after);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let before = self.before.clone().ok_or(CoreError::LayerNotFound(self.layer.0))?;
        doc.layer_mut(self.layer)?.set_props(&before);
        Ok(())
    }
}

fn check_index(index: usize, len: usize) -> Result<(), CoreError> {
    if index > len { Err(CoreError::LayerIndex { index, len }) } else { Ok(()) }
}

/// Insère un calque à `index` (0 = tout en bas).
pub struct AddLayer {
    index: usize,
    layer: Option<Layer>,
    id: LayerId,
}

impl AddLayer {
    pub fn new(index: usize, layer: Layer) -> Self {
        Self { index, id: layer.id, layer: Some(layer) }
    }
}

impl Command for AddLayer {
    fn name(&self) -> &str {
        "Nouveau calque"
    }

    fn apply(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        check_index(self.index, doc.layers.len())?;
        let layer = self.layer.take().ok_or(CoreError::LayerNotFound(self.id.0))?;
        doc.layers.insert(self.index, layer);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let idx = doc.layers.iter().position(|l| l.id == self.id).ok_or(CoreError::LayerNotFound(self.id.0))?;
        self.layer = Some(doc.layers.remove(idx));
        Ok(())
    }
}

pub struct RemoveLayer {
    id: LayerId,
    removed: Option<(usize, Layer)>,
}

impl RemoveLayer {
    pub fn new(id: LayerId) -> Self {
        Self { id, removed: None }
    }
}

impl Command for RemoveLayer {
    fn name(&self) -> &str {
        "Supprimer le calque"
    }

    fn apply(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let idx = doc.layers.iter().position(|l| l.id == self.id).ok_or(CoreError::LayerNotFound(self.id.0))?;
        self.removed = Some((idx, doc.layers.remove(idx)));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let (idx, layer) = self.removed.take().ok_or(CoreError::LayerNotFound(self.id.0))?;
        doc.layers.insert(idx.min(doc.layers.len()), layer);
        Ok(())
    }
}

/// Déplace un calque vers l'index `to` (index final dans la pile, 0 = tout en bas).
pub struct MoveLayer {
    id: LayerId,
    to: usize,
    from: Option<usize>,
}

impl MoveLayer {
    pub fn new(id: LayerId, to: usize) -> Self {
        Self { id, to, from: None }
    }
}

impl Command for MoveLayer {
    fn name(&self) -> &str {
        "Réorganiser les calques"
    }

    fn apply(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let len = doc.layers.len();
        if self.to >= len {
            return Err(CoreError::LayerIndex { index: self.to, len });
        }
        let from = doc.layers.iter().position(|l| l.id == self.id).ok_or(CoreError::LayerNotFound(self.id.0))?;
        let layer = doc.layers.remove(from);
        doc.layers.insert(self.to, layer);
        self.from = Some(from);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let from = self.from.ok_or(CoreError::LayerNotFound(self.id.0))?;
        let now = doc.layers.iter().position(|l| l.id == self.id).ok_or(CoreError::LayerNotFound(self.id.0))?;
        let layer = doc.layers.remove(now);
        doc.layers.insert(from.min(doc.layers.len()), layer);
        Ok(())
    }
}

/// Crée un masque : `reveal` = tout visible (blanc), sinon tout caché (noir).
pub struct AddMask {
    layer: LayerId,
    reveal: bool,
}

impl AddMask {
    pub fn new(layer: LayerId, reveal: bool) -> Self {
        Self { layer, reveal }
    }
}

impl Command for AddMask {
    fn name(&self) -> &str {
        if self.reveal { "Masque : tout révéler" } else { "Masque : tout masquer" }
    }

    fn apply(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let reveal = self.reveal;
        let layer = doc.layer_mut(self.layer)?;
        if layer.mask.is_some() {
            return Err(CoreError::HasMask(self.layer.0));
        }
        let mut grid = TileGrid::new(layer.pixels.width(), layer.pixels.height())?;
        if reveal {
            let white = std::sync::Arc::new(crate::tile::Tile::Uniform([255; 4]));
            for ty in 0..grid.tiles_y() {
                for tx in 0..grid.tiles_x() {
                    grid.set_tile(TileKey { tx, ty }, Some(white.clone()));
                }
            }
        }
        layer.mask = Some(Mask { pixels: grid, enabled: true });
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        doc.layer_mut(self.layer)?.mask = None;
        Ok(())
    }
}

pub struct RemoveMask {
    layer: LayerId,
    removed: Option<Mask>,
}

impl RemoveMask {
    pub fn new(layer: LayerId) -> Self {
        Self { layer, removed: None }
    }
}

impl Command for RemoveMask {
    fn name(&self) -> &str {
        "Supprimer le masque"
    }

    fn apply(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        self.removed = Some(doc.layer_mut(self.layer)?.mask.take().ok_or(CoreError::NoMask(self.layer.0))?);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        doc.layer_mut(self.layer)?.mask = self.removed.take();
        Ok(())
    }
}

pub struct SetMaskEnabled {
    layer: LayerId,
    enabled: bool,
    before: Option<bool>,
}

impl SetMaskEnabled {
    pub fn new(layer: LayerId, enabled: bool) -> Self {
        Self { layer, enabled, before: None }
    }
}

impl Command for SetMaskEnabled {
    fn name(&self) -> &str {
        if self.enabled { "Activer le masque" } else { "Désactiver le masque" }
    }

    fn apply(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let mask = doc.layer_mut(self.layer)?.mask.as_mut().ok_or(CoreError::NoMask(self.layer.0))?;
        self.before = Some(mask.enabled);
        mask.enabled = self.enabled;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let before = self.before.ok_or(CoreError::NoMask(self.layer.0))?;
        doc.layer_mut(self.layer)?.mask.as_mut().ok_or(CoreError::NoMask(self.layer.0))?.enabled = before;
        Ok(())
    }
}

/// Remplace la liste de styles d'un calque (valeurs bornées à l'application).
pub struct SetLayerStyles {
    name: String,
    layer: LayerId,
    after: Vec<LayerStyle>,
    before: Option<Vec<LayerStyle>>,
}

impl SetLayerStyles {
    pub fn new(name: impl Into<String>, layer: LayerId, styles: Vec<LayerStyle>) -> Self {
        Self { name: name.into(), layer, after: styles.iter().map(LayerStyle::sanitized).collect(), before: None }
    }
}

impl Command for SetLayerStyles {
    fn name(&self) -> &str {
        &self.name
    }

    fn apply(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let layer = doc.layer_mut(self.layer)?;
        self.before = Some(std::mem::replace(&mut layer.styles, self.after.clone()));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), CoreError> {
        let before = self.before.take().ok_or(CoreError::LayerNotFound(self.layer.0))?;
        doc.layer_mut(self.layer)?.styles = before;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocId;
    use crate::geom::Size;

    fn doc() -> Document {
        Document::new(DocId(1), "t", Size::new(600, 600)).unwrap()
    }

    #[test]
    fn replace_tiles_undo_redo_restores_pixels() {
        let mut d = doc();
        let layer = d.layers[0].id;
        let old = d.layers[0].pixels.clone();
        let mut new = old.clone();
        new.put_pixel(300, 10, [1, 2, 3, 255]);
        let cmd = ReplaceTiles::between("test", layer, &old, &new).unwrap();
        d.execute(Box::new(cmd)).unwrap();
        assert_eq!(d.layers[0].pixels.get_pixel(300, 10), [1, 2, 3, 255]);
        d.undo().unwrap();
        assert_eq!(d.layers[0].pixels.get_pixel(300, 10), [0; 4]);
        assert_eq!(d.layers[0].pixels.tile_count(), 0);
        d.redo().unwrap();
        assert_eq!(d.layers[0].pixels.get_pixel(300, 10), [1, 2, 3, 255]);
    }

    #[test]
    fn identical_result_yields_no_command() {
        let d = doc();
        let same = d.layers[0].pixels.clone();
        assert!(ReplaceTiles::between("x", d.layers[0].id, &d.layers[0].pixels, &same).is_none());
    }

    #[test]
    fn only_changed_tiles_are_retained() {
        let d = doc();
        let mut old = d.layers[0].pixels.clone();
        old.put_pixel(0, 0, [9, 9, 9, 255]);
        old.put_pixel(599, 599, [9, 9, 9, 255]);
        let mut new = old.clone();
        new.put_pixel(599, 599, [1, 1, 1, 255]);
        let cmd = ReplaceTiles::between("x", d.layers[0].id, &old, &new).unwrap();
        assert_eq!(cmd.after.len(), 1);
    }

    #[test]
    fn guide_commands_roundtrip_through_history() {
        let mut d = doc();
        d.execute(Box::new(AddGuide::new(Axis::Horizontal, 10.0))).unwrap();
        let id = d.guides[0].id;
        d.execute(Box::new(MoveGuide::new(id, 50.0))).unwrap();
        d.execute(Box::new(RemoveGuide::new(id))).unwrap();
        assert!(d.guides.is_empty());
        d.undo().unwrap();
        assert_eq!(d.guides[0].position, 50.0);
        d.undo().unwrap();
        assert_eq!(d.guides[0].position, 10.0);
        d.undo().unwrap();
        assert!(d.guides.is_empty());
        d.redo().unwrap();
        assert_eq!(d.guides[0].id, id);
    }

    #[test]
    fn clear_guides_is_one_undoable_step() {
        let mut d = doc();
        d.execute(Box::new(AddGuide::new(Axis::Vertical, 1.0))).unwrap();
        d.execute(Box::new(AddGuide::new(Axis::Horizontal, 2.0))).unwrap();
        d.execute(Box::new(ClearGuides::new())).unwrap();
        assert!(d.guides.is_empty());
        d.undo().unwrap();
        assert_eq!(d.guides.len(), 2);
        d.redo().unwrap();
        assert!(d.guides.is_empty());
    }

    #[test]
    fn history_entries_report_position() {
        let mut d = doc();
        d.execute(Box::new(AddGuide::new(Axis::Vertical, 1.0))).unwrap();
        d.execute(Box::new(AddGuide::new(Axis::Vertical, 2.0))).unwrap();
        d.undo().unwrap();
        let (names, pos) = d.history().entries();
        assert_eq!(names.len(), 2);
        assert_eq!(pos, 1);
    }

    #[test]
    fn move_unknown_guide_is_an_error() {
        let mut d = doc();
        assert_eq!(d.execute(Box::new(MoveGuide::new(99, 1.0))), Err(CoreError::GuideNotFound(99)));
        assert!(!d.history().can_undo());
    }

    fn two_layer_doc() -> (Document, LayerId, LayerId) {
        let mut d = doc();
        let bottom = d.layers[0].id;
        let top = d.add_layer("Haut", TileGrid::new(600, 600).unwrap());
        (d, bottom, top)
    }

    #[test]
    fn layer_props_roundtrip_through_history() {
        let (mut d, _, top) = two_layer_doc();
        let mut p = d.layer(top).unwrap().props();
        p.opacity = 0.4;
        p.blend = crate::BlendMode::Multiply;
        p.clip_to_below = true;
        d.execute(Box::new(SetLayerProps::new("Propriétés", top, p))).unwrap();
        assert_eq!(d.layer(top).unwrap().opacity, 0.4);
        d.undo().unwrap();
        let l = d.layer(top).unwrap();
        assert_eq!((l.opacity, l.blend, l.clip_to_below), (1.0, crate::BlendMode::Normal, false));
        d.redo().unwrap();
        assert!(d.layer(top).unwrap().clip_to_below);
    }

    #[test]
    fn out_of_range_opacity_is_clamped_never_invalid() {
        let (mut d, _, top) = two_layer_doc();
        let mut p = d.layer(top).unwrap().props();
        p.opacity = 7.0;
        p.fill = f32::NAN;
        d.execute(Box::new(SetLayerProps::new("x", top, p))).unwrap();
        let l = d.layer(top).unwrap();
        assert_eq!(l.opacity, 1.0);
        assert_eq!(l.fill, 1.0);
    }

    #[test]
    fn add_remove_move_layers_are_undoable() {
        let (mut d, bottom, top) = two_layer_doc();
        let extra = Layer::new(LayerId(d.alloc_id()), "Milieu", TileGrid::new(600, 600).unwrap());
        let extra_id = extra.id;
        d.execute(Box::new(AddLayer::new(1, extra))).unwrap();
        assert_eq!(d.layers.iter().map(|l| l.id).collect::<Vec<_>>(), vec![bottom, extra_id, top]);
        d.execute(Box::new(MoveLayer::new(extra_id, 2))).unwrap();
        assert_eq!(d.layers.iter().map(|l| l.id).collect::<Vec<_>>(), vec![bottom, top, extra_id]);
        d.execute(Box::new(RemoveLayer::new(top))).unwrap();
        assert_eq!(d.layers.len(), 2);
        d.undo().unwrap();
        assert_eq!(d.layers.iter().map(|l| l.id).collect::<Vec<_>>(), vec![bottom, top, extra_id]);
        d.undo().unwrap();
        assert_eq!(d.layers.iter().map(|l| l.id).collect::<Vec<_>>(), vec![bottom, extra_id, top]);
        d.undo().unwrap();
        assert_eq!(d.layers.len(), 2);
        d.redo().unwrap();
        assert_eq!(d.layers.len(), 3);
    }

    #[test]
    fn invalid_layer_indices_are_rejected_without_touching_history() {
        let (mut d, bottom, _) = two_layer_doc();
        assert!(matches!(d.execute(Box::new(MoveLayer::new(bottom, 5))), Err(CoreError::LayerIndex { .. })));
        let l = Layer::new(LayerId(99), "x", TileGrid::new(600, 600).unwrap());
        assert!(matches!(d.execute(Box::new(AddLayer::new(9, l))), Err(CoreError::LayerIndex { .. })));
        assert!(matches!(d.execute(Box::new(RemoveLayer::new(LayerId(404)))), Err(CoreError::LayerNotFound(404))));
        assert!(!d.history().can_undo());
    }

    #[test]
    fn removing_every_layer_keeps_the_canvas_size() {
        let (mut d, bottom, top) = two_layer_doc();
        d.execute(Box::new(RemoveLayer::new(top))).unwrap();
        d.execute(Box::new(RemoveLayer::new(bottom))).unwrap();
        assert_eq!(d.size(), crate::Size::new(600, 600));
        assert!(d.flatten_rgba8().iter().all(|&b| b == 0));
    }

    #[test]
    fn masks_reveal_hide_toggle_and_edit_with_history() {
        let (mut d, _, top) = two_layer_doc();
        d.layer_mut(top).unwrap().pixels.put_pixel(5, 5, [255, 0, 0, 255]);
        d.execute(Box::new(AddMask::new(top, false))).unwrap();
        assert!(d.flatten_rgba8()[(5 * 600 + 5) * 4 + 3] == 0, "masque noir : calque invisible");
        assert!(matches!(d.execute(Box::new(AddMask::new(top, true))), Err(CoreError::HasMask(_))));
        // Peindre le masque en blanc sur un pixel (via le même mécanisme de tuiles que les pixels).
        let old = d.layer(top).unwrap().mask.as_ref().unwrap().pixels.clone();
        let mut new = old.clone();
        new.put_pixel(5, 5, [255, 255, 255, 255]);
        d.execute(Box::new(ReplaceTiles::between_mask("Peindre le masque", top, &old, &new).unwrap())).unwrap();
        assert_eq!(d.flatten_rgba8()[(5 * 600 + 5) * 4 + 3], 255);
        d.undo().unwrap();
        assert_eq!(d.flatten_rgba8()[(5 * 600 + 5) * 4 + 3], 0);
        d.execute(Box::new(SetMaskEnabled::new(top, false))).unwrap();
        assert_eq!(d.flatten_rgba8()[(5 * 600 + 5) * 4 + 3], 255);
        d.undo().unwrap();
        d.execute(Box::new(RemoveMask::new(top))).unwrap();
        assert!(d.layer(top).unwrap().mask.is_none());
        d.undo().unwrap();
        assert!(d.layer(top).unwrap().mask.is_some());
        assert!(matches!(d.execute(Box::new(RemoveMask::new(LayerId(1)))), Err(CoreError::NoMask(1))));
    }

    #[test]
    fn reveal_all_mask_changes_nothing_visually() {
        let (mut d, _, top) = two_layer_doc();
        d.layer_mut(top).unwrap().pixels.put_pixel(7, 7, [10, 20, 30, 255]);
        let before = d.flatten_rgba8();
        d.execute(Box::new(AddMask::new(top, true))).unwrap();
        assert_eq!(d.flatten_rgba8(), before);
    }

    #[test]
    fn layer_styles_are_sanitized_and_undoable() {
        let (mut d, _, top) = two_layer_doc();
        let wild = LayerStyle::OuterGlow { color: [1, 2, 3], opacity: 5.0, size: 1e9 };
        d.execute(Box::new(SetLayerStyles::new("Styles de calque", top, vec![wild]))).unwrap();
        match &d.layer(top).unwrap().styles[0] {
            LayerStyle::OuterGlow { opacity, size, .. } => {
                assert_eq!((*opacity, *size), (1.0, crate::styles::MAX_SIZE))
            }
            other => panic!("{other:?}"),
        }
        d.undo().unwrap();
        assert!(d.layer(top).unwrap().styles.is_empty());
        d.redo().unwrap();
        assert_eq!(d.layer(top).unwrap().styles.len(), 1);
    }
}
