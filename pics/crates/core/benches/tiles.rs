// Code de mesure, pas de production : une panne de préparation doit arrêter le benchmark net.
#![allow(clippy::unwrap_used)]

use criterion::{Criterion, criterion_group, criterion_main};
use openpics_core::TileGrid;
use std::hint::black_box;

/// 48 Mpx (8000 × 6000) : l'ordre de grandeur de l'objectif « ouverture d'un 50 Mpx < 2 s ».
const W: u32 = 8000;
const H: u32 = 6000;

fn image() -> Vec<u8> {
    (0..W * H).flat_map(|i| [(i % 251) as u8, (i / 97 % 241) as u8, (i % 13) as u8, 255]).collect()
}

fn tiles(c: &mut Criterion) {
    let rgba = image();
    let mut group = c.benchmark_group("tiles 48 Mpx");
    group.sample_size(10);
    group.bench_function("from_rgba8 (découpage)", |b| {
        b.iter(|| TileGrid::from_rgba8(W, H, black_box(&rgba)).unwrap())
    });
    let grid = TileGrid::from_rgba8(W, H, &rgba).unwrap();
    group.bench_function("to_rgba8 (aplatissement)", |b| b.iter(|| black_box(&grid).to_rgba8()));
    group.bench_function("clone + changed_tiles (instantané d'historique)", |b| {
        b.iter(|| {
            let mut copy = black_box(&grid).clone();
            copy.put_pixel(10, 10, [1, 2, 3, 255]);
            grid.changed_tiles(&copy).len()
        })
    });
    group.finish();
}

criterion_group!(benches, tiles);
criterion_main!(benches);
