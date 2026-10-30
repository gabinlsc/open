//! Cœur d'OpenPics : tout ce qui décrit un document et son édition, sans GPU ni fichier.
//!
//! Cette séparation permet de tester l'historique, les tuiles et la vue de façon
//! déterministe, et d'éviter qu'une dépendance lourde (wgpu, codecs) n'entre ici.

pub mod blend;
pub mod commands;
pub mod composite;
pub mod document;
pub mod error;
pub mod geom;
pub mod history;
pub mod shortcuts;
pub mod snap;
pub mod tile;
pub mod view;

pub use blend::BlendMode;
pub use document::{Axis, DocId, Document, Guide, Layer, LayerId, LayerProps, Locks, Mask};
pub use error::CoreError;
pub use geom::{IRect, Size, Vec2};
pub use tile::{TILE_SIZE, Tile, TileGrid, TileKey};
pub use view::{ViewSettings, Viewport};
