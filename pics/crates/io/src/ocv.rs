//! Format natif `.ocv` : une archive zip contenant `document.json` et un PNG par calque.
//!
//! Choix : zip + PNG pour qu'un fichier reste récupérable avec des outils standards, même sans
//! OpenPics. Le champ `schema` permet de refuser proprement un fichier plus récent que l'app.

use std::io::{Cursor, Read, Write};
use std::path::Path;

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{DynamicImage, ImageReader, RgbaImage};
use openpics_core::tile::MAX_DIMENSION;
use openpics_core::{DocId, Document, Guide, Layer, LayerId, Size, TileGrid};
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::IoError;

const SCHEMA: u32 = 1;
const MANIFEST: &str = "document.json";
/// Un manifeste honnête pèse quelques Ko ; ce plafond protège d'une archive piégée.
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_LAYER_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    schema: u32,
    name: String,
    width: u32,
    height: u32,
    layers: Vec<LayerEntry>,
    #[serde(default)]
    guides: Vec<Guide>,
}

#[derive(Serialize, Deserialize)]
struct LayerEntry {
    id: u64,
    name: String,
    visible: bool,
}

fn layer_path(id: u64) -> String {
    format!("layers/{id}.png")
}

pub(crate) fn write(doc: &Document, path: &Path) -> Result<(), IoError> {
    let size = doc.size();
    let manifest = Manifest {
        format: "openpics".into(),
        schema: SCHEMA,
        name: doc.name.clone(),
        width: size.width,
        height: size.height,
        layers: doc
            .layers
            .iter()
            .map(|l| LayerEntry { id: l.id.0, name: l.name.clone(), visible: l.visible })
            .collect(),
        guides: doc.guides.clone(),
    };
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(MANIFEST, SimpleFileOptions::default().compression_method(CompressionMethod::Deflated))?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    for layer in &doc.layers {
        // Le PNG est déjà compressé : le re-deflater coûte du temps sans gagner d'espace.
        zip.start_file(
            layer_path(layer.id.0),
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored).large_file(true),
        )?;
        let rgba = RgbaImage::from_raw(layer.pixels.width(), layer.pixels.height(), layer.pixels.to_rgba8())
            .ok_or_else(|| IoError::Invalid("calque de taille incohérente".into()))?;
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(rgba).write_with_encoder(PngEncoder::new_with_quality(
            &mut png,
            CompressionType::Fast,
            FilterType::Adaptive,
        ))?;
        zip.write_all(png.get_ref())?;
    }
    let bytes = zip.finish()?.into_inner();
    crate::atomic_write(path, &bytes)
}

pub(crate) fn read(path: &Path, id: DocId) -> Result<Document, IoError> {
    let mut zip = ZipArchive::new(std::io::BufReader::new(std::fs::File::open(path)?))?;
    let manifest: Manifest = {
        let mut buf = Vec::new();
        zip.by_name(MANIFEST)
            .map_err(|_| IoError::Invalid(format!("« {MANIFEST} » est absent")))?
            .take(MAX_MANIFEST_BYTES)
            .read_to_end(&mut buf)?;
        serde_json::from_slice(&buf)?
    };
    if manifest.format != "openpics" {
        return Err(IoError::Invalid("ce n'est pas un document OpenPics".into()));
    }
    if manifest.schema > SCHEMA {
        return Err(IoError::Invalid(format!(
            "créé par une version plus récente d'OpenPics (schéma {}, cette version lit jusqu'au schéma {SCHEMA})",
            manifest.schema
        )));
    }
    if manifest.layers.is_empty() {
        return Err(IoError::Invalid("aucun calque".into()));
    }
    if manifest.width == 0 || manifest.height == 0 || manifest.width > MAX_DIMENSION || manifest.height > MAX_DIMENSION
    {
        return Err(IoError::Invalid(format!("dimensions invalides {}×{}", manifest.width, manifest.height)));
    }
    let mut doc = Document::new(id, manifest.name.clone(), Size::new(manifest.width, manifest.height))?;
    doc.layers.clear();
    let mut max_id = 0;
    for entry in &manifest.layers {
        let mut data = Vec::new();
        zip.by_name(&layer_path(entry.id))
            .map_err(|_| IoError::Invalid(format!("le calque {} est absent de l'archive", entry.id)))?
            .take(MAX_LAYER_BYTES)
            .read_to_end(&mut data)?;
        let mut reader = ImageReader::new(Cursor::new(data)).with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        reader.limits(limits);
        let rgba = reader.decode()?.into_rgba8();
        if rgba.dimensions() != (manifest.width, manifest.height) {
            return Err(IoError::Invalid(format!("le calque {} n'a pas la taille du document", entry.id)));
        }
        max_id = max_id.max(entry.id);
        doc.layers.push(Layer {
            id: LayerId(entry.id),
            name: entry.name.clone(),
            visible: entry.visible,
            pixels: TileGrid::from_rgba8(manifest.width, manifest.height, rgba.as_raw())?,
        });
    }
    max_id = manifest.guides.iter().map(|g| g.id).fold(max_id, u64::max);
    doc.guides = manifest.guides;
    doc.reserve_ids_up_to(max_id);
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{open, save};
    use openpics_core::Axis;
    use openpics_core::commands::AddGuide;

    fn sample() -> Document {
        let mut d = Document::new(DocId(1), "essai", Size::new(300, 260)).unwrap();
        d.layers[0].pixels.put_pixel(299, 259, [1, 2, 3, 255]);
        let mut top = TileGrid::new(300, 260).unwrap();
        top.put_pixel(0, 0, [9, 8, 7, 200]);
        let id = d.add_layer("Haut", top);
        d.layer_mut(id).unwrap().visible = false;
        d.execute(Box::new(AddGuide::new(Axis::Vertical, 42.5))).unwrap();
        d
    }

    #[test]
    fn roundtrip_preserves_layers_guides_and_ids() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("doc.ocv");
        let mut d = sample();
        save(&mut d, &p).unwrap();
        assert!(!d.is_dirty());
        assert_eq!(d.path.as_deref(), Some(p.as_path()));
        let back = open(&p, DocId(5)).unwrap();
        assert_eq!(back.layers.len(), 2);
        assert_eq!(back.layers[0].pixels.get_pixel(299, 259), [1, 2, 3, 255]);
        assert_eq!(back.layers[1].pixels.get_pixel(0, 0), [9, 8, 7, 200]);
        assert!(!back.layers[1].visible);
        assert_eq!(back.guides, d.guides);
        assert_eq!(back.layers[0].id, d.layers[0].id);
        assert_eq!(back.name, "doc");
        // Les identifiants relus ne doivent pas être réattribués.
        let mut back = back;
        let fresh = back.alloc_id();
        assert!(back.layers.iter().all(|l| l.id.0 != fresh) && back.guides.iter().all(|g| g.id != fresh));
    }

    #[test]
    fn rejects_newer_schema_and_foreign_zip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("new.ocv");
        let mut zip = ZipWriter::new(std::fs::File::create(&p).unwrap());
        zip.start_file(MANIFEST, SimpleFileOptions::default()).unwrap();
        zip.write_all(br#"{"format":"openpics","schema":99,"name":"x","width":1,"height":1,"layers":[]}"#).unwrap();
        zip.finish().unwrap();
        let err = open(&p, DocId(1)).unwrap_err().to_string();
        assert!(err.contains("plus récente"), "{err}");

        let q = dir.path().join("nomanifest.ocv");
        let mut zip = ZipWriter::new(std::fs::File::create(&q).unwrap());
        zip.start_file("other.txt", SimpleFileOptions::default()).unwrap();
        zip.finish().unwrap();
        assert!(open(&q, DocId(1)).is_err());
        std::fs::write(dir.path().join("junk.ocv"), b"PK nope").unwrap();
        assert!(open(&dir.path().join("junk.ocv"), DocId(1)).is_err());
    }

    #[test]
    fn rejects_mismatched_layer_size_and_missing_layer() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bad.ocv");
        let mut zip = ZipWriter::new(std::fs::File::create(&p).unwrap());
        zip.start_file(MANIFEST, SimpleFileOptions::default()).unwrap();
        zip.write_all(br#"{"format":"openpics","schema":1,"name":"x","width":4,"height":4,"layers":[{"id":1,"name":"a","visible":true}]}"#).unwrap();
        zip.finish().unwrap();
        let err = open(&p, DocId(1)).unwrap_err().to_string();
        assert!(err.contains("absent"), "{err}");
    }

    #[test]
    fn write_is_atomic_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("keep.ocv");
        std::fs::write(&p, b"old").unwrap();
        // Un répertoire à la place du fichier temporaire force l'échec avant le renommage.
        std::fs::create_dir(dir.path().join("keep.ocv.openpics-tmp")).unwrap();
        let mut d = sample();
        assert!(save(&mut d, &p).is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b"old");
    }
}
