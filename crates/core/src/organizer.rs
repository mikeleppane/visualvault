use color_eyre::eyre::Result;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{Mutex, RwLock};
use tracing::error;
use visualvault_config::{OrganizationMode, Settings};
use visualvault_models::{DuplicateStats, FileType, MediaFile, OrganizeResult};
use visualvault_utils::Progress;

use crate::UndoManager;
use crate::undo_manager::{FileOperation, MoveOperation};

pub struct FileOrganizer {
    is_organizing: Arc<Mutex<bool>>,
    result: Arc<Mutex<Option<Result<usize>>>>,
    undo_manager: Arc<UndoManager>,
}

impl FileOrganizer {
    /// Creates a new `FileOrganizer` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the undo manager fails to initialize with the provided config directory.
    pub async fn new(config_dir: PathBuf) -> Result<Self> {
        Ok(Self {
            is_organizing: Arc::new(Mutex::new(false)),
            result: Arc::new(Mutex::new(None)),
            undo_manager: Arc::new(UndoManager::new_with_history(config_dir).await?),
        })
    }

    /// Get a reference to the undo manager
    #[must_use]
    pub fn undo_manager(&self) -> &Arc<UndoManager> {
        &self.undo_manager
    }

    /// Organizes files into the configured destination folder, handling duplicates according to settings.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The destination folder is not configured in settings
    /// - File system operations fail (creating directories, moving files)
    /// - The organization mode in settings is invalid
    #[allow(clippy::cognitive_complexity)]
    pub async fn organize_files_with_duplicates(
        &self,
        files: Vec<Arc<MediaFile>>,
        duplicates: DuplicateStats,
        settings: &Settings,
        progress: Arc<RwLock<Progress>>,
    ) -> Result<OrganizeResult> {
        let dest_folder = settings
            .destination_folder
            .as_ref()
            .ok_or_else(|| color_eyre::eyre::eyre!("Destination folder not configured"))?;

        // Track which files have been processed to avoid duplicates
        let mut files_to_organize: Vec<Arc<MediaFile>> = Vec::new();
        let mut skipped_duplicates = 0;
        let files_total = files.len();

        // If rename_duplicates is false, filter out duplicates
        if !settings.rename_duplicates && !duplicates.is_empty() {
            // First, process all duplicate groups - keep only the oldest file from each group
            for group in &duplicates.groups {
                if group.files.len() > 1 {
                    // Choose the oldest file (by modified date) from the group
                    if let Some(chosen_file) = group.files.iter().min_by_key(|f| f.modified) {
                        files_to_organize.push(Arc::clone(chosen_file));
                        // Mark all other files in this group as skipped
                        skipped_duplicates += group.files.len() - 1;
                    }
                }
            }

            // Then, add all non-duplicate files
            for file in files {
                // Check if this file is part of any duplicate group
                let is_duplicate = duplicates
                    .groups
                    .iter()
                    .any(|group| group.files.iter().any(|dup_file| dup_file.path == file.path));

                if !is_duplicate {
                    // Not a duplicate, process normally
                    files_to_organize.push(file);
                }
            }
        } else {
            // If rename_duplicates is true, organize all files
            files_to_organize = files;
        }

        // Update progress
        {
            let mut prog = progress.write().await;
            prog.total = files_to_organize.len();
            prog.current = 0;
            prog.message = "Organizing files...".to_string();
        }

        // Now organize the filtered files
        let mut operations = Vec::new();
        let mut moved_files = 0;
        let mut errors = Vec::new();

        for (idx, file) in files_to_organize.iter().enumerate() {
            match self.organize_file(file, dest_folder, settings, &mut operations).await {
                Ok(dest_path) => {
                    moved_files += 1;
                    tracing::info!("Organized {} to {}", file.name, dest_path.display());
                }
                Err(e) => {
                    tracing::error!("Failed to organize {}: {}", file.name, e);
                    errors.push(format!("{}: {}", file.name, e));
                }
            }

            // Update progress
            {
                let mut prog = progress.write().await;
                prog.current = idx + 1;
            }
        }

        // Record the batch operation for undo (only if not dry run and operations were successful)
        if !operations.is_empty() && settings.undo_enabled {
            if let Err(e) = self.undo_manager.record_organize(operations).await {
                error!("Failed to record undo operation: {}", e);
            }
        }

        // Clear organizing flag
        *self.is_organizing.lock().await = false;

        // Store result
        *self.result.lock().await = Some(Ok(moved_files));

        Ok(OrganizeResult {
            files_organized: moved_files,
            files_total,
            destination: dest_folder.clone(),
            success: errors.is_empty(),
            timestamp: chrono::Local::now(),
            skipped_duplicates,
            errors,
        })
    }

    async fn organize_file(
        &self,
        file: &MediaFile,
        destination: &Path,
        settings: &Settings,
        operations: &mut Vec<FileOperation>,
    ) -> Result<PathBuf> {
        let target_dir = Self::determine_target_directory(file, destination, settings)?;

        // Create target directory if it doesn't exist
        fs::create_dir_all(&target_dir).await?;

        // Handle file naming
        let file_name = if settings.rename_duplicates {
            // Check if file exists in target directory
            if target_dir.join(&file.name).exists() {
                &Self::generate_unique_name(&target_dir, &file.name)?
            } else {
                &file.name
            }
        } else {
            &file.name
        };

        // Apply lowercase extension if configured
        let final_name = if settings.lowercase_extensions {
            let stem = Path::new(&file_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(file_name);
            let ext = Path::new(&file_name).extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext.is_empty() {
                file_name
            } else {
                &format!("{}.{}", stem, ext.to_lowercase())
            }
        } else {
            file_name
        };

        let target_path = target_dir.join(final_name);

        // Move the file
        fs::rename(&file.path, &target_path).await?;

        operations.push(FileOperation::Move(MoveOperation {
            source: file.path.clone(),
            destination: target_path.clone(),
        }));

        Ok(target_path)
    }

    fn determine_target_directory(file: &MediaFile, destination: &Path, settings: &Settings) -> Result<PathBuf> {
        let mut path = destination.to_path_buf();

        if settings.separate_videos && file.file_type == FileType::Video && settings.organize_by != "type" {
            path.push("Videos");
        }

        match OrganizationMode::from_str(&settings.organize_by) {
            Ok(OrganizationMode::Yearly) => {
                path.push(file.modified.format("%Y").to_string());
            }
            Ok(OrganizationMode::Monthly) => {
                path.push(file.modified.format("%Y").to_string());
                path.push(file.modified.format("%m-%B").to_string());
            }
            Ok(OrganizationMode::ByType) => {
                path.push(Self::get_type_folder(file));
            }
            Err(e) => {
                error!("Invalid organization mode: {}", e);
                return Err(color_eyre::eyre::eyre!("Invalid organization mode"));
            }
        }
        Ok(path)
    }

    fn get_type_folder(file: &MediaFile) -> String {
        match file.file_type {
            FileType::Image => "Images".to_string(),
            FileType::Video => "Videos".to_string(),
            FileType::Document => "Documents".to_string(),
            FileType::Other => "Others".to_string(),
        }
    }

    fn generate_unique_name(dir: &Path, original_name: &str) -> Result<String> {
        let mut counter = 1;
        let stem = Path::new(original_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let extension = Path::new(original_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        loop {
            let new_name = if extension.is_empty() {
                format!("{stem} ({counter})")
            } else {
                format!("{stem} ({counter}).{extension}")
            };

            if !dir.join(&new_name).exists() {
                return Ok(new_name);
            }

            counter += 1;
            if counter > 999 {
                return Err(color_eyre::eyre::eyre!("Too many duplicate filenames"));
            }
        }
    }

    pub async fn is_complete(&self) -> bool {
        !*self.is_organizing.lock().await
    }

    pub async fn get_result(&self) -> Option<Result<usize>> {
        self.result.lock().await.take()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::float_cmp)] // For comparing floats in tests
    #![allow(clippy::panic)]

    use super::*;
    use crate::undo_manager::OperationType::OrganizeFiles;
    use chrono::{DateTime, Local, TimeZone};
    use tempfile::TempDir;
    use tokio::fs;
    use visualvault_models::DuplicateGroup;

    // Helper function to create a test media file
    fn create_test_media_file(
        path: PathBuf,
        name: String,
        file_type: FileType,
        modified: DateTime<Local>,
        hash: Option<String>,
    ) -> Arc<MediaFile> {
        let extension = path.extension().unwrap_or_default().to_string_lossy().to_string();

        Arc::new(MediaFile {
            path,
            name,
            extension,
            file_type,
            size: 1024,
            created: modified,
            modified,
            hash,
            metadata: None,
        })
    }

    // Helper function to create test settings
    fn create_test_settings(destination: PathBuf) -> Settings {
        Settings {
            destination_folder: Some(destination),
            organize_by: "monthly".to_string(),
            rename_duplicates: false,
            separate_videos: true,
            lowercase_extensions: false,
            ..Default::default()
        }
    }

    // Helper function to create a test file on disk
    async fn create_test_file(path: &Path, content: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, content).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_file_organizer_creation() {
        let config_dir = TempDir::new().expect("Failed to create temp dir").path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();
        assert!(organizer.is_complete().await);
        assert!(organizer.get_result().await.is_none());
    }

    #[test]
    fn test_determine_target_directory_yearly() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let destination = temp_dir.path();
        let settings = Settings {
            organize_by: "yearly".to_string(),
            separate_videos: false,
            ..create_test_settings(destination.to_path_buf())
        };

        let file = create_test_media_file(
            PathBuf::from("/source/image.jpg"),
            "image.jpg".to_string(),
            FileType::Image,
            Local.with_ymd_and_hms(2024, 3, 15, 10, 0, 0).unwrap(),
            None,
        );

        let target_dir = FileOrganizer::determine_target_directory(&file, destination, &settings)?;
        assert_eq!(target_dir, destination.join("2024"));

        Ok(())
    }

    #[test]
    fn test_determine_target_directory_monthly() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let destination = temp_dir.path();
        let settings = Settings {
            organize_by: "monthly".to_string(),
            separate_videos: false,
            ..create_test_settings(destination.to_path_buf())
        };

        let file = create_test_media_file(
            PathBuf::from("/source/image.jpg"),
            "image.jpg".to_string(),
            FileType::Image,
            Local.with_ymd_and_hms(2024, 3, 15, 10, 0, 0).unwrap(),
            None,
        );

        let target_dir = FileOrganizer::determine_target_directory(&file, destination, &settings)?;
        assert_eq!(target_dir, destination.join("2024").join("03-March"));

        Ok(())
    }

    #[test]
    fn test_determine_target_directory_by_type() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let destination = temp_dir.path();
        let settings = Settings {
            organize_by: "type".to_string(),
            separate_videos: false,
            ..create_test_settings(destination.to_path_buf())
        };

        let file = create_test_media_file(
            PathBuf::from("/source/image.jpg"),
            "image.jpg".to_string(),
            FileType::Image,
            Local.with_ymd_and_hms(2024, 3, 15, 10, 0, 0).unwrap(),
            None,
        );

        let target_dir = FileOrganizer::determine_target_directory(&file, destination, &settings)?;
        assert_eq!(target_dir, destination.join("Images"));

        Ok(())
    }

    #[test]
    fn test_determine_target_directory_separate_videos() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let destination = temp_dir.path();
        let settings = Settings {
            organize_by: "monthly".to_string(),
            separate_videos: true,
            ..create_test_settings(destination.to_path_buf())
        };

        let video_file = create_test_media_file(
            PathBuf::from("/source/video.mp4"),
            "video.mp4".to_string(),
            FileType::Video,
            Local.with_ymd_and_hms(2024, 3, 15, 10, 0, 0).unwrap(),
            None,
        );

        let target_dir = FileOrganizer::determine_target_directory(&video_file, destination, &settings)?;
        assert_eq!(target_dir, destination.join("Videos").join("2024").join("03-March"));

        Ok(())
    }

    #[test]
    fn test_generate_unique_name() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let dir = temp_dir.path();

        // Create existing files
        std::fs::write(dir.join("image.jpg"), b"data")?;
        std::fs::write(dir.join("image (1).jpg"), b"data")?;

        // Test generating unique name
        let unique_name = FileOrganizer::generate_unique_name(dir, "image.jpg")?;
        assert_eq!(unique_name, "image (2).jpg");

        // Create the next file and test again
        std::fs::write(dir.join("image (2).jpg"), b"data")?;
        let unique_name = FileOrganizer::generate_unique_name(dir, "image.jpg")?;
        assert_eq!(unique_name, "image (3).jpg");

        // Test with file without extension
        std::fs::write(dir.join("file"), b"data")?;
        let unique_name = FileOrganizer::generate_unique_name(dir, "file")?;
        assert_eq!(unique_name, "file (1)");

        Ok(())
    }

    #[test]
    fn test_get_type_folder() {
        // Test image
        let image_file = create_test_media_file(
            PathBuf::from("image.jpg"),
            "image.jpg".to_string(),
            FileType::Image,
            Local::now(),
            None,
        );
        assert_eq!(FileOrganizer::get_type_folder(&image_file), "Images");

        // Test video with separate_videos = true
        let video_file = create_test_media_file(
            PathBuf::from("video.mp4"),
            "video.mp4".to_string(),
            FileType::Video,
            Local::now(),
            None,
        );
        assert_eq!(FileOrganizer::get_type_folder(&video_file), "Videos");

        // Test video with separate_videos = false
        assert_eq!(FileOrganizer::get_type_folder(&video_file), "Videos");

        // Test document
        let doc_file = create_test_media_file(
            PathBuf::from("doc.pdf"),
            "doc.pdf".to_string(),
            FileType::Document,
            Local::now(),
            None,
        );
        assert_eq!(FileOrganizer::get_type_folder(&doc_file), "Documents");

        // Test other
        let other_file = create_test_media_file(
            PathBuf::from("file.xyz"),
            "file.xyz".to_string(),
            FileType::Other,
            Local::now(),
            None,
        );
        assert_eq!(FileOrganizer::get_type_folder(&other_file), "Others");
    }

    #[tokio::test]
    async fn test_organize_file_basic() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).await?;
        fs::create_dir_all(&dest_dir).await?;

        // Create a test file
        let source_file = source_dir.join("image.jpg");
        create_test_file(&source_file, b"test data").await?;

        let file = create_test_media_file(
            source_file.clone(),
            "image.jpg".to_string(),
            FileType::Image,
            Local.with_ymd_and_hms(2024, 3, 15, 10, 0, 0).unwrap(),
            None,
        );

        let settings = create_test_settings(dest_dir.clone());
        let config_dir = temp_dir.path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();
        let result = organizer
            .organize_file(&file, &dest_dir, &settings, &mut Vec::new())
            .await?;

        // Check file was moved to correct location
        assert_eq!(result, dest_dir.join("2024").join("03-March").join("image.jpg"));
        assert!(result.exists());
        assert!(!source_file.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_organize_file_lowercase_extension() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).await?;
        fs::create_dir_all(&dest_dir).await?;

        // Create a test file with uppercase extension
        let source_file = source_dir.join("IMAGE.JPG");
        create_test_file(&source_file, b"test data").await?;

        let file = create_test_media_file(
            source_file.clone(),
            "IMAGE.JPG".to_string(),
            FileType::Image,
            Local.with_ymd_and_hms(2024, 3, 15, 10, 0, 0).unwrap(),
            None,
        );

        let mut settings = create_test_settings(dest_dir.clone());
        settings.lowercase_extensions = true;

        let config_dir = temp_dir.path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();
        let result = organizer
            .organize_file(&file, &dest_dir, &settings, &mut Vec::new())
            .await?;

        // Check file was renamed with lowercase extension
        assert_eq!(result, dest_dir.join("2024").join("03-March").join("IMAGE.jpg"));
        assert!(result.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_organize_file_rename_duplicates() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).await?;
        fs::create_dir_all(&dest_dir).await?;

        // Create target directory and existing file
        let target_dir = dest_dir.join("2024").join("03-March");
        fs::create_dir_all(&target_dir).await?;
        create_test_file(&target_dir.join("image.jpg"), b"existing").await?;

        // Create source file
        let source_file = source_dir.join("image.jpg");
        create_test_file(&source_file, b"new data").await?;

        let file = create_test_media_file(
            source_file.clone(),
            "image.jpg".to_string(),
            FileType::Image,
            Local.with_ymd_and_hms(2024, 3, 15, 10, 0, 0).unwrap(),
            None,
        );

        let mut settings = create_test_settings(dest_dir.clone());
        settings.rename_duplicates = true;

        let config_dir = temp_dir.path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();
        let result = organizer
            .organize_file(&file, &dest_dir, &settings, &mut Vec::new())
            .await?;

        // Check file was renamed
        assert_eq!(result, target_dir.join("image (1).jpg"));
        assert!(result.exists());
        assert!(target_dir.join("image.jpg").exists()); // Original still exists

        Ok(())
    }

    #[tokio::test]
    async fn test_organize_files_with_duplicates_skip() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).await?;

        // Create duplicate files
        let file1_path = source_dir.join("image1.jpg");
        let file2_path = source_dir.join("image2.jpg");
        let file3_path = source_dir.join("unique.jpg");

        create_test_file(&file1_path, b"duplicate data").await?;
        create_test_file(&file2_path, b"duplicate data").await?;
        create_test_file(&file3_path, b"unique data").await?;

        let modified_old = Local.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
        let modified_new = Local.with_ymd_and_hms(2024, 2, 1, 10, 0, 0).unwrap();

        let files = vec![
            create_test_media_file(
                file1_path.clone(),
                "image1.jpg".to_string(),
                FileType::Image,
                modified_old,
                Some("hash1".to_string()),
            ),
            create_test_media_file(
                file2_path.clone(),
                "image2.jpg".to_string(),
                FileType::Image,
                modified_new,
                Some("hash1".to_string()),
            ),
            create_test_media_file(
                file3_path.clone(),
                "unique.jpg".to_string(),
                FileType::Image,
                modified_new,
                Some("hash2".to_string()),
            ),
        ];

        let mut duplicates = DuplicateStats::new();
        duplicates.groups = vec![DuplicateGroup::new(
            vec![
                create_test_media_file(
                    file1_path.clone(),
                    "image1.jpg".to_string(),
                    FileType::Image,
                    modified_old,
                    Some("hash1".to_string()),
                ),
                create_test_media_file(
                    file2_path.clone(),
                    "image2.jpg".to_string(),
                    FileType::Image,
                    modified_new,
                    Some("hash1".to_string()),
                ),
            ],
            0, // Wasted space not used in this test
        )];

        let mut settings = create_test_settings(dest_dir.clone());
        settings.rename_duplicates = false;

        // Ensure the organizer is initialized
        let config_dir = temp_dir.path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();

        let progress = Arc::new(RwLock::new(Progress::default()));

        let result = organizer
            .organize_files_with_duplicates(files, duplicates, &settings, progress)
            .await?;

        assert_eq!(result.files_organized, 2); // Only oldest duplicate and unique file
        assert_eq!(result.files_total, 3);
        assert_eq!(result.skipped_duplicates, 1);
        assert!(result.success);

        // Check that only the older duplicate was kept
        assert!(dest_dir.join("2024").join("01-January").join("image1.jpg").exists());
        assert!(!dest_dir.join("2024").join("02-February").join("image2.jpg").exists());
        assert!(dest_dir.join("2024").join("02-February").join("unique.jpg").exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_organize_files_with_duplicates_rename() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).await?;

        // Create duplicate files
        let file1_path = source_dir.join("image1.jpg");
        let file2_path = source_dir.join("image2.jpg");

        create_test_file(&file1_path, b"duplicate data").await?;
        create_test_file(&file2_path, b"duplicate data").await?;

        let modified = Local.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();

        let files = vec![
            create_test_media_file(
                file1_path.clone(),
                "image1.jpg".to_string(),
                FileType::Image,
                modified,
                Some("hash1".to_string()),
            ),
            create_test_media_file(
                file2_path.clone(),
                "image2.jpg".to_string(),
                FileType::Image,
                modified,
                Some("hash1".to_string()),
            ),
        ];

        let mut duplicates = DuplicateStats::new();
        duplicates.groups = vec![DuplicateGroup::new(
            vec![
                create_test_media_file(
                    file1_path.clone(),
                    "image1.jpg".to_string(),
                    FileType::Image,
                    modified,
                    Some("hash1".to_string()),
                ),
                create_test_media_file(
                    file2_path.clone(),
                    "image2.jpg".to_string(),
                    FileType::Image,
                    modified,
                    Some("hash1".to_string()),
                ),
            ],
            0, // Wasted space not used in this test
        )];

        let mut settings = create_test_settings(dest_dir.clone());
        settings.rename_duplicates = true;

        let config_dir = temp_dir.path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();
        let progress = Arc::new(RwLock::new(Progress::default()));

        let result = organizer
            .organize_files_with_duplicates(files, duplicates, &settings, progress)
            .await?;

        assert_eq!(result.files_organized, 2); // Both files organized
        assert_eq!(result.skipped_duplicates, 0);
        assert!(result.success);

        // Check both files were organized
        assert!(dest_dir.join("2024").join("01-January").join("image1.jpg").exists());
        assert!(dest_dir.join("2024").join("01-January").join("image2.jpg").exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_organize_files_no_destination() -> Result<()> {
        let settings = Settings {
            destination_folder: None,
            ..Default::default()
        };

        let config_dir = TempDir::new()?.path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();
        let progress = Arc::new(RwLock::new(Progress::default()));

        let result = organizer
            .organize_files_with_duplicates(vec![], DuplicateStats::new(), &settings, progress)
            .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Destination folder not configured")
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_organize_files_invalid_mode() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let mut settings = create_test_settings(temp_dir.path().to_path_buf());
        settings.organize_by = "invalid_mode".to_string();

        let file = create_test_media_file(
            PathBuf::from("test.jpg"),
            "test.jpg".to_string(),
            FileType::Image,
            Local::now(),
            None,
        );

        let result = FileOrganizer::determine_target_directory(&file, temp_dir.path(), &settings);
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_progress_tracking() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).await?;

        // Create test files
        let mut files = vec![];
        for i in 0..5 {
            let file_path = source_dir.join(format!("image{i}.jpg"));
            create_test_file(&file_path, b"data").await?;
            files.push(create_test_media_file(
                file_path,
                format!("image{i}.jpg"),
                FileType::Image,
                Local::now(),
                None,
            ));
        }

        let settings = create_test_settings(dest_dir.clone());
        let config_dir = temp_dir.path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();
        let progress = Arc::new(RwLock::new(Progress::default()));

        let result = organizer
            .organize_files_with_duplicates(files, DuplicateStats::new(), &settings, progress.clone())
            .await?;

        let prog = progress.read().await;
        assert_eq!(prog.total, 5);
        assert_eq!(prog.current, 5);
        assert_eq!(result.files_organized, 5);
        drop(prog);

        Ok(())
    }

    // Add these test cases to the existing tests module

    #[tokio::test]
    async fn test_organize_by_type_all_file_types() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).await?;

        // Create test files for all supported types
        let test_files = vec![
            // Images
            ("photo1.jpg", FileType::Image, "Images"),
            ("photo2.png", FileType::Image, "Images"),
            ("photo3.gif", FileType::Image, "Images"),
            ("raw_photo.cr2", FileType::Image, "Images"),
            // Videos
            ("video1.mp4", FileType::Video, "Videos"),
            ("video2.avi", FileType::Video, "Videos"),
            ("video3.mkv", FileType::Video, "Videos"),
            // Documents
            ("document1.pdf", FileType::Document, "Documents"),
            ("document2.docx", FileType::Document, "Documents"),
            ("spreadsheet.xlsx", FileType::Document, "Documents"),
            ("presentation.pptx", FileType::Document, "Documents"),
            ("text.txt", FileType::Document, "Documents"),
            // Audio
            ("song1.mp3", FileType::Other, "Others"),
            ("song2.flac", FileType::Other, "Others"),
            ("podcast.m4a", FileType::Other, "Others"),
            // Archives
            ("backup.zip", FileType::Other, "Others"),
            ("compressed.7z", FileType::Other, "Others"),
            ("archive.tar.gz", FileType::Other, "Others"),
            // Other
            ("data.dat", FileType::Other, "Others"),
            ("config.cfg", FileType::Other, "Others"),
        ];

        let mut files = Vec::new();
        let modified = Local.with_ymd_and_hms(2024, 3, 15, 10, 0, 0).unwrap();

        // Create all test files
        for (filename, file_type, _) in &test_files {
            let file_path = source_dir.join(filename);
            create_test_file(&file_path, format!("content of {filename}").as_bytes()).await?;

            files.push(create_test_media_file(
                file_path,
                (*filename).to_string(),
                file_type.clone(),
                modified,
                None,
            ));
        }

        // Configure settings for type-based organization
        let settings = Settings {
            destination_folder: Some(dest_dir.clone()),
            organize_by: "type".to_string(),
            rename_duplicates: false,
            separate_videos: true,
            lowercase_extensions: false,
            ..Default::default()
        };

        let config_dir = temp_dir.path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();
        let progress = Arc::new(RwLock::new(Progress::default()));

        let result = organizer
            .organize_files_with_duplicates(files, DuplicateStats::new(), &settings, progress)
            .await?;

        // Verify all files were organized
        assert_eq!(result.files_organized, test_files.len());
        assert_eq!(result.files_total, test_files.len());
        assert_eq!(result.skipped_duplicates, 0);
        assert!(result.success);
        assert!(result.errors.is_empty());

        // Verify each file is in the correct type folder
        for (filename, _, expected_folder) in &test_files {
            let expected_path = dest_dir.join(expected_folder).join(filename);
            assert!(
                expected_path.exists(),
                "File {filename} should exist at {expected_path:?}"
            );

            // Verify source file was moved (not copied)
            let source_path = source_dir.join(filename);
            assert!(!source_path.exists(), "Source file {filename} should have been moved");
        }

        // Verify folder structure
        /* assert!(dest_dir.join("Images").exists());
        assert!(dest_dir.join("Videos").exists());
        assert!(dest_dir.join("Documents").exists());
        assert!(dest_dir.join("Others").exists());
        assert!(dest_dir.join("Others").exists());
        assert!(dest_dir.join("Others").exists()); */

        // Count files in each folder
        let mut images_dir = fs::read_dir(dest_dir.join("Images")).await?;
        let mut images_count = 0;
        while images_dir.next_entry().await?.is_some() {
            images_count += 1;
        }

        let mut videos_dir = fs::read_dir(dest_dir.join("Videos")).await?;
        let mut videos_count = 0;
        while videos_dir.next_entry().await?.is_some() {
            videos_count += 1;
        }

        let mut documents_dir = fs::read_dir(dest_dir.join("Documents")).await?;
        let mut documents_count = 0;
        while documents_dir.next_entry().await?.is_some() {
            documents_count += 1;
        }

        let mut other_dir = fs::read_dir(dest_dir.join("Others")).await?;
        let mut other_count = 0;
        while other_dir.next_entry().await?.is_some() {
            other_count += 1;
        }

        assert_eq!(images_count, 4);
        assert_eq!(videos_count, 3);
        assert_eq!(documents_count, 5);
        assert_eq!(other_count, 8);

        Ok(())
    }

    #[tokio::test]
    async fn test_organize_by_type_with_separate_videos_disabled() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).await?;

        // Create test video files
        let video_files = vec![("video1.mp4", FileType::Video), ("video2.avi", FileType::Video)];

        let mut files = Vec::new();
        let modified = Local::now();

        for (filename, file_type) in &video_files {
            let file_path = source_dir.join(filename);
            create_test_file(&file_path, b"video content").await?;

            files.push(create_test_media_file(
                file_path,
                (*filename).to_string(),
                file_type.clone(),
                modified,
                None,
            ));
        }

        // Configure settings with separate_videos = false
        let settings = Settings {
            destination_folder: Some(dest_dir.clone()),
            organize_by: "type".to_string(),
            separate_videos: false,
            ..Default::default()
        };

        let config_dir = temp_dir.path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();
        let progress = Arc::new(RwLock::new(Progress::default()));

        let result = organizer
            .organize_files_with_duplicates(files, DuplicateStats::new(), &settings, progress)
            .await?;

        assert_eq!(result.files_organized, 2);
        assert!(result.success);

        // Videos should be in "Media" folder when separate_videos is false
        assert!(dest_dir.join("Videos").join("video1.mp4").exists());
        assert!(dest_dir.join("Videos").join("video2.avi").exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_organize_by_type_with_duplicates() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).await?;

        // Create duplicate files of different types
        let files_data = vec![
            ("image1.jpg", FileType::Image, "duplicate_image", "hash_img"),
            ("image2.jpg", FileType::Image, "duplicate_image", "hash_img"),
            ("doc1.pdf", FileType::Document, "duplicate_doc", "hash_doc"),
            ("doc2.pdf", FileType::Document, "duplicate_doc", "hash_doc"),
            ("unique.mp3", FileType::Other, "unique_audio", "hash_unique"),
        ];

        let mut files = Vec::new();
        let modified_old = Local.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
        let modified_new = Local.with_ymd_and_hms(2024, 1, 2, 10, 0, 0).unwrap();

        // Create files with different modified times for duplicates
        for (filename, file_type, content, hash) in &files_data {
            let file_path = source_dir.join(filename);
            create_test_file(&file_path, content.as_bytes()).await?;

            // Make first file of each duplicate pair older
            let modified = if filename.ends_with("1.jpg") || filename.ends_with("1.pdf") {
                modified_old
            } else {
                modified_new
            };

            let file = create_test_media_file(
                file_path,
                (*filename).to_string(),
                file_type.clone(),
                modified,
                Some((*hash).to_string()),
            );

            files.push(file);
        }

        // Create proper duplicate groups
        let mut duplicate_stats = DuplicateStats::new();

        // Group 1: Two image files with same hash
        duplicate_stats.groups.push(DuplicateGroup::new(
            vec![
                files[0].clone(), // image1.jpg
                files[1].clone(), // image2.jpg
            ],
            files[0].size, // wasted space = size of one file
        ));

        // Group 2: Two document files with same hash
        duplicate_stats.groups.push(DuplicateGroup::new(
            vec![
                files[2].clone(), // doc1.pdf
                files[3].clone(), // doc2.pdf
            ],
            files[2].size, // wasted space = size of one file
        ));

        // Note: unique.mp3 is not in any duplicate group

        duplicate_stats.total_groups = 2;
        duplicate_stats.total_duplicates = 4; // Total files in duplicate groups
        duplicate_stats.total_wasted_space = files[0].size + files[2].size;

        let settings = Settings {
            destination_folder: Some(dest_dir.clone()),
            organize_by: "type".to_string(),
            rename_duplicates: false, // Skip duplicates
            ..Default::default()
        };

        let config_dir = temp_dir.path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();
        let progress = Arc::new(RwLock::new(Progress::default()));

        let result = organizer
            .organize_files_with_duplicates(files, duplicate_stats, &settings, progress)
            .await?;

        // Should organize 3 files (one from each duplicate group + unique)
        assert_eq!(result.files_organized, 3);
        assert_eq!(result.files_total, 5);
        assert_eq!(result.skipped_duplicates, 2);
        assert!(result.success);

        // Verify files are in correct type folders
        assert!(dest_dir.join("Images").exists());
        assert!(dest_dir.join("Documents").exists());
        assert!(dest_dir.join("Others").join("unique.mp3").exists());

        // Count files to ensure only one from each duplicate group
        let mut images_dir = fs::read_dir(dest_dir.join("Images")).await?;
        let mut images_count = 0;
        while images_dir.next_entry().await?.is_some() {
            images_count += 1;
        }

        let mut documents_dir = fs::read_dir(dest_dir.join("Documents")).await?;
        let mut documents_count = 0;
        while documents_dir.next_entry().await?.is_some() {
            documents_count += 1;
        }

        assert_eq!(images_count, 1); // Only one image from duplicate group
        assert_eq!(documents_count, 1); // Only one document from duplicate group

        // Verify the oldest files were kept (those ending with "1")
        assert!(dest_dir.join("Images").join("image1.jpg").exists());
        assert!(dest_dir.join("Documents").join("doc1.pdf").exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_organize_by_type_with_lowercase_extensions() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).await?;

        // Create files with uppercase extensions
        let files_data = vec![
            ("IMAGE.JPG", FileType::Image),
            ("DOCUMENT.PDF", FileType::Document),
            ("AUDIO.MP3", FileType::Other),
            ("MiXeD.ZiP", FileType::Other),
        ];

        let mut files = Vec::new();
        let modified = Local::now();

        for (filename, file_type) in &files_data {
            let file_path = source_dir.join(filename);
            create_test_file(&file_path, b"content").await?;

            files.push(create_test_media_file(
                file_path,
                (*filename).to_string(),
                file_type.clone(),
                modified,
                None,
            ));
        }

        let settings = Settings {
            destination_folder: Some(dest_dir.clone()),
            organize_by: "type".to_string(),
            lowercase_extensions: true,
            ..Default::default()
        };

        let config_dir = temp_dir.path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();
        let progress = Arc::new(RwLock::new(Progress::default()));

        let result = organizer
            .organize_files_with_duplicates(files, DuplicateStats::new(), &settings, progress)
            .await?;

        assert_eq!(result.files_organized, 4);
        assert!(result.success);

        // Verify files have lowercase extensions
        assert!(dest_dir.join("Images").join("IMAGE.jpg").exists());
        assert!(dest_dir.join("Documents").join("DOCUMENT.pdf").exists());
        assert!(dest_dir.join("Others").join("AUDIO.mp3").exists());
        assert!(dest_dir.join("Others").join("MiXeD.zip").exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_organize_mixed_modes() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).await?;

        // Create test files
        let test_data = vec![
            ("photo.jpg", FileType::Image),
            ("video.mp4", FileType::Video),
            ("document.pdf", FileType::Document),
        ];

        let mut files = Vec::new();
        let modified = Local.with_ymd_and_hms(2024, 3, 15, 10, 0, 0).unwrap();

        for (filename, file_type) in &test_data {
            let file_path = source_dir.join(filename);
            create_test_file(&file_path, b"content").await?;
            files.push(create_test_media_file(
                file_path,
                (*filename).to_string(),
                file_type.clone(),
                modified,
                None,
            ));
        }

        // Test different organization modes
        let modes = vec![
            (
                "type",
                vec!["Images/photo.jpg", "Videos/video.mp4", "Documents/document.pdf"],
            ),
            ("yearly", vec!["2024/photo.jpg", "2024/video.mp4", "2024/document.pdf"]),
            (
                "monthly",
                vec![
                    "2024/03-March/photo.jpg",
                    "2024/03-March/video.mp4",
                    "2024/03-March/document.pdf",
                ],
            ),
        ];

        for (mode, expected_paths) in modes {
            // Reset destination directory
            if dest_dir.exists() {
                fs::remove_dir_all(&dest_dir).await?;
            }
            fs::create_dir_all(&dest_dir).await?;

            // Recreate source files
            for (filename, _) in &test_data {
                let file_path = source_dir.join(filename);
                if !file_path.exists() {
                    create_test_file(&file_path, b"content").await?;
                }
            }

            let settings = Settings {
                destination_folder: Some(dest_dir.clone()),
                organize_by: mode.to_string(),
                separate_videos: false,
                ..Default::default()
            };

            let config_dir = temp_dir.path().to_path_buf();
            let organizer = FileOrganizer::new(config_dir).await.unwrap();
            let progress = Arc::new(RwLock::new(Progress::default()));

            let result = organizer
                .organize_files_with_duplicates(files.clone(), DuplicateStats::new(), &settings, progress)
                .await?;
            // print the actual paths for debugging

            assert_eq!(result.files_organized, 3, "Failed for mode: {mode}");
            assert!(result.success, "Failed for mode: {mode}");
            // Verify files are in expected locations
            for expected_path in expected_paths {
                let full_path = dest_dir.join(expected_path);
                assert!(
                    full_path.exists(),
                    "File should exist at {full_path:?} for mode: {mode}"
                );
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_organize_by_type_empty_extension() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).await?;

        // Create files without extensions
        let files_data = vec![
            ("README", FileType::Other),
            ("Makefile", FileType::Other),
            ("LICENSE", FileType::Other),
        ];

        let mut files = Vec::new();
        let modified = Local::now();

        for (filename, file_type) in &files_data {
            let file_path = source_dir.join(filename);
            create_test_file(&file_path, b"content").await?;

            files.push(create_test_media_file(
                file_path,
                (*filename).to_string(),
                file_type.clone(),
                modified,
                None,
            ));
        }

        let settings = Settings {
            destination_folder: Some(dest_dir.clone()),
            organize_by: "type".to_string(),
            lowercase_extensions: true, // Should handle files without extensions
            ..Default::default()
        };

        let config_dir = temp_dir.path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();
        let progress = Arc::new(RwLock::new(Progress::default()));

        let result = organizer
            .organize_files_with_duplicates(files, DuplicateStats::new(), &settings, progress)
            .await?;

        assert_eq!(result.files_organized, 3);
        assert!(result.success);

        // All files without extensions should go to "Other" folder
        assert!(dest_dir.join("Others").exists());
        assert!(dest_dir.join("Others").join("README").exists());
        assert!(dest_dir.join("Others").join("Makefile").exists());
        assert!(dest_dir.join("Others").join("LICENSE").exists());

        Ok(())
    }

    #[test]
    fn test_get_type_folder_all_types() {
        // Test all file types
        let test_cases = vec![
            (FileType::Image, "Images"),
            (FileType::Video, "Videos"),
            (FileType::Document, "Documents"),
            (FileType::Other, "Others"),
            (FileType::Other, "Others"),
            (FileType::Other, "Others"),
        ];

        for (file_type, expected_folder) in test_cases {
            let file = create_test_media_file(
                PathBuf::from("test"),
                "test".to_string(),
                file_type.clone(),
                Local::now(),
                None,
            );

            assert_eq!(
                FileOrganizer::get_type_folder(&file),
                expected_folder,
                "Failed ftype: {file_type:?}"
            );
        }

        // Test video with separate_videos = false
        let video_file = create_test_media_file(
            PathBuf::from("video.mp4"),
            "video.mp4".to_string(),
            FileType::Video,
            Local::now(),
            None,
        );

        assert_eq!(FileOrganizer::get_type_folder(&video_file), "Videos");
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn test_organize_records_move_operations() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).await?;

        // Create test files
        let test_files = vec![
            ("photo1.jpg", FileType::Image, "content1"),
            ("photo2.png", FileType::Image, "content2"),
            ("video1.mp4", FileType::Video, "content3"),
            ("document.pdf", FileType::Document, "content4"),
        ];

        let mut files = Vec::new();
        let modified = Local.with_ymd_and_hms(2024, 3, 15, 10, 0, 0).unwrap();

        for (filename, file_type, content) in &test_files {
            let file_path = source_dir.join(filename);
            create_test_file(&file_path, content.as_bytes()).await?;

            files.push(create_test_media_file(
                file_path,
                (*filename).to_string(),
                file_type.clone(),
                modified,
                None,
            ));
        }

        let settings = Settings {
            destination_folder: Some(dest_dir.clone()),
            organize_by: "monthly".to_string(),
            rename_duplicates: false,
            separate_videos: true,
            lowercase_extensions: false,
            undo_enabled: true, // Enable undo to record operations
            ..Default::default()
        };

        let config_dir = temp_dir.path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();
        let progress = Arc::new(RwLock::new(Progress::default()));

        // Organize the files
        let result = organizer
            .organize_files_with_duplicates(files.clone(), DuplicateStats::new(), &settings, progress)
            .await?;

        assert_eq!(result.files_organized, 4);
        assert!(result.success);

        // Get the undo history to verify operations were recorded
        let history = organizer.undo_manager.get_history().await;

        // Should have one batch operation
        assert_eq!(history.len(), 1, "Should have recorded one batch operation");

        let operation = &history[0];
        assert!(!operation.undone, "Operation should not be marked as undone");
        assert!(
            operation.description.contains("Organized 4 files"),
            "Description should mention organizing 4 files"
        );

        // Verify the operation type and contents
        match &operation.operation {
            OrganizeFiles { operations } => {
                assert_eq!(operations.len(), 4, "Should have 4 file operations");

                // Verify each move operation
                for (i, op) in operations.iter().enumerate() {
                    match op {
                        FileOperation::Move(move_op) => {
                            let (filename, file_type, _) = &test_files[i];

                            // Verify source path
                            assert_eq!(
                                move_op.source,
                                source_dir.join(filename),
                                "Source path mismatch for {filename}"
                            );

                            // Verify destination path based on organization settings
                            let expected_dest = if matches!(file_type, FileType::Video) && settings.separate_videos {
                                dest_dir.join("Videos").join("2024").join("03-March").join(filename)
                            } else {
                                dest_dir.join("2024").join("03-March").join(filename)
                            };

                            assert_eq!(
                                move_op.destination, expected_dest,
                                "Destination path mismatch for {filename}"
                            );
                        }
                        _ => panic!("Expected Move operation, got {op:?}"),
                    }
                }
            }
            _ => panic!("Expected OrganizeFiles operation, got {:?}", operation.operation),
        }

        // Test undo functionality - verify the operations can be undone
        let undo_result = organizer.undo_manager.undo().await?;
        assert!(undo_result.is_some(), "Undo should succeed");
        assert!(undo_result.unwrap().contains("Undid organization of 4 files"));

        // Verify files are back in their original locations
        for (filename, _, _) in &test_files {
            let source_path = source_dir.join(filename);
            assert!(source_path.exists(), "File {filename} should be restored to source");

            // Verify file is no longer in destination
            let dest_path = if std::path::Path::new(filename)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("mp4"))
            {
                dest_dir.join("Videos").join("2024").join("03-March").join(filename)
            } else {
                dest_dir.join("2024").join("03-March").join(filename)
            };
            assert!(
                !dest_path.exists(),
                "File {filename} should not be in destination after undo"
            );
        }

        // Test redo functionality
        let redo_result = organizer.undo_manager.redo().await?;
        assert!(redo_result.is_some(), "Redo should succeed");

        // Verify files are back in organized locations
        for (filename, file_type, _) in &test_files {
            let source_path = source_dir.join(filename);
            assert!(
                !source_path.exists(),
                "File {filename} should not be in source after redo"
            );

            let dest_path = if matches!(file_type, FileType::Video) && settings.separate_videos {
                dest_dir.join("Videos").join("2024").join("03-March").join(filename)
            } else {
                dest_dir.join("2024").join("03-March").join(filename)
            };
            assert!(
                dest_path.exists(),
                "File {filename} should be in destination after redo"
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_organize_no_operations_recorded_when_undo_disabled() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_dir = temp_dir.path().join("source");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir_all(&source_dir).await?;

        // Create a test file
        let file_path = source_dir.join("test.jpg");
        create_test_file(&file_path, b"content").await?;

        let file = create_test_media_file(file_path, "test.jpg".to_string(), FileType::Image, Local::now(), None);

        let settings = Settings {
            destination_folder: Some(dest_dir.clone()),
            organize_by: "yearly".to_string(),
            undo_enabled: false, // Disable undo
            ..Default::default()
        };

        let config_dir = temp_dir.path().to_path_buf();
        let organizer = FileOrganizer::new(config_dir).await.unwrap();
        let progress = Arc::new(RwLock::new(Progress::default()));

        // Organize the file
        let result = organizer
            .organize_files_with_duplicates(vec![file], DuplicateStats::new(), &settings, progress)
            .await?;

        assert_eq!(result.files_organized, 1);
        assert!(result.success);

        // Verify no operations were recorded
        let history = organizer.undo_manager.get_history().await;
        assert!(
            history.is_empty(),
            "No operations should be recorded when undo is disabled"
        );

        Ok(())
    }
}
