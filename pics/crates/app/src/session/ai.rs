//! IA locale : méthodes classiques (sans modèle) branchées sur la sélection et les masques.

use openpics_ai::classic;
use openpics_core::commands::ReplaceTiles;
use openpics_core::{LayerId, TileGrid};
use openpics_ops::selection::through_selection;

use super::{Session, SessionError};

impl Session {
    fn ai_source(&self) -> Result<(LayerId, TileGrid), SessionError> {
        let e = self.entry()?;
        let id = e.active_layer.ok_or_else(|| SessionError::InvalidArgument("aucun calque sélectionné".into()))?;
        let l = e.doc.layer(id)?;
        if !l.is_raster() {
            return Err(openpics_core::CoreError::NotRaster(l.name.clone()).into());
        }
        Ok((id, l.pixels.clone()))
    }

    /// Sélectionne le sujet du calque actif en estimant le fond depuis les bords.
    pub fn ai_select_subject(&mut self, tolerance: f32) -> Result<(), SessionError> {
        let (_, pixels) = self.ai_source()?;
        let mask = classic::subject_mask(&pixels, tolerance)?;
        self.commit_selection("Sélectionner le sujet", mask)
    }

    /// Masque le fond du calque actif : un masque est ajouté (le fond reste récupérable en le retirant).
    pub fn ai_remove_background(&mut self, tolerance: f32) -> Result<(), SessionError> {
        let (id, pixels) = self.ai_source()?;
        if self.entry()?.doc.layer(id)?.mask.is_some() {
            return Err(SessionError::InvalidArgument("ce calque a déjà un masque : retirez-le d'abord".into()));
        }
        let mask = classic::subject_mask(&pixels, tolerance)?;
        let doc = &mut self.entry_mut()?.doc;
        doc.execute(Box::new(openpics_core::commands::AddMask::new(id, false)))?;
        let empty = doc.layer(id)?.mask.as_ref().map(|m| m.pixels.clone());
        if let Some(old) = empty {
            if let Some(cmd) = ReplaceTiles::between_mask("Supprimer l'arrière-plan", id, &old, &mask) {
                doc.execute(Box::new(cmd))?;
            }
        }
        Ok(())
    }

    /// Comble la sélection par diffusion à partir de ses abords.
    pub fn ai_inpaint(&mut self) -> Result<(), SessionError> {
        let (id, pixels) = self.ai_source()?;
        let selection = self.selection_required()?;
        let filled = classic::inpaint(&pixels, &selection)?;
        // Seule la zone sélectionnée change (bords adoucis compris).
        let result = through_selection(&pixels, &filled, &selection)?;
        if let Some(cmd) = ReplaceTiles::between("Combler la sélection", id, &pixels, &result) {
            self.entry_mut()?.doc.execute(Box::new(cmd))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpics_core::ViewSettings;
    use openpics_core::shortcuts::Keymap;

    fn session_with_square() -> Session {
        let mut s = Session::new(ViewSettings::default(), Keymap::default());
        s.set_canvas(800, 600);
        s.new_document(None, 40, 40).unwrap(); // fond blanc
        let id = s.snapshot().layers[0].id;
        let mut img = vec![255u8; 40 * 40 * 4];
        for y in 10..30 {
            for x in 10..30 {
                let i = (y * 40 + x) * 4;
                img[i..i + 4].copy_from_slice(&[200, 30, 30, 255]);
            }
        }
        let grid = TileGrid::from_rgba8(40, 40, &img).unwrap();
        let e = s.entry_mut().unwrap();
        let old = e.doc.layer(LayerId(id)).unwrap().pixels.clone();
        e.doc.execute(Box::new(ReplaceTiles::between("Test", LayerId(id), &old, &grid).unwrap())).unwrap();
        s
    }

    #[test]
    fn subject_selection_and_background_removal_are_undoable() {
        let mut s = session_with_square();
        s.ai_select_subject(0.15).unwrap();
        let sel = s.entry().unwrap().doc.selection.clone().unwrap();
        assert_eq!((sel.get_pixel(20, 20)[0], sel.get_pixel(2, 2)[0]), (255, 0));
        s.deselect().unwrap();
        s.ai_remove_background(0.15).unwrap();
        let id = LayerId(s.snapshot().layers[0].id);
        let mask = |s: &Session, x, y| {
            s.entry().unwrap().doc.layer(id).unwrap().mask.as_ref().unwrap().pixels.get_pixel(x, y)[0]
        };
        assert_eq!((mask(&s, 20, 20), mask(&s, 2, 2)), (255, 0));
        assert!(s.ai_remove_background(0.15).is_err(), "déjà un masque");
        s.undo().unwrap();
        s.undo().unwrap();
        assert!(s.entry().unwrap().doc.layer(id).unwrap().mask.is_none());
    }

    #[test]
    fn inpaint_needs_a_selection_and_stays_inside_it() {
        let mut s = session_with_square();
        assert!(s.ai_inpaint().is_err(), "pas de sélection");
        let mut m = vec![0u8; 40 * 40 * 4];
        for y in 18..22 {
            for x in 18..22 {
                let i = (y * 40 + x) * 4;
                m[i..i + 4].copy_from_slice(&[255; 4]);
            }
        }
        s.commit_selection("Test", TileGrid::from_rgba8(40, 40, &m).unwrap()).unwrap();
        // on crève un trou noir dans le carré rouge
        let id = LayerId(s.snapshot().layers[0].id);
        let old = s.entry().unwrap().doc.layer(id).unwrap().pixels.clone();
        let mut holed = old.clone();
        for y in 18..22 {
            for x in 18..22 {
                holed.put_pixel(x, y, [0, 0, 0, 255]);
            }
        }
        s.entry_mut().unwrap().doc.execute(Box::new(ReplaceTiles::between("Trou", id, &old, &holed).unwrap())).unwrap();
        s.ai_inpaint().unwrap();
        let px = s.entry().unwrap().doc.layer(id).unwrap().pixels.get_pixel(20, 20);
        assert!(px[0] > 190 && px[1] < 40, "le trou est comblé avec le rouge environnant : {px:?}");
        assert_eq!(s.entry().unwrap().doc.layer(id).unwrap().pixels.get_pixel(5, 5), [255; 4]);
    }
}
