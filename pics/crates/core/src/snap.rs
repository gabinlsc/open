//! Aimantation sur repères, grille et bords du document.
//!
//! La tolérance est exprimée en pixels d'écran : à fort zoom elle couvre peu de pixels de
//! document, ce qui garde un placement précis ; à faible zoom elle reste confortable.

use serde::Serialize;

use crate::document::{Axis, Guide};
use crate::geom::Size;
use crate::view::ViewSettings;

pub const SNAP_TOLERANCE_PX: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapKind {
    Guide,
    Grid,
    Bounds,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapped {
    pub value: f64,
    pub hit: Option<SnapKind>,
}

/// Aimante une coordonnée (en pixels de document) sur l'axe donné. Priorité : repères, puis
/// bords, puis grille — le candidat le plus proche gagne, à égalité l'ordre ci-dessus.
pub fn snap_axis(value: f64, axis: Axis, guides: &[Guide], doc: Size, settings: &ViewSettings, zoom: f64) -> Snapped {
    let none = Snapped { value, hit: None };
    if !settings.snap_enabled || zoom <= 0.0 || !value.is_finite() {
        return none;
    }
    let tol = SNAP_TOLERANCE_PX / zoom;
    let mut best: Option<(f64, f64, SnapKind)> = None; // (distance, valeur, genre)
    let mut consider = |candidate: f64, kind: SnapKind| {
        let d = (candidate - value).abs();
        if d <= tol && best.is_none_or(|(bd, _, _)| d < bd) {
            best = Some((d, candidate, kind));
        }
    };
    if settings.snap_to_guides && settings.show_guides {
        for g in guides.iter().filter(|g| g.axis == axis) {
            consider(g.position, SnapKind::Guide);
        }
    }
    if settings.snap_to_bounds {
        let extent = match axis {
            Axis::Horizontal => f64::from(doc.height),
            Axis::Vertical => f64::from(doc.width),
        };
        consider(0.0, SnapKind::Bounds);
        consider(extent, SnapKind::Bounds);
    }
    if settings.snap_to_grid && settings.show_grid && settings.grid_spacing > 0 {
        let s = f64::from(settings.grid_spacing);
        consider((value / s).round() * s, SnapKind::Grid);
    }
    best.map_or(none, |(_, v, k)| Snapped { value: v, hit: Some(k) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> ViewSettings {
        ViewSettings { show_grid: true, grid_spacing: 10, ..ViewSettings::default() }
    }

    fn guide(axis: Axis, position: f64) -> Guide {
        Guide { id: 1, axis, position }
    }

    #[test]
    fn snaps_to_guide_within_tolerance_only() {
        let g = [guide(Axis::Vertical, 100.0)];
        let s = settings();
        let hit = snap_axis(103.0, Axis::Vertical, &g, Size::new(500, 500), &s, 1.0);
        assert_eq!(hit, Snapped { value: 100.0, hit: Some(SnapKind::Guide) });
        let miss = snap_axis(103.0, Axis::Vertical, &g, Size::new(500, 500), &s, 4.0);
        // À ×4, 8 px d'écran = 2 px de document : ni le repère ni la grille (à 3 px) ne sont à portée.
        assert_eq!(miss.hit, None);
    }

    #[test]
    fn ignores_guides_of_other_axis() {
        let g = [guide(Axis::Horizontal, 100.0)];
        let r = snap_axis(
            101.0,
            Axis::Vertical,
            &g,
            Size::new(500, 500),
            &ViewSettings { show_grid: false, ..ViewSettings::default() },
            1.0,
        );
        assert_eq!(r.hit, None);
    }

    #[test]
    fn snaps_to_bounds_and_grid() {
        let s = ViewSettings { show_grid: true, grid_spacing: 32, ..ViewSettings::default() };
        assert_eq!(snap_axis(498.0, Axis::Vertical, &[], Size::new(500, 400), &s, 1.0).hit, Some(SnapKind::Bounds));
        let r = snap_axis(66.0, Axis::Vertical, &[], Size::new(500, 400), &s, 1.0);
        assert_eq!((r.value, r.hit), (64.0, Some(SnapKind::Grid)));
    }

    #[test]
    fn disabled_or_invalid_input_is_untouched() {
        let s = ViewSettings { snap_enabled: false, ..settings() };
        assert_eq!(snap_axis(1.0, Axis::Vertical, &[], Size::new(10, 10), &s, 1.0).hit, None);
        assert_eq!(snap_axis(f64::NAN, Axis::Vertical, &[], Size::new(10, 10), &settings(), 1.0).hit, None);
    }

    #[test]
    fn hidden_guides_do_not_attract() {
        let g = [guide(Axis::Vertical, 100.0)];
        let s = ViewSettings { show_guides: false, show_grid: false, snap_to_bounds: false, ..ViewSettings::default() };
        assert_eq!(snap_axis(100.5, Axis::Vertical, &g, Size::new(500, 500), &s, 1.0).hit, None);
    }
}
