use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::geom::Size;
use crate::history::{Command, History};
use crate::tile::TileGrid;

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

/// Phase 1 : un calque raster unique. Les autres types (groupe, réglage, texte…) arrivent en
/// phase 2 en transformant ce struct en enum, sans changer `TileGrid` ni l'historique.
#[derive(Debug, Clone)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub pixels: TileGrid,
}

#[derive(Debug)]
pub struct Document {
    pub id: DocId,
    pub name: String,
    pub path: Option<PathBuf>,
    pub layers: Vec<Layer>,
    pub guides: Vec<Guide>,
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
            layers: Vec::new(),
            guides: Vec::new(),
            history: History::default(),
            revision: 0,
            next_id: 1,
        };
        doc.add_layer("Arrière-plan", background);
        Ok(doc)
    }

    pub fn size(&self) -> Size {
        self.layers.first().map_or(Size::default(), |l| Size::new(l.pixels.width(), l.pixels.height()))
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_layer(&mut self, name: impl Into<String>, pixels: TileGrid) -> LayerId {
        let id = LayerId(self.alloc_id());
        self.layers.push(Layer { id, name: name.into(), visible: true, pixels });
        id
    }

    /// Réserve les identifiants déjà utilisés par un document relu depuis un fichier.
    pub fn reserve_ids_up_to(&mut self, max_used: u64) {
        self.next_id = self.next_id.max(max_used + 1);
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
            layers: self.layers.clone(),
            guides: self.guides.clone(),
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

    /// Aplatit les calques visibles (de bas en haut, « normal ») en RGBA 8 bits non prémultiplié.
    pub fn flatten_rgba8(&self) -> Vec<u8> {
        let size = self.size();
        let mut out = vec![0u8; size.width as usize * size.height as usize * 4];
        for layer in self.layers.iter().filter(|l| l.visible) {
            let src = layer.pixels.to_rgba8();
            for (d, s) in out.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                over(d, s);
            }
        }
        out
    }
}

/// Opérateur « over » sur alpha non prémultiplié.
fn over(dst: &mut [u8], src: &[u8]) {
    let sa = f32::from(src[3]) / 255.0;
    if sa <= 0.0 {
        return;
    }
    let da = f32::from(dst[3]) / 255.0;
    let oa = sa + da * (1.0 - sa);
    for c in 0..3 {
        let v = (f32::from(src[c]) * sa + f32::from(dst[c]) * da * (1.0 - sa)) / oa;
        dst[c] = v.round().clamp(0.0, 255.0) as u8;
    }
    dst[3] = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
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
