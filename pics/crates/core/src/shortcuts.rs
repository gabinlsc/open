//! Raccourcis clavier personnalisables.
//!
//! Le cœur est la source de vérité : il valide la forme des combinaisons et refuse les
//! conflits. Le frontend charge la table une fois et résout les touches localement, pour ne pas
//! payer un aller-retour IPC à chaque frappe.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Forme canonique : modificateurs dans l'ordre Ctrl, Alt, Shift puis la touche, ex. `Ctrl+Shift+Z`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Chord {
    ctrl: bool,
    alt: bool,
    shift: bool,
    key: String,
}

impl Chord {
    pub fn parse(s: &str) -> Result<Chord, CoreError> {
        let bad = || CoreError::InvalidChord(s.to_owned());
        let parts: Vec<&str> = s.split('+').collect();
        // « Ctrl++ » (touche +) : le dernier segment est vide, la touche est « + ».
        let (mods, key) = if s.ends_with("++") || s == "+" {
            (&parts[..parts.len().saturating_sub(2)], "+")
        } else {
            let (key, mods) = parts.split_last().ok_or_else(bad)?;
            (mods, *key)
        };
        let (mut ctrl, mut alt, mut shift) = (false, false, false);
        for m in mods {
            let flag = match m.to_ascii_lowercase().as_str() {
                "ctrl" | "cmd" | "mod" => &mut ctrl,
                "alt" => &mut alt,
                "shift" => &mut shift,
                _ => return Err(bad()),
            };
            if *flag {
                return Err(bad());
            }
            *flag = true;
        }
        let key = normalize_key(key).ok_or_else(bad)?;
        Ok(Chord { ctrl, alt, shift, key })
    }
}

fn normalize_key(key: &str) -> Option<String> {
    if key.is_empty() || key.chars().count() > 16 || key.contains('+') && key != "+" {
        return None;
    }
    let mut chars = key.chars();
    let (first, rest) = (chars.next()?, chars.as_str());
    if rest.is_empty() {
        return Some(first.to_uppercase().collect());
    }
    // Touches nommées (F5, Escape, ArrowLeft…) : première lettre en capitale, le reste inchangé.
    if !key.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(first.to_uppercase().chain(rest.chars()).collect())
}

impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        f.write_str(&self.key)
    }
}

impl From<Chord> for String {
    fn from(c: Chord) -> String {
        c.to_string()
    }
}

impl TryFrom<String> for Chord {
    type Error = CoreError;
    fn try_from(s: String) -> Result<Self, CoreError> {
        Chord::parse(&s)
    }
}

/// Actions de la phase 1 liées à un raccourci : identifiant stable → raccourcis par défaut.
pub const DEFAULT_BINDINGS: &[(&str, &[&str])] = &[
    ("file.new", &["Ctrl+N"]),
    ("file.open", &["Ctrl+O"]),
    ("file.save", &["Ctrl+S"]),
    ("file.save_as", &["Ctrl+Shift+S"]),
    ("file.export", &["Ctrl+Alt+Shift+S"]),
    ("file.close", &["Ctrl+W"]),
    ("app.palette", &["Ctrl+K"]),
    ("view.compare", &["Ctrl+Alt+C"]),
    ("edit.undo", &["Ctrl+Z"]),
    ("edit.redo", &["Ctrl+Shift+Z", "Ctrl+Y"]),
    ("view.zoom_in", &["Ctrl++", "Ctrl+="]),
    ("view.zoom_out", &["Ctrl+-"]),
    ("view.fit", &["Ctrl+0"]),
    ("view.actual_size", &["Ctrl+1"]),
    ("view.toggle_rulers", &["Ctrl+R"]),
    ("view.toggle_grid", &["Ctrl+'"]),
    ("view.toggle_guides", &["Ctrl+;"]),
    ("view.toggle_snap", &["Ctrl+Shift+;"]),
    ("layer.new", &["Ctrl+Shift+N"]),
    ("select.all", &["Ctrl+A"]),
    ("select.none", &["Ctrl+D"]),
    ("select.invert", &["Ctrl+Shift+I"]),
    ("tool.rect_select", &["M"]),
    ("tool.lasso", &["L"]),
    ("tool.wand", &["W"]),
    ("tool.brush", &["B"]),
    ("tool.pencil", &["N"]),
    ("tool.eraser", &["E"]),
    ("tool.gradient", &["G"]),
    ("tool.bucket", &["K"]),
    ("tool.clone", &["S"]),
    ("tool.heal", &["J"]),
    ("tool.blur", &["R"]),
    ("paint.swap_colors", &["X"]),
    ("tool.finish", &["Enter"]),
    ("tool.cancel", &["Escape"]),
    ("tab.next", &["Ctrl+Tab"]),
    ("tab.prev", &["Ctrl+Shift+Tab"]),
    ("tool.hand", &["H"]),
    ("tool.zoom", &["Z"]),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keymap {
    bindings: BTreeMap<String, Vec<Chord>>,
}

impl Default for Keymap {
    fn default() -> Self {
        let bindings = DEFAULT_BINDINGS
            .iter()
            .map(|(action, chords)| {
                // Les défauts sont des constantes testées ; un échec ici ne peut venir que d'un
                // défaut mal écrit, ce que `defaults_are_valid_and_conflict_free` attrape.
                let parsed = chords.iter().filter_map(|c| Chord::parse(c).ok()).collect();
                ((*action).to_owned(), parsed)
            })
            .collect();
        Self { bindings }
    }
}

impl Keymap {
    pub fn bindings(&self) -> &BTreeMap<String, Vec<Chord>> {
        &self.bindings
    }

    pub fn action_for(&self, chord: &Chord) -> Option<&str> {
        self.bindings.iter().find(|(_, cs)| cs.contains(chord)).map(|(a, _)| a.as_str())
    }

    /// Remplace les raccourcis d'une action. Refuse une action inconnue ou un conflit avec une
    /// autre action : le premier raccourci n'est pas volé silencieusement.
    pub fn set(&mut self, action: &str, chords: Vec<Chord>) -> Result<(), CoreError> {
        if !self.bindings.contains_key(action) {
            return Err(CoreError::UnknownAction(action.to_owned()));
        }
        for c in &chords {
            if let Some(owner) = self.action_for(c) {
                if owner != action {
                    return Err(CoreError::ShortcutConflict { chord: c.to_string(), action: owner.to_owned() });
                }
            }
        }
        self.bindings.insert(action.to_owned(), chords);
        Ok(())
    }

    pub fn reset(&mut self) {
        *self = Keymap::default();
    }

    /// Relit une table enregistrée : ignore les actions disparues, complète celles ajoutées
    /// depuis, et écarte tout conflit pour qu'un fichier édité à la main ne casse pas l'app.
    pub fn merged_with_defaults(saved: Keymap) -> Keymap {
        let mut out = Keymap::default();
        for (action, chords) in saved.bindings {
            let _ = out.set(&action, chords);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_normalizes_order_and_case() {
        assert_eq!(Chord::parse("shift+ctrl+z").unwrap().to_string(), "Ctrl+Shift+Z");
        assert_eq!(Chord::parse("Ctrl+Alt+Shift+S").unwrap().to_string(), "Ctrl+Alt+Shift+S");
        assert_eq!(Chord::parse("f5").unwrap().to_string(), "F5");
        assert_eq!(Chord::parse("Ctrl++").unwrap().to_string(), "Ctrl++");
        assert_eq!(Chord::parse("Ctrl+-").unwrap().to_string(), "Ctrl+-");
    }

    #[test]
    fn parse_rejects_garbage() {
        for bad in ["", "Ctrl+", "Foo+Z", "Ctrl+Ctrl+Z", "Ctrl+Shift+@@", "Ctrl+Z+X"] {
            assert!(Chord::parse(bad).is_err(), "{bad} devrait être refusé");
        }
    }

    #[test]
    fn defaults_are_valid_and_conflict_free() {
        let mut seen = std::collections::HashSet::new();
        for (action, chords) in DEFAULT_BINDINGS {
            for c in *chords {
                let parsed = Chord::parse(c).unwrap_or_else(|_| panic!("défaut invalide {action}: {c}"));
                assert!(seen.insert(parsed), "doublon par défaut : {c}");
            }
        }
        assert_eq!(Keymap::default().bindings().len(), DEFAULT_BINDINGS.len());
    }

    #[test]
    fn rebind_and_conflict_detection() {
        let mut km = Keymap::default();
        km.set("file.save", vec![Chord::parse("Ctrl+Alt+S").unwrap()]).unwrap();
        assert_eq!(km.action_for(&Chord::parse("Ctrl+Alt+S").unwrap()), Some("file.save"));
        let err = km.set("file.open", vec![Chord::parse("Ctrl+Z").unwrap()]).unwrap_err();
        assert!(matches!(err, CoreError::ShortcutConflict { .. }));
        assert!(matches!(km.set("nope", vec![]), Err(CoreError::UnknownAction(_))));
        // Une action peut garder son propre raccourci sans conflit.
        km.set("edit.undo", vec![Chord::parse("Ctrl+Z").unwrap()]).unwrap();
    }

    #[test]
    fn json_roundtrip_and_merge() {
        let mut km = Keymap::default();
        km.set("tool.hand", vec![Chord::parse("Ctrl+H").unwrap()]).unwrap();
        let json = serde_json::to_string(&km).unwrap();
        let back: Keymap = serde_json::from_str(&json).unwrap();
        assert_eq!(back, km);
        let merged = Keymap::merged_with_defaults(back);
        assert_eq!(merged.action_for(&Chord::parse("Ctrl+H").unwrap()), Some("tool.hand"));
        // Un fichier qui vole « Ctrl+Z » à l'annulation est neutralisé.
        let mut hostile = Keymap::default();
        hostile.bindings.insert("tool.zoom".into(), vec![Chord::parse("Ctrl+Z").unwrap()]);
        let merged = Keymap::merged_with_defaults(hostile);
        assert_eq!(merged.action_for(&Chord::parse("Ctrl+Z").unwrap()), Some("edit.undo"));
    }
}
