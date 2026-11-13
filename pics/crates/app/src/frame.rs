//! Image figée du canvas : quand un menu ou une boîte de dialogue HTML s'ouvre, la fenêtre native (toujours
//! au-dessus du webview) doit être cachée ; l'interface affiche à sa place la dernière image rendue.
//!
//! Format binaire envoyé au frontend : largeur et hauteur en `u32` petit-boutiste, puis les pixels RGBA.

pub fn encode(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || rgba.len() != width as usize * height as usize * 4 {
        return None;
    }
    let mut out = Vec::with_capacity(8 + rgba.len());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(rgba);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_then_pixels_and_inconsistent_sizes_are_refused() {
        let px = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let e = encode(2, 1, &px).unwrap();
        assert_eq!(&e[..8], &[2, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(&e[8..], &px);
        assert!(encode(3, 1, &px).is_none());
        assert!(encode(0, 1, &[]).is_none());
    }
}
