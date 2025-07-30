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
use visualvault_core::DatabaseCache;
use visualvault_core::{FileOrganizer, Scanner};
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

async fn create_test_scanner() -> Result<Scanner> {
    let database_cache = DatabaseCache::new(":memory:")
        .await
        .expect("Failed to initialize database cache");
    let scanner = Scanner::new(database_cache);
    Ok(scanner)
}

#[tokio::test]
async fn test_organizer_source_equals_destination() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // Create test files
    create_test_file(&root.join("photo1.jpg"), b"PHOTO1", 1024 * 1024).await?;
    create_test_file(&root.join("photo2.jpg"), b"PHOTO2", 1024 * 1024).await?;
    create_test_file(&root.join("video.mp4"), b"VIDEO", 5 * 1024 * 1024).await?;

    let settings = Settings {
        source_folder: Some(root.to_path_buf()),
        destination_folder: Some(root.to_path_buf()), // Same as source!
        recurse_subfolders: true,
        organize_by: "Monthly".to_string(),
        ..Default::default()
    };

    let scanner = create_test_scanner().await?;
    let progress = Arc::new(RwLock::new(Progress::default()));

    // Scanning should work fine even when source equals destination
    let (files, dups) = scanner
        .scan_directory_with_duplicates(root, true, progress.clone(), &settings, None)
        .await?;

    assert_eq!(files.len(), 3, "Should find all 3 files");

    // Verify scanner doesn't create any issues
    // Files should still exist and be readable
    for file in &files {
        assert!(file.path.exists(), "File should still exist after scan");
        let content = fs::read(&file.path).await?;
        assert!(!content.is_empty(), "File should have content");
    }

    let organizer = FileOrganizer::new(temp_dir.path().to_path_buf()).await?;
    let result = organizer
        .organize_files_with_duplicates(files, dups, &settings, progress)
        .await
        .unwrap();

    // Ensure no files were moved since source equals destination
    assert!(
        result.success,
        "Organizing should succeed even with same source/destination"
    );
    assert!(
        result.errors.is_empty(),
        "No errors should occur when source equals destination"
    );

    // Verify that files are organized correctly
    assert_eq!(result.files_organized, 3, "Should organize all 3 files");
    assert_eq!(result.files_total, 3, "Total files should still be 3");

    let current_month = chrono::Local::now().format("%m-%B").to_string();
    let current_year = chrono::Local::now().format("%Y").to_string();
    let expected_destination = root.join(current_year).join(current_month);
    assert!(expected_destination.exists(), "Destination folder should exist");

    Ok(())
}

#[tokio::test]
async fn test_organize_by_type_with_separate_videos() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let source = temp_dir.path().join("source");
    let dest = temp_dir.path().join("organized");

    // Create mixed media files
    create_test_file(&source.join("vacation.jpg"), b"VACATION_JPG", 2 * 1024 * 1024).await?;
    create_test_file(&source.join("portrait.png"), b"PORTRAIT_PNG", 1024 * 1024).await?;
    create_test_file(&source.join("screenshot.bmp"), b"SCREENSHOT", 512 * 1024).await?;
    create_test_file(&source.join("movie.mp4"), b"MOVIE_MP4", 50 * 1024 * 1024).await?;
    create_test_file(&source.join("clip.avi"), b"CLIP_AVI", 20 * 1024 * 1024).await?;
    create_test_file(&source.join("raw_photo.raw"), b"RAW_DATA", 10 * 1024 * 1024).await?;

    let settings = Settings {
        source_folder: Some(source.clone()),
        destination_folder: Some(dest.clone()),
        organize_by: "type".to_string(),
        separate_videos: true,
        recurse_subfolders: true,
        ..Default::default()
    };

    // First scan the files
    let scanner = create_test_scanner().await?;
    let progress = Arc::new(RwLock::new(Progress::default()));
    let (files, dups) = scanner
        .scan_directory_with_duplicates(&source, true, progress.clone(), &settings, None)
        .await?;

    assert_eq!(files.len(), 6, "Should find all 6 media files");

    // Now organize them using FileOrganizer
    let organizer = FileOrganizer::new(temp_dir.path().to_path_buf()).await?;
    let result = organizer
        .organize_files_with_duplicates(files, dups, &settings, progress)
        .await?;

    assert!(result.success, "Organization should succeed");
    assert_eq!(result.files_organized, 6, "Should organize all 6 files");

    // Verify the directory structure
    // When organize_by = "type" and separate_videos = true:
    // - Images go to: organized/Photos/
    // - Videos go to: organized/Videos/
    // - Screenshots go to: organized/Screenshots/ (if separate_screenshots = true)
    // - Raw files go to: organized/Photos/RAW/

    // Check Photos directory
    let photos_dir = dest.join("Images");
    assert!(photos_dir.exists(), "Images directory should exist");
    assert!(
        photos_dir.join("vacation.jpg").exists(),
        "vacation.jpg should be in Images"
    );
    assert!(
        photos_dir.join("portrait.png").exists(),
        "portrait.png should be in Images"
    );
    assert!(
        photos_dir.join("screenshot.bmp").exists(),
        "screenshot.bmp should be in Images"
    );
    assert!(
        photos_dir.join("raw_photo.raw").exists(),
        "RAW subdirectory should exist under Images"
    );

    // Check Videos directory (separate from Photos due to separate_videos = true)
    let videos_dir = dest.join("Videos");
    assert!(videos_dir.exists(), "Videos directory should exist");
    assert!(videos_dir.join("movie.mp4").exists(), "movie.mp4 should be in Videos");
    assert!(videos_dir.join("clip.avi").exists(), "clip.avi should be in Videos");

    Ok(())
}

#[tokio::test]
async fn test_organize_by_type_without_separate_videos() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let source = temp_dir.path().join("source");
    let dest = temp_dir.path().join("organized");

    // Create mixed media files
    create_test_file(&source.join("vacation.jpg"), b"VACATION_JPG", 2 * 1024 * 1024).await?;
    create_test_file(&source.join("portrait.png"), b"PORTRAIT_PNG", 1024 * 1024).await?;
    create_test_file(&source.join("screenshot.bmp"), b"SCREENSHOT", 512 * 1024).await?;
    create_test_file(&source.join("movie.mp4"), b"MOVIE_MP4", 50 * 1024 * 1024).await?;
    create_test_file(&source.join("clip.avi"), b"CLIP_AVI", 20 * 1024 * 1024).await?;
    create_test_file(&source.join("raw_photo.raw"), b"RAW_DATA", 10 * 1024 * 1024).await?;

    let settings = Settings {
        source_folder: Some(source.clone()),
        destination_folder: Some(dest.clone()),
        organize_by: "type".to_string(),
        separate_videos: false,
        recurse_subfolders: true,
        ..Default::default()
    };

    // First scan the files
    let scanner = create_test_scanner().await?;
    let progress = Arc::new(RwLock::new(Progress::default()));
    let (files, dups) = scanner
        .scan_directory_with_duplicates(&source, true, progress.clone(), &settings, None)
        .await?;

    assert_eq!(files.len(), 6, "Should find all 6 media files");

    // Now organize them using FileOrganizer
    let organizer = FileOrganizer::new(temp_dir.path().to_path_buf()).await?;
    let result = organizer
        .organize_files_with_duplicates(files, dups, &settings, progress)
        .await?;

    assert!(result.success, "Organization should succeed");
    assert_eq!(result.files_organized, 6, "Should organize all 6 files");

    // Verify the directory structure
    // When organize_by = "type" and separate_videos = true:
    // - Images go to: organized/Photos/
    // - Videos go to: organized/Videos/
    // - Screenshots go to: organized/Screenshots/ (if separate_screenshots = true)
    // - Raw files go to: organized/Photos/RAW/

    // Check Photos directory
    let photos_dir = dest.join("Images");
    assert!(photos_dir.exists(), "Images directory should exist");
    assert!(
        photos_dir.join("vacation.jpg").exists(),
        "vacation.jpg should be in Images"
    );
    assert!(
        photos_dir.join("portrait.png").exists(),
        "portrait.png should be in Images"
    );
    assert!(
        photos_dir.join("screenshot.bmp").exists(),
        "screenshot.bmp should be in Images"
    );
    assert!(
        photos_dir.join("raw_photo.raw").exists(),
        "RAW subdirectory should exist under Images"
    );

    // Check Videos directory (separate from Photos due to separate_videos = true)
    let videos_dir = dest.join("Videos");
    assert!(videos_dir.exists(), "Videos directory should exist");
    assert!(videos_dir.join("movie.mp4").exists(), "movie.mp4 should be in Videos");
    assert!(videos_dir.join("clip.avi").exists(), "clip.avi should be in Videos");

    Ok(())
}
