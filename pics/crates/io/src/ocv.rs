//! Format natif `.ocv` : une archive zip contenant `document.json` et un PNG par calque.
//!
//! Choix : zip + PNG pour qu'un fichier reste récupérable avec des outils standards, même sans
//! OpenPics. Le champ `schema` permet de refuser proprement un fichier plus récent que l'app.

use std::io::{Cursor, Read, Write};
use std::path::Path;

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{DynamicImage, ImageReader, RgbaImage};
use openpics_core::tile::MAX_DIMENSION;
use openpics_core::{BlendMode, DocId, Document, Guide, Layer, LayerId, LayerStyle, Locks, Mask, Size, TileGrid};
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::IoError;

/// Schéma 2 : propriétés de calque (opacité, fusion, verrous, liaison, écrêtage) et masques.
/// Le schéma 1 reste lisible : les champs absents prennent leur valeur par défaut.
const SCHEMA: u32 = 2;
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

fn one() -> f32 {
    1.0
}

#[derive(Serialize, Deserialize)]
struct LayerEntry {
    id: u64,
    name: String,
    visible: bool,
    #[serde(default = "one")]
    opacity: f32,
    #[serde(default = "one")]
    fill: f32,
    #[serde(default)]
    blend: BlendMode,
    #[serde(default)]
    locks: Locks,
    #[serde(default)]
    link: Option<u64>,
    #[serde(default)]
    clip_to_below: bool,
    #[serde(default)]
    mask: Option<MaskEntry>,
    #[serde(default)]
    styles: Vec<LayerStyle>,
}

#[derive(Serialize, Deserialize)]
struct MaskEntry {
    enabled: bool,
}

impl LayerEntry {
    fn from_layer(l: &Layer) -> Self {
        Self {
            id: l.id.0,
            name: l.name.clone(),
            visible: l.visible,
            opacity: l.opacity,
            fill: l.fill,
            blend: l.blend,
            locks: l.locks,
            link: l.link,
            clip_to_below: l.clip_to_below,
            mask: l.mask.as_ref().map(|m| MaskEntry { enabled: m.enabled }),
            styles: l.styles.clone(),
        }
    }
}

fn layer_path(id: u64) -> String {
    format!("layers/{id}.png")
}

fn mask_path(id: u64) -> String {
    format!("masks/{id}.png")
}

fn encode_png(grid: &TileGrid) -> Result<Vec<u8>, IoError> {
    let rgba = RgbaImage::from_raw(grid.width(), grid.height(), grid.to_rgba8())
        .ok_or_else(|| IoError::Invalid("calque de taille incohérente".into()))?;
    let mut png = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(rgba).write_with_encoder(PngEncoder::new_with_quality(
        &mut png,
        CompressionType::Fast,
        FilterType::Adaptive,
    ))?;
    Ok(png.into_inner())
}

fn read_grid<R: Read + std::io::Seek>(zip: &mut ZipArchive<R>, name: &str, size: Size) -> Result<TileGrid, IoError> {
    let mut data = Vec::new();
    zip.by_name(name)
        .map_err(|_| IoError::Invalid(format!("« {name} » est absent de l'archive")))?
        .take(MAX_LAYER_BYTES)
        .read_to_end(&mut data)?;
    let mut reader = ImageReader::new(Cursor::new(data)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    reader.limits(limits);
    let rgba = reader.decode()?.into_rgba8();
    if rgba.dimensions() != (size.width, size.height) {
        return Err(IoError::Invalid(format!("« {name} » n'a pas la taille du document")));
    }
    Ok(TileGrid::from_rgba8(size.width, size.height, rgba.as_raw())?)
}

pub(crate) fn write(doc: &Document, path: &Path) -> Result<(), IoError> {
    let size = doc.size();
    let manifest = Manifest {
        format: "openpics".into(),
        schema: SCHEMA,
        name: doc.name.clone(),
        width: size.width,
        height: size.height,
        layers: doc.layers.iter().map(LayerEntry::from_layer).collect(),
        guides: doc.guides.clone(),
    };
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(MANIFEST, SimpleFileOptions::default().compression_method(CompressionMethod::Deflated))?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    // Le PNG est déjà compressé : le re-deflater coûte du temps sans gagner d'espace.
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored).large_file(true);
    for layer in &doc.layers {
        zip.start_file(layer_path(layer.id.0), stored)?;
        zip.write_all(&encode_png(&layer.pixels)?)?;
        if let Some(mask) = &layer.mask {
            zip.start_file(mask_path(layer.id.0), stored)?;
            zip.write_all(&encode_png(&mask.pixels)?)?;
        }
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
    let doc_size = Size::new(manifest.width, manifest.height);
    for entry in &manifest.layers {
        let pixels = read_grid(&mut zip, &layer_path(entry.id), doc_size)?;
        let mut layer = Layer::new(LayerId(entry.id), entry.name.clone(), pixels);
        layer.visible = entry.visible;
        // `set_props` borne opacité et remplissage : un fichier trafiqué ne peut pas produire un alpha invalide.
        let mut props = layer.props();
        props.opacity = entry.opacity;
        props.fill = entry.fill;
        props.blend = entry.blend;
        props.locks = entry.locks;
        props.link = entry.link;
        props.clip_to_below = entry.clip_to_below;
        layer.set_props(&props);
        layer.styles = entry.styles.iter().map(LayerStyle::sanitized).collect();
        if let Some(m) = &entry.mask {
            layer.mask =
                Some(Mask { pixels: read_grid(&mut zip, &mask_path(entry.id), doc_size)?, enabled: m.enabled });
        }
        max_id = max_id.max(entry.id);
        doc.layers.push(layer);
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
    fn roundtrip_preserves_layer_properties_and_masks() {
        use openpics_core::commands::{AddMask, SetLayerProps};
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("props.ocv");
        let mut d = sample();
        let top = d.layers[1].id;
        let mut props = d.layer(top).unwrap().props();
        props.opacity = 0.35;
        props.fill = 0.8;
        props.blend = BlendMode::SoftLight;
        props.locks = Locks { pixels: true, position: false, all: false };
        props.link = Some(7);
        props.clip_to_below = true;
        d.execute(Box::new(SetLayerProps::new("p", top, props))).unwrap();
        d.execute(Box::new(AddMask::new(top, false))).unwrap();
        d.layer_mut(top).unwrap().mask.as_mut().unwrap().pixels.put_pixel(3, 4, [255, 255, 255, 255]);
        d.layer_mut(top).unwrap().mask.as_mut().unwrap().enabled = false;
        save(&mut d, &p).unwrap();
        let back = open(&p, DocId(9)).unwrap();
        let l = &back.layers[1];
        assert_eq!(
            (l.opacity, l.fill, l.blend, l.link, l.clip_to_below),
            (0.35, 0.8, BlendMode::SoftLight, Some(7), true)
        );
        assert!(l.locks.pixels && !l.locks.all);
        let m = l.mask.as_ref().unwrap();
        assert!(!m.enabled);
        assert_eq!(m.pixels.get_pixel(3, 4)[0], 255);
        assert_eq!(m.pixels.get_pixel(0, 0)[0], 0);
        assert_eq!(back.flatten_rgba8(), {
            let mut again = open(&p, DocId(10)).unwrap();
            again.layers[1].mask.as_mut().unwrap().enabled = false;
            again.flatten_rgba8()
        });
    }

    #[test]
    fn styles_roundtrip_and_hostile_values_are_bounded() {
        use openpics_core::commands::SetLayerStyles;
        use openpics_core::styles::StrokePosition;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("styles.ocv");
        let mut d = sample();
        let top = d.layers[1].id;
        let styles = vec![
            LayerStyle::DropShadow { color: [1, 2, 3], opacity: 0.6, angle_deg: 45.0, distance: 7.0, size: 4.0 },
            LayerStyle::Stroke { color: [9, 8, 7], opacity: 1.0, size: 2.0, position: StrokePosition::Center },
            LayerStyle::GradientOverlay {
                from: [0; 3],
                to: [255; 3],
                angle_deg: 90.0,
                opacity: 0.5,
                blend: BlendMode::Overlay,
            },
        ];
        d.execute(Box::new(SetLayerStyles::new("s", top, styles.clone()))).unwrap();
        save(&mut d, &p).unwrap();
        assert_eq!(open(&p, DocId(2)).unwrap().layers[1].styles, styles);

        let q = dir.path().join("hostile.ocv");
        let mut zip = ZipWriter::new(std::fs::File::create(&q).unwrap());
        zip.start_file(MANIFEST, SimpleFileOptions::default()).unwrap();
        zip.write_all(br#"{"format":"openpics","schema":2,"name":"x","width":1,"height":1,"layers":[{"id":1,"name":"a","visible":true,"styles":[{"kind":"outer_glow","color":[1,2,3],"opacity":99.0,"size":1e12}]}]}"#).unwrap();
        zip.start_file(layer_path(1), SimpleFileOptions::default()).unwrap();
        let mut png = Vec::new();
        RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 255]))
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        zip.write_all(&png).unwrap();
        zip.finish().unwrap();
        match &open(&q, DocId(3)).unwrap().layers[0].styles[0] {
            LayerStyle::OuterGlow { opacity, size, .. } => assert!(*opacity <= 1.0 && *size <= 250.0),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn schema_1_files_still_open_with_default_properties() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("old.ocv");
        let mut zip = ZipWriter::new(std::fs::File::create(&p).unwrap());
        zip.start_file(MANIFEST, SimpleFileOptions::default()).unwrap();
        zip.write_all(br#"{"format":"openpics","schema":1,"name":"x","width":2,"height":2,"layers":[{"id":3,"name":"Fond","visible":true}]}"#).unwrap();
        zip.start_file(layer_path(3), SimpleFileOptions::default()).unwrap();
        let mut png = Vec::new();
        RgbaImage::from_pixel(2, 2, image::Rgba([9, 8, 7, 255]))
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        zip.write_all(&png).unwrap();
        zip.finish().unwrap();
        let d = open(&p, DocId(1)).unwrap();
        let l = &d.layers[0];
        assert_eq!(
            (l.opacity, l.fill, l.blend, l.mask.is_none(), l.clip_to_below),
            (1.0, 1.0, BlendMode::Normal, true, false)
        );
        assert_eq!(l.pixels.get_pixel(1, 1), [9, 8, 7, 255]);
    }

    #[test]
    fn tampered_opacity_is_clamped_on_read() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bad.ocv");
        let mut zip = ZipWriter::new(std::fs::File::create(&p).unwrap());
        zip.start_file(MANIFEST, SimpleFileOptions::default()).unwrap();
        zip.write_all(br#"{"format":"openpics","schema":2,"name":"x","width":1,"height":1,"layers":[{"id":1,"name":"a","visible":true,"opacity":42.0,"fill":-3.0}]}"#).unwrap();
        zip.start_file(layer_path(1), SimpleFileOptions::default()).unwrap();
        let mut png = Vec::new();
        RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 255]))
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        zip.write_all(&png).unwrap();
        zip.finish().unwrap();
        let d = open(&p, DocId(1)).unwrap();
        assert_eq!((d.layers[0].opacity, d.layers[0].fill), (1.0, 0.0));
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
