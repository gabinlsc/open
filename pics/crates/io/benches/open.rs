use criterion::{Criterion, criterion_group, criterion_main};
use openpics_core::{DocId, Document, Size};
use openpics_io::{ExportOptions, PngCompression};
use std::hint::black_box;

/// Ouverture d'un PNG de 48 Mpx, à comparer à l'objectif « < 2 s pour 50 Mpx ».
const W: u32 = 8000;
const H: u32 = 6000;

fn open(c: &mut Criterion) {
    let dir = std::env::temp_dir().join("openpics-bench");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("48mpx.png");
    if !path.exists() {
        let mut doc = Document::new(DocId(1), "bench", Size::new(W, H)).unwrap();
        let rgba: Vec<u8> =
            (0..W * H).flat_map(|i| [(i % 251) as u8, (i / 97 % 241) as u8, (i % 13) as u8, 255]).collect();
        doc.layers[0].pixels = openpics_core::TileGrid::from_rgba8(W, H, &rgba).unwrap();
        let options = ExportOptions { png_compression: PngCompression::Fast, ..ExportOptions::default() };
        openpics_io::export(&doc, &path, &options).unwrap();
    }
    let mut group = c.benchmark_group("io 48 Mpx");
    group.sample_size(10);
    group.bench_function("ouverture PNG", |b| b.iter(|| openpics_io::open(black_box(&path), DocId(2)).unwrap()));
    group.finish();
}

criterion_group!(benches, open);
criterion_main!(benches);
