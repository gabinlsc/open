//! Réglages de couleur non destructifs : chaque réglage est une fonction pure RGB → RGB (valeurs 0..1),
//! appliquée à ce qui est dessous par un calque de réglage (avec masque et opacité).
//!
//! Les paramètres numériques sont décrits par une table (`ParamSpec`) : l'interface génère ses curseurs
//! depuis cette table, et chaque valeur est bornée à l'application (un fichier ou un champ hostile ne peut
//! pas produire de NaN ni de dépassement).

use std::sync::Arc;

use serde::{Deserialize, Serialize};

pub type Rgb = [f32; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdjKind {
    BrightnessContrast,
    Exposure,
    Levels,
    Curves,
    HueSaturation,
    Vibrance,
    ColorBalance,
    BlackWhite,
    PhotoFilter,
    ChannelMixer,
    GradientMap,
    Lut3D,
    Invert,
    HdrToning,
    ColorMatch,
}

impl AdjKind {
    pub const ALL: [AdjKind; 15] = [
        AdjKind::BrightnessContrast,
        AdjKind::Exposure,
        AdjKind::Levels,
        AdjKind::Curves,
        AdjKind::HueSaturation,
        AdjKind::Vibrance,
        AdjKind::ColorBalance,
        AdjKind::BlackWhite,
        AdjKind::PhotoFilter,
        AdjKind::ChannelMixer,
        AdjKind::GradientMap,
        AdjKind::Lut3D,
        AdjKind::Invert,
        AdjKind::HdrToning,
        AdjKind::ColorMatch,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AdjKind::BrightnessContrast => "Luminosité / Contraste",
            AdjKind::Exposure => "Exposition",
            AdjKind::Levels => "Niveaux",
            AdjKind::Curves => "Courbes",
            AdjKind::HueSaturation => "Teinte / Saturation",
            AdjKind::Vibrance => "Vibrance",
            AdjKind::ColorBalance => "Balance des couleurs",
            AdjKind::BlackWhite => "Noir et blanc",
            AdjKind::PhotoFilter => "Filtre photo",
            AdjKind::ChannelMixer => "Mélangeur de canaux",
            AdjKind::GradientMap => "Courbes de dégradé",
            AdjKind::Lut3D => "LUT 3D",
            AdjKind::Invert => "Négatif",
            AdjKind::HdrToning => "Tonalité HDR",
            AdjKind::ColorMatch => "Correspondance de couleur",
        }
    }

    pub fn specs(self) -> &'static [ParamSpec] {
        match self {
            AdjKind::BrightnessContrast => &[
                ParamSpec { id: "brightness", label: "Luminosité", min: -1.0, max: 1.0, default: 0.0 },
                ParamSpec { id: "contrast", label: "Contraste", min: -1.0, max: 1.0, default: 0.0 },
            ],
            AdjKind::Exposure => &[
                ParamSpec { id: "ev", label: "Exposition (IL)", min: -5.0, max: 5.0, default: 0.0 },
                ParamSpec { id: "offset", label: "Décalage", min: -0.5, max: 0.5, default: 0.0 },
                ParamSpec { id: "gamma", label: "Gamma", min: 0.1, max: 5.0, default: 1.0 },
            ],
            AdjKind::Levels => &[
                ParamSpec { id: "in_black", label: "Entrée : noir", min: 0.0, max: 1.0, default: 0.0 },
                ParamSpec { id: "in_white", label: "Entrée : blanc", min: 0.0, max: 1.0, default: 1.0 },
                ParamSpec { id: "gamma", label: "Gamma", min: 0.1, max: 5.0, default: 1.0 },
                ParamSpec { id: "out_black", label: "Sortie : noir", min: 0.0, max: 1.0, default: 0.0 },
                ParamSpec { id: "out_white", label: "Sortie : blanc", min: 0.0, max: 1.0, default: 1.0 },
            ],
            AdjKind::Curves => &[],
            AdjKind::HueSaturation => &[
                ParamSpec { id: "hue", label: "Teinte (°)", min: -180.0, max: 180.0, default: 0.0 },
                ParamSpec { id: "saturation", label: "Saturation", min: -1.0, max: 1.0, default: 0.0 },
                ParamSpec { id: "lightness", label: "Clarté", min: -1.0, max: 1.0, default: 0.0 },
            ],
            AdjKind::Vibrance => &[
                ParamSpec { id: "vibrance", label: "Vibrance", min: -1.0, max: 1.0, default: 0.0 },
                ParamSpec { id: "saturation", label: "Saturation", min: -1.0, max: 1.0, default: 0.0 },
            ],
            AdjKind::ColorBalance => &[
                ParamSpec { id: "sh_cyan_red", label: "Ombres : cyan ↔ rouge", min: -1.0, max: 1.0, default: 0.0 },
                ParamSpec {
                    id: "sh_magenta_green",
                    label: "Ombres : magenta ↔ vert",
                    min: -1.0,
                    max: 1.0,
                    default: 0.0,
                },
                ParamSpec { id: "sh_yellow_blue", label: "Ombres : jaune ↔ bleu", min: -1.0, max: 1.0, default: 0.0 },
                ParamSpec {
                    id: "mid_cyan_red",
                    label: "Tons moyens : cyan ↔ rouge",
                    min: -1.0,
                    max: 1.0,
                    default: 0.0,
                },
                ParamSpec {
                    id: "mid_magenta_green",
                    label: "Tons moyens : magenta ↔ vert",
                    min: -1.0,
                    max: 1.0,
                    default: 0.0,
                },
                ParamSpec {
                    id: "mid_yellow_blue",
                    label: "Tons moyens : jaune ↔ bleu",
                    min: -1.0,
                    max: 1.0,
                    default: 0.0,
                },
                ParamSpec {
                    id: "hi_cyan_red",
                    label: "Hautes lumières : cyan ↔ rouge",
                    min: -1.0,
                    max: 1.0,
                    default: 0.0,
                },
                ParamSpec {
                    id: "hi_magenta_green",
                    label: "Hautes lumières : magenta ↔ vert",
                    min: -1.0,
                    max: 1.0,
                    default: 0.0,
                },
                ParamSpec {
                    id: "hi_yellow_blue",
                    label: "Hautes lumières : jaune ↔ bleu",
                    min: -1.0,
                    max: 1.0,
                    default: 0.0,
                },
                ParamSpec {
                    id: "preserve_luminosity",
                    label: "Conserver la luminosité (0/1)",
                    min: 0.0,
                    max: 1.0,
                    default: 1.0,
                },
            ],
            AdjKind::BlackWhite => &[
                ParamSpec { id: "reds", label: "Rouges", min: -1.0, max: 2.0, default: 0.4 },
                ParamSpec { id: "yellows", label: "Jaunes", min: -1.0, max: 2.0, default: 0.6 },
                ParamSpec { id: "greens", label: "Verts", min: -1.0, max: 2.0, default: 0.4 },
                ParamSpec { id: "cyans", label: "Cyans", min: -1.0, max: 2.0, default: 0.6 },
                ParamSpec { id: "blues", label: "Bleus", min: -1.0, max: 2.0, default: 0.2 },
                ParamSpec { id: "magentas", label: "Magentas", min: -1.0, max: 2.0, default: 0.8 },
            ],
            AdjKind::PhotoFilter => &[
                ParamSpec { id: "red", label: "Filtre : rouge", min: 0.0, max: 1.0, default: 0.93 },
                ParamSpec { id: "green", label: "Filtre : vert", min: 0.0, max: 1.0, default: 0.53 },
                ParamSpec { id: "blue", label: "Filtre : bleu", min: 0.0, max: 1.0, default: 0.0 },
                ParamSpec { id: "density", label: "Densité", min: 0.0, max: 1.0, default: 0.25 },
                ParamSpec {
                    id: "preserve_luminosity",
                    label: "Conserver la luminosité (0/1)",
                    min: 0.0,
                    max: 1.0,
                    default: 1.0,
                },
            ],
            AdjKind::ChannelMixer => &[
                ParamSpec { id: "rr", label: "Rouge ← rouge", min: -2.0, max: 2.0, default: 1.0 },
                ParamSpec { id: "rg", label: "Rouge ← vert", min: -2.0, max: 2.0, default: 0.0 },
                ParamSpec { id: "rb", label: "Rouge ← bleu", min: -2.0, max: 2.0, default: 0.0 },
                ParamSpec { id: "gr", label: "Vert ← rouge", min: -2.0, max: 2.0, default: 0.0 },
                ParamSpec { id: "gg", label: "Vert ← vert", min: -2.0, max: 2.0, default: 1.0 },
                ParamSpec { id: "gb", label: "Vert ← bleu", min: -2.0, max: 2.0, default: 0.0 },
                ParamSpec { id: "br", label: "Bleu ← rouge", min: -2.0, max: 2.0, default: 0.0 },
                ParamSpec { id: "bg", label: "Bleu ← vert", min: -2.0, max: 2.0, default: 0.0 },
                ParamSpec { id: "bb", label: "Bleu ← bleu", min: -2.0, max: 2.0, default: 1.0 },
                ParamSpec { id: "monochrome", label: "Monochrome (0/1)", min: 0.0, max: 1.0, default: 0.0 },
            ],
            AdjKind::GradientMap | AdjKind::Invert => &[],
            AdjKind::Lut3D => &[ParamSpec { id: "intensity", label: "Intensité", min: 0.0, max: 1.0, default: 1.0 }],
            AdjKind::HdrToning => &[
                ParamSpec { id: "exposure", label: "Exposition (IL)", min: -3.0, max: 3.0, default: 0.0 },
                ParamSpec { id: "white_point", label: "Point blanc", min: 0.5, max: 8.0, default: 2.0 },
                ParamSpec { id: "gamma", label: "Gamma", min: 0.3, max: 3.0, default: 1.0 },
                ParamSpec { id: "saturation", label: "Saturation", min: -1.0, max: 1.0, default: 0.0 },
            ],
            AdjKind::ColorMatch => &[ParamSpec { id: "fade", label: "Intensité", min: 0.0, max: 1.0, default: 1.0 }],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

/// Table de correspondance 3D (fichier `.cube`).
#[derive(Debug, Clone, PartialEq)]
pub struct Lut3D {
    pub size: usize,
    /// `size³` entrées, l'indice rouge variant le plus vite (ordre du format `.cube`).
    pub data: Vec<Rgb>,
    pub domain_min: Rgb,
    pub domain_max: Rgb,
}

pub const MAX_LUT_SIZE: usize = 65;

impl Lut3D {
    /// Lit un fichier `.cube` ; refuse tout ce qui est incohérent plutôt que de deviner.
    pub fn parse_cube(text: &str) -> Result<Lut3D, String> {
        let (mut size, mut dmin, mut dmax) = (0usize, [0.0f32; 3], [1.0f32; 3]);
        let mut data: Vec<Rgb> = Vec::new();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("TITLE") {
                continue;
            }
            let mut it = line.split_whitespace();
            let key = it.next().unwrap_or("");
            let err = |what: &str| format!("ligne {} : {what}", n + 1);
            let floats = |it: std::str::SplitWhitespace<'_>| -> Option<Vec<f32>> {
                it.map(|t| t.parse::<f32>().ok().filter(|v| v.is_finite())).collect()
            };
            match key {
                "LUT_3D_SIZE" => {
                    size = it.next().and_then(|t| t.parse().ok()).ok_or_else(|| err("taille illisible"))?;
                    if !(2..=MAX_LUT_SIZE).contains(&size) {
                        return Err(err(&format!("taille {size} hors de 2..={MAX_LUT_SIZE}")));
                    }
                }
                "LUT_1D_SIZE" => return Err(err("les LUT 1D ne sont pas prises en charge")),
                "DOMAIN_MIN" | "DOMAIN_MAX" => {
                    let v = floats(it).filter(|v| v.len() == 3).ok_or_else(|| err("domaine illisible"))?;
                    if key == "DOMAIN_MIN" {
                        dmin = [v[0], v[1], v[2]];
                    } else {
                        dmax = [v[0], v[1], v[2]];
                    }
                }
                _ => {
                    let mut all = vec![key];
                    all.extend(it);
                    let v: Option<Vec<f32>> =
                        all.iter().map(|t| t.parse::<f32>().ok().filter(|v| v.is_finite())).collect();
                    match v.filter(|v| v.len() == 3) {
                        Some(v) => data.push([v[0], v[1], v[2]]),
                        None => return Err(err("entrée illisible")),
                    }
                }
            }
        }
        if size == 0 {
            return Err("LUT_3D_SIZE absent".into());
        }
        if data.len() != size * size * size {
            return Err(format!("{} entrées lues, {} attendues", data.len(), size * size * size));
        }
        if (0..3).any(|i| dmax[i] <= dmin[i]) {
            return Err("domaine invalide".into());
        }
        Ok(Lut3D { size, data, domain_min: dmin, domain_max: dmax })
    }

    /// Interpolation trilinéaire.
    pub fn sample(&self, c: Rgb) -> Rgb {
        let n = (self.size - 1) as f32;
        let p: [f32; 3] = std::array::from_fn(|i| {
            (((c[i] - self.domain_min[i]) / (self.domain_max[i] - self.domain_min[i])).clamp(0.0, 1.0)) * n
        });
        let i0: [usize; 3] = std::array::from_fn(|i| (p[i].floor() as usize).min(self.size - 2));
        let f: [f32; 3] = std::array::from_fn(|i| p[i] - i0[i] as f32);
        let at = |r: usize, g: usize, b: usize| self.data[(b * self.size + g) * self.size + r];
        let mut out = [0.0; 3];
        for dz in 0..2 {
            for dy in 0..2 {
                for dx in 0..2 {
                    let w = (if dx == 1 { f[0] } else { 1.0 - f[0] })
                        * (if dy == 1 { f[1] } else { 1.0 - f[1] })
                        * (if dz == 1 { f[2] } else { 1.0 - f[2] });
                    let v = at(i0[0] + dx, i0[1] + dy, i0[2] + dz);
                    for k in 0..3 {
                        out[k] += v[k] * w;
                    }
                }
            }
        }
        out
    }

    pub fn to_cube(&self) -> String {
        let mut s = format!(
            "LUT_3D_SIZE {}\nDOMAIN_MIN {} {} {}\nDOMAIN_MAX {} {} {}\n",
            self.size,
            self.domain_min[0],
            self.domain_min[1],
            self.domain_min[2],
            self.domain_max[0],
            self.domain_max[1],
            self.domain_max[2]
        );
        for v in &self.data {
            s += &format!("{} {} {}\n", v[0], v[1], v[2]);
        }
        s
    }

    pub fn identity(size: usize) -> Lut3D {
        let n = (size - 1) as f32;
        let mut data = Vec::with_capacity(size * size * size);
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    data.push([r as f32 / n, g as f32 / n, b as f32 / n]);
                }
            }
        }
        Lut3D { size, data, domain_min: [0.0; 3], domain_max: [1.0; 3] }
    }
}

/// Données non numériques de certains réglages.
#[derive(Debug, Clone, PartialEq)]
pub enum Extra {
    None,
    /// Points de contrôle (x, y) en 0..1 : courbe commune puis courbes par canal.
    Curves {
        master: Vec<(f32, f32)>,
        r: Vec<(f32, f32)>,
        g: Vec<(f32, f32)>,
        b: Vec<(f32, f32)>,
    },
    Stops(Vec<(f32, [u8; 3])>),
    Lut(Arc<Lut3D>),
    /// Statistiques moyenne / écart-type par canal de l'image courante et de la référence.
    Match {
        src_mean: Rgb,
        src_std: Rgb,
        ref_mean: Rgb,
        ref_std: Rgb,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Adjustment {
    pub kind: AdjKind,
    pub values: Vec<f32>,
    pub extra: Extra,
}

impl Adjustment {
    pub fn new(kind: AdjKind) -> Self {
        let extra = match kind {
            AdjKind::Curves => Extra::Curves {
                master: identity_curve(),
                r: identity_curve(),
                g: identity_curve(),
                b: identity_curve(),
            },
            AdjKind::GradientMap => Extra::Stops(vec![(0.0, [0, 0, 0]), (1.0, [255, 255, 255])]),
            AdjKind::Lut3D => Extra::Lut(Arc::new(Lut3D::identity(2))),
            AdjKind::ColorMatch => {
                Extra::Match { src_mean: [0.5; 3], src_std: [0.2; 3], ref_mean: [0.5; 3], ref_std: [0.2; 3] }
            }
            _ => Extra::None,
        };
        Self { kind, values: kind.specs().iter().map(|s| s.default).collect(), extra }
    }

    /// Ramène chaque valeur dans sa plage (NaN → défaut) et les points/arrêts dans des limites raisonnables.
    pub fn sanitized(&self) -> Adjustment {
        let specs = self.kind.specs();
        let values = specs
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let v = self.values.get(i).copied().unwrap_or(s.default);
                if v.is_finite() { v.clamp(s.min, s.max) } else { s.default }
            })
            .collect();
        let clean = |pts: &[(f32, f32)]| -> Vec<(f32, f32)> {
            let mut v: Vec<(f32, f32)> = pts
                .iter()
                .filter(|(x, y)| x.is_finite() && y.is_finite())
                .map(|(x, y)| (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)))
                .take(32)
                .collect();
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            v.dedup_by(|b, a| (a.0 - b.0).abs() < 1e-4);
            if v.len() < 2 { identity_curve() } else { v }
        };
        let extra = match &self.extra {
            Extra::Curves { master, r, g, b } => {
                Extra::Curves { master: clean(master), r: clean(r), g: clean(g), b: clean(b) }
            }
            Extra::Stops(s) => {
                let mut v: Vec<_> =
                    s.iter().filter(|(p, _)| p.is_finite()).map(|(p, c)| (p.clamp(0.0, 1.0), *c)).take(32).collect();
                v.sort_by(|a, b| a.0.total_cmp(&b.0));
                if v.len() < 2 { Extra::Stops(vec![(0.0, [0; 3]), (1.0, [255; 3])]) } else { Extra::Stops(v) }
            }
            Extra::Match { src_mean, src_std, ref_mean, ref_std } => {
                let f = |a: &Rgb, lo: f32, hi: f32, d: f32| -> Rgb {
                    a.map(|v| if v.is_finite() { v.clamp(lo, hi) } else { d })
                };
                Extra::Match {
                    src_mean: f(src_mean, 0.0, 1.0, 0.5),
                    src_std: f(src_std, 0.001, 1.0, 0.2),
                    ref_mean: f(ref_mean, 0.0, 1.0, 0.5),
                    ref_std: f(ref_std, 0.001, 1.0, 0.2),
                }
            }
            other => other.clone(),
        };
        Adjustment { kind: self.kind, values, extra }
    }

    pub fn value(&self, id: &str) -> f32 {
        self.kind.specs().iter().position(|s| s.id == id).and_then(|i| self.values.get(i).copied()).unwrap_or(0.0)
    }

    /// Fixe un paramètre numérique par son identifiant ; `false` s'il n'existe pas pour ce réglage.
    pub fn set(&mut self, id: &str, v: f32) -> bool {
        let Some(i) = self.kind.specs().iter().position(|s| s.id == id) else { return false };
        if self.values.len() <= i {
            self.values.resize(self.kind.specs().len(), 0.0);
        }
        self.values[i] = v;
        *self = self.sanitized();
        true
    }

    pub fn prepare(&self) -> Prepared {
        Prepared::new(&self.sanitized())
    }
}

pub fn identity_curve() -> Vec<(f32, f32)> {
    vec![(0.0, 0.0), (1.0, 1.0)]
}

/// Table de 1024 valeurs d'une courbe passant par `pts` (interpolation cubique monotone de Fritsch–Carlson :
/// pas de dépassement ni d'oscillation entre les points de contrôle).
fn curve_lut(pts: &[(f32, f32)]) -> Vec<f32> {
    const N: usize = 1024;
    let n = pts.len();
    let mut d = vec![0.0f32; n - 1];
    let mut h = vec![0.0f32; n - 1];
    for i in 0..n - 1 {
        h[i] = pts[i + 1].0 - pts[i].0;
        d[i] = (pts[i + 1].1 - pts[i].1) / h[i];
    }
    let mut m = vec![0.0f32; n];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for i in 1..n - 1 {
        m[i] = if d[i - 1] * d[i] <= 0.0 { 0.0 } else { (d[i - 1] + d[i]) / 2.0 };
    }
    for i in 0..n - 1 {
        if d[i] == 0.0 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
        } else {
            let (a, b) = (m[i] / d[i], m[i + 1] / d[i]);
            let s = a * a + b * b;
            if s > 9.0 {
                let t = 3.0 / s.sqrt();
                m[i] = t * a * d[i];
                m[i + 1] = t * b * d[i];
            }
        }
    }
    (0..N)
        .map(|k| {
            let x = k as f32 / (N - 1) as f32;
            if x <= pts[0].0 {
                return pts[0].1;
            }
            if x >= pts[n - 1].0 {
                return pts[n - 1].1;
            }
            let i = pts.windows(2).position(|w| x <= w[1].0).unwrap_or(n - 2);
            let t = (x - pts[i].0) / h[i];
            let (t2, t3) = (t * t, t * t * t);
            ((2.0 * t3 - 3.0 * t2 + 1.0) * pts[i].1
                + (t3 - 2.0 * t2 + t) * h[i] * m[i]
                + (-2.0 * t3 + 3.0 * t2) * pts[i + 1].1
                + (t3 - t2) * h[i] * m[i + 1])
                .clamp(0.0, 1.0)
        })
        .collect()
}

fn lut_at(lut: &[f32], x: f32) -> f32 {
    let p = x.clamp(0.0, 1.0) * (lut.len() - 1) as f32;
    let i = (p.floor() as usize).min(lut.len() - 2);
    lut[i] + (lut[i + 1] - lut[i]) * (p - i as f32)
}

fn luma(c: Rgb) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

fn rgb_to_hsl(c: Rgb) -> (f32, f32, f32) {
    let (mx, mn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    if d < 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs()).max(1e-6);
    let h = if mx == c[0] {
        ((c[1] - c[2]) / d).rem_euclid(6.0)
    } else if mx == c[1] {
        (c[2] - c[0]) / d + 2.0
    } else {
        (c[0] - c[1]) / d + 4.0
    };
    (h * 60.0, s.clamp(0.0, 1.0), l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Rgb {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [r + m, g + m, b + m]
}

/// Réglage prêt à l'emploi : tables précalculées une fois par tuile.
pub struct Prepared {
    adj: Adjustment,
    curves: Option<[Vec<f32>; 4]>,
}

impl Prepared {
    fn new(adj: &Adjustment) -> Self {
        let curves = match &adj.extra {
            Extra::Curves { master, r, g, b } => Some([curve_lut(master), curve_lut(r), curve_lut(g), curve_lut(b)]),
            _ => None,
        };
        Prepared { adj: adj.clone(), curves }
    }

    /// Applique le réglage à une couleur (0..1) ; le résultat est borné à 0..1.
    pub fn apply(&self, c: Rgb) -> Rgb {
        let a = &self.adj;
        let v = |id: &str| a.value(id);
        let out: Rgb = match a.kind {
            AdjKind::BrightnessContrast => {
                let k = v("contrast");
                let factor = if k >= 0.0 { 1.0 / (1.0 - k).max(0.01) } else { 1.0 + k };
                c.map(|x| (x - 0.5) * factor + 0.5 + v("brightness"))
            }
            AdjKind::Exposure => {
                let gain = 2f32.powf(v("ev"));
                c.map(|x| (x * gain + v("offset")).max(0.0).powf(1.0 / v("gamma")))
            }
            AdjKind::Levels => {
                let (ib, iw) = (v("in_black"), v("in_white").max(v("in_black") + 0.001));
                let (ob, ow) = (v("out_black"), v("out_white"));
                c.map(|x| {
                    let t = ((x - ib) / (iw - ib)).clamp(0.0, 1.0).powf(1.0 / v("gamma"));
                    ob + t * (ow - ob)
                })
            }
            AdjKind::Curves => match &self.curves {
                Some([m, r, g, b]) => {
                    [lut_at(r, lut_at(m, c[0])), lut_at(g, lut_at(m, c[1])), lut_at(b, lut_at(m, c[2]))]
                }
                None => c,
            },
            AdjKind::HueSaturation => {
                let (h, s, l) = rgb_to_hsl(c);
                let sat = v("saturation");
                let s2 = if sat >= 0.0 { s + (1.0 - s) * sat } else { s * (1.0 + sat) };
                let light = v("lightness");
                let l2 = if light >= 0.0 { l + (1.0 - l) * light } else { l * (1.0 + light) };
                hsl_to_rgb(h + v("hue"), s2.clamp(0.0, 1.0), l2.clamp(0.0, 1.0))
            }
            AdjKind::Vibrance => {
                let avg = (c[0] + c[1] + c[2]) / 3.0;
                let sat = c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2]);
                // La vibrance agit surtout sur les couleurs peu saturées ; la saturation, sur toutes.
                let k = (1.0 + v("vibrance") * (1.0 - sat)) * (1.0 + v("saturation"));
                c.map(|x| avg + (x - avg) * k)
            }
            AdjKind::ColorBalance => {
                let l = luma(c).clamp(0.0, 1.0);
                let (ws, wh) = ((1.0 - l).powi(2), l * l);
                let wm = (1.0 - ws - wh).max(0.0);
                let shift = |i: usize| {
                    let names = [
                        ["sh_cyan_red", "mid_cyan_red", "hi_cyan_red"],
                        ["sh_magenta_green", "mid_magenta_green", "hi_magenta_green"],
                        ["sh_yellow_blue", "mid_yellow_blue", "hi_yellow_blue"],
                    ][i];
                    (v(names[0]) * ws + v(names[1]) * wm + v(names[2]) * wh) * 0.5
                };
                let moved = [c[0] + shift(0), c[1] + shift(1), c[2] + shift(2)];
                if v("preserve_luminosity") >= 0.5 {
                    let k = luma(c) / luma(moved).max(1e-4);
                    moved.map(|x| x * k)
                } else {
                    moved
                }
            }
            AdjKind::BlackWhite => {
                let (h, s, _) = rgb_to_hsl(c);
                let (mx, mn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
                let w = [v("reds"), v("yellows"), v("greens"), v("cyans"), v("blues"), v("magentas")];
                let pos = h.rem_euclid(360.0) / 60.0;
                let (i, f) = (pos.floor() as usize % 6, pos - pos.floor());
                let weight = if s < 1e-6 { 0.5 } else { w[i] * (1.0 - f) + w[(i + 1) % 6] * f };
                let g = mn + (mx - mn) * weight;
                [g, g, g]
            }
            AdjKind::PhotoFilter => {
                let f = [v("red"), v("green"), v("blue")];
                let d = v("density");
                let filtered = [
                    c[0] * (1.0 - d + d * f[0] * 2.0),
                    c[1] * (1.0 - d + d * f[1] * 2.0),
                    c[2] * (1.0 - d + d * f[2] * 2.0),
                ];
                if v("preserve_luminosity") >= 0.5 {
                    let k = luma(c) / luma(filtered).max(1e-4);
                    filtered.map(|x| x * k)
                } else {
                    filtered
                }
            }
            AdjKind::ChannelMixer => {
                let row = |a: &str, b: &str, d: &str| v(a) * c[0] + v(b) * c[1] + v(d) * c[2];
                let (r, g, b) = (row("rr", "rg", "rb"), row("gr", "gg", "gb"), row("br", "bg", "bb"));
                if v("monochrome") >= 0.5 { [r, r, r] } else { [r, g, b] }
            }
            AdjKind::GradientMap => match &a.extra {
                Extra::Stops(stops) => {
                    let t = luma(c).clamp(0.0, 1.0);
                    let col = |p: [u8; 3]| p.map(|x| f32::from(x) / 255.0);
                    if t <= stops[0].0 {
                        col(stops[0].1)
                    } else {
                        let mut out = col(stops[stops.len() - 1].1);
                        for w in stops.windows(2) {
                            if t <= w[1].0 {
                                let k = (t - w[0].0) / (w[1].0 - w[0].0).max(1e-6);
                                let (x, y) = (col(w[0].1), col(w[1].1));
                                out = std::array::from_fn(|i| x[i] + (y[i] - x[i]) * k);
                                break;
                            }
                        }
                        out
                    }
                }
                _ => c,
            },
            AdjKind::Lut3D => match &a.extra {
                Extra::Lut(lut) => {
                    let m = lut.sample(c);
                    let k = v("intensity");
                    std::array::from_fn(|i| c[i] + (m[i] - c[i]) * k)
                }
                _ => c,
            },
            AdjKind::Invert => c.map(|x| 1.0 - x),
            AdjKind::HdrToning => {
                let l = luma(c);
                let exposed = l * 2f32.powf(v("exposure"));
                let w = v("white_point");
                // Réinhard étendu : comprime les hautes lumières sans toucher aux ombres.
                let mapped = (exposed * (1.0 + exposed / (w * w)) / (1.0 + exposed)).powf(1.0 / v("gamma"));
                let k = if l > 1e-5 { mapped / l } else { 1.0 };
                let lum_after = luma(c.map(|x| x * k));
                let s = 1.0 + v("saturation");
                c.map(|x| lum_after + (x * k - lum_after) * s)
            }
            AdjKind::ColorMatch => match &a.extra {
                Extra::Match { src_mean, src_std, ref_mean, ref_std } => {
                    let fade = v("fade");
                    std::array::from_fn(|i| {
                        let matched = (c[i] - src_mean[i]) * (ref_std[i] / src_std[i]) + ref_mean[i];
                        c[i] + (matched - c[i]) * fade
                    })
                }
                _ => c,
            },
        };
        out.map(|x| if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(kind: AdjKind, set: &[(&str, f32)], c: Rgb) -> Rgb {
        let mut a = Adjustment::new(kind);
        for (id, v) in set {
            assert!(a.set(id, *v), "{id}");
        }
        a.prepare().apply(c)
    }

    fn near(a: Rgb, b: Rgb, tol: f32) {
        for i in 0..3 {
            assert!((a[i] - b[i]).abs() <= tol, "{a:?} ≉ {b:?}");
        }
    }

    #[test]
    fn defaults_are_identity_for_the_neutral_adjustments() {
        let c = [0.2, 0.5, 0.8];
        for kind in [
            AdjKind::BrightnessContrast,
            AdjKind::Exposure,
            AdjKind::Levels,
            AdjKind::Curves,
            AdjKind::HueSaturation,
            AdjKind::Vibrance,
            AdjKind::ColorBalance,
            AdjKind::ChannelMixer,
            AdjKind::Lut3D,
        ] {
            near(Adjustment::new(kind).prepare().apply(c), c, 0.01);
        }
        near(Adjustment::new(AdjKind::Invert).prepare().apply(c), [0.8, 0.5, 0.2], 1e-6);
    }

    #[test]
    fn exposure_levels_and_brightness_contrast() {
        near(apply(AdjKind::Exposure, &[("ev", 1.0)], [0.25; 3]), [0.5; 3], 1e-5);
        near(apply(AdjKind::Exposure, &[("ev", -1.0)], [0.8; 3]), [0.4; 3], 1e-5);
        near(apply(AdjKind::Levels, &[("in_black", 0.2), ("in_white", 0.8)], [0.5; 3]), [0.5; 3], 1e-5);
        near(apply(AdjKind::Levels, &[("in_black", 0.2), ("in_white", 0.8)], [0.2; 3]), [0.0; 3], 1e-5);
        near(apply(AdjKind::Levels, &[("out_black", 0.25), ("out_white", 0.75)], [1.0; 3]), [0.75; 3], 1e-5);
        assert!(apply(AdjKind::Levels, &[("gamma", 2.0)], [0.25; 3])[0] > 0.25, "gamma > 1 éclaircit les tons moyens");
        near(apply(AdjKind::BrightnessContrast, &[("brightness", 0.2)], [0.5; 3]), [0.7; 3], 1e-5);
        let hi = apply(AdjKind::BrightnessContrast, &[("contrast", 0.5)], [0.75; 3])[0];
        let lo = apply(AdjKind::BrightnessContrast, &[("contrast", 0.5)], [0.25; 3])[0];
        assert!(hi > 0.75 && lo < 0.25, "le contraste écarte les valeurs du gris moyen");
    }

    #[test]
    fn curves_pass_through_control_points_and_stay_monotone() {
        let mut a = Adjustment::new(AdjKind::Curves);
        a.extra = Extra::Curves {
            master: vec![(0.0, 0.0), (0.25, 0.4), (0.75, 0.6), (1.0, 1.0)],
            r: identity_curve(),
            g: identity_curve(),
            b: identity_curve(),
        };
        let p = a.prepare();
        for (x, y) in [(0.0, 0.0), (0.25, 0.4), (0.75, 0.6), (1.0, 1.0)] {
            assert!((p.apply([x; 3])[0] - y).abs() < 0.01, "({x}, {y})");
        }
        let ys: Vec<f32> = (0..=100).map(|i| p.apply([i as f32 / 100.0; 3])[0]).collect();
        assert!(ys.windows(2).all(|w| w[1] >= w[0] - 1e-4), "monotone, sans oscillation");
        // Une courbe par canal ne touche que ce canal.
        a.extra = Extra::Curves {
            master: identity_curve(),
            r: vec![(0.0, 0.0), (1.0, 0.5)],
            g: identity_curve(),
            b: identity_curve(),
        };
        near(a.prepare().apply([1.0, 1.0, 1.0]), [0.5, 1.0, 1.0], 0.01);
        // Points hostiles : retombe sur l'identité plutôt que de planter.
        a.extra = Extra::Curves { master: vec![(f32::NAN, 0.0)], r: vec![], g: identity_curve(), b: identity_curve() };
        near(a.prepare().apply([0.3; 3]), [0.3; 3], 0.01);
    }

    #[test]
    fn hue_saturation_vibrance_and_black_white() {
        near(apply(AdjKind::HueSaturation, &[("hue", 120.0)], [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0], 0.01);
        let grey = apply(AdjKind::HueSaturation, &[("saturation", -1.0)], [0.8, 0.4, 0.2]);
        near(grey, [grey[0]; 3], 0.01);
        assert!(
            apply(AdjKind::HueSaturation, &[("lightness", 1.0)], [0.2, 0.4, 0.6])[0] > 0.99,
            "clarté maximale = blanc"
        );
        let dull = [0.5, 0.45, 0.4];
        let vivid = [1.0, 0.0, 0.0];
        let gain = |c: Rgb| (apply(AdjKind::Vibrance, &[("vibrance", 1.0)], c)[0] - c[0]).abs();
        assert!(gain(dull) > gain(vivid), "la vibrance agit davantage sur les couleurs ternes");
        let bw = apply(AdjKind::BlackWhite, &[], [1.0, 0.0, 0.0]);
        near(bw, [0.4; 3], 0.01);
        let blue = apply(AdjKind::BlackWhite, &[("blues", 1.0)], [0.0, 0.0, 1.0]);
        near(blue, [1.0; 3], 0.01);
        near(apply(AdjKind::BlackWhite, &[], [0.3; 3]), [0.3; 3], 0.01);
    }

    #[test]
    fn color_balance_filter_and_mixer() {
        let red = apply(AdjKind::ColorBalance, &[("mid_cyan_red", 1.0), ("preserve_luminosity", 0.0)], [0.5; 3]);
        assert!(red[0] > 0.5 && red[1] == 0.5);
        let keep = apply(AdjKind::ColorBalance, &[("mid_cyan_red", 1.0), ("preserve_luminosity", 1.0)], [0.5; 3]);
        assert!((luma(keep) - 0.5).abs() < 0.02, "luminosité conservée");
        let warm = apply(AdjKind::PhotoFilter, &[("density", 1.0), ("preserve_luminosity", 0.0)], [0.5; 3]);
        assert!(warm[0] > warm[2], "filtre chaud : plus de rouge que de bleu");
        near(apply(AdjKind::PhotoFilter, &[("density", 0.0)], [0.4, 0.5, 0.6]), [0.4, 0.5, 0.6], 1e-5);
        near(
            apply(AdjKind::ChannelMixer, &[("rr", 0.0), ("rg", 1.0), ("gg", 0.0), ("gr", 1.0)], [0.2, 0.6, 0.9]),
            [0.6, 0.2, 0.9],
            1e-5,
        );
        let mono = apply(AdjKind::ChannelMixer, &[("monochrome", 1.0), ("rr", 0.0), ("rg", 1.0)], [0.2, 0.6, 0.9]);
        near(mono, [0.6; 3], 1e-5);
    }

    #[test]
    fn gradient_map_hdr_and_colour_match() {
        let mut g = Adjustment::new(AdjKind::GradientMap);
        g.extra = Extra::Stops(vec![(0.0, [0, 0, 255]), (1.0, [255, 0, 0])]);
        near(g.prepare().apply([0.0; 3]), [0.0, 0.0, 1.0], 0.01);
        near(g.prepare().apply([1.0; 3]), [1.0, 0.0, 0.0], 0.01);
        let mid = g.prepare().apply([0.5; 3]);
        assert!(mid[0] > 0.4 && mid[2] > 0.4);
        let compressed = apply(AdjKind::HdrToning, &[("exposure", 2.0), ("white_point", 1.0)], [0.9; 3])[0];
        assert!(compressed <= 1.0 && compressed > 0.5);
        let m = Adjustment {
            kind: AdjKind::ColorMatch,
            values: vec![1.0],
            extra: Extra::Match { src_mean: [0.4; 3], src_std: [0.1; 3], ref_mean: [0.6; 3], ref_std: [0.2; 3] },
        };
        near(m.prepare().apply([0.4; 3]), [0.6; 3], 1e-5);
        near(m.prepare().apply([0.5; 3]), [0.8; 3], 1e-5);
    }

    #[test]
    fn cube_files_parse_sample_roundtrip_and_reject_garbage() {
        let id = Lut3D::identity(5);
        let parsed = Lut3D::parse_cube(&id.to_cube()).unwrap();
        assert_eq!(parsed, id);
        near(parsed.sample([0.3, 0.6, 0.9]), [0.3, 0.6, 0.9], 1e-4);
        // LUT qui échange rouge et bleu.
        let mut swap = Lut3D::identity(2);
        for e in &mut swap.data {
            e.swap(0, 2);
        }
        near(swap.sample([1.0, 0.0, 0.0]), [0.0, 0.0, 1.0], 1e-5);
        let text =
            "# commentaire\nTITLE \"x\"\nLUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n";
        assert!(Lut3D::parse_cube(text).is_ok());
        for bad in [
            "",
            "LUT_3D_SIZE 2\n0 0 0\n",
            "LUT_3D_SIZE 99\n",
            "LUT_3D_SIZE 2\nDOMAIN_MIN 1 1 1\nDOMAIN_MAX 0 0 0\n0 0 0\n0 0 0\n0 0 0\n0 0 0\n0 0 0\n0 0 0\n0 0 0\n0 0 0\n",
            "LUT_1D_SIZE 4\n",
            "LUT_3D_SIZE 2\nx y z\n",
        ] {
            assert!(Lut3D::parse_cube(bad).is_err(), "{bad:?}");
        }
        let mut a = Adjustment::new(AdjKind::Lut3D);
        a.extra = Extra::Lut(Arc::new(swap));
        a.set("intensity", 0.5);
        near(a.prepare().apply([1.0, 0.0, 0.0]), [0.5, 0.0, 0.5], 1e-5);
    }

    #[test]
    fn hostile_values_are_bounded_and_unknown_params_refused() {
        let mut a = Adjustment::new(AdjKind::Exposure);
        assert!(!a.set("n'existe_pas", 1.0));
        a.values = vec![f32::NAN, f32::INFINITY, -9.0];
        let s = a.sanitized();
        assert_eq!(s.values, vec![0.0, 0.0, 0.1], "NaN et infini → défaut ; hors plage → borne");
        let out = a.prepare().apply([f32::NAN, 0.5, 2.0]);
        assert!(out.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        for kind in AdjKind::ALL {
            let p = Adjustment::new(kind).prepare();
            for c in [[0.0; 3], [1.0; 3], [0.3, 0.7, 0.1]] {
                assert!(p.apply(c).iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)), "{kind:?}");
            }
            assert!(!kind.label().is_empty());
        }
    }
}
