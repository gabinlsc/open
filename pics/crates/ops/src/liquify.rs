//! Fluidité : déformation interactive par un champ de déplacement. Le calque d'origine n'est jamais modifié ;
//! chaque pixel de sortie lit l'original en `(x, y) + champ(x, y)`, ce qui permet de reconstruire (revenir en arrière)
//! partiellement et évite de dégrader l'image à chaque coup de pinceau.

use openpics_core::selection::value as selection_value;
use openpics_core::{CoreError, TileGrid};

use crate::filters::Buf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquifyMode {
    /// Pousse la matière dans le sens du glissé.
    Push,
    /// Gonfle : éloigne la matière du centre du pinceau.
    Bloat,
    /// Pince : rapproche la matière du centre du pinceau.
    Pucker,
    /// Tourbillon horaire.
    Twirl,
    /// Ramène le champ vers zéro (annule la déformation sous le pinceau).
    Reconstruct,
}

pub struct Liquify {
    base: Buf,
    field: Vec<(f32, f32)>,
    work: TileGrid,
    selection: Option<TileGrid>,
}

impl Liquify {
    pub fn new(base: &TileGrid, selection: Option<TileGrid>) -> Liquify {
        let b = Buf::from_grid(base);
        let field = vec![(0.0, 0.0); b.w * b.h];
        Liquify { base: b, field, work: base.clone(), selection }
    }

    pub fn result(&self) -> &TileGrid {
        &self.work
    }

    /// Un coup de pinceau de `from` vers `to` : rayon `radius` px, intensité `strength` (0..1).
    pub fn brush(
        &mut self,
        mode: LiquifyMode,
        from: (f32, f32),
        to: (f32, f32),
        radius: f32,
        strength: f32,
    ) -> Result<(), CoreError> {
        let radius = if radius.is_finite() { radius.clamp(1.0, 1000.0) } else { 20.0 };
        let strength = if strength.is_finite() { strength.clamp(0.0, 1.0) } else { 1.0 };
        if ![from.0, from.1, to.0, to.1].iter().all(|v| v.is_finite()) {
            return Ok(());
        }
        let (w, h) = (self.base.w as i64, self.base.h as i64);
        let (x0, x1) = (((to.0 - radius).floor() as i64).max(0), ((to.0 + radius).ceil() as i64).min(w - 1));
        let (y0, y1) = (((to.1 - radius).floor() as i64).max(0), ((to.1 + radius).ceil() as i64).min(h - 1));
        let (mx, my) = (to.0 - from.0, to.1 - from.1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (dx, dy) = (x as f32 + 0.5 - to.0, y as f32 + 0.5 - to.1);
                let d = dx.hypot(dy);
                if d >= radius {
                    continue;
                }
                let t = d / radius;
                let falloff = (1.0 - t * t).powi(2);
                let sel =
                    self.selection.as_ref().map_or(1.0, |s| f32::from(selection_value(s, x as u32, y as u32)) / 255.0);
                let k = falloff * strength * sel;
                let f = &mut self.field[(y as usize) * self.base.w + x as usize];
                match mode {
                    // La sortie lit derrière le mouvement : la matière suit le glissé.
                    LiquifyMode::Push => {
                        f.0 -= mx * k;
                        f.1 -= my * k;
                    }
                    LiquifyMode::Bloat => {
                        f.0 -= dx * 0.08 * k;
                        f.1 -= dy * 0.08 * k;
                    }
                    LiquifyMode::Pucker => {
                        f.0 += dx * 0.08 * k;
                        f.1 += dy * 0.08 * k;
                    }
                    LiquifyMode::Twirl => {
                        let a = 0.12 * k;
                        let (rx, ry) = (dx * a.cos() - dy * a.sin(), dx * a.sin() + dy * a.cos());
                        f.0 += rx - dx;
                        f.1 += ry - dy;
                    }
                    LiquifyMode::Reconstruct => {
                        f.0 *= 1.0 - k.min(1.0);
                        f.1 *= 1.0 - k.min(1.0);
                    }
                }
                // Borne le déplacement : un champ qui diverge ne doit pas lire hors de l'image.
                f.0 = f.0.clamp(-2.0 * radius.max(50.0), 2.0 * radius.max(50.0));
                f.1 = f.1.clamp(-2.0 * radius.max(50.0), 2.0 * radius.max(50.0));
            }
        }
        self.refresh(x0, y0, x1, y1);
        Ok(())
    }

    /// Recalcule les pixels du rectangle depuis l'original et le champ.
    fn refresh(&mut self, x0: i64, y0: i64, x1: i64, y1: i64) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (fx, fy) = self.field[(y as usize) * self.base.w + x as usize];
                let sx = x as f32 + 0.5 + fx;
                let sy = y as f32 + 0.5 + fy;
                let p = if sx < 0.0 || sy < 0.0 || sx > self.base.w as f32 || sy > self.base.h as f32 {
                    [0.0; 4]
                } else {
                    self.base.bilinear(sx, sy)
                };
                let a = p[3].clamp(0.0, 1.0);
                let un = |c: f32| if a > 1e-6 { ((c / a).clamp(0.0, 1.0) * 255.0 + 0.5) as u8 } else { 0 };
                self.work.put_pixel(x as u32, y as u32, [un(p[0]), un(p[1]), un(p[2]), (a * 255.0 + 0.5) as u8]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> TileGrid {
        let mut g = TileGrid::new(100, 100).unwrap();
        for y in 40..60 {
            for x in 40..60 {
                g.put_pixel(x, y, [255, 0, 0, 255]);
            }
        }
        g
    }

    fn area(g: &TileGrid) -> usize {
        (0..100).flat_map(|y| (0..100).map(move |x| (x, y))).filter(|&(x, y)| g.get_pixel(x, y)[3] > 127).count()
    }

    #[test]
    fn push_drags_the_material_along_the_stroke() {
        let mut l = Liquify::new(&square(), None);
        for i in 0..10 {
            l.brush(LiquifyMode::Push, (50.0 + i as f32, 50.0), (51.0 + i as f32, 50.0), 25.0, 1.0).unwrap();
        }
        let g = l.result();
        assert!(g.get_pixel(63, 50)[3] > 100, "la matière a avancé à droite (le carré s'arrêtait à x = 59)");
        assert!(g.get_pixel(5, 5)[3] == 0, "loin du pinceau : inchangé");
    }

    #[test]
    fn bloat_grows_and_pucker_shrinks_the_shape() {
        let a0 = area(&square());
        let mut b = Liquify::new(&square(), None);
        let mut p = Liquify::new(&square(), None);
        for _ in 0..12 {
            b.brush(LiquifyMode::Bloat, (50.0, 50.0), (50.0, 50.0), 30.0, 1.0).unwrap();
            p.brush(LiquifyMode::Pucker, (50.0, 50.0), (50.0, 50.0), 30.0, 1.0).unwrap();
        }
        assert!(area(b.result()) > a0 + 20, "gonflé : {} → {}", a0, area(b.result()));
        assert!(area(p.result()) < a0 - 20, "pincé : {} → {}", a0, area(p.result()));
    }

    #[test]
    fn twirl_rotates_and_reconstruct_undoes() {
        let mut l = Liquify::new(&square(), None);
        for _ in 0..12 {
            l.brush(LiquifyMode::Twirl, (50.0, 50.0), (50.0, 50.0), 35.0, 1.0).unwrap();
        }
        assert_ne!(l.result().to_rgba8(), square().to_rgba8(), "la forme tourne");
        for _ in 0..20 {
            l.brush(LiquifyMode::Reconstruct, (50.0, 50.0), (50.0, 50.0), 40.0, 1.0).unwrap();
        }
        let orig = square();
        let diff = (0..100)
            .flat_map(|y| (0..100).map(move |x| (x, y)))
            .filter(|&(x, y)| l.result().get_pixel(x, y)[3].abs_diff(orig.get_pixel(x, y)[3]) > 8)
            .count();
        assert!(diff < 25, "{diff} pixels diffèrent encore de l'original");
    }

    #[test]
    fn the_original_is_untouched_selection_confines_and_hostile_input_is_ignored() {
        let base = square();
        let sel = crate::selection::rect(100, 100, (0.0, 0.0), (45.0, 100.0)).unwrap();
        let mut l = Liquify::new(&base, Some(sel));
        l.brush(LiquifyMode::Push, (60.0, 50.0), (70.0, 50.0), 30.0, 1.0).unwrap();
        assert_eq!(l.result().get_pixel(58, 50), base.get_pixel(58, 50), "hors sélection : inchangé");
        l.brush(LiquifyMode::Push, (f32::NAN, 0.0), (1.0, 1.0), f32::NAN, f32::NAN).unwrap();
        l.brush(LiquifyMode::Push, (-500.0, -500.0), (-490.0, -500.0), 5000.0, 9.0).unwrap();
        assert_eq!(base.get_pixel(50, 50), [255, 0, 0, 255], "le calque d'origine n'est jamais modifié");
    }
}
