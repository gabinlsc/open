//! Filtres d'image (référence CPU, parallélisés avec rayon) et leur registre.
//!
//! Tous travaillent sur un tampon flottant **prémultiplié** (RGBA 0..1) : un voisin transparent ne ternit
//! donc jamais la couleur d'un bord. Chaque filtre est déclaré une fois dans `REGISTRY` avec ses paramètres
//! bornés : l'interface en génère la galerie et les curseurs, et toute valeur est ramenée dans sa plage.

use openpics_core::adjust::ParamSpec;
use openpics_core::{CoreError, TileGrid};
use rayon::prelude::*;

type Px = [f32; 4];

/// Image flottante prémultipliée.
pub struct Buf {
    pub w: usize,
    pub h: usize,
    pub d: Vec<Px>,
}

impl Buf {
    pub fn from_grid(g: &TileGrid) -> Buf {
        let (w, h) = (g.width() as usize, g.height() as usize);
        let rgba = g.to_rgba8();
        let d = rgba
            .chunks_exact(4)
            .map(|p| {
                let a = f32::from(p[3]) / 255.0;
                [f32::from(p[0]) / 255.0 * a, f32::from(p[1]) / 255.0 * a, f32::from(p[2]) / 255.0 * a, a]
            })
            .collect();
        Buf { w, h, d }
    }

    pub fn into_grid(self) -> Result<TileGrid, CoreError> {
        let mut out = Vec::with_capacity(self.w * self.h * 4);
        for p in &self.d {
            let a = p[3].clamp(0.0, 1.0);
            let un = |c: f32| if a > 1e-6 { ((c / a).clamp(0.0, 1.0) * 255.0 + 0.5) as u8 } else { 0 };
            out.extend_from_slice(&[un(p[0]), un(p[1]), un(p[2]), (a * 255.0 + 0.5) as u8]);
        }
        TileGrid::from_rgba8(self.w as u32, self.h as u32, &out)
    }

    fn at(&self, x: i64, y: i64) -> Px {
        self.d[(y.clamp(0, self.h as i64 - 1) as usize) * self.w + x.clamp(0, self.w as i64 - 1) as usize]
    }

    /// Échantillon bilinéaire (centres de pixels en +0,5).
    pub(crate) fn bilinear(&self, x: f32, y: f32) -> Px {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (a, b, c, d) = (
            self.at(x0 as i64, y0 as i64),
            self.at(x0 as i64 + 1, y0 as i64),
            self.at(x0 as i64, y0 as i64 + 1),
            self.at(x0 as i64 + 1, y0 as i64 + 1),
        );
        std::array::from_fn(|i| (a[i] * (1.0 - tx) + b[i] * tx) * (1.0 - ty) + (c[i] * (1.0 - tx) + d[i] * tx) * ty)
    }

    fn map(&self, f: impl Fn(usize, usize) -> Px + Sync) -> Buf {
        let w = self.w;
        let d =
            (0..self.h).into_par_iter().flat_map_iter(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| f(x, y)).collect();
        Buf { w: self.w, h: self.h, d }
    }
}

fn gaussian_kernel(sigma: f32) -> Vec<f32> {
    let r = (sigma * 3.0).ceil().max(1.0) as i32;
    let mut k: Vec<f32> = (-r..=r).map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp()).collect();
    let s: f32 = k.iter().sum();
    k.iter_mut().for_each(|v| *v /= s);
    k
}

fn convolve(b: &Buf, k: &[f32], horizontal: bool) -> Buf {
    let r = (k.len() / 2) as i64;
    b.map(|x, y| {
        let mut acc = [0.0f32; 4];
        for (i, w) in k.iter().enumerate() {
            let o = i as i64 - r;
            let p = if horizontal { b.at(x as i64 + o, y as i64) } else { b.at(x as i64, y as i64 + o) };
            for (a, v) in acc.iter_mut().zip(p) {
                *a += v * w;
            }
        }
        acc
    })
}

pub fn gaussian_blur(b: &Buf, sigma: f32) -> Buf {
    if sigma < 0.1 {
        return Buf { w: b.w, h: b.h, d: b.d.clone() };
    }
    let k = gaussian_kernel(sigma);
    convolve(&convolve(b, &k, true), &k, false)
}

/// Flou de surface : moyenne pondérée qui ignore les voisins trop différents (les contours restent nets).
pub fn surface_blur(b: &Buf, radius: i64, threshold: f32) -> Buf {
    let sigma_s = (radius as f32 / 2.0).max(0.5);
    let sigma_r = threshold.max(0.005);
    b.map(|x, y| {
        let c = b.at(x as i64, y as i64);
        let mut acc = [0.0f32; 4];
        let mut wsum = 0.0;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let p = b.at(x as i64 + dx, y as i64 + dy);
                let dist2 = (dx * dx + dy * dy) as f32;
                let diff2 = (0..4).map(|i| (p[i] - c[i]).powi(2)).sum::<f32>();
                let w = (-dist2 / (2.0 * sigma_s * sigma_s)).exp() * (-diff2 / (2.0 * sigma_r * sigma_r)).exp();
                for (a, v) in acc.iter_mut().zip(p) {
                    *a += v * w;
                }
                wsum += w;
            }
        }
        acc.map(|v| v / wsum.max(1e-9))
    })
}

/// Flou directionnel : moyenne le long d'un segment centré de longueur `distance` et d'angle `angle_deg`.
pub fn motion_blur(b: &Buf, angle_deg: f32, distance: f32) -> Buf {
    let n = (distance.ceil() as usize).max(1);
    let (dx, dy) = (angle_deg.to_radians().cos(), angle_deg.to_radians().sin());
    b.map(|x, y| {
        let mut acc = [0.0f32; 4];
        for i in 0..=n {
            let t = (i as f32 / n as f32 - 0.5) * distance;
            let p = b.bilinear(x as f32 + 0.5 + dx * t, y as f32 + 0.5 + dy * t);
            for (a, v) in acc.iter_mut().zip(p) {
                *a += v;
            }
        }
        acc.map(|v| v / (n + 1) as f32)
    })
}

/// Flou radial : rotation (`spin`) ou zoom autour de `(cx, cy)` (en fraction de la taille de l'image).
pub fn radial_blur(b: &Buf, spin: bool, amount: f32, cx: f32, cy: f32) -> Buf {
    let (ox, oy) = (cx * b.w as f32, cy * b.h as f32);
    let steps = 16;
    b.map(|x, y| {
        let (px, py) = (x as f32 + 0.5 - ox, y as f32 + 0.5 - oy);
        let mut acc = [0.0f32; 4];
        for i in 0..steps {
            let t = (i as f32 / (steps - 1) as f32 - 0.5) * amount;
            let (sx, sy) = if spin {
                let a = t.to_radians();
                (px * a.cos() - py * a.sin(), px * a.sin() + py * a.cos())
            } else {
                let k = 1.0 + t / 100.0;
                (px * k, py * k)
            };
            let p = b.bilinear(ox + sx, oy + sy);
            for (a, v) in acc.iter_mut().zip(p) {
                *a += v;
            }
        }
        acc.map(|v| v / steps as f32)
    })
}

/// Flou de l'objectif : moyenne sur un disque ; `bloom` accentue les hautes lumières (bokeh).
pub fn lens_blur(b: &Buf, radius: i64, bloom: f32) -> Buf {
    let r2 = (radius * radius) as f32 + 0.5;
    b.map(|x, y| {
        let mut acc = [0.0f32; 4];
        let mut wsum = 0.0;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if (dx * dx + dy * dy) as f32 > r2 {
                    continue;
                }
                let p = b.at(x as i64 + dx, y as i64 + dy);
                let w = 1.0 + bloom * (0.3 * p[0] + 0.59 * p[1] + 0.11 * p[2]).powi(2) * 8.0;
                for (a, v) in acc.iter_mut().zip(p) {
                    *a += v * w;
                }
                wsum += w;
            }
        }
        acc.map(|v| v / wsum)
    })
}

/// Masque flou : `net = original + (original − flou) × quantité`, seulement où la différence dépasse `threshold`.
pub fn unsharp_mask(b: &Buf, amount: f32, sigma: f32, threshold: f32) -> Buf {
    let blurred = gaussian_blur(b, sigma);
    b.map(|x, y| {
        let (o, l) = (b.d[y * b.w + x], blurred.d[y * b.w + x]);
        let diff = [o[0] - l[0], o[1] - l[1], o[2] - l[2]];
        if diff.iter().map(|v| v.abs()).fold(0.0, f32::max) < threshold {
            return o;
        }
        [o[0] + diff[0] * amount, o[1] + diff[1] * amount, o[2] + diff[2] * amount, o[3]]
    })
}

/// Accentuation optimisée : comme le masque flou, mais atténuée dans les zones très uniformes (le bruit n'est pas
/// amplifié) et plafonnée près des contours (pas de halos).
pub fn smart_sharpen(b: &Buf, amount: f32, sigma: f32, reduce_noise: f32) -> Buf {
    let blurred = gaussian_blur(b, sigma);
    b.map(|x, y| {
        let (o, l) = (b.d[y * b.w + x], blurred.d[y * b.w + x]);
        let mag = (0..3).map(|i| (o[i] - l[i]).abs()).fold(0.0, f32::max);
        let gate = (mag / (reduce_noise * 0.05 + 1e-4)).min(1.0);
        std::array::from_fn(|i| if i == 3 { o[3] } else { (o[i] + (o[i] - l[i]) * amount * gate).clamp(0.0, o[3]) })
    })
}

fn hash2(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA6B) ^ seed.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Ajout de bruit (uniforme ou approximativement gaussien), reproductible pour une graine donnée.
pub fn add_noise(b: &Buf, amount: f32, gaussian: bool, monochrome: bool, seed: u32) -> Buf {
    b.map(|x, y| {
        let p = b.d[y * b.w + x];
        let n = |ch: u32| {
            let u = hash2(x as u32, y as u32, seed.wrapping_add(ch * 7919));
            if gaussian {
                // Somme de trois uniformes : cloche approchée, centrée, d'écart-type ≈ 0,5.
                let v = (u + hash2(y as u32, x as u32, seed ^ 0x55 ^ ch) + hash2(x as u32 ^ 0xAB, y as u32, seed + ch))
                    / 3.0
                    - 0.5;
                v * 2.0 * amount
            } else {
                (u - 0.5) * 2.0 * amount
            }
        };
        let (nr, ng, nb) = if monochrome {
            let v = n(0);
            (v, v, v)
        } else {
            (n(0), n(1), n(2))
        };
        [
            (p[0] + nr * p[3]).clamp(0.0, p[3]),
            (p[1] + ng * p[3]).clamp(0.0, p[3]),
            (p[2] + nb * p[3]).clamp(0.0, p[3]),
            p[3],
        ]
    })
}

/// Réduction du bruit : médiane `(2r+1)²` par canal, mélangée à l'original selon `strength`.
pub fn reduce_noise(b: &Buf, radius: i64, strength: f32) -> Buf {
    b.map(|x, y| {
        let o = b.d[y * b.w + x];
        let mut out = o;
        for (c, v) in out.iter_mut().enumerate() {
            let mut win: Vec<f32> = Vec::with_capacity(((2 * radius + 1) * (2 * radius + 1)) as usize);
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    win.push(b.at(x as i64 + dx, y as i64 + dy)[c]);
                }
            }
            win.sort_by(f32::total_cmp);
            *v = o[c] + (win[win.len() / 2] - o[c]) * strength;
        }
        out
    })
}

/// Remappage géométrique : chaque pixel de sortie lit la source en `map(x, y)` (None = transparent).
fn remap(b: &Buf, map: impl Fn(f32, f32) -> Option<(f32, f32)> + Sync) -> Buf {
    b.map(|x, y| match map(x as f32 + 0.5, y as f32 + 0.5) {
        Some((sx, sy)) if sx >= 0.0 && sy >= 0.0 && sx <= b.w as f32 && sy <= b.h as f32 => b.bilinear(sx, sy),
        _ => [0.0; 4],
    })
}

pub fn wave(b: &Buf, amplitude: f32, wavelength: f32, vertical_phase: bool) -> Buf {
    let k = std::f32::consts::TAU / wavelength.max(2.0);
    remap(b, |x, y| {
        Some(if vertical_phase { (x + amplitude * (y * k).sin(), y) } else { (x, y + amplitude * (x * k).sin()) })
    })
}

/// Pincement (`amount` > 0 : vers le centre ; < 0 : vers l'extérieur) dans un disque inscrit à l'image.
pub fn pinch(b: &Buf, amount: f32) -> Buf {
    let (cx, cy) = (b.w as f32 / 2.0, b.h as f32 / 2.0);
    let rad = cx.min(cy);
    remap(b, |x, y| {
        let (dx, dy) = (x - cx, y - cy);
        let d = dx.hypot(dy);
        if d >= rad || d < 1e-6 {
            return Some((x, y));
        }
        let t = d / rad;
        let k = (1.0 - t).powi(2) * amount;
        Some((cx + dx * (1.0 - k), cy + dy * (1.0 - k)))
    })
}

/// Sphérisation : bombe (`amount` > 0) ou creuse l'image dans un disque inscrit.
pub fn spherize(b: &Buf, amount: f32) -> Buf {
    let (cx, cy) = (b.w as f32 / 2.0, b.h as f32 / 2.0);
    let rad = cx.min(cy);
    remap(b, |x, y| {
        let (dx, dy) = (x - cx, y - cy);
        let d = dx.hypot(dy);
        if d >= rad || d < 1e-6 {
            return Some((x, y));
        }
        let t = d / rad;
        // Loi en puissance : rayon source = t^e. e > 1 (bombé) lit plus près du centre, donc agrandit le centre ;
        // e < 1 (creusé) fait l'inverse. e = 1 est l'identité, et le centre comme le bord restent fixes.
        let a = amount.clamp(-0.95, 0.95);
        let e = if a >= 0.0 { 1.0 + a } else { 1.0 / (1.0 - a) };
        let k = t.powf(e - 1.0);
        Some((cx + dx * k, cy + dy * k))
    })
}

fn luma(p: Px) -> f32 {
    0.3 * p[0] + 0.59 * p[1] + 0.11 * p[2]
}

/// Relief : gris moyen modulé par la pente de luminance dans la direction `angle_deg`.
pub fn emboss(b: &Buf, angle_deg: f32, height: f32) -> Buf {
    let (dx, dy) = (angle_deg.to_radians().cos(), -angle_deg.to_radians().sin());
    b.map(|x, y| {
        let p = b.d[y * b.w + x];
        let (a, c) = (
            b.bilinear(x as f32 + 0.5 + dx, y as f32 + 0.5 + dy),
            b.bilinear(x as f32 + 0.5 - dx, y as f32 + 0.5 - dy),
        );
        let v = (0.5 + (luma(a) - luma(c)) * height).clamp(0.0, 1.0);
        [v * p[3], v * p[3], v * p[3], p[3]]
    })
}

/// Contours : magnitude de Sobel, affichée en sombre sur fond clair.
pub fn find_edges(b: &Buf) -> Buf {
    b.map(|x, y| {
        let l = |dx: i64, dy: i64| luma(b.at(x as i64 + dx, y as i64 + dy));
        let gx = -l(-1, -1) - 2.0 * l(-1, 0) - l(-1, 1) + l(1, -1) + 2.0 * l(1, 0) + l(1, 1);
        let gy = -l(-1, -1) - 2.0 * l(0, -1) - l(1, -1) + l(-1, 1) + 2.0 * l(0, 1) + l(1, 1);
        let v = (1.0 - gx.hypot(gy)).clamp(0.0, 1.0);
        let a = b.d[y * b.w + x][3];
        [v * a, v * a, v * a, a]
    })
}

/// Opération par pixel sur la couleur droite (non prémultipliée).
fn per_pixel(b: &Buf, f: impl Fn([f32; 3]) -> [f32; 3] + Sync) -> Buf {
    b.map(|x, y| {
        let p = b.d[y * b.w + x];
        if p[3] <= 1e-6 {
            return p;
        }
        let c = f([p[0] / p[3], p[1] / p[3], p[2] / p[3]]);
        [c[0] * p[3], c[1] * p[3], c[2] * p[3], p[3]]
    })
}

pub fn posterize(b: &Buf, levels: u32) -> Buf {
    let n = (levels.max(2) - 1) as f32;
    per_pixel(b, |c| c.map(|v| (v * n).round() / n))
}

pub fn threshold(b: &Buf, level: f32) -> Buf {
    per_pixel(b, |c| {
        let v = if 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2] >= level { 1.0 } else { 0.0 };
        [v; 3]
    })
}

pub fn solarize(b: &Buf, level: f32) -> Buf {
    per_pixel(b, |c| c.map(|v| if v >= level { 1.0 - v } else { v }))
}

/// Mosaïque : chaque case de `cell` pixels prend la couleur moyenne de ses pixels.
pub fn mosaic(b: &Buf, cell: usize) -> Buf {
    let cell = cell.max(1);
    let (cw, ch) = (b.w.div_ceil(cell), b.h.div_ceil(cell));
    let means: Vec<Px> = (0..cw * ch)
        .into_par_iter()
        .map(|i| {
            let (cx, cy) = ((i % cw) * cell, (i / cw) * cell);
            let (mut acc, mut n) = ([0.0f32; 4], 0.0);
            for y in cy..(cy + cell).min(b.h) {
                for x in cx..(cx + cell).min(b.w) {
                    let p = b.d[y * b.w + x];
                    for (a, v) in acc.iter_mut().zip(p) {
                        *a += v;
                    }
                    n += 1.0;
                }
            }
            acc.map(|v| v / n)
        })
        .collect();
    b.map(|x, y| means[(y / cell) * cw + x / cell])
}

/// Cristallisation : cellules de Voronoï (un germe par case, décalé au hasard), colorées par leur moyenne.
pub fn crystallize(b: &Buf, cell: usize, seed: u32) -> Buf {
    let cell = cell.max(2);
    let (cw, ch) = (b.w.div_ceil(cell) + 2, b.h.div_ceil(cell) + 2);
    let seeds: Vec<(f32, f32)> = (0..cw * ch)
        .map(|i| {
            let (gx, gy) = ((i % cw) as i64 - 1, (i / cw) as i64 - 1);
            (
                (gx as f32 + hash2(gx as u32, gy as u32, seed)) * cell as f32,
                (gy as f32 + hash2(gy as u32, gx as u32, seed ^ 0x77)) * cell as f32,
            )
        })
        .collect();
    let nearest = |x: usize, y: usize| -> usize {
        let (gx, gy) = ((x / cell + 1) as i64, (y / cell + 1) as i64);
        let mut best = (f32::MAX, 0);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (nx, ny) = ((gx + dx).clamp(0, cw as i64 - 1) as usize, (gy + dy).clamp(0, ch as i64 - 1) as usize);
                let s = seeds[ny * cw + nx];
                let d = (s.0 - x as f32 - 0.5).powi(2) + (s.1 - y as f32 - 0.5).powi(2);
                if d < best.0 {
                    best = (d, ny * cw + nx);
                }
            }
        }
        best.1
    };
    let mut sums = vec![([0.0f32; 4], 0.0f32); cw * ch];
    for y in 0..b.h {
        for x in 0..b.w {
            let s = &mut sums[nearest(x, y)];
            let p = b.d[y * b.w + x];
            for (a, v) in s.0.iter_mut().zip(p) {
                *a += v;
            }
            s.1 += 1.0;
        }
    }
    b.map(|x, y| {
        let (acc, n) = sums[nearest(x, y)];
        acc.map(|v| v / n.max(1.0))
    })
}

fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (tx, ty) = (x - xi, y - yi);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let v = |dx: i32, dy: i32| hash2((xi as i32 + dx) as u32, (yi as i32 + dy) as u32, seed);
    let (a, b) = (v(0, 0) + (v(1, 0) - v(0, 0)) * s(tx), v(0, 1) + (v(1, 1) - v(0, 1)) * s(tx));
    a + (b - a) * s(ty)
}

/// Nuages : bruit fractal entre deux couleurs ; remplace le contenu (l'alpha d'origine est conservé).
pub fn clouds(b: &Buf, scale: f32, octaves: u32, seed: u32, fg: [f32; 3], bg: [f32; 3]) -> Buf {
    let base = 1.0 / scale.max(4.0);
    b.map(|x, y| {
        let (mut v, mut amp, mut freq, mut norm) = (0.0, 1.0, base, 0.0);
        for o in 0..octaves.clamp(1, 8) {
            v += value_noise(x as f32 * freq, y as f32 * freq, seed.wrapping_add(o * 131)) * amp;
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        let t = v / norm;
        // Les nuages remplissent entièrement le calque (ou la sélection, via `through_selection`).
        std::array::from_fn(|i| if i == 3 { 1.0 } else { bg[i] + (fg[i] - bg[i]) * t })
    })
}

/// Éclairage : lumière ponctuelle douce centrée en `(cx, cy)` (fractions), rayon en fraction de la diagonale.
pub fn lighting(b: &Buf, cx: f32, cy: f32, radius: f32, intensity: f32, ambient: f32) -> Buf {
    let (ox, oy) = (cx * b.w as f32, cy * b.h as f32);
    let r = radius.max(0.01) * (b.w as f32).hypot(b.h as f32);
    b.map(|x, y| {
        let p = b.d[y * b.w + x];
        let d = ((x as f32 + 0.5 - ox).powi(2) + (y as f32 + 0.5 - oy).powi(2)).sqrt() / r;
        let light = ambient + intensity * (1.0 - d.min(1.0)).powi(2);
        [p[0] * light, p[1] * light, p[2] * light, p[3]]
    })
}

// ---------------------------------------------------------------------------------------------------------------------
// Registre

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Blur,
    Sharpen,
    Noise,
    Distort,
    Stylize,
    Pixelate,
    Render,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Blur => "Flou",
            Category::Sharpen => "Netteté",
            Category::Noise => "Bruit",
            Category::Distort => "Déformation",
            Category::Stylize => "Stylisation",
            Category::Pixelate => "Pixellisation",
            Category::Render => "Rendu",
        }
    }
}

pub struct FilterDef {
    pub id: &'static str,
    pub label: &'static str,
    pub category: Category,
    pub params: &'static [ParamSpec],
    run: fn(&Buf, &[f32]) -> Buf,
}

const fn ps(id: &'static str, label: &'static str, min: f32, max: f32, default: f32) -> ParamSpec {
    ParamSpec { id, label, min, max, default }
}

pub static REGISTRY: &[FilterDef] = &[
    FilterDef {
        id: "gaussian_blur",
        label: "Flou gaussien",
        category: Category::Blur,
        params: &[ps("radius", "Rayon (px)", 0.0, 100.0, 5.0)],
        run: |b, v| gaussian_blur(b, v[0]),
    },
    FilterDef {
        id: "surface_blur",
        label: "Flou de surface",
        category: Category::Blur,
        params: &[ps("radius", "Rayon (px)", 1.0, 15.0, 4.0), ps("threshold", "Seuil", 0.01, 1.0, 0.15)],
        run: |b, v| surface_blur(b, v[0].round() as i64, v[1]),
    },
    FilterDef {
        id: "motion_blur",
        label: "Flou directionnel",
        category: Category::Blur,
        params: &[ps("angle", "Angle (°)", -180.0, 180.0, 0.0), ps("distance", "Distance (px)", 1.0, 200.0, 20.0)],
        run: |b, v| motion_blur(b, v[0], v[1]),
    },
    FilterDef {
        id: "spin_blur",
        label: "Flou radial (rotation)",
        category: Category::Blur,
        params: &[
            ps("amount", "Angle (°)", 0.0, 90.0, 10.0),
            ps("cx", "Centre X", 0.0, 1.0, 0.5),
            ps("cy", "Centre Y", 0.0, 1.0, 0.5),
        ],
        run: |b, v| radial_blur(b, true, v[0], v[1], v[2]),
    },
    FilterDef {
        id: "zoom_blur",
        label: "Flou radial (zoom)",
        category: Category::Blur,
        params: &[
            ps("amount", "Quantité (%)", 0.0, 100.0, 20.0),
            ps("cx", "Centre X", 0.0, 1.0, 0.5),
            ps("cy", "Centre Y", 0.0, 1.0, 0.5),
        ],
        run: |b, v| radial_blur(b, false, v[0], v[1], v[2]),
    },
    FilterDef {
        id: "lens_blur",
        label: "Flou de l'objectif",
        category: Category::Blur,
        params: &[ps("radius", "Rayon (px)", 1.0, 25.0, 6.0), ps("bloom", "Éclat des hautes lumières", 0.0, 1.0, 0.3)],
        run: |b, v| lens_blur(b, v[0].round() as i64, v[1]),
    },
    FilterDef {
        id: "unsharp_mask",
        label: "Masque flou",
        category: Category::Sharpen,
        params: &[
            ps("amount", "Quantité", 0.0, 5.0, 1.0),
            ps("radius", "Rayon (px)", 0.1, 50.0, 2.0),
            ps("threshold", "Seuil", 0.0, 0.5, 0.0),
        ],
        run: |b, v| unsharp_mask(b, v[0], v[1], v[2]),
    },
    FilterDef {
        id: "smart_sharpen",
        label: "Accentuation optimisée",
        category: Category::Sharpen,
        params: &[
            ps("amount", "Quantité", 0.0, 5.0, 1.5),
            ps("radius", "Rayon (px)", 0.1, 20.0, 1.5),
            ps("reduce_noise", "Réduction du bruit", 0.0, 1.0, 0.3),
        ],
        run: |b, v| smart_sharpen(b, v[0], v[1], v[2]),
    },
    FilterDef {
        id: "add_noise",
        label: "Ajout de bruit",
        category: Category::Noise,
        params: &[
            ps("amount", "Quantité", 0.0, 1.0, 0.1),
            ps("gaussian", "Gaussien (0/1)", 0.0, 1.0, 1.0),
            ps("monochrome", "Monochrome (0/1)", 0.0, 1.0, 0.0),
            ps("seed", "Graine", 0.0, 9999.0, 1.0),
        ],
        run: |b, v| add_noise(b, v[0], v[1] >= 0.5, v[2] >= 0.5, v[3] as u32),
    },
    FilterDef {
        id: "reduce_noise",
        label: "Réduction du bruit (médiane)",
        category: Category::Noise,
        params: &[ps("radius", "Rayon (px)", 1.0, 5.0, 1.0), ps("strength", "Force", 0.0, 1.0, 1.0)],
        run: |b, v| reduce_noise(b, v[0].round() as i64, v[1]),
    },
    FilterDef {
        id: "wave",
        label: "Vague",
        category: Category::Distort,
        params: &[
            ps("amplitude", "Amplitude (px)", 0.0, 200.0, 10.0),
            ps("wavelength", "Longueur d'onde (px)", 4.0, 500.0, 60.0),
            ps("vertical", "Ondulation verticale (0/1)", 0.0, 1.0, 0.0),
        ],
        run: |b, v| wave(b, v[0], v[1], v[2] >= 0.5),
    },
    FilterDef {
        id: "pinch",
        label: "Pincement",
        category: Category::Distort,
        params: &[ps("amount", "Quantité", -1.0, 1.0, 0.5)],
        run: |b, v| pinch(b, v[0]),
    },
    FilterDef {
        id: "spherize",
        label: "Sphérisation",
        category: Category::Distort,
        params: &[ps("amount", "Quantité", -1.0, 1.0, 0.5)],
        run: |b, v| spherize(b, v[0]),
    },
    FilterDef {
        id: "emboss",
        label: "Relief",
        category: Category::Stylize,
        params: &[ps("angle", "Angle (°)", -180.0, 180.0, 135.0), ps("height", "Hauteur", 0.0, 10.0, 2.0)],
        run: |b, v| emboss(b, v[0], v[1]),
    },
    FilterDef {
        id: "find_edges",
        label: "Contours",
        category: Category::Stylize,
        params: &[],
        run: |b, _| find_edges(b),
    },
    FilterDef {
        id: "posterize",
        label: "Isohélie",
        category: Category::Stylize,
        params: &[ps("levels", "Niveaux", 2.0, 32.0, 4.0)],
        run: |b, v| posterize(b, v[0].round() as u32),
    },
    FilterDef {
        id: "threshold",
        label: "Seuil",
        category: Category::Stylize,
        params: &[ps("level", "Niveau", 0.0, 1.0, 0.5)],
        run: |b, v| threshold(b, v[0]),
    },
    FilterDef {
        id: "solarize",
        label: "Solarisation",
        category: Category::Stylize,
        params: &[ps("level", "Niveau", 0.0, 1.0, 0.5)],
        run: |b, v| solarize(b, v[0]),
    },
    FilterDef {
        id: "mosaic",
        label: "Mosaïque",
        category: Category::Pixelate,
        params: &[ps("cell", "Taille de case (px)", 2.0, 200.0, 12.0)],
        run: |b, v| mosaic(b, v[0].round() as usize),
    },
    FilterDef {
        id: "crystallize",
        label: "Cristallisation",
        category: Category::Pixelate,
        params: &[ps("cell", "Taille de cellule (px)", 3.0, 200.0, 20.0), ps("seed", "Graine", 0.0, 9999.0, 1.0)],
        run: |b, v| crystallize(b, v[0].round() as usize, v[1] as u32),
    },
    FilterDef {
        id: "clouds",
        label: "Nuages",
        category: Category::Render,
        params: &[
            ps("scale", "Échelle (px)", 4.0, 1000.0, 120.0),
            ps("octaves", "Détail", 1.0, 8.0, 5.0),
            ps("seed", "Graine", 0.0, 9999.0, 1.0),
            ps("fg", "Clair (0–1)", 0.0, 1.0, 1.0),
            ps("bg", "Sombre (0–1)", 0.0, 1.0, 0.0),
        ],
        run: |b, v| clouds(b, v[0], v[1] as u32, v[2] as u32, [v[3]; 3], [v[4]; 3]),
    },
    FilterDef {
        id: "lighting",
        label: "Éclairage",
        category: Category::Render,
        params: &[
            ps("cx", "Position X", 0.0, 1.0, 0.5),
            ps("cy", "Position Y", 0.0, 1.0, 0.5),
            ps("radius", "Rayon", 0.05, 1.5, 0.5),
            ps("intensity", "Intensité", 0.0, 3.0, 1.0),
            ps("ambient", "Ambiance", 0.0, 1.0, 0.4),
        ],
        run: |b, v| lighting(b, v[0], v[1], v[2], v[3], v[4]),
    },
];

pub fn find(id: &str) -> Option<&'static FilterDef> {
    REGISTRY.iter().find(|f| f.id == id)
}

impl FilterDef {
    /// Valeurs complétées par les défauts et bornées à la plage de chaque paramètre.
    pub fn clamp_values(&self, values: &[f32]) -> Vec<f32> {
        self.params
            .iter()
            .enumerate()
            .map(|(i, s)| values.get(i).copied().filter(|v| v.is_finite()).map_or(s.default, |v| v.clamp(s.min, s.max)))
            .collect()
    }

    pub fn defaults(&self) -> Vec<f32> {
        self.params.iter().map(|s| s.default).collect()
    }

    pub fn apply(&self, src: &TileGrid, values: &[f32]) -> Result<TileGrid, CoreError> {
        (self.run)(&Buf::from_grid(src), &self.clamp_values(values)).into_grid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> TileGrid {
        let mut g = TileGrid::new(w, h).unwrap();
        for y in 0..h {
            for x in 0..w {
                g.put_pixel(x, y, f(x, y));
            }
        }
        g
    }

    fn run(id: &str, g: &TileGrid, values: &[f32]) -> TileGrid {
        find(id).unwrap().apply(g, values).unwrap()
    }

    fn flat(v: u8) -> TileGrid {
        filled(40, 40, |_, _| [v, v, v, 255])
    }

    fn edge() -> TileGrid {
        filled(60, 40, |x, _| if x < 30 { [40, 40, 40, 255] } else { [220, 220, 220, 255] })
    }

    fn variance(g: &TileGrid) -> f32 {
        let v: Vec<f32> = (0..g.height())
            .flat_map(|y| (0..g.width()).map(move |x| (x, y)))
            .map(|(x, y)| f32::from(g.get_pixel(x, y)[0]))
            .collect();
        let m = v.iter().sum::<f32>() / v.len() as f32;
        v.iter().map(|a| (a - m).powi(2)).sum::<f32>() / v.len() as f32
    }

    fn noisy() -> TileGrid {
        filled(40, 40, |x, y| {
            let v = if (x * 7 + y * 13) % 5 == 0 { 255 } else { 90 };
            [v, v, v, 255]
        })
    }

    #[test]
    fn registry_is_consistent_and_every_filter_runs_with_defaults_and_hostile_values() {
        let ids: std::collections::HashSet<_> = REGISTRY.iter().map(|f| f.id).collect();
        assert_eq!(ids.len(), REGISTRY.len(), "identifiants uniques");
        let g = filled(48, 32, |x, y| [(x * 5) as u8, (y * 7) as u8, 128, 255]);
        for f in REGISTRY {
            assert!(!f.label.is_empty() && !f.category.label().is_empty());
            let out = f.apply(&g, &f.defaults()).unwrap();
            assert_eq!((out.width(), out.height()), (48, 32), "{}", f.id);
            let wild: Vec<f32> = f.params.iter().map(|_| f32::NAN).collect();
            assert!(f.apply(&g, &wild).is_ok(), "{} avec NaN", f.id);
            let huge: Vec<f32> = f.params.iter().map(|_| 1e12).collect();
            let clamped = f.clamp_values(&huge);
            assert!(clamped.iter().zip(f.params).all(|(v, s)| *v <= s.max && *v >= s.min), "{}", f.id);
        }
        assert!(find("n'existe_pas").is_none());
    }

    #[test]
    fn gaussian_blur_keeps_flat_images_and_mean_while_reducing_variance() {
        assert_eq!(run("gaussian_blur", &flat(100), &[8.0]).get_pixel(20, 20), [100, 100, 100, 255]);
        assert_eq!(run("gaussian_blur", &noisy(), &[0.0]).to_rgba8(), noisy().to_rgba8(), "rayon nul = identité");
        let b = run("gaussian_blur", &noisy(), &[2.0]);
        assert!(variance(&b) < variance(&noisy()) * 0.3);
        let mean = |g: &TileGrid| {
            (10..30)
                .flat_map(|y| (10..30).map(move |x| (x, y)))
                .map(|(x, y)| f32::from(g.get_pixel(x, y)[0]))
                .sum::<f32>()
                / 400.0
        };
        assert!((mean(&b) - mean(&noisy())).abs() < 6.0, "moyenne conservée");
        let soft = run("gaussian_blur", &edge(), &[4.0]);
        assert!(soft.get_pixel(28, 20)[0] > 40 && soft.get_pixel(32, 20)[0] < 220, "le bord s'adoucit");
    }

    #[test]
    fn transparent_neighbours_do_not_darken_blurred_edges() {
        let g = filled(40, 40, |x, y| {
            if (10..30).contains(&x) && (10..30).contains(&y) { [255, 255, 255, 255] } else { [0, 0, 0, 0] }
        });
        let b = run("gaussian_blur", &g, &[3.0]);
        let p = b.get_pixel(10, 20);
        assert!(p[3] > 0 && p[3] < 255 && p[0] >= 250, "bord semi-transparent mais toujours blanc : {p:?}");
    }

    #[test]
    fn surface_blur_preserves_edges_better_than_gaussian() {
        let n = filled(60, 40, |x, y| {
            let base = if x < 30 { 40 } else { 220 };
            let v = base + if (x + y) % 2 == 0 { 6 } else { 0 };
            [v, v, v, 255]
        });
        let s = run("surface_blur", &n, &[4.0, 0.1]);
        let g = run("gaussian_blur", &n, &[3.0]);
        let step = |t: &TileGrid| i32::from(t.get_pixel(32, 20)[0]) - i32::from(t.get_pixel(27, 20)[0]);
        assert!(step(&s) > step(&g) + 40, "contour conservé : {} vs {}", step(&s), step(&g));
        assert!(variance(&run("surface_blur", &flat(100), &[4.0, 0.1])) < 1e-3);
    }

    #[test]
    fn directional_radial_and_lens_blurs() {
        let dot = filled(60, 60, |x, y| if (x, y) == (30, 30) { [255, 255, 255, 255] } else { [0, 0, 0, 255] });
        let m = run("motion_blur", &dot, &[0.0, 20.0]);
        assert!(m.get_pixel(38, 30)[0] > 5 && m.get_pixel(30, 38)[0] == 0, "étalé horizontalement seulement");
        let lens = run("lens_blur", &dot, &[5.0, 0.0]);
        assert!(lens.get_pixel(33, 30)[0] > 0 && lens.get_pixel(30, 33)[0] > 0 && lens.get_pixel(40, 40)[0] == 0);
        let zoom = run("zoom_blur", &edge(), &[40.0, 0.5, 0.5]);
        assert_eq!(
            zoom.get_pixel(30, 20)[0] / 8,
            run("zoom_blur", &edge(), &[40.0, 0.5, 0.5]).get_pixel(30, 20)[0] / 8
        );
        let spin_centre = run("spin_blur", &flat(77), &[30.0, 0.5, 0.5]);
        assert_eq!(spin_centre.get_pixel(5, 5), [77, 77, 77, 255]);
    }

    #[test]
    fn sharpening_raises_edge_contrast_and_leaves_flat_areas_alone() {
        let soft = run("gaussian_blur", &edge(), &[2.0]);
        let sharp = run("unsharp_mask", &soft, &[2.0, 3.0, 0.0]);
        assert!(
            sharp.get_pixel(24, 20)[0] < soft.get_pixel(24, 20)[0]
                && sharp.get_pixel(35, 20)[0] > soft.get_pixel(35, 20)[0]
        );
        assert_eq!(run("unsharp_mask", &flat(120), &[3.0, 3.0, 0.0]).get_pixel(20, 20), [120, 120, 120, 255]);
        let thresholded = run("unsharp_mask", &soft, &[2.0, 3.0, 0.5]);
        assert_eq!(thresholded.to_rgba8(), soft.to_rgba8(), "un seuil élevé n'affûte rien");
        let smart = run("smart_sharpen", &soft, &[2.0, 3.0, 0.0]);
        assert!(smart.get_pixel(24, 20)[0] <= soft.get_pixel(24, 20)[0]);
        let calm = run("smart_sharpen", &flat(120), &[3.0, 2.0, 1.0]);
        assert_eq!(calm.get_pixel(5, 5), [120, 120, 120, 255]);
    }

    #[test]
    fn noise_is_reproducible_bounded_and_the_median_removes_salt_and_pepper() {
        let a = run("add_noise", &flat(128), &[0.2, 1.0, 0.0, 7.0]);
        assert_eq!(
            a.to_rgba8(),
            run("add_noise", &flat(128), &[0.2, 1.0, 0.0, 7.0]).to_rgba8(),
            "même graine, même bruit"
        );
        assert_ne!(a.to_rgba8(), run("add_noise", &flat(128), &[0.2, 1.0, 0.0, 8.0]).to_rgba8());
        assert!(variance(&a) > 10.0);
        let mean: f32 = (0..40)
            .flat_map(|y| (0..40).map(move |x| (x, y)))
            .map(|(x, y)| f32::from(a.get_pixel(x, y)[0]))
            .sum::<f32>()
            / 1600.0;
        assert!((mean - 128.0).abs() < 4.0, "le bruit est centré : {mean}");
        let mono = run("add_noise", &flat(128), &[0.3, 0.0, 1.0, 1.0]);
        let p = mono.get_pixel(10, 10);
        assert!(p[0] == p[1] && p[1] == p[2], "monochrome");
        assert_eq!(run("add_noise", &flat(255), &[1.0, 0.0, 0.0, 1.0]).get_pixel(0, 0)[3], 255);
        let mut salty = flat(100);
        for (x, y) in [(10, 10), (20, 20), (30, 5)] {
            salty.put_pixel(x, y, [255, 255, 255, 255]);
        }
        let clean = run("reduce_noise", &salty, &[1.0, 1.0]);
        assert!(
            clean.get_pixel(10, 10)[0] < 105 && clean.get_pixel(20, 20)[0] < 105,
            "les points isolés disparaissent"
        );
    }

    #[test]
    fn distortions_are_identity_at_zero_and_keep_the_centre() {
        let g = filled(60, 60, |x, y| [(x * 4) as u8, (y * 4) as u8, 9, 255]);
        for id in ["pinch", "spherize"] {
            assert_eq!(run(id, &g, &[0.0]).to_rgba8(), g.to_rgba8(), "{id} à 0");
            let warped = run(id, &g, &[0.8]);
            assert_ne!(warped.to_rgba8(), g.to_rgba8());
            let (c, o) = (warped.get_pixel(30, 30), g.get_pixel(30, 30));
            assert!((i32::from(c[0]) - i32::from(o[0])).abs() <= 6, "{id} : centre conservé");
            assert_eq!(warped.get_pixel(0, 0), g.get_pixel(0, 0), "{id} : hors du disque inscrit, inchangé");
        }
        assert_ne!(
            run("pinch", &g, &[0.8]).get_pixel(20, 30),
            run("pinch", &g, &[-0.8]).get_pixel(20, 30),
            "signe opposé = effet opposé"
        );
        assert_eq!(run("wave", &g, &[0.0, 30.0, 0.0]).to_rgba8(), g.to_rgba8());
        let w = run("wave", &g, &[8.0, 40.0, 0.0]);
        assert_ne!(w.to_rgba8(), g.to_rgba8());
    }

    #[test]
    fn stylize_filters() {
        assert_eq!(
            run("emboss", &flat(200), &[135.0, 2.0]).get_pixel(20, 20),
            [128, 128, 128, 255],
            "aplat → gris moyen"
        );
        let e = run("emboss", &edge(), &[0.0, 2.0]);
        assert_ne!(e.get_pixel(30, 20)[0], 128, "le bord ressort");
        assert_eq!(run("find_edges", &flat(30), &[]).get_pixel(20, 20), [255, 255, 255, 255], "aplat → blanc");
        assert!(run("find_edges", &edge(), &[]).get_pixel(30, 20)[0] < 100, "bord → sombre");
        let p = run(
            "posterize",
            &filled(32, 1, |x, _| {
                let v = (x * 8) as u8;
                [v, v, v, 255]
            }),
            &[4.0],
        );
        let levels: std::collections::BTreeSet<u8> = (0..32).map(|x| p.get_pixel(x, 0)[0]).collect();
        assert_eq!(levels.len(), 4);
        let t = run("threshold", &edge(), &[0.5]);
        assert_eq!((t.get_pixel(10, 5)[0], t.get_pixel(50, 5)[0]), (0, 255));
        assert_eq!(
            run(
                "solarize",
                &filled(2, 1, |x, _| if x == 0 { [50, 50, 50, 255] } else { [200, 200, 200, 255] }),
                &[0.5]
            )
            .get_pixel(1, 0)[0],
            55
        );
        // Les pixels transparents restent transparents.
        assert_eq!(run("threshold", &TileGrid::new(8, 8).unwrap(), &[0.5]).tile_count(), 0);
    }

    #[test]
    fn mosaic_and_crystallize_make_uniform_cells() {
        let g = filled(40, 40, |x, y| [(x * 6) as u8, (y * 6) as u8, 0, 255]);
        let m = run("mosaic", &g, &[10.0]);
        for (x, y) in [(0, 0), (9, 9), (10, 3), (39, 39)] {
            let cell = (x / 10 * 10, y / 10 * 10);
            assert_eq!(m.get_pixel(x, y), m.get_pixel(cell.0, cell.1));
        }
        assert_ne!(m.get_pixel(0, 0), m.get_pixel(30, 30));
        let c = run("crystallize", &g, &[12.0, 3.0]);
        assert_eq!(c.to_rgba8(), run("crystallize", &g, &[12.0, 3.0]).to_rgba8(), "reproductible");
        let distinct: std::collections::HashSet<_> =
            (0..40).flat_map(|y| (0..40).map(move |x| (x, y))).map(|(x, y)| c.get_pixel(x, y)).collect();
        assert!(distinct.len() > 3 && distinct.len() < 60, "{} cellules", distinct.len());
    }

    #[test]
    fn clouds_and_lighting() {
        let empty = TileGrid::new(64, 64).unwrap();
        let c = run("clouds", &empty, &[30.0, 4.0, 5.0, 1.0, 0.0]);
        assert_eq!(c.to_rgba8(), run("clouds", &empty, &[30.0, 4.0, 5.0, 1.0, 0.0]).to_rgba8());
        assert_ne!(c.to_rgba8(), run("clouds", &empty, &[30.0, 4.0, 6.0, 1.0, 0.0]).to_rgba8());
        let vals: Vec<u8> =
            (0..64).flat_map(|y| (0..64).map(move |x| (x, y))).map(|(x, y)| c.get_pixel(x, y)[0]).collect();
        assert!(vals.iter().max().unwrap() - vals.iter().min().unwrap() > 40, "contrasté");
        assert_eq!(c.get_pixel(3, 3)[3], 255, "les nuages remplissent le calque");
        let l = run("lighting", &flat(100), &[0.5, 0.5, 0.5, 1.0, 0.3]);
        assert!(l.get_pixel(20, 20)[0] > l.get_pixel(0, 0)[0], "plus clair au centre");
        assert_eq!(l.get_pixel(0, 0)[0], 30, "ambiance seule hors du rayon");
    }
}
