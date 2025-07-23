#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::float_cmp)] // For comparing floats in tests
#![allow(clippy::panic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
use chrono::Local;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::{hint::black_box, path::PathBuf};
use visualvault::{
    core::DuplicateDetector,
    models::{FileType, MediaFile},
};

fn create_test_files_with_duplicates(total: usize, duplicate_ratio: f32) -> Vec<MediaFile> {
    let unique_count = ((total as f32) * (1.0 - duplicate_ratio)) as usize;
    let mut files = Vec::with_capacity(total);

    // Create unique files
    for i in 0..unique_count {
        files.push(MediaFile {
            path: PathBuf::from(format!("/tmp/unique_{i:04}.jpg")),
            name: format!("unique_{i:04}.jpg"),
            extension: "jpg".to_string(),
            file_type: FileType::Image,
            size: 1024 * 1024, // 1MB
            modified: Local::now(),
            created: Local::now(),
            metadata: None,
            hash: Some(format!("hash_{i:04}")),
        });
    }

    // Create duplicates
    let remaining = total - unique_count;
    for i in 0..remaining {
        let original_idx = i % unique_count;
        let mut duplicate = files[original_idx].clone();
        duplicate.path = PathBuf::from(format!("/tmp/duplicate_{i:04}.jpg"));
        duplicate.name = format!("duplicate_{i:04}.jpg");
        files.push(duplicate);
    }

    files
}

fn benchmark_duplicate_detection_without_quick_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("duplicate_detection");
    group.sample_size(10);

    for file_count in &[1000, 5000, 10000] {
        group.bench_with_input(BenchmarkId::from_parameter(file_count), file_count, |b, &file_count| {
            let files = create_test_files_with_duplicates(file_count, 0.3); // 30% duplicates
            let detector = DuplicateDetector::new();
            b.iter(|| detector.detect_duplicates(black_box(&files), false));
        });
    }

    group.finish();
}

fn benchmark_duplicate_detection_with_quick_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("duplicate_detection");
    group.sample_size(10);

    for file_count in &[1000, 5000, 10000] {
        group.bench_with_input(BenchmarkId::from_parameter(file_count), file_count, |b, &file_count| {
            let files = create_test_files_with_duplicates(file_count, 0.3); // 30% duplicates
            let detector = DuplicateDetector::new();
            b.iter(|| detector.detect_duplicates(black_box(&files), true));
        });
    }

    group.finish();
}

fn benchmark_duplicate_ratios(c: &mut Criterion) {
    let mut group = c.benchmark_group("duplicate_ratios");
    group.sample_size(10);

    let file_count = 10000;
    for ratio in &[0.1, 0.3, 0.5, 0.7] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}%", (ratio * 100.0) as u32)),
            ratio,
            |b, &ratio| {
                let files = create_test_files_with_duplicates(file_count, ratio);
                let detector = DuplicateDetector::new();
                b.iter(|| detector.detect_duplicates(black_box(&files), false));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_duplicate_detection_without_quick_hash,
    benchmark_duplicate_detection_with_quick_hash,
    benchmark_duplicate_ratios
);
criterion_main!(benches);
