use chrono::Local;
use color_eyre::eyre::Result;
use std::path::Path;
use std::sync::Arc;
use tracing::{error, info};
use visualvault_models::{DuplicateStats, ImageMetadata, MediaMetadata, ScanResult};
use visualvault_utils::FolderStats;
use walkdir::WalkDir;

use super::{App, AppState};

/// Parameters for executing a scan
struct ScanParameters {
    source: std::path::PathBuf,
    recursive: bool,
    scanner: Arc<visualvault_core::Scanner>,
    progress: Arc<tokio::sync::RwLock<visualvault_utils::Progress>>,
    filter_set: Option<visualvault_models::FilterSet>,
}

struct OrganizeParameters {
    files: Vec<Arc<visualvault_models::MediaFile>>,
    destination: std::path::PathBuf,
    rename_duplicates: bool,
    settings: visualvault_config::Settings,
    organizer: Arc<visualvault_core::FileOrganizer>,
    scanner: Arc<visualvault_core::Scanner>,
    progress: Arc<tokio::sync::RwLock<visualvault_utils::Progress>>,
    start_time: chrono::DateTime<Local>,
}

struct OrganizeExecutionResult {
    files_organized: usize,
    files_total: usize,
    destination: std::path::PathBuf,
    success: bool,
    skipped_duplicates: usize,
    errors: Vec<String>,
    start_time: chrono::DateTime<Local>,
}

impl OrganizeExecutionResult {
    fn success(
        result: visualvault_models::OrganizeResult,
        files_total: usize,
        destination: std::path::PathBuf,
        start_time: chrono::DateTime<Local>,
    ) -> Self {
        Self {
            files_organized: result.files_organized,
            files_total,
            destination,
            success: result.success,
            skipped_duplicates: result.skipped_duplicates,
            errors: result.errors,
            start_time,
        }
    }

    fn error(
        e: &color_eyre::eyre::Error,
        files_total: usize,
        destination: std::path::PathBuf,
        start_time: chrono::DateTime<Local>,
    ) -> Self {
        Self {
            files_organized: 0,
            files_total,
            destination,
            success: false,
            skipped_duplicates: 0,
            errors: vec![e.to_string()],
            start_time,
        }
    }

    fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    fn error_count(&self) -> usize {
        self.errors.len()
    }

    fn convert_to_organize_result(self) -> visualvault_models::OrganizeResult {
        visualvault_models::OrganizeResult {
            files_organized: self.files_organized,
            files_total: self.files_total,
            destination: self.destination,
            success: self.success,
            timestamp: self.start_time,
            skipped_duplicates: self.skipped_duplicates,
            errors: self.errors,
        }
    }
}

impl App {
    /// Starts a scan of the configured source folder.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No source folder is configured
    /// - The scanner fails to scan the directory
    pub async fn start_scan(&mut self) -> Result<()> {
        self.prepare_scan_state().await?;

        let source = self.get_source_folder().await?;
        let settings = self.settings.read().await.clone();

        let scan_params = self.build_scan_parameters(&source, &settings);
        let scan_result = self.execute_scan(scan_params).await;

        self.process_scan_result(scan_result).await
    }

    /// Prepares the application state for scanning
    async fn prepare_scan_state(&mut self) -> Result<()> {
        self.success_message = Some("Starting scan...".to_string());
        self.state = AppState::Scanning;
        self.progress.write().await.reset();
        Ok(())
    }

    /// Gets the configured source folder
    async fn get_source_folder(&self) -> Result<std::path::PathBuf> {
        let settings = self.settings.read().await;
        let source = settings
            .source_folder
            .clone()
            .ok_or_else(|| color_eyre::eyre::eyre!("Source folder not configured"))?;
        drop(settings);
        info!("Scanner: Starting scan of {:?}", source);
        Ok(source)
    }

    /// Builds scan parameters from current state
    fn build_scan_parameters(
        &self,
        source: &std::path::Path,
        settings: &visualvault_config::Settings,
    ) -> ScanParameters {
        ScanParameters {
            source: source.to_path_buf(),
            recursive: settings.recurse_subfolders,
            scanner: Arc::clone(&self.scanner),
            progress: Arc::clone(&self.progress),
            filter_set: if self.filter_set.is_active {
                Some(self.filter_set.clone())
            } else {
                None
            },
        }
    }

    /// Executes the scan with the given parameters
    async fn execute_scan(
        &self,
        params: ScanParameters,
    ) -> Result<(Vec<Arc<visualvault_models::MediaFile>>, DuplicateStats)> {
        let start_time = std::time::Instant::now();

        let (files, duplicates) = params
            .scanner
            .scan_directory_with_duplicates(
                &params.source,
                params.recursive,
                params.progress,
                &*self.settings.read().await,
                params.filter_set,
            )
            .await?;

        info!("Scan completed in {:?}", start_time.elapsed());
        Ok((files, duplicates))
    }

    /// Processes the scan result and updates application state
    async fn process_scan_result(
        &mut self,
        scan_result: Result<(Vec<Arc<visualvault_models::MediaFile>>, DuplicateStats)>,
    ) -> Result<()> {
        match scan_result {
            Ok((files, duplicates)) => self.handle_successful_scan(&files, duplicates).await,
            Err(e) => {
                self.handle_scan_error(&e);
                Err(e)
            }
        }
    }

    /// Handles a successful scan result
    async fn handle_successful_scan(
        &mut self,
        files: &[Arc<visualvault_models::MediaFile>],
        duplicates: DuplicateStats,
    ) -> Result<()> {
        Self::log_scan_results(files, &duplicates);
        self.update_scan_data(files, duplicates).await;
        self.create_scan_success_message(files.len());
        self.state = AppState::Dashboard;
        Ok(())
    }

    /// Logs scan results for debugging
    fn log_scan_results(files: &[Arc<visualvault_models::MediaFile>], duplicates: &DuplicateStats) {
        info!("=== SCAN RESULTS ===");
        info!("Scanner returned {} files", files.len());
        info!("Scanner returned {} duplicate groups", duplicates.len());
        info!("App cached_files now has {} entries", files.len());
    }

    /// Updates internal data structures with scan results
    async fn update_scan_data(&mut self, files: &[Arc<visualvault_models::MediaFile>], duplicates: DuplicateStats) {
        let files_found = files.len();

        self.statistics.update_from_scan_results(files, &duplicates);
        self.file_manager.write().await.set_files(files.to_vec());
        self.cached_files = files.to_vec();

        self.duplicate_groups = Self::convert_duplicate_groups(duplicates.groups);

        self.last_scan_result = Some(ScanResult {
            files_found,
            duration: std::time::Duration::from_secs(0), // This should be passed from execute_scan
            timestamp: Local::now(),
        });
    }

    /// Converts duplicate groups to the internal format
    fn convert_duplicate_groups(
        groups: Vec<visualvault_models::DuplicateGroup>,
    ) -> Option<Vec<Vec<visualvault_models::MediaFile>>> {
        if groups.is_empty() {
            None
        } else {
            Some(
                groups
                    .into_iter()
                    .map(|group| group.files.iter().map(|arc| (**arc).clone()).collect())
                    .collect(),
            )
        }
    }

    /// Creates the success message based on scan results
    fn create_scan_success_message(&mut self, files_found: usize) {
        let duplicate_count = self
            .duplicate_groups
            .as_ref()
            .map_or(0, |groups| groups.iter().map(|g| g.len().saturating_sub(1)).sum());

        self.success_message = if duplicate_count > 0 {
            Some(format!(
                "Scan complete: {files_found} files found ({duplicate_count} duplicates)"
            ))
        } else {
            Some(format!("Scan complete: {files_found} files found"))
        };
    }

    /// Handles scan errors
    fn handle_scan_error(&mut self, error: &color_eyre::eyre::Error) {
        error!("Scan failed: {}", error);
        self.error_message = Some(format!("Scan failed: {error}"));
        self.state = AppState::Dashboard;
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
        if !self.validate_organize_preconditions() {
            return Ok(());
        }

        self.prepare_organize_state().await?;

        let organize_params = self.build_organize_parameters().await?;
        let organize_result = self.execute_organization(organize_params).await;

        self.process_organize_result(organize_result);
        Ok(())
    }

    /// Validates that organization can proceed
    fn validate_organize_preconditions(&mut self) -> bool {
        if self.cached_files.is_empty() {
            self.error_message = Some("No files to organize. Run a scan first.".to_string());
            return false;
        }
        true
    }

    /// Prepares the application state for organizing
    async fn prepare_organize_state(&mut self) -> Result<()> {
        info!("Starting file organization");
        self.success_message = Some("Starting to organize files...".to_string());
        self.state = AppState::Organizing;
        self.progress.write().await.reset();
        Ok(())
    }

    /// Builds parameters needed for organization
    async fn build_organize_parameters(&self) -> Result<OrganizeParameters> {
        let settings = self.settings.read().await;
        let destination = settings
            .destination_folder
            .clone()
            .ok_or_else(|| color_eyre::eyre::eyre!("No destination folder configured"))?;

        let params = OrganizeParameters {
            files: self.cached_files.clone(),
            destination,
            rename_duplicates: settings.rename_duplicates,
            settings: settings.clone(),
            organizer: Arc::clone(&self.organizer),
            scanner: Arc::clone(&self.scanner),
            progress: Arc::clone(&self.progress),
            start_time: Local::now(),
        };
        drop(settings);

        Ok(params)
    }

    /// Executes the organization process
    async fn execute_organization(&self, params: OrganizeParameters) -> OrganizeExecutionResult {
        let mut files = params.files;
        let files_total = files.len();

        // Handle duplicates based on settings
        let duplicates = if params.rename_duplicates {
            DuplicateStats::new()
        } else {
            match params
                .scanner
                .find_duplicates(&mut files, params.progress.clone())
                .await
            {
                Ok(stats) => stats,
                Err(e) => {
                    return OrganizeExecutionResult::error(&e, files_total, params.destination, params.start_time);
                }
            }
        };

        // Perform organization
        match params
            .organizer
            .organize_files_with_duplicates(files, duplicates, &params.settings, params.progress)
            .await
        {
            Ok(result) => OrganizeExecutionResult::success(result, files_total, params.destination, params.start_time),
            Err(e) => OrganizeExecutionResult::error(&e, files_total, params.destination, params.start_time),
        }
    }

    /// Processes the organization result and updates application state
    fn process_organize_result(&mut self, result: OrganizeExecutionResult) {
        info!("Organization complete: {} files organized", result.files_organized);
        self.update_organize_state(result);
        self.clear_organize_data();
    }

    /// Updates the application state based on organization result
    fn update_organize_state(&mut self, result: OrganizeExecutionResult) {
        let message = Self::build_organize_message(&result);
        let has_errors = result.has_errors();

        self.last_organize_result = Some(result.convert_to_organize_result());

        if has_errors {
            self.error_message = Some(message);
        } else {
            self.success_message = Some(message);
        }

        self.state = AppState::Dashboard;
    }

    /// Builds the appropriate message based on organization result
    fn build_organize_message(result: &OrganizeExecutionResult) -> String {
        let base_message = if result.skipped_duplicates > 0 {
            format!(
                "Organization complete: {} files organized, {} duplicates skipped",
                result.files_organized, result.skipped_duplicates
            )
        } else {
            format!("Organization complete: {} files organized", result.files_organized)
        };

        if result.has_errors() {
            format!("{} (with {} errors)", base_message, result.error_count())
        } else {
            base_message
        }
    }

    /// Clears data used during organization
    fn clear_organize_data(&mut self) {
        self.cached_files.clear();
        self.duplicate_groups = None;
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
        self.cached_files = files.to_vec();
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
