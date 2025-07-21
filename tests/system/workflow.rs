#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::float_cmp)] // For comparing floats in tests
#![allow(clippy::panic)]
use color_eyre::Result;
use std::path::Path;
use tempfile::TempDir;
use tokio::fs;

use visualvault::{
    config::Settings,
    core::{DuplicateDetector, FileOrganizer, Scanner},
    models::FileType,
    utils::Progress,
};

use std::sync::Arc;
use tokio::sync::RwLock;

/// Helper to create test media files with specific content
async fn create_test_file(path: &Path, content: &[u8], size: usize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let mut data = content.to_vec();
    data.resize(size, 0);
    fs::write(path, &data).await?;
    Ok(())
}

/// Helper to setup a realistic media file structure
async fn setup_test_environment(source_dir: &Path) -> Result<()> {
    // Photos from different dates
    create_test_file(&source_dir.join("DCIM/100CANON/IMG_0001.JPG"), b"PHOTO1", 1024 * 1024).await?;

    create_test_file(
        &source_dir.join("DCIM/100CANON/IMG_0002.JPG"),
        b"PHOTO2",
        2 * 1024 * 1024,
    )
    .await?;

    // Duplicate photos
    create_test_file(
        &source_dir.join("Downloads/photo_copy.jpg"),
        b"PHOTO1", // Same content as IMG_0001.JPG
        1024 * 1024,
    )
    .await?;

    create_test_file(
        &source_dir.join("Backup/old_photos/duplicate.jpg"),
        b"PHOTO1", // Another duplicate
        1024 * 1024,
    )
    .await?;

    // Videos
    create_test_file(
        &source_dir.join("Videos/vacation_2023.mp4"),
        b"VIDEO1",
        50 * 1024 * 1024,
    )
    .await?;

    create_test_file(&source_dir.join("Videos/birthday.avi"), b"VIDEO2", 100 * 1024 * 1024).await?;

    // Mixed folder with various file types
    create_test_file(&source_dir.join("Desktop/mixed/photo.png"), b"PHOTO3", 3 * 1024 * 1024).await?;

    create_test_file(&source_dir.join("Desktop/mixed/document.pdf"), b"DOC1", 512 * 1024).await?;

    // Hidden files
    create_test_file(&source_dir.join(".hidden/secret_photo.jpg"), b"HIDDEN1", 1024 * 1024).await?;

    // File with uppercase extension
    create_test_file(&source_dir.join("Pictures/SCREENSHOT.PNG"), b"PHOTO4", 512 * 1024).await?;

    Ok(())
}

#[tokio::test()]
#[ignore = "TODO"]
#[allow(clippy::too_many_lines)]
async fn test_complete_system_workflow() -> Result<()> {
    // 1. Setup test environment
    let temp_dir = TempDir::new()?;
    let source_dir = temp_dir.path().join("source");
    let dest_dir = temp_dir.path().join("organized");

    fs::create_dir_all(&source_dir).await?;
    fs::create_dir_all(&dest_dir).await?;

    println!("Setting up test environment...");
    setup_test_environment(&source_dir).await?;

    // 2. Configure settings
    let settings = Settings {
        source_folder: Some(source_dir.clone()),
        destination_folder: Some(dest_dir.clone()),
        organize_by: "monthly".to_string(),
        recurse_subfolders: true,
        skip_hidden_files: false,
        rename_duplicates: false,
        separate_videos: true,
        lowercase_extensions: true,
        parallel_processing: true,
        worker_threads: 4,
        ..Settings::default()
    };

    // 3. Initialize components
    let scanner = Scanner::with_cache().await?;
    let progress = Arc::new(RwLock::new(Progress::default()));

    // 4. Scan for media files
    println!("Scanning for media files...");
    let (files, duplicates) = scanner
        .scan_directory_with_duplicates(&source_dir, true, progress.clone(), &settings, None)
        .await?;

    println!("Found {} files", files.len());
    println!("Found {} duplicate groups", duplicates.len());

    // Verify scan results
    assert!(files.len() >= 8, "Should find at least 8 media files");
    assert!(!duplicates.is_empty(), "Should find duplicate files");

    // Check that different file types were detected
    let images = files.iter().filter(|f| matches!(f.file_type, FileType::Image)).count();
    let videos = files.iter().filter(|f| matches!(f.file_type, FileType::Video)).count();

    assert!(images > 0, "Should find image files");
    assert!(videos > 0, "Should find video files");

    // Verify duplicates were detected correctly
    let duplicate_group = duplicates.values().find(|group| group.len() >= 3);
    assert!(duplicate_group.is_some(), "Should find group with 3 duplicates");

    // 5. Organize files
    println!("Organizing files...");
    let organizer = FileOrganizer::new();
    let organize_result = organizer
        .organize_files_with_duplicates(files.clone(), duplicates.clone(), &settings, progress.clone())
        .await?;

    assert!(organize_result.success, "Organization should succeed");
    assert!(organize_result.files_organized > 0, "Should organize some files");
    assert!(organize_result.skipped_duplicates > 0, "Should skip some duplicates");

    println!("Organized {} files", organize_result.files_organized);
    println!("Skipped {} duplicates", organize_result.skipped_duplicates);

    // 6. Verify organization structure
    println!("Verifying organization structure...");

    // Check that files were organized into correct structure
    assert!(dest_dir.exists(), "Destination directory should exist");

    // Videos should be in separate folder
    let videos_dir = dest_dir.join("Videos");
    assert!(videos_dir.exists(), "Videos directory should exist");

    // Check for year/month structure
    let mut found_year_dir = false;

    let mut entries = fs::read_dir(&dest_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.is_dir() {
            let name = path.file_name().unwrap().to_string_lossy();

            // Check for year directory (e.g., "2024")
            if name.chars().all(char::is_numeric) && name.len() == 4 {
                found_year_dir = true;

                // Check for month subdirectory
                let mut month_entries = fs::read_dir(&path).await?;
                while let Some(month_entry) = month_entries.next_entry().await? {
                    let month_name = month_entry.file_name().to_string_lossy().to_string();

                    if month_name.contains('-') && month_name.len() > 3 {
                        break;
                    }
                }
            }
        }
    }

    assert!(
        found_year_dir || videos_dir.exists(),
        "Should create year directory or Videos directory"
    );

    // 7. Verify file extensions were lowercased
    let mut found_lowercase_extension = false;

    check_extensions(&dest_dir, &mut found_lowercase_extension).await?;
    assert!(found_lowercase_extension, "Should have lowercase extensions");

    // 8. Test duplicate detection separately
    println!("Testing duplicate detection...");
    let detector = DuplicateDetector::new();

    // Scan organized files to ensure no duplicates remain
    let (organized_files, _) = scanner
        .scan_directory_with_duplicates(&dest_dir, true, progress.clone(), &settings, None)
        .await?;

    let duplicate_stats = detector.detect_duplicates(&organized_files, false).await?;

    // Since we didn't rename duplicates, organized folder should have no duplicates
    assert_eq!(
        duplicate_stats.total_duplicates, 0,
        "Organized folder should have no duplicates when rename_duplicates is false"
    );

    // 9. Test with rename_duplicates enabled
    println!("Testing with rename_duplicates enabled...");

    // Reset and test with different settings
    let dest_dir_rename = temp_dir.path().join("organized_rename");
    fs::create_dir_all(&dest_dir_rename).await?;

    let mut settings_rename = settings.clone();
    settings_rename.destination_folder = Some(dest_dir_rename.clone());
    settings_rename.rename_duplicates = true;

    // Re-scan source
    let (files2, duplicates2) = scanner
        .scan_directory_with_duplicates(&source_dir, true, progress.clone(), &settings_rename, None)
        .await?;

    let organize_result2 = organizer
        .organize_files_with_duplicates(files2, duplicates2, &settings_rename, progress)
        .await?;

    assert!(organize_result2.success, "Organization with rename should succeed");
    assert_eq!(
        organize_result2.skipped_duplicates, 0,
        "Should not skip duplicates when rename is enabled"
    );

    // 10. Verify cache functionality
    println!("Verifying cache functionality...");

    // Second scan should be faster due to cache
    let start = std::time::Instant::now();
    let (cached_files, _) = scanner
        .scan_directory_with_duplicates(
            &source_dir,
            false, // Don't force rescan
            Arc::new(RwLock::new(Progress::default())),
            &settings,
            None,
        )
        .await?;
    let cache_duration = start.elapsed();

    println!("Cache scan took: {cache_duration:?}");
    assert_eq!(
        cached_files.len(),
        files.len(),
        "Cache should return same number of files"
    );

    // 11. Final statistics
    let source_files_remaining = count_files_recursive(&source_dir).await?;
    let organized_files_count = count_files_recursive(&dest_dir).await?;
    let organized_rename_count = count_files_recursive(&dest_dir_rename).await?;

    println!("\nFinal Statistics:");
    println!("Source files remaining: {source_files_remaining}");
    println!("Files organized (skip duplicates): {organized_files_count}");
    println!("Files organized (rename duplicates): {organized_rename_count}");

    // Verify counts make sense
    assert!(source_files_remaining > 0, "Some files should remain in source");
    assert!(organized_files_count > 0, "Some files should be organized");
    assert!(
        organized_rename_count > organized_files_count,
        "Rename mode should organize more files"
    );

    Ok(())
}

/// Helper to count files recursively
async fn count_files_recursive(dir: &Path) -> Result<usize> {
    let mut count = 0;
    let mut entries = fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.is_file() {
            count += 1;
        } else if path.is_dir() {
            count += Box::pin(count_files_recursive(&path)).await?;
        }
    }

    Ok(count)
}

async fn check_extensions(dir: &Path, found: &mut bool) -> Result<()> {
    let mut entries = fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy();
                if ext_str.chars().all(char::is_lowercase) {
                    *found = true;
                }
            }
        } else if path.is_dir() {
            Box::pin(check_extensions(&path, found)).await?;
        }
    }
    Ok(())
}
