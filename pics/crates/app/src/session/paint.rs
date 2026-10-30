//! Outils de peinture et de retouche : un trait modifie le calque en direct (sans historique) puis, au
//! relâchement, devient un seul état d'historique (différence de tuiles entre avant et après).

use openpics_core::commands::ReplaceTiles;
use openpics_core::{LayerId, TileGrid};
use openpics_ops::brush::{PaintKind, PaintStroke};
use openpics_ops::fill::{Bucket, GradientKind, bucket_fill, gradient};
use openpics_ops::retouch::{RetouchKind, RetouchStroke, TonalRange, patch, spot_heal};
use openpics_ops::selection::through_selection;

use super::tools::Pointer;
use super::{Session, SessionError};
use crate::dto::{GradientKindDto, PaintOptionsDto, TonalRangeDto, Tool};

pub(super) enum StrokeKind {
    Paint(PaintStroke),
    Retouch(RetouchStroke),
    /// Dégradé ou pièce : le geste se joue entre deux points, appliqué au relâchement.
    TwoPoint {
        start: (f64, f64),
        base: TileGrid,
    },
}

pub(super) struct ActiveStroke {
    layer: LayerId,
    tool: Tool,
    kind: StrokeKind,
}

fn tool_name(tool: Tool) -> &'static str {
    match tool {
        Tool::Brush => "Pinceau",
        Tool::Pencil => "Crayon",
        Tool::Eraser => "Gomme",
        Tool::Bucket => "Pot de peinture",
        Tool::Gradient => "Dégradé",
        Tool::CloneStamp => "Tampon de duplication",
        Tool::Heal => "Correcteur",
        Tool::SpotHeal => "Correcteur localisé",
        Tool::Patch => "Pièce",
        Tool::Blur => "Goutte d'eau (flou)",
        Tool::Sharpen => "Netteté",
        Tool::Smudge => "Doigt",
        Tool::Dodge => "Densité −",
        Tool::Burn => "Densité +",
        Tool::Sponge => "Éponge",
        _ => "Peinture",
    }
}

impl Session {
    pub fn paint_options(&self) -> &PaintOptionsDto {
        &self.paint
    }

    pub fn set_paint_options(&mut self, options: &PaintOptionsDto) {
        self.paint = options.sanitized(&self.paint);
    }

    /// Calque actif modifiable : raster, déverrouillé.
    fn paintable_layer(&self) -> Result<LayerId, SessionError> {
        let e = self.entry()?;
        let id = e.active_layer.ok_or_else(|| SessionError::InvalidArgument("aucun calque sélectionné".into()))?;
        let layer = e.doc.layer(id)?;
        if layer.is_group() {
            return Err(openpics_core::CoreError::IsGroup(layer.name.clone()).into());
        }
        if layer.locks.blocks_painting() {
            return Err(openpics_core::CoreError::LayerLocked(layer.name.clone()).into());
        }
        Ok(id)
    }

    fn set_layer_pixels(&mut self, id: LayerId, pixels: TileGrid) -> Result<(), SessionError> {
        let e = self.entry_mut()?;
        e.doc.layer_mut(id)?.pixels = pixels;
        e.doc.touch();
        Ok(())
    }

    pub(super) fn paint_down(&mut self, p: &Pointer, at: (f64, f64)) -> Result<(), SessionError> {
        let tool = self.tool;
        // Alt + clic définit la source du tampon et du correcteur (et ne peint pas).
        if matches!(tool, Tool::CloneStamp | Tool::Heal) && p.alt {
            self.clone_source = Some(at);
            self.clone_offset = None;
            return Ok(());
        }
        let layer = self.paintable_layer()?;
        let base = self.entry()?.doc.layer(layer)?.pixels.clone();
        let selection = self.entry()?.doc.selection.clone();
        let opts = self.paint.clone();
        let brush = opts.brush(opts.fg());
        let (w, h) = (base.width(), base.height());
        let inside = at.0 >= 0.0 && at.1 >= 0.0 && at.0 < f64::from(w) && at.1 < f64::from(h);
        let kind = match tool {
            Tool::Brush | Tool::Pencil | Tool::Eraser => {
                let kind = match tool {
                    Tool::Pencil => PaintKind::Pencil,
                    Tool::Eraser => PaintKind::Eraser,
                    _ => PaintKind::Brush,
                };
                StrokeKind::Paint(PaintStroke::new(base, kind, brush, selection))
            }
            Tool::Blur
            | Tool::Sharpen
            | Tool::Smudge
            | Tool::Dodge
            | Tool::Burn
            | Tool::Sponge
            | Tool::CloneStamp
            | Tool::Heal => {
                let rk = match tool {
                    Tool::Blur => RetouchKind::Blur,
                    Tool::Sharpen => RetouchKind::Sharpen,
                    Tool::Smudge => RetouchKind::Smudge,
                    Tool::Dodge => RetouchKind::Dodge,
                    Tool::Burn => RetouchKind::Burn,
                    Tool::Sponge if opts.saturate => RetouchKind::Saturate,
                    Tool::Sponge => RetouchKind::Desaturate,
                    Tool::CloneStamp => RetouchKind::Clone,
                    _ => RetouchKind::Heal,
                };
                // Mode « aligné » : le décalage source − destination est fixé au premier trait.
                if matches!(tool, Tool::CloneStamp | Tool::Heal) {
                    let Some(src) = self.clone_source else {
                        return Err(SessionError::InvalidArgument("Alt + clic pour définir d'abord la source".into()));
                    };
                    if self.clone_offset.is_none() {
                        self.clone_offset = Some(((src.0 - at.0) as f32, (src.1 - at.1) as f32));
                    }
                }
                let range = match opts.tonal_range {
                    TonalRangeDto::Shadows => TonalRange::Shadows,
                    TonalRangeDto::Midtones => TonalRange::Midtones,
                    TonalRangeDto::Highlights => TonalRange::Highlights,
                };
                StrokeKind::Retouch(RetouchStroke::new(base, rk, brush, range, self.clone_offset, selection))
            }
            Tool::Bucket => {
                if !inside {
                    return Ok(());
                }
                let reference = if self.tool_options.sample_all_layers {
                    let size = self.entry()?.doc.size();
                    TileGrid::from_rgba8(size.width, size.height, &self.entry()?.doc.flatten_rgba8())?
                } else {
                    base.clone()
                };
                let tolerance = self.tool_options.tolerance;
                let filled = bucket_fill(
                    &base,
                    &reference,
                    Bucket {
                        x: at.0 as u32,
                        y: at.1 as u32,
                        tolerance,
                        contiguous: self.tool_options.contiguous,
                        color: opts.fg(),
                        opacity: opts.opacity,
                    },
                )?;
                return self.commit_pixels(layer, tool, &base, filled);
            }
            Tool::SpotHeal => {
                if !inside {
                    return Ok(());
                }
                let healed = spot_heal(&base, at.0 as f32, at.1 as f32, opts.size, selection)?;
                return self.commit_pixels(layer, tool, &base, healed);
            }
            Tool::Gradient | Tool::Patch => StrokeKind::TwoPoint { start: at, base },
            _ => return Ok(()),
        };
        self.stroke = Some(ActiveStroke { layer, tool, kind });
        if inside || !matches!(tool, Tool::Gradient | Tool::Patch) {
            self.paint_move(p, at)?;
        }
        Ok(())
    }

    pub(super) fn paint_move(&mut self, p: &Pointer, at: (f64, f64)) -> Result<(), SessionError> {
        let Some(mut stroke) = self.stroke.take() else { return Ok(()) };
        let result = match &mut stroke.kind {
            StrokeKind::Paint(s) => {
                s.add_point(at.0 as f32, at.1 as f32, p.pressure)?;
                Some(s.result().clone())
            }
            StrokeKind::Retouch(s) => {
                s.add_point(at.0 as f32, at.1 as f32, p.pressure)?;
                Some(s.result().clone())
            }
            // Aperçu du dégradé en direct, seulement tant que le calque reste petit.
            StrokeKind::TwoPoint { start, base }
                if stroke.tool == Tool::Gradient && u64::from(base.width()) * u64::from(base.height()) <= 4_000_000 =>
            {
                Some(self.gradient_result(base, *start, at)?)
            }
            StrokeKind::TwoPoint { .. } => None,
        };
        let layer = stroke.layer;
        self.stroke = Some(stroke);
        if let Some(pixels) = result {
            self.set_layer_pixels(layer, pixels)?;
        }
        Ok(())
    }

    fn gradient_result(&self, base: &TileGrid, start: (f64, f64), end: (f64, f64)) -> Result<TileGrid, SessionError> {
        let o = &self.paint;
        let (fg, bg) = (o.fg(), o.bg());
        let stops = [
            (0.0, [fg[0], fg[1], fg[2], 255]),
            (1.0, if o.gradient_to_transparent { [fg[0], fg[1], fg[2], 0] } else { [bg[0], bg[1], bg[2], 255] }),
        ];
        let kind = match o.gradient {
            GradientKindDto::Linear => GradientKind::Linear,
            GradientKindDto::Radial => GradientKind::Radial,
            GradientKindDto::Angular => GradientKind::Angular,
            GradientKindDto::Reflected => GradientKind::Reflected,
            GradientKindDto::Diamond => GradientKind::Diamond,
        };
        let painted = gradient(base, kind, start, end, &stops, o.opacity)?;
        Ok(match &self.entry()?.doc.selection {
            Some(sel) => through_selection(base, &painted, sel)?,
            None => painted,
        })
    }

    pub(super) fn paint_up(&mut self, _p: &Pointer, at: (f64, f64)) -> Result<(), SessionError> {
        let Some(stroke) = self.stroke.take() else { return Ok(()) };
        let (layer, tool) = (stroke.layer, stroke.tool);
        match stroke.kind {
            StrokeKind::Paint(s) => self.commit_pixels(layer, tool, s.base(), s.result().clone()),
            StrokeKind::Retouch(s) => self.commit_pixels(layer, tool, s.base(), s.result().clone()),
            StrokeKind::TwoPoint { start, base } if tool == Tool::Gradient => {
                let result = self.gradient_result(&base, start, at)?;
                self.commit_pixels(layer, tool, &base, result)
            }
            StrokeKind::TwoPoint { start, base } => {
                // Pièce : la sélection glissée de `start` vers `at` est remplacée par ce qui s'y trouve.
                let Some(selection) = self.entry()?.doc.selection.clone() else {
                    return Err(SessionError::InvalidArgument("la pièce a besoin d'une sélection".into()));
                };
                let offset = ((at.0 - start.0).round() as i32, (at.1 - start.1).round() as i32);
                let result = patch(&base, &selection, offset, true)?;
                self.commit_pixels(layer, tool, &base, result)
            }
        }
    }

    /// Abandonne un trait en cours en restituant le calque d'avant.
    pub(super) fn paint_cancel(&mut self) {
        if let Some(stroke) = self.stroke.take() {
            let base = match stroke.kind {
                StrokeKind::Paint(s) => s.base().clone(),
                StrokeKind::Retouch(s) => s.base().clone(),
                StrokeKind::TwoPoint { base, .. } => base,
            };
            let _ = self.set_layer_pixels(stroke.layer, base);
        }
    }

    /// Enregistre « avant → après » d'un calque en un seul état d'historique.
    fn commit_pixels(
        &mut self,
        layer: LayerId,
        tool: Tool,
        before: &TileGrid,
        after: TileGrid,
    ) -> Result<(), SessionError> {
        let Some(cmd) = ReplaceTiles::between(tool_name(tool), layer, before, &after) else {
            return self.set_layer_pixels(layer, before.clone());
        };
        // L'`apply` écrit l'état « après » ; le `revert` restitue « avant ».
        self.entry_mut()?.doc.execute(Box::new(cmd))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpics_core::ViewSettings;
    use openpics_core::shortcuts::Keymap;

    /// Document 100×100 blanc ; coordonnées de document = pixels du canvas − (350, 250).
    fn session() -> Session {
        let mut s = Session::new(ViewSettings::default(), Keymap::default());
        s.set_canvas(800, 600);
        s.new_document(None, 100, 100).unwrap();
        s
    }

    fn at(x: f64, y: f64) -> Pointer {
        Pointer { x: 350.0 + x, y: 250.0 + y, pressure: 1.0, ..Pointer::default() }
    }

    fn drag(s: &mut Session, pts: &[(f64, f64)]) {
        s.pointer_down(&at(pts[0].0, pts[0].1)).unwrap();
        for p in &pts[1..] {
            s.pointer_move(&at(p.0, p.1)).unwrap();
        }
        let last = pts[pts.len() - 1];
        s.pointer_up(&at(last.0, last.1)).unwrap();
    }

    fn px(s: &Session, x: u32, y: u32) -> [u8; 4] {
        s.render_state().document.unwrap().layers[0].pixels.get_pixel(x, y)
    }

    fn options(s: &mut Session, f: impl FnOnce(&mut PaintOptionsDto)) {
        let mut o = s.paint_options().clone();
        f(&mut o);
        s.set_paint_options(&o);
    }

    #[test]
    fn a_brush_stroke_is_one_undoable_history_entry() {
        let mut s = session();
        options(&mut s, |o| {
            o.foreground = "#ff0000".into();
            o.size = 10.0;
            o.hardness = 1.0;
            o.size_from_pressure = false;
        });
        s.set_tool(Tool::Brush);
        let before = s.snapshot().history.names.len();
        s.pointer_down(&at(20.0, 50.0)).unwrap();
        assert_eq!(px(&s, 20, 50), [255, 0, 0, 255], "le trait apparaît en direct");
        assert_eq!(s.snapshot().history.names.len(), before, "pas encore d'état d'historique");
        s.pointer_move(&at(60.0, 50.0)).unwrap();
        s.pointer_up(&at(60.0, 50.0)).unwrap();
        assert_eq!(px(&s, 40, 50), [255, 0, 0, 255]);
        let names = s.snapshot().history.names;
        assert_eq!(names.len(), before + 1);
        assert_eq!(names.last().unwrap(), "Pinceau");
        s.undo().unwrap();
        assert_eq!(px(&s, 40, 50), [255, 255, 255, 255]);
        s.redo().unwrap();
        assert_eq!(px(&s, 40, 50), [255, 0, 0, 255]);
    }

    #[test]
    fn eraser_pencil_and_selection_confinement() {
        let mut s = session();
        options(&mut s, |o| {
            o.size = 20.0;
            o.hardness = 1.0;
            o.size_from_pressure = false;
        });
        s.set_tool(Tool::Eraser);
        drag(&mut s, &[(30.0, 30.0), (60.0, 30.0)]);
        assert_eq!(px(&s, 45, 30)[3], 0, "la gomme efface jusqu'à la transparence");
        s.undo().unwrap();
        s.set_tool(Tool::RectSelect);
        drag(&mut s, &[(0.0, 0.0), (50.0, 100.0)]);
        s.set_tool(Tool::Eraser);
        drag(&mut s, &[(30.0, 50.0), (80.0, 50.0)]);
        assert_eq!((px(&s, 40, 50)[3], px(&s, 70, 50)[3]), (0, 255), "le trait ne sort pas de la sélection");
    }

    #[test]
    fn locked_and_group_layers_refuse_painting_with_a_clear_message() {
        let mut s = session();
        s.set_tool(Tool::Brush);
        let id = s.snapshot().layers[0].id;
        let mut p = crate::dto::LayerPropsDto {
            name: "Arrière-plan".into(),
            visible: true,
            opacity: 1.0,
            fill: 1.0,
            blend: "normal".into(),
            locks: crate::dto::LocksDto { pixels: true, position: false, all: false },
            clip_to_below: false,
            isolated: true,
        };
        s.layer_set_props(id, &p).unwrap();
        assert!(s.pointer_down(&at(10.0, 10.0)).unwrap_err().to_string().contains("verrouillé"));
        p.locks.pixels = false;
        s.layer_set_props(id, &p).unwrap();
        let g = s.group_add().unwrap();
        s.layer_select(g).unwrap();
        assert!(s.pointer_down(&at(10.0, 10.0)).unwrap_err().to_string().contains("groupe"));
    }

    #[test]
    fn cancelling_mid_stroke_restores_the_layer() {
        let mut s = session();
        options(&mut s, |o| o.foreground = "#00ff00".into());
        s.set_tool(Tool::Brush);
        s.pointer_down(&at(20.0, 20.0)).unwrap();
        s.pointer_move(&at(40.0, 20.0)).unwrap();
        assert_eq!(px(&s, 30, 20)[1], 255);
        let n = s.snapshot().history.names.len();
        s.tool_cancel();
        assert_eq!(px(&s, 30, 20), [255; 4]);
        assert_eq!(s.snapshot().history.names.len(), n);
        s.pointer_up(&at(40.0, 20.0)).unwrap(); // un relâchement tardif ne fait rien
        assert_eq!(px(&s, 30, 20), [255; 4]);
    }

    #[test]
    fn bucket_and_gradient_tools() {
        let mut s = session();
        options(&mut s, |o| {
            o.foreground = "#ff0000".into();
            o.background = "#0000ff".into();
            o.opacity = 1.0;
        });
        s.set_tool(Tool::Bucket);
        s.pointer_down(&at(10.0, 10.0)).unwrap();
        s.pointer_up(&at(10.0, 10.0)).unwrap();
        assert_eq!(px(&s, 90, 90), [255, 0, 0, 255]);
        assert_eq!(s.snapshot().history.names.last().unwrap(), "Pot de peinture");
        s.set_tool(Tool::Gradient);
        drag(&mut s, &[(0.0, 50.0), (50.0, 50.0), (99.0, 50.0)]);
        let (left, right) = (px(&s, 2, 50), px(&s, 97, 50));
        assert!(left[0] > 240 && right[2] > 240, "rouge à gauche, bleu à droite : {left:?} {right:?}");
        s.undo().unwrap();
        assert_eq!(px(&s, 97, 50), [255, 0, 0, 255]);
        options(&mut s, |o| o.gradient_to_transparent = true);
        drag(&mut s, &[(0.0, 50.0), (99.0, 50.0)]);
        assert!(px(&s, 97, 50)[0] > 240, "vers la transparence : le fond rouge réapparaît à droite");
    }

    #[test]
    fn clone_needs_a_source_and_then_copies_with_a_fixed_offset() {
        let mut s = session();
        options(&mut s, |o| {
            o.foreground = "#ff0000".into();
            o.size = 10.0;
            o.hardness = 1.0;
            o.size_from_pressure = false;
        });
        s.set_tool(Tool::Brush);
        drag(&mut s, &[(20.0, 20.0), (22.0, 20.0)]);
        s.set_tool(Tool::CloneStamp);
        assert!(s.pointer_down(&at(70.0, 70.0)).unwrap_err().to_string().contains("source"));
        s.pointer_down(&Pointer { alt: true, ..at(20.0, 20.0) }).unwrap(); // source
        assert_eq!(s.snapshot().history.names.last().unwrap(), "Pinceau", "Alt + clic ne peint pas");
        drag(&mut s, &[(70.0, 70.0), (71.0, 70.0)]);
        assert_eq!(px(&s, 70, 70), [255, 0, 0, 255], "copié depuis (20, 20)");
        assert_eq!(s.snapshot().history.names.last().unwrap(), "Tampon de duplication");
    }

    #[test]
    fn blur_spot_heal_and_patch() {
        let mut s = session();
        options(&mut s, |o| {
            o.foreground = "#000000".into();
            o.size = 12.0;
            o.hardness = 1.0;
            o.size_from_pressure = false;
        });
        s.set_tool(Tool::Brush);
        drag(&mut s, &[(50.0, 50.0), (51.0, 50.0)]);
        assert_eq!(px(&s, 50, 50), [0, 0, 0, 255]);
        s.set_tool(Tool::SpotHeal);
        s.pointer_down(&at(50.0, 50.0)).unwrap();
        s.pointer_up(&at(50.0, 50.0)).unwrap();
        assert!(px(&s, 50, 50)[0] > 200, "la tache est corrigée : {:?}", px(&s, 50, 50));
        s.set_tool(Tool::Patch);
        s.pointer_down(&at(10.0, 10.0)).unwrap();
        assert!(s.pointer_up(&at(30.0, 30.0)).unwrap_err().to_string().contains("sélection"));
        s.set_tool(Tool::Blur);
        let n = s.snapshot().history.names.len();
        drag(&mut s, &[(10.0, 10.0), (20.0, 10.0)]);
        assert_eq!(
            s.snapshot().history.names.len(),
            n,
            "flouter un aplat blanc ne change rien : pas d'état d'historique"
        );
    }

    #[test]
    fn hostile_options_are_bounded() {
        let mut s = session();
        options(&mut s, |o| {
            o.foreground = "pas une couleur".into();
            o.size = f32::NAN;
            o.opacity = 7.0;
            o.flow = 0.0;
        });
        let o = s.paint_options();
        assert_eq!((o.foreground.as_str(), o.size, o.opacity, o.flow), ("#000000", 20.0, 1.0, 0.01));
    }
}
