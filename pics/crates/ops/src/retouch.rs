//! Outils de retouche appliqués dab après dab : flou, netteté, doigt, densité −/+, éponge, tampon de
//! duplication, correcteur (séparation de fréquences), correcteur localisé et pièce.
//!
//! Ces outils lisent l'état courant du calque (sauf le tampon et le correcteur, qui lisent l'état d'avant le
//! trait pour ne pas se copier eux-mêmes) : ils modifient donc directement une grille de travail.

use openpics_core::blend::{BlendMode, composite};
use openpics_core::selection::value as selection_value;
use openpics_core::{CoreError, TILE_SIZE, TileGrid, TileKey};

use crate::brush::{BrushParams, PaintKind, dab_coverage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetouchKind {
    Blur,
    Sharpen,
    Smudge,
    Dodge,
    Burn,
    Saturate,
    Desaturate,
    Clone,
    Heal,
}

/// Plage tonale visée par densité −/+.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TonalRange {
    Shadows,
    Midtones,
    Highlights,
}

type Px = [f32; 4];

fn to_f(p: [u8; 4]) -> Px {
    p.map(f32::from)
}

fn to_u(p: Px) -> [u8; 4] {
    p.map(|c| (c.clamp(0.0, 255.0) + 0.5) as u8)
}

fn lum(p: Px) -> f32 {
    (0.3 * p[0] + 0.59 * p[1] + 0.11 * p[2]) / 255.0
}

fn lerp(a: Px, b: Px, k: f32) -> Px {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * k)
}

/// Moyenne rectangulaire de rayon `r` autour de `(x, y)` (les bords du calque sont répétés).
fn box_average(g: &TileGrid, x: i64, y: i64, r: i64) -> Px {
    let (w, h) = (i64::from(g.width()), i64::from(g.height()));
    let mut acc = [0.0f32; 4];
    let mut n = 0.0;
    for dy in -r..=r {
        for dx in -r..=r {
            let p = g.get_pixel((x + dx).clamp(0, w - 1) as u32, (y + dy).clamp(0, h - 1) as u32);
            // Moyenne pondérée par l'alpha : un voisin transparent ne ternit pas la couleur.
            let a = f32::from(p[3]);
            for c in 0..3 {
                acc[c] += f32::from(p[c]) * a;
            }
            acc[3] += a;
            n += 1.0;
        }
    }
    if acc[3] > 0.0 { [acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3], acc[3] / n] } else { [0.0; 4] }
}

pub struct RetouchStroke {
    kind: RetouchKind,
    params: BrushParams,
    range: TonalRange,
    base: TileGrid,
    work: TileGrid,
    selection: Option<TileGrid>,
    /// Décalage source − destination pour le tampon et le correcteur.
    offset: Option<(f32, f32)>,
    pickup: Option<Vec<Px>>,
    /// Correcteur localisé : l'éclairage de destination vient du pourtour du dab, pas de son intérieur
    /// (qui contient justement la tache à effacer).
    ring_lighting: bool,
    last_dab: Option<(f32, f32)>,
    carry: f32,
}

impl RetouchStroke {
    pub fn new(
        base: TileGrid,
        kind: RetouchKind,
        params: BrushParams,
        range: TonalRange,
        offset: Option<(f32, f32)>,
        selection: Option<TileGrid>,
    ) -> Self {
        Self {
            kind,
            params: params.sanitized(),
            range,
            work: base.clone(),
            base,
            selection,
            offset,
            pickup: None,
            ring_lighting: false,
            last_dab: None,
            carry: 0.0,
        }
    }

    pub fn result(&self) -> &TileGrid {
        &self.work
    }

    pub fn base(&self) -> &TileGrid {
        &self.base
    }

    pub fn add_point(&mut self, x: f32, y: f32, pressure: f32) -> Result<(), CoreError> {
        if !x.is_finite() || !y.is_finite() {
            return Ok(());
        }
        let pressure = if pressure.is_finite() { pressure.clamp(0.0, 1.0) } else { 1.0 };
        let size = if self.params.size_from_pressure {
            (self.params.size * pressure.max(0.05)).max(1.0)
        } else {
            self.params.size
        };
        let spacing = (size * self.params.spacing).max(0.5);
        match self.last_dab {
            None => {
                self.dab(x, y, size)?;
                self.last_dab = Some((x, y));
            }
            Some((lx, ly)) => {
                let (dx, dy) = (x - lx, y - ly);
                let len = dx.hypot(dy);
                let mut travelled = spacing - self.carry;
                while travelled <= len {
                    let t = travelled / len;
                    self.dab(lx + dx * t, ly + dy * t, size)?;
                    travelled += spacing;
                }
                self.carry = (len - (travelled - spacing)).max(0.0);
                self.last_dab = Some((x, y));
            }
        }
        Ok(())
    }

    fn dab(&mut self, cx: f32, cy: f32, size: f32) -> Result<(), CoreError> {
        let r = size / 2.0;
        let (w, h) = (i64::from(self.work.width()), i64::from(self.work.height()));
        let (x0, x1) = (((cx - r).floor() as i64).max(0), ((cx + r).ceil() as i64).min(w - 1));
        let (y0, y1) = (((cy - r).floor() as i64).max(0), ((cy + r).ceil() as i64).min(h - 1));
        if x1 < x0 || y1 < y0 {
            return Ok(());
        }
        let (rw, rh) = ((x1 - x0 + 1) as usize, (y1 - y0 + 1) as usize);
        let blur_r = (size / 12.0).round().clamp(1.0, 12.0) as i64;
        let orig: Vec<Px> = (0..rh)
            .flat_map(|j| (0..rw).map(move |i| (i, j)))
            .map(|(i, j)| to_f(self.work.get_pixel((x0 + i as i64) as u32, (y0 + j as i64) as u32)))
            .collect();
        let first_pickup = self.pickup.is_none();
        if first_pickup && self.kind == RetouchKind::Smudge {
            self.pickup = Some(orig.clone());
            return Ok(());
        }
        let strength = self.params.opacity * self.params.flow;
        let mut next_pickup = orig.clone();
        let mut changed: Vec<((u32, u32), [u8; 4])> = Vec::new();
        for j in 0..rh {
            for i in 0..rw {
                let (x, y) = (x0 + i as i64, y0 + j as i64);
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let sel =
                    self.selection.as_ref().map_or(1.0, |s| f32::from(selection_value(s, x as u32, y as u32)) / 255.0);
                let cov = dab_coverage(PaintKind::Brush, self.params.hardness, r, d) * strength * sel;
                if cov <= 0.0 {
                    continue;
                }
                let o = orig[j * rw + i];
                let out: Px = match self.kind {
                    RetouchKind::Blur => lerp(o, box_average(&self.work, x, y, blur_r), cov.min(1.0)),
                    RetouchKind::Sharpen => {
                        let avg = box_average(&self.work, x, y, blur_r);
                        [0, 1, 2, 3].map(|c| if c == 3 { o[3] } else { o[c] + (o[c] - avg[c]) * cov * 2.0 })
                    }
                    RetouchKind::Smudge => {
                        let p = self.pickup.as_ref().map_or(o, |p| p[j * rw + i]);
                        let out = lerp(o, p, cov.min(1.0));
                        next_pickup[j * rw + i] = lerp(o, p, strength.clamp(0.0, 1.0));
                        out
                    }
                    RetouchKind::Dodge | RetouchKind::Burn => {
                        let l = lum(o);
                        let weight = match self.range {
                            TonalRange::Shadows => (1.0 - l * 2.0).clamp(0.0, 1.0),
                            TonalRange::Highlights => (l * 2.0 - 1.0).clamp(0.0, 1.0),
                            TonalRange::Midtones => (1.0 - (l - 0.5).abs() * 2.0).clamp(0.0, 1.0),
                        };
                        let f = cov * weight.max(0.15);
                        [0, 1, 2, 3].map(|c| {
                            if c == 3 {
                                o[3]
                            } else if self.kind == RetouchKind::Dodge {
                                o[c] + (255.0 - o[c]) * f
                            } else {
                                o[c] * (1.0 - f)
                            }
                        })
                    }
                    RetouchKind::Saturate | RetouchKind::Desaturate => {
                        let gray = lum(o) * 255.0;
                        let k = if self.kind == RetouchKind::Saturate { 1.0 + cov } else { 1.0 - cov.min(1.0) };
                        [0, 1, 2, 3].map(|c| if c == 3 { o[3] } else { gray + (o[c] - gray) * k })
                    }
                    RetouchKind::Clone => {
                        let Some((dx, dy)) = self.offset else { continue };
                        let (sx, sy) = ((x as f32 + dx).round() as i64, (y as f32 + dy).round() as i64);
                        if sx < 0 || sy < 0 || sx >= w || sy >= h {
                            continue;
                        }
                        let s = to_f(self.base.get_pixel(sx as u32, sy as u32));
                        let back = [o[0] / 255.0, o[1] / 255.0, o[2] / 255.0, o[3] / 255.0];
                        let p = composite(
                            BlendMode::Normal,
                            back,
                            [s[0] / 255.0, s[1] / 255.0, s[2] / 255.0],
                            s[3] / 255.0 * cov.min(1.0),
                            x as u32,
                            y as u32,
                        );
                        p.map(|c| c * 255.0)
                    }
                    RetouchKind::Heal => {
                        let Some((dx, dy)) = self.offset else { continue };
                        let (sx, sy) = ((x as f32 + dx).round() as i64, (y as f32 + dy).round() as i64);
                        if sx < 0 || sy < 0 || sx >= w || sy >= h {
                            continue;
                        }
                        let s = to_f(self.base.get_pixel(sx as u32, sy as u32));
                        let low_s = box_average(&self.base, sx, sy, blur_r * 2);
                        let low_d = if self.ring_lighting {
                            ring_average(&self.base, cx, cy, r * 1.4)
                        } else {
                            box_average(&self.base, x, y, blur_r * 2)
                        };
                        // Texture de la source, éclairage de la destination.
                        let healed: Px = [
                            s[0] - low_s[0] + low_d[0],
                            s[1] - low_s[1] + low_d[1],
                            s[2] - low_s[2] + low_d[2],
                            o[3].max(s[3]),
                        ];
                        lerp(o, healed, cov.min(1.0))
                    }
                };
                changed.push(((x as u32, y as u32), to_u(out)));
            }
        }
        if self.kind == RetouchKind::Smudge {
            self.pickup = Some(next_pickup);
        }
        for ((x, y), p) in changed {
            self.work.put_pixel(x, y, p);
        }
        Ok(())
    }
}

/// Moyenne de 24 points répartis sur un cercle de rayon `r` : l'éclairage « autour » d'une zone.
fn ring_average(g: &TileGrid, cx: f32, cy: f32, r: f32) -> Px {
    let mut acc = [0.0f32; 4];
    for k in 0..24 {
        let a = k as f32 / 24.0 * std::f32::consts::TAU;
        let (x, y) = ((cx + a.cos() * r).round() as i64, (cy + a.sin() * r).round() as i64);
        let p = to_f(
            g.get_pixel(x.clamp(0, i64::from(g.width()) - 1) as u32, y.clamp(0, i64::from(g.height()) - 1) as u32),
        );
        for c in 0..4 {
            acc[c] += p[c] / 24.0;
        }
    }
    acc
}

/// Correcteur localisé : choisit automatiquement, parmi huit directions, la zone voisine dont le contour
/// ressemble le plus à celui de la tache, puis la corrige. Renvoie le calque corrigé.
pub fn spot_heal(
    src: &TileGrid,
    cx: f32,
    cy: f32,
    size: f32,
    selection: Option<TileGrid>,
) -> Result<TileGrid, CoreError> {
    let size = if size.is_finite() { size.clamp(2.0, 400.0) } else { 20.0 };
    let r = size / 2.0;
    let ring = |g: &TileGrid, ox: f32, oy: f32| -> f64 {
        let mut samples = Vec::new();
        for k in 0..24 {
            let a = k as f32 / 24.0 * std::f32::consts::TAU;
            for rr in [r * 1.25, r * 1.5] {
                let (x, y) = ((ox + a.cos() * rr).round() as i64, (oy + a.sin() * rr).round() as i64);
                if x < 0 || y < 0 || x >= i64::from(g.width()) || y >= i64::from(g.height()) {
                    return f64::INFINITY;
                }
                samples.push(g.get_pixel(x as u32, y as u32));
            }
        }
        samples.iter().map(|p| f64::from(p[0]) + f64::from(p[1]) + f64::from(p[2])).sum::<f64>() / samples.len() as f64
    };
    let ring_pixels = |g: &TileGrid, ox: f32, oy: f32| -> Vec<[u8; 4]> {
        (0..24)
            .map(|k| {
                let a = k as f32 / 24.0 * std::f32::consts::TAU;
                let (x, y) = ((ox + a.cos() * r * 1.25).round() as i64, (oy + a.sin() * r * 1.25).round() as i64);
                g.get_pixel(x.clamp(0, i64::from(g.width()) - 1) as u32, y.clamp(0, i64::from(g.height()) - 1) as u32)
            })
            .collect()
    };
    let target = ring_pixels(src, cx, cy);
    let mut best: Option<((f32, f32), f64)> = None;
    // Huit directions × trois distances : on garde la zone dont le pourtour ressemble le plus à celui de la tache.
    for dist in [1.4f32, 2.0, 2.8] {
        for dir in 0..8 {
            let a = dir as f32 / 8.0 * std::f32::consts::TAU;
            let (dx, dy) = (a.cos() * size * dist, a.sin() * size * dist);
            if ring(src, cx + dx, cy + dy).is_infinite() {
                continue;
            }
            let cand = ring_pixels(src, cx + dx, cy + dy);
            let score: f64 = target
                .iter()
                .zip(&cand)
                .map(|(t, c)| (0..3).map(|i| (f64::from(t[i]) - f64::from(c[i])).powi(2)).sum::<f64>())
                .sum();
            if best.is_none_or(|(_, s)| score < s) {
                best = Some(((dx, dy), score));
            }
        }
    }
    let Some((offset, _)) = best else { return Ok(src.clone()) };
    let params = BrushParams {
        size,
        hardness: 0.6,
        opacity: 1.0,
        flow: 1.0,
        spacing: 0.5,
        size_from_pressure: false,
        ..BrushParams::default()
    };
    let mut stroke =
        RetouchStroke::new(src.clone(), RetouchKind::Heal, params, TonalRange::Midtones, Some(offset), selection);
    stroke.ring_lighting = true;
    stroke.add_point(cx, cy, 1.0)?;
    Ok(stroke.work)
}

/// Pièce : remplace la zone `mask` par le contenu situé à `offset` (source − destination). Avec
/// `match_lighting`, seule la texture de la source est reprise (éclairage de la destination, comme le
/// correcteur) ; sinon c'est une copie directe.
pub fn patch(src: &TileGrid, mask: &TileGrid, offset: (i32, i32), match_lighting: bool) -> Result<TileGrid, CoreError> {
    let (w, h) = (i64::from(src.width()), i64::from(src.height()));
    let mut out = src.clone();
    for (key, tile) in mask.iter() {
        let TileKey { tx, ty } = *key;
        for ly in 0..TILE_SIZE {
            for lx in 0..TILE_SIZE {
                let m = tile.pixel(lx, ly)[0];
                if m == 0 {
                    continue;
                }
                let (x, y) = (i64::from(tx * TILE_SIZE + lx), i64::from(ty * TILE_SIZE + ly));
                let (sx, sy) = (x + i64::from(offset.0), y + i64::from(offset.1));
                if x >= w || y >= h || sx < 0 || sy < 0 || sx >= w || sy >= h {
                    continue;
                }
                let s = to_f(src.get_pixel(sx as u32, sy as u32));
                let (low_s, low_d) = (box_average(src, sx, sy, 6), box_average(src, x, y, 6));
                let healed: Px = if match_lighting {
                    [s[0] - low_s[0] + low_d[0], s[1] - low_s[1] + low_d[1], s[2] - low_s[2] + low_d[2], s[3]]
                } else {
                    s
                };
                let o = to_f(src.get_pixel(x as u32, y as u32));
                out.put_pixel(x as u32, y as u32, to_u(lerp(o, healed, f32::from(m) / 255.0)));
            }
        }
    }
    Ok(out)
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

    fn params(size: f32) -> BrushParams {
        BrushParams {
            size,
            hardness: 1.0,
            opacity: 1.0,
            flow: 1.0,
            spacing: 0.25,
            size_from_pressure: false,
            ..BrushParams::default()
        }
    }

    fn stroke(g: &TileGrid, kind: RetouchKind, size: f32, pts: &[(f32, f32)]) -> TileGrid {
        let mut s = RetouchStroke::new(g.clone(), kind, params(size), TonalRange::Midtones, None, None);
        for p in pts {
            s.add_point(p.0, p.1, 1.0).unwrap();
        }
        s.result().clone()
    }

    fn checker() -> TileGrid {
        filled(60, 60, |x, y| if (x / 2 + y / 2) % 2 == 0 { [0, 0, 0, 255] } else { [255, 255, 255, 255] })
    }

    fn contrast(g: &TileGrid) -> f32 {
        let vals: Vec<f32> = (10..50)
            .flat_map(|y| (10..50).map(move |x| (x, y)))
            .map(|(x, y)| f32::from(g.get_pixel(x, y)[0]))
            .collect();
        let mean = vals.iter().sum::<f32>() / vals.len() as f32;
        vals.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / vals.len() as f32
    }

    #[test]
    fn blur_lowers_contrast_and_sharpen_raises_it() {
        let g = checker();
        let blurred = stroke(&g, RetouchKind::Blur, 40.0, &[(30.0, 30.0)]);
        assert!(contrast(&blurred) < contrast(&g) * 0.7, "{} → {}", contrast(&g), contrast(&blurred));
        let mid = filled(60, 60, |x, _| if x < 30 { [100, 100, 100, 255] } else { [160, 160, 160, 255] });
        let sharp = stroke(&mid, RetouchKind::Sharpen, 40.0, &[(30.0, 30.0)]);
        assert!(
            sharp.get_pixel(29, 30)[0] < 100 && sharp.get_pixel(30, 30)[0] > 160,
            "halo de netteté de part et d'autre du bord"
        );
    }

    #[test]
    fn smudge_drags_colour_along_the_stroke() {
        let g = filled(100, 40, |x, _| if x < 30 { [255, 0, 0, 255] } else { [0, 0, 255, 255] });
        let out = stroke(
            &g,
            RetouchKind::Smudge,
            16.0,
            &[(25.0, 20.0), (35.0, 20.0), (45.0, 20.0), (55.0, 20.0), (65.0, 20.0)],
        );
        assert!(
            out.get_pixel(50, 20)[0] > 60,
            "du rouge a été entraîné dans la zone bleue : {:?}",
            out.get_pixel(50, 20)
        );
        assert_eq!(out.get_pixel(95, 5), [0, 0, 255, 255], "hors du trait : inchangé");
    }

    #[test]
    fn dodge_and_burn_respect_the_tonal_range() {
        let g = filled(40, 40, |x, _| if x < 20 { [30, 30, 30, 255] } else { [220, 220, 220, 255] });
        let mk = |kind, range| {
            let mut s =
                RetouchStroke::new(g.clone(), kind, BrushParams { opacity: 0.5, ..params(40.0) }, range, None, None);
            s.add_point(20.0, 20.0, 1.0).unwrap();
            s.result().clone()
        };
        let dodge_sh = mk(RetouchKind::Dodge, TonalRange::Shadows);
        assert!(dodge_sh.get_pixel(10, 20)[0] > 60, "les ombres s'éclaircissent");
        assert!(
            dodge_sh.get_pixel(30, 20)[0] - 220 < 8 || dodge_sh.get_pixel(30, 20)[0] >= 220,
            "les hautes lumières bougent peu"
        );
        let burn_hi = mk(RetouchKind::Burn, TonalRange::Highlights);
        assert!(burn_hi.get_pixel(30, 20)[0] < 200, "les hautes lumières s'assombrissent");
        assert!(burn_hi.get_pixel(10, 20)[0] >= 28, "les ombres sont épargnées");
    }

    #[test]
    fn sponge_changes_saturation_and_keeps_luminance_roughly() {
        let g = filled(40, 40, |_, _| [200, 100, 50, 255]);
        let sat = |p: [u8; 4]| i32::from(p[0]) - i32::from(p[2]);
        let more = stroke(&g, RetouchKind::Saturate, 30.0, &[(20.0, 20.0)]);
        let less = stroke(&g, RetouchKind::Desaturate, 30.0, &[(20.0, 20.0)]);
        assert!(sat(more.get_pixel(20, 20)) > sat(g.get_pixel(20, 20)));
        assert!(sat(less.get_pixel(20, 20)) < 5, "désaturation complète au centre");
    }

    #[test]
    fn clone_copies_from_the_offset_source_using_the_pre_stroke_state() {
        let g = filled(100, 40, |x, _| if x < 40 { [255, 0, 0, 255] } else { [0, 0, 0, 0] });
        let mut s =
            RetouchStroke::new(g, RetouchKind::Clone, params(10.0), TonalRange::Midtones, Some((-60.0, 0.0)), None);
        s.add_point(70.0, 20.0, 1.0).unwrap();
        s.add_point(85.0, 20.0, 1.0).unwrap();
        assert_eq!(s.result().get_pixel(70, 20), [255, 0, 0, 255]);
        assert_eq!(s.result().get_pixel(85, 20), [255, 0, 0, 255]);
        let mut none =
            RetouchStroke::new(s.base().clone(), RetouchKind::Clone, params(10.0), TonalRange::Midtones, None, None);
        none.add_point(70.0, 20.0, 1.0).unwrap();
        assert_eq!(none.result().get_pixel(70, 20), [0; 4], "sans source définie : rien ne se passe");
    }

    #[test]
    fn heal_keeps_source_texture_but_takes_destination_lighting() {
        // Source (gauche) : damier autour de 200 ; destination (droite) : damier d'amplitude identique autour de 60.
        let g = filled(120, 40, |x, y| {
            let t = if (x + y) % 2 == 0 { 20 } else { -20 };
            let base = if x < 60 { 200 } else { 60 };
            let v = (base + t) as u8;
            [v, v, v, 255]
        });
        let mut s =
            RetouchStroke::new(g, RetouchKind::Heal, params(24.0), TonalRange::Midtones, Some((-60.0, 0.0)), None);
        s.add_point(90.0, 20.0, 1.0).unwrap();
        let vals: Vec<f32> = (84..96).map(|x| f32::from(s.result().get_pixel(x, 20)[0])).collect();
        let mean = vals.iter().sum::<f32>() / vals.len() as f32;
        assert!((mean - 60.0).abs() < 25.0, "éclairage de la destination conservé : {mean}");
        assert!(
            vals.iter().any(|&v| v > mean + 10.0) && vals.iter().any(|&v| v < mean - 10.0),
            "texture conservée : {vals:?}"
        );
    }

    #[test]
    fn spot_heal_removes_a_blemish_on_a_flat_background() {
        let g = filled(200, 100, |x, y| {
            if (x as i32 - 100).pow(2) + (y as i32 - 50).pow(2) < 36 { [20, 20, 20, 255] } else { [180, 160, 140, 255] }
        });
        let out = spot_heal(&g, 100.0, 50.0, 24.0, None).unwrap();
        let p = out.get_pixel(100, 50);
        assert!((i32::from(p[0]) - 180).abs() < 12 && (i32::from(p[2]) - 140).abs() < 12, "{p:?}");
        assert_eq!(out.get_pixel(10, 10), g.get_pixel(10, 10));
        // Taille ou position absurde : pas de panique.
        assert!(spot_heal(&g, -50.0, -50.0, f32::NAN, None).is_ok());
    }

    #[test]
    fn patch_copies_directly_or_keeps_only_the_texture() {
        // Source (gauche) : damier autour de 200 ; destination (droite) : damier autour de 60.
        let g = filled(100, 40, |x, y| {
            let t = if (x + y) % 2 == 0 { 20 } else { -20 };
            let v = ((if x < 40 { 200 } else { 60 }) + t) as u8;
            [v, v, v, 255]
        });
        let mask = crate::selection::rect(100, 40, (60.0, 10.0), (80.0, 30.0)).unwrap();
        let copy = patch(&g, &mask, (-40, 0), false).unwrap();
        assert_eq!(copy.get_pixel(70, 20), g.get_pixel(30, 20), "copie directe");
        let healed = patch(&g, &mask, (-40, 0), true).unwrap();
        let vals: Vec<f32> = (64..76).map(|x| f32::from(healed.get_pixel(x, 20)[0])).collect();
        let mean = vals.iter().sum::<f32>() / vals.len() as f32;
        assert!((mean - 60.0).abs() < 25.0, "éclairage de la destination : {mean}");
        assert!(
            vals.iter().any(|&v| v > mean + 8.0) && vals.iter().any(|&v| v < mean - 8.0),
            "texture de la source : {vals:?}"
        );
        assert_eq!(healed.get_pixel(95, 20), g.get_pixel(95, 20));
        assert_eq!(healed.get_pixel(70, 5), g.get_pixel(70, 5), "hors masque : inchangé");
    }

    #[test]
    fn retouch_respects_the_selection_and_ignores_nan() {
        let g = checker();
        let sel = crate::selection::rect(60, 60, (0.0, 0.0), (30.0, 60.0)).unwrap();
        let mut s =
            RetouchStroke::new(g.clone(), RetouchKind::Blur, params(60.0), TonalRange::Midtones, None, Some(sel));
        s.add_point(30.0, 30.0, 1.0).unwrap();
        s.add_point(f32::NAN, 0.0, 1.0).unwrap();
        assert_ne!(s.result().get_pixel(10, 10), g.get_pixel(10, 10));
        assert_eq!(s.result().get_pixel(45, 30), g.get_pixel(45, 30));
    }
}
