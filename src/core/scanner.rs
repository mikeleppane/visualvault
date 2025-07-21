use crate::config::Settings;
use crate::core::DuplicateDetector;
use crate::core::file_cache::{CacheEntry, FileCache};
use crate::models::filters::FilterSet;
use crate::utils::datetime::system_time_to_datetime;
use crate::utils::media_types::{MEDIA_EXTENSIONS, determine_file_type};
use crate::{models::MediaFile, utils::Progress};
use ahash::AHashMap;
use chrono::Local;
use color_eyre::eyre::Result;
use rayon::iter::{ParallelBridge, ParallelIterator};
use sha2::Digest;
use sha2::Sha256;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::{path::Path, sync::atomic::AtomicUsize};
use tokio::io::AsyncSeekExt;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct Scanner {
    is_scanning: Arc<Mutex<bool>>,
    cache: Arc<Mutex<FileCache>>,
}

impl Scanner {
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
    pub async fn scan_directory(
        &self,
        path: &Path,
        recursive: bool,
        progress: Arc<RwLock<Progress>>,
        settings: &Settings,
        filter_set: Option<FilterSet>,
    ) -> Result<Vec<MediaFile>> {
        info!("Scanner: Starting scan of {:?}", path);

        if !path.exists() {
            error!("Scanner: Path does not exist: {:?}", path);
            return Err(color_eyre::eyre::eyre!("Path does not exist"));
        }
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
                .filter(|e| Self::is_media_file(e.path()))
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
                .filter(|p| Self::is_media_file(p))
                .collect()
        };

        info!("Scanner: Found {} media files in {:?}", paths.len(), path);

        // Update progress total
        {
            let mut prog = progress.write().await;
            prog.total = paths.len();
        }

        drop(cache); // Release lock before processing

        // Process files with cache support
        let files = if settings.parallel_processing && settings.worker_threads > 1 {
            self.process_files_parallel(paths, progress, settings, filter_set)
                .await?
        } else {
            self.process_files_sequential(paths, progress, filter_set).await?
        };

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
        paths: Vec<PathBuf>,
        progress: Arc<RwLock<Progress>>,
        filter_set: Option<FilterSet>,
    ) -> Result<Vec<MediaFile>> {
        let mut files = Vec::new();

        for (idx, path) in paths.iter().enumerate() {
            match self.process_file_with_cache(path).await {
                Ok(file) => {
                    if let Some(filters) = &filter_set {
                        if filters.is_active && !filters.matches_file(&file) {
                            continue; // Skip files that don't match filters
                        }
                    }
                    files.push(file);

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

    async fn process_files_parallel(
        &self,
        paths: Vec<PathBuf>,
        progress: Arc<RwLock<Progress>>,
        settings: &Settings,
        filter_set: Option<FilterSet>,
    ) -> Result<Vec<MediaFile>> {
        use tokio::task::JoinSet;

        info!("process_files_parallel: Starting with {} paths", paths.len());

        let mut join_set = JoinSet::new();
        let scanner = Arc::new(self.clone());
        let progress_counter = Arc::new(AtomicUsize::new(0));
        let mut files = Vec::new();

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
                    files.push(file);
                }
            }
        }

        // Make sure we collect any remaining results
        while let Some(result) = join_set.join_next().await {
            if let Ok(Some(file)) = result {
                files.push(file);
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
        let cache_result = {
            let cache = self.cache.lock().await;
            cache.get(path, size, &modified).cloned()
        };

        if let Some(entry) = cache_result {
            // Cache hit - reconstruct MediaFile without re-reading metadata
            let file_type = determine_file_type(&entry.extension);
            let created =
                system_time_to_datetime(metadata.created()).map_or_else(|| modified, |dt| dt.with_timezone(&Local));

            tracing::trace!("Cache hit for: {}", path.display());
            return Ok(entry.to_media_file(file_type, created));
        }

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
    pub async fn find_duplicates(
        &self,
        files: &mut [MediaFile],
        _progress: Arc<RwLock<Progress>>,
    ) -> Result<AHashMap<String, Vec<MediaFile>>> {
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

        // Convert DuplicateStats to the format expected by Scanner
        let mut hash_map: AHashMap<String, Vec<MediaFile>> = AHashMap::new();

        for group in duplicate_stats.groups {
            if group.files.len() > 1 {
                // Use the first file's hash as the group identifier
                // If no hash exists, calculate one for consistency
                let hash = if let Some(ref h) = group.files[0].hash {
                    h.clone()
                } else {
                    // Calculate a hash based on file content for grouping
                    match self.calculate_file_hash(&group.files[0].path).await {
                        Ok(h) => h,
                        Err(e) => {
                            tracing::warn!("Failed to calculate hash for {}: {}", group.files[0].path.display(), e);
                            continue;
                        }
                    }
                };

                // Update all files in the group with the same hash
                for file in &group.files {
                    if let Some(file_mut) = files.iter_mut().find(|f| f.path == file.path) {
                        file_mut.hash = Some(hash.clone());
                    }
                }

                // Add to hash map
                hash_map.insert(hash, group.files);
            }
        }

        // Update cache with the calculated hashes
        if !hash_map.is_empty() {
            let mut cache = self.cache.lock().await;
            let mut cache_updated = false;

            for file in files.iter() {
                if let Some(hash) = &file.hash {
                    if let Some(entry) = cache.get_mut(&file.path, file.size, &file.modified) {
                        if entry.hash.is_none() {
                            entry.hash = Some(hash.clone());
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

        info!("Scanner: Converted to {} duplicate groups", hash_map.len());

        // Log details about duplicates found
        let total_duplicates: usize = hash_map.values().map(|group| group.len().saturating_sub(1)).sum();
        let wasted_space: u64 = hash_map
            .values()
            .map(|group| group.iter().skip(1).map(|f| f.size).sum::<u64>())
            .sum();

        if !hash_map.is_empty() {
            info!(
                "Scanner: Total duplicate files: {}, wasted space: {} bytes",
                total_duplicates, wasted_space
            );
        }

        Ok(hash_map)
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
    ) -> Result<(Vec<MediaFile>, AHashMap<String, Vec<MediaFile>>)> {
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

    #[allow(clippy::cast_possible_truncation)]
    async fn calculate_file_hash(&self, path: &Path) -> Result<String> {
        use tokio::io::AsyncReadExt;

        let file_size = tokio::fs::metadata(path).await?.len();

        // For small files (< 1MB), hash the entire file
        if file_size < 1024 * 1024 {
            return self.calculate_full_hash(path).await;
        }

        // For larger files, use a sampling approach with larger buffers
        let mut file = tokio::fs::File::open(path).await?;
        let mut hasher = Sha256::new();

        // Use larger buffer for better I/O performance
        let buffer_size = (64 * 1024).min(file_size as usize); // 64KB or file size
        let mut buffer = vec![0; buffer_size];

        // Hash the first chunk
        let n = file.read(&mut buffer).await?;
        hasher.update(&buffer[..n]);

        // For files > 128KB, sample middle and end
        if file_size > 128 * 1024 {
            // Middle sample
            file.seek(std::io::SeekFrom::Start(file_size / 2)).await?;
            let n = file.read(&mut buffer).await?;
            hasher.update(&buffer[..n]);

            // End sample
            let end_pos = file_size.saturating_sub(buffer_size as u64);
            file.seek(std::io::SeekFrom::Start(end_pos)).await?;
            let n = file.read(&mut buffer).await?;
            hasher.update(&buffer[..n]);
        }

        // Include file size in hash
        hasher.update(file_size.to_le_bytes());

        Ok(format!("{:x}", hasher.finalize()))
    }

    async fn calculate_full_hash(&self, path: &Path) -> Result<String> {
        use tokio::io::AsyncReadExt;

        let mut file = tokio::fs::File::open(path).await?;
        let mut hasher = Sha256::new();

        // Use larger buffer for better performance
        let mut buffer = vec![0; 64 * 1024]; // 64KB buffer

        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(format!("{:x}", hasher.finalize()))
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
            extension: extension.clone(),
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
    use crate::models::FileType;

    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

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

        assert_eq!(duplicates.len(), 1);
        let (_, group) = duplicates.iter().next().unwrap();
        assert_eq!(group.len(), 2);

        // Verify hashes were updated
        assert!(
            files
                .iter()
                .filter(|f| f.name.starts_with("file"))
                .all(|f| f.hash.is_some())
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

        let (_, dup_group) = duplicates.iter().next().unwrap();
        assert_eq!(dup_group.len(), 2);
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
    async fn test_calculate_file_hash_small_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("small.txt");
        create_test_file(&file_path, b"Hello, World!").await?;

        let scanner = Scanner::with_cache().await?;
        let hash = scanner.calculate_file_hash(&file_path).await?;

        // Hash should be consistent
        let hash2 = scanner.calculate_file_hash(&file_path).await?;
        assert_eq!(hash, hash2);

        // Hash should be hex string
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        Ok(())
    }

    #[tokio::test]
    async fn test_calculate_file_hash_large_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("large.bin");

        // Create a 2MB file
        let data = vec![0u8; 2 * 1024 * 1024];
        create_test_file(&file_path, &data).await?;

        let scanner = Scanner::with_cache().await?;
        let hash = scanner.calculate_file_hash(&file_path).await?;

        // Hash should be consistent
        let hash2 = scanner.calculate_file_hash(&file_path).await?;
        assert_eq!(hash, hash2);
        Ok(())
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
}
