use serde::Serialize;
use thiserror::Error;
use ts_rs::TS;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("aucun document ouvert")]
    NoActiveDocument,
    #[error("document introuvable : {0}")]
    DocumentNotFound(u64),
    #[error("ce document n'a pas encore de fichier : choisissez un emplacement")]
    NeedsPath,
    #[error("{0}")]
    InvalidArgument(String),
    #[error(transparent)]
    Io(#[from] openpics_io::IoError),
    #[error(transparent)]
    Core(#[from] openpics_core::CoreError),
    #[error(transparent)]
    Ai(#[from] openpics_ai::AiError),
}

impl SessionError {
    fn code(&self) -> &'static str {
        match self {
            SessionError::NoActiveDocument => "no_active_document",
            SessionError::DocumentNotFound(_) => "document_not_found",
            SessionError::NeedsPath => "needs_path",
            SessionError::InvalidArgument(_) => "invalid_argument",
            SessionError::Io(_) => "io",
            SessionError::Core(openpics_core::CoreError::ShortcutConflict { .. }) => "shortcut_conflict",
            SessionError::Core(_) => "core",
            SessionError::Ai(_) => "ai",
        }
    }
}

/// Erreur renvoyée au frontend : un code stable (pour réagir) et un message (pour l'afficher).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl From<SessionError> for ApiError {
    fn from(e: SessionError) -> Self {
        ApiError { code: e.code().to_owned(), message: e.to_string() }
    }
}

impl From<openpics_core::CoreError> for ApiError {
    fn from(e: openpics_core::CoreError) -> Self {
        SessionError::Core(e).into()
    }
}
