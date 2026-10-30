//! Remplissages : dégradés (cinq formes) et pot de peinture. Chaque fonction renvoie un nouveau calque ;
//! l'appelant applique la sélection (`selection::through_selection`) et enregistre l'historique.

use std::f64::consts::PI;
use std::sync::Arc;

use openpics_core::blend::{BlendMode, composite};
use openpics_core::{CoreError, TILE_SIZE, Tile, TileGrid, TileKey};

use crate::selection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientKind {
    Linear,
    Radial,
    Angular,
    Reflected,
    Diamond,
}

/// Un arrêt de dégradé : position 0..=1 et couleur RGBA (alpha droit).
pub type Stop = (f32, [u8; 4]);

/// Couleur interpolée aux arrêts (triés par position). Deux arrêts au minimum ; sinon couleur unie/transparente.
fn sample_stops(stops: &[Stop], t: f32) -> [f32; 4] {
    let c = |p: [u8; 4]| [f32::from(p[0]), f32::from(p[1]), f32::from(p[2]), f32::from(p[3])];
    match stops {
        [] => [0.0; 4],
        [only] => c(only.1),
        _ => {
            if t <= stops[0].0 {
                return c(stops[0].1);
            }
            for w in stops.windows(2) {
                if t <= w[1].0 {
                    let span = (w[1].0 - w[0].0).max(1e-6);
                    let k = ((t - w[0].0) / span).clamp(0.0, 1.0);
                    let (a, b) = (c(w[0].1), c(w[1].1));
                    return std::array::from_fn(|i| a[i] + (b[i] - a[i]) * k);
                }
            }
            c(stops[stops.len() - 1].1)
        }
    }
}

/// Paramètre `t` (0..1) d'un point pour un dégradé de `p0` vers `p1`.
fn gradient_t(kind: GradientKind, p0: (f64, f64), p1: (f64, f64), x: f64, y: f64) -> f64 {
    let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
    let len2 = (dx * dx + dy * dy).max(1e-9);
    let len = len2.sqrt();
    let (rx, ry) = (x - p0.0, y - p0.1);
    match kind {
        GradientKind::Linear => (rx * dx + ry * dy) / len2,
        GradientKind::Radial => rx.hypot(ry) / len,
        GradientKind::Angular => {
            let a = ry.atan2(rx) - dy.atan2(dx);
            a.rem_euclid(2.0 * PI) / (2.0 * PI)
        }
        GradientKind::Reflected => ((rx * dx + ry * dy) / len2).abs(),
        GradientKind::Diamond => {
            // Coordonnées dans le repère de l'axe : la distance « Manhattan » trace un losange.
            let (u, v) = ((rx * dx + ry * dy) / len, (-rx * dy + ry * dx) / len);
            (u.abs() + v.abs()) / len
        }
    }
}

/// Peint un dégradé sur `src` (mode normal, opacité `opacity`), de `p0` vers `p1`.
pub fn gradient(
    src: &TileGrid,
    kind: GradientKind,
    p0: (f64, f64),
    p1: (f64, f64),
    stops: &[Stop],
    opacity: f32,
) -> Result<TileGrid, CoreError> {
    if ![p0.0, p0.1, p1.0, p1.1].iter().all(|v| v.is_finite()) {
        return Ok(src.clone());
    }
    let opacity = if opacity.is_finite() { opacity.clamp(0.0, 1.0) } else { 1.0 };
    let mut out = TileGrid::new(src.width(), src.height())?;
    for ty in 0..src.tiles_y() {
        for tx in 0..src.tiles_x() {
            let mut buf = vec![0u8; (TILE_SIZE * TILE_SIZE * 4) as usize];
            let (ox, oy) = (tx * TILE_SIZE, ty * TILE_SIZE);
            for ly in 0..TILE_SIZE.min(src.height() - oy) {
                for lx in 0..TILE_SIZE.min(src.width() - ox) {
                    let (x, y) = (ox + lx, oy + ly);
                    let t = gradient_t(kind, p0, p1, f64::from(x) + 0.5, f64::from(y) + 0.5).clamp(0.0, 1.0) as f32;
                    let s = sample_stops(stops, t);
                    let b = src.get_pixel(x, y);
                    let back = [
                        f32::from(b[0]) / 255.0,
                        f32::from(b[1]) / 255.0,
                        f32::from(b[2]) / 255.0,
                        f32::from(b[3]) / 255.0,
                    ];
                    let p = composite(
                        BlendMode::Normal,
                        back,
                        [s[0] / 255.0, s[1] / 255.0, s[2] / 255.0],
                        s[3] / 255.0 * opacity,
                        x,
                        y,
                    );
                    let i = ((ly * TILE_SIZE + lx) * 4) as usize;
                    for c in 0..4 {
                        buf[i + c] = (p[c].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    }
                }
            }
            let tile = Tile::from_pixels(buf)?;
            out.set_tile(TileKey { tx, ty }, (tile != Tile::Uniform([0; 4])).then(|| Arc::new(tile)));
        }
    }
    Ok(out)
}

/// Réglages du pot de peinture.
#[derive(Debug, Clone, Copy)]
pub struct Bucket {
    /// Pixel cliqué.
    pub x: u32,
    pub y: u32,
    /// 0..=255 : écart de couleur accepté.
    pub tolerance: u8,
    pub contiguous: bool,
    pub color: [u8; 3],
    pub opacity: f32,
}

/// Pot de peinture : remplit la zone de couleur proche du pixel cliqué (`reference` donne les couleurs de
/// comparaison, souvent le calque lui-même) avec la couleur et l'opacité de `b`.
pub fn bucket_fill(src: &TileGrid, reference: &TileGrid, b: Bucket) -> Result<TileGrid, CoreError> {
    let region = selection::magic_wand(reference, b.x, b.y, b.tolerance, b.contiguous)?;
    let opacity = if b.opacity.is_finite() { b.opacity.clamp(0.0, 1.0) } else { 1.0 };
    let color = b.color;
    let mut out = src.clone();
    for (key, tile) in region.iter() {
        let (ox, oy) = (key.tx * TILE_SIZE, key.ty * TILE_SIZE);
        for ly in 0..TILE_SIZE.min(src.height() - oy) {
            for lx in 0..TILE_SIZE.min(src.width() - ox) {
                if tile.pixel(lx, ly)[0] == 0 {
                    continue;
                }
                let (px, py) = (ox + lx, oy + ly);
                let back_px = src.get_pixel(px, py);
                let back = back_px.map(|c| f32::from(c) / 255.0);
                let c = [f32::from(color[0]) / 255.0, f32::from(color[1]) / 255.0, f32::from(color[2]) / 255.0];
                let p = composite(BlendMode::Normal, back, c, opacity, px, py);
                out.put_pixel(px, py, [0, 1, 2, 3].map(|i| (p[i].clamp(0.0, 1.0) * 255.0 + 0.5) as u8));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BW: [Stop; 2] = [(0.0, [0, 0, 0, 255]), (1.0, [255, 255, 255, 255])];

    fn blank(w: u32, h: u32) -> TileGrid {
        TileGrid::new(w, h).unwrap()
    }

    fn v(g: &TileGrid, x: u32, y: u32) -> u8 {
        g.get_pixel(x, y)[0]
    }

    #[test]
    fn linear_runs_from_first_to_last_stop_and_clamps() {
        let g = gradient(&blank(200, 20), GradientKind::Linear, (50.0, 0.0), (150.0, 0.0), &BW, 1.0).unwrap();
        assert_eq!((v(&g, 10, 5), v(&g, 190, 5)), (0, 255));
        assert!((i32::from(v(&g, 100, 5)) - 128).abs() <= 2, "milieu au centre du pixel");
        assert!(v(&g, 70, 5) < v(&g, 80, 5), "croissant");
    }

    #[test]
    fn radial_is_symmetric_around_the_start() {
        let g = gradient(&blank(100, 100), GradientKind::Radial, (50.0, 50.0), (90.0, 50.0), &BW, 1.0).unwrap();
        assert!(v(&g, 50, 50) < 8, "presque noir au centre");
        assert_eq!(v(&g, 70, 50), v(&g, 50, 70));
        assert_eq!(v(&g, 95, 50), 255);
    }

    #[test]
    fn angular_wraps_around_and_reflected_mirrors() {
        let g = gradient(&blank(100, 100), GradientKind::Angular, (50.0, 50.0), (90.0, 50.0), &BW, 1.0).unwrap();
        // Juste au-dessus de l'axe (angle ≈ 360°) : presque blanc ; juste en dessous (angle ≈ 0°) : presque noir.
        assert!(v(&g, 90, 49) > 240 && v(&g, 90, 51) < 15, "{} {}", v(&g, 90, 49), v(&g, 90, 51));
        assert!((i32::from(v(&g, 10, 50)) - 128).abs() < 10, "180° → milieu");
        let r = gradient(&blank(200, 20), GradientKind::Reflected, (100.0, 0.0), (150.0, 0.0), &BW, 1.0).unwrap();
        assert_eq!(v(&r, 59, 5), v(&r, 140, 5), "symétrique autour de x = 100 (centres de pixels)");
        assert!(v(&r, 100, 5) < 5 && v(&r, 190, 5) == 255);
    }

    #[test]
    fn diamond_contours_are_diamonds() {
        let g = gradient(&blank(100, 100), GradientKind::Diamond, (50.0, 50.0), (80.0, 50.0), &BW, 1.0).unwrap();
        // Mêmes valeurs sur |dx| + |dy| constant.
        assert!((i32::from(v(&g, 60, 50)) - i32::from(v(&g, 50, 60))).abs() <= 2);
        assert!((i32::from(v(&g, 55, 55)) - i32::from(v(&g, 60, 50))).abs() <= 3);
        assert!(v(&g, 55, 55) > v(&g, 52, 52));
    }

    #[test]
    fn transparency_stops_and_opacity_blend_with_the_layer() {
        let mut base = blank(100, 10);
        for x in 0..100 {
            base.put_pixel(x, 5, [255, 0, 0, 255]);
        }
        let fade = [(0.0, [0, 0, 255, 255]), (1.0, [0, 0, 255, 0])];
        let g = gradient(&base, GradientKind::Linear, (0.0, 0.0), (100.0, 0.0), &fade, 1.0).unwrap();
        assert!(g.get_pixel(2, 5)[2] > 240 && g.get_pixel(97, 5)[0] > 240, "bleu au début, rouge visible à la fin");
        let half = gradient(&base, GradientKind::Linear, (0.0, 0.0), (100.0, 0.0), &BW, 0.5).unwrap();
        assert!(half.get_pixel(0, 5)[0] > 100 && half.get_pixel(0, 5)[0] < 160, "mélange à 50 %");
        assert_eq!(
            gradient(&base, GradientKind::Linear, (f64::NAN, 0.0), (1.0, 1.0), &BW, 1.0).unwrap().to_rgba8(),
            base.to_rgba8()
        );
        assert!(
            gradient(&base, GradientKind::Linear, (5.0, 5.0), (5.0, 5.0), &BW, 1.0).is_ok(),
            "axe de longueur nulle toléré"
        );
    }

    #[test]
    fn bucket_fills_the_contiguous_region_only() {
        let mut g = blank(60, 20);
        for y in 0..20 {
            for x in 0..60 {
                g.put_pixel(x, y, if x == 30 { [0, 0, 0, 255] } else { [255, 255, 255, 255] });
            }
        }
        let f = bucket_fill(
            &g,
            &g,
            Bucket { x: 5, y: 5, tolerance: 10, contiguous: true, color: [255, 0, 0], opacity: 1.0 },
        )
        .unwrap();
        assert_eq!(
            (f.get_pixel(5, 5), f.get_pixel(29, 19), f.get_pixel(30, 5), f.get_pixel(40, 5)),
            ([255, 0, 0, 255], [255, 0, 0, 255], [0, 0, 0, 255], [255; 4])
        );
        let all = bucket_fill(
            &g,
            &g,
            Bucket { x: 5, y: 5, tolerance: 10, contiguous: false, color: [255, 0, 0], opacity: 0.5 },
        )
        .unwrap();
        assert_eq!(all.get_pixel(40, 5)[0], 255);
        assert!(all.get_pixel(40, 5)[1] > 100 && all.get_pixel(40, 5)[1] < 160, "opacité 50 %");
        assert_eq!(
            bucket_fill(&g, &g, Bucket { x: 999, y: 5, tolerance: 10, contiguous: true, color: [0; 3], opacity: 1.0 })
                .unwrap()
                .to_rgba8(),
            g.to_rgba8()
        );
    }
}
