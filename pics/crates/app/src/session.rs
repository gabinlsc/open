//! État de l'application : documents ouverts, vue de chacun, réglages et outil courant.
//!
//! Tout ici est indépendant de Tauri et du GPU afin d'être testé sur toute plateforme ; la coque
//! (commandes IPC, fenêtre native, rendu) ne fait que traduire des entrées en appels à `Session`.

use std::path::{Path, PathBuf};

use openpics_core::commands::{
    AddGuide, AddLayer, AddMask, ClearGuides, MoveLayer, RemoveGuide, RemoveLayer, RemoveMask, SetAdjustment,
    SetLayerGroup, SetLayerProps, SetLayerStyles, SetMaskEnabled,
};
use openpics_core::shortcuts::{Chord, Keymap};
use openpics_core::snap::snap_axis;
use openpics_core::view::next_zoom_step;
use openpics_core::{Axis, DocId, Document, Layer, LayerId, Size, TileGrid, Vec2, ViewSettings, Viewport};
use openpics_io::ExportOptions;
use openpics_ops::Op;
use openpics_ops::transform::Matrix3;

use crate::dto::{
    DocSummary, GuideDrag, GuideDto, HistoryDto, ImageOp, KeymapDto, LayerDto, LayerPropsDto, ResolvedTheme,
    SettingsDto, Snapshot, StyleDto, Tool, TransformDto, ViewState, blend_infos,
};
use crate::error::SessionError;

/// Marge laissée autour du document par « Ajuster à l'écran ».
const FIT_MARGIN_PX: f64 = 32.0;
/// Pixels de document qui doivent toujours rester visibles quand on déplace la vue.
const KEEP_VISIBLE_PX: f64 = 64.0;
/// Facteur de zoom par cran de molette : ×1,1 est assez fin pour viser, assez rapide pour traverser.
const WHEEL_ZOOM_BASE: f64 = 1.1;
/// Déplacement par cran de molette, en pixels physiques.
const WHEEL_PAN_PX: f64 = 80.0;
const MAX_NEW_DIMENSION: u32 = 30_000;

struct DocEntry {
    doc: Document,
    viewport: Viewport,
    fitted: bool,
    active_layer: Option<LayerId>,
}

mod paint;
mod tools;
pub use tools::Pointer;

pub struct Session {
    docs: Vec<DocEntry>,
    active: Option<DocId>,
    next_doc_id: u64,
    canvas: Size,
    pub settings: ViewSettings,
    pub theme: ResolvedTheme,
    pub keymap: Keymap,
    pub tool: Tool,
    tool_options: crate::dto::ToolOptionsDto,
    drag: Option<tools::Drag>,
    paint: crate::dto::PaintOptionsDto,
    stroke: Option<paint::ActiveStroke>,
    /// Source du tampon et du correcteur (Alt + clic) et décalage source − destination une fois fixé.
    clone_source: Option<(f64, f64)>,
    clone_offset: Option<(f32, f32)>,
    selection_preview: Option<TileGrid>,
    pub pan_modifier: bool,
    preview_guide: Option<(Axis, f64)>,
    untitled: u32,
}

/// Ce dont le rendu a besoin, copié sous le verrou puis utilisé sans lui.
pub struct RenderState {
    pub document: Option<Document>,
    pub viewport: Viewport,
    pub settings: ViewSettings,
    pub theme: ResolvedTheme,
    pub preview_guide: Option<(Axis, f64)>,
    /// Sélection à contourner : l'aperçu du geste en cours, sinon la sélection du document.
    pub selection: Option<TileGrid>,
}

impl Session {
    pub fn new(settings: ViewSettings, keymap: Keymap) -> Self {
        Self {
            docs: Vec::new(),
            active: None,
            next_doc_id: 1,
            canvas: Size::default(),
            settings,
            theme: ResolvedTheme::Light,
            keymap,
            tool: Tool::Hand,
            tool_options: crate::dto::ToolOptionsDto::default(),
            drag: None,
            paint: crate::dto::PaintOptionsDto::default(),
            stroke: None,
            clone_source: None,
            clone_offset: None,
            selection_preview: None,
            pan_modifier: false,
            preview_guide: None,
            untitled: 0,
        }
    }

    fn alloc_doc_id(&mut self) -> DocId {
        let id = DocId(self.next_doc_id);
        self.next_doc_id += 1;
        id
    }

    fn entry(&self) -> Result<&DocEntry, SessionError> {
        let id = self.active.ok_or(SessionError::NoActiveDocument)?;
        self.docs.iter().find(|e| e.doc.id == id).ok_or(SessionError::DocumentNotFound(id.0))
    }

    fn entry_mut(&mut self) -> Result<&mut DocEntry, SessionError> {
        let id = self.active.ok_or(SessionError::NoActiveDocument)?;
        self.docs.iter_mut().find(|e| e.doc.id == id).ok_or(SessionError::DocumentNotFound(id.0))
    }

    pub fn has_document(&self) -> bool {
        self.active.is_some()
    }

    // ---- documents --------------------------------------------------------------------------

    fn insert(&mut self, doc: Document) -> DocId {
        let id = doc.id;
        let active_layer = doc.layers.last().map(|l| l.id);
        self.docs.push(DocEntry { doc, viewport: Viewport::default(), fitted: false, active_layer });
        self.active = Some(id);
        self.preview_guide = None;
        self.ensure_fitted();
        id
    }

    pub fn new_document(&mut self, name: Option<String>, width: u32, height: u32) -> Result<DocId, SessionError> {
        if width == 0 || height == 0 || width > MAX_NEW_DIMENSION || height > MAX_NEW_DIMENSION {
            return Err(SessionError::InvalidArgument(format!(
                "dimensions invalides : {width}×{height} (de 1 à {MAX_NEW_DIMENSION} pixels par côté)"
            )));
        }
        let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| {
            self.untitled += 1;
            format!("Sans titre {}", self.untitled)
        });
        let id = self.alloc_doc_id();
        let mut doc = Document::new(id, name, Size::new(width, height))?;
        // Fond blanc opaque : un nouveau document « vide » qui s'affiche en damier désoriente.
        fill_white(&mut doc)?;
        doc.mark_saved();
        Ok(self.insert(doc))
    }

    /// Ouvre un fichier. Un `.ocv` déjà ouvert est simplement réactivé.
    pub fn open_path(&mut self, path: &Path) -> Result<DocId, SessionError> {
        if let Some(existing) = self.docs.iter().find(|e| e.doc.path.as_deref() == Some(path)) {
            let id = existing.doc.id;
            self.activate(id.0)?;
            return Ok(id);
        }
        let id = self.alloc_doc_id();
        let doc = openpics_io::open(path, id)?;
        Ok(self.insert(doc))
    }

    pub fn activate(&mut self, id: u64) -> Result<(), SessionError> {
        if !self.docs.iter().any(|e| e.doc.id.0 == id) {
            return Err(SessionError::DocumentNotFound(id));
        }
        self.active = Some(DocId(id));
        self.preview_guide = None;
        self.tool_cancel();
        self.ensure_fitted();
        Ok(())
    }

    /// Ferme un document ; le voisin de gauche (sinon de droite) prend le relais, comme dans un navigateur.
    pub fn close(&mut self, id: u64) -> Result<(), SessionError> {
        let idx = self.docs.iter().position(|e| e.doc.id.0 == id).ok_or(SessionError::DocumentNotFound(id))?;
        self.docs.remove(idx);
        if self.active == Some(DocId(id)) {
            self.active = self.docs.get(idx.saturating_sub(1)).map(|e| e.doc.id);
            self.preview_guide = None;
            self.ensure_fitted();
        }
        Ok(())
    }

    pub fn is_dirty(&self, id: u64) -> Result<bool, SessionError> {
        self.docs.iter().find(|e| e.doc.id.0 == id).map(|e| e.doc.is_dirty()).ok_or(SessionError::DocumentNotFound(id))
    }

    /// Enregistre en `.ocv`. Sans chemin explicite, utilise celui du document ; s'il n'en a pas,
    /// `NeedsPath` demande au frontend d'ouvrir la boîte « Enregistrer sous ».
    pub fn save_active(&mut self, path: Option<PathBuf>) -> Result<(), SessionError> {
        let entry = self.entry_mut()?;
        let target = path.or_else(|| entry.doc.path.clone()).ok_or(SessionError::NeedsPath)?;
        let target = with_extension(target, "ocv");
        openpics_io::save(&mut entry.doc, &target)?;
        Ok(())
    }

    pub fn export_active(&self, path: &Path, jpeg_quality: Option<u8>) -> Result<(), SessionError> {
        let options = ExportOptions { jpeg_quality: jpeg_quality.unwrap_or(90), ..ExportOptions::default() };
        openpics_io::export(&self.entry()?.doc, path, &options)?;
        Ok(())
    }

    // ---- vue --------------------------------------------------------------------------------

    pub fn canvas_size(&self) -> Size {
        self.canvas
    }

    /// Redimensionnement de la zone de canvas. Le centre de la vue reste en place, sinon le
    /// document dériverait vers un coin à chaque agrandissement de la fenêtre.
    pub fn set_canvas(&mut self, width: u32, height: u32) {
        let old = self.canvas;
        self.canvas = Size::new(width, height);
        if old.width > 0 && old.height > 0 && width > 0 && height > 0 {
            let (dx, dy) =
                ((f64::from(width) - f64::from(old.width)) / 2.0, (f64::from(height) - f64::from(old.height)) / 2.0);
            for e in &mut self.docs {
                e.viewport.pan_by(dx, dy);
            }
        }
        self.ensure_fitted();
    }

    /// Ajuste au premier affichage utilisable (canvas mesuré) ; les suivants gardent la vue de l'utilisateur.
    fn ensure_fitted(&mut self) {
        let canvas = self.canvas;
        if canvas.width == 0 || canvas.height == 0 {
            return;
        }
        if let Ok(e) = self.entry_mut() {
            if !e.fitted {
                let size = e.doc.size();
                e.viewport.fit(size, canvas, FIT_MARGIN_PX);
                e.fitted = true;
            }
        }
    }

    fn canvas_center(&self) -> Vec2 {
        Vec2::new(f64::from(self.canvas.width) / 2.0, f64::from(self.canvas.height) / 2.0)
    }

    fn clamp_active(&mut self) {
        let canvas = self.canvas;
        if let Ok(e) = self.entry_mut() {
            let size = e.doc.size();
            e.viewport.clamp_pan(size, canvas, KEEP_VISIBLE_PX);
        }
    }

    pub fn viewport(&self) -> Viewport {
        self.entry().map_or_else(|_| Viewport::default(), |e| e.viewport)
    }

    pub fn zoom_step(&mut self, zoom_in: bool, anchor: Option<Vec2>) -> Result<(), SessionError> {
        let anchor = anchor.unwrap_or_else(|| self.canvas_center());
        let e = self.entry_mut()?;
        let target = next_zoom_step(e.viewport.zoom, zoom_in);
        e.viewport.zoom_at(anchor, target);
        self.clamp_active();
        Ok(())
    }

    pub fn zoom_to(&mut self, zoom: f64, anchor: Option<Vec2>) -> Result<(), SessionError> {
        let anchor = anchor.unwrap_or_else(|| self.canvas_center());
        self.entry_mut()?.viewport.zoom_at(anchor, zoom);
        self.clamp_active();
        Ok(())
    }

    pub fn fit(&mut self) -> Result<(), SessionError> {
        let canvas = self.canvas;
        let e = self.entry_mut()?;
        let size = e.doc.size();
        e.viewport.fit(size, canvas, FIT_MARGIN_PX);
        e.fitted = true;
        Ok(())
    }

    pub fn actual_size(&mut self) -> Result<(), SessionError> {
        let canvas = self.canvas;
        let e = self.entry_mut()?;
        let size = e.doc.size();
        e.viewport.zoom = 1.0;
        e.viewport.center(size, canvas);
        e.fitted = true;
        Ok(())
    }

    pub fn pan_by(&mut self, dx: f64, dy: f64) -> Result<(), SessionError> {
        self.entry_mut()?.viewport.pan_by(dx, dy);
        self.clamp_active();
        Ok(())
    }

    /// Molette : Ctrl = zoom sous le curseur, Maj = défilement horizontal, sinon vertical.
    /// `notches` est positif quand la molette s'éloigne de l'utilisateur (convention Windows).
    pub fn wheel(&mut self, notches: f64, ctrl: bool, shift: bool, at: Vec2) -> Result<(), SessionError> {
        if !notches.is_finite() {
            return Ok(());
        }
        if ctrl {
            let z = self.entry()?.viewport.zoom;
            self.zoom_to(z * WHEEL_ZOOM_BASE.powf(notches), Some(at))
        } else if shift {
            self.pan_by(notches * WHEEL_PAN_PX, 0.0)
        } else {
            self.pan_by(0.0, notches * WHEEL_PAN_PX)
        }
    }

    pub fn cursor_to_doc(&self, at: Vec2) -> Option<(Vec2, bool)> {
        let e = self.entry().ok()?;
        let p = e.viewport.screen_to_doc(at);
        let s = e.doc.size();
        Some((p, p.x >= 0.0 && p.y >= 0.0 && p.x < f64::from(s.width) && p.y < f64::from(s.height)))
    }

    // ---- repères ----------------------------------------------------------------------------

    /// Aperçu d'un repère glissé depuis une règle ; la position renvoyée est celle après aimantation.
    pub fn guide_drag(&mut self, axis: Axis, position: f64) -> Result<GuideDrag, SessionError> {
        let e = self.entry()?;
        let snapped = snap_axis(position, axis, &e.doc.guides, e.doc.size(), &self.settings, e.viewport.zoom);
        self.preview_guide = Some((axis, snapped.value));
        Ok(GuideDrag { position: snapped.value, snapped: snapped.hit.is_some() })
    }

    pub fn guide_cancel(&mut self) {
        self.preview_guide = None;
    }

    /// Dépose le repère. Hors du document, il est abandonné (glisser un repère hors de l'image le supprime).
    pub fn guide_commit(&mut self, axis: Axis, position: f64) -> Result<bool, SessionError> {
        self.preview_guide = None;
        let settings = self.settings;
        let e = self.entry_mut()?;
        let size = e.doc.size();
        let extent = match axis {
            Axis::Horizontal => f64::from(size.height),
            Axis::Vertical => f64::from(size.width),
        };
        if !position.is_finite() || position < 0.0 || position > extent {
            return Ok(false);
        }
        let snapped = snap_axis(position, axis, &e.doc.guides, size, &settings, e.viewport.zoom).value;
        e.doc.execute(Box::new(AddGuide::new(axis, snapped)))?;
        Ok(true)
    }

    pub fn guide_remove(&mut self, id: u64) -> Result<(), SessionError> {
        self.entry_mut()?.doc.execute(Box::new(RemoveGuide::new(id)))?;
        Ok(())
    }

    pub fn guides_clear(&mut self) -> Result<(), SessionError> {
        let e = self.entry_mut()?;
        if e.doc.guides.is_empty() {
            return Ok(());
        }
        e.doc.execute(Box::new(ClearGuides::new()))?;
        Ok(())
    }

    // ---- historique et opérations ------------------------------------------------------------

    pub fn undo(&mut self) -> Result<bool, SessionError> {
        Ok(self.entry_mut()?.doc.undo()?)
    }

    pub fn redo(&mut self) -> Result<bool, SessionError> {
        Ok(self.entry_mut()?.doc.redo()?)
    }

    pub fn apply_op(&mut self, op: ImageOp) -> Result<bool, SessionError> {
        let op = match op {
            ImageOp::FlipHorizontal => Op::FlipHorizontal,
            ImageOp::FlipVertical => Op::FlipVertical,
            ImageOp::Invert => Op::Invert,
        };
        let entry = self.entry_mut()?;
        let layer =
            entry.active_layer.ok_or_else(|| SessionError::InvalidArgument("aucun calque sélectionné".into()))?;
        Ok(openpics_ops::apply_to_layer(&mut entry.doc, layer, op)?)
    }

    // ---- calques ----------------------------------------------------------------------------

    pub fn layer_select(&mut self, id: u64) -> Result<(), SessionError> {
        let e = self.entry_mut()?;
        e.doc.layer(LayerId(id))?;
        e.active_layer = Some(LayerId(id));
        Ok(())
    }

    /// Nouveau calque transparent juste au-dessus du calque actif (ou tout en haut).
    pub fn layer_add(&mut self) -> Result<u64, SessionError> {
        let e = self.entry_mut()?;
        let size = e.doc.size();
        let index = e
            .active_layer
            .and_then(|a| e.doc.layers.iter().position(|l| l.id == a))
            .map_or(e.doc.layers.len(), |i| i + 1);
        let id = LayerId(e.doc.alloc_id());
        let layer =
            Layer::new(id, format!("Calque {}", e.doc.layers.len() + 1), TileGrid::new(size.width, size.height)?);
        e.doc.execute(Box::new(AddLayer::new(index, layer)))?;
        e.active_layer = Some(id);
        Ok(id.0)
    }

    /// Nouveau groupe vide juste au-dessus du calque actif, dans le même groupe parent.
    pub fn group_add(&mut self) -> Result<u64, SessionError> {
        let e = self.entry_mut()?;
        let size = e.doc.size();
        let active = e.active_layer.and_then(|a| e.doc.layers.iter().position(|l| l.id == a));
        let index = active.map_or(e.doc.layers.len(), |i| i + 1);
        let parent = active.and_then(|i| e.doc.layers[i].group);
        let id = LayerId(e.doc.alloc_id());
        let mut group = Layer::new_group(
            id,
            format!("Groupe {}", e.doc.layers.iter().filter(|l| l.is_group()).count() + 1),
            TileGrid::new(size.width, size.height)?,
        );
        group.group = parent;
        e.doc.execute(Box::new(AddLayer::new(index, group)))?;
        e.active_layer = Some(id);
        Ok(id.0)
    }

    /// Place un calque dans un groupe (`None` = niveau supérieur).
    pub fn layer_set_group(&mut self, id: u64, group: Option<u64>) -> Result<(), SessionError> {
        let e = self.entry_mut()?;
        if e.doc.layer(LayerId(id))?.group == group.map(LayerId) {
            return Ok(());
        }
        e.doc.execute(Box::new(SetLayerGroup::new(LayerId(id), group.map(LayerId))))?;
        Ok(())
    }

    /// Monte ou descend d'un cran parmi les calques du même groupe.
    pub fn layer_move_step(&mut self, id: u64, up: bool) -> Result<(), SessionError> {
        let e = self.entry_mut()?;
        let layers = &e.doc.layers;
        let idx = layers
            .iter()
            .position(|l| l.id.0 == id)
            .ok_or(SessionError::Core(openpics_core::CoreError::LayerNotFound(id)))?;
        let parent = layers[idx].group;
        let sibling = if up {
            (idx + 1..layers.len()).find(|&i| layers[i].group == parent)
        } else {
            (0..idx).rev().find(|&i| layers[i].group == parent)
        };
        match sibling {
            Some(to) => {
                e.doc.execute(Box::new(MoveLayer::new(LayerId(id), to)))?;
                Ok(())
            }
            None => Ok(()), // déjà au bout de son groupe
        }
    }

    pub fn layer_remove(&mut self, id: u64) -> Result<(), SessionError> {
        let e = self.entry_mut()?;
        if e.doc.layers.len() <= 1 {
            return Err(SessionError::InvalidArgument("un document doit garder au moins un calque".into()));
        }
        let idx = e
            .doc
            .layers
            .iter()
            .position(|l| l.id.0 == id)
            .ok_or(SessionError::Core(openpics_core::CoreError::LayerNotFound(id)))?;
        // Supprimer un groupe emporte tout son contenu : il doit rester au moins un calque.
        let doomed = e.doc.layers.iter().filter(|l| e.doc.is_ancestor_or_self(LayerId(id), l.id)).count();
        if doomed >= e.doc.layers.len() {
            return Err(SessionError::InvalidArgument("un document doit garder au moins un calque".into()));
        }
        e.doc.execute(Box::new(RemoveLayer::new(LayerId(id))))?;
        if e.active_layer.is_some_and(|a| e.doc.layer(a).is_err()) {
            e.active_layer = e.doc.layers.get(idx.saturating_sub(1)).or_else(|| e.doc.layers.last()).map(|l| l.id);
        }
        Ok(())
    }

    /// `to` est l'index final dans la pile (0 = tout en bas).
    pub fn layer_move(&mut self, id: u64, to: usize) -> Result<(), SessionError> {
        self.entry_mut()?.doc.execute(Box::new(MoveLayer::new(LayerId(id), to)))?;
        Ok(())
    }

    pub fn layer_set_props(&mut self, id: u64, props: &LayerPropsDto) -> Result<(), SessionError> {
        let e = self.entry_mut()?;
        let current = e.doc.layer(LayerId(id))?.props();
        let next = props
            .apply_to(&current)
            .ok_or_else(|| SessionError::InvalidArgument(format!("mode de fusion inconnu : {}", props.blend)))?;
        if next.name.is_empty() {
            return Err(SessionError::InvalidArgument("le nom d'un calque ne peut pas être vide".into()));
        }
        if next == current {
            return Ok(());
        }
        e.doc.execute(Box::new(SetLayerProps::new(describe_change(&current, &next), LayerId(id), next)))?;
        Ok(())
    }

    pub fn layer_set_styles(&mut self, id: u64, styles: &[StyleDto]) -> Result<(), SessionError> {
        const MAX_STYLES: usize = 16;
        if styles.len() > MAX_STYLES {
            return Err(SessionError::InvalidArgument(format!("au plus {MAX_STYLES} styles par calque")));
        }
        let parsed = styles
            .iter()
            .map(|s| {
                s.to_style().ok_or_else(|| {
                    SessionError::InvalidArgument("couleur ou mode de fusion invalide dans un style".into())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let e = self.entry_mut()?;
        if e.doc.layer(LayerId(id))?.styles == parsed {
            return Ok(());
        }
        e.doc.execute(Box::new(SetLayerStyles::new("Styles de calque", LayerId(id), parsed)))?;
        Ok(())
    }

    /// Transformation du calque actif autour du centre du document. Renvoie `false` si rien n'a changé.
    pub fn layer_transform(&mut self, t: &TransformDto) -> Result<bool, SessionError> {
        let ok = |v: f64, lo: f64, hi: f64| v.is_finite() && (lo..=hi).contains(&v);
        if !ok(t.scale_x, 1.0, 1000.0) || !ok(t.scale_y, 1.0, 1000.0) {
            return Err(SessionError::InvalidArgument("l'échelle doit être comprise entre 1 % et 1000 %".into()));
        }
        if !ok(t.rotation_deg, -360.0, 360.0) || !ok(t.translate_x, -1e5, 1e5) || !ok(t.translate_y, -1e5, 1e5) {
            return Err(SessionError::InvalidArgument("rotation ou déplacement hors limites".into()));
        }
        let e = self.entry_mut()?;
        let layer = e.active_layer.ok_or_else(|| SessionError::InvalidArgument("aucun calque sélectionné".into()))?;
        let size = e.doc.size();
        let (cx, cy) = (f64::from(size.width) / 2.0, f64::from(size.height) / 2.0);
        let m = Matrix3::translate(t.translate_x, t.translate_y)
            .then_after(&Matrix3::rotate(t.rotation_deg).about(cx, cy))
            .then_after(&Matrix3::scale(t.scale_x / 100.0, t.scale_y / 100.0).about(cx, cy));
        Ok(openpics_ops::apply_transform(&mut e.doc, layer, &m)?)
    }

    /// Nouveau calque de réglage juste au-dessus du calque actif.
    pub fn adjustment_add(&mut self, kind: &str) -> Result<u64, SessionError> {
        let kind = crate::dto::parse_adj_kind(kind)
            .ok_or_else(|| SessionError::InvalidArgument(format!("réglage inconnu : {kind}")))?;
        let e = self.entry_mut()?;
        let size = e.doc.size();
        let index = e
            .active_layer
            .and_then(|a| e.doc.layers.iter().position(|l| l.id == a))
            .map_or(e.doc.layers.len(), |i| i + 1);
        let parent = index.checked_sub(1).and_then(|i| e.doc.layers.get(i)).and_then(|l| l.group);
        let id = LayerId(e.doc.alloc_id());
        let mut layer = Layer::new_adjustment(
            id,
            kind.label(),
            TileGrid::new(size.width, size.height)?,
            openpics_core::adjust::Adjustment::new(kind),
        );
        layer.group = parent;
        e.doc.execute(Box::new(AddLayer::new(index, layer)))?;
        e.active_layer = Some(id);
        Ok(id.0)
    }

    fn adjustment_of(&self, id: u64) -> Result<openpics_core::adjust::Adjustment, SessionError> {
        let layer = self.entry()?.doc.layer(LayerId(id))?;
        layer.adjustment().cloned().ok_or_else(|| openpics_core::CoreError::NotAdjustment(layer.name.clone()).into())
    }

    fn adjustment_commit(
        &mut self,
        id: u64,
        name: String,
        adj: openpics_core::adjust::Adjustment,
    ) -> Result<(), SessionError> {
        if self.adjustment_of(id)? == adj.sanitized() {
            return Ok(());
        }
        self.entry_mut()?.doc.execute(Box::new(SetAdjustment::new(name, LayerId(id), adj)))?;
        Ok(())
    }

    pub fn adjustment_set_param(&mut self, id: u64, param: &str, value: f32) -> Result<(), SessionError> {
        let mut adj = self.adjustment_of(id)?;
        let label = adj.kind.specs().iter().find(|s| s.id == param).map(|s| s.label);
        if !adj.set(param, value) {
            return Err(SessionError::InvalidArgument(format!("paramètre inconnu : {param}")));
        }
        self.adjustment_commit(id, format!("{} : {}", adj.kind.label(), label.unwrap_or(param)), adj)
    }

    /// Remplace les points de la courbe commune (la courbe est ramenée à des points triés dans 0..1).
    pub fn adjustment_set_curve(&mut self, id: u64, points: Vec<(f32, f32)>) -> Result<(), SessionError> {
        let mut adj = self.adjustment_of(id)?;
        match &mut adj.extra {
            openpics_core::adjust::Extra::Curves { master, .. } => *master = points,
            _ => return Err(SessionError::InvalidArgument("ce réglage n'a pas de courbe".into())),
        }
        self.adjustment_commit(id, "Courbes".into(), adj)
    }

    pub fn adjustment_set_stops(&mut self, id: u64, stops: &[(f32, String)]) -> Result<(), SessionError> {
        let mut adj = self.adjustment_of(id)?;
        let parsed = stops
            .iter()
            .map(|(p, c)| {
                crate::dto::parse_hex(c)
                    .map(|c| (*p, c))
                    .ok_or_else(|| SessionError::InvalidArgument(format!("couleur invalide : {c}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        match &mut adj.extra {
            openpics_core::adjust::Extra::Stops(s) => *s = parsed,
            _ => return Err(SessionError::InvalidArgument("ce réglage n'a pas d'arrêts de dégradé".into())),
        }
        self.adjustment_commit(id, "Courbes de dégradé".into(), adj)
    }

    /// Charge un fichier `.cube` (limité à 64 Mio) dans un réglage « LUT 3D ».
    pub fn adjustment_load_lut(&mut self, id: u64, path: &Path) -> Result<(), SessionError> {
        const MAX_BYTES: u64 = 64 * 1024 * 1024;
        let mut adj = self.adjustment_of(id)?;
        if !matches!(adj.extra, openpics_core::adjust::Extra::Lut(_)) {
            return Err(SessionError::InvalidArgument("ce réglage n'est pas une LUT 3D".into()));
        }
        let meta =
            std::fs::metadata(path).map_err(|e| SessionError::InvalidArgument(format!("{} : {e}", path.display())))?;
        if meta.len() > MAX_BYTES {
            return Err(SessionError::InvalidArgument("fichier .cube trop volumineux (64 Mio maximum)".into()));
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| SessionError::InvalidArgument(format!("{} : {e}", path.display())))?;
        let lut = openpics_core::adjust::Lut3D::parse_cube(&text)
            .map_err(|e| SessionError::InvalidArgument(format!("fichier .cube invalide — {e}")))?;
        adj.extra = openpics_core::adjust::Extra::Lut(std::sync::Arc::new(lut));
        self.adjustment_commit(id, "LUT 3D".into(), adj)
    }

    /// Correspondance de couleur : fait tendre les statistiques du calque `source` vers celles de `reference`.
    pub fn adjustment_match(&mut self, id: u64, source: u64, reference: u64) -> Result<(), SessionError> {
        let mut adj = self.adjustment_of(id)?;
        if adj.kind != openpics_core::adjust::AdjKind::ColorMatch {
            return Err(SessionError::InvalidArgument("ce réglage n'est pas une correspondance de couleur".into()));
        }
        let e = self.entry()?;
        let stats = |layer: u64| -> Result<([f32; 3], [f32; 3]), SessionError> {
            let l = e.doc.layer(LayerId(layer))?;
            if !l.is_raster() {
                return Err(openpics_core::CoreError::NotRaster(l.name.clone()).into());
            }
            openpics_ops::stats::color_stats(&l.pixels)
                .ok_or_else(|| SessionError::InvalidArgument(format!("le calque « {} » est vide", l.name)))
        };
        let ((src_mean, src_std), (ref_mean, ref_std)) = (stats(source)?, stats(reference)?);
        adj.extra = openpics_core::adjust::Extra::Match { src_mean, src_std, ref_mean, ref_std };
        self.adjustment_commit(id, "Correspondance de couleur".into(), adj)
    }

    pub fn mask_add(&mut self, id: u64, reveal: bool) -> Result<(), SessionError> {
        self.entry_mut()?.doc.execute(Box::new(AddMask::new(LayerId(id), reveal)))?;
        Ok(())
    }

    pub fn mask_remove(&mut self, id: u64) -> Result<(), SessionError> {
        self.entry_mut()?.doc.execute(Box::new(RemoveMask::new(LayerId(id))))?;
        Ok(())
    }

    pub fn mask_set_enabled(&mut self, id: u64, enabled: bool) -> Result<(), SessionError> {
        self.entry_mut()?.doc.execute(Box::new(SetMaskEnabled::new(LayerId(id), enabled)))?;
        Ok(())
    }

    // ---- raccourcis -------------------------------------------------------------------------

    pub fn keymap_dto(&self) -> KeymapDto {
        KeymapDto {
            bindings: self
                .keymap
                .bindings()
                .iter()
                .map(|(a, cs)| (a.clone(), cs.iter().map(ToString::to_string).collect()))
                .collect(),
        }
    }

    pub fn keymap_set(&mut self, action: &str, chords: &[String]) -> Result<(), SessionError> {
        let parsed = chords.iter().map(|c| Chord::parse(c)).collect::<Result<Vec<_>, _>>()?;
        self.keymap.set(action, parsed)?;
        Ok(())
    }

    // ---- instantanés ------------------------------------------------------------------------

    pub fn render_state(&self) -> RenderState {
        RenderState {
            document: self.entry().ok().map(|e| e.doc.render_snapshot()),
            viewport: self.viewport(),
            settings: self.settings,
            theme: self.theme,
            preview_guide: self.preview_guide,
            selection: self
                .selection_preview
                .clone()
                .or_else(|| self.entry().ok().and_then(|e| e.doc.selection.clone())),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let docs = self
            .docs
            .iter()
            .map(|e| {
                let s = e.doc.size();
                DocSummary {
                    id: e.doc.id.0,
                    name: e.doc.name.clone(),
                    width: s.width,
                    height: s.height,
                    dirty: e.doc.is_dirty(),
                    path: e.doc.path.as_ref().map(|p| p.display().to_string()),
                }
            })
            .collect();
        let (guides, layers, active_layer_id, history, can_undo, can_redo) = match self.entry() {
            Ok(e) => {
                let (names, position) = e.doc.history().entries();
                (
                    e.doc.guides.iter().map(GuideDto::from).collect(),
                    e.doc
                        .tree_top_first()
                        .into_iter()
                        .filter_map(|(id, depth)| e.doc.layer(id).ok().map(|l| LayerDto { depth, ..LayerDto::from(l) }))
                        .collect(),
                    e.active_layer.map(|l| l.0),
                    HistoryDto { names, position },
                    e.doc.history().can_undo(),
                    e.doc.history().can_redo(),
                )
            }
            Err(_) => (Vec::new(), Vec::new(), None, HistoryDto { names: Vec::new(), position: 0 }, false, false),
        };
        let vp = self.viewport();
        Snapshot {
            docs,
            active_id: self.active.map(|d| d.0),
            view: ViewState {
                zoom: vp.zoom,
                origin_x: vp.origin.x,
                origin_y: vp.origin.y,
                canvas_width: self.canvas.width,
                canvas_height: self.canvas.height,
            },
            guides,
            layers,
            active_layer_id,
            blend_modes: blend_infos(),
            has_selection: self.entry().is_ok_and(|e| e.doc.selection.is_some()),
            adjustment_kinds: crate::dto::adjustment_kinds(),
            tool_options: self.tool_options,
            paint_options: self.paint.clone(),
            history,
            can_undo,
            can_redo,
            settings: SettingsDto::from(self.settings),
            tool: self.tool,
        }
    }
}

/// Libellé d'historique lisible : ce qui a réellement changé.
fn describe_change(a: &openpics_core::LayerProps, b: &openpics_core::LayerProps) -> String {
    if a.blend != b.blend {
        format!("Mode de fusion : {}", b.blend.label())
    } else if a.opacity != b.opacity {
        format!("Opacité : {} %", (b.opacity * 100.0).round())
    } else if a.fill != b.fill {
        format!("Remplissage : {} %", (b.fill * 100.0).round())
    } else if a.visible != b.visible {
        (if b.visible { "Afficher le calque" } else { "Masquer le calque" }).to_owned()
    } else if a.clip_to_below != b.clip_to_below {
        (if b.clip_to_below { "Créer un masque d'écrêtage" } else { "Libérer le masque d'écrêtage" }).to_owned()
    } else if a.locks != b.locks {
        "Verrouillage du calque".to_owned()
    } else {
        "Renommer le calque".to_owned()
    }
}

fn fill_white(doc: &mut Document) -> Result<(), SessionError> {
    let size = doc.size();
    // Tuiles uniformes : un document de 30 000² ne coûte aucun tampon de pixels.
    let layer = doc.layers.first_mut().ok_or(SessionError::NoActiveDocument)?;
    let grid = &mut layer.pixels;
    for ty in 0..grid.tiles_y() {
        for tx in 0..grid.tiles_x() {
            let full =
                (tx + 1) * openpics_core::TILE_SIZE <= size.width && (ty + 1) * openpics_core::TILE_SIZE <= size.height;
            let key = openpics_core::TileKey { tx, ty };
            if full {
                grid.set_tile(key, Some(std::sync::Arc::new(openpics_core::Tile::Uniform([255; 4]))));
            } else {
                // Tuile de bord : seuls les pixels dans l'image sont blancs, le reste reste transparent.
                let x0 = tx * openpics_core::TILE_SIZE;
                let y0 = ty * openpics_core::TILE_SIZE;
                let mut tile = openpics_core::Tile::Uniform([0; 4]);
                for ly in 0..openpics_core::TILE_SIZE.min(size.height - y0) {
                    for lx in 0..openpics_core::TILE_SIZE.min(size.width - x0) {
                        tile.set_pixel(lx, ly, [255; 4]);
                    }
                }
                grid.set_tile(key, Some(std::sync::Arc::new(tile)));
            }
        }
    }
    Ok(())
}

fn with_extension(mut path: PathBuf, ext: &str) -> PathBuf {
    if path.extension().and_then(|e| e.to_str()).is_none_or(|e| !e.eq_ignore_ascii_case(ext)) {
        path.set_extension(ext);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpics_core::Vec2;

    fn session() -> Session {
        let mut s = Session::new(ViewSettings::default(), Keymap::default());
        s.set_canvas(800, 600);
        s
    }

    #[test]
    fn new_document_is_white_fitted_and_clean() {
        let mut s = session();
        let id = s.new_document(None, 400, 300).unwrap();
        let snap = s.snapshot();
        assert_eq!(snap.active_id, Some(id.0));
        assert_eq!(snap.docs[0].name, "Sans titre 1");
        assert!(!snap.docs[0].dirty);
        assert_eq!(snap.view.zoom, 1.0);
        let doc = s.render_state().document.unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(399, 299), [255; 4]);
        assert_eq!(doc.layers[0].pixels.get_pixel(400, 300), [0; 4]);
    }

    #[test]
    fn huge_new_document_uses_no_pixel_buffers() {
        let mut s = session();
        s.new_document(Some("Grand".into()), 20_000, 20_000).unwrap();
        let doc = s.render_state().document.unwrap();
        // Seules les tuiles de bord (20 000 n'est pas multiple de 256) portent des pixels.
        let textured =
            doc.layers[0].pixels.iter().filter(|(_, t)| matches!(t.as_ref(), openpics_core::Tile::Pixels(_))).count();
        assert!(textured <= 78 + 78 + 1, "{textured} tuiles texturées");
        assert!(doc.layers[0].pixels.tile_count() > 6000);
    }

    #[test]
    fn rejects_invalid_dimensions() {
        let mut s = session();
        assert!(matches!(s.new_document(None, 0, 10), Err(SessionError::InvalidArgument(_))));
        assert!(matches!(s.new_document(None, 10, 30_001), Err(SessionError::InvalidArgument(_))));
        assert!(!s.has_document());
    }

    #[test]
    fn closing_active_tab_activates_left_neighbour() {
        let mut s = session();
        let a = s.new_document(None, 10, 10).unwrap();
        let b = s.new_document(None, 10, 10).unwrap();
        let c = s.new_document(None, 10, 10).unwrap();
        s.close(b.0).unwrap();
        assert_eq!(s.snapshot().active_id, Some(c.0)); // fermer un onglet inactif ne change pas l'actif
        s.close(c.0).unwrap();
        assert_eq!(s.snapshot().active_id, Some(a.0));
        s.close(a.0).unwrap();
        assert!(!s.has_document());
        assert!(matches!(s.close(a.0), Err(SessionError::DocumentNotFound(_))));
    }

    #[test]
    fn zoom_anchor_stays_under_cursor_and_wheel_modes() {
        let mut s = session();
        s.new_document(None, 2000, 2000).unwrap();
        let at = Vec2::new(300.0, 200.0);
        let before = s.viewport().screen_to_doc(at);
        s.wheel(3.0, true, false, at).unwrap();
        let vp = s.viewport();
        let start = 0.268; // ajusté : (800 − 64) / 2000 = 0,368 en largeur, (600 − 64) / 2000 = 0,268 en hauteur
        assert!((vp.zoom - start * WHEEL_ZOOM_BASE.powi(3)).abs() < 1e-3, "zoom = {}", vp.zoom);
        let after = vp.screen_to_doc(at);
        assert!((before.x - after.x).abs() < 1e-6 && (before.y - after.y).abs() < 1e-6);
        let o = s.viewport().origin;
        s.wheel(1.0, false, false, at).unwrap();
        assert_eq!(s.viewport().origin.y - o.y, WHEEL_PAN_PX);
        s.wheel(-1.0, false, true, at).unwrap();
        assert_eq!(s.viewport().origin.x - o.x, -WHEEL_PAN_PX);
        s.wheel(f64::NAN, true, false, at).unwrap();
    }

    #[test]
    fn pan_cannot_lose_the_document() {
        let mut s = session();
        s.new_document(None, 500, 500).unwrap();
        s.pan_by(-1e9, 1e9).unwrap();
        let vp = s.viewport();
        assert!(vp.origin.x.is_finite() && vp.origin.x > -500.0 * vp.zoom);
        assert!(vp.origin.y < 600.0);
    }

    #[test]
    fn window_resize_keeps_view_centre() {
        let mut s = session();
        s.new_document(None, 100, 100).unwrap();
        let centre_before = s.viewport().screen_to_doc(Vec2::new(400.0, 300.0));
        s.set_canvas(1000, 800);
        let centre_after = s.viewport().screen_to_doc(Vec2::new(500.0, 400.0));
        assert!((centre_before.x - centre_after.x).abs() < 1e-9);
        assert!((centre_before.y - centre_after.y).abs() < 1e-9);
    }

    #[test]
    fn document_opened_before_canvas_is_measured_gets_fitted_later() {
        let mut s = Session::new(ViewSettings::default(), Keymap::default());
        s.new_document(None, 4000, 3000).unwrap();
        assert_eq!(s.viewport().zoom, 1.0);
        s.set_canvas(800, 600);
        assert!(s.viewport().zoom < 0.25);
    }

    #[test]
    fn guides_snap_commit_and_undo() {
        let mut s = session();
        s.new_document(None, 500, 500).unwrap();
        assert!(s.guide_commit(Axis::Vertical, 250.0).unwrap());
        let d = s.guide_drag(Axis::Vertical, 253.0).unwrap();
        assert!(d.snapped && d.position == 250.0);
        assert!(!s.guide_commit(Axis::Vertical, -5.0).unwrap());
        assert!(!s.guide_commit(Axis::Horizontal, 501.0).unwrap());
        assert_eq!(s.snapshot().guides.len(), 1);
        assert!(s.render_state().preview_guide.is_none());
        s.undo().unwrap();
        assert!(s.snapshot().guides.is_empty());
        s.guides_clear().unwrap();
        assert!(!s.snapshot().can_undo || s.snapshot().history.names.len() == 1);
    }

    #[test]
    fn ops_are_undoable_and_mark_dirty() {
        let mut s = session();
        s.new_document(None, 4, 4).unwrap();
        assert!(matches!(s.save_active(None), Err(SessionError::NeedsPath)));
        assert!(s.apply_op(ImageOp::Invert).unwrap());
        assert!(s.snapshot().docs[0].dirty);
        let doc = s.render_state().document.unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), [0, 0, 0, 255]);
        s.undo().unwrap();
        assert_eq!(s.render_state().document.unwrap().layers[0].pixels.get_pixel(0, 0), [255; 4]);
        assert!(!s.snapshot().docs[0].dirty);
    }

    #[test]
    fn save_open_export_roundtrip_and_reopen_activates() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = session();
        s.new_document(Some("a".into()), 40, 30).unwrap();
        s.guide_commit(Axis::Vertical, 10.0).unwrap();
        s.save_active(Some(dir.path().join("a"))).unwrap(); // l'extension .ocv est ajoutée
        let saved = dir.path().join("a.ocv");
        assert!(saved.exists());
        assert!(!s.snapshot().docs[0].dirty);
        s.export_active(&dir.path().join("a.png"), None).unwrap();
        s.export_active(&dir.path().join("a.jpg"), Some(80)).unwrap();
        let again = s.open_path(&saved).unwrap();
        assert_eq!(s.snapshot().docs.len(), 1, "rouvrir le même .ocv ne doit pas dupliquer l'onglet");
        assert_eq!(s.snapshot().active_id, Some(again.0));
        let png = s.open_path(&dir.path().join("a.png")).unwrap();
        assert_eq!(s.snapshot().docs.len(), 2);
        assert_eq!(s.snapshot().active_id, Some(png.0));
        assert!(s.open_path(&dir.path().join("nope.png")).is_err());
        assert_eq!(s.snapshot().docs.len(), 2, "un échec d'ouverture ne doit pas laisser d'onglet");
    }

    #[test]
    fn keymap_errors_map_to_conflict() {
        let mut s = session();
        let err = s.keymap_set("file.open", &["Ctrl+Z".to_owned()]).unwrap_err();
        let api: crate::error::ApiError = err.into();
        assert_eq!(api.code, "shortcut_conflict");
        s.keymap_set("file.open", &["Ctrl+Alt+O".to_owned()]).unwrap();
        assert!(s.keymap_set("file.open", &["???".to_owned()]).is_err());
        assert_eq!(s.keymap_dto().bindings["file.open"], vec!["Ctrl+Alt+O".to_owned()]);
    }

    #[test]
    fn no_document_errors_are_explicit() {
        let mut s = session();
        assert!(matches!(s.fit(), Err(SessionError::NoActiveDocument)));
        assert!(matches!(s.undo(), Err(SessionError::NoActiveDocument)));
        assert!(matches!(s.save_active(None), Err(SessionError::NoActiveDocument)));
        assert!(s.cursor_to_doc(Vec2::new(1.0, 1.0)).is_none());
    }

    fn props(s: &Session) -> LayerPropsDto {
        let l = &s.snapshot().layers[0];
        LayerPropsDto {
            name: l.name.clone(),
            visible: l.visible,
            opacity: l.opacity,
            fill: l.fill,
            blend: l.blend.clone(),
            locks: l.locks,
            clip_to_below: l.clip_to_below,
            isolated: l.isolated,
        }
    }

    #[test]
    fn layers_are_listed_top_first_with_an_active_layer() {
        let mut s = session();
        s.new_document(None, 20, 20).unwrap();
        let snap = s.snapshot();
        assert_eq!(snap.layers.len(), 1);
        assert_eq!(snap.active_layer_id, Some(snap.layers[0].id));
        let new = s.layer_add().unwrap();
        let snap = s.snapshot();
        assert_eq!(snap.layers[0].id, new, "le nouveau calque est tout en haut");
        assert_eq!(snap.active_layer_id, Some(new));
        assert_eq!(snap.blend_modes.len(), 27);
    }

    #[test]
    fn layer_edits_go_through_history_with_readable_names() {
        let mut s = session();
        s.new_document(None, 20, 20).unwrap();
        s.layer_add().unwrap();
        let id = s.snapshot().layers[0].id;
        let mut p = props(&s);
        p.blend = "multiply".into();
        s.layer_set_props(id, &p).unwrap();
        p.opacity = 0.5;
        s.layer_set_props(id, &p).unwrap();
        let names = s.snapshot().history.names;
        assert_eq!(&names[names.len() - 2..], ["Mode de fusion : Produit", "Opacité : 50 %"]);
        s.undo().unwrap();
        assert_eq!(s.snapshot().layers[0].opacity, 1.0);
        s.undo().unwrap();
        assert_eq!(s.snapshot().layers[0].blend, "normal");
        // Aucun changement = aucun état d'historique.
        let before = s.snapshot().history.names.len();
        s.layer_set_props(id, &props(&s)).unwrap();
        assert_eq!(s.snapshot().history.names.len(), before);
    }

    #[test]
    fn invalid_layer_edits_are_refused() {
        let mut s = session();
        s.new_document(None, 20, 20).unwrap();
        let id = s.snapshot().layers[0].id;
        let mut p = props(&s);
        p.blend = "inconnu".into();
        assert!(matches!(s.layer_set_props(id, &p), Err(SessionError::InvalidArgument(_))));
        p.blend = "normal".into();
        p.name = "   ".into();
        assert!(matches!(s.layer_set_props(id, &p), Err(SessionError::InvalidArgument(_))));
        assert!(s.layer_set_props(9999, &props(&s)).is_err());
        assert!(matches!(s.layer_remove(id), Err(SessionError::InvalidArgument(_))), "on garde au moins un calque");
    }

    #[test]
    fn removing_the_active_layer_selects_a_neighbour_and_undo_restores() {
        let mut s = session();
        s.new_document(None, 20, 20).unwrap();
        let bottom = s.snapshot().layers[0].id;
        let top = s.layer_add().unwrap();
        s.layer_remove(top).unwrap();
        assert_eq!(s.snapshot().active_layer_id, Some(bottom));
        s.undo().unwrap();
        assert_eq!(s.snapshot().layers[0].id, top);
        s.layer_move(top, 0).unwrap();
        assert_eq!(s.snapshot().layers[1].id, top, "déplacé tout en bas");
    }

    #[test]
    fn masks_and_locks_work_through_the_session() {
        let mut s = session();
        s.new_document(None, 20, 20).unwrap();
        let id = s.snapshot().layers[0].id;
        s.mask_add(id, true).unwrap();
        assert!(s.snapshot().layers[0].has_mask && s.snapshot().layers[0].mask_enabled);
        assert!(s.mask_add(id, true).is_err());
        s.mask_set_enabled(id, false).unwrap();
        assert!(!s.snapshot().layers[0].mask_enabled);
        s.mask_remove(id).unwrap();
        assert!(!s.snapshot().layers[0].has_mask);
        let mut p = props(&s);
        p.locks.pixels = true;
        s.layer_set_props(id, &p).unwrap();
        let err = s.apply_op(ImageOp::Invert).unwrap_err().to_string();
        assert!(err.contains("verrouillé"), "{err}");
    }

    #[test]
    fn operations_apply_to_the_selected_layer_only() {
        let mut s = session();
        s.new_document(None, 4, 4).unwrap();
        let bottom = s.snapshot().layers[0].id;
        s.layer_add().unwrap();
        s.layer_select(bottom).unwrap();
        assert!(s.apply_op(ImageOp::Invert).unwrap());
        let doc = s.render_state().document.unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), [0, 0, 0, 255]);
        assert_eq!(doc.layers[1].pixels.tile_count(), 0);
    }

    #[test]
    fn styles_are_validated_and_undoable_through_the_session() {
        let mut s = session();
        s.new_document(None, 20, 20).unwrap();
        let id = s.snapshot().layers[0].id;
        let glow = StyleDto::OuterGlow { color: "#ffcc00".into(), opacity: 0.8, size: 6.0 };
        s.layer_set_styles(id, std::slice::from_ref(&glow)).unwrap();
        assert_eq!(s.snapshot().layers[0].styles, vec![glow.clone()]);
        let bad = StyleDto::OuterGlow { color: "jaune".into(), opacity: 1.0, size: 1.0 };
        assert!(matches!(s.layer_set_styles(id, &[bad]), Err(SessionError::InvalidArgument(_))));
        assert!(matches!(s.layer_set_styles(id, &vec![glow.clone(); 17]), Err(SessionError::InvalidArgument(_))));
        let before = s.snapshot().history.names.len();
        s.layer_set_styles(id, &[glow]).unwrap();
        assert_eq!(s.snapshot().history.names.len(), before, "pas d'état d'historique si rien ne change");
        s.undo().unwrap();
        assert!(s.snapshot().layers[0].styles.is_empty());
    }

    #[test]
    fn transform_acts_on_the_active_layer_around_the_centre_and_validates_input() {
        let mut s = session();
        s.new_document(None, 64, 64).unwrap();
        let id = s.snapshot().layers[0].id;
        s.layer_add().unwrap();
        s.layer_select(id).unwrap();
        let identity =
            TransformDto { scale_x: 100.0, scale_y: 100.0, rotation_deg: 0.0, translate_x: 0.0, translate_y: 0.0 };
        s.layer_transform(&identity).unwrap();
        // Un demi-tour autour du centre laisse un calque uni identique, puis on décale de 10 px.
        let half_turn = TransformDto { rotation_deg: 180.0, ..identity };
        s.layer_transform(&half_turn).unwrap();
        let moved = TransformDto { translate_x: 10.0, ..identity };
        s.layer_transform(&moved).unwrap();
        let doc = s.render_state().document.unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(5, 30), [0; 4], "10 px de la bande gauche sont maintenant vides");
        assert_eq!(doc.layers[0].pixels.get_pixel(30, 30), [255; 4]);
        for bad in [
            TransformDto { scale_x: 0.0, ..identity },
            TransformDto { scale_y: f64::NAN, ..identity },
            TransformDto { rotation_deg: 1e9, ..identity },
            TransformDto { translate_x: f64::INFINITY, ..identity },
        ] {
            assert!(matches!(s.layer_transform(&bad), Err(SessionError::InvalidArgument(_))));
        }
        // Verrou de position : la transformation est refusée avec un message clair.
        let mut p = props(&s);
        p.name = s.snapshot().layers.iter().find(|l| l.id == id).unwrap().name.clone();
        p.locks.position = true;
        s.layer_set_props(id, &p).unwrap();
        let err = s.layer_transform(&moved).unwrap_err().to_string();
        assert!(err.contains("verrouillé"), "{err}");
    }

    #[test]
    fn groups_nest_move_and_delete_through_the_session() {
        let mut s = session();
        s.new_document(None, 20, 20).unwrap();
        let bottom = s.snapshot().layers[0].id;
        let g = s.group_add().unwrap();
        let snap = s.snapshot();
        assert!(snap.layers[0].is_group && snap.layers[0].id == g && snap.layers[0].isolated);
        s.layer_set_group(bottom, Some(g)).unwrap();
        let snap = s.snapshot();
        assert_eq!((snap.layers[1].id, snap.layers[1].depth, snap.layers[1].group), (bottom, 1, Some(g)));
        s.layer_select(g).unwrap();
        assert!(s.apply_op(ImageOp::Invert).unwrap_err().to_string().contains("pixels"));
        assert!(s.layer_set_group(g, Some(g)).is_err());
        assert!(s.layer_set_group(g, Some(bottom)).is_err());
        assert!(
            matches!(s.layer_remove(g), Err(SessionError::InvalidArgument(_))),
            "il faut garder au moins un calque"
        );
        let extra = s.layer_add().unwrap();
        s.layer_remove(g).unwrap();
        let snap = s.snapshot();
        assert_eq!(snap.layers.iter().map(|l| l.id).collect::<Vec<_>>(), vec![extra]);
        assert_eq!(snap.active_layer_id, Some(extra));
        s.undo().unwrap();
        assert_eq!(s.snapshot().layers.len(), 3);
    }

    #[test]
    fn move_step_swaps_with_the_sibling_inside_the_same_group() {
        let mut s = session();
        s.new_document(None, 20, 20).unwrap();
        let a = s.snapshot().layers[0].id;
        let b = s.layer_add().unwrap();
        let ids = |s: &Session| s.snapshot().layers.iter().map(|l| l.id).collect::<Vec<_>>();
        assert_eq!(ids(&s), vec![b, a]);
        s.layer_move_step(b, false).unwrap();
        assert_eq!(ids(&s), vec![a, b]);
        let n = s.snapshot().history.names.len();
        s.layer_move_step(b, false).unwrap(); // déjà tout en bas : sans effet ni état d'historique
        assert_eq!(s.snapshot().history.names.len(), n);
    }

    #[test]
    fn adjustment_layers_change_the_image_non_destructively_through_history() {
        let mut s = session();
        s.new_document(None, 8, 8).unwrap();
        let bg = s.snapshot().layers[0].id;
        let id = s.adjustment_add("invert").unwrap();
        let snap = s.snapshot();
        assert_eq!((snap.layers[0].id, snap.layers[0].adjustment.as_ref().unwrap().kind.as_str()), (id, "invert"));
        assert_eq!(snap.adjustment_kinds.len(), 15);
        let doc = s.render_state().document.unwrap();
        assert_eq!(&doc.flatten_rgba8()[0..4], &[0, 0, 0, 255], "blanc inversé");
        assert_eq!(doc.layers[0].pixels.get_pixel(0, 0), [255; 4], "les pixels du fond sont intacts");
        s.undo().unwrap();
        assert_eq!(s.snapshot().layers.len(), 1);
        assert!(s.adjustment_add("inconnu").is_err());
        // Les calques de réglage ne se peignent ni ne se transforment.
        s.redo().unwrap();
        s.layer_select(id).unwrap();
        assert!(s.apply_op(ImageOp::Invert).unwrap_err().to_string().contains("pixels"));
        assert!(s.adjustment_set_param(bg, "ev", 1.0).is_err(), "un calque raster n'est pas un réglage");
    }

    #[test]
    fn parameters_curves_and_stops_are_validated_and_undoable() {
        let mut s = session();
        s.new_document(None, 8, 8).unwrap();
        let ev = s.adjustment_add("exposure").unwrap();
        s.adjustment_set_param(ev, "ev", -1.0).unwrap();
        let p = |s: &Session| {
            s.snapshot().layers[0].adjustment.as_ref().unwrap().params.iter().find(|p| p.id == "ev").unwrap().value
        };
        assert_eq!(p(&s), -1.0);
        assert_eq!(s.snapshot().history.names.last().unwrap(), "Exposition : Exposition (IL)");
        s.adjustment_set_param(ev, "ev", 99.0).unwrap();
        assert_eq!(p(&s), 5.0, "borné à la plage");
        let n = s.snapshot().history.names.len();
        s.adjustment_set_param(ev, "ev", 5.0).unwrap();
        assert_eq!(s.snapshot().history.names.len(), n, "sans changement : pas d'état");
        assert!(s.adjustment_set_param(ev, "n'existe_pas", 1.0).is_err());
        s.undo().unwrap();
        assert_eq!(p(&s), -1.0);

        let curves = s.adjustment_add("curves").unwrap();
        s.adjustment_set_curve(curves, vec![(0.0, 0.0), (0.5, 0.8), (1.0, 1.0)]).unwrap();
        assert_eq!(s.snapshot().layers[0].adjustment.as_ref().unwrap().curve.as_ref().unwrap().len(), 3);
        assert!(s.adjustment_set_curve(ev, vec![]).is_err());
        let map = s.adjustment_add("gradient_map").unwrap();
        s.adjustment_set_stops(map, &[(0.0, "#000080".into()), (1.0, "#ffcc00".into())]).unwrap();
        assert_eq!(s.snapshot().layers[0].adjustment.as_ref().unwrap().stops.as_ref().unwrap()[1].color, "#ffcc00");
        assert!(s.adjustment_set_stops(map, &[(0.0, "bleu".into())]).is_err());
    }

    #[test]
    fn cube_files_load_validate_and_apply() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = session();
        s.new_document(None, 4, 4).unwrap();
        let lut = s.adjustment_add("lut3_d").unwrap();
        let mut swap = openpics_core::adjust::Lut3D::identity(2);
        for e in &mut swap.data {
            e.swap(0, 2);
        }
        let good = dir.path().join("swap.cube");
        std::fs::write(&good, swap.to_cube()).unwrap();
        s.adjustment_load_lut(lut, &good).unwrap();
        assert_eq!(s.snapshot().layers[0].adjustment.as_ref().unwrap().lut_size, Some(2));
        let bad = dir.path().join("bad.cube");
        std::fs::write(&bad, "LUT_3D_SIZE 2\n1 2\n").unwrap();
        assert!(s.adjustment_load_lut(lut, &bad).unwrap_err().to_string().contains("invalide"));
        assert!(s.adjustment_load_lut(lut, &dir.path().join("absent.cube")).is_err());
        let ev = s.adjustment_add("exposure").unwrap();
        assert!(s.adjustment_load_lut(ev, &good).is_err());
    }

    #[test]
    fn colour_match_uses_two_raster_layers() {
        let mut s = session();
        s.new_document(None, 8, 8).unwrap();
        let dark = s.snapshot().layers[0].id;
        let bright = s.layer_add().unwrap();
        {
            let e = s.entry_mut().unwrap();
            for y in 0..8 {
                for x in 0..8 {
                    e.doc.layer_mut(LayerId(bright)).unwrap().pixels.put_pixel(x, y, [220, 200, 180, 255]);
                    let v = if (x + y) % 2 == 0 { 40 } else { 80 };
                    e.doc.layer_mut(LayerId(dark)).unwrap().pixels.put_pixel(x, y, [v, v, v, 255]);
                }
            }
        }
        let m = s.adjustment_add("color_match").unwrap();
        assert!(s.adjustment_match(m, dark, m).is_err(), "la référence doit être un calque raster");
        s.adjustment_match(m, dark, bright).unwrap();
        assert_eq!(s.snapshot().history.names.last().unwrap(), "Correspondance de couleur");
        let flat = s.render_state().document.unwrap().flatten_rgba8();
        assert!(flat[0] > 100, "le gris sombre est ramené vers la teinte de la référence : {}", flat[0]);
        assert!(s.adjustment_match(dark, dark, bright).is_err());
    }
}
