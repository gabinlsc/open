//! Styles de calque (ombre portée, lueur, contour, incrustations), calculés à partir de la forme du
//! calque (son alpha, masque compris, remplissage exclu).
//!
//! Ce module est l'implémentation de référence en CPU. Le remplissage n'affecte pas les styles :
//! à 0 %, le contenu disparaît mais l'ombre, le contour et les incrustations restent.

use serde::{Deserialize, Serialize};

use crate::blend::BlendMode;

pub type Color = [u8; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrokePosition {
    Outside,
    Inside,
    Center,
}

/// Un style. `angle_deg` : 0 = vers la droite, 90 = vers le bas (axe y de l'écran).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayerStyle {
    DropShadow { color: Color, opacity: f32, angle_deg: f32, distance: f32, size: f32 },
    OuterGlow { color: Color, opacity: f32, size: f32 },
    Stroke { color: Color, opacity: f32, size: f32, position: StrokePosition },
    ColorOverlay { color: Color, opacity: f32, blend: BlendMode },
    GradientOverlay { from: Color, to: Color, angle_deg: f32, opacity: f32, blend: BlendMode },
}

/// Plafonds : un style hors bornes ne doit ni saturer la mémoire ni bloquer le rendu.
pub const MAX_SIZE: f32 = 250.0;
pub const MAX_DISTANCE: f32 = 500.0;

fn unit(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

fn bounded(v: f32, max: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, max) }
}

impl LayerStyle {
    /// Bornes toutes les valeurs numériques (fichier trafiqué, saisie hors limites).
    pub fn sanitized(&self) -> LayerStyle {
        match self {
            LayerStyle::DropShadow { color, opacity, angle_deg, distance, size } => LayerStyle::DropShadow {
                color: *color,
                opacity: unit(*opacity),
                angle_deg: if angle_deg.is_finite() { *angle_deg } else { 0.0 },
                distance: bounded(*distance, MAX_DISTANCE),
                size: bounded(*size, MAX_SIZE),
            },
            LayerStyle::OuterGlow { color, opacity, size } => {
                LayerStyle::OuterGlow { color: *color, opacity: unit(*opacity), size: bounded(*size, MAX_SIZE) }
            }
            LayerStyle::Stroke { color, opacity, size, position } => LayerStyle::Stroke {
                color: *color,
                opacity: unit(*opacity),
                size: bounded(*size, MAX_SIZE),
                position: *position,
            },
            LayerStyle::ColorOverlay { color, opacity, blend } => {
                LayerStyle::ColorOverlay { color: *color, opacity: unit(*opacity), blend: *blend }
            }
            LayerStyle::GradientOverlay { from, to, angle_deg, opacity, blend } => LayerStyle::GradientOverlay {
                from: *from,
                to: *to,
                angle_deg: if angle_deg.is_finite() { *angle_deg } else { 0.0 },
                opacity: unit(*opacity),
                blend: *blend,
            },
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            LayerStyle::DropShadow { .. } => "Ombre portée",
            LayerStyle::OuterGlow { .. } => "Lueur externe",
            LayerStyle::Stroke { .. } => "Contour",
            LayerStyle::ColorOverlay { .. } => "Incrustation de couleur",
            LayerStyle::GradientOverlay { .. } => "Incrustation de dégradé",
        }
    }

    /// Dessiné sous le contenu du calque (sinon par-dessus).
    pub fn is_under(&self) -> bool {
        matches!(self, LayerStyle::DropShadow { .. } | LayerStyle::OuterGlow { .. })
    }

    /// Distance maximale (px) à laquelle le style déborde de la forme : sert à agrandir la zone lue.
    pub fn reach(&self) -> u32 {
        let r = match self {
            LayerStyle::DropShadow { distance, size, .. } => distance + size,
            LayerStyle::OuterGlow { size, .. } => *size,
            LayerStyle::Stroke { size, position, .. } => match position {
                StrokePosition::Outside => *size,
                StrokePosition::Center => size / 2.0,
                StrokePosition::Inside => 0.0,
            },
            LayerStyle::ColorOverlay { .. } | LayerStyle::GradientOverlay { .. } => 0.0,
        };
        r.ceil().max(0.0) as u32 + 1
    }
}

/// Zone rectangulaire de valeurs flottantes (alpha) en coordonnées de document.
pub struct Region {
    pub x0: i64,
    pub y0: i64,
    pub w: usize,
    pub h: usize,
    pub data: Vec<f32>,
}

impl Region {
    pub fn at(&self, x: i64, y: i64) -> f32 {
        let (lx, ly) = (x - self.x0, y - self.y0);
        if lx < 0 || ly < 0 || lx as usize >= self.w || ly as usize >= self.h {
            0.0
        } else {
            self.data[ly as usize * self.w + lx as usize]
        }
    }

    fn map(&self, f: impl Fn(usize, usize) -> f32) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.data.len());
        for y in 0..self.h {
            for x in 0..self.w {
                out.push(f(x, y));
            }
        }
        out
    }

    /// Décalage entier : le résultat en (x, y) vaut la valeur d'origine en (x − dx, y − dy).
    pub fn shifted(&self, dx: i64, dy: i64) -> Region {
        let data = self.map(|x, y| self.at(self.x0 + x as i64 - dx, self.y0 + y as i64 - dy));
        Region { x0: self.x0, y0: self.y0, w: self.w, h: self.h, data }
    }

    /// Flou approximant une gaussienne (3 passes de flou rectangulaire, séparables, en O(n)).
    /// `radius` est le rayon visé en pixels ; 0 ne change rien.
    pub fn blurred(&self, radius: f32) -> Region {
        if radius < 0.5 {
            return Region { x0: self.x0, y0: self.y0, w: self.w, h: self.h, data: self.data.clone() };
        }
        let sigma = radius / 2.0;
        let mut data = self.data.clone();
        for width in box_widths(sigma) {
            let r = (width - 1) / 2;
            box_pass(&mut data, self.w, self.h, r, true);
            box_pass(&mut data, self.w, self.h, r, false);
        }
        Region { x0: self.x0, y0: self.y0, w: self.w, h: self.h, data }
    }

    /// Dilatation (max) ou érosion (min) par un disque de rayon `radius`.
    pub fn morph(&self, radius: f32, dilate: bool) -> Region {
        let r = radius.max(0.0);
        let ri = r.ceil() as i64;
        let r2 = r * r;
        let data = self.map(|x, y| {
            let mut best = if dilate { 0.0f32 } else { 1.0f32 };
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    if (dx * dx + dy * dy) as f32 > r2 {
                        continue;
                    }
                    let v = self.at(self.x0 + x as i64 + dx, self.y0 + y as i64 + dy);
                    best = if dilate { best.max(v) } else { best.min(v) };
                }
            }
            best
        });
        Region { x0: self.x0, y0: self.y0, w: self.w, h: self.h, data }
    }
}

/// Largeurs de 3 passes rectangulaires dont la composition approche une gaussienne d'écart-type `sigma`.
fn box_widths(sigma: f32) -> [usize; 3] {
    let n = 3.0f32;
    let ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let mut wl = ideal.floor() as usize;
    if wl % 2 == 0 {
        wl = wl.saturating_sub(1);
    }
    let wl = wl.max(1);
    let wu = wl + 2;
    let wlf = wl as f32;
    let m = ((12.0 * sigma * sigma - n * wlf * wlf - 4.0 * n * wlf - 3.0 * n) / (-4.0 * wlf - 4.0))
        .round()
        .clamp(0.0, 3.0) as usize;
    std::array::from_fn(|i| if i < m { wl } else { wu })
}

/// Une passe de flou rectangulaire de rayon `r`, bords considérés transparents (la masse est conservée
/// tant que la forme reste à plus de 3 × `r` des bords de la zone — d'où la marge `reach`).
fn box_pass(data: &mut [f32], w: usize, h: usize, r: usize, horizontal: bool) {
    let (len, lines) = if horizontal { (w, h) } else { (h, w) };
    let norm = 1.0 / (2 * r + 1) as f32;
    let mut line = vec![0.0f32; len];
    for l in 0..lines {
        let idx = |i: usize| if horizontal { l * w + i } else { i * w + l };
        for (i, v) in line.iter_mut().enumerate() {
            *v = data[idx(i)];
        }
        let mut sum: f32 = line.iter().take(r + 1).sum();
        for i in 0..len {
            data[idx(i)] = sum * norm;
            let add = i + r + 1;
            let sub = i as isize - r as isize;
            if add < len {
                sum += line[add];
            }
            if sub >= 0 {
                sum -= line[sub as usize];
            }
        }
    }
}

/// Alpha de l'effet « sous le contenu » (ombre, lueur) ou « contour », sur la zone de `shape`.
/// Renvoie la carte d'alpha déjà multipliée par l'opacité du style.
pub fn effect_alpha(style: &LayerStyle, shape: &Region) -> Region {
    let scale = |r: Region, k: f32| Region { data: r.data.iter().map(|v| (v * k).clamp(0.0, 1.0)).collect(), ..r };
    match style {
        LayerStyle::DropShadow { opacity, angle_deg, distance, size, .. } => {
            let a = angle_deg.to_radians();
            let (dx, dy) = ((a.cos() * distance).round() as i64, (a.sin() * distance).round() as i64);
            scale(shape.shifted(dx, dy).blurred(*size), *opacity)
        }
        LayerStyle::OuterGlow { opacity, size, .. } => scale(shape.blurred(*size), *opacity),
        LayerStyle::Stroke { opacity, size, position, .. } => {
            let ring = match position {
                StrokePosition::Outside => sub(&shape.morph(*size, true), shape),
                StrokePosition::Inside => sub(shape, &shape.morph(*size, false)),
                StrokePosition::Center => sub(&shape.morph(size / 2.0, true), &shape.morph(size / 2.0, false)),
            };
            scale(ring, *opacity)
        }
        LayerStyle::ColorOverlay { opacity, .. } | LayerStyle::GradientOverlay { opacity, .. } => {
            scale(Region { x0: shape.x0, y0: shape.y0, w: shape.w, h: shape.h, data: shape.data.clone() }, *opacity)
        }
    }
}

fn sub(a: &Region, b: &Region) -> Region {
    Region {
        x0: a.x0,
        y0: a.y0,
        w: a.w,
        h: a.h,
        data: a.data.iter().zip(&b.data).map(|(x, y)| (x - y).max(0.0)).collect(),
    }
}

/// Couleur du dégradé au pixel (x, y), projetée sur l'axe `angle_deg` à travers un rectangle `(w, h)`.
pub fn gradient_at(from: Color, to: Color, angle_deg: f32, x: f64, y: f64, w: f64, h: f64) -> [f32; 3] {
    let a = f64::from(angle_deg).to_radians();
    let (ux, uy) = (a.cos(), a.sin());
    // Étendue du rectangle le long de l'axe : le dégradé va exactement du coin le plus bas au plus haut.
    let extent = (w * ux.abs() + h * uy.abs()).max(1.0);
    let t = (((x - w / 2.0) * ux + (y - h / 2.0) * uy) / extent + 0.5).clamp(0.0, 1.0) as f32;
    std::array::from_fn(|i| (f32::from(from[i]) * (1.0 - t) + f32::from(to[i]) * t) / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(w: usize, h: usize, x0: usize, y0: usize, side: usize) -> Region {
        let mut data = vec![0.0; w * h];
        for y in y0..y0 + side {
            for x in x0..x0 + side {
                data[y * w + x] = 1.0;
            }
        }
        Region { x0: 0, y0: 0, w, h, data }
    }

    #[test]
    fn blur_conserves_mass_and_is_symmetric() {
        let r = square(80, 80, 35, 35, 10);
        let b = r.blurred(6.0);
        let (m0, m1): (f32, f32) = (r.data.iter().sum(), b.data.iter().sum());
        assert!((m0 - m1).abs() < 0.5, "{m0} vs {m1}");
        assert!(b.at(40, 40) < 1.0 && b.at(40, 40) > 0.5);
        assert!((b.at(30, 40) - b.at(49, 40)).abs() < 1e-3, "flou symétrique");
        assert_eq!(r.blurred(0.0).data, r.data);
    }

    #[test]
    fn morphology_grows_and_shrinks_by_the_radius() {
        let r = square(40, 40, 15, 15, 10);
        let d = r.morph(3.0, true);
        assert_eq!(d.at(12, 20), 1.0);
        assert_eq!(d.at(11, 20), 0.0);
        let e = r.morph(2.0, false);
        assert_eq!(e.at(16, 20), 0.0);
        assert_eq!(e.at(17, 20), 1.0);
    }

    #[test]
    fn shadow_offsets_in_the_requested_direction() {
        let r = square(60, 60, 20, 20, 10);
        let style = LayerStyle::DropShadow { color: [0; 3], opacity: 1.0, angle_deg: 0.0, distance: 8.0, size: 0.0 };
        let a = effect_alpha(&style, &r);
        assert_eq!(a.at(35, 25), 1.0);
        assert_eq!(a.at(15, 25), 0.0);
        let down = LayerStyle::DropShadow { color: [0; 3], opacity: 0.5, angle_deg: 90.0, distance: 5.0, size: 0.0 };
        let a = effect_alpha(&down, &r);
        assert_eq!(a.at(25, 33), 0.5);
        assert_eq!(a.at(25, 17), 0.0);
    }

    #[test]
    fn stroke_positions_are_complementary() {
        let r = square(60, 60, 20, 20, 10);
        let mk = |p| LayerStyle::Stroke { color: [255; 3], opacity: 1.0, size: 2.0, position: p };
        let out = effect_alpha(&mk(StrokePosition::Outside), &r);
        let inn = effect_alpha(&mk(StrokePosition::Inside), &r);
        let mid = effect_alpha(&mk(StrokePosition::Center), &r);
        assert_eq!((out.at(19, 25), out.at(25, 25), out.at(17, 25)), (1.0, 0.0, 0.0));
        assert_eq!((inn.at(20, 25), inn.at(25, 25), inn.at(19, 25)), (1.0, 0.0, 0.0));
        assert_eq!((mid.at(19, 25), mid.at(20, 25), mid.at(25, 25)), (1.0, 1.0, 0.0));
    }

    #[test]
    fn gradient_runs_from_one_colour_to_the_other() {
        let a = gradient_at([0; 3], [255; 3], 0.0, 0.0, 5.0, 100.0, 10.0);
        let b = gradient_at([0; 3], [255; 3], 0.0, 99.0, 5.0, 100.0, 10.0);
        assert!(a[0] < 0.02 && b[0] > 0.98);
        let v = gradient_at([0; 3], [255; 3], 90.0, 5.0, 0.0, 10.0, 100.0);
        assert!(v[0] < 0.02);
    }

    #[test]
    fn sanitizing_bounds_every_number() {
        let wild =
            LayerStyle::DropShadow { color: [1, 2, 3], opacity: 9.0, angle_deg: f32::NAN, distance: 1e9, size: -5.0 };
        match wild.sanitized() {
            LayerStyle::DropShadow { opacity, angle_deg, distance, size, .. } => {
                assert_eq!((opacity, angle_deg, distance, size), (1.0, 0.0, MAX_DISTANCE, 0.0));
            }
            other => panic!("{other:?}"),
        }
        assert!(wild.sanitized().reach() <= 502);
    }

    #[test]
    fn styles_roundtrip_through_json() {
        let s = vec![
            LayerStyle::Stroke { color: [1, 2, 3], opacity: 0.5, size: 3.0, position: StrokePosition::Inside },
            LayerStyle::ColorOverlay { color: [9, 9, 9], opacity: 1.0, blend: BlendMode::Multiply },
        ];
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"kind\":\"stroke\"") && json.contains("\"multiply\""));
        assert_eq!(serde_json::from_str::<Vec<LayerStyle>>(&json).unwrap(), s);
    }
}
