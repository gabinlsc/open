//! Méthodes sans modèle.
//!
//! - `subject_mask` : estime le fond à partir des bords de l'image, puis retient comme fond ce qui lui ressemble
//!   **et** touche le bord. Fiable sur fond uni ou peu texturé ; sur une scène complexe, il faut un vrai modèle.
//! - `inpaint` : comble une zone par diffusion (moyenne des voisins connus, puis relaxation). Convient aux
//!   poussières, fils, petits objets ; il ne « invente » aucune texture.

use std::collections::VecDeque;

use openpics_core::TileGrid;
use openpics_ops::filters::Buf;

use crate::catalog::AiError;

type Rgb = [f32; 3];

fn dist(a: Rgb, b: Rgb) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() / 3f32.sqrt()
}

fn straight(p: [f32; 4]) -> Rgb {
    if p[3] > 1e-6 { [p[0] / p[3], p[1] / p[3], p[2] / p[3]] } else { [0.0; 3] }
}

/// K-means (k ≤ 4) sur les couleurs du bord : le fond peut avoir plusieurs teintes (dégradé, ciel).
fn background_centers(samples: &[Rgb]) -> Vec<Rgb> {
    let mut centers: Vec<Rgb> = Vec::new();
    for s in samples.iter().step_by((samples.len() / 4).max(1)).take(4) {
        if centers.iter().all(|c| dist(*c, *s) > 0.05) {
            centers.push(*s);
        }
    }
    if centers.is_empty() {
        return vec![[0.0; 3]];
    }
    for _ in 0..8 {
        let mut sums = vec![([0.0f32; 3], 0usize); centers.len()];
        for s in samples {
            let k = nearest(&centers, *s).0;
            for (acc, v) in sums[k].0.iter_mut().zip(s) {
                *acc += v;
            }
            sums[k].1 += 1;
        }
        for (c, (sum, n)) in centers.iter_mut().zip(sums) {
            if n > 0 {
                *c = [sum[0] / n as f32, sum[1] / n as f32, sum[2] / n as f32];
            }
        }
    }
    centers
}

fn nearest(centers: &[Rgb], p: Rgb) -> (usize, f32) {
    centers.iter().enumerate().map(|(i, c)| (i, dist(*c, p))).fold((0, f32::MAX), |a, b| if b.1 < a.1 { b } else { a })
}

/// Masque de sélection du sujet (255 = sujet). `tolerance` ∈ [0,1] : écart de couleur toléré pour le fond.
pub fn subject_mask(src: &TileGrid, tolerance: f32) -> Result<TileGrid, AiError> {
    let buf = Buf::from_grid(src);
    let (w, h) = (buf.w, buf.h);
    let tol = tolerance.clamp(0.01, 1.0);
    let ring = (w.min(h) / 100).max(1);
    let on_border = |x: usize, y: usize| x < ring || y < ring || x >= w - ring.min(w) || y >= h - ring.min(h);
    let mut samples = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let p = buf.d[y * w + x];
            if on_border(x, y) && p[3] > 0.5 {
                samples.push(straight(p));
            }
        }
    }
    let centers = background_centers(&samples);
    // Écart au fond de chaque pixel ; un pixel transparent est du fond d'office.
    let score: Vec<f32> =
        buf.d.iter().map(|p| if p[3] < 0.5 { 0.0 } else { nearest(&centers, straight(*p)).1 }).collect();
    let hi = tol * 1.5;
    let mut is_bg = vec![false; w * h];
    let mut queue = VecDeque::new();
    for y in 0..h {
        for x in 0..w {
            if on_border(x, y) && score[y * w + x] <= hi {
                is_bg[y * w + x] = true;
                queue.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            let i = ny as usize * w + nx as usize;
            if !is_bg[i] && score[i] <= hi {
                is_bg[i] = true;
                queue.push_back((nx as usize, ny as usize));
            }
        }
    }
    let mut rgba = Vec::with_capacity(w * h * 4);
    for i in 0..w * h {
        let v = if is_bg[i] {
            // Lisse la transition entre « sûrement fond » (≤ tol) et « fond tout juste toléré » (≤ 1,5 tol).
            let t = ((score[i] - tol) / (hi - tol)).clamp(0.0, 1.0);
            (t * t * (3.0 - 2.0 * t) * 255.0 + 0.5) as u8
        } else {
            255
        };
        rgba.extend_from_slice(&[v, v, v, v]);
    }
    Ok(TileGrid::from_rgba8(w as u32, h as u32, &rgba)?)
}

/// Comble les pixels dont le masque vaut ≥ 128. Couche par couche depuis les bords connus, puis relaxation.
pub fn inpaint(src: &TileGrid, mask: &TileGrid) -> Result<TileGrid, AiError> {
    let mut buf = Buf::from_grid(src);
    let (w, h) = (buf.w, buf.h);
    let mrgba = mask.to_rgba8();
    let unknown: Vec<bool> = (0..w * h).map(|i| mrgba[i * 4] >= 128).collect();
    if !unknown.iter().any(|&u| u) {
        return Ok(src.clone());
    }
    if unknown.iter().all(|&u| u) {
        return Err(AiError::Invalid("la zone à combler couvre toute l'image".into()));
    }
    let mut known: Vec<bool> = unknown.iter().map(|u| !u).collect();
    let mut region: Vec<usize> = Vec::new();
    let neighbours = |i: usize| {
        let (x, y) = ((i % w) as i64, (i / w) as i64);
        (-1i64..=1).flat_map(move |dy| (-1i64..=1).map(move |dx| (dx, dy))).filter_map(move |(dx, dy)| {
            let (nx, ny) = (x + dx, y + dy);
            ((dx, dy) != (0, 0) && nx >= 0 && ny >= 0 && nx < w as i64 && ny < h as i64)
                .then(|| (ny as usize * w + nx as usize, if dx != 0 && dy != 0 { 0.707 } else { 1.0 }))
        })
    };
    let mut frontier: Vec<usize> = (0..w * h).filter(|&i| unknown[i] && neighbours(i).any(|(n, _)| known[n])).collect();
    while !frontier.is_empty() {
        let mut filled = Vec::with_capacity(frontier.len());
        for &i in &frontier {
            let (mut acc, mut wsum) = ([0.0f32; 4], 0.0f32);
            for (n, wt) in neighbours(i) {
                if known[n] {
                    for (a, v) in acc.iter_mut().zip(buf.d[n]) {
                        *a += v * wt;
                    }
                    wsum += wt;
                }
            }
            if wsum > 0.0 {
                buf.d[i] = acc.map(|v| v / wsum);
                filled.push(i);
            }
        }
        for &i in &filled {
            known[i] = true;
            region.push(i);
        }
        let mut next = Vec::new();
        for &i in &filled {
            for (n, _) in neighbours(i) {
                if unknown[n] && !known[n] {
                    known[n] = true; // marqué pour ne pas être ajouté deux fois
                    next.push(n);
                }
            }
        }
        // Les voisins marqués seront remplis au tour suivant : on les rend « inconnus » jusque-là.
        for &n in &next {
            known[n] = false;
        }
        next.sort_unstable();
        next.dedup();
        frontier = next;
    }
    // Relaxation (Gauss-Seidel) : lisse les marches laissées par le remplissage couche par couche.
    for _ in 0..40 {
        for &i in &region {
            let (mut acc, mut n) = ([0.0f32; 4], 0.0f32);
            for (j, _) in neighbours(i).filter(|(_, wt)| *wt == 1.0) {
                for (a, v) in acc.iter_mut().zip(buf.d[j]) {
                    *a += v;
                }
                n += 1.0;
            }
            if n > 0.0 {
                buf.d[i] = acc.map(|v| v / n);
            }
        }
    }
    Ok(buf.into_grid()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> TileGrid {
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&f(x, y));
            }
        }
        TileGrid::from_rgba8(w, h, &rgba).unwrap()
    }

    fn m(g: &TileGrid, x: u32, y: u32) -> u8 {
        g.get_pixel(x, y)[0]
    }

    #[test]
    fn subject_is_found_on_a_plain_background_and_enclosed_holes_stay_subject() {
        let blue = [20, 40, 200, 255];
        let img = image(60, 60, |x, y| {
            let in_sq = (15..45).contains(&x) && (15..45).contains(&y);
            // un « trou » de la couleur du fond, entièrement entouré par le sujet
            let hole = (28..32).contains(&x) && (28..32).contains(&y);
            if in_sq && !hole { [230, 60, 40, 255] } else { blue }
        });
        let mask = subject_mask(&img, 0.15).unwrap();
        assert_eq!((m(&mask, 2, 2), m(&mask, 58, 30), m(&mask, 30, 3)), (0, 0, 0), "le fond n'est pas sélectionné");
        assert_eq!((m(&mask, 20, 20), m(&mask, 40, 40)), (255, 255), "le sujet l'est");
        assert_eq!(m(&mask, 30, 30), 255, "un trou entouré par le sujet ne touche pas le bord : il reste sujet");
    }

    #[test]
    fn subject_handles_gradient_backgrounds_and_transparency() {
        let img = image(40, 40, |x, y| {
            if (12..28).contains(&x) && (12..28).contains(&y) {
                [10, 10, 10, 255]
            } else {
                [200, 200 + x as u8 / 2, 230, 255]
            }
        });
        let mask = subject_mask(&img, 0.2).unwrap();
        assert_eq!((m(&mask, 1, 1), m(&mask, 38, 38), m(&mask, 20, 20)), (0, 0, 255));
        let transparent =
            image(
                20,
                20,
                |x, y| if (6..14).contains(&x) && (6..14).contains(&y) { [255, 0, 0, 255] } else { [0, 0, 0, 0] },
            );
        let mask = subject_mask(&transparent, 0.1).unwrap();
        assert_eq!((m(&mask, 2, 10), m(&mask, 10, 10)), (0, 255));
    }

    #[test]
    fn inpaint_fills_a_hole_smoothly_and_leaves_the_rest_untouched() {
        // dégradé horizontal avec un « défaut » noir : la diffusion doit retrouver une valeur proche du dégradé
        let img = image(50, 30, |x, y| {
            if (20..26).contains(&x) && (10..16).contains(&y) { [0, 0, 0, 255] } else { [(x * 5) as u8, 100, 50, 255] }
        });
        let mask = image(50, 30, |x, y| if (20..26).contains(&x) && (10..16).contains(&y) { [255; 4] } else { [0; 4] });
        let out = inpaint(&img, &mask).unwrap();
        let p = out.get_pixel(23, 12);
        assert!((i32::from(p[0]) - 115).abs() <= 12, "valeur proche du dégradé (115) : {p:?}");
        assert_eq!((p[1], p[2]), (100, 50), "les autres canaux restent constants");
        assert_eq!(out.get_pixel(5, 5), img.get_pixel(5, 5), "hors de la zone, rien ne change");
        assert_eq!(out.get_pixel(19, 12), img.get_pixel(19, 12));
        // pas de zone → image identique ; zone = image entière → erreur
        assert_eq!(inpaint(&img, &image(50, 30, |_, _| [0; 4])).unwrap().to_rgba8(), img.to_rgba8());
        assert!(inpaint(&img, &image(50, 30, |_, _| [255; 4])).is_err());
    }
}
