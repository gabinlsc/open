//! Plans de travail : zones nommées d'un document, exportées chacune comme une image (formats multiples dans un même
//! document). Les calques sont partagés entre les plans : ce ne sont pas des pages indépendantes.

use std::path::{Path, PathBuf};

use openpics_core::commands::SetArtboards;
use openpics_core::{Artboard, Document, EditOverlay, Size, TileGrid};
use openpics_io::ExportOptions;

use super::image::{scaled_flat, scaled_path};
use super::{Session, SessionError};
use crate::dto::Tool;

/// En dessous de ce glissé (px d'écran), c'est un clic : on sélectionne le plan sous le pointeur.
const MIN_DRAG_PX: f64 = 3.0;

/// Nom de fichier sûr tiré du nom d'un plan.
fn file_part(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(60)
        .collect();
    if s.is_empty() { "plan".into() } else { s }
}

/// Rectangle entier (x, y, l, h) entre deux points du document, ramené dans le document.
fn rect_between(a: (f64, f64), b: (f64, f64), size: Size) -> (u32, u32, u32, u32) {
    let clamp = |v: f64, max: u32| v.round().clamp(0.0, f64::from(max)) as u32;
    let (x0, x1) = (clamp(a.0.min(b.0), size.width), clamp(a.0.max(b.0), size.width));
    let (y0, y1) = (clamp(a.1.min(b.1), size.height), clamp(a.1.max(b.1), size.height));
    (x0, y0, x1 - x0, y1 - y0)
}

impl Session {
    /// Plan actif s'il existe encore (il a pu disparaître par une annulation).
    pub(super) fn active_artboard_checked(&self) -> Option<u64> {
        let id = self.active_artboard?;
        self.entry().ok()?.doc.artboards.iter().any(|a| a.id == id).then_some(id)
    }

    fn set_artboards(&mut self, label: &str, list: Vec<Artboard>) -> Result<(), SessionError> {
        self.entry_mut()?.doc.execute(Box::new(SetArtboards::new(label, list)))?;
        Ok(())
    }

    pub fn artboard_add(&mut self, x: u32, y: u32, w: u32, h: u32, name: Option<String>) -> Result<u64, SessionError> {
        let doc = &mut self.entry_mut()?.doc;
        if doc.artboards.len() >= openpics_core::document::MAX_ARTBOARDS {
            return Err(SessionError::InvalidArgument("trop de plans de travail".into()));
        }
        let id = doc.alloc_id();
        let name = name.unwrap_or_else(|| format!("Plan de travail {}", doc.artboards.len() + 1));
        let mut list = doc.artboards.clone();
        list.push(Artboard { id, name, x, y, w, h });
        self.set_artboards("Ajouter un plan de travail", list)?;
        if !self.entry()?.doc.artboards.iter().any(|a| a.id == id) {
            // Rien ne reste dans le document : on n'ajoute pas d'étape vide à l'historique.
            self.entry_mut()?.doc.undo()?;
            return Err(SessionError::InvalidArgument("le plan de travail est en dehors du document".into()));
        }
        self.active_artboard = Some(id);
        Ok(id)
    }

    pub fn artboard_set(&mut self, id: u64, name: &str, x: u32, y: u32, w: u32, h: u32) -> Result<(), SessionError> {
        let mut list = self.entry()?.doc.artboards.clone();
        let a = list
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| SessionError::InvalidArgument("plan de travail introuvable".into()))?;
        let next = Artboard { id, name: name.to_owned(), x, y, w, h };
        if *a == next {
            return Ok(());
        }
        if next.clone().clamped(self.entry()?.doc.size()).is_none() {
            return Err(SessionError::InvalidArgument("le plan de travail sortirait du document".into()));
        }
        *a = next;
        self.set_artboards("Modifier le plan de travail", list)
    }

    pub fn artboard_remove(&mut self, id: u64) -> Result<(), SessionError> {
        let mut list = self.entry()?.doc.artboards.clone();
        let before = list.len();
        list.retain(|a| a.id != id);
        if list.len() == before {
            return Ok(());
        }
        self.set_artboards("Supprimer le plan de travail", list)?;
        if self.active_artboard == Some(id) {
            self.active_artboard = None;
        }
        Ok(())
    }

    pub fn artboard_select(&mut self, id: Option<u64>) {
        self.active_artboard = id;
    }

    // ---- outil à la souris ---------------------------------------------------------------------------------------

    pub(super) fn artboard_down(&mut self, at: (f64, f64)) {
        self.artboard_drag = Some(at);
    }

    pub(super) fn artboard_move(&mut self, at: (f64, f64)) -> Result<(), SessionError> {
        let Some(start) = self.artboard_drag else { return Ok(()) };
        let size = self.entry()?.doc.size();
        self.selection_preview = Some(openpics_ops::selection::rect(size.width, size.height, start, at)?);
        Ok(())
    }

    pub(super) fn artboard_up(&mut self, at: (f64, f64)) -> Result<(), SessionError> {
        let Some(start) = self.artboard_drag.take() else { return Ok(()) };
        self.selection_preview = None;
        if (start.0 - at.0).hypot(start.1 - at.1) * self.viewport().zoom < MIN_DRAG_PX {
            // Clic : le plan le plus récent qui contient le point devient actif.
            let hit = self.entry()?.doc.artboards.iter().rev().find(|a| {
                at.0 >= f64::from(a.x)
                    && at.1 >= f64::from(a.y)
                    && at.0 < f64::from(a.x + a.w)
                    && at.1 < f64::from(a.y + a.h)
            });
            self.active_artboard = hit.map(|a| a.id);
            return Ok(());
        }
        let (x, y, w, h) = rect_between(start, at, self.entry()?.doc.size());
        if w == 0 || h == 0 {
            return Ok(());
        }
        self.artboard_add(x, y, w, h, None)?;
        Ok(())
    }

    /// Contour du plan actif, mis en évidence pendant l'outil Plan de travail.
    pub(super) fn artboard_overlay(&self) -> Option<EditOverlay> {
        if self.tool != Tool::Artboard {
            return None;
        }
        let id = self.active_artboard_checked()?;
        let a = self.entry().ok()?.doc.artboards.iter().find(|a| a.id == id)?.clone();
        let (x0, y0, x1, y1) = (f64::from(a.x), f64::from(a.y), f64::from(a.x + a.w), f64::from(a.y + a.h));
        let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        Some(EditOverlay {
            segments: (0..4).map(|i| [corners[i], corners[(i + 1) % 4]]).collect(),
            anchors: corners.to_vec(),
            handles: Vec::new(),
            selected: None,
        })
    }

    // ---- export --------------------------------------------------------------------------------------------------

    /// Exporte chaque plan de travail : `base-<nom du plan>.png`, ou `base-<nom>@2x.png`… avec plusieurs échelles.
    pub fn artboards_export(
        &self,
        base: &Path,
        scales: &[f64],
        jpeg_quality: Option<u8>,
    ) -> Result<Vec<PathBuf>, SessionError> {
        if scales.is_empty() || scales.len() > 16 || scales.iter().any(|s| !(0.01..=16.0).contains(s)) {
            return Err(SessionError::InvalidArgument("échelles : 1 à 16 valeurs entre 0,01 et 16".into()));
        }
        let doc = &self.entry()?.doc;
        if doc.artboards.is_empty() {
            return Err(SessionError::InvalidArgument("ce document n'a aucun plan de travail".into()));
        }
        let size = doc.size();
        let flat = doc.flatten_rgba8();
        let stem = base.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
        let ext = base.extension().and_then(|e| e.to_str()).unwrap_or("png");
        let options = ExportOptions { jpeg_quality: jpeg_quality.unwrap_or(90), ..ExportOptions::default() };
        let single = scales.len() == 1 && (scales[0] - 1.0).abs() < 1e-9;
        let mut used = std::collections::HashSet::new();
        let mut written = Vec::new();
        for a in &doc.artboards {
            // Découpe de l'image aplatie : une ligne de pixels à la fois.
            let mut rgba = Vec::with_capacity(a.w as usize * a.h as usize * 4);
            for row in a.y..a.y + a.h {
                let start = (row as usize * size.width as usize + a.x as usize) * 4;
                rgba.extend_from_slice(&flat[start..start + a.w as usize * 4]);
            }
            let mut crop = Document::new(doc.id, a.name.clone(), Size::new(a.w, a.h))?;
            crop.layers[0].pixels = TileGrid::from_rgba8(a.w, a.h, &rgba)?;
            let mut part = file_part(&a.name);
            if !used.insert(part.clone()) {
                part = format!("{part}-{}", a.id);
                used.insert(part.clone());
            }
            let path = base.with_file_name(format!("{stem}-{part}.{ext}"));
            for &scale in scales {
                let target = if single { path.clone() } else { scaled_path(&path, scale) };
                openpics_io::export(&scaled_flat(&crop, scale)?, &target, &options)?;
                written.push(target);
            }
        }
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpics_core::Vec2;

    /// Position d'écran d'un point du document.
    fn screen(s: &Session, x: f64, y: f64) -> super::super::tools::Pointer {
        let v = s.viewport().doc_to_screen(Vec2::new(x, y));
        super::super::tools::Pointer { x: v.x, y: v.y, pressure: 1.0, ..Default::default() }
    }
    use openpics_core::ViewSettings;
    use openpics_core::shortcuts::Keymap;

    fn session() -> Session {
        let mut s = Session::new(ViewSettings::default(), Keymap::default());
        s.set_canvas(800, 600);
        s.new_document(None, 200, 100).unwrap();
        s
    }

    fn drag(s: &mut Session, a: (f64, f64), b: (f64, f64)) {
        s.pointer_down(&screen(s, a.0, a.1)).unwrap();
        s.pointer_move(&screen(s, b.0, b.1)).unwrap();
        s.pointer_up(&screen(s, b.0, b.1)).unwrap();
    }

    #[test]
    fn the_tool_draws_selects_and_the_panel_edits_or_removes_artboards() {
        let mut s = session();
        s.set_tool(Tool::Artboard);
        let n = s.snapshot().history.position;
        drag(&mut s, (10.0, 10.0), (60.4, 50.6));
        let snap = s.snapshot();
        assert_eq!(snap.artboards.len(), 1);
        let a = &snap.artboards[0];
        assert_eq!((a.name.as_str(), a.x, a.y, a.w, a.h), ("Plan de travail 1", 10, 10, 50, 41));
        assert_eq!(snap.active_artboard_id, Some(a.id));
        assert_eq!(snap.history.position, n + 1);
        assert!(s.render_state().overlay.is_some(), "le plan actif est mis en évidence");
        // Glissé qui déborde du document : rogné.
        drag(&mut s, (150.0, 60.0), (400.0, 300.0));
        let b = s.snapshot().artboards[1].clone();
        assert_eq!((b.x, b.y, b.w, b.h), (150, 60, 50, 40));
        // Clic dans le premier : il redevient actif ; clic dans le vide : plus rien d'actif.
        let first = a.id;
        drag(&mut s, (20.0, 20.0), (20.0, 20.0));
        assert_eq!(s.snapshot().active_artboard_id, Some(first));
        drag(&mut s, (100.0, 5.0), (100.0, 5.0));
        assert_eq!(s.snapshot().active_artboard_id, None);
        // Panneau : renommer et déplacer, puis supprimer ; tout s'annule.
        s.artboard_set(first, "Carré", 0, 0, 100, 100).unwrap();
        let a = s.snapshot().artboards[0].clone();
        assert_eq!((a.name.as_str(), a.w, a.h), ("Carré", 100, 100));
        assert!(s.artboard_set(first, "Hors", 500, 500, 10, 10).is_err());
        s.artboard_remove(first).unwrap();
        assert_eq!(s.snapshot().artboards.len(), 1);
        s.undo().unwrap();
        assert_eq!(s.snapshot().artboards.len(), 2);
        assert!(s.artboard_add(300, 300, 10, 10, None).is_err(), "entièrement hors du document");
    }

    #[test]
    fn each_artboard_is_exported_as_its_own_image_at_every_scale() {
        let dir = std::env::temp_dir().join(format!("openpics-artboards-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = session();
        assert!(s.artboards_export(&dir.join("x.png"), &[1.0], None).is_err(), "aucun plan");
        // Un carré rouge dans le document blanc, couvert par le premier plan.
        let id = openpics_core::LayerId(s.snapshot().layers[0].id);
        let e = s.entry_mut().unwrap();
        for y in 0..40 {
            for x in 0..40 {
                e.doc.layer_mut(id).unwrap().pixels.put_pixel(x, y, [255, 0, 0, 255]);
            }
        }
        s.artboard_add(0, 0, 40, 40, Some("Carré rouge".into())).unwrap();
        s.artboard_add(100, 20, 80, 50, Some("Bannière / web".into())).unwrap();
        s.artboard_add(0, 0, 10, 10, Some("Carré rouge".into())).unwrap();
        let files = s.artboards_export(&dir.join("affiche.png"), &[1.0, 2.0], None).unwrap();
        let names: Vec<String> = files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect();
        let third = s.snapshot().artboards[2].id;
        assert_eq!(
            names,
            [
                "affiche-Carré-rouge@1x.png".to_owned(),
                "affiche-Carré-rouge@2x.png".to_owned(),
                "affiche-Bannière---web@1x.png".to_owned(),
                "affiche-Bannière---web@2x.png".to_owned(),
                format!("affiche-Carré-rouge-{third}@1x.png"),
                format!("affiche-Carré-rouge-{third}@2x.png"),
            ]
        );
        let img = |n: &str| openpics_io::open(&dir.join(n), openpics_core::DocId(1)).unwrap();
        let red = img("affiche-Carré-rouge@1x.png");
        assert_eq!((red.size().width, red.size().height), (40, 40));
        assert_eq!(red.layers[0].pixels.get_pixel(20, 20), [255, 0, 0, 255], "le contenu du plan est bien découpé");
        let banner = img("affiche-Bannière---web@2x.png");
        assert_eq!((banner.size().width, banner.size().height), (160, 100));
        assert_eq!(banner.layers[0].pixels.get_pixel(10, 10), [255, 255, 255, 255]);
        let single = s.artboards_export(&dir.join("seul.jpg"), &[1.0], Some(80)).unwrap();
        assert!(single[0].file_name().unwrap().to_string_lossy().starts_with("seul-Carré-rouge.jpg"));
    }

    #[test]
    fn resizing_the_image_scales_the_artboards() {
        let mut s = session();
        s.artboard_add(20, 10, 100, 50, None).unwrap();
        s.image_resize(400, 200).unwrap();
        let a = s.snapshot().artboards[0].clone();
        assert_eq!((a.x, a.y, a.w, a.h), (40, 20, 200, 100));
        s.undo().unwrap();
        assert_eq!(s.snapshot().artboards[0].w, 100);
    }
}
