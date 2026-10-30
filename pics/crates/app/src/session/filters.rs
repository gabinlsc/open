//! Filtres : aperçu en direct (le calque est modifié sans historique), validation (un seul état d'historique)
//! ou annulation (le calque d'origine est restitué).

use openpics_core::commands::ReplaceTiles;
use openpics_core::{LayerId, TileGrid};
use openpics_ops::filters;
use openpics_ops::selection::through_selection;

use super::{Session, SessionError};

pub(super) struct FilterPreview {
    layer: LayerId,
    base: TileGrid,
    filter: &'static filters::FilterDef,
}

impl Session {
    /// Applique (ou réapplique avec d'autres valeurs) un filtre en aperçu sur le calque actif.
    pub fn filter_preview(&mut self, id: &str, values: &[f32]) -> Result<(), SessionError> {
        let def = filters::find(id).ok_or_else(|| SessionError::InvalidArgument(format!("filtre inconnu : {id}")))?;
        // Un changement de filtre repart de l'état d'origine, jamais d'un aperçu précédent.
        if self.filter_preview.as_ref().is_some_and(|p| !std::ptr::eq(p.filter, def)) {
            self.filter_cancel();
        }
        if self.filter_preview.is_none() {
            let e = self.entry()?;
            let layer =
                e.active_layer.ok_or_else(|| SessionError::InvalidArgument("aucun calque sélectionné".into()))?;
            let l = e.doc.layer(layer)?;
            if !l.is_raster() {
                return Err(openpics_core::CoreError::NotRaster(l.name.clone()).into());
            }
            if l.locks.blocks_painting() {
                return Err(openpics_core::CoreError::LayerLocked(l.name.clone()).into());
            }
            self.filter_preview = Some(FilterPreview { layer, base: l.pixels.clone(), filter: def });
        }
        let Some(preview) = &self.filter_preview else { return Ok(()) };
        let (layer, base) = (preview.layer, preview.base.clone());
        let filtered = def.apply(&base, values)?;
        let result = match &self.entry()?.doc.selection {
            Some(sel) => through_selection(&base, &filtered, sel)?,
            None => filtered,
        };
        let e = self.entry_mut()?;
        e.doc.layer_mut(layer)?.pixels = result;
        e.doc.touch();
        Ok(())
    }

    pub fn filter_commit(&mut self) -> Result<(), SessionError> {
        let Some(p) = self.filter_preview.take() else { return Ok(()) };
        let after = self.entry()?.doc.layer(p.layer)?.pixels.clone();
        match ReplaceTiles::between(p.filter.label, p.layer, &p.base, &after) {
            Some(cmd) => {
                self.entry_mut()?.doc.execute(Box::new(cmd))?;
            }
            None => {
                let e = self.entry_mut()?;
                e.doc.layer_mut(p.layer)?.pixels = p.base;
                e.doc.touch();
            }
        }
        Ok(())
    }

    pub fn filter_cancel(&mut self) {
        if let Some(p) = self.filter_preview.take() {
            if let Ok(e) = self.entry_mut() {
                if let Ok(l) = e.doc.layer_mut(p.layer) {
                    l.pixels = p.base;
                }
                e.doc.touch();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::Tool;
    use openpics_core::ViewSettings;
    use openpics_core::shortcuts::Keymap;

    fn session() -> Session {
        let mut s = Session::new(ViewSettings::default(), Keymap::default());
        s.set_canvas(800, 600);
        s.new_document(None, 40, 40).unwrap();
        s
    }

    fn px(s: &Session, x: u32, y: u32) -> [u8; 4] {
        s.render_state().document.unwrap().layers[0].pixels.get_pixel(x, y)
    }

    #[test]
    fn preview_is_live_cancel_restores_and_commit_is_one_history_entry() {
        let mut s = session();
        let n = s.snapshot().history.names.len();
        s.filter_preview("threshold", &[0.5]).unwrap();
        assert_eq!(px(&s, 5, 5), [255; 4]);
        s.filter_preview("posterize", &[3.0]).unwrap(); // changer de filtre repart de l'original
        assert_eq!(s.snapshot().history.names.len(), n, "l'aperçu n'est pas dans l'historique");
        s.filter_cancel();
        assert_eq!(px(&s, 5, 5), [255; 4]);
        s.filter_preview("add_noise", &[0.0, 1.0, 0.0, 1.0]).unwrap();
        assert_eq!(px(&s, 5, 5), [255; 4], "bruit nul : rien ne change");
        s.filter_preview("add_noise", &[1.0, 1.0, 0.0, 1.0]).unwrap(); // autres valeurs, même filtre
        let changed = (0..40).filter(|&x| px(&s, x, 5) != [255; 4]).count();
        assert!(changed > 5, "le bruit maximal modifie l'image : {changed}");
        s.filter_preview("solarize", &[0.5]).unwrap(); // autre filtre : repart de l'original
        assert_eq!(px(&s, 5, 5), [0, 0, 0, 255], "blanc solarisé, sans trace du bruit");
        s.filter_commit().unwrap();
        let names = s.snapshot().history.names;
        assert_eq!((names.len(), names.last().unwrap().as_str()), (n + 1, "Solarisation"));
        s.undo().unwrap();
        assert_eq!(px(&s, 5, 5), [255; 4]);
    }

    #[test]
    fn filters_respect_selection_locks_and_unknown_ids() {
        let mut s = session();
        s.set_tool(Tool::RectSelect);
        let at = |x: f64, y: f64| crate::session::Pointer {
            x: 400.0 + x - 20.0 + 0.0,
            y: 300.0 + y - 20.0,
            pressure: 1.0,
            ..Default::default()
        };
        s.pointer_down(&at(0.0, 0.0)).unwrap();
        s.pointer_move(&at(20.0, 40.0)).unwrap();
        s.pointer_up(&at(20.0, 40.0)).unwrap();
        s.filter_preview("solarize", &[0.5]).unwrap();
        s.filter_commit().unwrap();
        assert_eq!((px(&s, 5, 20), px(&s, 35, 20)), ([0, 0, 0, 255], [255; 4]), "seule la sélection est filtrée");
        assert!(s.filter_preview("nope", &[]).is_err());
        let id = s.snapshot().layers[0].id;
        let p = crate::dto::LayerPropsDto {
            name: "Arrière-plan".into(),
            visible: true,
            opacity: 1.0,
            fill: 1.0,
            blend: "normal".into(),
            locks: crate::dto::LocksDto { pixels: true, position: false, all: false },
            clip_to_below: false,
            isolated: true,
        };
        s.layer_set_props(id, &p).unwrap();
        assert!(s.filter_preview("threshold", &[0.5]).unwrap_err().to_string().contains("verrouillé"));
        let adj = s.adjustment_add("invert").unwrap();
        s.layer_select(adj).unwrap();
        assert!(s.filter_preview("threshold", &[0.5]).unwrap_err().to_string().contains("pixels"));
    }

    #[test]
    fn a_filter_that_changes_nothing_leaves_no_history_entry() {
        let mut s = session();
        let n = s.snapshot().history.names.len();
        s.filter_preview("gaussian_blur", &[5.0]).unwrap(); // sur un aplat uni
        s.filter_commit().unwrap();
        assert_eq!(s.snapshot().history.names.len(), n);
    }
}
