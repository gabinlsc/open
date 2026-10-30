//! Lecture et écriture de documents.
//!
//! Phase 1 : PNG, JPEG, WebP (écriture sans perte), TIFF, BMP, GIF (première image) et le format
//! natif `.ocv`. Les images sont décodées en RGBA 8 bits sRGB ; la gestion des profils ICC et du
//! 16 bits arrive avec la phase couleur.

mod error;
mod format;
mod ocv;
mod raster;

pub use error::IoError;
pub use format::Format;
pub use raster::{ExportOptions, PngCompression};

use std::path::Path;

use openpics_core::{DocId, Document};

/// Ouvre un fichier image ou `.ocv` dans un nouveau document.
pub fn open(path: &Path, id: DocId) -> Result<Document, IoError> {
    let format = Format::from_path(path)?;
    let mut doc = match format {
        Format::Ocv => ocv::read(path, id)?,
        _ => raster::read(path, id)?,
    };
    if format == Format::Ocv {
        doc.path = Some(path.to_path_buf());
        // Le nom affiché est celui du fichier : le fichier peut avoir été renommé depuis l'enregistrement.
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            doc.name = stem.to_owned();
        }
    }
    doc.mark_saved();
    Ok(doc)
}

/// Enregistre au format natif `.ocv` (seul format qui conserve calques et repères).
pub fn save(doc: &mut Document, path: &Path) -> Result<(), IoError> {
    ocv::write(doc, path)?;
    doc.path = Some(path.to_path_buf());
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        doc.name = stem.to_owned();
    }
    doc.mark_saved();
    Ok(())
}

/// Exporte une image aplatie dans le format déduit de l'extension. N'altère pas le document.
pub fn export(doc: &Document, path: &Path, options: &ExportOptions) -> Result<(), IoError> {
    match Format::from_path(path)? {
        Format::Ocv => Err(IoError::UnsupportedFormat("utilisez « Enregistrer » pour le format .ocv".into())),
        format => raster::write(doc, path, format, options),
    }
}

/// Écrit dans un fichier voisin puis renomme : un crash ou un disque plein ne détruit pas
/// l'ancien fichier.
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), IoError> {
    use std::io::Write;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".openpics-tmp");
    let tmp = std::path::PathBuf::from(tmp);
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(result?)
}
