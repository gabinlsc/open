//! Rendu du canvas avec wgpu.
//!
//! Le frontend n'échantillonne jamais les pixels : ce crate dessine directement les tuiles du
//! document dans une surface native (ou une texture hors écran pour les tests).

mod error;
mod renderer;

pub use error::GpuError;
pub use renderer::{FrameParams, Renderer, Theme, WindowTarget};
