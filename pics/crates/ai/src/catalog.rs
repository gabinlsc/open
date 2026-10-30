//! Catalogue de modèles ONNX et leur stockage local.
//!
//! Le catalogue est un fichier `models.json` dans le dossier des modèles. Chaque entrée porte l'empreinte
//! SHA-256 attendue : un fichier dont l'empreinte diffère n'est jamais rangé. Aucun modèle n'est livré avec
//! le logiciel, et l'application n'en télécharge aucun d'elle-même.

use std::fmt::Write as _;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("modèle inconnu : {0}")]
    UnknownModel(String),
    #[error("empreinte SHA-256 incorrecte : attendue {expected}, obtenue {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("catalogue illisible : {0}")]
    Catalog(String),
    #[error("identifiant de modèle invalide : {0}")]
    BadId(String),
    #[error("{0}")]
    Invalid(String),
    #[error("entrée/sortie : {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Core(#[from] openpics_core::CoreError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Task {
    BackgroundRemoval,
    SubjectSelection,
    Inpainting,
    SuperResolution,
    Denoise,
    Colorize,
}

impl Task {
    pub fn label(self) -> &'static str {
        match self {
            Task::BackgroundRemoval => "Suppression de l'arrière-plan",
            Task::SubjectSelection => "Sélection du sujet",
            Task::Inpainting => "Remplissage génératif",
            Task::SuperResolution => "Super-résolution",
            Task::Denoise => "Débruitage",
            Task::Colorize => "Colorisation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub task: Task,
    pub name: String,
    pub licence: String,
    /// Où l'obtenir (information affichée, jamais appelée par le logiciel).
    pub source: String,
    pub sha256: String,
    pub size_bytes: u64,
}

pub struct ModelStore {
    dir: PathBuf,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn sha256_file(path: &Path) -> Result<String, AiError> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    }))
}

impl ModelStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn model_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.onnx"))
    }

    /// Un catalogue absent est un catalogue vide ; un catalogue corrompu est une erreur explicite.
    /// Les entrées dont l'identifiant pourrait sortir du dossier sont refusées.
    pub fn catalog(&self) -> Result<Vec<ModelEntry>, AiError> {
        let bytes = match std::fs::read(self.dir.join("models.json")) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let entries: Vec<ModelEntry> = serde_json::from_slice(&bytes).map_err(|e| AiError::Catalog(e.to_string()))?;
        if let Some(bad) = entries.iter().find(|e| !valid_id(&e.id)) {
            return Err(AiError::BadId(bad.id.clone()));
        }
        Ok(entries)
    }

    pub fn is_installed(&self, id: &str) -> bool {
        valid_id(id) && self.model_path(id).is_file()
    }

    /// Copie `src` dans le dossier des modèles **après** avoir vérifié son empreinte ; l'écriture est
    /// atomique, donc un import interrompu ne laisse jamais un modèle à moitié écrit sous le bon nom.
    pub fn import(&self, id: &str, src: &Path) -> Result<(), AiError> {
        let entry =
            self.catalog()?.into_iter().find(|e| e.id == id).ok_or_else(|| AiError::UnknownModel(id.to_string()))?;
        let actual = sha256_file(src)?;
        if !actual.eq_ignore_ascii_case(&entry.sha256) {
            return Err(AiError::HashMismatch { expected: entry.sha256, actual });
        }
        std::fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join(format!("{id}.onnx.part"));
        std::fs::copy(src, &tmp)?;
        std::fs::rename(&tmp, self.model_path(id))?;
        Ok(())
    }

    pub fn remove(&self, id: &str) -> Result<(), AiError> {
        if !valid_id(id) {
            return Err(AiError::BadId(id.to_string()));
        }
        match std::fs::remove_file(self.model_path(id)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("openpics-ai-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn entry(id: &str, sha: &str) -> ModelEntry {
        ModelEntry {
            id: id.into(),
            task: Task::BackgroundRemoval,
            name: "Test".into(),
            licence: "MIT".into(),
            source: "local".into(),
            sha256: sha.into(),
            size_bytes: 5,
        }
    }

    // SHA-256("hello") — valeur de référence publique.
    const HELLO: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    #[test]
    fn import_verifies_the_hash_and_installs_atomically() {
        let dir = scratch("import");
        let store = ModelStore::new(dir.join("models"));
        std::fs::create_dir_all(dir.join("models")).unwrap();
        std::fs::write(
            dir.join("models/models.json"),
            serde_json::to_vec(&[entry("good", HELLO), entry("bad", &"0".repeat(64))]).unwrap(),
        )
        .unwrap();
        let file = dir.join("m.onnx");
        std::fs::write(&file, b"hello").unwrap();
        assert_eq!(sha256_file(&file).unwrap(), HELLO);
        store.import("good", &file).unwrap();
        assert!(store.is_installed("good"));
        let err = store.import("bad", &file).unwrap_err();
        assert!(matches!(err, AiError::HashMismatch { .. }), "{err}");
        assert!(!store.is_installed("bad"), "un modèle à l'empreinte fausse n'est jamais rangé");
        assert!(!dir.join("models/bad.onnx.part").exists());
        assert!(matches!(store.import("nope", &file), Err(AiError::UnknownModel(_))));
        store.remove("good").unwrap();
        assert!(!store.is_installed("good"));
        store.remove("good").unwrap(); // idempotent
    }

    #[test]
    fn catalog_is_empty_when_absent_and_rejects_corruption_and_path_traversal() {
        let dir = scratch("catalog");
        let store = ModelStore::new(&dir);
        assert!(store.catalog().unwrap().is_empty());
        std::fs::write(dir.join("models.json"), b"{pas du json").unwrap();
        assert!(matches!(store.catalog(), Err(AiError::Catalog(_))));
        std::fs::write(dir.join("models.json"), serde_json::to_vec(&[entry("../evil", HELLO)]).unwrap()).unwrap();
        assert!(matches!(store.catalog(), Err(AiError::BadId(_))));
        assert!(!store.is_installed("../evil"));
        assert!(matches!(store.remove("../evil"), Err(AiError::BadId(_))));
    }
}
