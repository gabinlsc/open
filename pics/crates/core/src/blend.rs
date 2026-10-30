//! Les 27 modes de fusion, implémentation de référence en CPU.
//!
//! Les formules suivent la spécification W3C « Compositing and Blending Level 1 » pour les modes
//! qu'elle définit ; les autres (densité linéaire, lumière vive/linéaire/ponctuelle, mélange maître,
//! soustraction, division, couleur plus sombre/claire) suivent les définitions usuelles des éditeurs
//! raster. Les shaders GPU devront reproduire ces fonctions à 1/255 près (voir les tests de
//! comparaison de la phase 2).

use serde::{Deserialize, Serialize};

pub type Rgb = [f32; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    #[default]
    Normal,
    Dissolve,
    Darken,
    Multiply,
    ColorBurn,
    LinearBurn,
    DarkerColor,
    Lighten,
    Screen,
    ColorDodge,
    LinearDodge,
    LighterColor,
    Overlay,
    SoftLight,
    HardLight,
    VividLight,
    LinearLight,
    PinLight,
    HardMix,
    Difference,
    Exclusion,
    Subtract,
    Divide,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    pub const ALL: [BlendMode; 27] = [
        BlendMode::Normal,
        BlendMode::Dissolve,
        BlendMode::Darken,
        BlendMode::Multiply,
        BlendMode::ColorBurn,
        BlendMode::LinearBurn,
        BlendMode::DarkerColor,
        BlendMode::Lighten,
        BlendMode::Screen,
        BlendMode::ColorDodge,
        BlendMode::LinearDodge,
        BlendMode::LighterColor,
        BlendMode::Overlay,
        BlendMode::SoftLight,
        BlendMode::HardLight,
        BlendMode::VividLight,
        BlendMode::LinearLight,
        BlendMode::PinLight,
        BlendMode::HardMix,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Subtract,
        BlendMode::Divide,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Color,
        BlendMode::Luminosity,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Dissolve => "Dissolution",
            BlendMode::Darken => "Obscurcir",
            BlendMode::Multiply => "Produit",
            BlendMode::ColorBurn => "Densité couleur +",
            BlendMode::LinearBurn => "Densité linéaire +",
            BlendMode::DarkerColor => "Couleur plus sombre",
            BlendMode::Lighten => "Éclaircir",
            BlendMode::Screen => "Écran",
            BlendMode::ColorDodge => "Densité couleur −",
            BlendMode::LinearDodge => "Densité linéaire − (ajout)",
            BlendMode::LighterColor => "Couleur plus claire",
            BlendMode::Overlay => "Superposition",
            BlendMode::SoftLight => "Lumière tamisée",
            BlendMode::HardLight => "Lumière crue",
            BlendMode::VividLight => "Lumière vive",
            BlendMode::LinearLight => "Lumière linéaire",
            BlendMode::PinLight => "Lumière ponctuelle",
            BlendMode::HardMix => "Mélange maître",
            BlendMode::Difference => "Différence",
            BlendMode::Exclusion => "Exclusion",
            BlendMode::Subtract => "Soustraction",
            BlendMode::Divide => "Division",
            BlendMode::Hue => "Teinte",
            BlendMode::Saturation => "Saturation",
            BlendMode::Color => "Couleur",
            BlendMode::Luminosity => "Luminosité",
        }
    }
}

fn lum(c: Rgb) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn clip_color(c: Rgb) -> Rgb {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut out = c;
    if n < 0.0 {
        for v in &mut out {
            *v = l + (*v - l) * l / (l - n);
        }
    }
    if x > 1.0 {
        for v in &mut out {
            *v = l + (*v - l) * (1.0 - l) / (x - l);
        }
    }
    out
}

fn set_lum(c: Rgb, l: f32) -> Rgb {
    let d = l - lum(c);
    clip_color([c[0] + d, c[1] + d, c[2] + d])
}

fn sat(c: Rgb) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn set_sat(c: Rgb, s: f32) -> Rgb {
    let mut idx = [0usize, 1, 2];
    idx.sort_by(|&a, &b| c[a].total_cmp(&c[b]));
    let (min, mid, max) = (idx[0], idx[1], idx[2]);
    let mut out = [0.0; 3];
    if c[max] > c[min] {
        out[mid] = (c[mid] - c[min]) * s / (c[max] - c[min]);
        out[max] = s;
    }
    out
}

fn per_channel(cb: Rgb, cs: Rgb, f: impl Fn(f32, f32) -> f32) -> Rgb {
    [f(cb[0], cs[0]), f(cb[1], cs[1]), f(cb[2], cs[2])]
}

fn multiply(b: f32, s: f32) -> f32 {
    b * s
}

fn screen(b: f32, s: f32) -> f32 {
    b + s - b * s
}

fn hard_light(b: f32, s: f32) -> f32 {
    if s <= 0.5 { multiply(b, 2.0 * s) } else { screen(b, 2.0 * s - 1.0) }
}

fn color_burn(b: f32, s: f32) -> f32 {
    if b >= 1.0 {
        1.0
    } else if s <= 0.0 {
        0.0
    } else {
        1.0 - ((1.0 - b) / s).min(1.0)
    }
}

fn color_dodge(b: f32, s: f32) -> f32 {
    if b <= 0.0 {
        0.0
    } else if s >= 1.0 {
        1.0
    } else {
        (b / (1.0 - s)).min(1.0)
    }
}

fn soft_light(b: f32, s: f32) -> f32 {
    if s <= 0.5 {
        b - (1.0 - 2.0 * s) * b * (1.0 - b)
    } else {
        let d = if b <= 0.25 { ((16.0 * b - 12.0) * b + 4.0) * b } else { b.sqrt() };
        b + (2.0 * s - 1.0) * (d - b)
    }
}

/// Résultat du mode pour des couleurs (0..1) opaques ; `cb` = fond, `cs` = source.
/// La dissolution se comporte comme « normal » ici : son aléa porte sur l'alpha (voir `composite`).
pub fn blend(mode: BlendMode, cb: Rgb, cs: Rgb) -> Rgb {
    let out = match mode {
        BlendMode::Normal | BlendMode::Dissolve => cs,
        BlendMode::Darken => per_channel(cb, cs, f32::min),
        BlendMode::Multiply => per_channel(cb, cs, multiply),
        BlendMode::ColorBurn => per_channel(cb, cs, color_burn),
        BlendMode::LinearBurn => per_channel(cb, cs, |b, s| b + s - 1.0),
        BlendMode::DarkerColor => {
            if lum(cs) < lum(cb) {
                cs
            } else {
                cb
            }
        }
        BlendMode::Lighten => per_channel(cb, cs, f32::max),
        BlendMode::Screen => per_channel(cb, cs, screen),
        BlendMode::ColorDodge => per_channel(cb, cs, color_dodge),
        BlendMode::LinearDodge => per_channel(cb, cs, |b, s| b + s),
        BlendMode::LighterColor => {
            if lum(cs) > lum(cb) {
                cs
            } else {
                cb
            }
        }
        BlendMode::Overlay => per_channel(cb, cs, |b, s| hard_light(s, b)),
        BlendMode::SoftLight => per_channel(cb, cs, soft_light),
        BlendMode::HardLight => per_channel(cb, cs, hard_light),
        BlendMode::VividLight => {
            per_channel(cb, cs, |b, s| if s <= 0.5 { color_burn(b, 2.0 * s) } else { color_dodge(b, 2.0 * (s - 0.5)) })
        }
        BlendMode::LinearLight => per_channel(cb, cs, |b, s| b + 2.0 * s - 1.0),
        BlendMode::PinLight => per_channel(cb, cs, |b, s| if s < 0.5 { b.min(2.0 * s) } else { b.max(2.0 * s - 1.0) }),
        BlendMode::HardMix => per_channel(cb, cs, |b, s| if b + s >= 1.0 { 1.0 } else { 0.0 }),
        BlendMode::Difference => per_channel(cb, cs, |b, s| (b - s).abs()),
        BlendMode::Exclusion => per_channel(cb, cs, |b, s| b + s - 2.0 * b * s),
        BlendMode::Subtract => per_channel(cb, cs, |b, s| b - s),
        BlendMode::Divide => per_channel(cb, cs, |b, s| if s <= 0.0 { 1.0 } else { b / s }),
        BlendMode::Hue => set_lum(set_sat(cs, sat(cb)), lum(cb)),
        BlendMode::Saturation => set_lum(set_sat(cb, sat(cs)), lum(cb)),
        BlendMode::Color => set_lum(cs, lum(cb)),
        BlendMode::Luminosity => set_lum(cb, lum(cs)),
    };
    [out[0].clamp(0.0, 1.0), out[1].clamp(0.0, 1.0), out[2].clamp(0.0, 1.0)]
}

/// Aléa déterministe par pixel pour la dissolution : le même document donne toujours la même
/// image, ce qui permet les tests de référence et évite un scintillement entre deux rendus.
pub fn dissolve_threshold(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Fusionne une source (alpha droit, `alpha` déjà multiplié par opacité/masque) sur un fond (alpha
/// droit) : formule de composition W3C. `(x, y)` ne sert qu'à la dissolution.
pub fn composite(mode: BlendMode, backdrop: [f32; 4], src_rgb: Rgb, src_alpha: f32, x: u32, y: u32) -> [f32; 4] {
    let mut sa = src_alpha.clamp(0.0, 1.0);
    if mode == BlendMode::Dissolve {
        sa = if sa > dissolve_threshold(x, y) { 1.0 } else { 0.0 };
    }
    if sa <= 0.0 {
        return backdrop;
    }
    let ba = backdrop[3];
    let cb = [backdrop[0], backdrop[1], backdrop[2]];
    let blended = blend(mode, cb, src_rgb);
    let ra = sa + ba * (1.0 - sa);
    let mut out = [0.0, 0.0, 0.0, ra];
    for c in 0..3 {
        // Où le fond est transparent, la couleur source passe telle quelle (pas de fusion avec « rien »).
        let cs = (1.0 - ba) * src_rgb[c] + ba * blended[c];
        out[c] = (sa * cs + (1.0 - sa) * ba * cb[c]) / ra;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: Rgb = [0.2, 0.5, 0.8];
    const S: Rgb = [0.6, 0.3, 0.9];

    fn close(a: Rgb, b: Rgb) {
        for i in 0..3 {
            assert!((a[i] - b[i]).abs() < 1e-5, "{a:?} != {b:?}");
        }
    }

    #[test]
    fn there_are_27_distinct_modes() {
        let set: std::collections::HashSet<_> = BlendMode::ALL.iter().collect();
        assert_eq!(set.len(), 27);
        assert!(BlendMode::ALL.iter().all(|m| !m.label().is_empty()));
    }

    #[test]
    fn known_values_for_separable_modes() {
        close(blend(BlendMode::Multiply, B, S), [0.12, 0.15, 0.72]);
        close(blend(BlendMode::Screen, B, S), [0.68, 0.65, 0.98]);
        close(blend(BlendMode::Darken, B, S), [0.2, 0.3, 0.8]);
        close(blend(BlendMode::Lighten, B, S), [0.6, 0.5, 0.9]);
        close(blend(BlendMode::Difference, B, S), [0.4, 0.2, 0.1]);
        close(blend(BlendMode::Exclusion, B, S), [0.56, 0.5, 0.26]);
        close(blend(BlendMode::LinearBurn, B, S), [0.0, 0.0, 0.7]);
        close(blend(BlendMode::LinearDodge, B, S), [0.8, 0.8, 1.0]);
        close(blend(BlendMode::Subtract, B, S), [0.0, 0.2, 0.0]);
        close(blend(BlendMode::Divide, B, S), [0.2 / 0.6, 1.0, 0.8 / 0.9]);
        close(blend(BlendMode::LinearLight, B, S), [0.4, 0.1, 1.0]);
        close(blend(BlendMode::HardMix, B, S), [0.0, 0.0, 1.0]);
        // Surimpression = lumière crue avec les rôles inversés.
        close(blend(BlendMode::Overlay, B, S), blend(BlendMode::HardLight, S, B));
    }

    #[test]
    fn identity_and_absorbing_elements() {
        let white = [1.0; 3];
        let black = [0.0; 3];
        close(blend(BlendMode::Multiply, B, white), B);
        close(blend(BlendMode::Multiply, B, black), black);
        close(blend(BlendMode::Screen, B, black), B);
        close(blend(BlendMode::Screen, B, white), white);
        close(blend(BlendMode::Difference, B, B), black);
        close(blend(BlendMode::Divide, B, white), B);
        close(blend(BlendMode::ColorDodge, black, S), black);
        close(blend(BlendMode::ColorBurn, white, S), white);
        close(blend(BlendMode::Normal, B, S), S);
    }

    #[test]
    fn non_separable_modes_keep_the_right_component() {
        // Luminosité : la teinte/saturation du fond, la luminance de la source.
        let r = blend(BlendMode::Luminosity, B, S);
        assert!((lum(r) - lum(S)).abs() < 1e-4);
        // Couleur : la luminance du fond est conservée.
        let r = blend(BlendMode::Color, B, S);
        assert!((lum(r) - lum(B)).abs() < 1e-4);
        // Saturation et teinte conservent la luminance du fond.
        assert!((lum(blend(BlendMode::Saturation, B, S)) - lum(B)).abs() < 1e-4);
        assert!((lum(blend(BlendMode::Hue, B, S)) - lum(B)).abs() < 1e-4);
        // Un gris n'a pas de teinte : « teinte » d'une source grise donne un gris.
        let grey = blend(BlendMode::Hue, B, [0.5; 3]);
        assert!((grey[0] - grey[1]).abs() < 1e-5 && (grey[1] - grey[2]).abs() < 1e-5);
    }

    #[test]
    fn darker_and_lighter_color_pick_a_whole_pixel() {
        assert_eq!(blend(BlendMode::DarkerColor, B, [0.1, 0.1, 0.1]), [0.1, 0.1, 0.1]);
        assert_eq!(blend(BlendMode::DarkerColor, B, [0.9, 0.9, 0.9]), B);
        assert_eq!(blend(BlendMode::LighterColor, B, [0.9, 0.9, 0.9]), [0.9, 0.9, 0.9]);
    }

    #[test]
    fn every_mode_stays_in_range_and_finite() {
        let samples = [0.0, 0.001, 0.25, 0.5, 0.75, 0.999, 1.0];
        for mode in BlendMode::ALL {
            for &r in &samples {
                for &g in &samples {
                    for &s in &samples {
                        let out = blend(mode, [r, g, 0.4], [s, 1.0 - s, g]);
                        assert!(out.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)), "{mode:?} -> {out:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn composite_over_transparent_backdrop_keeps_source_colour() {
        for mode in BlendMode::ALL {
            if mode == BlendMode::Dissolve {
                continue;
            }
            let out = composite(mode, [0.0; 4], S, 0.5, 0, 0);
            close([out[0], out[1], out[2]], S);
            assert!((out[3] - 0.5).abs() < 1e-6, "{mode:?}");
        }
    }

    #[test]
    fn composite_normal_matches_porter_duff_over() {
        let out = composite(BlendMode::Normal, [1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0], 0.5, 0, 0);
        close([out[0], out[1], out[2]], [0.5, 0.0, 0.5]);
        assert_eq!(out[3], 1.0);
        // Source transparente : le fond est inchangé, à l'identique.
        assert_eq!(composite(BlendMode::Multiply, [0.3, 0.4, 0.5, 0.7], S, 0.0, 0, 0), [0.3, 0.4, 0.5, 0.7]);
    }

    #[test]
    fn dissolve_is_deterministic_and_proportional_to_alpha() {
        let covered = (0..64u32)
            .flat_map(|y| (0..64u32).map(move |x| (x, y)))
            .filter(|&(x, y)| composite(BlendMode::Dissolve, [0.0; 4], S, 0.25, x, y)[3] > 0.0)
            .count();
        let ratio = covered as f32 / 4096.0;
        assert!((ratio - 0.25).abs() < 0.05, "ratio {ratio}");
        assert_eq!(dissolve_threshold(7, 9), dissolve_threshold(7, 9));
    }
}
