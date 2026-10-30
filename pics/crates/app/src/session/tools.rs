//! Outils à la souris : sélections (rectangle, ellipse, lasso, polygone, baguette magique) et opérations
//! de sélection. Le frontend natif transmet des événements de pointeur ; toute la logique est ici, donc testable.

use openpics_core::commands::SetSelection;
use openpics_core::selection::{self, CombineMode};
use openpics_core::{TileGrid, Vec2};

use super::{Session, SessionError};
use crate::dto::{Tool, ToolOptionsDto};

/// Un événement de pointeur : position dans la zone du canvas (pixels physiques) et modificateurs.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pointer {
    pub x: f64,
    pub y: f64,
    pub pressure: f32,
    pub shift: bool,
    pub alt: bool,
}

impl Pointer {
    /// Maj = ajouter, Alt = soustraire, Maj + Alt = intersecter (convention des éditeurs raster).
    fn mode(&self) -> CombineMode {
        match (self.shift, self.alt) {
            (true, true) => CombineMode::Intersect,
            (true, false) => CombineMode::Add,
            (false, true) => CombineMode::Subtract,
            (false, false) => CombineMode::Replace,
        }
    }
}

/// Geste de sélection en cours.
pub(super) struct Drag {
    tool: Tool,
    start: (f64, f64),
    points: Vec<(f64, f64)>,
    mode: CombineMode,
}

impl Session {
    pub fn set_tool(&mut self, tool: Tool) {
        if self.tool != tool {
            self.tool_cancel();
        }
        self.tool = tool;
    }

    pub fn tool_options(&self) -> ToolOptionsDto {
        self.tool_options
    }

    pub fn set_tool_options(&mut self, options: ToolOptionsDto) {
        self.tool_options = options;
    }

    fn doc_point(&self, p: &Pointer) -> (f64, f64) {
        let d = self.viewport().screen_to_doc(Vec2::new(p.x, p.y));
        (d.x, d.y)
    }

    fn live_selection_mode(&self, tool: Tool, d: &Drag, current: (f64, f64)) -> Result<Option<TileGrid>, SessionError> {
        let size = self.entry()?.doc.size();
        let shape = match tool {
            Tool::RectSelect => openpics_ops::selection::rect(size.width, size.height, d.start, current)?,
            Tool::EllipseSelect => openpics_ops::selection::ellipse(size.width, size.height, d.start, current)?,
            Tool::Lasso => openpics_ops::selection::polygon(size.width, size.height, &d.points)?,
            Tool::PolygonLasso => {
                let mut pts = d.points.clone();
                pts.push(current);
                openpics_ops::selection::polygon(size.width, size.height, &pts)?
            }
            _ => return Ok(None),
        };
        Ok(Some(selection::combine(self.entry()?.doc.selection.as_ref(), &shape, d.mode)?))
    }

    pub fn pointer_down(&mut self, p: &Pointer) -> Result<(), SessionError> {
        self.filter_cancel();
        if !self.has_document() {
            return Ok(());
        }
        let at = self.doc_point(p);
        match self.tool {
            Tool::RectSelect | Tool::EllipseSelect | Tool::Lasso => {
                self.drag = Some(Drag { tool: self.tool, start: at, points: vec![at], mode: p.mode() });
            }
            Tool::PolygonLasso => match &mut self.drag {
                Some(d) => d.points.push(at),
                None => {
                    self.drag = Some(Drag { tool: Tool::PolygonLasso, start: at, points: vec![at], mode: p.mode() })
                }
            },
            Tool::MagicWand => self.wand(at, p.mode())?,
            Tool::Hand | Tool::Zoom => {}
            Tool::Text
            | Tool::ShapeRect
            | Tool::ShapeRoundedRect
            | Tool::ShapeEllipse
            | Tool::ShapeLine
            | Tool::Pen => self.vector_down(p, at)?,
            _ => self.paint_down(p, at)?,
        }
        Ok(())
    }

    pub fn pointer_move(&mut self, p: &Pointer) -> Result<(), SessionError> {
        let at = self.doc_point(p);
        if self.stroke.is_some() {
            return self.paint_move(p, at);
        }
        if self.shape_drag.is_some() || self.pen.is_some() {
            return self.vector_move(at);
        }
        let Some(mut drag) = self.drag.take() else { return Ok(()) };
        if drag.tool == Tool::Lasso {
            // Un point par pixel d'écran suffit ; au-delà on alourdit le polygone sans rien gagner.
            if drag.points.last().is_none_or(|l| (l.0 - at.0).hypot(l.1 - at.1) * self.viewport().zoom >= 1.0)
                && drag.points.len() < 50_000
            {
                drag.points.push(at);
            }
        }
        let preview = self.live_selection_mode(drag.tool, &drag, at);
        self.drag = Some(drag);
        self.selection_preview = preview?;
        Ok(())
    }

    pub fn pointer_up(&mut self, p: &Pointer) -> Result<(), SessionError> {
        let at = self.doc_point(p);
        if self.stroke.is_some() {
            return self.paint_up(p, at);
        }
        if self.shape_drag.is_some() || self.pen.is_some() {
            return self.vector_up(at);
        }
        let Some(drag) = self.drag.take() else { return Ok(()) };
        if drag.tool == Tool::PolygonLasso {
            // Le polygone se construit clic après clic ; il se ferme avec `tool_finish`.
            self.drag = Some(drag);
            return Ok(());
        }
        self.selection_preview = None;
        let moved = (drag.start.0 - at.0).hypot(drag.start.1 - at.1) * self.viewport().zoom >= 2.0;
        if !moved {
            // Un simple clic avec un outil de sélection désélectionne (sauf en mode « ajouter/soustraire »).
            if drag.mode == CombineMode::Replace {
                return self.deselect();
            }
            return Ok(());
        }
        if let Some(shape) = self.live_selection_mode(drag.tool, &drag, at)? {
            self.commit_selection(drag.tool_label(), shape)?;
        }
        Ok(())
    }

    /// Entrée : ferme le polygone en cours.
    pub fn tool_finish(&mut self) -> Result<(), SessionError> {
        if self.pen.is_some() {
            return self.pen_finish(false);
        }
        let Some(drag) = self.drag.take() else { return Ok(()) };
        self.selection_preview = None;
        if drag.tool == Tool::PolygonLasso && drag.points.len() >= 3 {
            let size = self.entry()?.doc.size();
            let shape = openpics_ops::selection::polygon(size.width, size.height, &drag.points)?;
            let combined = selection::combine(self.entry()?.doc.selection.as_ref(), &shape, drag.mode)?;
            self.commit_selection(drag.tool_label(), combined)?;
        }
        Ok(())
    }

    /// Échap : abandonne le geste en cours sans toucher à la sélection.
    pub fn tool_cancel(&mut self) {
        self.paint_cancel();
        self.vector_cancel();
        self.drag = None;
        self.selection_preview = None;
    }

    fn wand(&mut self, at: (f64, f64), mode: CombineMode) -> Result<(), SessionError> {
        let opts = self.tool_options;
        let e = self.entry()?;
        let (x, y) = (at.0.floor(), at.1.floor());
        let size = e.doc.size();
        if x < 0.0 || y < 0.0 || x >= f64::from(size.width) || y >= f64::from(size.height) {
            return Ok(());
        }
        let source = if opts.sample_all_layers {
            TileGrid::from_rgba8(size.width, size.height, &e.doc.flatten_rgba8())?
        } else {
            let id = e.active_layer.ok_or_else(|| SessionError::InvalidArgument("aucun calque sélectionné".into()))?;
            e.doc.layer(id)?.pixels.clone()
        };
        let shape = openpics_ops::selection::magic_wand(&source, x as u32, y as u32, opts.tolerance, opts.contiguous)?;
        let combined = selection::combine(self.entry()?.doc.selection.as_ref(), &shape, mode)?;
        self.commit_selection("Baguette magique", combined)
    }

    /// Enregistre une sélection dans l'historique ; une sélection vide équivaut à « désélectionner ».
    pub(super) fn commit_selection(&mut self, name: &str, mask: TileGrid) -> Result<(), SessionError> {
        let value = selection::bounds(&mask).map(|_| mask);
        self.entry_mut()?.doc.execute(Box::new(SetSelection::new(name, value)))?;
        Ok(())
    }

    pub fn select_all(&mut self) -> Result<(), SessionError> {
        let size = self.entry()?.doc.size();
        self.commit_selection("Tout sélectionner", selection::select_all(size.width, size.height)?)
    }

    pub fn deselect(&mut self) -> Result<(), SessionError> {
        if self.entry()?.doc.selection.is_none() {
            return Ok(());
        }
        self.entry_mut()?.doc.execute(Box::new(SetSelection::new("Désélectionner", None)))?;
        Ok(())
    }

    pub fn select_invert(&mut self) -> Result<(), SessionError> {
        let size = self.entry()?.doc.size();
        let inverted = match &self.entry()?.doc.selection {
            Some(m) => selection::invert(m)?,
            None => selection::select_all(size.width, size.height)?,
        };
        self.commit_selection("Inverser la sélection", inverted)
    }

    pub(super) fn selection_required(&self) -> Result<TileGrid, SessionError> {
        self.entry()?.doc.selection.clone().ok_or_else(|| SessionError::InvalidArgument("aucune sélection".into()))
    }

    pub fn select_feather(&mut self, radius: f32) -> Result<(), SessionError> {
        let m = openpics_ops::selection::feather(&self.selection_required()?, radius)?;
        self.commit_selection("Contour progressif", m)
    }

    pub fn select_grow(&mut self, radius: f32, grow: bool) -> Result<(), SessionError> {
        let m = openpics_ops::selection::grow(&self.selection_required()?, radius, grow)?;
        self.commit_selection(if grow { "Agrandir la sélection" } else { "Contracter la sélection" }, m)
    }
}

impl Drag {
    fn tool_label(&self) -> &'static str {
        match self.tool {
            Tool::RectSelect => "Sélection rectangulaire",
            Tool::EllipseSelect => "Sélection elliptique",
            Tool::Lasso => "Lasso",
            Tool::PolygonLasso => "Lasso polygonal",
            _ => "Sélection",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use openpics_core::ViewSettings;
    use openpics_core::shortcuts::Keymap;

    /// Document 100×100 dans un canvas 800×600 : zoom 100 %, origine du document en (350, 250).
    fn session() -> Session {
        let mut s = Session::new(ViewSettings::default(), Keymap::default());
        s.set_canvas(800, 600);
        s.new_document(None, 100, 100).unwrap();
        s
    }

    fn at(x: f64, y: f64) -> Pointer {
        Pointer { x: 350.0 + x, y: 250.0 + y, pressure: 1.0, ..Pointer::default() }
    }

    fn drag(s: &mut Session, a: (f64, f64), b: (f64, f64), shift: bool, alt: bool) {
        let m = |p: (f64, f64)| Pointer { shift, alt, ..at(p.0, p.1) };
        s.pointer_down(&m(a)).unwrap();
        s.pointer_move(&m(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0))).unwrap();
        s.pointer_move(&m(b)).unwrap();
        s.pointer_up(&m(b)).unwrap();
    }

    fn sel(s: &Session, x: u32, y: u32) -> u8 {
        s.render_state().document.unwrap().selection.map_or(0, |m| selection::value(&m, x, y))
    }

    #[test]
    fn rectangle_drag_selects_previews_and_is_undoable() {
        let mut s = session();
        s.set_tool(Tool::RectSelect);
        s.pointer_down(&at(10.0, 10.0)).unwrap();
        s.pointer_move(&at(40.0, 30.0)).unwrap();
        assert!(s.render_state().selection.is_some(), "aperçu pendant le geste");
        assert_eq!(sel(&s, 20, 20), 0, "pas encore dans le document");
        s.pointer_up(&at(40.0, 30.0)).unwrap();
        assert_eq!((sel(&s, 20, 20), sel(&s, 50, 50)), (255, 0));
        assert!(s.snapshot().has_selection);
        assert_eq!(s.snapshot().history.names.last().unwrap(), "Sélection rectangulaire");
        s.undo().unwrap();
        assert!(!s.snapshot().has_selection);
    }

    #[test]
    fn modifier_keys_add_subtract_and_intersect() {
        let mut s = session();
        s.set_tool(Tool::RectSelect);
        drag(&mut s, (0.0, 0.0), (40.0, 40.0), false, false);
        drag(&mut s, (30.0, 30.0), (70.0, 70.0), true, false); // Maj : ajouter
        assert_eq!((sel(&s, 10, 10), sel(&s, 60, 60)), (255, 255));
        drag(&mut s, (0.0, 0.0), (20.0, 20.0), false, true); // Alt : soustraire
        assert_eq!((sel(&s, 10, 10), sel(&s, 25, 25)), (0, 255));
        drag(&mut s, (20.0, 20.0), (50.0, 50.0), true, true); // Maj+Alt : intersecter
        assert_eq!((sel(&s, 25, 25), sel(&s, 60, 60)), (255, 0));
    }

    #[test]
    fn a_plain_click_deselects_but_a_modified_click_does_not() {
        let mut s = session();
        s.set_tool(Tool::EllipseSelect);
        drag(&mut s, (10.0, 10.0), (60.0, 60.0), false, false);
        assert!(s.snapshot().has_selection);
        s.pointer_down(&Pointer { shift: true, ..at(5.0, 5.0) }).unwrap();
        s.pointer_up(&Pointer { shift: true, ..at(5.0, 5.0) }).unwrap();
        assert!(s.snapshot().has_selection);
        s.pointer_down(&at(5.0, 5.0)).unwrap();
        s.pointer_up(&at(5.0, 5.0)).unwrap();
        assert!(!s.snapshot().has_selection);
    }

    #[test]
    fn lasso_and_polygon_build_shapes_and_cancel_discards_them() {
        let mut s = session();
        s.set_tool(Tool::Lasso);
        s.pointer_down(&at(10.0, 10.0)).unwrap();
        for p in [(50.0, 10.0), (50.0, 50.0), (10.0, 50.0)] {
            s.pointer_move(&at(p.0, p.1)).unwrap();
        }
        s.pointer_up(&at(10.0, 49.0)).unwrap();
        assert_eq!((sel(&s, 30, 30), sel(&s, 70, 70)), (255, 0));

        s.deselect().unwrap();
        s.set_tool(Tool::PolygonLasso);
        for p in [(10.0, 10.0), (60.0, 10.0), (10.0, 60.0)] {
            s.pointer_down(&at(p.0, p.1)).unwrap();
            s.pointer_up(&at(p.0, p.1)).unwrap();
        }
        assert!(!s.snapshot().has_selection, "le polygone n'est validé qu'à la fermeture");
        s.tool_finish().unwrap();
        assert_eq!((sel(&s, 20, 20), sel(&s, 55, 55)), (255, 0));

        s.deselect().unwrap();
        s.pointer_down(&at(10.0, 10.0)).unwrap();
        s.pointer_up(&at(10.0, 10.0)).unwrap();
        s.tool_cancel();
        s.tool_finish().unwrap();
        assert!(!s.snapshot().has_selection);
        // Changer d'outil abandonne aussi un polygone commencé.
        s.pointer_down(&at(10.0, 10.0)).unwrap();
        s.set_tool(Tool::RectSelect);
        s.tool_finish().unwrap();
        assert!(s.render_state().selection.is_none());
    }

    #[test]
    fn magic_wand_uses_the_active_layer_tolerance_and_contiguity() {
        let mut s = session();
        s.set_tool(Tool::MagicWand);
        // Calque actif : fond blanc ; on y trace un rectangle rouge via une sélection + inversion des couleurs.
        s.set_tool(Tool::RectSelect);
        drag(&mut s, (20.0, 20.0), (60.0, 60.0), false, false);
        s.apply_op(crate::dto::ImageOp::Invert).unwrap(); // blanc → noir dans la zone
        s.deselect().unwrap();
        s.set_tool(Tool::MagicWand);
        s.pointer_down(&at(30.0, 30.0)).unwrap();
        assert_eq!((sel(&s, 30, 30), sel(&s, 80, 80)), (255, 0), "seule la zone noire");
        assert_eq!(s.snapshot().history.names.last().unwrap(), "Baguette magique");
        s.deselect().unwrap();
        s.set_tool_options(ToolOptionsDto { tolerance: 255, ..ToolOptionsDto::default() });
        s.pointer_down(&at(30.0, 30.0)).unwrap();
        assert_eq!(sel(&s, 80, 80), 255, "tolérance maximale : tout est proche");
        // En dehors du document : sans effet.
        s.deselect().unwrap();
        s.pointer_down(&at(-50.0, -50.0)).unwrap();
        assert!(!s.snapshot().has_selection);
    }

    #[test]
    fn select_menu_operations() {
        let mut s = session();
        s.select_all().unwrap();
        assert_eq!(sel(&s, 99, 99), 255);
        s.select_invert().unwrap();
        assert!(!s.snapshot().has_selection, "inverser « tout » ne laisse rien : sélection vide = aucune");
        s.select_invert().unwrap();
        assert_eq!(sel(&s, 5, 5), 255, "inverser « rien » sélectionne tout");
        s.deselect().unwrap();
        assert!(s.select_feather(5.0).is_err() && s.select_grow(3.0, true).is_err());
        s.set_tool(Tool::RectSelect);
        drag(&mut s, (30.0, 30.0), (70.0, 70.0), false, false);
        s.select_grow(5.0, true).unwrap();
        assert_eq!((sel(&s, 25, 50), sel(&s, 24, 50)), (255, 0));
        s.select_grow(5.0, false).unwrap();
        s.select_feather(6.0).unwrap();
        let edge = sel(&s, 30, 50);
        assert!(edge > 40 && edge < 220, "{edge}");
    }
}
