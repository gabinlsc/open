// Code de mesure, pas de production : une panne de préparation doit arrêter le benchmark net.
#![allow(clippy::unwrap_used)]

use criterion::{Criterion, criterion_group, criterion_main};
use openpics_core::TileGrid;
use openpics_ops::Op;
use std::hint::black_box;

/// 24 Mpx (6000 × 4000), la taille de référence des objectifs de performance.
const W: u32 = 6000;
const H: u32 = 4000;

fn ops(c: &mut Criterion) {
    let rgba: Vec<u8> = (0..W * H).flat_map(|i| [(i % 251) as u8, (i / 97 % 241) as u8, (i % 13) as u8, 255]).collect();
    let grid = TileGrid::from_rgba8(W, H, &rgba).unwrap();
    let mut group = c.benchmark_group("ops 24 Mpx");
    group.sample_size(10);
    for op in [Op::FlipHorizontal, Op::FlipVertical, Op::Invert] {
        group.bench_function(op.label(), |b| b.iter(|| op.run(black_box(&grid)).unwrap()));
    }
    group.finish();
}

criterion_group!(benches, ops);
criterion_main!(benches);
