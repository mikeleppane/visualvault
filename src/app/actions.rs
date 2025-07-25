use chrono::Local;
use color_eyre::eyre::Result;
use std::path::Path;
use std::sync::Arc;
use tracing::{error, info};
use walkdir::WalkDir;

use crate::{
    core::DuplicateStats,
    models::{ImageMetadata, MediaMetadata},
    utils::FolderStats,
};

use super::{App, AppState, OrganizeResult, ScanResult};

impl App {
    /// Starts a scan of the configured source folder.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No source folder is configured
    /// - The scanner fails to scan the directory
    pub async fn start_scan(&mut self) -> Result<()> {
        self.success_message = Some("Starting scan...".to_string());
        let settings = self.settings.read().await;
        let source = settings
            .source_folder
            .clone()
            .ok_or_else(|| color_eyre::eyre::eyre!("Source folder not configured"))?;
        info!("Scanner: Starting scan of {:?}", source);
        let recursive = settings.recurse_subfolders;
        let settings_clone = settings.clone();
        drop(settings);

        self.state = AppState::Scanning;
        self.progress.write().await.reset();

        let start_time = std::time::Instant::now();
        let scanner = Arc::clone(&self.scanner);
        let progress = Arc::clone(&self.progress);

        let filter_set = if self.filter_set.is_active {
            Some(self.filter_set.clone())
        } else {
            None
        };

        let scan_result = scanner
            .scan_directory_with_duplicates(&source, recursive, progress, &settings_clone, filter_set)
            .await;

        match scan_result {
            Ok((files, duplicates)) => {
                info!("=== SCAN RESULTS ===");
                info!("Scanner returned {} files", files.len());
                info!("Scanner returned {} duplicate groups", duplicates.len());

                let files_found = files.len();

                info!(
                    "Scan complete: {} files found, {} duplicates",
                    files_found, duplicates.total_duplicates
                );

                self.statistics.update_from_scan_results(&files, &duplicates);
                self.file_manager.write().await.set_files(files.clone());
                self.cached_files = files;

                info!("App cached_files now has {} entries", self.cached_files.len());

                self.duplicate_groups = if duplicates.is_empty() {
                    None
                } else {
                    Some(
                        duplicates
                            .groups
                            .into_iter()
                            .map(|group| group.files.iter().map(|arc| (**arc).clone()).collect())
                            .collect(),
                    )
                };

                self.last_scan_result = Some(ScanResult {
                    files_found,
                    duration: start_time.elapsed(),
                    timestamp: Local::now(),
                });

                let duplicate_count = duplicates.total_duplicates;
                self.success_message = if duplicate_count > 0 {
                    Some(format!(
                        "Scan complete: {files_found} files found ({duplicate_count} duplicates)"
                    ))
                } else {
                    Some(format!("Scan complete: {files_found} files found"))
                };

                self.state = AppState::Dashboard;
            }
            Err(e) => {
                error!("Scan failed: {}", e);
                self.error_message = Some(format!("Scan failed: {e}"));
                self.state = AppState::Dashboard;
            }
        }

        Ok(())
    }

    /// Starts organizing files from the cached scan results.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No destination folder is configured
    /// - The organizer fails to organize the files
    /// - File operations fail during organization
    pub async fn start_organize(&mut self) -> Result<()> {
        if self.cached_files.is_empty() {
            self.error_message = Some("No files to organize. Run a scan first.".to_string());
            return Ok(());
        }

        info!("Starting file organization");
        self.success_message = Some("Starting to organize files...".to_string());
        self.state = AppState::Organizing;
        self.progress.write().await.reset();

        let start_time = Local::now();
        let organizer = self.organizer.clone();
        let progress = self.progress.clone();
        let scanner = self.scanner.clone();
        let settings = self.settings.read().await;
        let destination = settings
            .destination_folder
            .clone()
            .ok_or_else(|| color_eyre::eyre::eyre!("No destination folder configured"))?;

        let mut files = self.cached_files.clone();
        let files_total = files.len();

        let duplicates = if settings.rename_duplicates {
            DuplicateStats::new()
        } else {
            scanner.find_duplicates(&mut files, progress.clone()).await?
        };

        let organize_result = organizer
            .organize_files_with_duplicates(files, duplicates, &settings, progress)
            .await;
        drop(settings);

        match organize_result {
            Ok(result) => {
                info!("Organization complete: {} files organized", result.files_organized);

                let has_errors = !result.errors.is_empty();
                let error_count = result.errors.len();

                self.last_organize_result = Some(OrganizeResult {
                    files_organized: result.files_organized,
                    files_total,
                    destination,
                    success: result.success,
                    timestamp: start_time,
                    skipped_duplicates: result.skipped_duplicates,
                    errors: result.errors,
                });

                let message = if result.skipped_duplicates > 0 {
                    format!(
                        "Organization complete: {} files organized, {} duplicates skipped",
                        result.files_organized, result.skipped_duplicates
                    )
                } else {
                    format!("Organization complete: {} files organized", result.files_organized)
                };

                if has_errors {
                    self.error_message = Some(format!("{message} (with {error_count} errors)"));
                } else {
                    self.success_message = Some(message);
                }

                self.state = AppState::Dashboard;
                self.cached_files.clear();
                self.duplicate_groups = None;
            }
            Err(e) => {
                error!("Organization failed: {}", e);

                self.last_organize_result = Some(OrganizeResult {
                    files_organized: 0,
                    files_total,
                    destination,
                    success: false,
                    timestamp: start_time,
                    skipped_duplicates: 0,
                    errors: vec![e.to_string()],
                });

                self.error_message = Some(format!("Organization failed: {e}"));
                self.state = AppState::Dashboard;
            }
        }

        Ok(())
    }

    /// Updates the application statistics based on the current file list.
    ///
    /// # Errors
    ///
    /// This function currently does not return any errors, but the `Result` type
    /// is maintained for future compatibility with potential error conditions.
    pub async fn update_statistics(&mut self) -> Result<()> {
        let files = self.file_manager.read().await.get_files();
        self.statistics.update_from_files(&files);
        self.cached_files.clone_from(&(*files));
        Ok(())
    }

    /// Loads image metadata from the specified file path.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be read from disk
    /// - The file is not a valid image format
    /// - The image data is corrupted or invalid
    pub async fn load_image_metadata(&self, path: &Path) -> Result<MediaMetadata> {
        use image::GenericImageView;

        let file_data = tokio::fs::read(path).await?;
        let path_owned = path.to_path_buf();

        let metadata = tokio::task::spawn_blocking(move || -> Result<MediaMetadata> {
            let img = image::load_from_memory(&file_data)?;
            let (width, height) = img.dimensions();
            let color_type = match img.color() {
                image::ColorType::L8 => "Grayscale 8-bit",
                image::ColorType::La8 => "Grayscale + Alpha 8-bit",
                image::ColorType::Rgb8 => "RGB 8-bit",
                image::ColorType::Rgba8 => "RGBA 8-bit",
                image::ColorType::L16 => "Grayscale 16-bit",
                image::ColorType::La16 => "Grayscale + Alpha 16-bit",
                image::ColorType::Rgb16 => "RGB 16-bit",
                image::ColorType::Rgba16 => "RGBA 16-bit",
                _ => "Unknown",
            };

            let format = path_owned
                .extension()
                .and_then(|e| e.to_str())
                .map_or_else(|| "Unknown".to_string(), str::to_uppercase);

            Ok(MediaMetadata::Image(ImageMetadata {
                width,
                height,
                format,
                color_type: color_type.to_string(),
            }))
        })
        .await??;

        Ok(metadata)
    }

    /// Updates folder statistics for the configured source and destination folders.
    ///
    /// This function clears the existing folder stats cache and recalculates
    /// statistics for both source and destination folders if they are configured.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The tokio task spawning fails
    /// - File system operations fail during statistics calculation
    pub async fn update_folder_stats(&mut self) -> Result<()> {
        info!("Updating folder statistics...");
        self.folder_stats_cache.clear();
        self.success_message = Some("Updating folder statistics...".to_string());

        let settings = self.settings.read().await;
        let mut paths_to_update = Vec::new();
        if let Some(source) = &settings.source_folder {
            paths_to_update.push(source.clone());
        }
        if let Some(dest) = &settings.destination_folder {
            paths_to_update.push(dest.clone());
        }
        drop(settings);

        let mut update_count = 0;
        for path in paths_to_update {
            let path_clone = path.clone();
            let stats_result = tokio::task::spawn_blocking(move || calculate_folder_stats_sync(&path_clone)).await;

            match stats_result {
                Ok(stats) => {
                    self.folder_stats_cache.insert(path, stats);
                    update_count += 1;
                }
                Err(e) => {
                    error!("Failed to calculate stats for {:?}: {}", path, e);
                }
            }
        }

        if update_count > 0 {
            self.success_message = Some(format!("Updated statistics for {update_count} folder(s)"));
        } else {
            self.error_message = Some("No folders configured to update".to_string());
        }

        Ok(())
    }

    pub fn update_folder_stats_if_needed(&mut self) {
        let mut paths_to_update = Vec::new();

        {
            let Ok(settings) = self.settings.try_read() else { return };

            if let Some(source) = &settings.source_folder {
                if !self.folder_stats_cache.contains_key(source) {
                    paths_to_update.push(source.clone());
                }
            }

            if let Some(dest) = &settings.destination_folder {
                if !self.folder_stats_cache.contains_key(dest) {
                    paths_to_update.push(dest.clone());
                }
            }
        }

        for path in paths_to_update {
            let path_clone = path.clone();
            let stats_result = std::thread::spawn(move || calculate_folder_stats_sync(&path_clone));

            if let Ok(stats) = stats_result.join() {
                self.folder_stats_cache.insert(path, stats);
            }
        }
    }

    /// Updates the progress state for ongoing operations.
    ///
    /// This function checks if the application is currently in a state that requires
    /// progress updates (scanning or organizing) and updates the progress accordingly.
    ///
    /// # Errors
    ///
    /// This function currently does not return any errors, but the `Result` type
    /// is maintained for future compatibility with potential error conditions.
    pub async fn update_progress(&mut self) -> Result<()> {
        if matches!(self.state, AppState::Scanning | AppState::Organizing) {
            let _ = self.progress.write().await;
        }
        Ok(())
    }

    /// Checks if ongoing operations (scanning or organizing) have completed and updates the application state accordingly.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Statistics update fails after scan completion
    /// - Any internal state updates fail during the completion check
    pub async fn check_operation_completion(&mut self) -> Result<()> {
        match self.state {
            AppState::Scanning => {
                if self.scanner.is_complete().await {
                    let count = self.file_manager.read().await.get_file_count();
                    self.success_message = Some(format!("Scan complete: {count} files found"));
                    self.state = AppState::Dashboard;
                    self.update_statistics().await?;
                }
            }
            AppState::Organizing => {
                if self.organizer.is_complete().await {
                    let result = self.organizer.get_result().await;
                    match result {
                        Some(Ok(count)) => {
                            self.success_message = Some(format!("Successfully organized {count} files"));
                        }
                        Some(Err(e)) => {
                            self.error_message = Some(format!("Organization failed: {e}"));
                        }
                        None => {}
                    }
                    self.state = AppState::Dashboard;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn calculate_folder_stats_sync(path: &std::path::Path) -> FolderStats {
    let mut stats = FolderStats { ..Default::default() };

    for entry in WalkDir::new(path).follow_links(false).into_iter().flatten() {
        if let Ok(metadata) = entry.metadata() {
            if metadata.is_file() {
                stats.total_files += 1;
                stats.total_size += metadata.len();

                if let Some(ext) = entry.path().extension() {
                    if let Some(ext_str) = ext.to_str() {
                        if is_media_extension(ext_str) {
                            stats.media_files += 1;
                        }
                    }
                }
            } else if metadata.is_dir() {
                stats.total_dirs += 1;
            }
        }
    }

    stats
}

fn is_media_extension(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "jpg"
            | "jpeg"
            | "png"
            | "gif"
            | "bmp"
            | "webp"
            | "tiff"
            | "tif"
            | "svg"
            | "ico"
            | "heic"
            | "heif"
            | "mp4"
            | "avi"
            | "mkv"
            | "mov"
            | "wmv"
            | "flv"
            | "webm"
            | "m4v"
            | "mpg"
            | "mpeg"
            | "3gp"
            | "mp3"
            | "wav"
            | "flac"
            | "aac"
            | "ogg"
            | "wma"
            | "m4a"
            | "opus"
            | "ape"
    )
}
