#![allow(clippy::panic)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic_in_result_fn)]

use color_eyre::Result;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs;
use tokio::sync::RwLock;

use visualvault_config::Settings;
use visualvault_core::{DuplicateDetector, Scanner};
use visualvault_models::FileType;
use visualvault_utils::Progress;

/// Create a test file with specific content and size
async fn create_test_file(path: &Path, content: &[u8], size: usize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let mut data = content.to_vec();
    data.resize(size, 0);
    fs::write(path, &data).await?;
    Ok(())
}

/// Create a test media file structure
async fn setup_test_files(root: &Path) -> Result<()> {
    // Create various media files
    create_test_file(&root.join("photos/vacation/beach.jpg"), b"JPG_DATA", 1024 * 1024).await?;
    create_test_file(&root.join("photos/vacation/sunset.jpg"), b"JPG_DATA_2", 1024 * 1024).await?;
    create_test_file(&root.join("photos/family/birthday.png"), b"PNG_DATA", 2 * 1024 * 1024).await?;
    create_test_file(&root.join("videos/holiday.mp4"), b"MP4_DATA", 10 * 1024 * 1024).await?;
    create_test_file(&root.join("videos/wedding.avi"), b"AVI_DATA", 20 * 1024 * 1024).await?;
    //create_test_file(&root.join("documents/report.pdf"), b"PDF_DATA", 512 * 1024).await?;

    // Create some duplicate files
    create_test_file(&root.join("duplicates/beach_copy.jpg"), b"JPG_DATA", 1024 * 1024).await?;
    create_test_file(&root.join("duplicates/beach_backup.jpg"), b"JPG_DATA", 1024 * 1024).await?;

    // Create hidden files
    create_test_file(&root.join(".hidden/secret.jpg"), b"SECRET", 1024).await?;

    Ok(())
}

async fn create_test_scanner() -> Result<Scanner> {
    let scanner = Scanner::new();
    scanner.init_in_memory_cache().await?;
    Ok(scanner)
}

#[tokio::test]
async fn test_scanner_finds_all_media_files() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();
    setup_test_files(root).await?;

    let settings = Settings {
        recurse_subfolders: true,
        skip_hidden_files: false,
        ..Default::default()
    };

    let scanner = create_test_scanner().await?;
    let progress = Arc::new(RwLock::new(Progress::default()));

    let files = scanner.scan_directory(root, true, progress, &settings, None).await?;

    // Should find all files including hidden
    assert_eq!(files.len(), 8, "Should find 8 files total");

    // Verify file types
    let images: Vec<_> = files
        .iter()
        .filter(|f| matches!(f.file_type, FileType::Image))
        .collect();
    let videos: Vec<_> = files
        .iter()
        .filter(|f| matches!(f.file_type, FileType::Video))
        .collect();

    assert_eq!(
        images.len(),
        6,
        "Should find 6 images (including duplicates and hidden)"
    );
    assert_eq!(videos.len(), 2, "Should find 2 videos");

    Ok(())
}

#[tokio::test]
async fn test_scanner_respects_hidden_files_setting() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();
    setup_test_files(root).await?;

    let settings = Settings {
        recurse_subfolders: true,
        skip_hidden_files: true,
        ..Default::default()
    };

    let scanner = create_test_scanner().await?;
    let progress = Arc::new(RwLock::new(Progress::default()));

    let files = scanner.scan_directory(root, true, progress, &settings, None).await?;

    // Should not find hidden files
    assert_eq!(files.len(), 0, "Should find 0 files (excluding hidden)");
    assert!(!files.iter().any(|f| f.path.to_string_lossy().contains(".hidden")));

    Ok(())
}

#[tokio::test]
async fn test_duplicate_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();
    setup_test_files(root).await?;

    let settings = Settings::default();
    let scanner = create_test_scanner().await?;
    let progress = Arc::new(RwLock::new(Progress::default()));

    // Scan for files and duplicates
    let (files, duplicates) = scanner
        .scan_directory_with_duplicates(root, true, progress.clone(), &settings, None)
        .await?;

    // Should find duplicate groups
    assert!(!duplicates.is_empty(), "Should find duplicates");

    // Verify duplicate group
    let duplicate_count: usize = duplicates.total_files();
    assert_eq!(duplicate_count, 3, "Should find 3 files in duplicate groups");

    // Use DuplicateDetector directly
    let detector = DuplicateDetector::new();
    let stats = detector.detect_duplicates(&files, false).await?;

    assert_eq!(stats.groups.len(), 1, "Should find 1 duplicate group");
    assert_eq!(stats.total_duplicates, 2, "Should find 2 duplicate files");
    assert_eq!(stats.total_wasted_space, 2 * 1024 * 1024, "Should waste 2MB");

    Ok(())
}

#[tokio::test]
async fn test_scanner_non_recursive_scan() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // Create files in root and subdirectories
    create_test_file(&root.join("root_photo.jpg"), b"ROOT_JPG", 1024 * 1024).await?;
    create_test_file(&root.join("root_video.mp4"), b"ROOT_MP4", 2 * 1024 * 1024).await?;
    create_test_file(&root.join("subdir/nested_photo.jpg"), b"NESTED_JPG", 1024 * 1024).await?;
    create_test_file(&root.join("subdir/deep/very_nested.png"), b"DEEP_PNG", 512 * 1024).await?;

    let settings = Settings {
        recurse_subfolders: false, // This is the key setting
        skip_hidden_files: false,
        ..Default::default()
    };

    let scanner = create_test_scanner().await?;
    let progress = Arc::new(RwLock::new(Progress::default()));

    // Use recursive=false in scan_directory
    let files = scanner.scan_directory(root, false, progress, &settings, None).await?;

    // Should only find files in root directory
    assert_eq!(files.len(), 2, "Should only find 2 files in root directory");

    // Verify all files are in root (no subdirectory files)
    for file in &files {
        assert_eq!(
            file.path.parent().unwrap(),
            root,
            "All files should be directly in root directory"
        );
    }

    // Verify we got the expected files
    let file_names: Vec<String> = files.iter().map(|f| f.name.to_string()).collect();
    assert!(file_names.contains(&"root_photo.jpg".to_string()));
    assert!(file_names.contains(&"root_video.mp4".to_string()));
    assert!(!file_names.contains(&"nested_photo.jpg".to_string()));
    assert!(!file_names.contains(&"very_nested.png".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_scanner_handles_empty_directories() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // Create empty directories
    fs::create_dir_all(root.join("empty1")).await?;
    fs::create_dir_all(root.join("empty2/nested/deep")).await?;

    let settings = Settings {
        recurse_subfolders: true,
        ..Default::default()
    };

    let scanner = create_test_scanner().await?;
    let progress = Arc::new(RwLock::new(Progress::default()));

    let (files, dups) = scanner
        .scan_directory_with_duplicates(root, true, progress, &settings, None)
        .await?;

    // Should find no files in empty directories
    assert_eq!(files.len(), 0, "Should find no files in empty directories");
    assert!(dups.is_empty(), "Should find no duplicates in empty directories");

    Ok(())
}
