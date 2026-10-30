//! Enregistrement automatique de récupération (voir `crate::recovery`).

use std::path::{Path, PathBuf};

use crate::recovery;

use super::{Session, SessionError};

impl Session {
    pub fn set_recovery_dir(&mut self, dir: PathBuf) {
        self.recovery_dir = Some(dir);
    }

    pub(super) fn drop_recovery(&self, doc_id: u64) {
        if let Some(dir) = &self.recovery_dir {
            recovery::remove_for(dir, &self.recovery_tag, doc_id);
        }
    }

    /// Recopie les documents modifiés et efface les copies de ceux qui ne le sont plus. Renvoie le nombre de copies
    /// écrites. Une erreur sur un document n'empêche pas de sauvegarder les autres.
    pub fn autosave(&mut self) -> Result<usize, SessionError> {
        let Some(dir) = self.recovery_dir.clone() else { return Ok(0) };
        std::fs::create_dir_all(&dir).map_err(openpics_io::IoError::from)?;
        let mut written = 0;
        let mut first_error = None;
        for e in &self.docs {
            let id = e.doc.id.0;
            if !e.doc.is_dirty() {
                recovery::remove_for(&dir, &self.recovery_tag, id);
                continue;
            }
            // Le nom peut avoir changé depuis la dernière copie : on repart d'une copie propre.
            let file = recovery::file_for(&dir, &self.recovery_tag, id, &e.doc.name);
            match openpics_io::write_copy(&e.doc, &file) {
                Ok(()) => {
                    written += 1;
                    cleanup_other_names(&dir, &self.recovery_tag, id, &file);
                }
                Err(err) => first_error = first_error.or(Some(err)),
            }
        }
        match first_error {
            Some(err) => Err(err.into()),
            None => Ok(written),
        }
    }

    /// Rouvre une copie de récupération comme nouveau document non enregistré, puis la supprime.
    pub fn recovery_restore(&mut self, dir: &Path, file: &str) -> Result<u64, SessionError> {
        let path = recovery::resolve(dir, file)
            .ok_or_else(|| SessionError::InvalidArgument(format!("fichier de récupération invalide : {file}")))?;
        let id = self.alloc_doc_id();
        let doc = openpics_io::open_recovered(&path, id)?;
        let id = self.insert(doc);
        let _ = std::fs::remove_file(path);
        Ok(id.0)
    }
}

fn cleanup_other_names(dir: &Path, tag: &str, id: u64, keep: &Path) {
    let prefix = format!("{tag}-{id}-");
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for e in read.filter_map(Result::ok) {
        let p = e.path();
        if p != keep && e.file_name().to_str().is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".ocv")) {
            let _ = std::fs::remove_file(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpics_core::ViewSettings;
    use openpics_core::shortcuts::Keymap;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("openpics-rec-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn session(dir: &Path) -> Session {
        let mut s = Session::new(ViewSettings::default(), Keymap::default());
        s.set_canvas(800, 600);
        s.set_recovery_dir(dir.to_path_buf());
        s
    }

    #[test]
    fn dirty_documents_are_copied_and_clean_or_closed_ones_are_not_kept() {
        let dir = scratch("cycle");
        let mut s = session(&dir);
        s.new_document(Some("Mon projet".into()), 30, 20).unwrap();
        assert_eq!(s.autosave().unwrap(), 0, "un document neuf n'est pas modifié");
        s.select_all().unwrap();
        s.select_invert().unwrap(); // modifie l'historique
        assert_eq!(s.autosave().unwrap(), 1);
        assert_eq!(recovery::list(&dir).len(), 1);
        assert_eq!(s.autosave().unwrap(), 1);
        assert_eq!(recovery::list(&dir).len(), 1, "une seule copie par document");
        let id = s.snapshot().active_id.unwrap();
        s.close(id).unwrap();
        assert!(recovery::list(&dir).is_empty(), "fermer le document supprime sa copie");
    }

    #[test]
    fn saving_removes_the_copy_and_a_crash_leaves_one_that_can_be_restored() {
        let dir = scratch("restore");
        let out = scratch("restore-out");
        std::fs::create_dir_all(&out).unwrap();
        let mut s = session(&dir);
        s.new_document(Some("Rapport".into()), 30, 20).unwrap();
        s.select_all().unwrap();
        s.autosave().unwrap();
        assert_eq!(recovery::list(&dir).len(), 1);
        s.save_active(Some(out.join("r.ocv"))).unwrap();
        assert!(recovery::list(&dir).is_empty(), "enregistrer supprime la copie");
        s.select_invert().unwrap();
        s.autosave().unwrap();
        drop(s); // « plantage » : la copie reste
        let left = recovery::list(&dir);
        assert_eq!(left.len(), 1);
        let mut fresh = session(&dir);
        let id = fresh.recovery_restore(&dir, &left[0].file).unwrap();
        assert!(fresh.is_dirty(id).unwrap(), "le document restauré est à enregistrer");
        assert_eq!(fresh.snapshot().docs.iter().find(|t| t.id == id).map(|t| t.name.clone()).as_deref(), Some("r"));
        assert!(recovery::list(&dir).is_empty(), "la copie est consommée");
        assert!(fresh.save_active(None).is_err(), "pas de chemin : « Enregistrer sous » est demandé");
    }

    #[test]
    fn restore_refuses_paths_that_leave_the_recovery_folder() {
        let dir = scratch("traversal");
        let mut s = session(&dir);
        for bad in ["../x.ocv", "a/b.ocv", "a\\b.ocv", "x.png", ""] {
            assert!(s.recovery_restore(&dir, bad).is_err(), "{bad}");
        }
    }
}
