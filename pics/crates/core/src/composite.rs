//! Compositeur de référence (CPU) : calques, modes de fusion, opacité, remplissage, masques, écrêtage.
//!
//! Il travaille tuile par tuile en flottant, ce qui borne la mémoire à une tuile même sur un
//! document de 100 Mpx. C'est la vérité de référence contre laquelle les shaders GPU sont comparés.

use crate::adjust::Prepared;
use crate::blend::{BlendMode, composite};
use crate::document::{Document, Layer};
use crate::styles::{LayerStyle, Region, effect_alpha, gradient_at};
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

/// Vrai si la tuile peut contenir quelque chose : un calque y a des pixels, ou un calque à styles a des
/// pixels assez près pour que son ombre, sa lueur ou son contour y débordent.
pub fn tile_has_content(doc: &Document, key: TileKey) -> bool {
    doc.layers.iter().any(|l| {
        if l.pixels.tile(key).is_some() {
            return true;
        }
        if l.styles.is_empty() {
            return false;
        }
        let reach = l.styles.iter().map(LayerStyle::reach).max().unwrap_or(0);
        let tiles = reach.div_ceil(TILE_SIZE) as i64;
        (-tiles..=tiles).any(|dy| {
            (-tiles..=tiles).any(|dx| {
                let (tx, ty) = (i64::from(key.tx) + dx, i64::from(key.ty) + dy);
                tx >= 0 && ty >= 0 && l.pixels.tile(TileKey { tx: tx as u32, ty: ty as u32 }).is_some()
            })
        })
    })
}

/// Forme du calque (alpha × masque, sans remplissage) sur un rectangle de coordonnées de document.
fn read_shape(layer: &Layer, x0: i64, y0: i64, side: usize) -> Region {
    let mut data = Vec::with_capacity(side * side);
    for dy in 0..side as i64 {
        for dx in 0..side as i64 {
            let (x, y) = (x0 + dx, y0 + dy);
            let v = if x < 0 || y < 0 || x >= i64::from(layer.pixels.width()) || y >= i64::from(layer.pixels.height()) {
                0.0
            } else {
                let (ux, uy) = (x as u32, y as u32);
                let a = f32::from(layer.pixels.get_pixel(ux, uy)[3]) / 255.0;
                let m = layer
                    .mask
                    .as_ref()
                    .filter(|m| m.enabled)
                    .map_or(1.0, |m| f32::from(m.pixels.get_pixel(ux, uy)[0]) / 255.0);
                a * m
            };
            data.push(v);
        }
    }
    Region { x0, y0, w: side, h: side, data }
}

/// Entoure le contenu d'une tuile de ses styles : ombre et lueur dessous, contour et incrustations dessus.
fn apply_styles(doc: &Document, layer: &Layer, key: TileKey, content: &[Px]) -> Vec<Px> {
    let halo = i64::from(layer.styles.iter().map(LayerStyle::reach).max().unwrap_or(0));
    let (ox, oy) = tile_origin(key);
    let side = TS + 2 * halo as usize;
    let shape = read_shape(layer, i64::from(ox) - halo, i64::from(oy) - halo, side);
    let size = doc.size();
    let mut out = vec![[0.0; 4]; N];
    let rgb = |c: [u8; 3]| [f32::from(c[0]) / 255.0, f32::from(c[1]) / 255.0, f32::from(c[2]) / 255.0];
    let pos = |i: usize| (ox + (i % TS) as u32, oy + (i / TS) as u32);
    let region_alpha = |eff: &Region, i: usize| eff.at(i64::from(pos(i).0), i64::from(pos(i).1));

    for style in layer.styles.iter().filter(|s| s.is_under()) {
        let eff = effect_alpha(style, &shape);
        let color = match style {
            LayerStyle::DropShadow { color, .. } | LayerStyle::OuterGlow { color, .. } => rgb(*color),
            _ => continue,
        };
        for (i, o) in out.iter_mut().enumerate() {
            let a = region_alpha(&eff, i);
            if a > 0.0 {
                let (x, y) = pos(i);
                *o = composite(BlendMode::Normal, *o, color, a, x, y);
            }
        }
    }
    for (i, (o, c)) in out.iter_mut().zip(content).enumerate() {
        if c[3] > 0.0 {
            let (x, y) = pos(i);
            *o = composite(BlendMode::Normal, *o, [c[0], c[1], c[2]], c[3], x, y);
        }
    }
    for style in layer.styles.iter().filter(|s| !s.is_under()) {
        let eff = effect_alpha(style, &shape);
        for (i, o) in out.iter_mut().enumerate() {
            let a = region_alpha(&eff, i);
            if a <= 0.0 {
                continue;
            }
            let (x, y) = pos(i);
            let (color, mode) = match style {
                LayerStyle::Stroke { color, .. } => (rgb(*color), BlendMode::Normal),
                LayerStyle::ColorOverlay { color, blend, .. } => (rgb(*color), *blend),
                LayerStyle::GradientOverlay { from, to, angle_deg, blend, .. } => (
                    gradient_at(
                        *from,
                        *to,
                        *angle_deg,
                        f64::from(x),
                        f64::from(y),
                        f64::from(size.width),
                        f64::from(size.height),
                    ),
                    *blend,
                ),
                _ => continue,
            };
            *o = composite(mode, *o, color, a, x, y);
        }
    }
    out
}

/// Contenu d'un calque pour une tuile : ses pixels, ou pour un groupe isolé le résultat de ses enfants.
fn layer_content(doc: &Document, layer: &Layer, key: TileKey) -> Option<Content> {
    if !layer.is_group() {
        return content(layer, key);
    }
    let mut buf = vec![[0.0; 4]; N];
    compose_into(doc, Some(layer.id), key, &mut buf);
    let mask = read_mask(layer, key);
    for (i, p) in buf.iter_mut().enumerate() {
        p[3] *= layer.fill * mask.as_ref().map_or(1.0, |m| m[i]);
    }
    Some(Content { px: buf })
}

/// Composition d'une tuile : un calque de base et les calques écrêtés qui le suivent forment un groupe
/// d'écrêtage, composé à part puis fusionné avec le mode et l'opacité de la base. Un groupe est composé
/// récursivement : isolé (ses enfants forment d'abord une image, fusionnée ensuite comme un calque) ou
/// « traverser » (ses enfants se fusionnent directement avec ce qui est dessous).
pub fn composite_tile(doc: &Document, key: TileKey) -> Vec<Px> {
    let mut backdrop = vec![[0.0; 4]; N];
    compose_into(doc, None, key, &mut backdrop);
    backdrop
}

/// Applique un calque de réglage à `target` : `couleur ← mélange(couleur, réglage(couleur), masque × opacité × limite)`,
/// seulement là où il y a de la matière (alpha > 0) ; l'alpha n'est jamais modifié.
fn apply_adjustment(layer: &Layer, key: TileKey, target: &mut [Px], limit: Option<&[f32]>) {
    let Some(adj) = layer.adjustment() else { return };
    let prepared: Prepared = adj.prepare();
    let mask = read_mask(layer, key);
    for (i, p) in target.iter_mut().enumerate() {
        let amount = layer.opacity * layer.fill * mask.as_ref().map_or(1.0, |m| m[i]) * limit.map_or(1.0, |l| l[i]);
        if p[3] <= 0.0 || amount <= 0.0 {
            continue;
        }
        let adjusted = prepared.apply([p[0], p[1], p[2]]);
        for c in 0..3 {
            p[c] += (adjusted[c] - p[c]) * amount.min(1.0);
        }
    }
}

fn compose_into(doc: &Document, parent: Option<crate::LayerId>, key: TileKey, backdrop: &mut [Px]) {
    let layers: Vec<&Layer> = doc.layers.iter().filter(|l| l.group == parent).collect();
    let mut i = 0;
    while i < layers.len() {
        let base = layers[i];
        let mut end = i + 1;
        while end < layers.len() && layers[end].clip_to_below {
            end += 1;
        }
        let clipped = &layers[i + 1..end];
        i = end;
        if !base.visible {
            continue; // un calque caché cache aussi ce qui lui est écrêté
        }
        if base.adjustment().is_some() {
            apply_adjustment(base, key, backdrop, None);
            continue;
        }
        if matches!(base.kind, crate::LayerKind::Group { isolated: false })
            && base.blend == BlendMode::Normal
            && base.opacity >= 1.0
            && base.fill >= 1.0
            && base.mask.is_none()
            && clipped.is_empty()
        {
            compose_into(doc, Some(base.id), key, backdrop);
            continue;
        }
        let styled = !base.styles.is_empty() && !base.is_group();
        let Some(mut group) =
            layer_content(doc, base, key).or_else(|| styled.then(|| Content { px: vec![[0.0; 4]; N] }))
        else {
            continue;
        };
        let base_alpha: Vec<f32> = group.px.iter().map(|p| p[3]).collect();
        for layer in clipped.iter().filter(|l| l.visible) {
            if layer.adjustment().is_some() {
                // Réglage écrêté : il ne touche que le calque de base, dans sa forme.
                apply_adjustment(layer, key, &mut group.px, Some(&base_alpha));
                continue;
            }
            let Some(src) = layer_content(doc, layer, key) else { continue };
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
        if styled {
            group.px = apply_styles(doc, base, key, &group.px);
        }
        blend_onto(backdrop, &group.px, base.blend, base.opacity, key);
    }
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
            if !tile_has_content(doc, key) {
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

    use crate::styles::StrokePosition;

    /// Calque 64×64 avec un carré rouge opaque de 10 px en (20, 20).
    fn square_doc() -> (Document, crate::LayerId) {
        let mut d = doc(64, 64);
        let id = d.layers[0].id;
        for y in 20..30 {
            for x in 20..30 {
                d.layers[0].pixels.put_pixel(x, y, [255, 0, 0, 255]);
            }
        }
        (d, id)
    }

    #[test]
    fn drop_shadow_appears_beside_the_shape_and_under_it() {
        let (mut d, id) = square_doc();
        d.layer_mut(id).unwrap().styles =
            vec![LayerStyle::DropShadow { color: [0, 0, 255], opacity: 1.0, angle_deg: 0.0, distance: 8.0, size: 0.0 }];
        assert_eq!(px(&d, 33, 25), [0, 0, 255, 255], "ombre à droite");
        assert_eq!(px(&d, 25, 25), [255, 0, 0, 255], "le contenu reste au-dessus de l'ombre");
        assert_eq!(px(&d, 15, 25), [0, 0, 0, 0]);
    }

    #[test]
    fn shadow_spills_into_tiles_where_the_layer_has_no_pixels() {
        let mut d = doc(300, 40);
        d.layers[0].pixels.put_pixel(250, 20, [255, 255, 255, 255]);
        let id = d.layers[0].id;
        d.layer_mut(id).unwrap().styles =
            vec![LayerStyle::DropShadow { color: [0, 0, 0], opacity: 1.0, angle_deg: 0.0, distance: 20.0, size: 0.0 }];
        // La tuile x∈[256,300) n'a aucun pixel de calque mais reçoit l'ombre de (250,20) → (270,20).
        assert_eq!(px(&d, 270, 20), [0, 0, 0, 255]);
    }

    #[test]
    fn strokes_and_overlays_follow_the_shape() {
        let (mut d, id) = square_doc();
        d.layer_mut(id).unwrap().styles =
            vec![LayerStyle::Stroke { color: [0, 255, 0], opacity: 1.0, size: 2.0, position: StrokePosition::Outside }];
        assert_eq!(px(&d, 19, 25), [0, 255, 0, 255]);
        assert_eq!(px(&d, 25, 25), [255, 0, 0, 255]);
        d.layer_mut(id).unwrap().styles =
            vec![LayerStyle::ColorOverlay { color: [0, 0, 255], opacity: 1.0, blend: BlendMode::Normal }];
        assert_eq!(px(&d, 25, 25), [0, 0, 255, 255]);
        assert_eq!(px(&d, 5, 5), [0, 0, 0, 0], "l'incrustation ne déborde pas de la forme");
        d.layer_mut(id).unwrap().styles = vec![LayerStyle::GradientOverlay {
            from: [0, 0, 0],
            to: [255, 255, 255],
            angle_deg: 0.0,
            opacity: 1.0,
            blend: BlendMode::Normal,
        }];
        assert!(px(&d, 21, 25)[0] < px(&d, 28, 25)[0], "le dégradé progresse de gauche à droite");
    }

    #[test]
    fn fill_hides_the_content_but_not_the_styles_while_opacity_hides_everything() {
        let (mut d, id) = square_doc();
        d.layer_mut(id).unwrap().styles =
            vec![LayerStyle::Stroke { color: [0, 255, 0], opacity: 1.0, size: 2.0, position: StrokePosition::Outside }];
        d.layer_mut(id).unwrap().fill = 0.0;
        assert_eq!(px(&d, 25, 25), [0, 0, 0, 0], "contenu masqué par le remplissage");
        assert_eq!(px(&d, 19, 25), [0, 255, 0, 255], "contour conservé");
        d.layer_mut(id).unwrap().opacity = 0.0;
        assert_eq!(px(&d, 19, 25), [0, 0, 0, 0], "l'opacité masque aussi les styles");
    }

    #[test]
    fn styles_respect_the_layer_mask() {
        let (mut d, id) = square_doc();
        let mut m = TileGrid::new(64, 64).unwrap();
        for y in 20..30 {
            for x in 20..25 {
                m.put_pixel(x, y, [255; 4]);
            }
        }
        d.layer_mut(id).unwrap().mask = Some(Mask { pixels: m, enabled: true });
        d.layer_mut(id).unwrap().styles =
            vec![LayerStyle::ColorOverlay { color: [0, 0, 255], opacity: 1.0, blend: BlendMode::Normal }];
        assert_eq!(px(&d, 22, 25), [0, 0, 255, 255]);
        assert_eq!(px(&d, 27, 25), [0, 0, 0, 0]);
    }

    #[test]
    fn clipped_layers_stay_inside_the_base_shape_even_with_a_shadow() {
        let (mut d, id) = square_doc();
        d.layer_mut(id).unwrap().styles =
            vec![LayerStyle::DropShadow { color: [0, 0, 0], opacity: 1.0, angle_deg: 0.0, distance: 8.0, size: 0.0 }];
        let top = d.add_layer("écrêté", solid(64, 64, [0, 0, 255, 255]));
        d.layer_mut(top).unwrap().clip_to_below = true;
        assert_eq!(px(&d, 25, 25), [0, 0, 255, 255]);
        assert_eq!(px(&d, 33, 25), [0, 0, 0, 255], "l'ombre n'est pas recouverte par le calque écrêté");
    }

    fn group_doc() -> (Document, crate::LayerId, crate::LayerId, crate::LayerId) {
        // Fond blanc ; un groupe contenant deux calques : rouge (bas) et bleu (haut), chacun sur un pixel.
        let mut d = doc(8, 1);
        d.layers[0].pixels = solid(8, 1, [255, 255, 255, 255]);
        let g = crate::LayerId(d.alloc_id());
        d.layers.push(Layer::new_group(g, "Groupe", TileGrid::new(8, 1).unwrap()));
        let mut red = TileGrid::new(8, 1).unwrap();
        red.put_pixel(2, 0, [255, 0, 0, 255]);
        let r = d.add_layer("rouge", red);
        let mut blue = TileGrid::new(8, 1).unwrap();
        blue.put_pixel(2, 0, [0, 0, 255, 255]);
        blue.put_pixel(3, 0, [0, 0, 255, 255]);
        let b = d.add_layer("bleu", blue);
        d.layer_mut(r).unwrap().group = Some(g);
        d.layer_mut(b).unwrap().group = Some(g);
        (d, g, r, b)
    }

    #[test]
    fn group_children_composite_like_a_flat_stack_when_isolated_and_normal() {
        let (d, ..) = group_doc();
        assert_eq!(px(&d, 2, 0), [0, 0, 255, 255], "le calque bleu est au-dessus du rouge dans le groupe");
        assert_eq!(px(&d, 3, 0), [0, 0, 255, 255]);
        assert_eq!(px(&d, 5, 0), [255, 255, 255, 255]);
    }

    #[test]
    fn group_opacity_applies_to_the_whole_group_once() {
        let (mut d, g, ..) = group_doc();
        d.layer_mut(g).unwrap().opacity = 0.5;
        // Pixel 2 : rouge recouvert par le bleu DANS le groupe, puis le groupe à 50 % sur blanc.
        // Appliquer 50 % à chaque enfant laisserait transparaître du rouge ; l'isolation l'interdit.
        let p = px(&d, 2, 0);
        assert!(p[0] > 120 && p[0] < 135 && p[2] == 255, "{p:?}");
    }

    #[test]
    fn multiply_inside_an_isolated_group_does_not_reach_the_backdrop_but_pass_through_does() {
        let (mut d, g, r, _) = group_doc();
        d.layer_mut(r).unwrap().blend = BlendMode::Multiply;
        d.layer_mut(r).unwrap().visible = true;
        // Isolé : le rouge « multiplie » le vide du groupe → il reste rouge opaque (puis recouvert par le bleu).
        d.layers.iter_mut().find(|l| l.name == "bleu").unwrap().visible = false;
        assert_eq!(px(&d, 2, 0), [255, 0, 0, 255]);
        // Traverser : le rouge multiplie le fond blanc, le résultat est le même ici ; avec un fond gris la différence se voit.
        d.layers[0].pixels = solid(8, 1, [128, 128, 128, 255]);
        let isolated = px(&d, 2, 0);
        d.layer_mut(g).unwrap().kind = crate::LayerKind::Group { isolated: false };
        let pass = px(&d, 2, 0);
        assert_eq!(isolated, [255, 0, 0, 255]);
        assert_eq!(pass, [128, 0, 0, 255], "multiplier le gris par le rouge");
    }

    #[test]
    fn hidden_group_hides_its_children_and_group_mask_applies() {
        let (mut d, g, ..) = group_doc();
        d.layer_mut(g).unwrap().visible = false;
        assert_eq!(px(&d, 2, 0), [255, 255, 255, 255]);
        d.layer_mut(g).unwrap().visible = true;
        let mut m = TileGrid::new(8, 1).unwrap();
        m.put_pixel(3, 0, [255; 4]);
        d.layer_mut(g).unwrap().mask = Some(Mask { pixels: m, enabled: true });
        assert_eq!(px(&d, 2, 0), [255, 255, 255, 255], "masqué par le masque du groupe");
        assert_eq!(px(&d, 3, 0), [0, 0, 255, 255]);
    }

    #[test]
    fn nested_groups_compose_and_the_tree_order_lists_children_after_their_group() {
        let (mut d, g, _, b) = group_doc();
        let inner = crate::LayerId(d.alloc_id());
        d.layers.push(Layer::new_group(inner, "Interne", TileGrid::new(8, 1).unwrap()));
        d.layer_mut(inner).unwrap().group = Some(g);
        d.layer_mut(b).unwrap().group = Some(inner);
        assert_eq!(px(&d, 3, 0), [0, 0, 255, 255]);
        let order = d.tree_top_first();
        let pos = |id| order.iter().position(|(l, _)| *l == id).unwrap();
        assert!(pos(g) < pos(inner) && pos(inner) < pos(b));
        assert_eq!(d.depth(b), 2);
        assert!(d.is_ancestor_or_self(g, b) && !d.is_ancestor_or_self(b, g));
    }

    fn adj_doc(kind: crate::adjust::AdjKind, set: &[(&str, f32)]) -> (Document, crate::LayerId) {
        let mut d = doc(4, 1);
        d.layers[0].pixels = solid(4, 1, [128, 64, 192, 255]);
        let mut a = crate::adjust::Adjustment::new(kind);
        for (id, v) in set {
            a.set(id, *v);
        }
        let id = crate::LayerId(d.alloc_id());
        d.layers.push(Layer::new_adjustment(id, "Réglage", TileGrid::new(4, 1).unwrap(), a));
        (d, id)
    }

    #[test]
    fn an_adjustment_layer_changes_what_is_below_without_touching_pixels() {
        let (mut d, id) = adj_doc(crate::adjust::AdjKind::Invert, &[]);
        assert_eq!(px(&d, 1, 0), [127, 191, 63, 255]);
        assert_eq!(d.layers[0].pixels.get_pixel(1, 0), [128, 64, 192, 255], "les pixels du dessous sont intacts");
        d.layer_mut(id).unwrap().visible = false;
        assert_eq!(px(&d, 1, 0), [128, 64, 192, 255], "non destructif : masquer le réglage restitue l'original");
    }

    #[test]
    fn adjustment_opacity_mask_and_transparent_pixels() {
        let (mut d, id) = adj_doc(crate::adjust::AdjKind::Invert, &[]);
        d.layer_mut(id).unwrap().opacity = 0.5;
        let half = px(&d, 0, 0);
        assert!((i32::from(half[0]) - 128).abs() <= 1, "50 % d'un négatif sur 128 : {half:?}");
        d.layer_mut(id).unwrap().opacity = 1.0;
        let mut m = TileGrid::new(4, 1).unwrap();
        m.put_pixel(0, 0, [255; 4]);
        d.layer_mut(id).unwrap().mask = Some(Mask { pixels: m, enabled: true });
        assert_eq!(px(&d, 0, 0), [127, 191, 63, 255]);
        assert_eq!(px(&d, 3, 0), [128, 64, 192, 255], "masqué : pas de réglage");
        // Un fond transparent n'est pas coloré par un réglage.
        let mut e = doc(2, 1);
        let mut a = crate::adjust::Adjustment::new(crate::adjust::AdjKind::Exposure);
        a.set("offset", 0.5);
        let id = crate::LayerId(e.alloc_id());
        e.layers.push(Layer::new_adjustment(id, "R", TileGrid::new(2, 1).unwrap(), a));
        assert_eq!(px(&e, 0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn a_clipped_adjustment_only_affects_the_layer_below_inside_its_shape() {
        let mut d = doc(4, 1);
        d.layers[0].pixels = solid(4, 1, [255, 255, 255, 255]);
        let mut shape = TileGrid::new(4, 1).unwrap();
        shape.put_pixel(1, 0, [100, 100, 100, 255]);
        let top = d.add_layer("forme", shape);
        let mut a = crate::adjust::Adjustment::new(crate::adjust::AdjKind::Invert);
        a.set("x", 0.0);
        let id = crate::LayerId(d.alloc_id());
        let mut adj = Layer::new_adjustment(id, "Négatif écrêté", TileGrid::new(4, 1).unwrap(), a);
        adj.clip_to_below = true;
        d.layers.push(adj);
        let _ = top;
        assert_eq!(px(&d, 1, 0), [155, 155, 155, 255], "la forme est inversée");
        assert_eq!(px(&d, 0, 0), [255, 255, 255, 255], "le fond ne l'est pas");
    }

    #[test]
    fn adjustments_inside_an_isolated_group_do_not_reach_the_backdrop() {
        let mut d = doc(2, 1);
        d.layers[0].pixels = solid(2, 1, [100, 100, 100, 255]);
        let g = crate::LayerId(d.alloc_id());
        d.layers.push(Layer::new_group(g, "G", TileGrid::new(2, 1).unwrap()));
        let a = crate::adjust::Adjustment::new(crate::adjust::AdjKind::Invert);
        let id = crate::LayerId(d.alloc_id());
        let mut adj = Layer::new_adjustment(id, "N", TileGrid::new(2, 1).unwrap(), a);
        adj.group = Some(g);
        d.layers.push(adj);
        assert_eq!(px(&d, 0, 0), [100, 100, 100, 255], "isolé : le réglage ne voit que le contenu du groupe (vide)");
        d.layer_mut(g).unwrap().kind = crate::LayerKind::Group { isolated: false };
        assert_eq!(px(&d, 0, 0), [155, 155, 155, 255], "traverser : le réglage agit sur le fond");
    }
}
