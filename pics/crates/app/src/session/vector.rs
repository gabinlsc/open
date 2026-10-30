//! Texte, formes vectorielles et plume : création, édition, opérations booléennes, chemin → sélection, SVG.
//! Le modèle (texte ou chemins) est la source ; les pixels du calque en sont le rendu, recalculé à chaque édition.

use std::path::Path as FsPath;

use openpics_core::commands::{AddLayer, RemoveLayer, SetLayerContent};
use openpics_core::text::{TextAlign, TextModel};
use openpics_core::vector::{Anchor, FillRule, Path, Stroke, VectorModel};
use openpics_core::{Layer, LayerId, LayerKind, TileGrid};
use openpics_ops::vector::{BooleanOp, boolean, import_svg, render, to_selection};

use super::tools::Pointer;
use super::{Session, SessionError};
use crate::dto::{ShapeDto, TextAlignDto, TextDto, Tool, parse_hex};

/// En-dessous de cette taille de glissé (px d'écran), une forme n'est pas créée : c'était un simple clic.
const MIN_DRAG_PX: f64 = 3.0;

/// Tracé de la plume en cours de construction.
pub(super) struct PenState {
    anchors: Vec<Anchor>,
    /// Une ancre vient d'être posée, la souris est enfoncée : le glissé en règle les poignées.
    dragging: bool,
}

impl Session {
    fn raster_for(&self, model: &VectorModel) -> Result<TileGrid, SessionError> {
        let size = self.entry()?.doc.size();
        Ok(render(model, size.width, size.height)?)
    }

    fn insert_layer(&mut self, name: String, kind: LayerKind, pixels: TileGrid) -> Result<u64, SessionError> {
        let e = self.entry_mut()?;
        let index = e
            .active_layer
            .and_then(|a| e.doc.layers.iter().position(|l| l.id == a))
            .map_or(e.doc.layers.len(), |i| i + 1);
        let parent = index.checked_sub(1).and_then(|i| e.doc.layers.get(i)).and_then(|l| l.group);
        let id = LayerId(e.doc.alloc_id());
        let mut layer = Layer::new(id, name, pixels);
        layer.kind = kind;
        layer.group = parent;
        e.doc.execute(Box::new(AddLayer::new(index, layer)))?;
        e.active_layer = Some(id);
        Ok(id.0)
    }

    // ---- texte ----------------------------------------------------------------------------------------------------

    fn render_text(&self, model: &TextModel) -> Result<TileGrid, SessionError> {
        let size = self.entry()?.doc.size();
        openpics_ops::text::render(model, size.width, size.height).map_err(SessionError::InvalidArgument)
    }

    pub fn text_add(&mut self, x: f64, y: f64) -> Result<u64, SessionError> {
        let fg = self.paint_options().fg();
        let model = TextModel { origin: (x as f32, y as f32), color: fg, ..TextModel::default() }.sanitized();
        let pixels = self.render_text(&model)?;
        self.insert_layer("Texte".into(), LayerKind::Text(Box::new(model)), pixels)
    }

    pub fn text_set(&mut self, id: u64, dto: &TextDto) -> Result<(), SessionError> {
        let current = self
            .entry()?
            .doc
            .layer(LayerId(id))?
            .text()
            .cloned()
            .ok_or_else(|| SessionError::InvalidArgument("ce calque n'est pas un calque de texte".into()))?;
        let color = parse_hex(&dto.color)
            .ok_or_else(|| SessionError::InvalidArgument(format!("couleur invalide : {}", dto.color)))?;
        let model = TextModel {
            text: dto.text.clone(),
            font: dto.font.clone(),
            size: dto.size,
            color,
            bold: dto.bold,
            italic: dto.italic,
            align: match dto.align {
                TextAlignDto::Left => TextAlign::Left,
                TextAlignDto::Center => TextAlign::Center,
                TextAlignDto::Right => TextAlign::Right,
                TextAlignDto::Justify => TextAlign::Justify,
            },
            line_height: dto.line_height,
            box_width: dto.box_width,
            origin: (dto.origin_x, dto.origin_y),
            // Le tracé du texte se règle séparément (`text_set_path`) : on le conserve.
            path: current.path.clone(),
        }
        .sanitized();
        if model == current {
            return Ok(());
        }
        let pixels = self.render_text(&model)?;
        self.entry_mut()?.doc.execute(Box::new(SetLayerContent::new(
            "Texte",
            LayerId(id),
            LayerKind::Text(Box::new(model)),
            pixels,
        )))?;
        Ok(())
    }

    /// Fait suivre au texte le chemin d'un calque de forme (`None` = retour au texte droit).
    pub fn text_set_path(&mut self, id: u64, from_shape: Option<u64>) -> Result<(), SessionError> {
        let mut model = self
            .entry()?
            .doc
            .layer(LayerId(id))?
            .text()
            .cloned()
            .ok_or_else(|| SessionError::InvalidArgument("ce calque n'est pas un calque de texte".into()))?;
        model.path =
            match from_shape {
                Some(s) => {
                    let shape =
                        self.entry()?.doc.layer(LayerId(s))?.shape().cloned().ok_or_else(|| {
                            SessionError::InvalidArgument("le calque choisi n'est pas une forme".into())
                        })?;
                    Some(
                        shape
                            .paths
                            .into_iter()
                            .next()
                            .ok_or_else(|| SessionError::InvalidArgument("la forme n'a pas de chemin".into()))?,
                    )
                }
                None => None,
            };
        let pixels = self.render_text(&model)?;
        self.entry_mut()?.doc.execute(Box::new(SetLayerContent::new(
            "Texte sur tracé",
            LayerId(id),
            LayerKind::Text(Box::new(model)),
            pixels,
        )))?;
        Ok(())
    }

    // ---- formes ---------------------------------------------------------------------------------------------------

    fn shape_model(&self, path: Path) -> VectorModel {
        let fg = self.paint_options().fg();
        let open = !path.closed;
        VectorModel {
            paths: vec![path],
            fill: if open { None } else { Some([fg[0], fg[1], fg[2], 255]) },
            // Un chemin ouvert n'a rien à remplir : il reçoit un contour de la couleur de premier plan.
            stroke: open.then_some(Stroke { color: [fg[0], fg[1], fg[2], 255], width: 3.0, round_caps: true }),
            fill_rule: FillRule::NonZero,
        }
    }

    fn create_shape(&mut self, name: &str, path: Path) -> Result<u64, SessionError> {
        let model = self.shape_model(path).sanitized();
        let pixels = self.raster_for(&model)?;
        self.insert_layer(name.to_owned(), LayerKind::Shape(Box::new(model)), pixels)
    }

    pub fn shape_set_style(&mut self, id: u64, dto: &ShapeDto) -> Result<(), SessionError> {
        let current = self
            .entry()?
            .doc
            .layer(LayerId(id))?
            .shape()
            .cloned()
            .ok_or_else(|| SessionError::InvalidArgument("ce calque n'est pas une forme".into()))?;
        let bad = |c: &str| SessionError::InvalidArgument(format!("couleur invalide : {c}"));
        let fill = match &dto.fill {
            Some(f) => {
                let c = parse_hex(&f.color).ok_or_else(|| bad(&f.color))?;
                Some([c[0], c[1], c[2], (f.opacity.clamp(0.0, 1.0) * 255.0).round() as u8])
            }
            None => None,
        };
        let stroke = match &dto.stroke {
            Some(s) => {
                let c = parse_hex(&s.color).ok_or_else(|| bad(&s.color))?;
                Some(Stroke {
                    color: [c[0], c[1], c[2], (s.opacity.clamp(0.0, 1.0) * 255.0).round() as u8],
                    width: s.width,
                    round_caps: s.round_caps,
                })
            }
            None => None,
        };
        let model = VectorModel {
            fill,
            stroke,
            fill_rule: if dto.even_odd { FillRule::EvenOdd } else { FillRule::NonZero },
            paths: current.paths.clone(),
        }
        .sanitized();
        if model == current {
            return Ok(());
        }
        let pixels = self.raster_for(&model)?;
        self.entry_mut()?.doc.execute(Box::new(SetLayerContent::new(
            "Style de forme",
            LayerId(id),
            LayerKind::Shape(Box::new(model)),
            pixels,
        )))?;
        Ok(())
    }

    /// Fusionne la forme `b` dans la forme `a` (le calque `b` disparaît), avec l'opération booléenne demandée.
    pub fn shape_boolean(&mut self, a: u64, b: u64, op: BooleanOp) -> Result<(), SessionError> {
        if a == b {
            return Err(SessionError::InvalidArgument("choisissez deux formes différentes".into()));
        }
        let get = |s: &Session, id: u64| -> Result<VectorModel, SessionError> {
            s.entry()?
                .doc
                .layer(LayerId(id))?
                .shape()
                .cloned()
                .ok_or_else(|| SessionError::InvalidArgument("le calque n'est pas une forme".into()))
        };
        let (ma, mb) = (get(self, a)?, get(self, b)?);
        let result = boolean(&ma, &mb, op);
        let pixels = self.raster_for(&result)?;
        let e = self.entry_mut()?;
        e.doc.execute(Box::new(SetLayerContent::new(
            "Opération booléenne",
            LayerId(a),
            LayerKind::Shape(Box::new(result)),
            pixels,
        )))?;
        e.doc.execute(Box::new(RemoveLayer::new(LayerId(b))))?;
        e.active_layer = Some(LayerId(a));
        Ok(())
    }

    pub fn shape_to_selection(&mut self, id: u64) -> Result<(), SessionError> {
        let model = self
            .entry()?
            .doc
            .layer(LayerId(id))?
            .shape()
            .cloned()
            .ok_or_else(|| SessionError::InvalidArgument("ce calque n'est pas une forme".into()))?;
        let size = self.entry()?.doc.size();
        let mask = to_selection(&model, size.width, size.height)?;
        self.commit_selection("Tracé en sélection", mask)
    }

    pub fn shape_export_svg(&self, id: u64, path: &FsPath) -> Result<(), SessionError> {
        let e = self.entry()?;
        let model = e
            .doc
            .layer(LayerId(id))?
            .shape()
            .ok_or_else(|| SessionError::InvalidArgument("ce calque n'est pas une forme".into()))?;
        let size = e.doc.size();
        std::fs::write(path, model.to_svg(size.width, size.height))
            .map_err(|err| SessionError::InvalidArgument(format!("{} : {err}", path.display())))
    }

    /// Retour à un calque de pixels (le texte ou la forme n'est plus modifiable ensuite).
    pub fn layer_rasterize(&mut self, id: u64) -> Result<(), SessionError> {
        let l = self.entry()?.doc.layer(LayerId(id))?;
        if l.is_raster() || !l.has_pixels() {
            return Err(SessionError::InvalidArgument("ce calque n'a rien à rastériser".into()));
        }
        let pixels = l.pixels.clone();
        self.entry_mut()?.doc.execute(Box::new(SetLayerContent::new(
            "Rastériser le calque",
            LayerId(id),
            LayerKind::Raster,
            pixels,
        )))?;
        Ok(())
    }

    /// Importe un SVG comme nouveau calque raster, à l'échelle 1 depuis le coin haut-gauche du document.
    pub fn svg_import(&mut self, path: &FsPath) -> Result<u64, SessionError> {
        let data =
            std::fs::read(path).map_err(|e| SessionError::InvalidArgument(format!("{} : {e}", path.display())))?;
        let size = self.entry()?.doc.size();
        let image = import_svg(&data, size.width.max(size.height)).map_err(SessionError::InvalidArgument)?;
        let mut canvas = TileGrid::new(size.width, size.height)?;
        for y in 0..image.height().min(size.height) {
            for x in 0..image.width().min(size.width) {
                let p = image.get_pixel(x, y);
                if p[3] > 0 {
                    canvas.put_pixel(x, y, p);
                }
            }
        }
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("SVG").to_owned();
        self.insert_layer(name, LayerKind::Raster, canvas)
    }

    // ---- outils à la souris ---------------------------------------------------------------------------------------

    pub(super) fn vector_down(&mut self, p: &Pointer, at: (f64, f64)) -> Result<(), SessionError> {
        match self.tool {
            Tool::Text => {
                self.text_add(at.0, at.1)?;
            }
            Tool::ShapeRect | Tool::ShapeRoundedRect | Tool::ShapeEllipse | Tool::ShapeLine => {
                self.shape_drag = Some((at, p.shift));
            }
            Tool::Pen => {
                let zoom = self.viewport().zoom;
                let pen = self.pen.get_or_insert(PenState { anchors: Vec::new(), dragging: false });
                // Cliquer près du premier point ferme le tracé.
                let close = pen.anchors.len() >= 3 && {
                    let f = pen.anchors[0].pos;
                    (f.0 - at.0).hypot(f.1 - at.1) * zoom <= 8.0
                };
                if close {
                    return self.pen_finish(true);
                }
                pen.anchors.push(Anchor::corner(at.0, at.1));
                pen.dragging = true;
                self.pen_preview()?;
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn vector_move(&mut self, at: (f64, f64)) -> Result<(), SessionError> {
        if let Some((start, square)) = self.shape_drag {
            self.selection_preview = self.shape_outline(start, at, square)?;
            return Ok(());
        }
        if let Some(pen) = &mut self.pen {
            if pen.dragging {
                if let Some(a) = pen.anchors.last_mut() {
                    // Glisser depuis un point règle des poignées symétriques (courbe lisse).
                    let h = (at.0 - a.pos.0, at.1 - a.pos.1);
                    a.h_out = h;
                    a.h_in = (-h.0, -h.1);
                }
            }
        }
        Ok(())
    }

    pub(super) fn vector_up(&mut self, at: (f64, f64)) -> Result<(), SessionError> {
        if let Some(pen) = &mut self.pen {
            pen.dragging = false;
        }
        let Some((start, square)) = self.shape_drag.take() else { return Ok(()) };
        self.selection_preview = None;
        if (start.0 - at.0).hypot(start.1 - at.1) * self.viewport().zoom < MIN_DRAG_PX {
            return Ok(());
        }
        let (mut x1, mut y1) = at;
        if square {
            let s = (x1 - start.0).abs().max((y1 - start.1).abs());
            x1 = start.0 + s * (x1 - start.0).signum();
            y1 = start.1 + s * (y1 - start.1).signum();
        }
        let (x0, y0, w, h) = (start.0.min(x1), start.1.min(y1), (x1 - start.0).abs(), (y1 - start.1).abs());
        let (name, path) = match self.tool {
            Tool::ShapeRect => ("Rectangle", Path::rect(x0, y0, w, h)),
            Tool::ShapeRoundedRect => ("Rectangle arrondi", Path::rounded_rect(x0, y0, w, h, w.min(h) * 0.2)),
            Tool::ShapeEllipse => ("Ellipse", Path::ellipse(x0 + w / 2.0, y0 + h / 2.0, w / 2.0, h / 2.0)),
            Tool::ShapeLine => ("Ligne", Path::line(start, (x1, y1))),
            _ => return Ok(()),
        };
        self.create_shape(name, path)?;
        Ok(())
    }

    fn shape_outline(&self, start: (f64, f64), at: (f64, f64), square: bool) -> Result<Option<TileGrid>, SessionError> {
        let size = self.entry()?.doc.size();
        let (mut x1, mut y1) = at;
        if square {
            let s = (x1 - start.0).abs().max((y1 - start.1).abs());
            x1 = start.0 + s * (x1 - start.0).signum();
            y1 = start.1 + s * (y1 - start.1).signum();
        }
        Ok(Some(match self.tool {
            Tool::ShapeEllipse => openpics_ops::selection::ellipse(size.width, size.height, start, (x1, y1))?,
            Tool::ShapeLine => {
                openpics_ops::selection::polygon(size.width, size.height, &[start, (x1, y1), (x1 + 0.01, y1 + 0.01)])?
            }
            _ => openpics_ops::selection::rect(size.width, size.height, start, (x1, y1))?,
        }))
    }

    /// Aperçu du tracé de la plume : le polygone des ancres est montré comme contour.
    fn pen_preview(&mut self) -> Result<(), SessionError> {
        let size = self.entry()?.doc.size();
        let Some(pen) = &self.pen else { return Ok(()) };
        let pts: Vec<_> = pen.anchors.iter().map(|a| a.pos).collect();
        self.selection_preview =
            if pts.len() >= 3 { Some(openpics_ops::selection::polygon(size.width, size.height, &pts)?) } else { None };
        Ok(())
    }

    /// Termine le tracé de la plume : crée une forme (fermée = remplie, ouverte = contour).
    pub(super) fn pen_finish(&mut self, closed: bool) -> Result<(), SessionError> {
        let Some(pen) = self.pen.take() else { return Ok(()) };
        self.selection_preview = None;
        if pen.anchors.len() < 2 {
            return Ok(());
        }
        self.create_shape("Tracé", Path { anchors: pen.anchors, closed })?;
        Ok(())
    }

    pub(super) fn vector_cancel(&mut self) {
        self.shape_drag = None;
        self.pen = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpics_core::ViewSettings;
    use openpics_core::shortcuts::Keymap;

    fn session() -> Session {
        let mut s = Session::new(ViewSettings::default(), Keymap::default());
        s.set_canvas(800, 600);
        s.new_document(None, 200, 100).unwrap();
        s
    }

    fn fonts_ok() -> bool {
        let ok = !openpics_ops::text::families().is_empty();
        if !ok {
            assert!(std::env::var_os("OPENPICS_REQUIRE_FONTS").is_none());
            eprintln!("test texte ignoré : aucune police système");
        }
        ok
    }

    fn at(x: f64, y: f64) -> Pointer {
        Pointer { x: 300.0 + x, y: 250.0 + y, pressure: 1.0, ..Pointer::default() }
    }

    fn drag(s: &mut Session, a: (f64, f64), b: (f64, f64)) {
        s.pointer_down(&at(a.0, a.1)).unwrap();
        s.pointer_move(&at(b.0, b.1)).unwrap();
        s.pointer_up(&at(b.0, b.1)).unwrap();
    }

    fn layer_px(s: &Session, id: u64, x: u32, y: u32) -> [u8; 4] {
        s.render_state().document.unwrap().layer(LayerId(id)).unwrap().pixels.get_pixel(x, y)
    }

    #[test]
    fn shape_tools_create_editable_shape_layers_with_one_history_entry() {
        let mut s = session();
        let mut o = s.paint_options().clone();
        o.foreground = "#ff0000".into();
        s.set_paint_options(&o);
        s.set_tool(Tool::ShapeRect);
        let n = s.snapshot().history.names.len();
        s.pointer_down(&at(20.0, 20.0)).unwrap();
        s.pointer_move(&at(60.0, 50.0)).unwrap();
        assert!(s.render_state().selection.is_some(), "contour d'aperçu pendant le glissé");
        s.pointer_up(&at(60.0, 50.0)).unwrap();
        assert!(s.render_state().selection.is_none());
        let snap = s.snapshot();
        assert_eq!(snap.history.names.len(), n + 1);
        let id = snap.active_layer_id.unwrap();
        assert!(snap.layers[0].shape.is_some() && snap.layers[0].name == "Rectangle");
        assert_eq!(layer_px(&s, id, 30, 30), [255, 0, 0, 255]);
        assert_eq!(layer_px(&s, id, 10, 10)[3], 0);
        // Un simple clic ne crée rien.
        s.pointer_down(&at(80.0, 80.0)).unwrap();
        s.pointer_up(&at(80.0, 80.0)).unwrap();
        assert_eq!(s.snapshot().layers.len(), 2);
        // Le style est éditable : remplissage bleu + contour.
        let dto = ShapeDto {
            fill: Some(crate::dto::ColorOpacityDto { color: "#0000ff".into(), opacity: 1.0 }),
            stroke: Some(crate::dto::StrokeDto {
                color: "#00ff00".into(),
                opacity: 1.0,
                width: 4.0,
                round_caps: false,
            }),
            even_odd: false,
            path_count: 1,
        };
        s.shape_set_style(id, &dto).unwrap();
        assert_eq!(layer_px(&s, id, 40, 35), [0, 0, 255, 255]);
        assert_eq!(layer_px(&s, id, 20, 35), [0, 255, 0, 255], "contour");
        s.undo().unwrap();
        assert_eq!(layer_px(&s, id, 40, 35), [255, 0, 0, 255]);
        assert!(
            s.shape_set_style(
                id,
                &ShapeDto { fill: Some(crate::dto::ColorOpacityDto { color: "x".into(), opacity: 1.0 }), ..dto }
            )
            .is_err()
        );
    }

    #[test]
    fn ellipse_line_and_pen_tools() {
        let mut s = session();
        s.set_tool(Tool::ShapeEllipse);
        drag(&mut s, (20.0, 20.0), (80.0, 60.0));
        let e = s.snapshot().active_layer_id.unwrap();
        assert_eq!(layer_px(&s, e, 50, 40)[3], 255);
        assert_eq!(layer_px(&s, e, 21, 21)[3], 0, "le coin du rectangle englobant est hors de l'ellipse");
        s.set_tool(Tool::ShapeLine);
        drag(&mut s, (10.0, 90.0), (190.0, 90.0));
        let l = s.snapshot().active_layer_id.unwrap();
        assert!(layer_px(&s, l, 100, 90)[3] > 200, "la ligne a un contour (3 px)");
        assert!(s.snapshot().layers[0].shape.as_ref().unwrap().stroke.is_some());
        // Plume : trois points puis Entrée = chemin ouvert ; refermer sur le premier = forme remplie.
        s.set_tool(Tool::Pen);
        for p in [(100.0, 10.0), (150.0, 10.0), (150.0, 40.0)] {
            s.pointer_down(&at(p.0, p.1)).unwrap();
            s.pointer_up(&at(p.0, p.1)).unwrap();
        }
        assert!(s.render_state().selection.is_some(), "aperçu du tracé");
        s.tool_finish().unwrap();
        let open = s.snapshot();
        assert!(
            open.layers[0].shape.as_ref().unwrap().stroke.is_some()
                && open.layers[0].shape.as_ref().unwrap().fill.is_none()
        );
        for p in [(100.0, 50.0), (140.0, 50.0), (140.0, 80.0), (100.0, 80.0), (100.0, 50.0)] {
            s.pointer_down(&at(p.0, p.1)).unwrap();
            s.pointer_up(&at(p.0, p.1)).unwrap();
        }
        let closed = s.snapshot();
        assert!(closed.layers[0].shape.as_ref().unwrap().fill.is_some(), "refermé : rempli");
        // Poignées : un glissé règle une courbe lisse.
        s.pointer_down(&at(10.0, 10.0)).unwrap();
        s.pointer_move(&at(30.0, 10.0)).unwrap();
        s.pointer_up(&at(30.0, 10.0)).unwrap();
        s.pointer_down(&at(60.0, 10.0)).unwrap();
        s.pointer_up(&at(60.0, 10.0)).unwrap();
        s.tool_finish().unwrap();
        let id = s.snapshot().active_layer_id.unwrap();
        let first = s.render_state().document.unwrap().layer(LayerId(id)).unwrap().shape().unwrap().paths[0].anchors[0];
        assert_eq!((first.h_out, first.h_in), ((20.0, 0.0), (-20.0, 0.0)));
    }

    #[test]
    fn booleans_selection_svg_rasterize_and_import() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = session();
        s.set_tool(Tool::ShapeRect);
        drag(&mut s, (10.0, 10.0), (60.0, 60.0));
        let a = s.snapshot().active_layer_id.unwrap();
        drag(&mut s, (40.0, 40.0), (90.0, 90.0));
        let b = s.snapshot().active_layer_id.unwrap();
        s.shape_to_selection(a).unwrap();
        assert!(s.snapshot().has_selection);
        let svg = dir.path().join("a.svg");
        s.shape_export_svg(a, &svg).unwrap();
        assert!(std::fs::read_to_string(&svg).unwrap().contains("<path"));
        assert!(s.shape_boolean(a, a, BooleanOp::Union).is_err());
        s.shape_boolean(a, b, BooleanOp::Intersect).unwrap();
        let snap = s.snapshot();
        assert_eq!(snap.layers.len(), 2, "la forme B a été fusionnée");
        assert_eq!(
            (layer_px(&s, a, 50, 50)[3], layer_px(&s, a, 20, 20)[3]),
            (255, 0),
            "intersection : seul le carré commun reste"
        );
        s.layer_rasterize(a).unwrap();
        assert!(s.snapshot().layers[0].shape.is_none());
        assert!(s.layer_rasterize(a).is_err());
        let file = dir.path().join("rond.svg");
        std::fs::write(&file, r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><circle cx="20" cy="20" r="15" fill="#00ff00"/></svg>"##).unwrap();
        let imported = s.svg_import(&file).unwrap();
        assert_eq!(layer_px(&s, imported, 20, 20), [0, 255, 0, 255]);
        assert_eq!(s.snapshot().layers[0].name, "rond");
        std::fs::write(&file, "pas un svg").unwrap();
        assert!(s.svg_import(&file).is_err());
    }

    #[test]
    fn text_layers_render_edit_follow_a_path_and_rasterize() {
        if !fonts_ok() {
            return;
        }
        let mut s = session();
        s.set_tool(Tool::Text);
        s.pointer_down(&at(10.0, 10.0)).unwrap();
        s.pointer_up(&at(10.0, 10.0)).unwrap();
        let snap = s.snapshot();
        let id = snap.active_layer_id.unwrap();
        let t = snap.layers[0].text.clone().unwrap();
        assert_eq!((t.text.as_str(), snap.layers[0].name.as_str()), ("Texte", "Texte"));
        let ink = |s: &Session| {
            (0..200)
                .flat_map(|x| (0..100).map(move |y| (x, y)))
                .filter(|&(x, y)| layer_px(s, id, x, y)[3] > 100)
                .count()
        };
        let before = ink(&s);
        assert!(before > 30);
        s.text_set(
            id,
            &TextDto { text: "Texte plus long".into(), size: 20.0, font: "DejaVu Sans".into(), ..t.clone() },
        )
        .unwrap();
        assert_ne!(ink(&s), before);
        let n = s.snapshot().history.names.len();
        s.text_set(
            id,
            &TextDto { text: "Texte plus long".into(), size: 20.0, font: "DejaVu Sans".into(), ..t.clone() },
        )
        .unwrap();
        assert_eq!(s.snapshot().history.names.len(), n, "sans changement : pas d'état");
        assert!(s.text_set(id, &TextDto { color: "rouge".into(), ..t.clone() }).is_err());
        s.text_set_path(id, None).unwrap();
        s.undo().unwrap();
        s.set_tool(Tool::ShapeLine);
        drag(&mut s, (10.0, 80.0), (190.0, 80.0));
        let line = s.snapshot().active_layer_id.unwrap();
        s.text_set_path(id, Some(line)).unwrap();
        assert!(s.snapshot().layers.iter().find(|l| l.id == id).unwrap().text.as_ref().unwrap().has_path);
        assert!(s.text_set_path(id, Some(id)).is_err(), "un texte n'est pas une forme");
        s.layer_rasterize(id).unwrap();
        assert!(s.snapshot().layers.iter().find(|l| l.id == id).unwrap().text.is_none());
        let some_shape = s.snapshot().layers[0].id;
        assert!(s.text_set(some_shape, &t).is_err(), "pas un calque de texte");
    }
}
