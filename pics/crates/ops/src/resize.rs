//! Rééchantillonnage Lanczos-3 séparable, sur tampon prémultiplié (un bord transparent ne ternit pas la couleur).
//! En réduction, le noyau est élargi du facteur d'échelle : c'est ce qui évite le crénelage.

use openpics_core::{CoreError, TileGrid};
use rayon::prelude::*;

use crate::filters::Buf;

type Px = [f32; 4];

fn lanczos3(x: f32) -> f32 {
    let x = x.abs();
    if x < 1e-6 {
        1.0
    } else if x >= 3.0 {
        0.0
    } else {
        let px = std::f32::consts::PI * x;
        3.0 * px.sin() * (px / 3.0).sin() / (px * px)
    }
}

/// Pour chaque pixel de sortie : premier pixel source et poids normalisés.
fn weights(src: usize, dst: usize) -> Vec<(usize, Vec<f32>)> {
    let scale = src as f32 / dst as f32;
    let filter = scale.max(1.0);
    let support = 3.0 * filter;
    (0..dst)
        .map(|i| {
            let center = (i as f32 + 0.5) * scale;
            let lo = ((center - support).floor().max(0.0)) as usize;
            let hi = (((center + support).ceil()) as usize).min(src - 1);
            let mut w: Vec<f32> = (lo..=hi).map(|j| lanczos3((j as f32 + 0.5 - center) / filter)).collect();
            let sum: f32 = w.iter().sum();
            if sum.abs() > 1e-9 {
                w.iter_mut().for_each(|v| *v /= sum);
            }
            (lo, w)
        })
        .collect()
}

fn resample_px(src: &[Px], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<Px> {
    let wx = weights(sw, dw);
    // Passe horizontale : sh lignes de dw pixels.
    let tmp: Vec<Px> = (0..sh)
        .into_par_iter()
        .flat_map_iter(|y| {
            let row = &src[y * sw..(y + 1) * sw];
            wx.iter().map(move |(lo, w)| {
                let mut acc = [0.0f32; 4];
                for (k, wt) in w.iter().enumerate() {
                    for (a, v) in acc.iter_mut().zip(row[lo + k]) {
                        *a += v * wt;
                    }
                }
                acc
            })
        })
        .collect();
    let wy = weights(sh, dh);
    // Passe verticale.
    (0..dh)
        .into_par_iter()
        .flat_map_iter(|y| {
            let (lo, w) = &wy[y];
            let tmp = &tmp;
            (0..dw).map(move |x| {
                let mut acc = [0.0f32; 4];
                for (k, wt) in w.iter().enumerate() {
                    for (a, v) in acc.iter_mut().zip(tmp[(lo + k) * dw + x]) {
                        *a += v * wt;
                    }
                }
                acc
            })
        })
        .collect()
}

fn check(w: u32, h: u32) -> Result<(), CoreError> {
    if w == 0 || h == 0 || w > openpics_core::tile::MAX_DIMENSION || h > openpics_core::tile::MAX_DIMENSION {
        return Err(CoreError::InvalidSize { width: w, height: h, max: openpics_core::tile::MAX_DIMENSION });
    }
    Ok(())
}

/// Image RGBA (alpha droit) → nouvelle taille.
pub fn resample(src: &TileGrid, width: u32, height: u32) -> Result<TileGrid, CoreError> {
    check(width, height)?;
    let b = Buf::from_grid(src);
    let mut d = resample_px(&b.d, b.w, b.h, width as usize, height as usize);
    // Lanczos déborde légèrement : on ramène les couleurs sous l'alpha pour rester une image prémultipliée valide.
    for p in &mut d {
        let a = p[3].clamp(0.0, 1.0);
        p[3] = a;
        for c in &mut p[..3] {
            *c = c.clamp(0.0, a);
        }
    }
    Buf { w: width as usize, h: height as usize, d }.into_grid()
}

/// Masque (valeur répétée sur les quatre canaux, sans prémultiplication).
pub fn resample_mask(src: &TileGrid, width: u32, height: u32) -> Result<TileGrid, CoreError> {
    check(width, height)?;
    let rgba = src.to_rgba8();
    let px: Vec<Px> = rgba.chunks_exact(4).map(|p| [f32::from(p[0]) / 255.0; 4]).collect();
    let d = resample_px(&px, src.width() as usize, src.height() as usize, width as usize, height as usize);
    let mut out = Vec::with_capacity(d.len() * 4);
    for p in d {
        let v = (p[0].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        out.extend_from_slice(&[v, v, v, v]);
    }
    TileGrid::from_rgba8(width, height, &out)
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

    #[test]
    fn size_changes_and_flat_colours_are_preserved_both_ways() {
        let img = image(37, 21, |_, _| [200, 100, 50, 255]);
        for (w, h) in [(74, 42), (10, 6), (37, 21), (1, 1)] {
            let out = resample(&img, w, h).unwrap();
            assert_eq!((out.width(), out.height()), (w, h));
            let p = out.get_pixel(w / 2, h / 2);
            assert!(
                p[0].abs_diff(200) <= 1 && p[1].abs_diff(100) <= 1 && p[2].abs_diff(50) <= 1 && p[3] == 255,
                "{p:?}"
            );
        }
        assert!(resample(&img, 0, 5).is_err());
    }

    #[test]
    fn downscaling_a_fine_checkerboard_averages_instead_of_aliasing() {
        let img = image(64, 64, |x, y| if (x + y) % 2 == 0 { [255, 255, 255, 255] } else { [0, 0, 0, 255] });
        let out = resample(&img, 16, 16).unwrap();
        let p = out.get_pixel(8, 8);
        assert!(p[0].abs_diff(128) <= 12, "gris moyen attendu : {p:?}");
    }

    #[test]
    fn transparent_surroundings_do_not_darken_edges_and_masks_resample() {
        let img = image(20, 20, |x, _| if x < 10 { [255, 0, 0, 255] } else { [0, 0, 0, 0] });
        let out = resample(&img, 40, 40).unwrap();
        let p = out.get_pixel(15, 20); // dans la partie rouge, près du bord
        assert!(p[3] > 200 && p[0] > 240 && p[1] < 12, "le bord reste rouge, non noirci : {p:?}");
        let mask = image(10, 10, |x, _| if x < 5 { [255; 4] } else { [0; 4] });
        let m = resample_mask(&mask, 20, 20).unwrap();
        assert_eq!((m.get_pixel(2, 5)[0], m.get_pixel(18, 5)[0]), (255, 0));
    }
}
