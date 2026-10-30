//! Types échangés avec le frontend. Les définitions TypeScript sont générées (ts-rs) dans
//! `ui/src/gen/` : on ne les recopie jamais à la main.

use std::collections::BTreeMap;

use openpics_core::adjust::{AdjKind, Adjustment, Extra};
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
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Tool {
    Hand,
    Zoom,
    RectSelect,
    EllipseSelect,
    Lasso,
    PolygonLasso,
    MagicWand,
    Brush,
    Pencil,
    Eraser,
    Bucket,
    Gradient,
    CloneStamp,
    Heal,
    SpotHeal,
    Patch,
    Blur,
    Sharpen,
    Smudge,
    Dodge,
    Burn,
    Sponge,
    Liquify,
    Text,
    ShapeRect,
    ShapeRoundedRect,
    ShapeEllipse,
    ShapeLine,
    Pen,
}

/// Réglages des outils de sélection.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolOptionsDto {
    /// 0..=255 : écart de couleur accepté par la baguette magique.
    pub tolerance: u8,
    pub contiguous: bool,
    /// La baguette lit l'image aplatie plutôt que le calque actif.
    pub sample_all_layers: bool,
}

impl Default for ToolOptionsDto {
    fn default() -> Self {
        Self { tolerance: 32, contiguous: true, sample_all_layers: false }
    }
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
    pub has_selection: bool,
    pub filters: Vec<FilterInfoDto>,
    pub adjustment_kinds: Vec<BlendInfo>,
    pub tool_options: ToolOptionsDto,
    pub paint_options: PaintOptionsDto,
    pub history: HistoryDto,
    pub can_undo: bool,
    pub can_redo: bool,
    pub settings: SettingsDto,
    pub tool: Tool,
    /// Le document affiché est son original (comparaison avant/après) : il est gelé.
    pub comparing: bool,
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
    pub is_group: bool,
    /// Groupes : isolé (`true`) ou « traverser » (`false`).
    pub isolated: bool,
    #[ts(type = "number | null")]
    pub group: Option<u64>,
    /// Imbrication (0 = niveau supérieur), pour l'indentation du panneau.
    pub depth: usize,
    pub adjustment: Option<AdjustmentDto>,
    pub text: Option<TextDto>,
    pub shape: Option<ShapeDto>,
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
            is_group: l.is_group(),
            isolated: l.props().isolated,
            group: l.group.map(|g| g.0),
            depth: 0,
            adjustment: l.adjustment().map(AdjustmentDto::from),
            text: l.text().map(TextDto::from),
            shape: l.shape().map(ShapeDto::from),
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
    pub isolated: bool,
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
            isolated: self.isolated,
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
    fn every_registered_filter_is_exposed_with_defaults_inside_their_range() {
        let infos = filter_infos();
        assert_eq!(infos.len(), openpics_ops::filters::REGISTRY.len());
        assert!(
            infos
                .iter()
                .all(|f| f.params.iter().all(|p| p.value >= p.min && p.value <= p.max) && !f.category.is_empty())
        );
    }

    #[test]
    fn text_and_shape_dtos_mirror_their_models() {
        let t = openpics_core::text::TextModel { color: [255, 0, 128], italic: true, ..Default::default() };
        let d = TextDto::from(&t);
        assert_eq!((d.color.as_str(), d.italic, d.has_path, d.align), ("#ff0080", true, false, TextAlignDto::Left));
        let v = openpics_core::vector::VectorModel { fill: Some([1, 2, 3, 128]), ..Default::default() };
        let s = ShapeDto::from(&v);
        assert_eq!(
            s.fill.as_ref().map(|f| (f.color.as_str(), (f.opacity * 255.0).round() as u8)),
            Some(("#010203", 128))
        );
        assert_eq!(parse_boolean_op("xor"), Some(openpics_ops::vector::BooleanOp::Xor));
        assert_eq!(parse_boolean_op("n'importe quoi"), None);
    }

    #[test]
    fn adjustment_kind_ids_roundtrip_and_dtos_expose_every_parameter() {
        for k in AdjKind::ALL {
            assert_eq!(parse_adj_kind(&adj_kind_id(k)), Some(k));
            let dto = AdjustmentDto::from(&Adjustment::new(k));
            assert_eq!(dto.params.len(), k.specs().len());
            assert!(dto.params.iter().all(|p| p.value >= p.min && p.value <= p.max));
        }
        assert_eq!(parse_adj_kind("n'importe quoi"), None);
        assert_eq!(adjustment_kinds().len(), 15);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum GradientKindDto {
    Linear,
    Radial,
    Angular,
    Reflected,
    Diamond,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TonalRangeDto {
    Shadows,
    Midtones,
    Highlights,
}

/// Couleurs et réglages communs aux outils de peinture et de retouche.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PaintOptionsDto {
    pub foreground: String,
    pub background: String,
    pub size: f32,
    pub hardness: f32,
    pub spacing: f32,
    pub opacity: f32,
    pub flow: f32,
    pub smoothing: f32,
    pub size_from_pressure: bool,
    pub opacity_from_pressure: bool,
    pub gradient: GradientKindDto,
    /// Dégradé du premier plan vers la transparence plutôt que vers l'arrière-plan.
    pub gradient_to_transparent: bool,
    pub tonal_range: TonalRangeDto,
    /// Éponge : `true` = saturer, `false` = désaturer.
    pub saturate: bool,
    pub liquify_mode: LiquifyModeDto,
}

impl Default for PaintOptionsDto {
    fn default() -> Self {
        Self {
            foreground: "#000000".into(),
            background: "#ffffff".into(),
            size: 20.0,
            hardness: 0.8,
            spacing: 0.15,
            opacity: 1.0,
            flow: 1.0,
            smoothing: 0.0,
            size_from_pressure: true,
            opacity_from_pressure: false,
            gradient: GradientKindDto::Linear,
            gradient_to_transparent: false,
            tonal_range: TonalRangeDto::Midtones,
            saturate: true,
            liquify_mode: LiquifyModeDto::Push,
        }
    }
}

impl PaintOptionsDto {
    /// Remplace les couleurs invalides par leur valeur actuelle et borne les réglages numériques.
    pub fn sanitized(&self, current: &PaintOptionsDto) -> PaintOptionsDto {
        let keep = |new: &str, old: &str| if parse_hex(new).is_some() { new.to_owned() } else { old.to_owned() };
        let brush = self.brush([0; 3]).sanitized();
        PaintOptionsDto {
            foreground: keep(&self.foreground, &current.foreground),
            background: keep(&self.background, &current.background),
            size: brush.size,
            hardness: brush.hardness,
            spacing: brush.spacing,
            opacity: brush.opacity,
            flow: brush.flow,
            smoothing: brush.smoothing,
            ..self.clone()
        }
    }

    pub fn brush(&self, color: [u8; 3]) -> openpics_ops::brush::BrushParams {
        openpics_ops::brush::BrushParams {
            size: self.size,
            hardness: self.hardness,
            spacing: self.spacing,
            opacity: self.opacity,
            flow: self.flow,
            color,
            smoothing: self.smoothing,
            size_from_pressure: self.size_from_pressure,
            opacity_from_pressure: self.opacity_from_pressure,
        }
    }

    pub fn fg(&self) -> [u8; 3] {
        parse_hex(&self.foreground).unwrap_or([0; 3])
    }

    pub fn bg(&self) -> [u8; 3] {
        parse_hex(&self.background).unwrap_or([255; 3])
    }
}

/// Paramètre numérique d'un réglage, avec sa plage : l'interface génère un curseur par paramètre.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ParamDto {
    pub id: String,
    pub label: String,
    pub min: f32,
    pub max: f32,
    pub value: f32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StopDto {
    pub position: f32,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdjustmentDto {
    pub kind: String,
    pub label: String,
    pub params: Vec<ParamDto>,
    /// Courbes : points (x, y) de la courbe commune.
    pub curve: Option<Vec<(f32, f32)>>,
    /// Courbes de dégradé : arrêts triés.
    pub stops: Option<Vec<StopDto>>,
    /// LUT 3D : taille de la table chargée.
    pub lut_size: Option<usize>,
    pub is_color_match: bool,
}

pub fn adj_kind_id(kind: AdjKind) -> String {
    serde_json::to_value(kind).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()
}

pub fn parse_adj_kind(id: &str) -> Option<AdjKind> {
    serde_json::from_value(serde_json::Value::String(id.to_owned())).ok()
}

pub fn adjustment_kinds() -> Vec<BlendInfo> {
    AdjKind::ALL.iter().map(|k| BlendInfo { id: adj_kind_id(*k), label: k.label().to_owned() }).collect()
}

impl From<&Adjustment> for AdjustmentDto {
    fn from(a: &Adjustment) -> Self {
        AdjustmentDto {
            kind: adj_kind_id(a.kind),
            label: a.kind.label().to_owned(),
            params: a
                .kind
                .specs()
                .iter()
                .map(|s| ParamDto {
                    id: s.id.to_owned(),
                    label: s.label.to_owned(),
                    min: s.min,
                    max: s.max,
                    value: a.value(s.id),
                })
                .collect(),
            curve: match &a.extra {
                Extra::Curves { master, .. } => Some(master.clone()),
                _ => None,
            },
            stops: match &a.extra {
                Extra::Stops(s) => Some(s.iter().map(|(p, c)| StopDto { position: *p, color: hex(*c) }).collect()),
                _ => None,
            },
            lut_size: match &a.extra {
                Extra::Lut(l) => Some(l.size),
                _ => None,
            },
            is_color_match: a.kind == AdjKind::ColorMatch,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LiquifyModeDto {
    Push,
    Bloat,
    Pucker,
    Twirl,
    Reconstruct,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FilterInfoDto {
    pub id: String,
    pub label: String,
    pub category: String,
    /// Paramètres avec leur plage ; `value` est la valeur par défaut.
    pub params: Vec<ParamDto>,
}

pub fn filter_infos() -> Vec<FilterInfoDto> {
    openpics_ops::filters::REGISTRY
        .iter()
        .map(|f| FilterInfoDto {
            id: f.id.to_owned(),
            label: f.label.to_owned(),
            category: f.category.label().to_owned(),
            params: f
                .params
                .iter()
                .map(|p| ParamDto {
                    id: p.id.to_owned(),
                    label: p.label.to_owned(),
                    min: p.min,
                    max: p.max,
                    value: p.default,
                })
                .collect(),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TextAlignDto {
    Left,
    Center,
    Right,
    Justify,
}

/// Calque de texte tel que l'interface l'édite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TextDto {
    pub text: String,
    pub font: String,
    pub size: f32,
    pub color: String,
    pub bold: bool,
    pub italic: bool,
    pub align: TextAlignDto,
    pub line_height: f32,
    pub box_width: Option<f32>,
    pub origin_x: f32,
    pub origin_y: f32,
    /// En sortie seulement : le texte suit un tracé.
    #[serde(default)]
    pub has_path: bool,
}

impl From<&openpics_core::text::TextModel> for TextDto {
    fn from(t: &openpics_core::text::TextModel) -> Self {
        use openpics_core::text::TextAlign;
        TextDto {
            text: t.text.clone(),
            font: t.font.clone(),
            size: t.size,
            color: hex(t.color),
            bold: t.bold,
            italic: t.italic,
            align: match t.align {
                TextAlign::Left => TextAlignDto::Left,
                TextAlign::Center => TextAlignDto::Center,
                TextAlign::Right => TextAlignDto::Right,
                TextAlign::Justify => TextAlignDto::Justify,
            },
            line_height: t.line_height,
            box_width: t.box_width,
            origin_x: t.origin.0,
            origin_y: t.origin.1,
            has_path: t.path.is_some(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ColorOpacityDto {
    pub color: String,
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StrokeDto {
    pub color: String,
    pub opacity: f32,
    pub width: f32,
    pub round_caps: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShapeDto {
    pub fill: Option<ColorOpacityDto>,
    pub stroke: Option<StrokeDto>,
    pub even_odd: bool,
    /// En sortie seulement : nombre de chemins de la forme.
    #[serde(default)]
    pub path_count: usize,
}

impl From<&openpics_core::vector::VectorModel> for ShapeDto {
    fn from(v: &openpics_core::vector::VectorModel) -> Self {
        let co = |c: [u8; 4]| ColorOpacityDto { color: hex([c[0], c[1], c[2]]), opacity: f32::from(c[3]) / 255.0 };
        ShapeDto {
            fill: v.fill.map(co),
            stroke: v.stroke.map(|s| StrokeDto {
                color: hex([s.color[0], s.color[1], s.color[2]]),
                opacity: f32::from(s.color[3]) / 255.0,
                width: s.width,
                round_caps: s.round_caps,
            }),
            even_odd: v.fill_rule == openpics_core::vector::FillRule::EvenOdd,
            path_count: v.paths.len(),
        }
    }
}

pub fn parse_boolean_op(id: &str) -> Option<openpics_ops::vector::BooleanOp> {
    use openpics_ops::vector::BooleanOp;
    Some(match id {
        "union" => BooleanOp::Union,
        "intersect" => BooleanOp::Intersect,
        "subtract" => BooleanOp::Subtract,
        "xor" => BooleanOp::Xor,
        _ => return None,
    })
}

/// Un modèle du catalogue local (voir `openpics-ai`).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelDto {
    pub id: String,
    pub task: String,
    pub name: String,
    pub licence: String,
    pub source: String,
    #[ts(type = "number")]
    pub size_bytes: u64,
    pub installed: bool,
    /// Sait isoler un sujet (suppression d'arrière-plan ou sélection du sujet).
    pub isolates_subject: bool,
}
