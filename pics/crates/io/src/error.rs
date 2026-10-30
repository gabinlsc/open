use thiserror::Error;

#[derive(Debug, Error)]
pub enum IoError {
    #[error("erreur de fichier : {0}")]
    Io(#[from] std::io::Error),
    #[error("image illisible ou non prise en charge : {0}")]
    Image(#[from] image::ImageError),
    #[error("archive .ocv invalide : {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("métadonnées .ocv invalides : {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Core(#[from] openpics_core::CoreError),
    #[error("format non pris en charge : {0}")]
    UnsupportedFormat(String),
    #[error("fichier .ocv invalide : {0}")]
    Invalid(String),
}
