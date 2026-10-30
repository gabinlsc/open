//! Modèle vectoriel : chemins de Bézier (plume), formes et leur style. Le rendu (remplissage, contour,
//! opérations booléennes) est dans le crate `ops` ; ce module ne contient que des données et de la géométrie pure.

use serde::{Deserialize, Serialize};

pub type Pt = (f64, f64);

/// Limites : un fichier ou une saisie hostile ne doit ni saturer la mémoire ni produire de coordonnées absurdes.
pub const MAX_ANCHORS: usize = 4096;
pub const MAX_COORD: f64 = 1.0e6;
pub const MAX_STROKE: f32 = 2000.0;

/// Point d'ancrage d'un chemin. Les poignées sont **relatives** à `pos` ; `(0, 0)` = sommet anguleux.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub pos: Pt,
    #[serde(default)]
    pub h_in: Pt,
    #[serde(default)]
    pub h_out: Pt,
}

impl Anchor {
    pub fn corner(x: f64, y: f64) -> Self {
        Anchor { pos: (x, y), h_in: (0.0, 0.0), h_out: (0.0, 0.0) }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Path {
    pub anchors: Vec<Anchor>,
    pub closed: bool,
}

/// Constante de la Bézier cubique qui approche un quart de cercle.
const KAPPA: f64 = 0.552_284_749_830_793_4;

impl Path {
    pub fn polygon(points: &[Pt]) -> Path {
        Path { anchors: points.iter().map(|p| Anchor::corner(p.0, p.1)).collect(), closed: true }
    }

    pub fn line(a: Pt, b: Pt) -> Path {
        Path { anchors: vec![Anchor::corner(a.0, a.1), Anchor::corner(b.0, b.1)], closed: false }
    }

    pub fn rect(x: f64, y: f64, w: f64, h: f64) -> Path {
        Path::polygon(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)])
    }

    pub fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Path {
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        let a = |x: f64, y: f64, hi: Pt, ho: Pt| Anchor { pos: (x, y), h_in: hi, h_out: ho };
        Path {
            anchors: vec![
                a(cx + rx, cy, (0.0, -ky), (0.0, ky)),
                a(cx, cy + ry, (kx, 0.0), (-kx, 0.0)),
                a(cx - rx, cy, (0.0, ky), (0.0, -ky)),
                a(cx, cy - ry, (-kx, 0.0), (kx, 0.0)),
            ],
            closed: true,
        }
    }

    /// Rectangle à coins arrondis (rayon borné à la moitié du plus petit côté).
    pub fn rounded_rect(x: f64, y: f64, w: f64, h: f64, radius: f64) -> Path {
        let r = radius.clamp(0.0, w.min(h) / 2.0);
        if r <= 0.0 {
            return Path::rect(x, y, w, h);
        }
        let k = r * KAPPA;
        let a = |px: f64, py: f64, hi: Pt, ho: Pt| Anchor { pos: (px, py), h_in: hi, h_out: ho };
        Path {
            anchors: vec![
                a(x + r, y, (-k, 0.0), (0.0, 0.0)),
                a(x + w - r, y, (0.0, 0.0), (k, 0.0)),
                a(x + w, y + r, (0.0, -k), (0.0, 0.0)),
                a(x + w, y + h - r, (0.0, 0.0), (0.0, k)),
                a(x + w - r, y + h, (k, 0.0), (0.0, 0.0)),
                a(x + r, y + h, (0.0, 0.0), (-k, 0.0)),
                a(x, y + h - r, (0.0, k), (0.0, 0.0)),
                a(x, y + r, (0.0, 0.0), (0.0, -k)),
            ],
            closed: true,
        }
    }

    /// Polyligne qui approche le chemin : chaque Bézier est subdivisée jusqu'à ce que l'écart tienne sous `tol` px.
    pub fn flatten(&self, tol: f64) -> Vec<Pt> {
        let n = self.anchors.len();
        if n == 0 {
            return Vec::new();
        }
        let mut out = vec![self.anchors[0].pos];
        let segs = if self.closed { n } else { n.saturating_sub(1) };
        for i in 0..segs {
            let (a, b) = (&self.anchors[i], &self.anchors[(i + 1) % n]);
            let p0 = a.pos;
            let p1 = (a.pos.0 + a.h_out.0, a.pos.1 + a.h_out.1);
            let p2 = (b.pos.0 + b.h_in.0, b.pos.1 + b.h_in.1);
            let p3 = b.pos;
            if a.h_out == (0.0, 0.0) && b.h_in == (0.0, 0.0) {
                out.push(p3);
            } else {
                flatten_cubic(p0, p1, p2, p3, tol.max(0.01), 0, &mut out);
            }
        }
        // Un chemin fermé repasse par son point de départ : on évite le doublon final.
        if self.closed && out.len() > 1 && out.last() == out.first() {
            out.pop();
        }
        out
    }

    pub fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let pts = self.flatten(0.25);
        let first = *pts.first()?;
        Some(pts.iter().fold((first.0, first.1, first.0, first.1), |b, p| {
            (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1))
        }))
    }

    pub fn translated(&self, dx: f64, dy: f64) -> Path {
        Path {
            anchors: self.anchors.iter().map(|a| Anchor { pos: (a.pos.0 + dx, a.pos.1 + dy), ..*a }).collect(),
            closed: self.closed,
        }
    }

    /// Aire signée d'un polygone (formule du lacet) ; positive dans un sens, négative dans l'autre.
    pub fn signed_area(points: &[Pt]) -> f64 {
        let n = points.len();
        (0..n).map(|i| points[i].0 * points[(i + 1) % n].1 - points[(i + 1) % n].0 * points[i].1).sum::<f64>() / 2.0
    }
}

fn flatten_cubic(p0: Pt, p1: Pt, p2: Pt, p3: Pt, tol: f64, depth: u32, out: &mut Vec<Pt>) {
    // Écart des points de contrôle à la corde : borne l'erreur de la courbe.
    let (dx, dy) = (p3.0 - p0.0, p3.1 - p0.1);
    let len = dx.hypot(dy);
    let dist = |p: Pt| {
        if len < 1e-9 { (p.0 - p0.0).hypot(p.1 - p0.1) } else { ((p.0 - p0.0) * dy - (p.1 - p0.1) * dx).abs() / len }
    };
    if depth >= 16 || dist(p1).max(dist(p2)) <= tol {
        out.push(p3);
        return;
    }
    let mid = |a: Pt, b: Pt| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let (p01, p12, p23) = (mid(p0, p1), mid(p1, p2), mid(p2, p3));
    let (p012, p123) = (mid(p01, p12), mid(p12, p23));
    let m = mid(p012, p123);
    flatten_cubic(p0, p01, p012, m, tol, depth + 1, out);
    flatten_cubic(m, p123, p23, p3, tol, depth + 1, out);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub color: [u8; 4],
    pub width: f32,
    /// Extrémités arrondies (sinon plates) pour les chemins ouverts. Les jonctions sont toujours arrondies.
    pub round_caps: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorModel {
    pub paths: Vec<Path>,
    pub fill: Option<[u8; 4]>,
    pub stroke: Option<Stroke>,
    pub fill_rule: FillRule,
}

impl Default for VectorModel {
    fn default() -> Self {
        Self { paths: Vec::new(), fill: Some([0, 0, 0, 255]), stroke: None, fill_rule: FillRule::NonZero }
    }
}

impl VectorModel {
    /// Bornes toutes les coordonnées et le nombre d'ancrages ; écarte les valeurs non finies.
    pub fn sanitized(&self) -> VectorModel {
        let clamp = |v: f64| if v.is_finite() { v.clamp(-MAX_COORD, MAX_COORD) } else { 0.0 };
        let cp = |p: Pt| (clamp(p.0), clamp(p.1));
        let mut budget = MAX_ANCHORS;
        let paths = self
            .paths
            .iter()
            .filter_map(|path| {
                if budget == 0 {
                    return None;
                }
                let take = path.anchors.len().min(budget);
                budget -= take;
                Some(Path {
                    anchors: path.anchors[..take]
                        .iter()
                        .map(|a| Anchor { pos: cp(a.pos), h_in: cp(a.h_in), h_out: cp(a.h_out) })
                        .collect(),
                    closed: path.closed,
                })
            })
            .collect();
        let stroke = self
            .stroke
            .map(|s| Stroke { width: if s.width.is_finite() { s.width.clamp(0.0, MAX_STROKE) } else { 1.0 }, ..s });
        VectorModel { paths, fill: self.fill, stroke, fill_rule: self.fill_rule }
    }

    pub fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        self.paths
            .iter()
            .filter_map(Path::bounds)
            .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
    }

    /// Export SVG (un `<path>` par chemin, Bézier conservées).
    pub fn to_svg(&self, width: u32, height: u32) -> String {
        let hex = |c: [u8; 4]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
        let mut s = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n"
        );
        for path in &self.paths {
            let Some(first) = path.anchors.first() else { continue };
            let mut d = format!("M{} {}", first.pos.0, first.pos.1);
            let n = path.anchors.len();
            let segs = if path.closed { n } else { n.saturating_sub(1) };
            for i in 0..segs {
                let (a, b) = (&path.anchors[i], &path.anchors[(i + 1) % n]);
                let c1 = (a.pos.0 + a.h_out.0, a.pos.1 + a.h_out.1);
                let c2 = (b.pos.0 + b.h_in.0, b.pos.1 + b.h_in.1);
                d += &format!(" C{} {} {} {} {} {}", c1.0, c1.1, c2.0, c2.1, b.pos.0, b.pos.1);
            }
            if path.closed {
                d += " Z";
            }
            let fill = self.fill.map_or("none".to_owned(), hex);
            let fill_opacity = self.fill.map_or(1.0, |c| f32::from(c[3]) / 255.0);
            let rule = if self.fill_rule == FillRule::EvenOdd { "evenodd" } else { "nonzero" };
            s += &format!("  <path d=\"{d}\" fill=\"{fill}\" fill-opacity=\"{fill_opacity}\" fill-rule=\"{rule}\"");
            if let Some(st) = self.stroke {
                let cap = if st.round_caps { "round" } else { "butt" };
                s += &format!(
                    " stroke=\"{}\" stroke-opacity=\"{}\" stroke-width=\"{}\" stroke-linecap=\"{cap}\" stroke-linejoin=\"round\"",
                    hex(st.color),
                    f32::from(st.color[3]) / 255.0,
                    st.width
                );
            }
            s += "/>\n";
        }
        s + "</svg>\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_have_the_expected_geometry() {
        let r = Path::rect(10.0, 20.0, 30.0, 40.0);
        assert_eq!(r.bounds(), Some((10.0, 20.0, 40.0, 60.0)));
        assert!((Path::signed_area(&r.flatten(0.1)).abs() - 1200.0).abs() < 1e-9);
        let e = Path::ellipse(50.0, 50.0, 30.0, 20.0);
        let area = Path::signed_area(&e.flatten(0.05)).abs();
        assert!(
            (area - std::f64::consts::PI * 600.0).abs() / (std::f64::consts::PI * 600.0) < 0.002,
            "aire de l'ellipse : {area}"
        );
        let (x0, y0, x1, y1) = e.bounds().unwrap();
        assert!(
            (x0 - 20.0).abs() < 0.1 && (x1 - 80.0).abs() < 0.1 && (y0 - 30.0).abs() < 0.1 && (y1 - 70.0).abs() < 0.1
        );
    }

    #[test]
    fn rounded_rect_clamps_its_radius_and_is_smaller_than_the_rect() {
        let rr = Path::rounded_rect(0.0, 0.0, 100.0, 50.0, 10.0);
        let area = Path::signed_area(&rr.flatten(0.05)).abs();
        let expected = 5000.0 - (4.0 - std::f64::consts::PI) * 100.0;
        assert!((area - expected).abs() < 2.0, "{area} vs {expected}");
        assert_eq!(Path::rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0), Path::rect(0.0, 0.0, 10.0, 10.0));
        let huge = Path::rounded_rect(0.0, 0.0, 40.0, 20.0, 999.0);
        assert_eq!(huge.bounds().map(|b| (b.2 - b.0, b.3 - b.1)), Some((40.0, 20.0)));
    }

    #[test]
    fn flattening_respects_the_tolerance_and_handles_open_and_empty_paths() {
        let curve = Path {
            anchors: vec![
                Anchor { pos: (0.0, 0.0), h_in: (0.0, 0.0), h_out: (0.0, 50.0) },
                Anchor { pos: (100.0, 0.0), h_in: (0.0, 50.0), h_out: (0.0, 0.0) },
            ],
            closed: false,
        };
        let coarse = curve.flatten(5.0).len();
        let fine = curve.flatten(0.05).len();
        assert!(fine > coarse && coarse >= 3);
        let line = Path::line((0.0, 0.0), (10.0, 0.0));
        assert_eq!(line.flatten(0.25), vec![(0.0, 0.0), (10.0, 0.0)]);
        assert!(Path::default().flatten(0.25).is_empty() && Path::default().bounds().is_none());
        // Une courbe dégénérée (extrémités confondues) ne boucle pas indéfiniment.
        let degenerate = Path {
            anchors: vec![
                Anchor { pos: (5.0, 5.0), h_in: (0.0, 0.0), h_out: (30.0, 0.0) },
                Anchor { pos: (5.0, 5.0), h_in: (-30.0, 0.0), h_out: (0.0, 0.0) },
            ],
            closed: false,
        };
        assert!(degenerate.flatten(0.01).len() < 70_000);
    }

    #[test]
    fn sanitizing_bounds_coordinates_anchors_and_stroke() {
        let wild = VectorModel {
            paths: vec![
                Path {
                    anchors: vec![Anchor::corner(f64::NAN, 1e12), Anchor::corner(5.0, -1e12)],
                    closed: false
                };
                3000
            ],
            fill: None,
            stroke: Some(Stroke { color: [1, 2, 3, 4], width: f32::INFINITY, round_caps: true }),
            fill_rule: FillRule::EvenOdd,
        };
        let s = wild.sanitized();
        assert_eq!(s.paths.iter().map(|p| p.anchors.len()).sum::<usize>(), MAX_ANCHORS);
        assert!(
            s.paths.iter().flat_map(|p| &p.anchors).all(|a| a.pos.0.abs() <= MAX_COORD && a.pos.1.abs() <= MAX_COORD)
        );
        assert_eq!(s.stroke.unwrap().width, 1.0);
    }

    #[test]
    fn svg_export_keeps_curves_and_style_and_roundtrips_through_json() {
        let m = VectorModel {
            paths: vec![Path::ellipse(50.0, 50.0, 20.0, 10.0)],
            fill: Some([255, 0, 0, 128]),
            stroke: Some(Stroke { color: [0, 0, 255, 255], width: 3.0, round_caps: false }),
            fill_rule: FillRule::EvenOdd,
        };
        let svg = m.to_svg(100, 100);
        assert!(
            svg.contains("viewBox=\"0 0 100 100\"")
                && svg.contains(" C")
                && svg.contains("Z")
                && svg.contains("#ff0000")
                && svg.contains("evenodd")
                && svg.contains("stroke-width=\"3\"")
                && svg.contains("butt"),
            "{svg}"
        );
        assert!(svg.contains("fill-opacity=\"0.5"));
        assert_eq!(serde_json::from_str::<VectorModel>(&serde_json::to_string(&m).unwrap()).unwrap(), m);
        assert!(VectorModel::default().to_svg(1, 1).contains("</svg>"));
    }
}
