//! Opérations sur pixels. Elles ne modifient jamais le document : elles produisent une nouvelle
//! `TileGrid` qui partage ses tuiles inchangées avec l'ancienne ; l'appelant en déduit la
//! commande d'historique (`ReplaceTiles::between`).

pub mod brush;
pub mod fill;
pub mod filters;
pub mod liquify;
pub mod resize;
pub mod retouch;
pub mod selection;
pub mod stats;
pub mod text;
pub mod transform;
pub mod vector;

use std::sync::Arc;

use openpics_core::CoreError;
use openpics_core::commands::ReplaceTiles;
use openpics_core::{Document, LayerId, TILE_SIZE, Tile, TileGrid, TileKey};
use rayon::prelude::*;

/// Opérations de la phase 1, exposées au menu « Image ».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    FlipHorizontal,
    FlipVertical,
    Invert,
}

impl Op {
    pub fn label(self) -> &'static str {
        match self {
            Op::FlipHorizontal => "Retourner horizontalement",
            Op::FlipVertical => "Retourner verticalement",
            Op::Invert => "Inverser les couleurs",
        }
    }

    pub fn run(self, src: &TileGrid) -> Result<TileGrid, CoreError> {
        match self {
            Op::FlipHorizontal => flip(src, true),
            Op::FlipVertical => flip(src, false),
            Op::Invert => Ok(invert(src)),
        }
    }
}

/// Applique `op` au calque et l'enregistre dans l'historique. Renvoie `false` si rien n'a changé.
pub fn apply_to_layer(doc: &mut Document, layer: LayerId, op: Op) -> Result<bool, CoreError> {
    let target = doc.layer(layer)?;
    if !target.is_raster() {
        return Err(CoreError::NotRaster(target.name.clone()));
    }
    if target.locks.blocks_painting() {
        return Err(CoreError::LayerLocked(target.name.clone()));
    }
    let old = &target.pixels;
    let mut new = op.run(old)?;
    // Avec une sélection active, l'opération n'agit qu'à travers elle (bords adoucis compris).
    if let Some(selection) = &doc.selection {
        new = selection::through_selection(old, &new, selection)?;
    }
    let Some(cmd) = ReplaceTiles::between(op.label(), layer, old, &new) else {
        return Ok(false);
    };
    doc.execute(Box::new(cmd))?;
    Ok(true)
}

fn record(doc: &mut Document, layer: LayerId, name: &str, new: TileGrid) -> Result<bool, CoreError> {
    let Some(cmd) = ReplaceTiles::between(name, layer, &doc.layer(layer)?.pixels, &new) else { return Ok(false) };
    doc.execute(Box::new(cmd))?;
    Ok(true)
}

fn check_movable(doc: &Document, layer: LayerId) -> Result<(), CoreError> {
    let l = doc.layer(layer)?;
    if !l.is_raster() {
        return Err(CoreError::NotRaster(l.name.clone()));
    }
    if l.locks.blocks_moving() { Err(CoreError::LayerLocked(l.name.clone())) } else { Ok(()) }
}

/// Transforme les pixels d'un calque (échelle, rotation, perspective…) : annulable, refusé si la
/// position du calque est verrouillée.
pub fn apply_transform(doc: &mut Document, layer: LayerId, m: &transform::Matrix3) -> Result<bool, CoreError> {
    check_movable(doc, layer)?;
    let new = transform::transform(&doc.layer(layer)?.pixels, m)?;
    record(doc, layer, "Transformation", new)
}

/// Déformation par grille d'un calque (voir `transform::warp`).
pub fn apply_warp(
    doc: &mut Document,
    layer: LayerId,
    cols: usize,
    rows: usize,
    offsets: &[(f64, f64)],
) -> Result<bool, CoreError> {
    check_movable(doc, layer)?;
    let new = transform::warp(&doc.layer(layer)?.pixels, cols, rows, offsets)?;
    record(doc, layer, "Déformation", new)
}

/// Retournement : chaque tuile de destination lit au plus deux tuiles source par ligne ; un petit
/// cache évite une recherche de table par pixel.
fn flip(src: &TileGrid, horizontal: bool) -> Result<TileGrid, CoreError> {
    let (w, h) = (src.width(), src.height());
    let keys: Vec<TileKey> =
        (0..src.tiles_y()).flat_map(|ty| (0..src.tiles_x()).map(move |tx| TileKey { tx, ty })).collect();
    let built: Vec<(TileKey, Option<Tile>)> = keys
        .par_iter()
        .map(|&key| {
            let mut buf = vec![0u8; (TILE_SIZE * TILE_SIZE * 4) as usize];
            let mut any = false;
            let mut cache: Option<(TileKey, Option<&Arc<Tile>>)> = None;
            let x0 = key.tx * TILE_SIZE;
            let y0 = key.ty * TILE_SIZE;
            for ly in 0..TILE_SIZE.min(h - y0) {
                for lx in 0..TILE_SIZE.min(w - x0) {
                    let (dx, dy) = (x0 + lx, y0 + ly);
                    let (sx, sy) = if horizontal { (w - 1 - dx, dy) } else { (dx, h - 1 - dy) };
                    let sk = TileKey { tx: sx / TILE_SIZE, ty: sy / TILE_SIZE };
                    let tile = match cache {
                        Some((k, t)) if k == sk => t,
                        _ => {
                            let t = src.tile(sk);
                            cache = Some((sk, t));
                            t
                        }
                    };
                    let Some(tile) = tile else { continue };
                    let px = tile.pixel(sx % TILE_SIZE, sy % TILE_SIZE);
                    if px != [0; 4] {
                        any = true;
                    }
                    let i = ((ly * TILE_SIZE + lx) * 4) as usize;
                    buf[i..i + 4].copy_from_slice(&px);
                }
            }
            let tile = if any { Tile::from_pixels(buf).ok() } else { None };
            (key, tile)
        })
        .collect();
    let mut out = TileGrid::new(w, h)?;
    for (key, tile) in built {
        out.set_tile(key, tile.map(Arc::new));
    }
    Ok(out)
}

/// Inverse R, G, B ; l'alpha est conservé. Les tuiles absentes (transparentes) le restent.
fn invert(src: &TileGrid) -> TileGrid {
    let mut out = src.clone();
    let tiles: Vec<(TileKey, Arc<Tile>)> = src.iter().map(|(k, t)| (*k, t.clone())).collect();
    let mapped: Vec<(TileKey, Tile)> = tiles
        .par_iter()
        .map(|(k, t)| {
            let tile = match t.as_ref() {
                Tile::Uniform([r, g, b, a]) => Tile::Uniform([255 - r, 255 - g, 255 - b, *a]),
                Tile::Pixels(d) => {
                    let mut v = d.to_vec();
                    for p in v.chunks_exact_mut(4) {
                        p[0] = 255 - p[0];
                        p[1] = 255 - p[1];
                        p[2] = 255 - p[2];
                    }
                    Tile::Pixels(v.into_boxed_slice())
                }
            };
            (*k, tile)
        })
        .collect();
    for (k, t) in mapped {
        out.set_tile(k, Some(Arc::new(t)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpics_core::{DocId, Size};

    fn sample(w: u32, h: u32) -> TileGrid {
        let rgba: Vec<u8> = (0..w * h)
            .flat_map(|i| [(i % 251) as u8, (i / 7 % 241) as u8, (i % 13) as u8, if i % 5 == 0 { 0 } else { 255 }])
            .collect();
        TileGrid::from_rgba8(w, h, &rgba).unwrap()
    }

    #[test]
    fn flip_horizontal_matches_reference_across_tile_boundaries() {
        let (w, h) = (300, 270);
        let src = sample(w, h);
        let out = Op::FlipHorizontal.run(&src).unwrap();
        for y in [0, 1, 255, 256, 269] {
            for x in [0, 1, 44, 45, 255, 256, 299] {
                assert_eq!(out.get_pixel(x, y), src.get_pixel(w - 1 - x, y), "({x},{y})");
            }
        }
    }

    #[test]
    fn flip_vertical_matches_reference() {
        let (w, h) = (270, 300);
        let src = sample(w, h);
        let out = Op::FlipVertical.run(&src).unwrap();
        for y in [0, 43, 44, 255, 299] {
            for x in [0, 255, 256, 269] {
                assert_eq!(out.get_pixel(x, y), src.get_pixel(x, h - 1 - y), "({x},{y})");
            }
        }
    }

    #[test]
    fn double_flip_is_identity() {
        let src = sample(300, 300);
        let twice = Op::FlipHorizontal.run(&Op::FlipHorizontal.run(&src).unwrap()).unwrap();
        assert_eq!(twice.to_rgba8(), src.to_rgba8());
    }

    #[test]
    fn invert_keeps_alpha_and_is_involutive() {
        let src = sample(40, 40);
        let inv = Op::Invert.run(&src).unwrap();
        let p = src.get_pixel(1, 0);
        assert_eq!(inv.get_pixel(1, 0), [255 - p[0], 255 - p[1], 255 - p[2], p[3]]);
        assert_eq!(Op::Invert.run(&inv).unwrap().to_rgba8(), src.to_rgba8());
    }

    #[test]
    fn locked_layers_refuse_pixel_operations() {
        let mut doc = Document::new(DocId(1), "t", Size::new(8, 8)).unwrap();
        let layer = doc.layers[0].id;
        doc.layers[0].pixels.put_pixel(0, 0, [1, 2, 3, 255]);
        for lock in [|l: &mut openpics_core::Locks| l.pixels = true, |l: &mut openpics_core::Locks| l.all = true] {
            lock(&mut doc.layers[0].locks);
            assert!(matches!(apply_to_layer(&mut doc, layer, Op::Invert), Err(CoreError::LayerLocked(_))));
            doc.layers[0].locks = openpics_core::Locks::default();
        }
        // Le verrou de position n'empêche pas de peindre.
        doc.layers[0].locks.position = true;
        assert!(apply_to_layer(&mut doc, layer, Op::Invert).unwrap());
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), [254, 253, 252, 255]);
    }

    #[test]
    fn apply_to_layer_is_undoable_and_skips_noops() {
        let mut doc = Document::new(DocId(1), "t", Size::new(64, 64)).unwrap();
        let layer = doc.layers[0].id;
        // Calque vide : retourner ne change rien et ne crée pas d'état d'historique.
        assert!(!apply_to_layer(&mut doc, layer, Op::FlipHorizontal).unwrap());
        assert!(!doc.history().can_undo());
        doc.layers[0].pixels.put_pixel(0, 0, [10, 20, 30, 255]);
        assert!(apply_to_layer(&mut doc, layer, Op::FlipHorizontal).unwrap());
        assert_eq!(doc.layers[0].pixels.get_pixel(63, 0), [10, 20, 30, 255]);
        doc.undo().unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), [10, 20, 30, 255]);
        assert_eq!(doc.layers[0].pixels.get_pixel(63, 0), [0; 4]);
    }

    #[test]
    fn transform_command_is_undoable_and_respects_the_position_lock() {
        use crate::transform::Matrix3;
        let mut doc = Document::new(DocId(1), "t", Size::new(64, 64)).unwrap();
        let layer = doc.layers[0].id;
        doc.layers[0].pixels.put_pixel(10, 10, [9, 9, 9, 255]);
        assert!(apply_transform(&mut doc, layer, &Matrix3::translate(5.0, 0.0)).unwrap());
        assert_eq!(doc.layers[0].pixels.get_pixel(15, 10), [9, 9, 9, 255]);
        doc.undo().unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(10, 10), [9, 9, 9, 255]);
        doc.layers[0].locks.position = true;
        assert!(matches!(
            apply_transform(&mut doc, layer, &Matrix3::translate(1.0, 1.0)),
            Err(CoreError::LayerLocked(_))
        ));
        assert!(matches!(apply_warp(&mut doc, layer, 2, 2, &[(0.0, 0.0); 4]), Err(CoreError::LayerLocked(_))));
        // Verrouiller les pixels n'interdit pas de déplacer ; l'inverse non plus.
        doc.layers[0].locks = openpics_core::Locks { pixels: true, ..Default::default() };
        assert!(apply_transform(&mut doc, layer, &Matrix3::translate(1.0, 0.0)).is_ok());
    }

    #[test]
    fn operations_only_touch_the_selection() {
        let mut doc = Document::new(DocId(1), "t", Size::new(64, 64)).unwrap();
        let layer = doc.layers[0].id;
        for y in 0..64 {
            for x in 0..64 {
                doc.layers[0].pixels.put_pixel(x, y, [10, 10, 10, 255]);
            }
        }
        doc.selection = Some(selection::rect(64, 64, (8.0, 8.0), (16.0, 16.0)).unwrap());
        assert!(apply_to_layer(&mut doc, layer, Op::Invert).unwrap());
        assert_eq!(doc.layers[0].pixels.get_pixel(10, 10), [245, 245, 245, 255]);
        assert_eq!(doc.layers[0].pixels.get_pixel(40, 40), [10, 10, 10, 255]);
        doc.undo().unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(10, 10), [10, 10, 10, 255]);
    }
}
