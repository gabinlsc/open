use thiserror::Error;

#[derive(Debug, Error)]
pub enum GpuError {
    #[error("aucune carte graphique compatible n'a été trouvée : {0}")]
    NoAdapter(String),
    #[error("impossible d'initialiser le GPU : {0}")]
    Device(String),
    #[error("impossible de créer la surface d'affichage : {0}")]
    Surface(String),
    #[error("ce moteur de rendu n'a pas de surface d'affichage")]
    NoSurface,
    #[error("lecture de l'image rendue impossible : {0}")]
    Readback(String),
    #[error("taille de rendu invalide : {width}×{height}")]
    InvalidSize { width: u32, height: u32 },
}
