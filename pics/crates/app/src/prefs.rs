//! Préférences : un fichier JSON local, jamais envoyé nulle part.

use std::path::{Path, PathBuf};

use openpics_core::ViewSettings;
use openpics_core::shortcuts::Keymap;
use serde::{Deserialize, Serialize};

use crate::dto::ThemeChoice;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub theme: ThemeChoice,
    pub settings: ViewSettings,
    pub keymap: Keymap,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { theme: ThemeChoice::System, settings: ViewSettings::default(), keymap: Keymap::default() }
    }
}

impl Prefs {
    /// Un fichier absent, illisible ou corrompu redonne les valeurs par défaut : une préférence
    /// cassée ne doit jamais empêcher de démarrer.
    pub fn load(path: &Path) -> Prefs {
        let Ok(bytes) = std::fs::read(path) else {
            return Prefs::default();
        };
        match serde_json::from_slice::<Prefs>(&bytes) {
            Ok(mut p) => {
                p.keymap = Keymap::merged_with_defaults(p.keymap);
                p
            }
            Err(_) => Prefs::default(),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)?;
        std::fs::rename(&tmp, path)
    }
}

/// Mode portable : un fichier `portable` à côté de l'exécutable range les données dans `data/`
/// à côté de lui, et rien n'est écrit ailleurs sur la machine.
pub fn config_dir(exe_dir: Option<&Path>, default_dir: PathBuf) -> PathBuf {
    match exe_dir {
        Some(dir) if dir.join("portable").exists() => dir.join("data"),
        _ => default_dir,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_defaults_on_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub/prefs.json");
        assert_eq!(Prefs::load(&p).theme, ThemeChoice::System);
        let mut prefs = Prefs { theme: ThemeChoice::Dark, ..Prefs::default() };
        prefs.settings.show_grid = true;
        prefs.save(&p).unwrap();
        let back = Prefs::load(&p);
        assert_eq!(back.theme, ThemeChoice::Dark);
        assert!(back.settings.show_grid);
        std::fs::write(&p, b"{ not json").unwrap();
        assert_eq!(Prefs::load(&p).theme, ThemeChoice::System);
    }

    #[test]
    fn partial_file_is_completed_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("prefs.json");
        std::fs::write(&p, br#"{"theme":"light"}"#).unwrap();
        let prefs = Prefs::load(&p);
        assert_eq!(prefs.theme, ThemeChoice::Light);
        assert_eq!(prefs.keymap, Keymap::default());
    }

    #[test]
    fn portable_flag_redirects_data_dir() {
        let dir = tempfile::tempdir().unwrap();
        let default = PathBuf::from("/appdata");
        assert_eq!(config_dir(Some(dir.path()), default.clone()), default);
        std::fs::write(dir.path().join("portable"), b"").unwrap();
        assert_eq!(config_dir(Some(dir.path()), default), dir.path().join("data"));
    }
}
