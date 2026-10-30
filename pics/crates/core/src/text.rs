//! Modèle d'un calque de texte. La mise en forme (polices, crénage, ligatures) et le rendu sont dans `ops` ;
//! ici ne vivent que les données, bornées à l'application.

use serde::{Deserialize, Serialize};

use crate::vector::Path;

pub const MAX_TEXT_CHARS: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextModel {
    pub text: String,
    /// Famille de police ; vide ou introuvable = police sans empattement du système.
    pub font: String,
    /// Corps en pixels.
    pub size: f32,
    pub color: [u8; 3],
    pub bold: bool,
    pub italic: bool,
    pub align: TextAlign,
    /// Interligne, multiple du corps.
    pub line_height: f32,
    /// Largeur du bloc pour un texte de paragraphe (retour à la ligne automatique) ; `None` = texte sur une ligne.
    pub box_width: Option<f32>,
    /// Coin haut-gauche du bloc, en pixels de document.
    pub origin: (f32, f32),
    /// Texte sur tracé : le texte suit ce chemin (la ligne de base épouse la courbe).
    pub path: Option<Path>,
}

impl Default for TextModel {
    fn default() -> Self {
        Self {
            text: "Texte".to_owned(),
            font: String::new(),
            size: 32.0,
            color: [0, 0, 0],
            bold: false,
            italic: false,
            align: TextAlign::Left,
            line_height: 1.25,
            box_width: None,
            origin: (0.0, 0.0),
            path: None,
        }
    }
}

impl TextModel {
    pub fn sanitized(&self) -> TextModel {
        let f = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        TextModel {
            text: self.text.chars().take(MAX_TEXT_CHARS).collect(),
            font: self.font.chars().take(200).collect(),
            size: f(self.size, 1.0, 2000.0, 32.0),
            line_height: f(self.line_height, 0.5, 5.0, 1.25),
            box_width: self.box_width.map(|w| f(w, 1.0, 100_000.0, 400.0)),
            origin: (f(self.origin.0, -1e6, 1e6, 0.0), f(self.origin.1, -1e6, 1e6, 0.0)),
            path: self.path.as_ref().map(|p| {
                crate::vector::VectorModel { paths: vec![p.clone()], ..Default::default() }
                    .sanitized()
                    .paths
                    .into_iter()
                    .next()
                    .unwrap_or_default()
            }),
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane_and_sanitizing_bounds_everything() {
        let d = TextModel::default();
        assert_eq!(d.sanitized(), d);
        let wild = TextModel {
            text: "é".repeat(MAX_TEXT_CHARS + 5),
            size: f32::NAN,
            line_height: 99.0,
            box_width: Some(-4.0),
            origin: (f32::INFINITY, 1e9),
            font: "x".repeat(500),
            ..TextModel::default()
        };
        let s = wild.sanitized();
        assert_eq!(s.text.chars().count(), MAX_TEXT_CHARS);
        assert_eq!((s.size, s.line_height, s.box_width, s.font.len()), (32.0, 5.0, Some(1.0), 200));
        assert_eq!(s.origin, (0.0, 1e6));
        let json = serde_json::to_string(&TextModel { align: TextAlign::Justify, ..TextModel::default() }).unwrap();
        assert!(json.contains("\"justify\""));
    }
}
