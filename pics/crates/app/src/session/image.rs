//! Taille de l'image, export à plusieurs échelles, comparaison avec l'original.

use std::path::{Path, PathBuf};

use openpics_core::commands::{DocState, ReplaceDocState};
use openpics_core::tile::MAX_DIMENSION;
use openpics_core::{Document, Guide, Layer, LayerKind, Size, TileGrid};
use openpics_io::ExportOptions;
use openpics_ops::resize::{resample, resample_mask};

use super::{Session, SessionError};

fn scaled_layer(l: &Layer, w: u32, h: u32) -> Result<Layer, SessionError> {
    let mut out = l.clone();
    out.pixels = resample(&l.pixels, w, h)?;
    if let Some(m) = &mut out.mask {
        m.pixels = resample_mask(&l.mask.as_ref().map(|m| m.pixels.clone()).unwrap_or(m.pixels.clone()), w, h)?;
    }
    // Un texte ou une forme dont le rendu a été rééchantillonné n'est plus synchronisé avec son modèle : il devient
    // un calque de pixels plutôt que de se « réparer » plus tard à l'ancienne taille.
    if matches!(out.kind, LayerKind::Text(_) | LayerKind::Shape(_)) {
        out.kind = LayerKind::Raster;
    }
    Ok(out)
}

/// Document aplati, rééchantillonné à `scale`, prêt à être exporté.
fn scaled_flat(doc: &Document, scale: f64) -> Result<Document, SessionError> {
    let size = doc.size();
    let (w, h) = (
        (f64::from(size.width) * scale).round().clamp(1.0, f64::from(MAX_DIMENSION)) as u32,
        (f64::from(size.height) * scale).round().clamp(1.0, f64::from(MAX_DIMENSION)) as u32,
    );
    let flat = TileGrid::from_rgba8(size.width, size.height, &doc.flatten_rgba8())?;
    let mut out = Document::new(doc.id, doc.name.clone(), Size::new(w, h))?;
    out.layers[0].pixels = if (w, h) == (size.width, size.height) { flat } else { resample(&flat, w, h)? };
    Ok(out)
}

/// `nom@2x.png` pour les échelles ≥ 1 entières, `nom@0.5x.png` sinon.
pub fn scaled_path(base: &Path, scale: f64) -> PathBuf {
    let stem = base.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
    let tag =
        if (scale - scale.round()).abs() < 1e-9 { format!("{}", scale.round() as u64) } else { format!("{scale}") };
    let name = match base.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{stem}@{tag}x.{ext}"),
        None => format!("{stem}@{tag}x"),
    };
    base.with_file_name(name)
}

impl Session {
    /// Redimensionne tout le document (calques, masques, repères, sélection), en une seule étape d'historique.
    pub fn image_resize(&mut self, width: u32, height: u32) -> Result<(), SessionError> {
        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(SessionError::InvalidArgument(format!(
                "dimensions invalides : {width}×{height} (1 à {MAX_DIMENSION} pixels par côté)"
            )));
        }
        self.filter_cancel();
        let doc = &self.entry()?.doc;
        let old = doc.size();
        if (old.width, old.height) == (width, height) {
            return Ok(());
        }
        let layers = doc.layers.iter().map(|l| scaled_layer(l, width, height)).collect::<Result<Vec<_>, _>>()?;
        let (fx, fy) = (f64::from(width) / f64::from(old.width), f64::from(height) / f64::from(old.height));
        let guides = doc
            .guides
            .iter()
            .map(|g| Guide {
                position: g.position * if g.axis == openpics_core::Axis::Vertical { fx } else { fy },
                ..*g
            })
            .collect();
        let selection = doc.selection.as_ref().map(|s| resample_mask(s, width, height)).transpose()?;
        let state = DocState { size: Size::new(width, height), layers, guides, selection };
        self.entry_mut()?.doc.execute(Box::new(ReplaceDocState::new("Taille de l'image", state)))?;
        self.entry_mut()?.fitted = false;
        self.ensure_fitted();
        self.record(super::script::Step::Resize(width, height));
        Ok(())
    }

    /// Exporte l'image aplatie à plusieurs échelles (`nom@2x.png`…). Renvoie les fichiers écrits.
    pub fn export_scales(
        &self,
        base: &Path,
        scales: &[f64],
        jpeg_quality: Option<u8>,
    ) -> Result<Vec<PathBuf>, SessionError> {
        if scales.is_empty() || scales.len() > 16 || scales.iter().any(|s| !(0.01..=16.0).contains(s)) {
            return Err(SessionError::InvalidArgument("échelles : 1 à 16 valeurs entre 0,01 et 16".into()));
        }
        let doc = &self.entry()?.doc;
        let options = ExportOptions { jpeg_quality: jpeg_quality.unwrap_or(90), ..ExportOptions::default() };
        let mut written = Vec::new();
        for &scale in scales {
            let target = scaled_path(base, scale);
            openpics_io::export(&scaled_flat(doc, scale)?, &target, &options)?;
            written.push(target);
        }
        Ok(written)
    }

    /// Bascule la comparaison avec l'original. Elle est levée si le document change d'onglet.
    pub fn compare_set(&mut self, on: bool) -> Result<(), SessionError> {
        self.filter_cancel();
        let doc = &mut self.entry_mut()?.doc;
        if on {
            doc.begin_compare()?;
        } else {
            doc.end_compare()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpics_core::ViewSettings;
    use openpics_core::shortcuts::Keymap;

    fn session() -> Session {
        let mut s = Session::new(ViewSettings::default(), Keymap::default());
        s.set_canvas(800, 600);
        s.new_document(None, 40, 20).unwrap();
        s
    }

    #[test]
    fn resize_scales_everything_and_is_one_undoable_step() {
        let mut s = session();
        s.guide_commit(openpics_core::Axis::Vertical, 10.0).unwrap();
        s.select_all().unwrap();
        let n = s.snapshot().history.names.len();
        s.image_resize(80, 40).unwrap();
        let snap = s.snapshot();
        assert_eq!((snap.docs[0].width, snap.docs[0].height), (80, 40));
        assert_eq!(snap.history.names.len(), n + 1);
        assert_eq!(snap.guides[0].position, 20.0, "repère vertical mis à l'échelle en x");
        let doc = &s.entry().unwrap().doc;
        assert_eq!(doc.layers[0].pixels.width(), 80);
        assert_eq!(doc.selection.as_ref().unwrap().width(), 80);
        assert_eq!(doc.layers[0].pixels.get_pixel(60, 30), [255; 4], "le fond blanc reste blanc");
        s.undo().unwrap();
        assert_eq!(s.snapshot().docs[0].width, 40);
        assert_eq!(s.entry().unwrap().doc.layers[0].pixels.width(), 40);
        assert!(s.image_resize(0, 5).is_err());
        assert!(s.image_resize(70_000, 5).is_err());
    }

    #[test]
    fn multi_resolution_export_writes_named_files_at_the_right_sizes() {
        let dir = std::env::temp_dir().join(format!("openpics-scales-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let s = session();
        let files = s.export_scales(&dir.join("logo.png"), &[0.5, 1.0, 2.0], None).unwrap();
        let names: Vec<_> = files.iter().map(|f| f.file_name().unwrap().to_str().unwrap().to_owned()).collect();
        assert_eq!(names, ["logo@0.5x.png", "logo@1x.png", "logo@2x.png"]);
        let sizes: Vec<_> = files
            .iter()
            .map(|f| {
                let d = openpics_io::open(f, openpics_core::DocId(1)).unwrap();
                (d.size().width, d.size().height)
            })
            .collect();
        assert_eq!(sizes, [(20, 10), (40, 20), (80, 40)]);
        assert!(s.export_scales(&dir.join("x.png"), &[], None).is_err());
        assert!(s.export_scales(&dir.join("x.png"), &[100.0], None).is_err());
    }

    #[test]
    fn compare_shows_the_original_blocks_edits_and_comes_back() {
        let mut s = session();
        s.select_all().unwrap();
        assert!(s.snapshot().has_selection);
        s.compare_set(true).unwrap();
        let snap = s.snapshot();
        assert!(snap.comparing && !snap.has_selection, "l'original n'a pas la sélection");
        assert!(s.select_all().is_err(), "modification refusée pendant la comparaison");
        assert!(s.undo().is_err());
        s.compare_set(false).unwrap();
        assert!(s.snapshot().has_selection && !s.snapshot().comparing);
    }
}
