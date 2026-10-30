//! Opérations sur pixels. Elles ne modifient jamais le document : elles produisent une nouvelle
//! `TileGrid` qui partage ses tuiles inchangées avec l'ancienne ; l'appelant en déduit la
//! commande d'historique (`ReplaceTiles::between`).

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
    let old = &doc.layer(layer)?.pixels;
    let new = op.run(old)?;
    let Some(cmd) = ReplaceTiles::between(op.label(), layer, old, &new) else {
        return Ok(false);
    };
    doc.execute(Box::new(cmd))?;
    Ok(true)
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
}
