//! Récupération après plantage : les documents modifiés sont recopiés régulièrement en `.ocv` dans un dossier
//! dédié, et ces copies sont supprimées dès que le document est enregistré ou fermé. Au démarrage, ce qui reste
//! est la trace d'une fermeture anormale : l'utilisateur choisit de le restaurer ou de l'abandonner.

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecoveryDto {
    pub name: String,
    pub file: String,
}

/// Nom de fichier sûr pour un nom de document quelconque.
pub(crate) fn safe_name(name: &str) -> String {
    let cleaned: String =
        name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    cleaned.chars().take(48).collect()
}

pub(crate) fn file_for(dir: &Path, tag: &str, doc_id: u64, name: &str) -> PathBuf {
    dir.join(format!("{tag}-{doc_id}-{}.ocv", safe_name(name)))
}

/// Les copies présentes, avec le nom d'origine (le nom enregistré dans le fichier, pas celui du disque).
pub fn list(dir: &Path) -> Vec<RecoveryDto> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<RecoveryDto> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ocv"))
        .filter_map(|p| {
            let file = p.file_name()?.to_str()?.to_string();
            let stem = p.file_stem()?.to_str()?;
            // `<tag>-<id>-<nom>` : le nom lisible est tout ce qui suit le deuxième tiret.
            let name = stem.splitn(3, '-').nth(2).unwrap_or(stem).replace('_', " ");
            Some(RecoveryDto { name, file })
        })
        .collect();
    out.sort_by(|a, b| a.file.cmp(&b.file));
    out
}

/// Un nom de fichier venu de l'interface ne doit jamais sortir du dossier.
pub fn resolve(dir: &Path, file: &str) -> Option<PathBuf> {
    let ok = !file.is_empty() && !file.contains(['/', '\\']) && !file.contains("..") && file.ends_with(".ocv");
    ok.then(|| dir.join(file))
}

pub fn discard(dir: &Path, file: &str) {
    if let Some(p) = resolve(dir, file) {
        let _ = std::fs::remove_file(p);
    }
}

/// Supprime les copies d'un document (quel que soit le nom qu'il portait au moment de la copie).
pub(crate) fn remove_for(dir: &Path, tag: &str, doc_id: u64) {
    let prefix = format!("{tag}-{doc_id}-");
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for e in read.filter_map(Result::ok) {
        if e.file_name().to_str().is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".ocv")) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}
