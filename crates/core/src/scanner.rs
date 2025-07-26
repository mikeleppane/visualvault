use chrono::Local;
use color_eyre::eyre::Result;
use rayon::iter::{ParallelBridge, ParallelIterator};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::{path::Path, sync::atomic::AtomicUsize};
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info};
use visualvault_config::Settings;
use visualvault_models::{DuplicateStats, FilterSet, MediaFile};
use visualvault_utils::Progress;
use visualvault_utils::datetime::system_time_to_datetime;
use visualvault_utils::media_types::{MEDIA_EXTENSIONS, determine_file_type};
use walkdir::WalkDir;

use crate::DuplicateDetector;
use crate::file_cache::{CacheEntry, FileCache};

#[derive(Debug, Clone)]
pub struct Scanner {
    is_scanning: Arc<Mutex<bool>>,
    cache: Arc<Mutex<FileCache>>,
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner {
    #[must_use]
    pub fn new() -> Self {
        Self {
            is_scanning: Arc::new(Mutex::new(false)),
            cache: Arc::new(Mutex::new(FileCache::new())),
        }
    }
    /// Creates a new Scanner instance with cache support.
    ///
    /// # Errors
    ///
    /// This function currently always returns `Ok`, but the `Result` type is used
    /// for future compatibility in case cache loading might fail.
    pub async fn with_cache() -> Result<Self> {
        let cache = FileCache::load().await.unwrap_or_else(|_| {
            tracing::warn!("Failed to load cache, starting fresh");
            FileCache::new()
        });

        Ok(Self {
            is_scanning: Arc::new(Mutex::new(false)),
            cache: Arc::new(Mutex::new(cache)),
        })
    }

    #[allow(dead_code)]
    pub async fn cache_size(&self) -> usize {
        self.cache.lock().await.len()
    }

    /// Scans a directory for media files and returns a list of `MediaFile` objects.
    ///
    /// # Arguments
    ///
    /// * `path` - The directory path to scan
    /// * `recursive` - Whether to scan subdirectories recursively
    /// * `progress` - Progress tracker for the scanning operation
    /// * `settings` - Scanner settings configuration
    /// * `filter_set` - Optional filter set to apply to found files
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The specified path does not exist
    /// - There are I/O errors while reading directory contents or file metadata
    /// - File processing fails during parallel or sequential processing
    #[allow(clippy::cognitive_complexity)]
    pub async fn scan_directory(
        &self,
        path: &Path,
        recursive: bool,
        progress: Arc<RwLock<Progress>>,
        settings: &Settings,
        filter_set: Option<FilterSet>,
    ) -> Result<Vec<Arc<MediaFile>>> {
        info!("Scanner: Starting scan of {:?}", path);

        if !path.exists() {
            error!("Scanner: Path does not exist: {:?}", path);
            return Err(color_eyre::eyre::eyre!("Path does not exist"));
        }

        let scan_all_types = matches!(settings.organize_by.as_str(), "type");

        info!(
            "Scanner: Scanning {} files in {:?} (recursive: {}, all types: {}, organize by: {})",
            if recursive { "all" } else { "top-level" },
            path,
            recursive,
            scan_all_types,
            settings.organize_by
        );

        // Load cache at the start
        let mut cache = self.cache.lock().await;

        // Remove stale entries periodically
        if cache.len() > 1000 {
            cache.remove_stale_entries().await;
        }

        info!("Scanner: Cache loaded with {} entries", cache.len());

        // Collect all paths first
        let paths: Vec<PathBuf> = if recursive {
            WalkDir::new(path)
                .into_iter()
                .par_bridge()
                .filter_map(std::result::Result::ok)
                .filter(|e| e.file_type().is_file())
                .filter(|e| {
                    if settings.skip_hidden_files && is_hidden_in_path(e.path()) {
                        false // Skip this file
                    } else {
                        true // Include this file
                    }
                })
                .filter(|e| {
                    if scan_all_types {
                        true
                    } else {
                        Self::is_media_file(e.path())
                    }
                })
                .map(|e| e.path().to_path_buf())
                .collect()
        } else {
            std::fs::read_dir(path)?
                .par_bridge()
                .filter_map(std::result::Result::ok)
                .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
                .map(|e| e.path())
                .filter(|p| {
                    if settings.skip_hidden_files && is_hidden_in_path(p) {
                        false // Skip this file
                    } else {
                        true // Include this file
                    }
                })
                .filter(|p| if scan_all_types { true } else { Self::is_media_file(p) })
                .collect()
        };

        info!("Scanner: Found {} files in {:?}", paths.len(), path);

        // Update progress total
        {
            let mut prog = progress.write().await;
            prog.total = paths.len();
        }

        drop(cache); // Release lock before processing

        // Process files with cache support
        let files = if settings.parallel_processing && settings.worker_threads > 1 {
            self.process_files_parallel(&paths, progress, settings, filter_set)
                .await?
        } else {
            self.process_files_sequential(&paths, progress, filter_set).await?
        };

        // Log file type distribution if organizing by type
        if scan_all_types && !files.is_empty() {
            let mut type_counts = std::collections::HashMap::new();
            for file in &files {
                *type_counts.entry(file.file_type.clone()).or_insert(0) += 1;
            }

            info!("Scanner: File type distribution:");
            for (file_type, count) in type_counts {
                info!("  {}: {} files", file_type, count);
            }
        }

        // Save cache after processing
        if (self.cache.lock().await.save().await).is_ok() {
            tracing::debug!("Cache saved successfully");
            // print also how many cache entries were saved
            //tracing::debug!("Cache entries saved: {}", self.cache.lock().await.len());
        }

        Ok(files)
    }

    async fn process_files_sequential(
        &self,
        paths: &[PathBuf],
        progress: Arc<RwLock<Progress>>,
        filter_set: Option<FilterSet>,
    ) -> Result<Vec<Arc<MediaFile>>> {
        let mut files: Vec<Arc<MediaFile>> = Vec::new();

        for (idx, path) in paths.iter().enumerate() {
            match self.process_file_with_cache(path).await {
                Ok(file) => {
                    if let Some(filters) = &filter_set {
                        if filters.is_active && !filters.matches_file(&file) {
                            continue; // Skip files that don't match filters
                        }
                    }
                    files.push(file.into());

                    let mut prog = progress.write().await;
                    prog.current = idx + 1;
                    prog.message = format!("Scanning: {}", path.file_name().unwrap_or_default().to_string_lossy());
                }
                Err(e) => {
                    tracing::warn!("Failed to process file {:?}: {}", path, e);
                }
            }
        }

        Ok(files)
    }

    #[allow(clippy::cognitive_complexity)]
    async fn process_files_parallel(
        &self,
        paths: &[PathBuf],
        progress: Arc<RwLock<Progress>>,
        settings: &Settings,
        filter_set: Option<FilterSet>,
    ) -> Result<Vec<Arc<MediaFile>>> {
        use tokio::task::JoinSet;

        info!("process_files_parallel: Starting with {} paths", paths.len());

        let mut join_set = JoinSet::new();
        let scanner = Arc::new(self.clone());
        let progress_counter = Arc::new(AtomicUsize::new(0));
        let mut files: Vec<Arc<MediaFile>> = Vec::new();

        // Process files in chunks
        let chunk_size = settings.worker_threads * 10;

        for chunk in paths.chunks(chunk_size) {
            // Spawn tasks for this chunk
            for path in chunk {
                let scanner_clone = Arc::clone(&scanner);
                let progress_clone = Arc::clone(&progress);
                let progress_counter_clone = Arc::clone(&progress_counter);
                let path_clone = path.clone();
                let filter_set_clone = filter_set.clone();

                join_set.spawn(async move {
                    match scanner_clone.process_file_with_cache(&path_clone).await {
                        Ok(file) => {
                            if let Some(filters) = &filter_set_clone {
                                if filters.is_active && !filters.matches_file(&file) {
                                    return None; // Skip files that don't match filters
                                }
                            }

                            let current = progress_counter_clone.fetch_add(1, Ordering::SeqCst) + 1;

                            if let Ok(mut prog) = progress_clone.try_write() {
                                prog.current = current;
                                prog.message = format!("Scanning: {}", file.name);
                            }

                            Some(file)
                        }
                        Err(e) => {
                            tracing::warn!("Failed to process file {:?}: {}", path_clone, e);
                            None
                        }
                    }
                });
            }

            // Wait for this chunk to complete before starting the next one
            while let Some(result) = join_set.join_next().await {
                if let Ok(Some(file)) = result {
                    files.push(file.into());
                }
            }
        }

        // Make sure we collect any remaining results
        while let Some(result) = join_set.join_next().await {
            if let Ok(Some(file)) = result {
                files.push(file.into());
            }
        }

        info!("process_files_parallel: Collected {} files", files.len());

        Ok(files)
    }

    async fn process_file_with_cache(&self, path: &Path) -> Result<MediaFile> {
        let metadata = tokio::fs::metadata(path).await?;
        let size = metadata.len();
        let modified =
            system_time_to_datetime(metadata.modified()).map_or_else(Local::now, |dt| dt.with_timezone(&Local));

        // Try cache lookup with minimal locking
        let cache = self.cache.lock().await;
        if let Some(entry) = cache.get(path, size, &modified) {
            let file_type = determine_file_type(&entry.extension);
            let created =
                system_time_to_datetime(metadata.created()).map_or_else(|| modified, |dt| dt.with_timezone(&Local));

            tracing::trace!("Cache hit for: {}", path.display());
            return Ok(entry.to_media_file(file_type, created));
        }
        drop(cache); // Explicitly drop the lock

        // Cache miss - process file
        tracing::trace!("Cache miss for: {}", path.display());
        let file = Self::process_file(path, &metadata, size, modified);

        // Update cache asynchronously
        {
            let mut cache = self.cache.lock().await;
            cache.insert(path.to_path_buf(), CacheEntry::from(&file));
        }

        Ok(file)
    }

    /// Finds duplicate files among the provided media files.
    ///
    /// # Arguments
    ///
    /// * `files` - Mutable slice of media files to check for duplicates
    /// * `_progress` - Progress tracker (currently unused)
    ///
    /// # Returns
    ///
    /// Returns a hash map where keys are file hashes and values are vectors of duplicate files.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The duplicate detection process fails
    /// - Hash calculation for files fails during duplicate detection
    /// - Cache operations fail when updating file hashes
    #[allow(clippy::cognitive_complexity)]
    pub async fn find_duplicates(
        &self,
        files: &mut [Arc<MediaFile>],
        _progress: Arc<RwLock<Progress>>,
    ) -> Result<DuplicateStats> {
        info!(
            "Scanner: Using DuplicateDetector to find duplicates for {} files",
            files.len()
        );

        // Create a new DuplicateDetector instance
        let detector = DuplicateDetector::new();

        // Use the DuplicateDetector to find duplicates
        // Use full hash (false) for accuracy, matching what works in the UI
        let duplicate_stats = detector.detect_duplicates(files, false).await?;

        info!(
            "DuplicateDetector found {} duplicate groups",
            duplicate_stats.groups.len()
        );

        // Update cache with the calculated hashes
        if !duplicate_stats.is_empty() {
            let mut cache = self.cache.lock().await;
            let mut cache_updated = false;

            for file in files.iter() {
                if let Some(hash) = &file.hash {
                    if let Some(entry) = cache.get_mut(&file.path, file.size, &file.modified) {
                        if entry.hash.is_none() {
                            entry.hash = Some(hash.to_string());
                            cache_updated = true;
                        }
                    }
                }
            }

            if cache_updated {
                if let Err(e) = cache.save().await {
                    tracing::warn!("Failed to save cache after updating hashes: {}", e);
                }
            }
        }

        info!("Scanner: Converted to {} duplicate groups", duplicate_stats.len());

        if !duplicate_stats.is_empty() {
            info!(
                "Scanner: Total duplicate files: {}, wasted space: {} bytes",
                duplicate_stats.total_files(),
                duplicate_stats.total_size()
            );
        }

        Ok(duplicate_stats)
    }

    /// Scans a directory for media files and detects duplicates in one operation.
    ///
    /// This function combines directory scanning and duplicate detection for efficiency.
    /// It first scans the directory for media files, then analyzes them for duplicates.
    ///
    /// # Arguments
    ///
    /// * `path` - The directory path to scan
    /// * `recursive` - Whether to scan subdirectories recursively
    /// * `progress` - Progress tracker for the scanning and duplicate detection operations
    /// * `settings` - Scanner settings configuration
    /// * `filter_set` - Optional filter set to apply to found files
    ///
    /// # Returns
    ///
    /// Returns a tuple containing:
    /// - A vector of all media files found
    /// - A hash map where keys are file hashes and values are vectors of duplicate files
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The directory scanning fails (path doesn't exist, I/O errors, etc.)
    /// - Duplicate detection fails during hash calculation or file processing
    /// - Cache operations fail when updating file information
    pub async fn scan_directory_with_duplicates(
        &self,
        path: &Path,
        recursive: bool,
        progress: Arc<RwLock<Progress>>,
        settings: &Settings,
        filter_set: Option<FilterSet>,
    ) -> Result<(Vec<Arc<MediaFile>>, DuplicateStats)> {
        // First, scan all files
        let mut files = self
            .scan_directory(path, recursive, progress.clone(), settings, filter_set)
            .await?;

        info!("Scanner: Found {} files, checking for duplicates...", files.len());

        // Reset progress for duplicate detection
        {
            let mut prog = progress.write().await;
            prog.current = 0;
            prog.total = files.len();
            prog.message = "Detecting duplicates...".to_string();
        }

        // Find duplicates using DuplicateDetector
        let duplicates = self.find_duplicates(&mut files, progress).await?;

        info!("Scanner: Found {} duplicate groups", duplicates.len());

        Ok((files, duplicates))
    }

    fn process_file(
        path: &Path,
        metadata: &std::fs::Metadata,
        size: u64,
        modified: chrono::DateTime<Local>,
    ) -> MediaFile {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();

        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

        let file_type = determine_file_type(&extension);

        let created =
            system_time_to_datetime(metadata.created()).map_or_else(|| modified, |dt| dt.with_timezone(&Local));

        MediaFile {
            path: path.to_path_buf(),
            name,
            extension,
            file_type,
            size,
            created,
            modified,
            hash: None,
            metadata: None, // Skip metadata extraction for performance
        }
    }

    fn is_media_file(path: &Path) -> bool {
        path.to_str().is_some_and(|s| MEDIA_EXTENSIONS.is_match(s))
    }

    pub async fn is_complete(&self) -> bool {
        !*self.is_scanning.lock().await
    }
}

fn is_hidden_in_path(path: &Path) -> bool {
    // Check if any component in the path starts with '.' (except for current dir)
    path.components().any(|component| {
        if let std::path::Component::Normal(name) = component {
            name.to_str().is_some_and(|s| s.starts_with('.'))
        } else {
            false
        }
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::float_cmp)] // For comparing floats in tests
    #![allow(clippy::panic)]

    use super::*;
    use tempfile::TempDir;
    use tokio::fs;
    use visualvault_models::FileType;

    // Helper function to create test files
    async fn create_test_file(path: &Path, content: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, content).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_scanner_creation() -> Result<()> {
        let scanner = Scanner::with_cache().await?;
        assert!(scanner.is_complete().await);
        Ok(())
    }

    #[tokio::test]
    async fn test_scan_empty_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let scanner = Scanner::with_cache().await?;
        let progress = Arc::new(RwLock::new(Progress::default()));
        let settings = Settings::default();

        let files = scanner
            .scan_directory(temp_dir.path(), false, progress, &settings, None)
            .await?;

        assert_eq!(files.len(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_scan_non_existent_directory() -> Result<()> {
        let scanner = Scanner::with_cache().await?;
        let progress = Arc::new(RwLock::new(Progress::default()));
        let settings = Settings::default();

        let result = scanner
            .scan_directory(Path::new("/non/existent/path"), false, progress, &settings, None)
            .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Path does not exist");
        Ok(())
    }

    #[tokio::test]
    async fn test_scan_media_files_non_recursive() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create media files in root
        create_test_file(&root.join("image1.jpg"), b"JPG_DATA").await?;
        create_test_file(&root.join("image2.png"), b"PNG_DATA").await?;
        create_test_file(&root.join("video1.mp4"), b"MP4_DATA").await?;

        // Create files in subdirectory (should be ignored in non-recursive scan)
        create_test_file(&root.join("subdir/image3.jpg"), b"JPG_DATA").await?;

        let scanner = Scanner::with_cache().await?;
        let progress = Arc::new(RwLock::new(Progress::default()));
        let settings = Settings {
            recurse_subfolders: false,
            ..Default::default()
        };

        let files = scanner.scan_directory(root, false, progress, &settings, None).await?;

        assert_eq!(files.len(), 3);
        assert!(files.iter().all(|f| f.path.parent() == Some(root)));
        Ok(())
    }

    #[tokio::test]
    async fn test_scan_media_files_recursive() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create media files in various directories
        create_test_file(&root.join("image1.jpg"), b"JPG_DATA").await?;
        create_test_file(&root.join("subdir1/image2.png"), b"PNG_DATA").await?;
        create_test_file(&root.join("subdir1/subdir2/video1.mp4"), b"MP4_DATA").await?;
        create_test_file(&root.join("subdir3/image3.gif"), b"GIF_DATA").await?;

        let scanner = Scanner::with_cache().await?;
        let progress = Arc::new(RwLock::new(Progress::default()));
        let settings = Settings {
            recurse_subfolders: true,
            ..Default::default()
        };

        let files = scanner.scan_directory(root, true, progress, &settings, None).await?;

        assert_eq!(files.len(), 4);

        // Verify all file types are detected correctly
        let images = files.iter().filter(|f| matches!(f.file_type, FileType::Image)).count();
        let videos = files.iter().filter(|f| matches!(f.file_type, FileType::Video)).count();

        assert_eq!(images, 3);
        assert_eq!(videos, 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_hidden_files_handling() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create regular and hidden files
        create_test_file(&root.join("visible.jpg"), b"JPG_DATA").await?;
        create_test_file(&root.join(".hidden.jpg"), b"JPG_DATA").await?;
        create_test_file(&root.join(".hidden_dir/image.jpg"), b"JPG_DATA").await?;

        let scanner = Scanner::with_cache().await?;
        let progress = Arc::new(RwLock::new(Progress::default()));

        // Test with skip_hidden_files = true
        let settings = Settings {
            skip_hidden_files: true,
            recurse_subfolders: true,
            ..Default::default()
        };

        let files = scanner
            .scan_directory(root, true, progress.clone(), &settings, None)
            .await?;

        assert_eq!(files.len(), 0);

        // Test with skip_hidden_files = false
        let settings = Settings {
            skip_hidden_files: false,
            recurse_subfolders: true,
            ..Default::default()
        };

        let files = scanner.scan_directory(root, true, progress, &settings, None).await?;

        assert_eq!(files.len(), 3);
        Ok(())
    }

    #[tokio::test]
    async fn test_filter_set_application() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create files of different sizes
        create_test_file(&root.join("small.jpg"), &vec![0; 512 * 1024]).await?; // 512KB
        create_test_file(&root.join("medium.jpg"), &vec![0; 2 * 1024 * 1024]).await?; // 2MB
        create_test_file(&root.join("large.jpg"), &vec![0; 5 * 1024 * 1024]).await?; // 5MB

        let scanner = Scanner::with_cache().await?;
        let progress = Arc::new(RwLock::new(Progress::default()));
        let settings = Settings::default();

        // Create filter for files larger than 1MB
        let mut filter = FilterSet::new();
        filter.add_size_range("Large files".to_string(), Some(1.0), None);
        filter.is_active = true;

        let files = scanner
            .scan_directory(root, false, progress, &settings, Some(filter))
            .await?;

        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.size > 1024 * 1024));
        Ok(())
    }

    #[tokio::test]
    async fn test_cache_functionality() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        create_test_file(&root.join("test.jpg"), b"JPG_DATA").await?;

        let scanner = Scanner::with_cache().await?;
        let progress = Arc::new(RwLock::new(Progress::default()));
        let settings = Settings::default();

        // First scan - should populate cache
        let files1 = scanner
            .scan_directory(root, false, progress.clone(), &settings, None)
            .await?;

        assert_eq!(files1.len(), 1);

        // Second scan - should use cache
        let files2 = scanner.scan_directory(root, false, progress, &settings, None).await?;

        assert_eq!(files2.len(), 1);
        assert_eq!(files1[0].path, files2[0].path);
        assert_eq!(files1[0].size, files2[0].size);
        Ok(())
    }

    #[tokio::test]
    async fn test_find_duplicates_empty_list() -> Result<()> {
        let scanner = Scanner::with_cache().await?;
        let progress = Arc::new(RwLock::new(Progress::default()));
        let mut files = vec![];

        let duplicates = scanner.find_duplicates(&mut files, progress).await?;
        assert!(duplicates.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_find_duplicates_with_duplicates() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create duplicate files
        create_test_file(&root.join("file1.jpg"), b"DUPLICATE_DATA").await?;
        create_test_file(&root.join("file2.jpg"), b"DUPLICATE_DATA").await?;
        create_test_file(&root.join("unique.jpg"), b"UNIQUE_DATA").await?;

        let scanner = Scanner::with_cache().await?;
        let progress = Arc::new(RwLock::new(Progress::default()));
        let settings = Settings::default();

        let mut files = scanner
            .scan_directory(root, false, progress.clone(), &settings, None)
            .await?;

        let duplicates = scanner.find_duplicates(&mut files, progress).await?;

        // Check the duplicate stats
        assert_eq!(duplicates.total_groups, 1);
        assert_eq!(duplicates.total_files(), 2);

        let group = duplicates.groups.first().unwrap();
        assert_eq!(group.files.len(), 2);

        // Verify that both duplicate files are in the group
        let file_names: Vec<&str> = group.files.iter().map(|f| f.name.as_str()).collect();
        assert!(file_names.contains(&"file1.jpg"));
        assert!(file_names.contains(&"file2.jpg"));

        // The unique file should not be in any duplicate group
        assert!(!group.files.iter().any(|f| f.name == "unique.jpg"));

        // After find_duplicates, the files should have hashes updated by DuplicateDetector
        // This happens inside detect_duplicates method
        assert!(
            duplicates
                .groups
                .iter()
                .all(|g| { g.files.iter().all(|f| f.hash.is_some()) }),
            "All files should have hashes after duplicate detection"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_scan_directory_with_duplicates() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create duplicate and unique files
        create_test_file(&root.join("dup1.jpg"), b"DUPLICATE").await?;
        create_test_file(&root.join("dup2.jpg"), b"DUPLICATE").await?;
        create_test_file(&root.join("unique1.jpg"), b"UNIQUE1").await?;
        create_test_file(&root.join("unique2.jpg"), b"UNIQUE2").await?;

        let scanner = Scanner::with_cache().await?;
        let progress = Arc::new(RwLock::new(Progress::default()));
        let settings = Settings::default();

        let (files, duplicates) = scanner
            .scan_directory_with_duplicates(root, false, progress, &settings, None)
            .await?;

        assert_eq!(files.len(), 4);
        assert_eq!(duplicates.len(), 1);

        assert_eq!(duplicates.groups.first().unwrap().files.len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn test_parallel_vs_sequential_processing() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create multiple files
        for i in 0..10 {
            create_test_file(&root.join(format!("image{i}.jpg")), b"DATA").await?;
        }

        let scanner = Scanner::with_cache().await?;
        let progress = Arc::new(RwLock::new(Progress::default()));

        // Test sequential processing
        let settings_seq = Settings {
            parallel_processing: false,
            ..Default::default()
        };

        let files_seq = scanner
            .scan_directory(root, false, progress.clone(), &settings_seq, None)
            .await?;

        // Test parallel processing
        let settings_par = Settings {
            parallel_processing: true,
            worker_threads: 4,
            ..Default::default()
        };

        let files_par = scanner
            .scan_directory(root, false, progress, &settings_par, None)
            .await?;

        assert_eq!(files_seq.len(), files_par.len());
        assert_eq!(files_seq.len(), 10);
        Ok(())
    }

    #[tokio::test]
    async fn test_is_media_file() {
        assert!(Scanner::is_media_file(Path::new("test.jpg")));
        assert!(Scanner::is_media_file(Path::new("test.PNG")));
        assert!(Scanner::is_media_file(Path::new("test.mp4")));
        assert!(Scanner::is_media_file(Path::new("test.AVI")));

        assert!(!Scanner::is_media_file(Path::new("test.txt")));
        assert!(!Scanner::is_media_file(Path::new("test.pdf")));
        assert!(!Scanner::is_media_file(Path::new("test")));
    }

    #[tokio::test]
    async fn test_is_hidden_in_path() {
        assert!(is_hidden_in_path(Path::new(".hidden")));
        assert!(is_hidden_in_path(Path::new(".hidden/file.jpg")));
        assert!(is_hidden_in_path(Path::new("path/.hidden/file.jpg")));
        assert!(is_hidden_in_path(Path::new("path/to/.hidden")));

        assert!(!is_hidden_in_path(Path::new("visible")));
        assert!(!is_hidden_in_path(Path::new("path/to/file.jpg")));
        assert!(!is_hidden_in_path(Path::new("")));
    }

    #[tokio::test]
    async fn test_process_file_metadata() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.jpg");
        create_test_file(&file_path, b"JPG_DATA").await?;

        let metadata = tokio::fs::metadata(&file_path).await?;
        let size = metadata.len();
        let modified =
            system_time_to_datetime(metadata.modified()).map_or_else(Local::now, |dt| dt.with_timezone(&Local));

        let file = Scanner::process_file(&file_path, &metadata, size, modified);

        assert_eq!(file.name, "test.jpg");
        assert_eq!(file.extension, "jpg");
        assert_eq!(file.size, 8);
        assert!(matches!(file.file_type, FileType::Image));
        assert!(file.hash.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn test_progress_tracking() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create multiple files
        for i in 0..5 {
            create_test_file(&root.join(format!("file{i}.jpg")), b"DATA").await?;
        }

        let scanner = Scanner::with_cache().await?;
        let progress = Arc::new(RwLock::new(Progress::default()));
        let settings = Settings::default();

        let _ = scanner
            .scan_directory(root, false, progress.clone(), &settings, None)
            .await?;

        let prog = progress.read().await;
        assert_eq!(prog.total, 5);
        assert_eq!(prog.current, 5);
        assert!(!prog.message.is_empty());
        drop(prog);
        Ok(())
    }

    // Add this test case to the existing tests module

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn test_scan_all_file_types_with_type_organization() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create various file types to test comprehensive scanning
        // Images
        create_test_file(&root.join("photo.jpg"), b"JPG_DATA").await?;
        create_test_file(&root.join("screenshot.png"), b"PNG_DATA").await?;
        create_test_file(&root.join("raw_photo.cr2"), b"CR2_DATA").await?;

        // Videos
        create_test_file(&root.join("movie.mp4"), b"MP4_DATA").await?;
        create_test_file(&root.join("clip.avi"), b"AVI_DATA").await?;

        // Documents
        create_test_file(&root.join("document.pdf"), b"PDF_DATA").await?;
        create_test_file(&root.join("spreadsheet.xlsx"), b"XLSX_DATA").await?;
        create_test_file(&root.join("presentation.pptx"), b"PPTX_DATA").await?;
        create_test_file(&root.join("text.txt"), b"TXT_DATA").await?;
        create_test_file(&root.join("markdown.md"), b"MD_DATA").await?;

        // Audio files
        create_test_file(&root.join("song.mp3"), b"MP3_DATA").await?;
        create_test_file(&root.join("audio.flac"), b"FLAC_DATA").await?;

        // Archives
        create_test_file(&root.join("archive.zip"), b"ZIP_DATA").await?;
        create_test_file(&root.join("compressed.7z"), b"7Z_DATA").await?;

        // Other files
        create_test_file(&root.join("data.xml"), b"XML_DATA").await?;
        create_test_file(&root.join("config.json"), b"JSON_DATA").await?;

        // Files that should NOT be scanned (no extension or unsupported)
        create_test_file(&root.join("no_extension"), b"NO_EXT").await?;
        create_test_file(&root.join("executable.exe"), b"EXE_DATA").await?;

        let scanner = Scanner::new();
        let progress = Arc::new(RwLock::new(Progress::default()));

        // Test with organize by type mode - should scan ALL supported file types
        let settings_type = Settings {
            organize_by: "type".to_string(),
            ..Default::default()
        };

        let files_type = scanner
            .scan_directory(root, false, progress.clone(), &settings_type, None)
            .await?;

        // Should find all supported files (16 files)
        assert_eq!(
            files_type.len(),
            18,
            "Should scan all supported file types when organize_by = 'type'"
        );

        // Verify file type distribution
        let mut type_counts = std::collections::HashMap::new();
        for file in &files_type {
            *type_counts.entry(file.file_type.clone()).or_insert(0) += 1;
        }

        // Check that we have files of each type
        assert_eq!(
            type_counts.get(&FileType::Image).copied().unwrap_or(0),
            3,
            "Should find 3 image files"
        );
        assert_eq!(
            type_counts.get(&FileType::Video).copied().unwrap_or(0),
            2,
            "Should find 2 video files"
        );
        assert_eq!(
            type_counts.get(&FileType::Document).copied().unwrap_or(0),
            7,
            "Should find 7 document files"
        );
        assert_eq!(
            type_counts.get(&FileType::Other).copied().unwrap_or(0),
            6,
            "Should find 6 other files"
        );

        // Test with default mode (not organize by type) - should only scan media files
        let settings_default = Settings {
            organize_by: "monthly".to_string(),
            ..Default::default()
        };

        let files_default = scanner
            .scan_directory(root, false, progress.clone(), &settings_default, None)
            .await?;

        // Should only find image and video files (5 files)
        assert_eq!(
            files_default.len(),
            5,
            "Should only scan media files when organize_by != 'type'"
        );

        // Verify only media files are found
        for file in &files_default {
            assert!(
                matches!(file.file_type, FileType::Image | FileType::Video),
                "Should only find image and video files in default mode"
            );
        }

        // Test recursive scanning with type organization
        create_test_file(&root.join("subdir/nested.pdf"), b"PDF_DATA").await?;
        create_test_file(&root.join("subdir/image.jpg"), b"JPG_DATA").await?;
        create_test_file(&root.join("subdir/deep/audio.mp3"), b"MP3_DATA").await?;

        let files_recursive = scanner
            .scan_directory(root, true, progress.clone(), &settings_type, None)
            .await?;

        // Should find original 18 + 3 new files in subdirectories
        assert_eq!(files_recursive.len(), 21, "Should scan all files recursively");

        Ok(())
    }

    #[tokio::test]
    async fn test_scan_file_type_edge_cases() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Test case sensitivity
        create_test_file(&root.join("UPPERCASE.JPG"), b"JPG_DATA").await?;
        create_test_file(&root.join("MiXeD.PdF"), b"PDF_DATA").await?;
        create_test_file(&root.join("lowercase.mp3"), b"MP3_DATA").await?;

        // Test files with multiple dots
        create_test_file(&root.join("file.name.with.dots.jpg"), b"JPG_DATA").await?;
        create_test_file(&root.join("backup.tar.gz"), b"TAR_GZ_DATA").await?;

        // Test files with special characters
        create_test_file(&root.join("file with spaces.pdf"), b"PDF_DATA").await?;
        create_test_file(&root.join("file-with-dashes.mp4"), b"MP4_DATA").await?;
        create_test_file(&root.join("file_with_underscores.zip"), b"ZIP_DATA").await?;

        let scanner = Scanner::with_cache().await?;
        let progress = Arc::new(RwLock::new(Progress::default()));

        let settings = Settings {
            organize_by: "type".to_string(),
            ..Default::default()
        };

        let files = scanner.scan_directory(root, false, progress, &settings, None).await?;

        // All files should be scanned successfully
        assert_eq!(files.len(), 8, "Should handle all edge cases");

        // Verify case insensitive extension handling
        let jpg_files: Vec<_> = files
            .iter()
            .filter(|f| {
                matches!(f.file_type, FileType::Image) && (f.extension == "jpg" || f.extension == "JPG".to_lowercase())
            })
            .collect();
        assert_eq!(jpg_files.len(), 2, "Should handle uppercase and dotted JPG files");

        // Verify special character handling
        let special_char_files: Vec<_> = files
            .iter()
            .filter(|f| f.name.contains(' ') || f.name.contains('-') || f.name.contains('_'))
            .collect();
        assert_eq!(
            special_char_files.len(),
            3,
            "Should handle files with special characters"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_scan_performance_with_many_file_types() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // Create many files of different types to test performance
        let file_types = vec![
            ("jpg", "IMAGE", FileType::Image),
            ("pdf", "PDF", FileType::Document),
            ("mp3", "AUDIO", FileType::Other),
            ("zip", "ARCHIVE", FileType::Other),
            ("mp4", "VIDEO", FileType::Video),
        ];

        // Create 20 files of each type (100 files total)
        for (ext, data, _) in &file_types {
            for i in 0..20 {
                create_test_file(&root.join(format!("file_{i:03}.{ext}")), data.as_bytes()).await?;
            }
        }

        let scanner = Scanner::with_cache().await?;
        let progress = Arc::new(RwLock::new(Progress::default()));

        // Test with type organization
        let settings_type = Settings {
            organize_by: "type".to_string(),
            parallel_processing: true,
            worker_threads: 4,
            ..Default::default()
        };

        let start = std::time::Instant::now();
        let files = scanner
            .scan_directory(root, false, progress.clone(), &settings_type, None)
            .await?;
        let duration = start.elapsed();

        assert_eq!(files.len(), 100, "Should scan all 100 files");

        // Verify distribution
        let mut type_counts = std::collections::HashMap::new();
        for file in &files {
            *type_counts.entry(file.file_type.clone()).or_insert(0) += 1;
        }

        for (_, _, expected_type) in &file_types {
            match expected_type {
                FileType::Image => assert_eq!(type_counts.get(&FileType::Image).copied().unwrap_or(0), 20),
                FileType::Video => assert_eq!(type_counts.get(&FileType::Video).copied().unwrap_or(0), 20),
                FileType::Document => assert_eq!(type_counts.get(&FileType::Document).copied().unwrap_or(0), 20),
                FileType::Other => assert_eq!(type_counts.get(&FileType::Other).copied().unwrap_or(0), 40),
            }
        }

        println!("Scanned 100 files of mixed types in {duration:?}");

        // Compare with media-only scanning
        let settings_media = Settings {
            organize_by: "monthly".to_string(),
            parallel_processing: true,
            worker_threads: 4,
            ..Default::default()
        };

        let start_media = std::time::Instant::now();
        let files_media = scanner
            .scan_directory(root, false, progress, &settings_media, None)
            .await?;
        let duration_media = start_media.elapsed();

        // Should only find image and video files (40 files)
        assert_eq!(files_media.len(), 40, "Should only scan media files");

        println!("Scanned 40 media files in {duration_media:?}");

        Ok(())
    }
}
