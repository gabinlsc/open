use std::path::Path;

use crate::error::IoError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
    Webp,
    Tiff,
    Bmp,
    Gif,
    Ocv,
}

impl Format {
    pub fn from_path(path: &Path) -> Result<Format, IoError> {
        let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
        match ext.as_deref() {
            Some("png") => Ok(Format::Png),
            Some("jpg" | "jpeg" | "jpe") => Ok(Format::Jpeg),
            Some("webp") => Ok(Format::Webp),
            Some("tif" | "tiff") => Ok(Format::Tiff),
            Some("bmp") => Ok(Format::Bmp),
            Some("gif") => Ok(Format::Gif),
            Some("ocv") => Ok(Format::Ocv),
            Some(other) => Err(IoError::UnsupportedFormat(format!(".{other}"))),
            None => Err(IoError::UnsupportedFormat("extension manquante".into())),
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpg",
            Format::Webp => "webp",
            Format::Tiff => "tiff",
            Format::Bmp => "bmp",
            Format::Gif => "gif",
            Format::Ocv => "ocv",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_case_insensitively() {
        assert_eq!(Format::from_path(Path::new("a/B.JPEG")).unwrap(), Format::Jpeg);
        assert_eq!(Format::from_path(Path::new("x.OCV")).unwrap(), Format::Ocv);
    }

    #[test]
    fn rejects_unknown_and_missing() {
        assert!(Format::from_path(Path::new("x.psd")).is_err());
        assert!(Format::from_path(Path::new("noext")).is_err());
    }
}
