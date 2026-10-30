use thiserror::Error;

/// Erreurs du cœur. Chaque variante porte assez de contexte pour être affichée telle quelle.
#[derive(Debug, Error, PartialEq)]
pub enum CoreError {
    #[error("dimensions invalides : {width}×{height} (le maximum est {max} pixels par côté)")]
    InvalidSize { width: u32, height: u32, max: u32 },
    #[error("tampon de pixels de {actual} octets, {expected} attendus pour {width}×{height} RGBA")]
    BufferSize { width: u32, height: u32, expected: usize, actual: usize },
    #[error("calque introuvable : {0}")]
    LayerNotFound(u64),
    #[error("le calque « {0} » est verrouillé")]
    LayerLocked(String),
    #[error("le calque {0} n'a pas de masque")]
    NoMask(u64),
    #[error("le calque {0} a déjà un masque")]
    HasMask(u64),
    #[error("position de calque invalide : {index} (le document a {len} calques)")]
    LayerIndex { index: usize, len: usize },
    #[error("repère introuvable : {0}")]
    GuideNotFound(u64),
    #[error("raccourci invalide « {0} »")]
    InvalidChord(String),
    #[error("le raccourci {chord} est déjà utilisé par « {action} »")]
    ShortcutConflict { chord: String, action: String },
    #[error("action inconnue : {0}")]
    UnknownAction(String),
}
