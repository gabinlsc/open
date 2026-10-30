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

    pub fn set_models_dir(&mut self, dir: std::path::PathBuf) {
        self.models_dir = Some(dir);
    }

    /// Masque du sujet : par un modèle ONNX installé si `model` est donné, sinon par la méthode classique.
    fn subject_mask(&self, pixels: &TileGrid, tolerance: f32, model: Option<&str>) -> Result<TileGrid, SessionError> {
        let Some(id) = model else { return Ok(classic::subject_mask(pixels, tolerance)?) };
        let dir = self
            .models_dir
            .clone()
            .ok_or_else(|| SessionError::InvalidArgument("dossier des modèles inconnu".into()))?;
        let store = openpics_ai::ModelStore::new(&dir);
        let entry = store
            .catalog()?
            .into_iter()
            .find(|m| m.id == id)
            .ok_or_else(|| SessionError::InvalidArgument(format!("modèle inconnu : {id}")))?;
        if !matches!(entry.task, openpics_ai::Task::BackgroundRemoval | openpics_ai::Task::SubjectSelection) {
            return Err(SessionError::InvalidArgument(format!("« {} » ne sait pas isoler un sujet", entry.name)));
        }
        if !store.is_installed(id) {
            return Err(SessionError::InvalidArgument(format!("« {} » n'est pas installé", entry.name)));
        }
        run_model(&dir, id, pixels)
    }

    /// Sélectionne le sujet du calque actif en estimant le fond depuis les bords.
    pub fn ai_select_subject(&mut self, tolerance: f32) -> Result<(), SessionError> {
        self.ai_select_subject_with(tolerance, None)
    }

    pub fn ai_select_subject_with(&mut self, tolerance: f32, model: Option<&str>) -> Result<(), SessionError> {
        let (_, pixels) = self.ai_source()?;
        let mask = self.subject_mask(&pixels, tolerance, model)?;
        self.commit_selection("Sélectionner le sujet", mask)?;
        if model.is_none() {
            self.record(super::script::Step::Subject(tolerance));
        }
        Ok(())
    }

    /// Masque le fond du calque actif : un masque est ajouté (le fond reste récupérable en le retirant).
    pub fn ai_remove_background(&mut self, tolerance: f32) -> Result<(), SessionError> {
        self.ai_remove_background_with(tolerance, None)
    }

    pub fn ai_remove_background_with(&mut self, tolerance: f32, model: Option<&str>) -> Result<(), SessionError> {
        let (id, pixels) = self.ai_source()?;
        if self.entry()?.doc.layer(id)?.mask.is_some() {
            return Err(SessionError::InvalidArgument("ce calque a déjà un masque : retirez-le d'abord".into()));
        }
        let mask = self.subject_mask(&pixels, tolerance, model)?;
        let doc = &mut self.entry_mut()?.doc;
        doc.execute(Box::new(openpics_core::commands::AddMask::new(id, false)))?;
        let empty = doc.layer(id)?.mask.as_ref().map(|m| m.pixels.clone());
        if let Some(old) = empty {
            if let Some(cmd) = ReplaceTiles::between_mask("Supprimer l'arrière-plan", id, &old, &mask) {
                doc.execute(Box::new(cmd))?;
            }
        }
        if model.is_none() {
            self.record(super::script::Step::RemoveBackground(tolerance));
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
        self.record(super::script::Step::Inpaint);
        Ok(())
    }
}

#[cfg(feature = "onnx")]
fn run_model(dir: &std::path::Path, id: &str, pixels: &TileGrid) -> Result<TileGrid, SessionError> {
    // La bibliothèque ONNX Runtime n'est pas livrée : à côté de l'exécutable ou dans le dossier des modèles.
    let name = if cfg!(windows) { "onnxruntime.dll" } else { "libonnxruntime.so" };
    let beside_exe = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name)));
    let lib = [beside_exe, Some(dir.join(name))].into_iter().flatten().find(|p| p.is_file());
    if lib.is_some() || std::env::var_os("ORT_DYLIB_PATH").is_some() {
        openpics_ai::onnx::init(lib.as_deref())?;
        Ok(openpics_ai::onnx::run_mask(&dir.join(format!("{id}.onnx")), pixels)?)
    } else {
        Err(SessionError::InvalidArgument(format!(
            "{name} est introuvable : placez la bibliothèque ONNX Runtime à côté de l'exécutable ou dans le dossier des modèles"
        )))
    }
}

#[cfg(not(feature = "onnx"))]
fn run_model(_: &std::path::Path, _: &str, _: &TileGrid) -> Result<TileGrid, SessionError> {
    Err(SessionError::InvalidArgument("cette version a été compilée sans ONNX Runtime".into()))
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

    fn catalog_with(dir: &std::path::Path, task: &str, sha: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("models.json"),
            format!(
                r#"[{{"id":"toy","task":"{task}","name":"Jouet","licence":"MIT","source":"test","sha256":"{sha}","size_bytes":1}}]"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn a_model_that_is_unknown_wrong_or_not_installed_is_refused_clearly() {
        let dir = std::env::temp_dir().join(format!("openpics-aimodels-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = session_with_square();
        assert!(s.ai_select_subject_with(0.1, Some("toy")).unwrap_err().to_string().contains("dossier"));
        s.set_models_dir(dir.clone());
        catalog_with(&dir, "background_removal", &"0".repeat(64));
        assert!(s.ai_select_subject_with(0.1, Some("absent")).unwrap_err().to_string().contains("inconnu"));
        assert!(s.ai_select_subject_with(0.1, Some("toy")).unwrap_err().to_string().contains("pas installé"));
        catalog_with(&dir, "colorize", &"0".repeat(64));
        assert!(s.ai_remove_background_with(0.1, Some("toy")).unwrap_err().to_string().contains("isoler"));
        assert!(s.snapshot().history.names.len() <= 3, "aucune modification en cas de refus");
    }

    /// De bout en bout avec ONNX Runtime réel et un modèle jouet (voir `openpics-ai`, `onnx.rs`).
    #[cfg(feature = "onnx")]
    #[test]
    fn an_installed_onnx_model_drives_subject_selection_and_background_removal() {
        let Some(model) = std::env::var_os("OPENPICS_TEST_ONNX").map(std::path::PathBuf::from) else {
            assert!(std::env::var_os("OPENPICS_REQUIRE_ORT").is_none(), "OPENPICS_REQUIRE_ORT=1 mais pas de modèle");
            return;
        };
        let dir = std::env::temp_dir().join(format!("openpics-aionnx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let sha = openpics_ai::catalog::sha256_file(&model).unwrap();
        catalog_with(&dir, "background_removal", &sha);
        openpics_ai::ModelStore::new(&dir).import("toy", &model).unwrap();
        // Image : carré sombre sur fond blanc, comme dans le test d'`openpics-ai`.
        let mut s = session_with_square();
        s.set_models_dir(dir);
        s.ai_select_subject_with(0.1, Some("toy")).unwrap();
        let sel = s.entry().unwrap().doc.selection.clone().unwrap();
        assert!(
            sel.get_pixel(20, 20)[0] > 128 && sel.get_pixel(2, 2)[0] < 128,
            "{:?} {:?}",
            sel.get_pixel(20, 20),
            sel.get_pixel(2, 2)
        );
        s.deselect().unwrap();
        s.ai_remove_background_with(0.1, Some("toy")).unwrap();
        let id = LayerId(s.snapshot().layers[0].id);
        assert!(s.entry().unwrap().doc.layer(id).unwrap().mask.is_some());
    }
}
