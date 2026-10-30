//! Sélection : un masque de 0 à 255 (canal rouge, comme les masques de calque ; une tuile absente vaut
//! « non sélectionné »). Ce module ne contient que des opérations pures sur les masques ; la création de
//! formes (rectangle, lasso, baguette magique…) est dans le crate `ops`.

use std::sync::Arc;

use crate::error::CoreError;
use crate::tile::{TILE_SIZE, Tile, TileGrid, TileKey};

pub type Mask = TileGrid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombineMode {
    Replace,
    Add,
    Subtract,
    Intersect,
}

/// Valeur de sélection 0..=255 au pixel (x, y).
pub fn value(mask: &Mask, x: u32, y: u32) -> u8 {
    mask.get_pixel(x, y)[0]
}

fn px(v: u8) -> [u8; 4] {
    [v, v, v, v]
}

/// Applique `f` pixel par pixel sur les tuiles de `a` et `b` ; les tuiles absentes des deux côtés
/// donnent `f(0, 0)` sans être créées si ce résultat est 0.
fn map2(a: &Mask, b: &Mask, f: impl Fn(u8, u8) -> u8) -> Result<Mask, CoreError> {
    let (w, h) = (a.width(), a.height());
    let mut out = TileGrid::new(w, h)?;
    let empty = f(0, 0);
    for ty in 0..a.tiles_y() {
        for tx in 0..a.tiles_x() {
            let key = TileKey { tx, ty };
            let (ta, tb) = (a.tile(key), b.tile(key));
            if ta.is_none() && tb.is_none() {
                if empty != 0 {
                    out.set_tile(key, Some(Arc::new(Tile::Uniform(px(empty)))));
                }
                continue;
            }
            let mut buf = vec![0u8; (TILE_SIZE * TILE_SIZE * 4) as usize];
            for ly in 0..TILE_SIZE {
                for lx in 0..TILE_SIZE {
                    let va = ta.map_or(0, |t| t.pixel(lx, ly)[0]);
                    let vb = tb.map_or(0, |t| t.pixel(lx, ly)[0]);
                    let i = ((ly * TILE_SIZE + lx) * 4) as usize;
                    buf[i..i + 4].copy_from_slice(&px(f(va, vb)));
                }
            }
            let tile = Tile::from_pixels(buf)?;
            if tile != Tile::Uniform([0; 4]) {
                out.set_tile(key, Some(Arc::new(tile)));
            }
        }
    }
    Ok(out)
}

/// Combine une sélection existante (`current`, éventuellement absente) avec une nouvelle forme.
pub fn combine(current: Option<&Mask>, new: &Mask, mode: CombineMode) -> Result<Mask, CoreError> {
    let Some(cur) = current else {
        // Rien de sélectionné : « soustraire » et « intersecter » ne laissent rien ; les autres prennent la forme.
        return match mode {
            CombineMode::Subtract | CombineMode::Intersect => TileGrid::new(new.width(), new.height()),
            _ => Ok(new.clone()),
        };
    };
    match mode {
        CombineMode::Replace => Ok(new.clone()),
        CombineMode::Add => map2(cur, new, u8::max),
        CombineMode::Subtract => map2(cur, new, |a, b| (u16::from(a) * (255 - u16::from(b)) / 255) as u8),
        CombineMode::Intersect => map2(cur, new, u8::min),
    }
}

pub fn invert(mask: &Mask) -> Result<Mask, CoreError> {
    map2(mask, mask, |a, _| 255 - a)
}

/// Sélection totale : des tuiles uniformes blanches.
pub fn select_all(width: u32, height: u32) -> Result<Mask, CoreError> {
    let mut m = TileGrid::new(width, height)?;
    let white = Arc::new(Tile::Uniform(px(255)));
    for ty in 0..m.tiles_y() {
        for tx in 0..m.tiles_x() {
            m.set_tile(TileKey { tx, ty }, Some(white.clone()));
        }
    }
    Ok(m)
}

/// Plus petit rectangle `(x0, y0, x1, y1)` (fin exclusive) contenant tout pixel sélectionné, par tuiles
/// puis par pixels ; `None` si la sélection est vide.
pub fn bounds(mask: &Mask) -> Option<(u32, u32, u32, u32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for (key, tile) in mask.iter() {
        let (ox, oy) = (key.tx * TILE_SIZE, key.ty * TILE_SIZE);
        for ly in 0..TILE_SIZE.min(mask.height() - oy) {
            for lx in 0..TILE_SIZE.min(mask.width() - ox) {
                if tile.pixel(lx, ly)[0] > 0 {
                    x0 = x0.min(ox + lx);
                    y0 = y0.min(oy + ly);
                    x1 = x1.max(ox + lx + 1);
                    y1 = y1.max(oy + ly + 1);
                }
            }
        }
    }
    (x1 > x0).then_some((x0, y0, x1, y1))
}

/// Segments (x0, y0, x1, y1) en pixels de document, le long des frontières entre « sélectionné »
/// (valeur ≥ 128) et « non sélectionné », dans le rectangle `rect` (x0, y0, x1, y1). Plafonné à `limit`
/// segments : un contour illisible à ce niveau de zoom ne doit pas saturer le rendu.
pub fn outline(mask: &Mask, rect: (u32, u32, u32, u32), limit: usize) -> Vec<[f32; 4]> {
    let (w, h) = (mask.width(), mask.height());
    let inside = |x: i64, y: i64| {
        x >= 0 && y >= 0 && x < i64::from(w) && y < i64::from(h) && value(mask, x as u32, y as u32) >= 128
    };
    let (rx0, ry0, rx1, ry1) = (rect.0.min(w), rect.1.min(h), rect.2.min(w), rect.3.min(h));
    let mut out = Vec::new();
    for y in ry0..ry1 {
        for x in rx0..rx1 {
            let here = inside(i64::from(x), i64::from(y));
            // Arête haute et gauche de chaque pixel, plus les arêtes basse/droite du bord du document.
            if here != inside(i64::from(x), i64::from(y) - 1) {
                out.push([x as f32, y as f32, x as f32 + 1.0, y as f32]);
            }
            if here != inside(i64::from(x) - 1, i64::from(y)) {
                out.push([x as f32, y as f32, x as f32, y as f32 + 1.0]);
            }
            if x + 1 == w && here {
                out.push([x as f32 + 1.0, y as f32, x as f32 + 1.0, y as f32 + 1.0]);
            }
            if y + 1 == h && here {
                out.push([x as f32, y as f32 + 1.0, x as f32 + 1.0, y as f32 + 1.0]);
            }
            if out.len() >= limit {
                out.truncate(limit);
                return out;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: u32, h: u32, x0: u32, y0: u32, x1: u32, y1: u32) -> Mask {
        let mut m = TileGrid::new(w, h).unwrap();
        for y in y0..y1 {
            for x in x0..x1 {
                m.put_pixel(x, y, px(255));
            }
        }
        m
    }

    #[test]
    fn combine_modes() {
        let a = rect(300, 300, 0, 0, 100, 100);
        let b = rect(300, 300, 50, 50, 150, 150);
        let add = combine(Some(&a), &b, CombineMode::Add).unwrap();
        let sub = combine(Some(&a), &b, CombineMode::Subtract).unwrap();
        let int = combine(Some(&a), &b, CombineMode::Intersect).unwrap();
        assert_eq!((value(&add, 10, 10), value(&add, 120, 120), value(&add, 200, 200)), (255, 255, 0));
        assert_eq!((value(&sub, 10, 10), value(&sub, 60, 60), value(&sub, 120, 120)), (255, 0, 0));
        assert_eq!((value(&int, 10, 10), value(&int, 60, 60)), (0, 255));
        assert_eq!(value(&combine(Some(&a), &b, CombineMode::Replace).unwrap(), 10, 10), 0);
    }

    #[test]
    fn combining_with_no_selection() {
        let b = rect(64, 64, 10, 10, 20, 20);
        assert_eq!(value(&combine(None, &b, CombineMode::Add).unwrap(), 15, 15), 255);
        assert_eq!(value(&combine(None, &b, CombineMode::Subtract).unwrap(), 15, 15), 0);
        assert_eq!(value(&combine(None, &b, CombineMode::Intersect).unwrap(), 15, 15), 0);
    }

    #[test]
    fn invert_and_select_all_cross_tile_boundaries() {
        let a = rect(300, 300, 250, 250, 260, 260);
        let inv = invert(&a).unwrap();
        assert_eq!((value(&inv, 255, 255), value(&inv, 0, 0), value(&inv, 299, 299)), (0, 255, 255));
        assert_eq!(value(&invert(&inv).unwrap(), 255, 255), 255);
        let all = select_all(300, 300).unwrap();
        assert_eq!(value(&all, 299, 299), 255);
        assert_eq!(bounds(&all), Some((0, 0, 300, 300)));
    }

    #[test]
    fn bounds_and_empty() {
        let a = rect(300, 300, 40, 270, 60, 290);
        assert_eq!(bounds(&a), Some((40, 270, 60, 290)));
        assert_eq!(bounds(&TileGrid::new(10, 10).unwrap()), None);
    }

    #[test]
    fn outline_follows_the_boundary_and_is_capped() {
        let a = rect(32, 32, 8, 8, 16, 16);
        let edges = outline(&a, (0, 0, 32, 32), 10_000);
        assert_eq!(edges.len(), 8 * 4, "périmètre d'un carré de 8 px");
        assert_eq!(outline(&a, (0, 0, 32, 32), 5).len(), 5);
        // Une sélection qui touche le bord du document est fermée par des segments sur ce bord.
        let full = select_all(16, 16).unwrap();
        assert_eq!(outline(&full, (0, 0, 16, 16), 10_000).len(), 16 * 4);
    }
}
