use std::collections::HashMap;

use crate::document::{Axis, Document, Guide, LayerId};
use crate::error::CoreError;
use crate::history::Command;
use crate::tile::{TileGrid, TileKey, TileSlot};

/// Différence de pixels d'un calque : n'embarque que les tuiles modifiées, par `Arc`.
pub struct ReplaceTiles {
    name: String,
    layer: LayerId,
    before: HashMap<TileKey, TileSlot>,
    after: HashMap<TileKey, TileSlot>,
}

impl ReplaceTiles {
    /// Compare `old` (état courant du calque) à `new` (résultat d'une opération). `None` si
    /// l'opération n'a rien changé, pour ne pas polluer l'historique d'états identiques.
    pub fn between(name: impl Into<String>, layer: LayerId, old: &TileGrid, new: &TileGrid) -> Option<Self> {
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
        Some(Self { name: name.into(), layer, before, after })
    }

    fn write(&self, doc: &mut Document, tiles: &HashMap<TileKey, TileSlot>) -> Result<(), CoreError> {
        let layer = doc.layer_mut(self.layer)?;
        for (key, tile) in tiles {
            layer.pixels.set_tile(*key, tile.clone());
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
}
