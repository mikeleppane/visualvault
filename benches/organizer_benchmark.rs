#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::float_cmp)] // For comparing floats in tests
#![allow(clippy::panic)]
#![allow(clippy::significant_drop_tightening)]
use chrono::Local;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::sync::Arc;
use std::{hint::black_box, path::PathBuf};
use tempfile::TempDir;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use visualvault_config::Settings;
use visualvault_core::FileOrganizer;
use visualvault_models::{DuplicateStats, FileType, MediaFile};
use visualvault_utils::Progress;

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

fn benchmark_organize_by_type(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("organize_by_type");
    group.sample_size(10);

    for file_count in &[100, 500, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(file_count), file_count, |b, &file_count| {
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
                    rt.block_on(async {
                        let organizer = FileOrganizer::new(temp_dir.path().to_path_buf()).await.unwrap();
                        let progress = Arc::new(RwLock::new(Progress::default()));

                        organizer
                            .organize_files_with_duplicates(
                                black_box(files),
                                DuplicateStats::new(),
                                &settings,
                                progress,
                            )
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

fn benchmark_organize_modes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("organize_modes");
    group.sample_size(10);

    let modes = vec!["yearly", "monthly", "type"];
    let files = create_test_media_files(1000);

    for mode in modes {
        group.bench_with_input(BenchmarkId::from_parameter(mode), &mode, |b, &mode| {
            b.iter_batched(
                || {
                    let temp_dir = TempDir::new().unwrap();
                    let settings = Settings {
                        destination_folder: Some(temp_dir.path().to_path_buf()),
                        organize_by: mode.to_string(),
                        ..Default::default()
                    };
                    (temp_dir, files.clone(), settings)
                },
                |(temp_dir, files, settings)| {
                    rt.block_on(async {
                        let organizer = FileOrganizer::new(temp_dir.path().to_path_buf()).await.unwrap();
                        let progress = Arc::new(RwLock::new(Progress::default()));

                        organizer
                            .organize_files_with_duplicates(
                                black_box(files),
                                DuplicateStats::new(),
                                &settings,
                                progress,
                            )
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

criterion_group!(benches, benchmark_organize_by_type, benchmark_organize_modes);
criterion_main!(benches);
