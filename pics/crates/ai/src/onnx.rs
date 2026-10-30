//! Inférence de modèles ONNX (fonctionnalité `onnx`).
//!
//! ONNX Runtime est chargé **dynamiquement** (`onnxruntime.dll` / `libonnxruntime.so`) : rien n'est téléchargé à la
//! compilation et la bibliothèque n'est pas livrée avec le logiciel. Sans elle, l'erreur est explicite et les
//! méthodes classiques restent disponibles.
//!
//! Un modèle « masque » prend une image (NCHW, RGB 0..1 normalisé ImageNet) et rend une carte de probabilité
//! (type U²-Net / ISNet / RMBG) qui devient un masque de sélection à la taille de l'image.

use std::path::Path;
use std::sync::OnceLock;

use openpics_core::TileGrid;
use openpics_ops::resize::{resample, resample_mask};
use ort::environment::Environment;
use ort::session::Session;
use ort::value::{Tensor, ValueType};

use crate::catalog::AiError;

const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];
/// Taille d'entrée quand le modèle accepte des dimensions dynamiques.
const DEFAULT_SIZE: usize = 320;

fn ai(e: impl std::fmt::Display) -> AiError {
    AiError::Invalid(format!("ONNX Runtime : {e}"))
}

static ENV: OnceLock<Result<Environment, String>> = OnceLock::new();

/// Crée l'environnement ONNX Runtime (une fois par processus). `dylib` indique où trouver la bibliothèque ; sans
/// lui, `ORT_DYLIB_PATH` puis le chemin système. La télémétrie d'ONNX Runtime est **désactivée** explicitement.
pub fn init(dylib: Option<&Path>) -> Result<&'static Environment, AiError> {
    let env = ENV.get_or_init(|| {
        let builder = match dylib {
            Some(path) => ort::init_from(path).map_err(|e| e.to_string())?,
            None => ort::init(),
        };
        builder.with_name("openpics").with_telemetry(false).build().map_err(|e| e.to_string())
    });
    env.as_ref().map_err(ai)
}

fn input_size(session: &Session) -> (usize, usize) {
    let dims = session.inputs().first().and_then(|i| match i.dtype() {
        ValueType::Tensor { shape, .. } => Some(shape.iter().copied().collect::<Vec<i64>>()),
        _ => None,
    });
    match dims.as_deref() {
        Some([_, 3, h, w]) if *h > 0 && *w > 0 => (*w as usize, *h as usize),
        _ => (DEFAULT_SIZE, DEFAULT_SIZE),
    }
}

/// Exécute un modèle image → carte de probabilité et renvoie un masque 0..255 de la taille de `image`.
pub fn run_mask(model: &Path, image: &TileGrid) -> Result<TileGrid, AiError> {
    let env = init(None)?;
    let mut session = Session::builder(env).map_err(ai)?.commit_from_file(model).map_err(ai)?;
    let (w, h) = input_size(&session);
    // Mise à l'échelle d'entrée : on aplatit sur fond blanc pour que la transparence ne soit pas lue comme du noir.
    let small = resample(image, w as u32, h as u32)?.to_rgba8();
    let mut data = vec![0f32; 3 * w * h];
    for (i, px) in small.chunks_exact(4).enumerate() {
        let a = f32::from(px[3]) / 255.0;
        for c in 0..3 {
            let v = f32::from(px[c]) / 255.0 * a + (1.0 - a);
            data[c * w * h + i] = (v - MEAN[c]) / STD[c];
        }
    }
    let input = Tensor::from_array(([1usize, 3, h, w], data)).map_err(ai)?;
    let name = session.inputs().first().map(|i| i.name().to_owned()).ok_or_else(|| ai("le modèle n'a pas d'entrée"))?;
    let outputs = session.run(ort::inputs![name => input]).map_err(ai)?;
    let (shape, values) = outputs[0].try_extract_tensor::<f32>().map_err(ai)?;
    let dims: Vec<usize> = shape.iter().map(|&d| d.max(0) as usize).collect();
    let (oh, ow) = match dims.as_slice() {
        [.., oh, ow] if oh * ow > 0 && values.len() >= oh * ow => (*oh, *ow),
        _ => return Err(ai(format!("sortie inattendue : forme {dims:?}"))),
    };
    // Première carte seulement ; normalisée en 0..1 si le modèle sort des logits ou une échelle libre.
    let map = &values[..oh * ow];
    let (lo, hi) = map.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    let in_unit = lo >= -1e-3 && hi <= 1.0 + 1e-3;
    let scale = if in_unit || hi - lo < 1e-9 { 1.0 } else { hi - lo };
    let offset = if in_unit { 0.0 } else { lo };
    let mut rgba = Vec::with_capacity(oh * ow * 4);
    for &v in map {
        let b = (((v - offset) / scale).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        rgba.extend_from_slice(&[b, b, b, b]);
    }
    let small_mask = TileGrid::from_rgba8(ow as u32, oh as u32, &rgba)?;
    Ok(resample_mask(&small_mask, image.width(), image.height())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les tests exigent la bibliothèque (`ORT_DYLIB_PATH`) et un modèle jouet (`OPENPICS_TEST_ONNX`). Sans eux ils
    /// sont ignorés, sauf si `OPENPICS_REQUIRE_ORT=1` : un test « vert » parce que sauté ne prouve rien.
    fn fixture() -> Option<std::path::PathBuf> {
        let model = std::env::var_os("OPENPICS_TEST_ONNX").map(std::path::PathBuf::from);
        if model.is_none() || std::env::var_os("ORT_DYLIB_PATH").is_none() {
            assert!(
                std::env::var_os("OPENPICS_REQUIRE_ORT").is_none(),
                "OPENPICS_REQUIRE_ORT=1 mais ORT/modèle absents"
            );
            return None;
        }
        model
    }

    #[test]
    fn a_toy_model_yields_a_mask_of_the_image_size() {
        let Some(model) = fixture() else { return };
        // Image : carré noir sur fond blanc ; le modèle jouet rend 1 - luminance moyenne => le carré ressort.
        let mut rgba = vec![255u8; 64 * 48 * 4];
        for y in 12..36 {
            for x in 16..48 {
                let i = (y * 64 + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
        let image = TileGrid::from_rgba8(64, 48, &rgba).unwrap();
        let mask = run_mask(&model, &image).unwrap();
        assert_eq!((mask.width(), mask.height()), (64, 48));
        assert!(
            mask.get_pixel(32, 24)[0] > 200,
            "le centre (carré noir) est sélectionné : {:?}",
            mask.get_pixel(32, 24)
        );
        assert!(mask.get_pixel(2, 2)[0] < 60, "le fond blanc ne l'est pas : {:?}", mask.get_pixel(2, 2));
        assert!(run_mask(Path::new("/nonexistent.onnx"), &image).is_err());
    }

    /// Vérification manuelle avec un vrai modèle (U²-Net) : `OPENPICS_REAL_MODEL`, `OPENPICS_REAL_IMAGE` (JPEG/PNG) et
    /// `OPENPICS_REAL_OUT` (PNG du masque écrit). Ignoré si ces variables sont absentes.
    #[test]
    fn a_real_u2net_model_separates_the_subject_from_the_background() {
        let (Some(model), Some(img), Some(out)) = (
            std::env::var_os("OPENPICS_REAL_MODEL"),
            std::env::var_os("OPENPICS_REAL_IMAGE"),
            std::env::var_os("OPENPICS_REAL_OUT"),
        ) else {
            return;
        };
        if std::env::var_os("ORT_DYLIB_PATH").is_none() {
            return;
        }
        let doc = openpics_io::open(Path::new(&img), openpics_core::DocId(1)).unwrap();
        let image = doc.layers[0].pixels.clone();
        let mask = run_mask(Path::new(&model), &image).unwrap();
        let rgba = mask.to_rgba8();
        let covered = rgba.chunks_exact(4).filter(|p| p[0] > 128).count() as f64 / (rgba.len() / 4) as f64;
        println!("part sélectionnée : {:.1} %", covered * 100.0);
        assert!((0.03..0.9).contains(&covered), "ni vide ni tout : {covered}");
        let gray: Vec<u8> = rgba.chunks_exact(4).map(|p| p[0]).collect();
        image::save_buffer(&out, &gray, mask.width(), mask.height(), image::ColorType::L8).unwrap();
    }
}
