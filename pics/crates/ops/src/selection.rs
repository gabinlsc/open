//! Création et transformation de sélections (formes, baguette magique, plage de couleurs, contour
//! progressif, agrandir/contracter) et application d'une opération « à travers » la sélection.

use std::collections::VecDeque;
use std::sync::Arc;

use openpics_core::selection::{bounds, value};
use openpics_core::styles::Region;
use openpics_core::{CoreError, TILE_SIZE, Tile, TileGrid, TileKey};

fn px(v: u8) -> [u8; 4] {
    [v, v, v, v]
}

/// Construit un masque depuis une carte de couverture (0..1) sur le rectangle `(x0, y0, w, h)`.
fn mask_from_coverage(
    width: u32,
    height: u32,
    x0: u32,
    y0: u32,
    w: usize,
    h: usize,
    cov: &[f32],
) -> Result<TileGrid, CoreError> {
    let mut m = TileGrid::new(width, height)?;
    for row in 0..h {
        for col in 0..w {
            let c = cov[row * w + col];
            if c > 0.0 {
                let (x, y) = (x0 + col as u32, y0 + row as u32);
                if x < width && y < height {
                    m.put_pixel(x, y, px((c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8));
                }
            }
        }
    }
    Ok(m)
}

/// Rectangle aligné sur les pixels (comme les éditeurs raster : pas d'anticrénelage). Les coins peuvent
/// être donnés dans n'importe quel ordre et sortir du document.
pub fn rect(width: u32, height: u32, a: (f64, f64), b: (f64, f64)) -> Result<TileGrid, CoreError> {
    let (x0, x1) = (a.0.min(b.0).round().max(0.0), a.0.max(b.0).round().min(f64::from(width)));
    let (y0, y1) = (a.1.min(b.1).round().max(0.0), a.1.max(b.1).round().min(f64::from(height)));
    let mut m = TileGrid::new(width, height)?;
    if x1 <= x0 || y1 <= y0 {
        return Ok(m);
    }
    for y in y0 as u32..y1 as u32 {
        for x in x0 as u32..x1 as u32 {
            m.put_pixel(x, y, px(255));
        }
    }
    Ok(m)
}

/// Ellipse inscrite dans le rectangle `a`-`b`, anticrénelée (grille 4×4 de sous-échantillons par pixel).
pub fn ellipse(width: u32, height: u32, a: (f64, f64), b: (f64, f64)) -> Result<TileGrid, CoreError> {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    if rx < 0.5 || ry < 0.5 {
        return TileGrid::new(width, height);
    }
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (px0, py0) = (x0.floor().max(0.0) as u32, y0.floor().max(0.0) as u32);
    let (px1, py1) = ((x1.ceil() as u32).min(width), (y1.ceil() as u32).min(height));
    let (w, h) = (px1.saturating_sub(px0) as usize, py1.saturating_sub(py0) as usize);
    let mut cov = vec![0.0f32; w * h];
    for row in 0..h {
        for col in 0..w {
            let mut n = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let x = f64::from(px0) + col as f64 + (f64::from(sx) + 0.5) / 4.0;
                    let y = f64::from(py0) + row as f64 + (f64::from(sy) + 0.5) / 4.0;
                    if ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2) <= 1.0 {
                        n += 1;
                    }
                }
            }
            cov[row * w + col] = n as f32 / 16.0;
        }
    }
    mask_from_coverage(width, height, px0, py0, w, h, &cov)
}

/// Polygone (lasso libre ou polygonal), règle pair-impair, anticrénelé : 4 sous-lignes par pixel avec
/// couverture horizontale exacte.
pub fn polygon(width: u32, height: u32, points: &[(f64, f64)]) -> Result<TileGrid, CoreError> {
    if points.len() < 3 || points.iter().any(|(x, y)| !x.is_finite() || !y.is_finite()) {
        return TileGrid::new(width, height);
    }
    let min_y = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min).floor().max(0.0);
    let max_y = points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max).ceil().min(f64::from(height));
    let min_x = points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min).floor().max(0.0);
    let max_x = points.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max).ceil().min(f64::from(width));
    if max_y <= min_y || max_x <= min_x {
        return TileGrid::new(width, height);
    }
    let (x0, y0) = (min_x as u32, min_y as u32);
    let (w, h) = ((max_x - min_x) as usize, (max_y - min_y) as usize);
    let mut cov = vec![0.0f32; w * h];
    let n = points.len();
    for row in 0..h {
        for sub in 0..4 {
            let y = f64::from(y0) + row as f64 + (f64::from(sub) + 0.5) / 4.0;
            let mut xs: Vec<f64> = Vec::new();
            for i in 0..n {
                let (p, q) = (points[i], points[(i + 1) % n]);
                if (p.1 <= y) != (q.1 <= y) {
                    xs.push(p.0 + (y - p.1) / (q.1 - p.1) * (q.0 - p.0));
                }
            }
            xs.sort_by(f64::total_cmp);
            for span in xs.chunks_exact(2) {
                let (a, b) = ((span[0] - f64::from(x0)).max(0.0), (span[1] - f64::from(x0)).min(w as f64));
                if b <= a {
                    continue;
                }
                let (c0, c1) = (a.floor() as usize, (b.ceil() as usize).min(w));
                for col in c0..c1 {
                    let overlap = (b.min(col as f64 + 1.0) - a.max(col as f64)).max(0.0);
                    cov[row * w + col] += (overlap / 4.0) as f32;
                }
            }
        }
    }
    mask_from_coverage(width, height, x0, y0, w, h, &cov)
}

fn color_distance(a: [u8; 4], b: [u8; 4]) -> u8 {
    (0..4).map(|i| a[i].abs_diff(b[i])).max().unwrap_or(0)
}

/// Baguette magique : pixels dont la couleur est à moins de `tolerance` (0..=255, écart maximal par canal,
/// alpha compris) de celle du pixel `(x, y)`. `contiguous` : seulement les voisins connectés (4-voisinage).
pub fn magic_wand(src: &TileGrid, x: u32, y: u32, tolerance: u8, contiguous: bool) -> Result<TileGrid, CoreError> {
    let (w, h) = (src.width(), src.height());
    let mut out = TileGrid::new(w, h)?;
    if x >= w || y >= h {
        return Ok(out);
    }
    let target = src.get_pixel(x, y);
    if !contiguous {
        return color_range(src, target, tolerance);
    }
    let mut seen = vec![false; w as usize * h as usize];
    let mut queue = VecDeque::from([(x, y)]);
    seen[(y * w + x) as usize] = true;
    while let Some((cx, cy)) = queue.pop_front() {
        out.put_pixel(cx, cy, px(255));
        let neighbours = [(cx.wrapping_sub(1), cy), (cx + 1, cy), (cx, cy.wrapping_sub(1)), (cx, cy + 1)];
        for (nx, ny) in neighbours {
            if nx >= w || ny >= h {
                continue;
            }
            let i = (ny * w + nx) as usize;
            if !seen[i] {
                seen[i] = true;
                if color_distance(src.get_pixel(nx, ny), target) <= tolerance {
                    queue.push_back((nx, ny));
                }
            }
        }
    }
    Ok(out)
}

/// Plage de couleurs : tous les pixels du calque proches de `color`, où qu'ils soient.
pub fn color_range(src: &TileGrid, color: [u8; 4], tolerance: u8) -> Result<TileGrid, CoreError> {
    let (w, h) = (src.width(), src.height());
    let mut out = TileGrid::new(w, h)?;
    for y in 0..h {
        for x in 0..w {
            if color_distance(src.get_pixel(x, y), color) <= tolerance {
                out.put_pixel(x, y, px(255));
            }
        }
    }
    Ok(out)
}

/// Région de valeurs flottantes (0..1) couvrant la sélection plus une marge, pour les traitements spatiaux.
fn region_of(mask: &TileGrid, margin: u32) -> Option<(Region, u32, u32)> {
    let (bx0, by0, bx1, by1) = bounds(mask)?;
    let (x0, y0) = (bx0.saturating_sub(margin), by0.saturating_sub(margin));
    let (x1, y1) = ((bx1 + margin).min(mask.width()), (by1 + margin).min(mask.height()));
    let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let mut data = Vec::with_capacity(w * h);
    for y in y0..y1 {
        for x in x0..x1 {
            data.push(f32::from(value(mask, x, y)) / 255.0);
        }
    }
    Some((Region { x0: i64::from(x0), y0: i64::from(y0), w, h, data }, x0, y0))
}

fn mask_of(mask: &TileGrid, region: &Region, x0: u32, y0: u32) -> Result<TileGrid, CoreError> {
    mask_from_coverage(mask.width(), mask.height(), x0, y0, region.w, region.h, &region.data)
}

/// Contour progressif : adoucit le bord sur `radius` pixels.
pub fn feather(mask: &TileGrid, radius: f32) -> Result<TileGrid, CoreError> {
    let r = radius.clamp(0.0, 250.0);
    let Some((region, x0, y0)) = region_of(mask, (r * 2.0).ceil() as u32 + 2) else {
        return TileGrid::new(mask.width(), mask.height());
    };
    mask_of(mask, &region.blurred(r), x0, y0)
}

/// Agrandit (`grow`) ou contracte la sélection d'un disque de `radius` pixels.
pub fn grow(mask: &TileGrid, radius: f32, grow: bool) -> Result<TileGrid, CoreError> {
    let r = radius.clamp(0.0, 100.0);
    let Some((region, x0, y0)) = region_of(mask, r.ceil() as u32 + 1) else {
        return TileGrid::new(mask.width(), mask.height());
    };
    mask_of(mask, &region.morph(r, grow), x0, y0)
}

/// Applique `new` à `old` seulement à travers `selection` : `résultat = old + (new − old) × masque`.
/// Les tuiles dont le masque est vide gardent exactement celles de `old` (partage d'`Arc`, donc rien à stocker
/// dans l'historique pour elles).
pub fn through_selection(old: &TileGrid, new: &TileGrid, selection: &TileGrid) -> Result<TileGrid, CoreError> {
    let mut out = old.clone();
    for ty in 0..old.tiles_y() {
        for tx in 0..old.tiles_x() {
            let key = TileKey { tx, ty };
            let Some(sel) = selection.tile(key) else { continue };
            let full = matches!(sel.as_ref(), Tile::Uniform(c) if c[0] == 255);
            if full {
                out.set_tile(key, new.tile(key).cloned());
                continue;
            }
            let mut buf = vec![0u8; (TILE_SIZE * TILE_SIZE * 4) as usize];
            for ly in 0..TILE_SIZE {
                for lx in 0..TILE_SIZE {
                    let (x, y) = (tx * TILE_SIZE + lx, ty * TILE_SIZE + ly);
                    if x >= old.width() || y >= old.height() {
                        continue;
                    }
                    let (o, n, m) = (old.get_pixel(x, y), new.get_pixel(x, y), u32::from(sel.pixel(lx, ly)[0]));
                    let i = ((ly * TILE_SIZE + lx) * 4) as usize;
                    for c in 0..4 {
                        buf[i + c] = ((u32::from(o[c]) * (255 - m) + u32::from(n[c]) * m + 127) / 255) as u8;
                    }
                }
            }
            let tile = Tile::from_pixels(buf)?;
            out.set_tile(key, (tile != Tile::Uniform([0; 4])).then(|| Arc::new(tile)));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(m: &TileGrid, x: u32, y: u32) -> u8 {
        value(m, x, y)
    }

    #[test]
    fn rect_is_pixel_aligned_clipped_and_order_independent() {
        let m = rect(64, 64, (20.2, 30.0), (10.0, 10.0)).unwrap();
        assert_eq!((v(&m, 10, 10), v(&m, 19, 29), v(&m, 20, 10), v(&m, 9, 10)), (255, 255, 0, 0));
        let big = rect(64, 64, (-50.0, -50.0), (500.0, 500.0)).unwrap();
        assert_eq!((v(&big, 0, 0), v(&big, 63, 63)), (255, 255));
        assert_eq!(openpics_core::selection::bounds(&rect(64, 64, (5.0, 5.0), (5.0, 9.0)).unwrap()), None);
    }

    #[test]
    fn ellipse_has_a_solid_core_antialiased_edge_and_clean_outside() {
        let m = ellipse(100, 100, (10.0, 20.0), (90.0, 80.0)).unwrap();
        assert_eq!(v(&m, 50, 50), 255);
        assert_eq!(v(&m, 11, 21), 0, "le coin du rectangle est hors de l'ellipse");
        assert_eq!(v(&m, 5, 50), 0);
        let partial = (0..100)
            .flat_map(|y| (0..100).map(move |x| (x, y)))
            .filter(|&(x, y)| v(&m, x, y) > 0 && v(&m, x, y) < 255)
            .count();
        assert!(partial > 100, "bord anticrénelé : {partial} pixels partiels");
        // Aire ≈ π·a·b (40 × 30).
        let area: f64 =
            (0..100).flat_map(|y| (0..100).map(move |x| (x, y))).map(|(x, y)| f64::from(v(&m, x, y)) / 255.0).sum();
        assert!((area - std::f64::consts::PI * 40.0 * 30.0).abs() < 40.0, "aire {area}");
    }

    #[test]
    fn polygon_fills_inside_with_exact_area_and_even_odd_holes() {
        let tri = polygon(64, 64, &[(10.0, 10.0), (50.0, 10.0), (10.0, 50.0)]).unwrap();
        assert_eq!((v(&tri, 15, 15), v(&tri, 45, 45)), (255, 0));
        let area: f64 =
            (0..64).flat_map(|y| (0..64).map(move |x| (x, y))).map(|(x, y)| f64::from(v(&tri, x, y)) / 255.0).sum();
        assert!((area - 800.0).abs() < 8.0, "aire {area}");
        // Étoile à 5 branches tracée d'un trait : le centre (pentagone) est creux avec la règle pair-impair.
        let star: Vec<(f64, f64)> = (0..5)
            .map(|i| {
                let a = f64::from(i) * 4.0 * std::f64::consts::PI / 5.0 - std::f64::consts::FRAC_PI_2;
                (50.0 + 40.0 * a.cos(), 50.0 + 40.0 * a.sin())
            })
            .collect();
        let s = polygon(100, 100, &star).unwrap();
        assert_eq!(v(&s, 50, 50), 0, "centre creux");
        assert_eq!(v(&s, 50, 18), 255, "branche pleine");
        assert_eq!(polygon(10, 10, &[(1.0, 1.0), (5.0, 5.0)]).unwrap().tile_count(), 0);
        assert_eq!(polygon(10, 10, &[(f64::NAN, 1.0), (5.0, 5.0), (2.0, 9.0)]).unwrap().tile_count(), 0);
    }

    fn two_zones() -> TileGrid {
        let mut g = TileGrid::new(40, 20).unwrap();
        for y in 0..20 {
            for x in 0..40 {
                let c = if (10..15).contains(&x) {
                    [0, 0, 0, 255]
                } else if x < 10 || (15..25).contains(&x) {
                    [200, 0, 0, 255]
                } else {
                    [205, 5, 0, 255]
                };
                g.put_pixel(x, y, c);
            }
        }
        g
    }

    #[test]
    fn wand_respects_tolerance_and_contiguity() {
        let src = two_zones();
        // Zone rouge de gauche (x < 10) : séparée de celle de droite par une bande noire.
        let contiguous = magic_wand(&src, 2, 5, 0, true).unwrap();
        assert_eq!((v(&contiguous, 9, 19), v(&contiguous, 12, 5), v(&contiguous, 16, 5)), (255, 0, 0));
        let anywhere = magic_wand(&src, 2, 5, 0, false).unwrap();
        assert_eq!(v(&anywhere, 16, 5), 255, "non contigu : la zone de droite de même couleur est prise");
        // Tolérance : 205/5 est à 5 de 200/0.
        assert_eq!(v(&magic_wand(&src, 16, 5, 2, true).unwrap(), 30, 5), 0);
        assert_eq!(v(&magic_wand(&src, 16, 5, 5, true).unwrap(), 30, 5), 255);
        assert_eq!(magic_wand(&src, 99, 99, 10, true).unwrap().tile_count(), 0);
    }

    #[test]
    fn color_range_picks_matching_pixels() {
        let src = two_zones();
        let m = color_range(&src, [0, 0, 0, 255], 0).unwrap();
        assert_eq!((v(&m, 12, 3), v(&m, 2, 3)), (255, 0));
    }

    #[test]
    fn feather_softens_and_keeps_the_core() {
        let m = rect(100, 100, (30.0, 30.0), (70.0, 70.0)).unwrap();
        let f = feather(&m, 8.0).unwrap();
        assert_eq!(v(&f, 50, 50), 255);
        let edge = v(&f, 30, 50);
        assert!(edge > 60 && edge < 200, "mi-ombre au bord : {edge}");
        assert!(v(&f, 24, 50) > 0 && v(&f, 24, 50) < edge, "déborde vers l'extérieur");
        assert_eq!(v(&f, 5, 5), 0);
        assert_eq!(feather(&TileGrid::new(10, 10).unwrap(), 5.0).unwrap().tile_count(), 0);
    }

    #[test]
    fn grow_and_shrink_move_the_edge_by_the_radius() {
        let m = rect(100, 100, (30.0, 30.0), (70.0, 70.0)).unwrap();
        let big = grow(&m, 5.0, true).unwrap();
        assert_eq!((v(&big, 25, 50), v(&big, 24, 50)), (255, 0));
        let small = grow(&m, 5.0, false).unwrap();
        assert_eq!((v(&small, 34, 50), v(&small, 35, 50)), (0, 255));
    }

    #[test]
    fn through_selection_only_touches_selected_pixels() {
        let (mut old, mut new) = (TileGrid::new(300, 300).unwrap(), TileGrid::new(300, 300).unwrap());
        for y in 0..300 {
            for x in 0..300 {
                old.put_pixel(x, y, [100, 100, 100, 255]);
                new.put_pixel(x, y, [200, 200, 200, 255]);
            }
        }
        let mut sel = rect(300, 300, (10.0, 10.0), (20.0, 20.0)).unwrap();
        sel.put_pixel(20, 10, [128; 4]); // moitié sélectionné
        let out = through_selection(&old, &new, &sel).unwrap();
        assert_eq!(out.get_pixel(15, 15), [200, 200, 200, 255]);
        assert_eq!(out.get_pixel(5, 5), [100, 100, 100, 255]);
        let half = out.get_pixel(20, 10)[0];
        assert!((148..=152).contains(&half), "{half}");
        // Les tuiles hors sélection sont les mêmes `Arc` qu'avant : rien à retenir dans l'historique.
        assert!(Arc::ptr_eq(out.tile(TileKey { tx: 1, ty: 1 }).unwrap(), old.tile(TileKey { tx: 1, ty: 1 }).unwrap()));
    }
}
