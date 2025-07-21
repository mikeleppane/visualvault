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
                .filter(|e| !settings.skip_hidden_files || !is_hidden(e.path()))
                .filter(|e| Self::is_media_file(e.path()))
                .map(|e| e.path().to_path_buf())
                .collect()
        } else {
            std::fs::read_dir(path)?
                .par_bridge()
                .filter_map(std::result::Result::ok)
                .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
                .map(|e| e.path())
                .filter(|p| !settings.skip_hidden_files || !is_hidden(p))
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

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
}
