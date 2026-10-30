//! Stockage creux par tuiles de 256×256, partagées en copy-on-write.
//!
//! Une tuile absente est transparente ; une tuile uniforme ne coûte aucun tampon. Les tuiles
//! sont derrière un `Arc` : cloner une grille ne copie aucun pixel, et l'historique garde
//! l'ancienne `Arc` d'une tuile avant de la modifier (`Arc::make_mut`), si bien que seules les
//! tuiles touchées occupent de la mémoire supplémentaire.

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::CoreError;
use crate::geom::IRect;

pub const TILE_SIZE: u32 = 256;
const TS: usize = TILE_SIZE as usize;
pub const TILE_BYTES: usize = TS * TS * 4;
/// Limite par côté : garde les calculs d'index en `u32` et refuse les entrées absurdes.
pub const MAX_DIMENSION: u32 = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileKey {
    pub tx: u32,
    pub ty: u32,
}

/// Pixels RGBA 8 bits, alpha non prémultiplié.
#[derive(Debug, Clone, PartialEq)]
pub enum Tile {
    Uniform([u8; 4]),
    Pixels(Box<[u8]>),
}

impl Tile {
    /// Construit une tuile depuis un tampon complet et la compacte si elle est uniforme.
    /// Un tampon de mauvaise taille est rejeté plutôt que tronqué.
    pub fn from_pixels(data: Vec<u8>) -> Result<Tile, CoreError> {
        if data.len() != TILE_BYTES {
            return Err(CoreError::BufferSize {
                width: TILE_SIZE,
                height: TILE_SIZE,
                expected: TILE_BYTES,
                actual: data.len(),
            });
        }
        let first = [data[0], data[1], data[2], data[3]];
        if data.chunks_exact(4).all(|p| p == first) {
            Ok(Tile::Uniform(first))
        } else {
            Ok(Tile::Pixels(data.into_boxed_slice()))
        }
    }

    /// `x` et `y` sont locaux à la tuile ; hors tuile renvoie du transparent.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        match self {
            Tile::Uniform(c) => *c,
            Tile::Pixels(d) => {
                if x >= TILE_SIZE || y >= TILE_SIZE {
                    return [0; 4];
                }
                let i = (y as usize * TS + x as usize) * 4;
                [d[i], d[i + 1], d[i + 2], d[i + 3]]
            }
        }
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, value: [u8; 4]) {
        if x >= TILE_SIZE || y >= TILE_SIZE {
            return;
        }
        if let Tile::Uniform(c) = *self {
            if c == value {
                return;
            }
            let mut buf = vec![0u8; TILE_BYTES];
            for p in buf.chunks_exact_mut(4) {
                p.copy_from_slice(&c);
            }
            *self = Tile::Pixels(buf.into_boxed_slice());
        }
        if let Tile::Pixels(d) = self {
            let i = (y as usize * TS + x as usize) * 4;
            d[i..i + 4].copy_from_slice(&value);
        }
    }

    /// Tampon RGBA complet (alloue pour les tuiles uniformes).
    pub fn to_pixels(&self) -> Vec<u8> {
        match self {
            Tile::Uniform(c) => c.repeat(TS * TS),
            Tile::Pixels(d) => d.to_vec(),
        }
    }
}

/// Emplacement d'une tuile : `None` = absente, donc transparente.
pub type TileSlot = Option<Arc<Tile>>;

/// Une tuile modifiée : clé, état avant, état après.
pub type TileChange = (TileKey, TileSlot, TileSlot);

#[derive(Debug, Clone, PartialEq)]
pub struct TileGrid {
    width: u32,
    height: u32,
    tiles: HashMap<TileKey, Arc<Tile>>,
}

impl TileGrid {
    pub fn new(width: u32, height: u32) -> Result<Self, CoreError> {
        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(CoreError::InvalidSize { width, height, max: MAX_DIMENSION });
        }
        Ok(Self { width, height, tiles: HashMap::new() })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn tiles_x(&self) -> u32 {
        self.width.div_ceil(TILE_SIZE)
    }

    pub fn tiles_y(&self) -> u32 {
        self.height.div_ceil(TILE_SIZE)
    }

    pub fn tile(&self, key: TileKey) -> Option<&Arc<Tile>> {
        self.tiles.get(&key)
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&TileKey, &Arc<Tile>)> {
        self.tiles.iter()
    }

    /// Remplace (ou supprime) une tuile. Les clés hors grille sont ignorées.
    pub fn set_tile(&mut self, key: TileKey, tile: Option<Arc<Tile>>) {
        if key.tx >= self.tiles_x() || key.ty >= self.tiles_y() {
            return;
        }
        match tile {
            Some(t) => {
                self.tiles.insert(key, t);
            }
            None => {
                self.tiles.remove(&key);
            }
        }
    }

    pub fn from_rgba8(width: u32, height: u32, rgba: &[u8]) -> Result<Self, CoreError> {
        let mut grid = Self::new(width, height)?;
        let expected = width as usize * height as usize * 4;
        if rgba.len() != expected {
            return Err(CoreError::BufferSize { width, height, expected, actual: rgba.len() });
        }
        let stride = width as usize * 4;
        for ty in 0..grid.tiles_y() {
            for tx in 0..grid.tiles_x() {
                let mut buf = vec![0u8; TILE_BYTES];
                let x0 = (tx * TILE_SIZE) as usize;
                let y0 = (ty * TILE_SIZE) as usize;
                let cols = (width as usize - x0).min(TS);
                let rows = (height as usize - y0).min(TS);
                for row in 0..rows {
                    let src = (y0 + row) * stride + x0 * 4;
                    buf[row * TS * 4..row * TS * 4 + cols * 4].copy_from_slice(&rgba[src..src + cols * 4]);
                }
                // Le bord hors image reste transparent : on ne compacte donc en `Uniform`
                // que si la tuile est pleine, sinon les pixels hors image fausseraient l'égalité.
                let tile = Tile::from_pixels(buf)?;
                if tile != Tile::Uniform([0; 4]) {
                    grid.tiles.insert(TileKey { tx, ty }, Arc::new(tile));
                }
            }
        }
        Ok(grid)
    }

    pub fn to_rgba8(&self) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut out = vec![0u8; w * h * 4];
        for (key, tile) in &self.tiles {
            let x0 = key.tx as usize * TS;
            let y0 = key.ty as usize * TS;
            let cols = (w - x0).min(TS);
            let rows = (h - y0).min(TS);
            for row in 0..rows {
                let dst = ((y0 + row) * w + x0) * 4;
                match tile.as_ref() {
                    Tile::Uniform(c) => {
                        for p in out[dst..dst + cols * 4].chunks_exact_mut(4) {
                            p.copy_from_slice(c);
                        }
                    }
                    Tile::Pixels(d) => {
                        out[dst..dst + cols * 4].copy_from_slice(&d[row * TS * 4..row * TS * 4 + cols * 4]);
                    }
                }
            }
        }
        out
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0; 4];
        }
        let key = TileKey { tx: x / TILE_SIZE, ty: y / TILE_SIZE };
        self.tiles.get(&key).map_or([0; 4], |t| t.pixel(x % TILE_SIZE, y % TILE_SIZE))
    }

    pub fn put_pixel(&mut self, x: u32, y: u32, value: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let key = TileKey { tx: x / TILE_SIZE, ty: y / TILE_SIZE };
        let entry = self.tiles.entry(key).or_insert_with(|| Arc::new(Tile::Uniform([0; 4])));
        Arc::make_mut(entry).set_pixel(x % TILE_SIZE, y % TILE_SIZE, value);
    }

    /// Clés des tuiles (présentes ou non) qui recouvrent `rect`, en pixels de document.
    pub fn keys_in_rect(&self, rect: IRect) -> Vec<TileKey> {
        let bounds = IRect::new(0, 0, self.width, self.height);
        let Some(r) = rect.intersect(&bounds) else {
            return Vec::new();
        };
        let (x0, y0) = (r.x as u32 / TILE_SIZE, r.y as u32 / TILE_SIZE);
        let (x1, y1) = ((r.x as u32 + r.w - 1) / TILE_SIZE, (r.y as u32 + r.h - 1) / TILE_SIZE);
        let mut keys = Vec::with_capacity(((x1 - x0 + 1) * (y1 - y0 + 1)) as usize);
        for ty in y0..=y1 {
            for tx in x0..=x1 {
                keys.push(TileKey { tx, ty });
            }
        }
        keys
    }

    /// Tuiles qui diffèrent (par identité d'`Arc`, donc en O(tuiles) sans comparer les pixels)
    /// entre `self` et `other` : `(avant, après)` pour chaque clé modifiée.
    pub fn changed_tiles(&self, other: &TileGrid) -> Vec<TileChange> {
        let mut out = Vec::new();
        for (k, old) in &self.tiles {
            match other.tiles.get(k) {
                // Même `Arc`, ou contenu identique : rien à retenir (un traitement qui ne change aucun
                // pixel ne doit pas créer d'état d'historique).
                Some(new) if Arc::ptr_eq(old, new) || old == new => {}
                None if **old == Tile::Uniform([0; 4]) => {}
                new => out.push((*k, Some(old.clone()), new.cloned())),
            }
        }
        for (k, new) in &other.tiles {
            if !self.tiles.contains_key(k) && **new != Tile::Uniform([0; 4]) {
                out.push((*k, None, Some(new.clone())));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: u32, h: u32) -> Vec<u8> {
        (0..w * h).flat_map(|i| [(i % 251) as u8, (i % 241) as u8, (i % 239) as u8, 255]).collect()
    }

    #[test]
    fn roundtrip_non_multiple_of_tile() {
        let (w, h) = (300, 270);
        let src = gradient(w, h);
        let grid = TileGrid::from_rgba8(w, h, &src).unwrap();
        assert_eq!(grid.tiles_x(), 2);
        assert_eq!(grid.tiles_y(), 2);
        assert_eq!(grid.to_rgba8(), src);
    }

    #[test]
    fn rejects_bad_buffer_and_size() {
        assert!(matches!(TileGrid::from_rgba8(2, 2, &[0; 3]), Err(CoreError::BufferSize { .. })));
        assert!(matches!(TileGrid::new(0, 5), Err(CoreError::InvalidSize { .. })));
        assert!(matches!(TileGrid::new(MAX_DIMENSION + 1, 5), Err(CoreError::InvalidSize { .. })));
    }

    #[test]
    fn transparent_tiles_are_not_stored() {
        let grid = TileGrid::from_rgba8(512, 512, &vec![0; 512 * 512 * 4]).unwrap();
        assert_eq!(grid.tile_count(), 0);
    }

    #[test]
    fn full_opaque_tile_is_uniform() {
        let grid = TileGrid::from_rgba8(256, 256, &[10, 20, 30, 255].repeat(256 * 256)).unwrap();
        assert!(matches!(
            grid.tile(TileKey { tx: 0, ty: 0 }).map(|t| t.as_ref()),
            Some(Tile::Uniform([10, 20, 30, 255]))
        ));
    }

    #[test]
    fn clone_shares_tiles_and_write_is_copy_on_write() {
        let mut a = TileGrid::from_rgba8(300, 300, &gradient(300, 300)).unwrap();
        let b = a.clone();
        let k = TileKey { tx: 1, ty: 1 };
        assert!(Arc::ptr_eq(a.tile(k).unwrap(), b.tile(k).unwrap()));
        a.put_pixel(299, 299, [1, 2, 3, 4]);
        assert!(!Arc::ptr_eq(a.tile(k).unwrap(), b.tile(k).unwrap()));
        assert_eq!(b.get_pixel(299, 299), gradient(300, 300)[(299 * 300 + 299) * 4..][..4]);
        assert_eq!(a.get_pixel(299, 299), [1, 2, 3, 4]);
        // Les tuiles non touchées restent partagées.
        let k0 = TileKey { tx: 0, ty: 0 };
        assert!(Arc::ptr_eq(a.tile(k0).unwrap(), b.tile(k0).unwrap()));
        assert_eq!(a.changed_tiles(&b).len(), 1);
    }

    #[test]
    fn changed_tiles_compares_content_when_identity_differs() {
        let a = TileGrid::from_rgba8(300, 300, &gradient(300, 300)).unwrap();
        // Même contenu reconstruit : tuiles distinctes en mémoire, mais aucune différence réelle.
        let b = TileGrid::from_rgba8(300, 300, &gradient(300, 300)).unwrap();
        assert!(a.changed_tiles(&b).is_empty());
        let mut c = b.clone();
        c.put_pixel(1, 1, [9, 9, 9, 9]);
        assert_eq!(a.changed_tiles(&c).len(), 1);
        // Une tuile transparente explicite équivaut à une tuile absente.
        let mut d = TileGrid::new(300, 300).unwrap();
        d.set_tile(TileKey { tx: 0, ty: 0 }, Some(Arc::new(Tile::Uniform([0; 4]))));
        assert!(TileGrid::new(300, 300).unwrap().changed_tiles(&d).is_empty());
        assert!(d.changed_tiles(&TileGrid::new(300, 300).unwrap()).is_empty());
    }

    #[test]
    fn put_pixel_creates_tile_and_ignores_out_of_bounds() {
        let mut g = TileGrid::new(10, 10).unwrap();
        g.put_pixel(3, 4, [9, 8, 7, 255]);
        g.put_pixel(10, 0, [1, 1, 1, 1]);
        assert_eq!(g.get_pixel(3, 4), [9, 8, 7, 255]);
        assert_eq!(g.get_pixel(10, 0), [0; 4]);
        assert_eq!(g.tile_count(), 1);
    }

    #[test]
    fn keys_in_rect_clips_to_grid() {
        let g = TileGrid::new(600, 300).unwrap();
        assert_eq!(g.keys_in_rect(IRect::new(-50, -50, 10_000, 10_000)).len(), 3 * 2);
        assert_eq!(g.keys_in_rect(IRect::new(255, 0, 2, 1)).len(), 2);
        assert!(g.keys_in_rect(IRect::new(700, 0, 5, 5)).is_empty());
    }
}
