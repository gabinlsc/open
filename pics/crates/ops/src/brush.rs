//! Moteur de trait : pinceau, crayon, gomme. Les dabs sont accumulés dans une couche de couverture (flottante)
//! puis composés avec le calque d'origine ; cela permet de plafonner l'opacité *par trait* (un trait qui se
//! recoupe ne devient pas plus opaque) tout en laissant le flux s'accumuler dab après dab.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use openpics_core::blend::{BlendMode, composite};
use openpics_core::selection::value as selection_value;
use openpics_core::{CoreError, TILE_SIZE, Tile, TileGrid, TileKey};

const TS: usize = TILE_SIZE as usize;
const MAX_DIAMETER: f32 = 2000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintKind {
    Brush,
    /// Bords nets, sans anticrénelage ni dureté.
    Pencil,
    Eraser,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrushParams {
    /// Diamètre en pixels (1..=2000).
    pub size: f32,
    /// 0 = bord très doux, 1 = bord net.
    pub hardness: f32,
    /// Distance entre deux dabs, en fraction du diamètre (0,02..=2).
    pub spacing: f32,
    /// Plafond d'opacité du trait entier.
    pub opacity: f32,
    /// Quantité déposée par dab ; les dabs qui se recoupent s'accumulent.
    pub flow: f32,
    pub color: [u8; 3],
    /// 0 = aucun lissage ; proche de 1 = le trait suit le curseur avec beaucoup de retard.
    pub smoothing: f32,
    pub size_from_pressure: bool,
    pub opacity_from_pressure: bool,
}

impl Default for BrushParams {
    fn default() -> Self {
        Self {
            size: 20.0,
            hardness: 0.8,
            spacing: 0.15,
            opacity: 1.0,
            flow: 1.0,
            color: [0, 0, 0],
            smoothing: 0.0,
            size_from_pressure: true,
            opacity_from_pressure: false,
        }
    }
}

impl BrushParams {
    /// Ramène chaque réglage dans sa plage : une valeur absurde ne doit ni figer ni saturer le trait.
    pub fn sanitized(&self) -> BrushParams {
        let f = |v: f32, lo: f32, hi: f32, default: f32| if v.is_finite() { v.clamp(lo, hi) } else { default };
        BrushParams {
            size: f(self.size, 1.0, MAX_DIAMETER, 20.0),
            hardness: f(self.hardness, 0.0, 1.0, 0.8),
            spacing: f(self.spacing, 0.02, 2.0, 0.15),
            opacity: f(self.opacity, 0.0, 1.0, 1.0),
            flow: f(self.flow, 0.01, 1.0, 1.0),
            smoothing: f(self.smoothing, 0.0, 0.98, 0.0),
            ..*self
        }
    }
}

/// Couverture (0..1) d'un dab à la distance `d` de son centre, pour le rayon `r`.
pub fn dab_coverage(kind: PaintKind, hardness: f32, r: f32, d: f32) -> f32 {
    if kind == PaintKind::Pencil {
        return if d <= r { 1.0 } else { 0.0 };
    }
    let inner = r * hardness;
    if d <= inner {
        1.0
    } else if d >= r {
        0.0
    } else {
        // Chute lissée (smoothstep inversé) entre le cœur et le bord.
        let t = (d - inner) / (r - inner).max(1e-6);
        1.0 - t * t * (3.0 - 2.0 * t)
    }
}

pub struct PaintStroke {
    kind: PaintKind,
    params: BrushParams,
    base: TileGrid,
    result: TileGrid,
    cov: HashMap<TileKey, Vec<f32>>,
    selection: Option<TileGrid>,
    dirty: HashSet<TileKey>,
    last_dab: Option<(f32, f32)>,
    carry: f32,
    smoothed: Option<(f32, f32)>,
}

impl PaintStroke {
    pub fn new(base: TileGrid, kind: PaintKind, params: BrushParams, selection: Option<TileGrid>) -> Self {
        Self {
            kind,
            params: params.sanitized(),
            result: base.clone(),
            base,
            cov: HashMap::new(),
            selection,
            dirty: HashSet::new(),
            last_dab: None,
            carry: 0.0,
            smoothed: None,
        }
    }

    pub fn base(&self) -> &TileGrid {
        &self.base
    }

    /// Le calque tel qu'il est avec le trait en cours.
    pub fn result(&self) -> &TileGrid {
        &self.result
    }

    /// Ajoute un point de trait ; `pressure` entre 0 et 1 (1 pour une souris).
    pub fn add_point(&mut self, x: f32, y: f32, pressure: f32) -> Result<(), CoreError> {
        if !x.is_finite() || !y.is_finite() {
            return Ok(());
        }
        let pressure = if pressure.is_finite() { pressure.clamp(0.0, 1.0) } else { 1.0 };
        let (sx, sy) = match self.smoothed {
            Some((px, py)) => {
                let k = 1.0 - self.params.smoothing;
                (px + (x - px) * k, py + (y - py) * k)
            }
            None => (x, y),
        };
        self.smoothed = Some((sx, sy));
        let size = if self.params.size_from_pressure {
            (self.params.size * pressure.max(0.05)).max(1.0)
        } else {
            self.params.size
        };
        let spacing = (size * self.params.spacing).max(0.5);
        match self.last_dab {
            None => {
                self.dab(sx, sy, size, pressure);
                self.last_dab = Some((sx, sy));
            }
            Some((lx, ly)) => {
                let (dx, dy) = (sx - lx, sy - ly);
                let len = dx.hypot(dy);
                let mut travelled = spacing - self.carry;
                while travelled <= len {
                    let t = travelled / len;
                    self.dab(lx + dx * t, ly + dy * t, size, pressure);
                    travelled += spacing;
                }
                self.carry = (len - (travelled - spacing)).max(0.0);
                self.last_dab = Some((sx, sy));
            }
        }
        self.flush()
    }

    fn dab(&mut self, cx: f32, cy: f32, size: f32, pressure: f32) {
        let r = size / 2.0;
        let strength = self.params.flow * if self.params.opacity_from_pressure { pressure } else { 1.0 };
        let (w, h) = (self.base.width() as i64, self.base.height() as i64);
        let (x0, x1) = (((cx - r).floor() as i64).max(0), ((cx + r).ceil() as i64).min(w - 1));
        let (y0, y1) = (((cy - r).floor() as i64).max(0), ((cy + r).ceil() as i64).min(h - 1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let c = dab_coverage(self.kind, self.params.hardness, r, d) * strength;
                if c <= 0.0 {
                    continue;
                }
                let key = TileKey { tx: x as u32 / TILE_SIZE, ty: y as u32 / TILE_SIZE };
                let buf = self.cov.entry(key).or_insert_with(|| vec![0.0; TS * TS]);
                let i = (y as usize % TS) * TS + x as usize % TS;
                buf[i] += c * (1.0 - buf[i]);
                self.dirty.insert(key);
            }
        }
    }

    /// Recompose les tuiles touchées depuis le calque d'origine et la couverture.
    fn flush(&mut self) -> Result<(), CoreError> {
        let color = self.params.color;
        let rgb = [f32::from(color[0]) / 255.0, f32::from(color[1]) / 255.0, f32::from(color[2]) / 255.0];
        for key in std::mem::take(&mut self.dirty) {
            let Some(cov) = self.cov.get(&key) else { continue };
            let mut buf = vec![0u8; TS * TS * 4];
            let (ox, oy) = (key.tx * TILE_SIZE, key.ty * TILE_SIZE);
            for ly in 0..TILE_SIZE.min(self.base.height() - oy) {
                for lx in 0..TILE_SIZE.min(self.base.width() - ox) {
                    let (x, y) = (ox + lx, oy + ly);
                    let b = self.base.get_pixel(x, y);
                    let i = ly as usize * TS + lx as usize;
                    let sel = self.selection.as_ref().map_or(1.0, |s| f32::from(selection_value(s, x, y)) / 255.0);
                    let amount = cov[i] * self.params.opacity * sel;
                    let out = if amount <= 0.0 {
                        b
                    } else if self.kind == PaintKind::Eraser {
                        [b[0], b[1], b[2], (f32::from(b[3]) * (1.0 - amount) + 0.5) as u8]
                    } else {
                        let back = [
                            f32::from(b[0]) / 255.0,
                            f32::from(b[1]) / 255.0,
                            f32::from(b[2]) / 255.0,
                            f32::from(b[3]) / 255.0,
                        ];
                        let p = composite(BlendMode::Normal, back, rgb, amount, x, y);
                        [
                            (p[0] * 255.0 + 0.5) as u8,
                            (p[1] * 255.0 + 0.5) as u8,
                            (p[2] * 255.0 + 0.5) as u8,
                            (p[3] * 255.0 + 0.5) as u8,
                        ]
                    };
                    let o = (ly as usize * TS + lx as usize) * 4;
                    buf[o..o + 4].copy_from_slice(&out);
                }
            }
            let tile = Tile::from_pixels(buf)?;
            self.result.set_tile(key, (tile != Tile::Uniform([0; 4])).then(|| Arc::new(tile)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank() -> TileGrid {
        TileGrid::new(200, 100).unwrap()
    }

    fn hard() -> BrushParams {
        BrushParams {
            size: 10.0,
            hardness: 1.0,
            size_from_pressure: false,
            color: [255, 0, 0],
            ..BrushParams::default()
        }
    }

    #[test]
    fn single_hard_dab_is_a_filled_disc() {
        let mut s = PaintStroke::new(blank(), PaintKind::Brush, hard(), None);
        s.add_point(50.0, 50.0, 1.0).unwrap();
        let g = s.result();
        assert_eq!(g.get_pixel(50, 50), [255, 0, 0, 255]);
        assert_eq!(g.get_pixel(47, 50), [255, 0, 0, 255]);
        assert_eq!(g.get_pixel(40, 50), [0; 4]);
        assert_eq!(g.get_pixel(50, 60), [0; 4]);
        let area =
            (0..100).flat_map(|y| (0..200).map(move |x| (x, y))).filter(|&(x, y)| g.get_pixel(x, y)[3] > 0).count()
                as f32;
        assert!((area - std::f32::consts::PI * 25.0).abs() < 14.0, "aire {area}");
    }

    #[test]
    fn softness_falls_off_monotonically() {
        let p = BrushParams { hardness: 0.0, ..hard() };
        let mut s = PaintStroke::new(blank(), PaintKind::Brush, p, None);
        s.add_point(50.0, 50.0, 1.0).unwrap();
        let a: Vec<u8> = (0..5).map(|d| s.result().get_pixel(50 + d, 50)[3]).collect();
        assert!(a.windows(2).all(|w| w[0] >= w[1]) && a[0] > 200 && a[4] < 100, "{a:?}");
        assert!(
            dab_coverage(PaintKind::Brush, 0.5, 10.0, 4.0) == 1.0
                && dab_coverage(PaintKind::Brush, 0.5, 10.0, 10.0) == 0.0
        );
    }

    #[test]
    fn a_stroke_has_no_gaps_between_points() {
        let mut s = PaintStroke::new(blank(), PaintKind::Brush, hard(), None);
        s.add_point(20.0, 50.0, 1.0).unwrap();
        s.add_point(180.0, 50.0, 1.0).unwrap();
        for x in 20..=180 {
            assert_eq!(s.result().get_pixel(x, 50)[3], 255, "trou en x = {x}");
        }
        // Aucune dépendance au découpage : deux points intermédiaires donnent le même trait.
        let mut t = PaintStroke::new(blank(), PaintKind::Brush, hard(), None);
        for x in [20.0, 60.0, 61.0, 100.0, 180.0] {
            t.add_point(x, 50.0, 1.0).unwrap();
        }
        assert_eq!(t.result().get_pixel(100, 50), s.result().get_pixel(100, 50));
    }

    #[test]
    fn opacity_caps_the_whole_stroke_while_flow_accumulates_per_dab() {
        let capped = BrushParams { opacity: 0.5, spacing: 0.1, ..hard() };
        let mut s = PaintStroke::new(blank(), PaintKind::Brush, capped, None);
        for x in (20..120).step_by(2) {
            s.add_point(x as f32, 50.0, 1.0).unwrap();
        }
        let a = s.result().get_pixel(70, 50)[3];
        assert!((126..=129).contains(&a), "même après des dizaines de dabs, l'alpha reste plafonné à 50 % : {a}");

        let flow = BrushParams { flow: 0.1, ..hard() };
        let mut one = PaintStroke::new(blank(), PaintKind::Brush, flow, None);
        one.add_point(50.0, 50.0, 1.0).unwrap();
        let a1 = one.result().get_pixel(50, 50)[3];
        assert!((24..=27).contains(&a1), "un dab à flux 10 % dépose ≈ 25/255 : {a1}");
        one.add_point(50.0, 50.0, 1.0).unwrap(); // même position : pas de nouveau dab (distance nulle)
        let mut two = PaintStroke::new(blank(), PaintKind::Brush, BrushParams { spacing: 0.02, ..flow }, None);
        two.add_point(50.0, 50.0, 1.0).unwrap();
        two.add_point(51.0, 50.0, 1.0).unwrap();
        assert!(two.result().get_pixel(50, 50)[3] > a1, "les dabs qui se recoupent s'accumulent");
    }

    #[test]
    fn eraser_removes_alpha_and_pencil_has_hard_edges() {
        let mut base = blank();
        for y in 0..100 {
            for x in 0..200 {
                base.put_pixel(x, y, [10, 20, 30, 255]);
            }
        }
        let mut e = PaintStroke::new(base, PaintKind::Eraser, BrushParams { opacity: 0.5, ..hard() }, None);
        e.add_point(50.0, 50.0, 1.0).unwrap();
        let p = e.result().get_pixel(50, 50);
        assert_eq!((p[0], p[1], p[2]), (10, 20, 30));
        assert!((126..=129).contains(&p[3]), "{p:?}");
        let mut pen = PaintStroke::new(blank(), PaintKind::Pencil, BrushParams { hardness: 0.0, ..hard() }, None);
        pen.add_point(50.0, 50.0, 1.0).unwrap();
        let g = pen.result();
        assert!(
            (0..100).flat_map(|y| (0..200).map(move |x| (x, y))).all(|(x, y)| matches!(g.get_pixel(x, y)[3], 0 | 255))
        );
    }

    #[test]
    fn selection_confines_the_stroke() {
        let sel = crate::selection::rect(200, 100, (0.0, 0.0), (50.0, 100.0)).unwrap();
        let mut s = PaintStroke::new(blank(), PaintKind::Brush, hard(), Some(sel));
        s.add_point(50.0, 50.0, 1.0).unwrap();
        assert_eq!(s.result().get_pixel(48, 50)[3], 255);
        assert_eq!(s.result().get_pixel(52, 50)[3], 0);
    }

    #[test]
    fn pressure_scales_size_and_smoothing_lags_behind() {
        let p = BrushParams { size_from_pressure: true, ..hard() };
        let mut light = PaintStroke::new(blank(), PaintKind::Brush, p, None);
        light.add_point(50.0, 50.0, 0.3).unwrap();
        let mut full = PaintStroke::new(blank(), PaintKind::Brush, p, None);
        full.add_point(50.0, 50.0, 1.0).unwrap();
        let count = |s: &PaintStroke| (0..200).filter(|&x| s.result().get_pixel(x, 50)[3] > 0).count();
        assert!(count(&light) < count(&full));
        let sm = BrushParams { smoothing: 0.9, ..hard() };
        let mut s = PaintStroke::new(blank(), PaintKind::Brush, sm, None);
        s.add_point(20.0, 50.0, 1.0).unwrap();
        s.add_point(120.0, 50.0, 1.0).unwrap();
        assert_eq!(s.result().get_pixel(120, 50)[3], 0, "le trait lissé n'a pas encore rattrapé le curseur");
        assert_eq!(s.result().get_pixel(25, 50)[3], 255);
    }

    #[test]
    fn hostile_parameters_are_bounded_and_nan_points_ignored() {
        let wild = BrushParams {
            size: f32::NAN,
            hardness: 9.0,
            spacing: 0.0,
            opacity: -1.0,
            flow: 0.0,
            smoothing: 5.0,
            ..hard()
        };
        let ok = wild.sanitized();
        assert!(
            ok.size == 20.0
                && ok.hardness == 1.0
                && ok.spacing == 0.02
                && ok.opacity == 0.0
                && ok.flow == 0.01
                && ok.smoothing == 0.98
        );
        let mut s = PaintStroke::new(blank(), PaintKind::Brush, wild, None);
        s.add_point(f32::NAN, 5.0, 1.0).unwrap();
        s.add_point(-500.0, -500.0, f32::NAN).unwrap(); // hors document : sans effet ni panique
        assert_eq!(s.result().tile_count(), 0);
    }

    #[test]
    fn stroke_leaves_the_base_untouched_and_untouched_tiles_shared() {
        let mut base = TileGrid::new(600, 100).unwrap();
        base.put_pixel(500, 50, [1, 2, 3, 255]);
        let mut s = PaintStroke::new(base.clone(), PaintKind::Brush, hard(), None);
        s.add_point(20.0, 50.0, 1.0).unwrap();
        assert_eq!(s.base().get_pixel(20, 50), [0; 4]);
        assert!(Arc::ptr_eq(
            s.result().tile(TileKey { tx: 1, ty: 0 }).unwrap(),
            base.tile(TileKey { tx: 1, ty: 0 }).unwrap()
        ));
    }
}
