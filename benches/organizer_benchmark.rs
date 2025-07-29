#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::float_cmp)] // For comparing floats in tests

use chrono::Local;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::sync::Arc;
use std::{path::PathBuf};
use tempfile::TempDir;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use visualvault_config::Settings;
use visualvault_core::FileOrganizer;
use visualvault_models::{DuplicateStats, FileType, MediaFile};
use visualvault_utils::Progress;
use color_eyre::eyre::Result;

fn create_test_media_files(count: usize) -> Vec<Arc<MediaFile>> {
    (0..count)
        .map(|i| {
            Arc::new(MediaFile {
                path: PathBuf::from(format!("/tmp/test_{i:04}.jpg")),
                name: Arc::from(format!("test_{i:04}.jpg")),
                extension: Arc::from("jpg"),
                file_type: FileType::Image,
                size: 1024 * 1024, // 1MB
                modified: Local::now(),
                created: Local::now(),
                metadata: None,
                hash: None,
            })
        })
        .collect()
}

async fn run_organize(
    dest: &std::path::Path,
    files: Vec<Arc<MediaFile>>,
    settings: Settings,
) -> Result<usize> {
    let organizer = FileOrganizer::new(dest.to_path_buf()).await?;

    // Для тесту передаємо пусті duplicates
    let duplicates = DuplicateStats::default();
    let progress = Arc::new(RwLock::new(Progress::default()));

    let result = organizer
        .organize_files_with_duplicates(files, duplicates, &settings, progress)
        .await?;

    Ok(result.files_organized)
}

fn benchmark_organize_by_type(c: &mut Criterion, rt: &Runtime) {
    let mut group = c.benchmark_group("FileOrganizer::organize_by_type");
    group.sample_size(10);

    for &file_count in &[100usize, 500, 1000] {
        group.bench_with_input(BenchmarkId::new("files", file_count), &file_count, |b, &file_count| {
            b.iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let files = create_test_media_files(file_count);
                    let settings = Settings {
                        destination_folder: Some(temp_dir.path().to_path_buf()),
                        organize_by: "type".to_string(),
                        ..Default::default()
                    };
                    (temp_dir, files, settings)
                },
                |(temp_dir, files, settings)| {
                    rt.block_on(run_organize(temp_dir.path(), files, settings))
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn benchmark_organize_modes(c: &mut Criterion, rt: &Runtime) {
    let mut group = c.benchmark_group("FileOrganizer::organize_modes");
    group.sample_size(10);

    let files = Arc::new(create_test_media_files(1000));

    let modes = vec!["yearly", "monthly", "type"];

    for mode in modes {
        group.bench_with_input(BenchmarkId::new("mode", mode), &mode, |b, &mode| {
            let files = files.clone();
            b.iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let settings = Settings {
                        destination_folder: Some(temp_dir.path().to_path_buf()),
                        organize_by: mode.to_string(),
                        ..Default::default()
                    };
                    (temp_dir, (*files).clone(), settings)  // <- РОЗПАКОВКА Arc
                },
                |(temp_dir, files, settings)| {
                    rt.block_on(run_organize(temp_dir.path(), files, settings))
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn run_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    benchmark_organize_by_type(c, &rt);
    benchmark_organize_modes(c, &rt);
}

criterion_group!(benches, run_benchmarks);
criterion_main!(benches);

