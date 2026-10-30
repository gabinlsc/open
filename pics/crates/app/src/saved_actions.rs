//! Actions enregistrées : un script Rhai par fichier `<nom>.rhai` dans le dossier de configuration.

use std::path::{Path, PathBuf};

use crate::error::SessionError;

fn valid(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 48
        && name.chars().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        && !name.starts_with(' ')
}

fn path(dir: &Path, name: &str) -> Result<PathBuf, SessionError> {
    if !valid(name) {
        return Err(SessionError::InvalidArgument(
            "nom d'action invalide : lettres, chiffres, espaces, - et _ (48 caractères maximum)".into(),
        ));
    }
    Ok(dir.join(format!("{name}.rhai")))
}

pub fn list(dir: &Path) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut names: Vec<String> = read
        .filter_map(Result::ok)
        .filter_map(|e| {
            let p = e.path();
            (p.extension()? == "rhai").then(|| p.file_stem()?.to_str().map(str::to_owned))?
        })
        .filter(|n| valid(n))
        .collect();
    names.sort();
    names
}

pub fn load(dir: &Path, name: &str) -> Result<String, SessionError> {
    std::fs::read_to_string(path(dir, name)?).map_err(|e| SessionError::InvalidArgument(format!("{name} : {e}")))
}

pub fn save(dir: &Path, name: &str, script: &str) -> Result<(), SessionError> {
    let target = path(dir, name)?;
    std::fs::create_dir_all(dir).map_err(openpics_io::IoError::from)?;
    std::fs::write(target, script).map_err(openpics_io::IoError::from)?;
    Ok(())
}

pub fn delete(dir: &Path, name: &str) -> Result<(), SessionError> {
    match std::fs::remove_file(path(dir, name)?) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(openpics_io::IoError::from(e).into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_round_trip_and_names_cannot_escape_the_folder() {
        let dir = std::env::temp_dir().join(format!("openpics-actions-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(list(&dir).is_empty());
        save(&dir, "Noir et blanc", "filter(\"threshold\", [0.5]);").unwrap();
        save(&dir, "Autre", "op(\"invert\");").unwrap();
        assert_eq!(list(&dir), ["Autre", "Noir et blanc"]);
        assert_eq!(load(&dir, "Autre").unwrap(), "op(\"invert\");");
        for bad in ["", "../x", "a/b", "a\\b", ".cache", " lead", &"x".repeat(49)] {
            assert!(save(&dir, bad, "x").is_err(), "{bad:?}");
            assert!(load(&dir, bad).is_err());
            assert!(delete(&dir, bad).is_err());
        }
        delete(&dir, "Autre").unwrap();
        delete(&dir, "Autre").unwrap();
        assert_eq!(list(&dir), ["Noir et blanc"]);
    }
}
