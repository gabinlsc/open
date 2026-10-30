//! Transformation libre d'un calque : matrice 3×3 (translation, rotation, échelle, cisaillement,
//! perspective) et déformation par grille. Rééchantillonnage bilinéaire en alpha prémultiplié, pour
//! ne pas faire apparaître de liseré sombre sur les bords semi-transparents.

use std::sync::Arc;

use openpics_core::{CoreError, TILE_SIZE, Tile, TileGrid, TileKey};
use rayon::prelude::*;

/// Matrice homogène 3×3 : `(x', y', w) = M · (x, y, 1)`, puis `(x'/w, y'/w)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix3(pub [[f64; 3]; 3]);

impl Matrix3 {
    pub const IDENTITY: Matrix3 = Matrix3([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

    pub fn translate(dx: f64, dy: f64) -> Self {
        Matrix3([[1.0, 0.0, dx], [0.0, 1.0, dy], [0.0, 0.0, 1.0]])
    }

    pub fn scale(sx: f64, sy: f64) -> Self {
        Matrix3([[sx, 0.0, 0.0], [0.0, sy, 0.0], [0.0, 0.0, 1.0]])
    }

    /// Rotation de `deg` degrés dans le sens horaire à l'écran (l'axe y pointe vers le bas).
    pub fn rotate(deg: f64) -> Self {
        let (s, c) = deg.to_radians().sin_cos();
        Matrix3([[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]])
    }

    /// `self` appliquée après `other` : `(self * other)(p) = self(other(p))`.
    pub fn then_after(&self, other: &Matrix3) -> Matrix3 {
        let mut out = [[0.0; 3]; 3];
        for (i, row) in out.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|k| self.0[i][k] * other.0[k][j]).sum();
            }
        }
        Matrix3(out)
    }

    /// Fait agir `self` autour du point `(cx, cy)` au lieu de l'origine.
    pub fn about(&self, cx: f64, cy: f64) -> Matrix3 {
        Matrix3::translate(cx, cy).then_after(self).then_after(&Matrix3::translate(-cx, -cy))
    }

    pub fn apply(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let m = &self.0;
        let w = m[2][0] * x + m[2][1] * y + m[2][2];
        if w.abs() < 1e-12 || !w.is_finite() {
            return None;
        }
        Some(((m[0][0] * x + m[0][1] * y + m[0][2]) / w, (m[1][0] * x + m[1][1] * y + m[1][2]) / w))
    }

    /// `None` si la matrice n'est pas inversible (par exemple une échelle nulle).
    pub fn inverse(&self) -> Option<Matrix3> {
        let m = &self.0;
        let c = |r1: usize, c1: usize, r2: usize, c2: usize| m[r1][c1] * m[r2][c2] - m[r1][c2] * m[r2][c1];
        let det = m[0][0] * c(1, 1, 2, 2) - m[0][1] * c(1, 0, 2, 2) + m[0][2] * c(1, 0, 2, 1);
        if det.abs() < 1e-12 || !det.is_finite() {
            return None;
        }
        let inv = [
            [c(1, 1, 2, 2), -c(0, 1, 2, 2), c(0, 1, 1, 2)],
            [-c(1, 0, 2, 2), c(0, 0, 2, 2), -c(0, 0, 1, 2)],
            [c(1, 0, 2, 1), -c(0, 0, 2, 1), c(0, 0, 1, 1)],
        ];
        Some(Matrix3(inv.map(|r| r.map(|v| v / det))))
    }

    /// Perspective : envoie le rectangle `(0,0)-(w,h)` sur le quadrilatère `quad`
    /// (haut-gauche, haut-droite, bas-droite, bas-gauche). `None` si le quadrilatère est dégénéré.
    pub fn perspective(w: f64, h: f64, quad: [(f64, f64); 4]) -> Option<Matrix3> {
        let src = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)];
        // Système 8×8 : pour chaque coin, deux équations en (a..h) avec i = 1.
        let mut a = [[0.0f64; 9]; 8];
        for (i, ((x, y), (u, v))) in src.iter().zip(quad.iter()).enumerate() {
            a[2 * i] = [*x, *y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, *u];
            a[2 * i + 1] = [0.0, 0.0, 0.0, *x, *y, 1.0, -v * x, -v * y, *v];
        }
        let s = solve8(a)?;
        Some(Matrix3([[s[0], s[1], s[2]], [s[3], s[4], s[5]], [s[6], s[7], 1.0]]))
    }
}

/// Élimination de Gauss avec pivot partiel sur un système 8×8 augmenté.
fn solve8(mut a: [[f64; 9]; 8]) -> Option<[f64; 8]> {
    for col in 0..8 {
        let pivot = (col..8).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-10 {
            return None;
        }
        a.swap(col, pivot);
        let pivot_row = a[col];
        for (row, line) in a.iter_mut().enumerate() {
            if row != col {
                let f = line[col] / pivot_row[col];
                for (v, p) in line[col..].iter_mut().zip(&pivot_row[col..]) {
                    *v -= f * p;
                }
            }
        }
    }
    Some(std::array::from_fn(|i| a[i][8] / a[i][i]))
}

/// Échantillon bilinéaire en alpha prémultiplié ; hors du calque = transparent.
fn sample(src: &TileGrid, x: f64, y: f64) -> [f32; 4] {
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = ((fx - x0) as f32, (fy - y0) as f32);
    let mut acc = [0.0f32; 4];
    for (dy, wy) in [(0.0, 1.0 - ty), (1.0, ty)] {
        for (dx, wx) in [(0.0, 1.0 - tx), (1.0, tx)] {
            let (px, py) = (x0 + dx, y0 + dy);
            if px < 0.0 || py < 0.0 {
                continue;
            }
            let p = src.get_pixel(px as u32, py as u32);
            let a = f32::from(p[3]) / 255.0;
            let w = wx * wy;
            acc[0] += f32::from(p[0]) / 255.0 * a * w;
            acc[1] += f32::from(p[1]) / 255.0 * a * w;
            acc[2] += f32::from(p[2]) / 255.0 * a * w;
            acc[3] += a * w;
        }
    }
    acc
}

fn to_pixel(p: [f32; 4]) -> [u8; 4] {
    let a = p[3].clamp(0.0, 1.0);
    if a <= 0.0 {
        return [0; 4];
    }
    let c = |v: f32| ((v / a).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    [c(p[0]), c(p[1]), c(p[2]), (a * 255.0 + 0.5) as u8]
}

/// Remplit chaque pixel de destination avec `fetch(x, y)` (centre de pixel) ; les tuiles vides restent absentes.
fn resample(src: &TileGrid, fetch: impl Fn(f64, f64) -> Option<(f64, f64)> + Sync) -> Result<TileGrid, CoreError> {
    let (w, h) = (src.width(), src.height());
    let keys: Vec<TileKey> =
        (0..src.tiles_y()).flat_map(|ty| (0..src.tiles_x()).map(move |tx| TileKey { tx, ty })).collect();
    let built: Vec<(TileKey, Option<Tile>)> = keys
        .par_iter()
        .map(|&key| {
            let mut buf = vec![0u8; (TILE_SIZE * TILE_SIZE * 4) as usize];
            let mut any = false;
            let (x0, y0) = (key.tx * TILE_SIZE, key.ty * TILE_SIZE);
            for ly in 0..TILE_SIZE.min(h - y0) {
                for lx in 0..TILE_SIZE.min(w - x0) {
                    let Some((sx, sy)) = fetch(f64::from(x0 + lx) + 0.5, f64::from(y0 + ly) + 0.5) else { continue };
                    let px = to_pixel(sample(src, sx, sy));
                    if px[3] > 0 {
                        any = true;
                        let i = ((ly * TILE_SIZE + lx) * 4) as usize;
                        buf[i..i + 4].copy_from_slice(&px);
                    }
                }
            }
            (key, if any { Tile::from_pixels(buf).ok() } else { None })
        })
        .collect();
    let mut out = TileGrid::new(w, h)?;
    for (key, tile) in built {
        out.set_tile(key, tile.map(Arc::new));
    }
    Ok(out)
}

/// Applique `m` (source → destination). La taille du calque ne change pas : ce qui sort du cadre est perdu.
pub fn transform(src: &TileGrid, m: &Matrix3) -> Result<TileGrid, CoreError> {
    let inv =
        m.inverse().ok_or_else(|| CoreError::InvalidTransform("la transformation n'est pas inversible".into()))?;
    if src.tile_count() == 0 {
        return TileGrid::new(src.width(), src.height());
    }
    resample(src, |x, y| inv.apply(x, y))
}

/// Déformation par grille : `offsets` (colonnes × lignes points de contrôle, ligne par ligne) donne, en
/// pixels, d'où lire la source pour un pixel situé au point de contrôle. Interpolé bilinéairement entre
/// les points ; 0 partout = aucune déformation.
pub fn warp(src: &TileGrid, cols: usize, rows: usize, offsets: &[(f64, f64)]) -> Result<TileGrid, CoreError> {
    if cols < 2
        || rows < 2
        || cols > 64
        || rows > 64
        || offsets.len() != cols * rows
        || offsets.iter().any(|(x, y)| !x.is_finite() || !y.is_finite())
    {
        return Err(CoreError::InvalidTransform(format!(
            "grille de déformation invalide ({cols}×{rows}, {} points)",
            offsets.len()
        )));
    }
    if src.tile_count() == 0 {
        return TileGrid::new(src.width(), src.height());
    }
    let (w, h) = (f64::from(src.width()), f64::from(src.height()));
    resample(src, |x, y| {
        let (gx, gy) = ((x / w).clamp(0.0, 1.0) * (cols - 1) as f64, (y / h).clamp(0.0, 1.0) * (rows - 1) as f64);
        let (c0, r0) = ((gx.floor() as usize).min(cols - 2), (gy.floor() as usize).min(rows - 2));
        let (fx, fy) = (gx - c0 as f64, gy - r0 as f64);
        let at = |c: usize, r: usize| offsets[r * cols + c];
        let mix = |f: fn((f64, f64)) -> f64| {
            let top = f(at(c0, r0)) * (1.0 - fx) + f(at(c0 + 1, r0)) * fx;
            let bot = f(at(c0, r0 + 1)) * (1.0 - fx) + f(at(c0 + 1, r0 + 1)) * fx;
            top * (1.0 - fy) + bot * fy
        };
        Some((x + mix(|o| o.0), y + mix(|o| o.1)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> TileGrid {
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&f(x, y));
            }
        }
        TileGrid::from_rgba8(w, h, &rgba).unwrap()
    }

    fn marked() -> TileGrid {
        grid(64, 64, |x, y| if (x, y) == (10, 20) { [200, 50, 10, 255] } else { [0, 0, 0, 0] })
    }

    #[test]
    fn matrix_inverse_and_composition() {
        let m = Matrix3::rotate(30.0).about(10.0, 20.0).then_after(&Matrix3::translate(3.0, -4.0));
        let id = m.then_after(&m.inverse().unwrap());
        for i in 0..3 {
            for j in 0..3 {
                assert!((id.0[i][j] - Matrix3::IDENTITY.0[i][j]).abs() < 1e-9, "{id:?}");
            }
        }
        assert!(Matrix3::scale(0.0, 1.0).inverse().is_none());
        let (x, y) = Matrix3::rotate(90.0).about(32.0, 32.0).apply(40.0, 32.0).unwrap();
        assert!((x - 32.0).abs() < 1e-9 && (y - 40.0).abs() < 1e-9, "({x},{y})");
    }

    #[test]
    fn identity_and_translation_are_exact() {
        let src = marked();
        assert_eq!(transform(&src, &Matrix3::IDENTITY).unwrap().to_rgba8(), src.to_rgba8());
        let moved = transform(&src, &Matrix3::translate(7.0, 5.0)).unwrap();
        assert_eq!(moved.get_pixel(17, 25), [200, 50, 10, 255]);
        assert_eq!(moved.get_pixel(10, 20), [0; 4]);
    }

    #[test]
    fn rotation_by_90_degrees_moves_pixels_to_the_expected_cell() {
        let r = transform(&marked(), &Matrix3::rotate(90.0).about(32.0, 32.0)).unwrap();
        assert_eq!(r.get_pixel(43, 10), [200, 50, 10, 255]);
    }

    #[test]
    fn scaling_up_covers_a_block() {
        let src = grid(64, 64, |x, y| if x < 32 && y < 32 { [10, 20, 30, 255] } else { [0; 4] });
        let big = transform(&src, &Matrix3::scale(2.0, 2.0)).unwrap();
        assert_eq!(big.get_pixel(40, 40), [10, 20, 30, 255]);
        assert_eq!(big.get_pixel(60, 60), [10, 20, 30, 255]);
        assert!(big.get_pixel(63, 63)[3] < 255, "le bord est anti-aliasé");
        let small = transform(&src, &Matrix3::scale(0.5, 0.5)).unwrap();
        assert_eq!(small.get_pixel(10, 10), [10, 20, 30, 255]);
        assert_eq!(small.get_pixel(20, 20), [0; 4]);
    }

    #[test]
    fn rotated_edges_have_no_dark_fringe() {
        let src = grid(64, 64, |x, y| {
            if (16..48).contains(&x) && (16..48).contains(&y) { [255, 255, 255, 255] } else { [0, 0, 0, 0] }
        });
        let r = transform(&src, &Matrix3::rotate(45.0).about(32.0, 32.0)).unwrap();
        let mut edge = 0;
        for y in 0..64 {
            for x in 0..64 {
                let p = r.get_pixel(x, y);
                if p[3] > 0 {
                    assert!(p[0] >= 250 && p[1] >= 250 && p[2] >= 250, "liseré en ({x},{y}) : {p:?}");
                    if p[3] < 255 {
                        edge += 1;
                    }
                }
            }
        }
        assert!(edge > 20, "il doit y avoir des pixels de bord semi-transparents");
    }

    #[test]
    fn perspective_maps_the_corners_and_degenerates_safely() {
        let quad = [(10.0, 5.0), (90.0, 15.0), (80.0, 70.0), (5.0, 60.0)];
        let m = Matrix3::perspective(100.0, 80.0, quad).unwrap();
        for ((x, y), (u, v)) in [(0.0, 0.0), (100.0, 0.0), (100.0, 80.0), (0.0, 80.0)].into_iter().zip(quad) {
            let (a, b) = m.apply(x, y).unwrap();
            assert!((a - u).abs() < 1e-6 && (b - v).abs() < 1e-6, "({x},{y}) → ({a},{b}) au lieu de ({u},{v})");
        }
        let id = Matrix3::perspective(10.0, 10.0, [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]).unwrap();
        assert!((id.0[0][0] - 1.0).abs() < 1e-9 && id.0[2][0].abs() < 1e-9);
        assert!(Matrix3::perspective(10.0, 10.0, [(0.0, 0.0); 4]).is_none());
        // Une perspective réelle donne un résultat sans panique ni calcul invalide.
        let src = grid(100, 80, |_, _| [1, 2, 3, 255]);
        assert!(transform(&src, &m).unwrap().tile_count() > 0);
    }

    #[test]
    fn singular_transform_is_an_error() {
        assert!(matches!(transform(&marked(), &Matrix3::scale(0.0, 0.0)), Err(CoreError::InvalidTransform(_))));
    }

    #[test]
    fn warp_zero_is_identity_and_constant_offset_translates() {
        let src = marked();
        let zero = vec![(0.0, 0.0); 9];
        assert_eq!(warp(&src, 3, 3, &zero).unwrap().to_rgba8(), src.to_rgba8());
        // Lire 5 px plus à gauche/en haut = déplacer le contenu de 5 px vers la droite/le bas.
        let shift = vec![(-5.0, -5.0); 9];
        assert_eq!(warp(&src, 3, 3, &shift).unwrap().get_pixel(15, 25), [200, 50, 10, 255]);
        assert!(warp(&src, 1, 3, &zero).is_err());
        assert!(warp(&src, 3, 3, &zero[..8]).is_err());
        let mut nan = zero.clone();
        nan[4] = (f64::NAN, 0.0);
        assert!(warp(&src, 3, 3, &nan).is_err());
    }

    #[test]
    fn empty_layer_stays_empty() {
        let empty = TileGrid::new(300, 300).unwrap();
        assert_eq!(transform(&empty, &Matrix3::rotate(10.0)).unwrap().tile_count(), 0);
    }
}
