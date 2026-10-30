use serde::{Deserialize, Serialize};

use crate::geom::{IRect, Size, Vec2};

pub const MIN_ZOOM: f64 = 0.02;
pub const MAX_ZOOM: f64 = 64.0;

/// Paliers des raccourcis « zoom avant/arrière ».
pub const ZOOM_STEPS: [f64; 20] = [
    0.02, 0.03, 0.05, 0.0625, 0.125, 0.25, 0.333, 0.5, 0.667, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 32.0, 64.0,
];

/// Position du document dans la zone du canvas : `écran = document × zoom + origin` (en pixels
/// physiques, origine = coin haut-gauche du canvas).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub zoom: f64,
    pub origin: Vec2,
}

impl Default for Viewport {
    fn default() -> Self {
        Self { zoom: 1.0, origin: Vec2::default() }
    }
}

impl Viewport {
    pub fn doc_to_screen(&self, p: Vec2) -> Vec2 {
        Vec2::new(p.x * self.zoom + self.origin.x, p.y * self.zoom + self.origin.y)
    }

    pub fn screen_to_doc(&self, p: Vec2) -> Vec2 {
        Vec2::new((p.x - self.origin.x) / self.zoom, (p.y - self.origin.y) / self.zoom)
    }

    pub fn pan_by(&mut self, dx: f64, dy: f64) {
        self.origin.x += dx;
        self.origin.y += dy;
    }

    /// Change le zoom en gardant fixe le point du document situé sous `anchor` (curseur).
    pub fn zoom_at(&mut self, anchor: Vec2, new_zoom: f64) {
        let zoom = clamp_zoom(new_zoom);
        let doc = self.screen_to_doc(anchor);
        self.zoom = zoom;
        self.origin = Vec2::new(anchor.x - doc.x * zoom, anchor.y - doc.y * zoom);
    }

    /// Cadre tout le document avec une marge, centré. Ne dépasse jamais 100 % : un petit
    /// document n'est pas agrandi automatiquement.
    pub fn fit(&mut self, doc: Size, canvas: Size, margin: f64) {
        let avail_w = (f64::from(canvas.width) - 2.0 * margin).max(1.0);
        let avail_h = (f64::from(canvas.height) - 2.0 * margin).max(1.0);
        let zoom =
            clamp_zoom((avail_w / f64::from(doc.width.max(1))).min(avail_h / f64::from(doc.height.max(1))).min(1.0));
        self.zoom = zoom;
        self.center(doc, canvas);
    }

    pub fn center(&mut self, doc: Size, canvas: Size) {
        self.origin = Vec2::new(
            (f64::from(canvas.width) - f64::from(doc.width) * self.zoom) / 2.0,
            (f64::from(canvas.height) - f64::from(doc.height) * self.zoom) / 2.0,
        );
    }

    /// Empêche de perdre le document hors écran : au moins `keep` pixels doivent rester visibles.
    pub fn clamp_pan(&mut self, doc: Size, canvas: Size, keep: f64) {
        let (dw, dh) = (f64::from(doc.width) * self.zoom, f64::from(doc.height) * self.zoom);
        let (cw, ch) = (f64::from(canvas.width), f64::from(canvas.height));
        let keep_x = keep.min(dw).min(cw);
        let keep_y = keep.min(dh).min(ch);
        self.origin.x = self.origin.x.clamp(keep_x - dw, cw - keep_x);
        self.origin.y = self.origin.y.clamp(keep_y - dh, ch - keep_y);
    }

    /// Partie du document visible, arrondie vers l'extérieur.
    pub fn visible_doc_rect(&self, canvas: Size) -> IRect {
        let tl = self.screen_to_doc(Vec2::new(0.0, 0.0));
        let br = self.screen_to_doc(Vec2::new(f64::from(canvas.width), f64::from(canvas.height)));
        let (x0, y0) = (tl.x.floor(), tl.y.floor());
        let (x1, y1) = (br.x.ceil(), br.y.ceil());
        IRect::new(
            x0.clamp(-1e9, 1e9) as i32,
            y0.clamp(-1e9, 1e9) as i32,
            (x1 - x0).clamp(0.0, 1e9) as u32,
            (y1 - y0).clamp(0.0, 1e9) as u32,
        )
    }
}

pub fn clamp_zoom(z: f64) -> f64 {
    if z.is_nan() { 1.0 } else { z.clamp(MIN_ZOOM, MAX_ZOOM) }
}

/// Palier suivant dans le sens demandé ; reste sur place aux extrémités.
pub fn next_zoom_step(current: f64, zoom_in: bool) -> f64 {
    let eps = 1e-6;
    if zoom_in {
        ZOOM_STEPS.iter().copied().find(|s| *s > current + eps).unwrap_or(MAX_ZOOM)
    } else {
        ZOOM_STEPS.iter().rev().copied().find(|s| *s < current - eps).unwrap_or(MIN_ZOOM)
    }
}

/// Réglages d'affichage, communs à tous les documents.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewSettings {
    pub show_rulers: bool,
    pub show_guides: bool,
    pub show_grid: bool,
    pub grid_spacing: u32,
    pub snap_enabled: bool,
    pub snap_to_guides: bool,
    pub snap_to_grid: bool,
    pub snap_to_bounds: bool,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            show_rulers: true,
            show_guides: true,
            show_grid: false,
            grid_spacing: 32,
            snap_enabled: true,
            snap_to_guides: true,
            snap_to_grid: true,
            snap_to_bounds: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_doc_roundtrip() {
        let v = Viewport { zoom: 2.5, origin: Vec2::new(30.0, -12.0) };
        let p = Vec2::new(41.0, 7.5);
        let back = v.screen_to_doc(v.doc_to_screen(p));
        assert!((back.x - p.x).abs() < 1e-9 && (back.y - p.y).abs() < 1e-9);
    }

    #[test]
    fn zoom_at_keeps_anchor_fixed() {
        let mut v = Viewport { zoom: 1.0, origin: Vec2::new(100.0, 50.0) };
        let anchor = Vec2::new(300.0, 200.0);
        let before = v.screen_to_doc(anchor);
        v.zoom_at(anchor, 4.0);
        let after = v.screen_to_doc(anchor);
        assert!((before.x - after.x).abs() < 1e-9 && (before.y - after.y).abs() < 1e-9);
        assert_eq!(v.zoom, 4.0);
    }

    #[test]
    fn zoom_is_clamped_and_nan_safe() {
        assert_eq!(clamp_zoom(1000.0), MAX_ZOOM);
        assert_eq!(clamp_zoom(0.0), MIN_ZOOM);
        assert_eq!(clamp_zoom(f64::NAN), 1.0);
    }

    #[test]
    fn fit_centers_and_never_upscales() {
        let mut v = Viewport::default();
        v.fit(Size::new(100, 100), Size::new(800, 600), 20.0);
        assert_eq!(v.zoom, 1.0);
        assert_eq!(v.origin, Vec2::new(350.0, 250.0));
        v.fit(Size::new(4000, 2000), Size::new(800, 600), 0.0);
        assert!((v.zoom - 0.2).abs() < 1e-9);
    }

    #[test]
    fn zoom_steps_walk_both_ways() {
        assert_eq!(next_zoom_step(1.0, true), 1.5);
        assert_eq!(next_zoom_step(1.0, false), 0.667);
        assert_eq!(next_zoom_step(64.0, true), MAX_ZOOM);
        assert_eq!(next_zoom_step(0.02, false), MIN_ZOOM);
        // Entre deux paliers on rejoint le palier le plus proche dans le sens demandé.
        assert_eq!(next_zoom_step(1.2, true), 1.5);
        assert_eq!(next_zoom_step(1.2, false), 1.0);
    }

    #[test]
    fn clamp_pan_keeps_document_reachable() {
        let mut v = Viewport { zoom: 1.0, origin: Vec2::new(-5000.0, 5000.0) };
        v.clamp_pan(Size::new(400, 400), Size::new(800, 600), 50.0);
        assert_eq!(v.origin, Vec2::new(-350.0, 550.0));
    }

    #[test]
    fn visible_rect_covers_canvas() {
        let v = Viewport { zoom: 2.0, origin: Vec2::new(-10.0, 0.0) };
        let r = v.visible_doc_rect(Size::new(100, 50));
        assert_eq!((r.x, r.y, r.w, r.h), (5, 0, 50, 25));
    }
}
