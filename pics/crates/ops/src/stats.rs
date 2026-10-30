//! Statistiques de couleur d'un calque, pour la correspondance de couleur.

use openpics_core::{TILE_SIZE, TileGrid};

/// Moyenne et écart-type par canal (0..1), pondérés par l'alpha. `None` si le calque est vide.
pub fn color_stats(g: &TileGrid) -> Option<([f32; 3], [f32; 3])> {
    let (mut sum, mut sum2, mut weight) = ([0.0f64; 3], [0.0f64; 3], 0.0f64);
    for (key, tile) in g.iter() {
        let (ox, oy) = (key.tx * TILE_SIZE, key.ty * TILE_SIZE);
        for ly in 0..TILE_SIZE.min(g.height().saturating_sub(oy)) {
            for lx in 0..TILE_SIZE.min(g.width().saturating_sub(ox)) {
                let p = tile.pixel(lx, ly);
                let a = f64::from(p[3]) / 255.0;
                if a <= 0.0 {
                    continue;
                }
                for c in 0..3 {
                    let v = f64::from(p[c]) / 255.0;
                    sum[c] += v * a;
                    sum2[c] += v * v * a;
                }
                weight += a;
            }
        }
    }
    if weight <= 0.0 {
        return None;
    }
    let mean: [f64; 3] = std::array::from_fn(|c| sum[c] / weight);
    let std: [f64; 3] = std::array::from_fn(|c| (sum2[c] / weight - mean[c] * mean[c]).max(0.0).sqrt());
    Some((mean.map(|v| v as f32), std.map(|v| (v as f32).max(0.001))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_and_deviation_of_a_two_tone_layer() {
        let mut g = TileGrid::new(4, 1).unwrap();
        for x in 0..4 {
            let v = if x < 2 { 0 } else { 255 };
            g.put_pixel(x, 0, [v, v, v, 255]);
        }
        let (mean, std) = color_stats(&g).unwrap();
        assert!((mean[0] - 0.5).abs() < 1e-3 && (std[0] - 0.5).abs() < 1e-3);
        assert!(color_stats(&TileGrid::new(4, 1).unwrap()).is_none());
    }

    #[test]
    fn transparent_pixels_do_not_count_and_flat_layers_get_a_floor_deviation() {
        let mut g = TileGrid::new(4, 1).unwrap();
        g.put_pixel(0, 0, [255, 0, 0, 255]);
        g.put_pixel(1, 0, [0, 255, 0, 0]);
        let (mean, std) = color_stats(&g).unwrap();
        assert!((mean[0] - 1.0).abs() < 1e-3 && mean[1] < 1e-3);
        assert!(std[0] >= 0.001, "jamais zéro : on divise par cet écart-type");
    }
}
