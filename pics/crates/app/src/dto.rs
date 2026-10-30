//! Types échangés avec le frontend. Les définitions TypeScript sont générées (ts-rs) dans
//! `ui/src/gen/` : on ne les recopie jamais à la main.

use std::collections::BTreeMap;

use openpics_core::styles::StrokePosition;
use openpics_core::{Axis, BlendMode, Guide, Layer, LayerProps, LayerStyle, Locks, ViewSettings};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum AxisDto {
    Horizontal,
    Vertical,
}

impl From<Axis> for AxisDto {
    fn from(a: Axis) -> Self {
        match a {
            Axis::Horizontal => AxisDto::Horizontal,
            Axis::Vertical => AxisDto::Vertical,
        }
    }
}

impl From<AxisDto> for Axis {
    fn from(a: AxisDto) -> Self {
        match a {
            AxisDto::Horizontal => Axis::Horizontal,
            AxisDto::Vertical => Axis::Vertical,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Tool {
    Hand,
    Zoom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ThemeChoice {
    System,
    Light,
    Dark,
}

/// Thème effectivement affiché (« système » est résolu par le frontend).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ResolvedTheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ImageOp {
    FlipHorizontal,
    FlipVertical,
    Invert,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SettingsDto {
    pub show_rulers: bool,
    pub show_guides: bool,
    pub show_grid: bool,
    pub grid_spacing: u32,
    pub snap_enabled: bool,
    pub snap_to_guides: bool,
    pub snap_to_grid: bool,
    pub snap_to_bounds: bool,
}

impl From<ViewSettings> for SettingsDto {
    fn from(s: ViewSettings) -> Self {
        Self {
            show_rulers: s.show_rulers,
            show_guides: s.show_guides,
            show_grid: s.show_grid,
            grid_spacing: s.grid_spacing,
            snap_enabled: s.snap_enabled,
            snap_to_guides: s.snap_to_guides,
            snap_to_grid: s.snap_to_grid,
            snap_to_bounds: s.snap_to_bounds,
        }
    }
}

impl From<SettingsDto> for ViewSettings {
    fn from(s: SettingsDto) -> Self {
        Self {
            show_rulers: s.show_rulers,
            show_guides: s.show_guides,
            show_grid: s.show_grid,
            // Un pas nul ou énorme n'a aucun sens : on borne plutôt que de refuser.
            grid_spacing: s.grid_spacing.clamp(2, 4096),
            snap_enabled: s.snap_enabled,
            snap_to_guides: s.snap_to_guides,
            snap_to_grid: s.snap_to_grid,
            snap_to_bounds: s.snap_to_bounds,
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DocSummary {
    #[ts(type = "number")]
    pub id: u64,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub dirty: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GuideDto {
    #[ts(type = "number")]
    pub id: u64,
    pub axis: AxisDto,
    pub position: f64,
}

impl From<&Guide> for GuideDto {
    fn from(g: &Guide) -> Self {
        Self { id: g.id, axis: g.axis.into(), position: g.position }
    }
}

/// Position du document dans la zone de canvas, en pixels physiques.
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ViewState {
    pub zoom: f64,
    pub origin_x: f64,
    pub origin_y: f64,
    pub canvas_width: u32,
    pub canvas_height: u32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryDto {
    pub names: Vec<String>,
    /// Nombre d'états appliqués : les `names[position..]` sont rétablissables.
    pub position: usize,
}

/// État complet renvoyé après chaque commande : le frontend remplace son état d'un bloc, ce qui
/// évite toute dérive entre les deux côtés.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Snapshot {
    pub docs: Vec<DocSummary>,
    #[ts(type = "number | null")]
    pub active_id: Option<u64>,
    pub view: ViewState,
    pub guides: Vec<GuideDto>,
    /// Calques du document actif, du plus haut au plus bas.
    pub layers: Vec<LayerDto>,
    #[ts(type = "number | null")]
    pub active_layer_id: Option<u64>,
    pub blend_modes: Vec<BlendInfo>,
    pub history: HistoryDto,
    pub can_undo: bool,
    pub can_redo: bool,
    pub settings: SettingsDto,
    pub tool: Tool,
}

#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GuideDrag {
    pub position: f64,
    pub snapped: bool,
}

#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CursorEvent {
    pub x: f64,
    pub y: f64,
    pub inside: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KeymapDto {
    pub bindings: BTreeMap<String, Vec<String>>,
}

/// Rectangle du canvas dans la fenêtre, en pixels physiques.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CanvasRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Informations de démarrage que le frontend demande une fois.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppStatus {
    pub version: String,
    pub portable: bool,
    pub gpu_error: Option<String>,
    pub theme: ThemeChoice,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpenResult {
    pub snapshot: Snapshot,
    /// Un message par fichier qui n'a pas pu être ouvert ; les autres le sont quand même.
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LocksDto {
    pub pixels: bool,
    pub position: bool,
    pub all: bool,
}

impl From<Locks> for LocksDto {
    fn from(l: Locks) -> Self {
        Self { pixels: l.pixels, position: l.position, all: l.all }
    }
}

impl From<LocksDto> for Locks {
    fn from(l: LocksDto) -> Self {
        Self { pixels: l.pixels, position: l.position, all: l.all }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LayerDto {
    #[ts(type = "number")]
    pub id: u64,
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub fill: f32,
    /// Identifiant stable du mode (voir `Snapshot::blend_modes`).
    pub blend: String,
    pub locks: LocksDto,
    pub clip_to_below: bool,
    pub has_mask: bool,
    pub mask_enabled: bool,
    pub styles: Vec<StyleDto>,
}

/// Identifiant JSON d'un mode (`multiply`, `soft_light`…), identique à celui du format `.ocv`.
pub fn blend_id(mode: BlendMode) -> String {
    serde_json::to_value(mode).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_else(|| "normal".to_owned())
}

pub fn parse_blend(id: &str) -> Option<BlendMode> {
    serde_json::from_value(serde_json::Value::String(id.to_owned())).ok()
}

impl From<&Layer> for LayerDto {
    fn from(l: &Layer) -> Self {
        Self {
            id: l.id.0,
            name: l.name.clone(),
            visible: l.visible,
            opacity: l.opacity,
            fill: l.fill,
            blend: blend_id(l.blend),
            locks: l.locks.into(),
            clip_to_below: l.clip_to_below,
            has_mask: l.mask.is_some(),
            mask_enabled: l.mask.as_ref().is_some_and(|m| m.enabled),
            styles: l.styles.iter().map(StyleDto::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct BlendInfo {
    pub id: String,
    pub label: String,
}

pub fn blend_infos() -> Vec<BlendInfo> {
    BlendMode::ALL.iter().map(|m| BlendInfo { id: blend_id(*m), label: m.label().to_owned() }).collect()
}

/// Propriétés modifiables d'un calque (la liaison n'a pas encore d'interface).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LayerPropsDto {
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub fill: f32,
    pub blend: String,
    pub locks: LocksDto,
    pub clip_to_below: bool,
}

impl LayerPropsDto {
    /// Fusionne dans les propriétés existantes ; un mode inconnu est refusé plutôt qu'ignoré.
    pub fn apply_to(&self, base: &LayerProps) -> Option<LayerProps> {
        Some(LayerProps {
            name: self.name.trim().to_owned(),
            visible: self.visible,
            opacity: self.opacity,
            fill: self.fill,
            blend: parse_blend(&self.blend)?,
            locks: self.locks.into(),
            link: base.link,
            clip_to_below: self.clip_to_below,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_dtos_roundtrip_and_reject_invalid_colours() {
        let styles = vec![
            LayerStyle::DropShadow { color: [1, 2, 3], opacity: 0.5, angle_deg: 30.0, distance: 4.0, size: 2.0 },
            LayerStyle::Stroke { color: [255, 0, 128], opacity: 1.0, size: 3.0, position: StrokePosition::Inside },
            LayerStyle::ColorOverlay { color: [9, 9, 9], opacity: 1.0, blend: BlendMode::Multiply },
        ];
        for s in &styles {
            assert_eq!(StyleDto::from(s).to_style().as_ref(), Some(s));
        }
        assert_eq!(hex([255, 0, 128]), "#ff0080");
        for bad in ["ff0080", "#ff00", "#gg0000", "#ff00801", "", "#é0000"] {
            assert_eq!(parse_hex(bad), None, "{bad}");
        }
        let json = serde_json::to_string(&StyleDto::from(&styles[0])).unwrap();
        assert!(json.contains("\"angleDeg\"") && json.contains("\"kind\":\"drop_shadow\""), "{json}");
    }

    #[test]
    fn blend_ids_roundtrip_for_all_27_modes() {
        for mode in BlendMode::ALL {
            assert_eq!(parse_blend(&blend_id(mode)), Some(mode));
        }
        assert_eq!(parse_blend("n'importe quoi"), None);
        assert_eq!(blend_infos().len(), 27);
        assert_eq!(blend_id(BlendMode::SoftLight), "soft_light");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum StrokePositionDto {
    Outside,
    Inside,
    Center,
}

/// Style de calque tel que l'interface le manipule : couleurs en « #rrggbb », mode en identifiant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum StyleDto {
    #[serde(rename_all = "camelCase")]
    DropShadow { color: String, opacity: f32, angle_deg: f32, distance: f32, size: f32 },
    #[serde(rename_all = "camelCase")]
    OuterGlow { color: String, opacity: f32, size: f32 },
    #[serde(rename_all = "camelCase")]
    Stroke { color: String, opacity: f32, size: f32, position: StrokePositionDto },
    #[serde(rename_all = "camelCase")]
    ColorOverlay { color: String, opacity: f32, blend: String },
    #[serde(rename_all = "camelCase")]
    GradientOverlay { from: String, to: String, angle_deg: f32, opacity: f32, blend: String },
}

pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

impl From<&LayerStyle> for StyleDto {
    fn from(s: &LayerStyle) -> Self {
        let pos = |p: &StrokePosition| match p {
            StrokePosition::Outside => StrokePositionDto::Outside,
            StrokePosition::Inside => StrokePositionDto::Inside,
            StrokePosition::Center => StrokePositionDto::Center,
        };
        match s {
            LayerStyle::DropShadow { color, opacity, angle_deg, distance, size } => StyleDto::DropShadow {
                color: hex(*color),
                opacity: *opacity,
                angle_deg: *angle_deg,
                distance: *distance,
                size: *size,
            },
            LayerStyle::OuterGlow { color, opacity, size } => {
                StyleDto::OuterGlow { color: hex(*color), opacity: *opacity, size: *size }
            }
            LayerStyle::Stroke { color, opacity, size, position } => {
                StyleDto::Stroke { color: hex(*color), opacity: *opacity, size: *size, position: pos(position) }
            }
            LayerStyle::ColorOverlay { color, opacity, blend } => {
                StyleDto::ColorOverlay { color: hex(*color), opacity: *opacity, blend: blend_id(*blend) }
            }
            LayerStyle::GradientOverlay { from, to, angle_deg, opacity, blend } => StyleDto::GradientOverlay {
                from: hex(*from),
                to: hex(*to),
                angle_deg: *angle_deg,
                opacity: *opacity,
                blend: blend_id(*blend),
            },
        }
    }
}

impl StyleDto {
    /// `None` si une couleur ou un mode de fusion est invalide : on refuse plutôt que de deviner.
    pub fn to_style(&self) -> Option<LayerStyle> {
        let pos = |p: &StrokePositionDto| match p {
            StrokePositionDto::Outside => StrokePosition::Outside,
            StrokePositionDto::Inside => StrokePosition::Inside,
            StrokePositionDto::Center => StrokePosition::Center,
        };
        Some(match self {
            StyleDto::DropShadow { color, opacity, angle_deg, distance, size } => LayerStyle::DropShadow {
                color: parse_hex(color)?,
                opacity: *opacity,
                angle_deg: *angle_deg,
                distance: *distance,
                size: *size,
            },
            StyleDto::OuterGlow { color, opacity, size } => {
                LayerStyle::OuterGlow { color: parse_hex(color)?, opacity: *opacity, size: *size }
            }
            StyleDto::Stroke { color, opacity, size, position } => {
                LayerStyle::Stroke { color: parse_hex(color)?, opacity: *opacity, size: *size, position: pos(position) }
            }
            StyleDto::ColorOverlay { color, opacity, blend } => {
                LayerStyle::ColorOverlay { color: parse_hex(color)?, opacity: *opacity, blend: parse_blend(blend)? }
            }
            StyleDto::GradientOverlay { from, to, angle_deg, opacity, blend } => LayerStyle::GradientOverlay {
                from: parse_hex(from)?,
                to: parse_hex(to)?,
                angle_deg: *angle_deg,
                opacity: *opacity,
                blend: parse_blend(blend)?,
            },
        })
    }
}

/// Transformation libre saisie dans l'interface : échelle en pourcentage, rotation en degrés (sens horaire),
/// déplacement en pixels, le tout autour du centre du document.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TransformDto {
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation_deg: f64,
    pub translate_x: f64,
    pub translate_y: f64,
}
