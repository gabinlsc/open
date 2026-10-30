//! Traitement par lot : applique un filtre à tous les fichiers d'un dossier et exporte les résultats ailleurs.
//! Les originaux ne sont jamais écrasés.

use std::path::{Path, PathBuf};

use openpics_core::DocId;
use openpics_io::{ExportOptions, Format};
use openpics_ops::filters;
use serde::Serialize;
use ts_rs::TS;

use crate::error::SessionError;

#[derive(Debug, Clone, Serialize, TS, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BatchReport {
    pub done: u32,
    pub failed: Vec<String>,
}

/// Les fichiers image d'un dossier (non récursif), triés par nom.
pub fn list_images(dir: &Path) -> Result<Vec<PathBuf>, SessionError> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(openpics_io::IoError::from)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && Format::from_path(p).is_ok())
        .collect();
    files.sort();
    Ok(files)
}

pub fn run_filter(
    inputs: &[PathBuf],
    out_dir: &Path,
    filter_id: &str,
    values: &[f32],
    extension: &str,
    jpeg_quality: u8,
) -> Result<BatchReport, SessionError> {
    let def = filters::find(filter_id)
        .ok_or_else(|| SessionError::InvalidArgument(format!("filtre inconnu : {filter_id}")))?;
    let ext = extension.trim_start_matches('.').to_ascii_lowercase();
    let target_format = Format::from_path(Path::new(&format!("x.{ext}")))?;
    if target_format == Format::Ocv {
        return Err(SessionError::InvalidArgument("le lot exporte des images : choisissez PNG, JPEG, WebP…".into()));
    }
    std::fs::create_dir_all(out_dir).map_err(openpics_io::IoError::from)?;
    let options = ExportOptions { jpeg_quality, ..ExportOptions::default() };
    let mut report = BatchReport::default();
    for input in inputs {
        let name = input.file_name().map_or_else(|| input.display().to_string(), |n| n.to_string_lossy().into_owned());
        let result = (|| -> Result<(), SessionError> {
            let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
            let output = out_dir.join(format!("{stem}.{ext}"));
            if output == *input {
                return Err(SessionError::InvalidArgument("le fichier de sortie écraserait l'original".into()));
            }
            let mut doc = openpics_io::open(input, DocId(1))?;
            for layer in doc.layers.iter_mut().filter(|l| l.is_raster()) {
                layer.pixels = def.apply(&layer.pixels, values)?;
            }
            openpics_io::export(&doc, &output, &options)?;
            Ok(())
        })();
        match result {
            Ok(()) => report.done += 1,
            Err(e) => report.failed.push(format!("{name} : {e}")),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use openpics_core::ViewSettings;
    use openpics_core::shortcuts::Keymap;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("openpics-batch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn filter_is_applied_to_every_image_and_errors_are_reported_per_file() {
        let dir = scratch("run");
        let (input, output) = (dir.join("in"), dir.join("out"));
        std::fs::create_dir_all(&input).unwrap();
        let mut s = Session::new(ViewSettings::default(), Keymap::default());
        s.new_document(None, 8, 8).unwrap(); // blanc
        s.export_active(&input.join("a.png"), None).unwrap();
        s.export_active(&input.join("b.png"), None).unwrap();
        std::fs::write(input.join("casse.png"), b"pas une image").unwrap();
        std::fs::write(input.join("notes.txt"), "ignoré".as_bytes()).unwrap();
        let files = list_images(&input).unwrap();
        assert_eq!(files.len(), 3, "le .txt est ignoré : {files:?}");
        let report = run_filter(&files, &output, "solarize", &[0.5], "png", 90).unwrap();
        assert_eq!(report.done, 2);
        assert_eq!(report.failed.len(), 1);
        assert!(report.failed[0].starts_with("casse.png"), "{:?}", report.failed);
        let out = openpics_io::open(&output.join("a.png"), DocId(1)).unwrap();
        assert_eq!(out.layers[0].pixels.get_pixel(3, 3), [0, 0, 0, 255], "blanc solarisé");
        let original = openpics_io::open(&input.join("a.png"), DocId(1)).unwrap();
        assert_eq!(original.layers[0].pixels.get_pixel(3, 3), [255; 4], "l'original est intact");
    }

    #[test]
    fn refuses_to_overwrite_originals_unknown_filters_and_native_output() {
        let dir = scratch("guards");
        let mut s = Session::new(ViewSettings::default(), Keymap::default());
        s.new_document(None, 4, 4).unwrap();
        s.export_active(&dir.join("a.png"), None).unwrap();
        let files = vec![dir.join("a.png")];
        let same = run_filter(&files, &dir, "solarize", &[0.5], "png", 90).unwrap();
        assert_eq!((same.done, same.failed.len()), (0, 1), "même dossier et même extension : refusé");
        assert!(run_filter(&files, &dir, "nope", &[], "png", 90).is_err());
        assert!(run_filter(&files, &dir, "solarize", &[0.5], "ocv", 90).is_err());
    }
}
