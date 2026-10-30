//! Rendu vectoriel : remplissage et contour anticrénelés, opérations booléennes, chemin → sélection, import SVG.

use i_overlay::core::fill_rule::FillRule as OFillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use openpics_core::blend::{BlendMode, composite};
use openpics_core::vector::{Anchor, FillRule, Path, Pt, VectorModel};
use openpics_core::{CoreError, TileGrid};

/// Tolérance d'aplatissement des courbes, en pixels.
const FLATNESS: f64 = 0.15;
const SUBSAMPLES: usize = 4;

/// Couverture (0..1) d'une zone rectangulaire du document.
pub struct Coverage {
    pub x0: u32,
    pub y0: u32,
    pub w: usize,
    pub h: usize,
    pub data: Vec<f32>,
}

fn contours_of(model: &VectorModel, closed_only: bool) -> Vec<Vec<Pt>> {
    model
        .paths
        .iter()
        .filter(|p| !closed_only || p.closed)
        .map(|p| p.flatten(FLATNESS))
        .filter(|c| c.len() >= if closed_only { 3 } else { 2 })
        .collect()
}

fn clip_box(contours: &[Vec<Pt>], width: u32, height: u32, margin: f64) -> Option<(u32, u32, u32, u32)> {
    let pts = contours.iter().flatten();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in pts {
        x0 = x0.min(p.0);
        y0 = y0.min(p.1);
        x1 = x1.max(p.0);
        y1 = y1.max(p.1);
    }
    if !x0.is_finite() {
        return None;
    }
    let (bx0, by0) = ((x0 - margin).floor().max(0.0) as u32, (y0 - margin).floor().max(0.0) as u32);
    let (bx1, by1) = (
        ((x1 + margin).ceil().min(f64::from(width))).max(0.0) as u32,
        ((y1 + margin).ceil().min(f64::from(height))).max(0.0) as u32,
    );
    (bx1 > bx0 && by1 > by0).then_some((bx0, by0, bx1, by1))
}

/// Remplissage anticrénélé de contours fermés avec la règle donnée (4 sous-lignes par pixel, couverture horizontale exacte).
pub fn fill_coverage(contours: &[Vec<Pt>], rule: FillRule, width: u32, height: u32) -> Option<Coverage> {
    let (x0, y0, x1, y1) = clip_box(contours, width, height, 0.0)?;
    let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let mut cov = vec![0.0f32; w * h];
    for row in 0..h {
        for sub in 0..SUBSAMPLES {
            let y = f64::from(y0) + row as f64 + (sub as f64 + 0.5) / SUBSAMPLES as f64;
            let mut crossings: Vec<(f64, i32)> = Vec::new();
            for c in contours {
                let n = c.len();
                for i in 0..n {
                    let (p, q) = (c[i], c[(i + 1) % n]);
                    if (p.1 <= y) != (q.1 <= y) {
                        crossings.push((p.0 + (y - p.1) / (q.1 - p.1) * (q.0 - p.0), if q.1 > p.1 { 1 } else { -1 }));
                    }
                }
            }
            crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut winding = 0;
            for k in 0..crossings.len().saturating_sub(1) {
                winding += crossings[k].1;
                let inside = match rule {
                    FillRule::NonZero => winding != 0,
                    FillRule::EvenOdd => (k + 1) % 2 == 1,
                };
                if !inside {
                    continue;
                }
                let (a, b) =
                    ((crossings[k].0 - f64::from(x0)).max(0.0), (crossings[k + 1].0 - f64::from(x0)).min(w as f64));
                if b <= a {
                    continue;
                }
                for col in a.floor() as usize..(b.ceil() as usize).min(w) {
                    let overlap = (b.min(col as f64 + 1.0) - a.max(col as f64)).max(0.0);
                    cov[row * w + col] += (overlap / SUBSAMPLES as f64) as f32;
                }
            }
        }
    }
    for v in &mut cov {
        *v = v.clamp(0.0, 1.0);
    }
    Some(Coverage { x0, y0, w, h, data: cov })
}

/// Contour : couverture des pixels à moins de `width / 2` d'un segment (distance anticrénelée sur 1 px).
/// Les jonctions sont rondes ; les extrémités des chemins ouverts sont rondes ou plates.
pub fn stroke_coverage(model: &VectorModel, width: u32, height: u32) -> Option<Coverage> {
    let stroke = model.stroke?;
    let hw = f64::from(stroke.width) / 2.0;
    if hw <= 0.0 {
        return None;
    }
    let all = contours_of(model, false);
    let (x0, y0, x1, y1) = clip_box(&all, width, height, hw + 1.0)?;
    let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let mut cov = vec![0.0f32; w * h];
    for (path, contour) in model.paths.iter().filter(|p| p.flatten(FLATNESS).len() >= 2).zip(all.iter()) {
        let n = contour.len();
        let segs = if path.closed { n } else { n - 1 };
        for i in 0..segs {
            let (a, b) = (contour[i], contour[(i + 1) % n]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len2 = (dx * dx + dy * dy).max(1e-12);
            let (sx0, sx1) = (
                ((a.0.min(b.0) - hw - 1.0).floor().max(f64::from(x0)) as i64),
                ((a.0.max(b.0) + hw + 1.0).ceil().min(f64::from(x1)) as i64),
            );
            let (sy0, sy1) = (
                ((a.1.min(b.1) - hw - 1.0).floor().max(f64::from(y0)) as i64),
                ((a.1.max(b.1) + hw + 1.0).ceil().min(f64::from(y1)) as i64),
            );
            for py in sy0..sy1 {
                for px in sx0..sx1 {
                    let (qx, qy) = (px as f64 + 0.5, py as f64 + 0.5);
                    let t = ((qx - a.0) * dx + (qy - a.1) * dy) / len2;
                    // Extrémités plates d'un chemin ouvert : rien au-delà du dernier point.
                    if !path.closed && !stroke.round_caps && ((i == 0 && t < 0.0) || (i == segs - 1 && t > 1.0)) {
                        continue;
                    }
                    let tc = t.clamp(0.0, 1.0);
                    let d = (qx - (a.0 + dx * tc)).hypot(qy - (a.1 + dy * tc));
                    let c = (hw + 0.5 - d).clamp(0.0, 1.0) as f32;
                    let cell = &mut cov[(py as u32 - y0) as usize * w + (px as u32 - x0) as usize];
                    if c > *cell {
                        *cell = c;
                    }
                }
            }
        }
    }
    Some(Coverage { x0, y0, w, h, data: cov })
}

fn paint(out: &mut TileGrid, cov: &Coverage, color: [u8; 4]) {
    let rgb = [f32::from(color[0]) / 255.0, f32::from(color[1]) / 255.0, f32::from(color[2]) / 255.0];
    let base_alpha = f32::from(color[3]) / 255.0;
    for row in 0..cov.h {
        for col in 0..cov.w {
            let a = cov.data[row * cov.w + col] * base_alpha;
            if a <= 0.0 {
                continue;
            }
            let (x, y) = (cov.x0 + col as u32, cov.y0 + row as u32);
            let b = out.get_pixel(x, y).map(|c| f32::from(c) / 255.0);
            let p = composite(BlendMode::Normal, b, rgb, a, x, y);
            out.put_pixel(x, y, p.map(|c| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8));
        }
    }
}

/// Image du modèle sur un calque `width × height` : remplissage puis contour.
pub fn render(model: &VectorModel, width: u32, height: u32) -> Result<TileGrid, CoreError> {
    let model = model.sanitized();
    let mut out = TileGrid::new(width, height)?;
    if let Some(color) = model.fill {
        if let Some(c) = fill_coverage(&contours_of(&model, true), model.fill_rule, width, height) {
            paint(&mut out, &c, color);
        }
    }
    if let (Some(stroke), Some(c)) = (model.stroke, stroke_coverage(&model, width, height)) {
        paint(&mut out, &c, stroke.color);
    }
    Ok(out)
}

/// Chemin → sélection : la forme remplie (sans le contour) devient un masque 0..255.
pub fn to_selection(model: &VectorModel, width: u32, height: u32) -> Result<TileGrid, CoreError> {
    let model = model.sanitized();
    let mut m = TileGrid::new(width, height)?;
    if let Some(c) = fill_coverage(&contours_of(&model, true), model.fill_rule, width, height) {
        for row in 0..c.h {
            for col in 0..c.w {
                let v = c.data[row * c.w + col];
                if v > 0.0 {
                    let g = (v * 255.0 + 0.5) as u8;
                    m.put_pixel(c.x0 + col as u32, c.y0 + row as u32, [g, g, g, g]);
                }
            }
        }
    }
    Ok(m)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    Union,
    Intersect,
    /// A moins B.
    Subtract,
    Xor,
}

fn to_shape(model: &VectorModel) -> Vec<Vec<[f64; 2]>> {
    contours_of(model, true).into_iter().map(|c| c.into_iter().map(|p| [p.0, p.1]).collect()).collect()
}

/// Opération booléenne entre les formes remplies de deux modèles. Le résultat est polygonal (les courbes sont
/// aplaties à `FLATNESS` px) ; il adopte le style de `a`.
pub fn boolean(a: &VectorModel, b: &VectorModel, op: BooleanOp) -> VectorModel {
    let rule = |m: &VectorModel| if m.fill_rule == FillRule::EvenOdd { OFillRule::EvenOdd } else { OFillRule::NonZero };
    // i_overlay applique une seule règle : on normalise chaque opérande en lui appliquant sa propre règle
    // (union avec rien) avant l'opération.
    let norm = |m: &VectorModel| -> Vec<Vec<[f64; 2]>> {
        let shape = to_shape(m);
        shape.overlay(&Vec::<Vec<[f64; 2]>>::new(), OverlayRule::Subject, rule(m)).into_iter().flatten().collect()
    };
    let (sa, sb) = (norm(a), norm(b));
    let rule_op = match op {
        BooleanOp::Union => OverlayRule::Union,
        BooleanOp::Intersect => OverlayRule::Intersect,
        BooleanOp::Subtract => OverlayRule::Difference,
        BooleanOp::Xor => OverlayRule::Xor,
    };
    let result = sa.overlay(&sb, rule_op, OFillRule::NonZero);
    let paths = result
        .into_iter()
        .flatten()
        .filter(|c| c.len() >= 3)
        .map(|c| Path { anchors: c.into_iter().map(|p| Anchor::corner(p[0], p[1])).collect(), closed: true })
        .collect();
    VectorModel { paths, fill: a.fill, stroke: a.stroke, fill_rule: FillRule::EvenOdd }.sanitized()
}

/// Importe un SVG et le rend en image (raster) à la taille de sa déclaration, limité à `max_side` px.
pub fn import_svg(data: &[u8], max_side: u32) -> Result<TileGrid, String> {
    const MAX_BYTES: usize = 20 * 1024 * 1024;
    if data.len() > MAX_BYTES {
        return Err("fichier SVG trop volumineux (20 Mio maximum)".into());
    }
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(data, &opt).map_err(|e| format!("SVG illisible : {e}"))?;
    let size = tree.size();
    let (w, h) = (size.width().ceil() as u32, size.height().ceil() as u32);
    if w == 0 || h == 0 {
        return Err("le SVG n'a pas de taille".into());
    }
    let scale = (f32::from(u16::MAX).min(max_side as f32) / w.max(h) as f32).min(1.0);
    let (pw, ph) = (((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1));
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pw, ph).ok_or("dimensions SVG invalides")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    // tiny-skia fournit du RGBA prémultiplié : on repasse en alpha droit.
    let mut rgba = Vec::with_capacity((pw * ph * 4) as usize);
    for p in pixmap.pixels() {
        let c = p.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    TileGrid::from_rgba8(pw, ph, &rgba).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpics_core::vector::Stroke;

    fn model(paths: Vec<Path>, fill: Option<[u8; 4]>, stroke: Option<Stroke>) -> VectorModel {
        VectorModel { paths, fill, stroke, fill_rule: FillRule::NonZero }
    }

    fn alpha_area(g: &TileGrid) -> f64 {
        (0..g.height())
            .flat_map(|y| (0..g.width()).map(move |x| (x, y)))
            .map(|(x, y)| f64::from(g.get_pixel(x, y)[3]) / 255.0)
            .sum()
    }

    #[test]
    fn filled_rectangle_and_ellipse_have_exact_colour_and_area() {
        let r =
            render(&model(vec![Path::rect(10.0, 10.0, 30.0, 20.0)], Some([255, 0, 0, 255]), None), 100, 100).unwrap();
        assert_eq!((r.get_pixel(20, 20), r.get_pixel(9, 20), r.get_pixel(40, 20)), ([255, 0, 0, 255], [0; 4], [0; 4]));
        assert!((alpha_area(&r) - 600.0).abs() < 1.0);
        let e =
            render(&model(vec![Path::ellipse(50.0, 50.0, 30.0, 20.0)], Some([0, 0, 0, 255]), None), 100, 100).unwrap();
        let expected = std::f64::consts::PI * 600.0;
        assert!((alpha_area(&e) - expected).abs() / expected < 0.01, "{}", alpha_area(&e));
        // Demi-pixel : un bord fractionnaire est anticrénelé.
        let half =
            render(&model(vec![Path::rect(10.0, 10.0, 10.5, 10.0)], Some([255, 255, 255, 255]), None), 40, 40).unwrap();
        let edge = half.get_pixel(20, 15)[3];
        assert!((120..=135).contains(&edge), "{edge}");
    }

    #[test]
    fn fill_rules_differ_on_overlapping_same_direction_contours_and_holes_work() {
        let outer = Path::rect(10.0, 10.0, 60.0, 60.0);
        let inner = Path::rect(30.0, 30.0, 20.0, 20.0);
        let nz = render(&model(vec![outer.clone(), inner.clone()], Some([0, 0, 0, 255]), None), 100, 100).unwrap();
        assert_eq!(nz.get_pixel(40, 40)[3], 255, "non-zéro : même sens, pas de trou");
        let mut eo = model(vec![outer.clone(), inner.clone()], Some([0, 0, 0, 255]), None);
        eo.fill_rule = FillRule::EvenOdd;
        assert_eq!(render(&eo, 100, 100).unwrap().get_pixel(40, 40)[3], 0, "pair-impair : trou");
        // Non-zéro avec un contour de sens opposé : trou aussi.
        let mut reversed = inner.anchors.clone();
        reversed.reverse();
        let hole = model(vec![outer, Path { anchors: reversed, closed: true }], Some([0, 0, 0, 255]), None);
        assert_eq!(render(&hole, 100, 100).unwrap().get_pixel(40, 40)[3], 0);
    }

    #[test]
    fn stroke_has_the_requested_width_caps_and_draws_over_the_fill() {
        let st = |round| Some(Stroke { color: [0, 0, 255, 255], width: 6.0, round_caps: round });
        let line =
            |round| render(&model(vec![Path::line((20.0, 50.0), (80.0, 50.0))], None, st(round)), 100, 100).unwrap();
        let l = line(false);
        assert_eq!(
            (l.get_pixel(50, 48)[3], l.get_pixel(50, 52)[3], l.get_pixel(50, 45)[3], l.get_pixel(50, 55)[3]),
            (255, 255, 0, 0),
            "6 px de large"
        );
        assert_eq!(l.get_pixel(18, 50)[3], 0, "extrémité plate : rien au-delà");
        assert!(line(true).get_pixel(18, 50)[3] > 100, "extrémité ronde : déborde");
        let area = alpha_area(&l);
        assert!((area - 360.0).abs() < 8.0, "aire du trait : {area}");
        let both = render(
            &model(
                vec![Path::rect(20.0, 20.0, 40.0, 40.0)],
                Some([255, 0, 0, 255]),
                Some(Stroke { color: [0, 0, 255, 255], width: 4.0, round_caps: false }),
            ),
            100,
            100,
        )
        .unwrap();
        assert_eq!(both.get_pixel(40, 40), [255, 0, 0, 255]);
        assert_eq!(both.get_pixel(20, 40), [0, 0, 255, 255], "le contour recouvre le bord du remplissage");
        assert!(
            render(
                &model(
                    vec![Path::line((0.0, 0.0), (9.0, 9.0))],
                    None,
                    Some(Stroke { color: [0; 4], width: 0.0, round_caps: true })
                ),
                10,
                10
            )
            .unwrap()
            .tile_count()
                == 0
        );
    }

    #[test]
    fn boolean_operations_have_the_expected_areas() {
        let a = model(vec![Path::rect(0.0, 0.0, 40.0, 40.0)], Some([255, 0, 0, 255]), None);
        let b = model(vec![Path::rect(20.0, 20.0, 40.0, 40.0)], Some([0, 0, 255, 255]), None);
        let area = |m: &VectorModel| alpha_area(&render(m, 100, 100).unwrap());
        assert!((area(&boolean(&a, &b, BooleanOp::Union)) - 2800.0).abs() < 2.0);
        assert!((area(&boolean(&a, &b, BooleanOp::Intersect)) - 400.0).abs() < 2.0);
        assert!((area(&boolean(&a, &b, BooleanOp::Subtract)) - 1200.0).abs() < 2.0);
        assert!((area(&boolean(&a, &b, BooleanOp::Xor)) - 2400.0).abs() < 2.0);
        assert_eq!(boolean(&a, &b, BooleanOp::Union).fill, a.fill, "le résultat adopte le style de A");
        let disjoint = model(vec![Path::rect(70.0, 70.0, 5.0, 5.0)], Some([0; 4]), None);
        assert!(boolean(&a, &disjoint, BooleanOp::Intersect).paths.is_empty());
        // Un trou (soustraction) reste un trou au rendu.
        let ring = boolean(
            &model(vec![Path::rect(0.0, 0.0, 60.0, 60.0)], Some([0, 0, 0, 255]), None),
            &model(vec![Path::rect(20.0, 20.0, 20.0, 20.0)], None, None),
            BooleanOp::Subtract,
        );
        assert_eq!(render(&ring, 100, 100).unwrap().get_pixel(30, 30)[3], 0);
    }

    #[test]
    fn path_to_selection_matches_the_fill_and_ignores_the_stroke() {
        let m = model(
            vec![Path::ellipse(50.0, 50.0, 20.0, 20.0)],
            Some([1, 2, 3, 4]),
            Some(Stroke { color: [0; 4], width: 30.0, round_caps: true }),
        );
        let sel = to_selection(&m, 100, 100).unwrap();
        assert_eq!(
            (openpics_core::selection::value(&sel, 50, 50), openpics_core::selection::value(&sel, 5, 5)),
            (255, 0)
        );
        assert_eq!(
            openpics_core::selection::value(&sel, 50, 25),
            0,
            "au-delà du rayon 20 : pas de contour dans la sélection"
        );
        assert_eq!(to_selection(&VectorModel::default(), 10, 10).unwrap().tile_count(), 0);
    }

    #[test]
    fn svg_import_renders_shapes_and_rejects_garbage() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30"><rect x="10" y="5" width="20" height="10" fill="#ff0000"/></svg>"##;
        let g = import_svg(svg, 4096).unwrap();
        assert_eq!((g.width(), g.height()), (40, 30));
        assert_eq!(g.get_pixel(20, 10), [255, 0, 0, 255]);
        assert_eq!(g.get_pixel(2, 2)[3], 0);
        let big = br##"<svg xmlns="http://www.w3.org/2000/svg" width="8000" height="4000"><rect width="8000" height="4000" fill="#00ff00"/></svg>"##;
        let scaled = import_svg(big, 800).unwrap();
        assert_eq!((scaled.width(), scaled.height()), (800, 400), "ramené à la taille maximale");
        assert!(import_svg(b"pas du svg", 100).is_err());
        assert!(import_svg(br#"<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0"/>"#, 100).is_err());
        assert!(import_svg(&vec![b' '; 21 * 1024 * 1024], 100).is_err());
    }
}
