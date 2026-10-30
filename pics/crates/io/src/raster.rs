use std::io::Cursor;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, RgbaImage};
use openpics_core::tile::MAX_DIMENSION;
use openpics_core::{DocId, Document, Size, TileGrid};

use crate::error::IoError;
use crate::format::Format;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PngCompression {
    Fast,
    Default,
    Best,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportOptions {
    /// 1 à 100 ; borné à l'écriture.
    pub jpeg_quality: u8,
    pub png_compression: PngCompression,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self { jpeg_quality: 90, png_compression: PngCompression::Default }
    }
}

/// Plafond mémoire du décodeur : 100 Mpx RGBA 8 bits ≈ 400 Mo, avec de la marge.
fn limits() -> Limits {
    let mut l = Limits::default();
    l.max_image_width = Some(MAX_DIMENSION);
    l.max_image_height = Some(MAX_DIMENSION);
    l.max_alloc = Some(2 * 1024 * 1024 * 1024);
    l
}

pub(crate) fn read(path: &Path, id: DocId) -> Result<Document, IoError> {
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    reader.limits(limits());
    let mut decoder = reader.into_decoder()?;
    // L'orientation EXIF est appliquée à l'ouverture : sinon les photos de téléphone s'ouvrent couchées.
    let orientation = decoder.orientation()?;
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Sans titre");
    let mut doc = Document::new(id, name, Size::new(w, h))?;
    doc.layers[0].pixels = TileGrid::from_rgba8(w, h, rgba.as_raw())?;
    Ok(doc)
}

pub(crate) fn write(doc: &Document, path: &Path, format: Format, options: &ExportOptions) -> Result<(), IoError> {
    let size = doc.size();
    let flat = doc.flatten_rgba8();
    let rgba = RgbaImage::from_raw(size.width, size.height, flat)
        .ok_or_else(|| IoError::Invalid("tampon aplati de taille incohérente".into()))?;
    let mut out = Cursor::new(Vec::new());
    match format {
        Format::Png => {
            let level = match options.png_compression {
                PngCompression::Fast => CompressionType::Fast,
                PngCompression::Default => CompressionType::Default,
                PngCompression::Best => CompressionType::Best,
            };
            DynamicImage::ImageRgba8(rgba).write_with_encoder(PngEncoder::new_with_quality(
                &mut out,
                level,
                FilterType::Adaptive,
            ))?;
        }
        Format::Jpeg => {
            // Le JPEG n'a pas d'alpha : on aplatit sur du blanc plutôt que de laisser le noir
            // des pixels transparents apparaître.
            let rgb = DynamicImage::ImageRgba8(over_white(rgba)).into_rgb8();
            let quality = options.jpeg_quality.clamp(1, 100);
            DynamicImage::ImageRgb8(rgb).write_with_encoder(JpegEncoder::new_with_quality(&mut out, quality))?;
        }
        Format::Webp => DynamicImage::ImageRgba8(rgba).write_to(&mut out, ImageFormat::WebP)?,
        Format::Tiff => DynamicImage::ImageRgba8(rgba).write_to(&mut out, ImageFormat::Tiff)?,
        Format::Bmp => DynamicImage::ImageRgba8(rgba).write_to(&mut out, ImageFormat::Bmp)?,
        Format::Gif => DynamicImage::ImageRgba8(rgba).write_to(&mut out, ImageFormat::Gif)?,
        Format::Ocv => return Err(IoError::UnsupportedFormat("ocv".into())),
    }
    crate::atomic_write(path, out.get_ref())
}

fn over_white(mut img: RgbaImage) -> RgbaImage {
    for p in img.pixels_mut() {
        let a = u32::from(p.0[3]);
        for c in 0..3 {
            p.0[c] = ((u32::from(p.0[c]) * a + 255 * (255 - a) + 127) / 255) as u8;
        }
        p.0[3] = 255;
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{export, open};

    fn doc_with(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Document {
        let mut d = Document::new(DocId(1), "t", Size::new(w, h)).unwrap();
        for y in 0..h {
            for x in 0..w {
                d.layers[0].pixels.put_pixel(x, y, f(x, y));
            }
        }
        d
    }

    #[test]
    fn png_roundtrip_is_lossless() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.png");
        let d = doc_with(300, 40, |x, y| [(x % 256) as u8, (y * 5) as u8, 7, if x % 9 == 0 { 0 } else { 255 }]);
        export(&d, &p, &ExportOptions::default()).unwrap();
        let back = open(&p, DocId(2)).unwrap();
        // Les pixels transparents perdent leur RGB à l'aplatissement : on compare l'aplati.
        assert_eq!(back.flatten_rgba8(), d.flatten_rgba8());
        assert_eq!(back.name, "a");
        assert!(!back.is_dirty());
    }

    #[test]
    fn every_raster_format_roundtrips_dimensions() {
        let dir = tempfile::tempdir().unwrap();
        let d = doc_with(33, 17, |x, y| [x as u8 * 7, y as u8 * 13, 90, 255]);
        for ext in ["png", "jpg", "webp", "tiff", "bmp", "gif"] {
            let p = dir.path().join(format!("img.{ext}"));
            export(&d, &p, &ExportOptions::default()).unwrap_or_else(|e| panic!("{ext}: {e}"));
            let back = open(&p, DocId(2)).unwrap_or_else(|e| panic!("{ext}: {e}"));
            assert_eq!(back.size(), Size::new(33, 17), "{ext}");
        }
    }

    #[test]
    fn jpeg_flattens_transparency_on_white_and_respects_quality() {
        let dir = tempfile::tempdir().unwrap();
        let d = doc_with(16, 16, |_, _| [0, 0, 0, 0]);
        let p = dir.path().join("t.jpg");
        export(&d, &p, &ExportOptions { jpeg_quality: 95, ..ExportOptions::default() }).unwrap();
        let back = open(&p, DocId(2)).unwrap();
        let px = back.layers[0].pixels.get_pixel(8, 8);
        assert!(px[0] > 250 && px[1] > 250 && px[2] > 250, "{px:?}");
    }

    #[test]
    fn corrupt_and_unknown_files_error_instead_of_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.png");
        std::fs::write(&bad, b"not a png at all").unwrap();
        assert!(open(&bad, DocId(1)).is_err());
        let trunc = dir.path().join("trunc.png");
        let d = doc_with(64, 64, |x, y| [x as u8, y as u8, 0, 255]);
        export(&d, &trunc, &ExportOptions::default()).unwrap();
        let bytes = std::fs::read(&trunc).unwrap();
        std::fs::write(&trunc, &bytes[..bytes.len() / 2]).unwrap();
        assert!(open(&trunc, DocId(1)).is_err());
        assert!(open(&dir.path().join("missing.png"), DocId(1)).is_err());
        assert!(open(&dir.path().join("x.xyz"), DocId(1)).is_err());
    }

    #[test]
    fn export_does_not_leave_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let d = doc_with(4, 4, |_, _| [1, 2, 3, 255]);
        export(&d, &dir.path().join("a.png"), &ExportOptions::default()).unwrap();
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names.len(), 1);
    }
}
