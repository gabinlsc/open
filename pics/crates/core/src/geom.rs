use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Rectangle entier ; `w`/`h` nuls = vide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl IRect {
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    /// Bornes exclusives en i64 pour éviter tout débordement sur de grandes images.
    fn bounds(&self) -> (i64, i64, i64, i64) {
        let (x, y) = (i64::from(self.x), i64::from(self.y));
        (x, y, x + i64::from(self.w), y + i64::from(self.h))
    }

    pub fn intersect(&self, other: &IRect) -> Option<IRect> {
        let (ax0, ay0, ax1, ay1) = self.bounds();
        let (bx0, by0, bx1, by1) = other.bounds();
        let (x0, y0, x1, y1) = (ax0.max(bx0), ay0.max(by0), ax1.min(bx1), ay1.min(by1));
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some(IRect {
            x: i32::try_from(x0).ok()?,
            y: i32::try_from(y0).ok()?,
            w: u32::try_from(x1 - x0).ok()?,
            h: u32::try_from(y1 - y0).ok()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersect_overlap_and_disjoint() {
        let a = IRect::new(0, 0, 10, 10);
        assert_eq!(a.intersect(&IRect::new(5, 5, 10, 10)), Some(IRect::new(5, 5, 5, 5)));
        assert_eq!(a.intersect(&IRect::new(10, 0, 5, 5)), None);
        assert_eq!(a.intersect(&IRect::new(-20, -20, 5, 5)), None);
    }
}
