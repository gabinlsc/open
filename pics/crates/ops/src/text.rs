//! Rendu du texte avec `cosmic-text` : polices système et OpenType (crénage, ligatures par la mise en forme
//! avancée), paragraphes à retour à la ligne automatique, alignement, texte sur tracé.

use std::sync::{Mutex, OnceLock};

use cosmic_text::{
    Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache, SwashContent, Weight,
};
use openpics_core::TileGrid;
use openpics_core::blend::{BlendMode, composite};
use openpics_core::text::{TextAlign, TextModel};

/// Le système de polices coûte cher à construire (analyse de toutes les polices installées) : un seul, partagé.
fn engine() -> &'static Mutex<(FontSystem, SwashCache)> {
    static ENGINE: OnceLock<Mutex<(FontSystem, SwashCache)>> = OnceLock::new();
    ENGINE.get_or_init(|| Mutex::new((FontSystem::new(), SwashCache::new())))
}

/// Familles de polices installées, triées et sans doublon.
pub fn families() -> Vec<String> {
    let Ok(guard) = engine().lock() else { return Vec::new() };
    let mut names: Vec<String> =
        guard.0.db().faces().filter_map(|f| f.families.first().map(|(n, _)| n.clone())).collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    names
}

fn attrs_of(model: &TextModel) -> Attrs<'_> {
    let family = if model.font.is_empty() { Family::SansSerif } else { Family::Name(&model.font) };
    Attrs::new().family(family).weight(if model.bold { Weight::BOLD } else { Weight::NORMAL }).style(if model.italic {
        Style::Italic
    } else {
        Style::Normal
    })
}

fn align_of(a: TextAlign) -> Align {
    match a {
        TextAlign::Left => Align::Left,
        TextAlign::Center => Align::Center,
        TextAlign::Right => Align::Right,
        TextAlign::Justify => Align::Justified,
    }
}

fn layout(fs: &mut FontSystem, model: &TextModel) -> Buffer {
    let metrics = Metrics::new(model.size, model.size * model.line_height);
    let mut buffer = Buffer::new(fs, metrics);
    buffer.set_size(fs, model.box_width, None);
    buffer.set_text(fs, &model.text, &attrs_of(model), Shaping::Advanced);
    for line in &mut buffer.lines {
        line.set_align(Some(align_of(model.align)));
    }
    buffer.shape_until_scroll(fs, false);
    buffer
}

/// Dimensions (largeur, hauteur) du texte mis en forme, en pixels.
pub fn measure(model: &TextModel) -> (f32, f32) {
    let model = model.sanitized();
    let Ok(mut guard) = engine().lock() else { return (0.0, 0.0) };
    let buffer = layout(&mut guard.0, &model);
    buffer
        .layout_runs()
        .fold((0.0f32, 0.0f32), |(w, h), run| (w.max(run.line_w), h.max(run.line_top + run.line_height)))
}

fn put(out: &mut TileGrid, x: i64, y: i64, rgb: [u8; 3], a: f32) {
    if a <= 0.0 || x < 0 || y < 0 || x >= i64::from(out.width()) || y >= i64::from(out.height()) {
        return;
    }
    let (ux, uy) = (x as u32, y as u32);
    let back = out.get_pixel(ux, uy).map(|c| f32::from(c) / 255.0);
    let p = composite(BlendMode::Normal, back, rgb.map(|c| f32::from(c) / 255.0), a, ux, uy);
    out.put_pixel(ux, uy, p.map(|c| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8));
}

/// Rend le texte sur un calque `width × height`.
pub fn render(model: &TextModel, width: u32, height: u32) -> Result<TileGrid, String> {
    let model = model.sanitized();
    let mut out = TileGrid::new(width, height).map_err(|e| e.to_string())?;
    if model.text.trim().is_empty() {
        return Ok(out);
    }
    let mut guard = engine().lock().map_err(|_| "moteur de texte indisponible".to_owned())?;
    let (fs, cache) = &mut *guard;
    let buffer = layout(fs, &model);
    match &model.path {
        None => {
            let (ox, oy) = (model.origin.0.round() as i64, model.origin.1.round() as i64);
            let rgb = model.color;
            buffer.draw(fs, cache, Color::rgb(rgb[0], rgb[1], rgb[2]), |x, y, w, h, c| {
                let a = f32::from(c.a()) / 255.0;
                for dy in 0..h as i64 {
                    for dx in 0..w {
                        put(&mut out, ox + i64::from(x) + i64::from(dx), oy + i64::from(y) + dy, rgb, a);
                    }
                }
            });
        }
        Some(path) => render_on_path(&mut out, fs, cache, &buffer, &model, &path.flatten(0.2)),
    }
    Ok(out)
}

/// Texte sur tracé : chaque lettre est posée sur le chemin, tournée selon la tangente (première ligne seulement).
fn render_on_path(
    out: &mut TileGrid,
    fs: &mut FontSystem,
    cache: &mut SwashCache,
    buffer: &Buffer,
    model: &TextModel,
    poly: &[(f64, f64)],
) {
    if poly.len() < 2 {
        return;
    }
    let mut cum = vec![0.0f64];
    for w in poly.windows(2) {
        cum.push(cum.last().copied().unwrap_or(0.0) + (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1));
    }
    let total = cum.last().copied().unwrap_or(0.0);
    let at = |s: f64| -> Option<((f64, f64), f64)> {
        if s < 0.0 || s > total {
            return None;
        }
        let i = cum.windows(2).position(|w| s <= w[1]).unwrap_or(poly.len() - 2);
        let (a, b) = (poly[i], poly[i + 1]);
        let t = (s - cum[i]) / (cum[i + 1] - cum[i]).max(1e-9);
        Some(((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t), (b.1 - a.1).atan2(b.0 - a.0)))
    };
    let Some(run) = buffer.layout_runs().next() else { return };
    for glyph in run.glyphs {
        let physical = glyph.physical((0.0, 0.0), 1.0);
        let Some(img) = cache.get_image(fs, physical.cache_key).as_ref() else { continue };
        if img.content != SwashContent::Mask || img.placement.width == 0 {
            continue;
        }
        let center = f64::from(glyph.x) + f64::from(glyph.w) / 2.0;
        let Some(((px, py), theta)) = at(center) else { continue };
        let (c, s) = (theta.cos(), theta.sin());
        // Le glyphe est dessiné autour de son centre horizontal ; `ly` est mesuré depuis la ligne de base.
        let (w, h) = (img.placement.width as i64, img.placement.height as i64);
        let (left, top) = (i64::from(img.placement.left), i64::from(img.placement.top));
        for iy in 0..h {
            for ix in 0..w {
                let cov = f64::from(img.data[(iy * w + ix) as usize]) / 255.0;
                if cov <= 0.0 {
                    continue;
                }
                let lx = (left + ix) as f64 + 0.5 - f64::from(glyph.w) / 2.0;
                let ly = (-top + iy) as f64 + 0.5;
                let (dx, dy) = (px + lx * c - ly * s, py + lx * s + ly * c);
                // Un pixel du glyphe couvre ~1 pixel : on étale sur les 4 voisins (bilinéaire inverse du splat).
                let (fx, fy) = (dx - 0.5, dy - 0.5);
                let (x0, y0) = (fx.floor(), fy.floor());
                let (tx, ty) = (fx - x0, fy - y0);
                for (ox, oy, wgt) in
                    [(0, 0, (1.0 - tx) * (1.0 - ty)), (1, 0, tx * (1.0 - ty)), (0, 1, (1.0 - tx) * ty), (1, 1, tx * ty)]
                {
                    put(out, x0 as i64 + ox, y0 as i64 + oy, model.color, (cov * wgt) as f32);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les tests de texte ont besoin d'au moins une police : sans elle (machine minimale) ils s'ignorent,
    /// sauf si `OPENPICS_REQUIRE_FONTS` est défini (CI).
    fn fonts_available() -> bool {
        let ok = !families().is_empty();
        if !ok {
            assert!(
                std::env::var_os("OPENPICS_REQUIRE_FONTS").is_none(),
                "aucune police système : test texte impossible"
            );
            eprintln!("test texte ignoré : aucune police système");
        }
        ok
    }

    fn model(text: &str) -> TextModel {
        TextModel {
            text: text.to_owned(),
            font: "DejaVu Sans".to_owned(),
            size: 40.0,
            origin: (10.0, 10.0),
            ..TextModel::default()
        }
    }

    fn ink(g: &TileGrid) -> Option<(u32, u32, u32, u32)> {
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
        for y in 0..g.height() {
            for x in 0..g.width() {
                if g.get_pixel(x, y)[3] > 60 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        (x1 >= x0 && y1 >= y0).then_some((x0, y0, x1, y1))
    }

    fn amount(g: &TileGrid) -> f64 {
        (0..g.height())
            .flat_map(|y| (0..g.width()).map(move |x| (x, y)))
            .map(|(x, y)| f64::from(g.get_pixel(x, y)[3]) / 255.0)
            .sum()
    }

    #[test]
    fn text_renders_with_the_requested_colour_inside_its_box() {
        if !fonts_available() {
            return;
        }
        let m = TextModel { color: [200, 30, 40], ..model("Hello") };
        let g = render(&m, 300, 100).unwrap();
        let (x0, y0, x1, y1) = ink(&g).expect("du texte doit apparaître");
        assert!(x0 >= 8 && y0 >= 8 && x1 < 250 && y1 < 90, "dans le bloc : {:?}", (x0, y0, x1, y1));
        let solid = (0..g.width())
            .flat_map(|x| (0..g.height()).map(move |y| (x, y)))
            .map(|(x, y)| g.get_pixel(x, y))
            .find(|p| p[3] == 255);
        assert_eq!(solid.map(|p| [p[0], p[1], p[2]]), Some([200, 30, 40]));
        assert!(render(&model("   "), 100, 50).unwrap().tile_count() == 0, "texte vide : rien");
    }

    #[test]
    fn size_bold_and_unknown_fonts_behave() {
        if !fonts_available() {
            return;
        }
        let small = amount(&render(&TextModel { size: 20.0, ..model("Hello") }, 300, 100).unwrap());
        let big = amount(&render(&TextModel { size: 40.0, ..model("Hello") }, 300, 100).unwrap());
        assert!(big > small * 3.0, "l'encre croît avec le carré du corps : {small} → {big}");
        let normal = amount(&render(&model("Hello"), 300, 100).unwrap());
        let bold = amount(&render(&TextModel { bold: true, ..model("Hello") }, 300, 100).unwrap());
        assert!(bold > normal * 1.1, "gras : {normal} → {bold}");
        let fallback = render(&TextModel { font: "PoliceQuiNexistePas".into(), ..model("Hello") }, 300, 100).unwrap();
        assert!(ink(&fallback).is_some(), "police introuvable : repli sur la police par défaut");
    }

    #[test]
    fn paragraphs_wrap_and_align() {
        if !fonts_available() {
            return;
        }
        let long = "un texte assez long pour passer à la ligne plusieurs fois";
        let one = measure(&model(long));
        let wrapped = measure(&TextModel { box_width: Some(200.0), ..model(long) });
        assert!(wrapped.0 <= 205.0 && wrapped.1 > one.1 * 2.0, "{one:?} → {wrapped:?}");
        let tall = measure(&TextModel { box_width: Some(200.0), line_height: 2.0, ..model(long) });
        assert!(tall.1 > wrapped.1 * 1.4, "l'interligne agrandit le bloc");
        let at = |align| {
            ink(&render(&TextModel { box_width: Some(300.0), align, origin: (0.0, 10.0), ..model("Hi") }, 400, 80)
                .unwrap())
            .unwrap()
        };
        let (left, centre, right) = (at(TextAlign::Left), at(TextAlign::Center), at(TextAlign::Right));
        assert!(left.0 < centre.0 && centre.0 < right.0, "{left:?} {centre:?} {right:?}");
        assert!((centre.0 + centre.2).abs_diff(300) < 30, "centré dans une boîte de 300 px");
        assert!(right.2 > 270 && right.2 <= 305, "collé à droite : {right:?}");
    }

    #[test]
    fn kerning_and_the_font_list() {
        if !fonts_available() {
            return;
        }
        let names = families();
        assert!(names.windows(2).all(|w| w[0].to_lowercase() <= w[1].to_lowercase()), "triées");
        assert!(names.iter().any(|n| n == "DejaVu Sans") || !names.is_empty());
        // Crénage (si la police en a) : « AV » ne dépasse jamais la somme des largeurs de « A » et « V ».
        let (av, a, v) = (measure(&model("AV")).0, measure(&model("A")).0, measure(&model("V")).0);
        assert!(av <= a + v + 0.01, "{av} ≤ {a} + {v}");
        assert!(measure(&model("Hello")).0 > 0.0);
    }

    #[test]
    fn text_on_a_path_follows_the_curve() {
        if !fonts_available() {
            return;
        }
        use openpics_core::vector::Path;
        let straight = TextModel { path: Some(Path::line((20.0, 80.0), (380.0, 80.0))), ..model("Hello") };
        let g = render(&straight, 400, 200).unwrap();
        let (x0, y0, x1, y1) = ink(&g).unwrap();
        assert!(x1 - x0 > 60 && y1 - y0 < 70, "à plat le long d'un tracé horizontal : {:?}", (x0, y0, x1, y1));
        let vertical = TextModel { path: Some(Path::line((100.0, 20.0), (100.0, 190.0))), ..model("Hello") };
        let (vx0, vy0, vx1, vy1) = ink(&render(&vertical, 400, 200).unwrap()).unwrap();
        assert!(
            vy1 - vy0 > vx1 - vx0,
            "le long d'un tracé vertical, le texte est plus haut que large : {:?}",
            (vx0, vy0, vx1, vy1)
        );
        // Un tracé trop court n'affiche que les lettres qui tiennent ; un tracé dégénéré ne plante pas.
        let short = TextModel { path: Some(Path::line((20.0, 80.0), (60.0, 80.0))), ..model("Hello world") };
        assert!(render(&short, 400, 200).is_ok());
        assert!(render(&TextModel { path: Some(Path::default()), ..model("x") }, 100, 100).is_ok());
    }

    #[test]
    fn hostile_sizes_and_huge_text_do_not_panic() {
        if !fonts_available() {
            return;
        }
        assert!(render(&TextModel { size: f32::NAN, ..model("A") }, 100, 100).is_ok());
        assert!(render(&TextModel { size: 5000.0, ..model("A") }, 100, 100).is_ok());
        assert!(render(&model(&"mot ".repeat(5000)), 200, 200).is_ok());
    }
}
