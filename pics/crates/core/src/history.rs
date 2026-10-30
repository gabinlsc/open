//! Historique linéaire, non borné en nombre d'états.
//!
//! La borne réelle est la mémoire : une commande de pixels ne retient que les tuiles qu'elle a
//! modifiées (cf. `commands::ReplaceTiles`), partagées en copy-on-write avec le document. Un
//! plafond mémoire avec déversement sur disque des tuiles froides est prévu et hors phase 1.

use crate::document::Document;
use crate::error::CoreError;

pub trait Command: Send {
    fn name(&self) -> &str;
    fn apply(&mut self, doc: &mut Document) -> Result<(), CoreError>;
    fn revert(&mut self, doc: &mut Document) -> Result<(), CoreError>;
}

pub struct History {
    undo: Vec<Box<dyn Command>>,
    redo: Vec<Box<dyn Command>>,
    /// Position (nombre d'états appliqués) du dernier enregistrement. `None` si cet état n'est plus
    /// atteignable parce qu'une nouvelle action a effacé la branche « rétablir » qui le contenait.
    saved_at: Option<usize>,
}

impl Default for History {
    fn default() -> Self {
        Self { undo: Vec::new(), redo: Vec::new(), saved_at: Some(0) }
    }
}

impl std::fmt::Debug for History {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("History").field("undo", &self.undo.len()).field("redo", &self.redo.len()).finish()
    }
}

impl History {
    /// Enregistre une commande déjà appliquée ; toute nouvelle action invalide le « rétablir ».
    pub(crate) fn record(&mut self, cmd: Box<dyn Command>) {
        if self.saved_at.is_some_and(|s| s > self.undo.len()) {
            self.saved_at = None;
        }
        self.redo.clear();
        self.undo.push(cmd);
    }

    pub(crate) fn take_undo(&mut self) -> Option<Box<dyn Command>> {
        self.undo.pop()
    }

    pub(crate) fn restore_undo(&mut self, cmd: Box<dyn Command>) {
        self.undo.push(cmd);
    }

    pub(crate) fn take_redo(&mut self) -> Option<Box<dyn Command>> {
        self.redo.pop()
    }

    pub(crate) fn push_redo(&mut self, cmd: Box<dyn Command>) {
        self.redo.push(cmd);
    }

    /// Aucun état n'est « enregistré » : le document est toujours considéré comme modifié.
    pub(crate) fn mark_unsaved(&mut self) {
        self.saved_at = None;
    }

    pub(crate) fn mark_saved(&mut self) {
        self.saved_at = Some(self.undo.len());
    }

    /// Vrai si l'état courant est exactement celui du dernier enregistrement : annuler puis
    /// rétablir une modification ne laisse donc pas le document « modifié » à tort.
    pub fn is_at_saved_state(&self) -> bool {
        self.saved_at == Some(self.undo.len())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Noms des états, du plus ancien au plus récent ; `position` = nombre d'états appliqués.
    pub fn entries(&self) -> (Vec<String>, usize) {
        let applied = self.undo.len();
        let names = self
            .undo
            .iter()
            .map(|c| c.name().to_owned())
            .chain(self.redo.iter().rev().map(|c| c.name().to_owned()))
            .collect();
        (names, applied)
    }
}
