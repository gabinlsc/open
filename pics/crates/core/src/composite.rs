//! Compositeur de référence (CPU) : calques, modes de fusion, opacité, remplissage, masques, écrêtage.
//!
//! Il travaille tuile par tuile en flottant, ce qui borne la mémoire à une tuile même sur un
//! document de 100 Mpx. C'est la vérité de référence contre laquelle les shaders GPU sont comparés.

use crate::blend::{BlendMode, composite};
use crate::document::{Document, Layer};
use crate::tile::{TILE_SIZE, Tile, TileKey};

const TS: usize = TILE_SIZE as usize;
const N: usize = TS * TS;

type Px = [f32; 4];

/// Lit une tuile de calque en flottant (alpha droit). `None` = tuile absente, donc transparente.
fn read_tile(layer: &Layer, key: TileKey) -> Option<Vec<Px>> {
    let tile = layer.pixels.tile(key)?;
    let mut out = Vec::with_capacity(N);
    match tile.as_ref() {
        Tile::Uniform(c) => out.resize(N, to_f32(*c)),
        Tile::Pixels(d) => out.extend(d.chunks_exact(4).map(|p| to_f32([p[0], p[1], p[2], p[3]]))),
    }
    Some(out)
}

fn to_f32(p: [u8; 4]) -> Px {
    [f32::from(p[0]) / 255.0, f32::from(p[1]) / 255.0, f32::from(p[2]) / 255.0, f32::from(p[3]) / 255.0]
}

/// Valeur de masque (0..1) pour chaque pixel de la tuile ; `None` si le calque n'a pas de masque actif.
fn read_mask(layer: &Layer, key: TileKey) -> Option<Vec<f32>> {
    let mask = layer.mask.as_ref().filter(|m| m.enabled)?;
    Some(match mask.pixels.tile(key).map(|t| t.as_ref()) {
        None => vec![0.0; N],
        Some(Tile::Uniform(c)) => vec![f32::from(c[0]) / 255.0; N],
        Some(Tile::Pixels(d)) => d.chunks_exact(4).map(|p| f32::from(p[0]) / 255.0).collect(),
    })
}

/// Contenu d'un calque pour une tuile : couleur, alpha « de contenu » (pixels × masque × remplissage).
struct Content {
    px: Vec<Px>,
}

fn content(layer: &Layer, key: TileKey) -> Option<Content> {
    let mut px = read_tile(layer, key)?;
    let mask = read_mask(layer, key);
    for (i, p) in px.iter_mut().enumerate() {
        p[3] *= layer.fill * mask.as_ref().map_or(1.0, |m| m[i]);
    }
    Some(Content { px })
}

fn tile_origin(key: TileKey) -> (u32, u32) {
    (key.tx * TILE_SIZE, key.ty * TILE_SIZE)
}

/// Fusionne `src` (déjà prémultiplié par fill/masque) sur `backdrop` avec le mode et l'opacité donnés.
fn blend_onto(backdrop: &mut [Px], src: &[Px], mode: BlendMode, opacity: f32, key: TileKey) {
    let (ox, oy) = tile_origin(key);
    for (i, (b, s)) in backdrop.iter_mut().zip(src).enumerate() {
        let alpha = s[3] * opacity;
        if alpha <= 0.0 {
            continue;
        }
        let (x, y) = (ox + (i % TS) as u32, oy + (i / TS) as u32);
        *b = composite(mode, *b, [s[0], s[1], s[2]], alpha, x, y);
    }
}

/// Composition d'une tuile : un calque de base et les calques écrêtés qui le suivent forment un groupe
/// d'écrêtage, composé à part puis fusionné avec le mode et l'opacité de la base.
pub fn composite_tile(doc: &Document, key: TileKey) -> Vec<Px> {
    let mut backdrop = vec![[0.0; 4]; N];
    let layers = &doc.layers;
    let mut i = 0;
    while i < layers.len() {
        let base = &layers[i];
        let mut end = i + 1;
        while end < layers.len() && layers[end].clip_to_below {
            end += 1;
        }
        let clipped = &layers[i + 1..end];
        i = end;
        if !base.visible {
            continue; // un calque caché cache aussi ce qui lui est écrêté
        }
        let Some(mut group) = content(base, key) else { continue };
        let base_alpha: Vec<f32> = group.px.iter().map(|p| p[3]).collect();
        for layer in clipped.iter().filter(|l| l.visible) {
            let Some(src) = content(layer, key) else { continue };
            let (ox, oy) = tile_origin(key);
            for (idx, (g, s)) in group.px.iter_mut().zip(&src.px).enumerate() {
                let alpha = s[3] * layer.opacity * base_alpha[idx];
                if alpha <= 0.0 {
                    continue;
                }
                let (x, y) = (ox + (idx % TS) as u32, oy + (idx / TS) as u32);
                let out = composite(layer.blend, *g, [s[0], s[1], s[2]], alpha, x, y);
                // L'écrêtage ne peut jamais élargir la forme de la base.
                *g = [out[0], out[1], out[2], base_alpha[idx]];
            }
        }
        blend_onto(&mut backdrop, &group.px, base.blend, base.opacity, key);
    }
    backdrop
}

/// Aplatit le document en RGBA 8 bits non prémultiplié.
pub fn flatten_rgba8(doc: &Document) -> Vec<u8> {
    let size = doc.size();
    let (w, h) = (size.width as usize, size.height as usize);
    let mut out = vec![0u8; w * h * 4];
    for ty in 0..size.height.div_ceil(TILE_SIZE) {
        for tx in 0..size.width.div_ceil(TILE_SIZE) {
            let key = TileKey { tx, ty };
            // Une tuile que personne ne touche reste transparente : on évite de la composer.
            if doc.layers.iter().all(|l| l.pixels.tile(key).is_none()) {
                continue;
            }
            let tile = composite_tile(doc, key);
            let (x0, y0) = (tx as usize * TS, ty as usize * TS);
            for row in 0..TS.min(h - y0) {
                for col in 0..TS.min(w - x0) {
                    let p = tile[row * TS + col];
                    let o = ((y0 + row) * w + x0 + col) * 4;
                    for c in 0..4 {
                        out[o + c] = (p[c].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{DocId, Mask};
    use crate::geom::Size;
    use crate::tile::TileGrid;

    fn doc(w: u32, h: u32) -> Document {
        Document::new(DocId(1), "t", Size::new(w, h)).unwrap()
    }

    fn solid(w: u32, h: u32, c: [u8; 4]) -> TileGrid {
        TileGrid::from_rgba8(w, h, &c.repeat((w * h) as usize)).unwrap()
    }

    fn px(d: &Document, x: u32, y: u32) -> [u8; 4] {
        let flat = d.flatten_rgba8();
        let w = d.size().width as usize;
        let o = (y as usize * w + x as usize) * 4;
        [flat[o], flat[o + 1], flat[o + 2], flat[o + 3]]
    }

    fn two_layers(blend: BlendMode) -> Document {
        let mut d = doc(4, 4);
        d.layers[0].pixels = solid(4, 4, [51, 128, 204, 255]);
        let id = d.add_layer("haut", solid(4, 4, [153, 77, 230, 255]));
        d.layer_mut(id).unwrap().blend = blend;
        d
    }

    #[test]
    fn multiply_matches_the_reference_function() {
        let d = two_layers(BlendMode::Multiply);
        // R 0,2×0,6 = 0,12 → 31 ; V (128/255)×(77/255) = 38,65 → 39 ; B 0,8×0,9 = 0,72 → 184
        assert_eq!(px(&d, 1, 1), [31, 39, 184, 255]);
    }

    #[test]
    fn every_mode_composites_through_the_document_without_panicking() {
        for mode in BlendMode::ALL {
            let d = two_layers(mode);
            let p = px(&d, 2, 2);
            assert_eq!(p[3], if mode == BlendMode::Dissolve { p[3] } else { 255 }, "{mode:?}");
        }
    }

    #[test]
    fn opacity_and_fill_both_scale_the_layer() {
        let mut d = two_layers(BlendMode::Normal);
        d.layers[1].opacity = 0.5;
        let half = px(&d, 0, 0);
        d.layers[1].opacity = 1.0;
        d.layers[1].fill = 0.5;
        assert_eq!(px(&d, 0, 0), half);
        d.layers[1].fill = 0.0;
        assert_eq!(px(&d, 0, 0), [51, 128, 204, 255]);
    }

    #[test]
    fn hidden_layer_is_skipped() {
        let mut d = two_layers(BlendMode::Normal);
        d.layers[1].visible = false;
        assert_eq!(px(&d, 0, 0), [51, 128, 204, 255]);
    }

    #[test]
    fn mask_hides_and_reveals() {
        let mut d = two_layers(BlendMode::Normal);
        let mut m = TileGrid::new(4, 4).unwrap();
        m.put_pixel(0, 0, [255, 255, 255, 255]);
        m.put_pixel(1, 0, [128, 128, 128, 255]);
        d.layers[1].mask = Some(Mask { pixels: m, enabled: true });
        assert_eq!(px(&d, 0, 0), [153, 77, 230, 255]);
        assert_eq!(px(&d, 3, 3), [51, 128, 204, 255]); // masqué
        let mid = px(&d, 1, 0);
        assert!(mid[0] > 51 && mid[0] < 153);
        d.layers[1].mask.as_mut().unwrap().enabled = false; // masque désactivé : calque entier
        assert_eq!(px(&d, 3, 3), [153, 77, 230, 255]);
    }

    #[test]
    fn clipping_restricts_the_layer_to_the_base_shape() {
        let mut d = doc(4, 1);
        // Base : opaque seulement sur les 2 premiers pixels.
        let mut base = TileGrid::new(4, 1).unwrap();
        base.put_pixel(0, 0, [255, 0, 0, 255]);
        base.put_pixel(1, 0, [255, 0, 0, 255]);
        d.layers[0].pixels = base;
        let id = d.add_layer("écrêté", solid(4, 1, [0, 0, 255, 255]));
        d.layer_mut(id).unwrap().clip_to_below = true;
        assert_eq!(px(&d, 0, 0), [0, 0, 255, 255]);
        assert_eq!(px(&d, 3, 0), [0, 0, 0, 0], "l'écrêtage ne doit pas déborder de la forme de la base");
        // Sans écrêtage, le calque du haut recouvre tout.
        d.layer_mut(id).unwrap().clip_to_below = false;
        assert_eq!(px(&d, 3, 0), [0, 0, 255, 255]);
    }

    #[test]
    fn hidden_base_hides_its_clipped_layers() {
        let mut d = doc(2, 1);
        d.layers[0].pixels = solid(2, 1, [255, 0, 0, 255]);
        let id = d.add_layer("écrêté", solid(2, 1, [0, 0, 255, 255]));
        d.layer_mut(id).unwrap().clip_to_below = true;
        d.layers[0].visible = false;
        assert_eq!(px(&d, 0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn clipped_layer_uses_its_own_blend_mode_inside_the_group() {
        let mut d = doc(1, 1);
        d.layers[0].pixels = solid(1, 1, [255, 255, 255, 255]);
        let id = d.add_layer("écrêté", solid(1, 1, [128, 128, 128, 255]));
        let l = d.layer_mut(id).unwrap();
        l.clip_to_below = true;
        l.blend = BlendMode::Multiply;
        assert_eq!(px(&d, 0, 0), [128, 128, 128, 255]);
    }

    #[test]
    fn composition_is_tile_independent_on_large_documents() {
        // Une photo-test qui traverse plusieurs tuiles doit donner le même résultat que 4 petits calculs.
        let (w, h) = (300, 300);
        let mut d = doc(w, h);
        d.layers[0].pixels = solid(w, h, [200, 100, 50, 255]);
        let id = d.add_layer("haut", solid(w, h, [10, 200, 90, 200]));
        d.layer_mut(id).unwrap().blend = BlendMode::Overlay;
        let reference = px(&d, 0, 0);
        for (x, y) in [(255, 0), (256, 255), (299, 299), (128, 280)] {
            assert_eq!(px(&d, x, y), reference, "({x},{y})");
        }
    }

    #[test]
    fn untouched_tiles_stay_transparent_and_empty_document_flattens() {
        let d = doc(600, 600);
        assert!(d.flatten_rgba8().iter().all(|&b| b == 0));
    }
}
