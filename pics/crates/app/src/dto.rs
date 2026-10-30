//! Types échangés avec le frontend. Les définitions TypeScript sont générées (ts-rs) dans
//! `ui/src/gen/` : on ne les recopie jamais à la main.

use std::collections::BTreeMap;

use openpics_core::{Axis, Guide, ViewSettings};
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
