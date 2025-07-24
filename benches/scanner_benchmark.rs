#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::float_cmp)] // For comparing floats in tests
#![allow(clippy::panic)]
#![allow(clippy::significant_drop_tightening)]
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::fs;
use std::hint::black_box;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use visualvault::config::settings::Settings;
use visualvault::core::Scanner;
use visualvault::utils::Progress;

fn create_test_files(dir: &Path, count: usize) {
    for i in 0..count {
        let file_path = dir.join(format!("test_{i:04}.jpg"));
        fs::write(&file_path, b"fake image data").unwrap();
    }
}

fn benchmark_scanner(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("scanner");
    group.sample_size(10);

    for file_count in &[100, 1000, 5000] {
        group.bench_with_input(BenchmarkId::from_parameter(file_count), file_count, |b, &file_count| {
            b.iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    create_test_files(temp_dir.path(), file_count);
                    temp_dir
                },
                |temp_dir| {
                    rt.block_on(async {
                        let scanner = Scanner::new();
                        let progress = Arc::new(RwLock::new(Progress::default()));
                        let settings = Settings::default();

                        scanner
                            .scan_directory(black_box(temp_dir.path()), false, progress, &settings, None)
                            .await
                            .unwrap()
                    })
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn benchmark_scanner_parallel(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("scanner_parallel");
    group.sample_size(10);

    let temp_dir = TempDir::new().unwrap();
    create_test_files(temp_dir.path(), 5000);

    for thread_count in &[1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let scanner = Scanner::new();
                        let progress = Arc::new(RwLock::new(Progress::default()));
                        let settings = Settings {
                            parallel_processing: true,
                            worker_threads: thread_count,
                            ..Default::default()
                        };

                        scanner
                            .scan_directory(black_box(temp_dir.path()), false, progress, &settings, None)
                            .await
                            .unwrap()
                    })
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_scanner, benchmark_scanner_parallel);
criterion_main!(benches);
