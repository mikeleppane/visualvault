use crate::config::Settings;
use crate::core::file_cache::{CacheEntry, FileCache};
use crate::{
    models::{FileType, ImageMetadata, MediaFile, MediaMetadata},
    utils::Progress,
};
use chrono::{DateTime, Local, Utc};
use color_eyre::eyre::Result;
use image::GenericImageView;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::SystemTime;
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
                .filter_map(std::result::Result::ok)
                .filter(|e| {
                    if settings.skip_hidden_files {
                        !is_hidden(e.path())
                    } else {
                        true
                    }
                })
                .filter(|e| e.file_type().is_file())
                .filter(|e| Self::is_media_file(e.path()))
                .map(|e| e.path().to_path_buf())
                .collect()
        } else {
            std::fs::read_dir(path)?
                .filter_map(std::result::Result::ok)
                .filter(|e| {
                    if settings.skip_hidden_files {
                        !is_hidden(&e.path())
                    } else {
                        true
                    }
                })
                .filter(|e| e.file_type().ok().is_some_and(|ft| ft.is_file()))
                .map(|e| e.path())
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
            self.process_files_parallel(paths, progress, settings).await?
        } else {
            self.process_files_sequential(paths, progress).await?
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
    ) -> Result<Vec<MediaFile>> {
        let mut files = Vec::new();

        for (idx, path) in paths.iter().enumerate() {
            match self.process_file_with_cache(path).await {
                Ok(file) => {
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

                join_set.spawn(async move {
                    match scanner_clone.process_file_with_cache(&path_clone).await {
                        Ok(file) => {
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
            Self::system_time_to_datetime(metadata.modified()).map_or_else(Local::now, |dt| dt.with_timezone(&Local));

        // Check cache first
        {
            let cache = self.cache.lock().await;
            if let Some(entry) = cache.get(path, size, &modified) {
                let file_type = Self::determine_file_type(&entry.extension);
                let created = Self::system_time_to_datetime(metadata.created())
                    .map_or_else(|| modified, |dt| dt.with_timezone(&Local));

                tracing::trace!("Cache hit for: {}", path.display());
                return Ok(entry.to_media_file(file_type, created));
            }
        }

        // Cache miss, process the file
        tracing::trace!("Cache miss for: {}", path.display());
        let file = self.process_file(path).await?;

        // Update cache
        {
            let mut cache = self.cache.lock().await;
            let entry = CacheEntry::from(&file);
            cache.insert(path.to_path_buf(), entry);
        }

        Ok(file)
    }

    pub async fn find_duplicates(
        &self,
        files: &mut [MediaFile],
        progress: Arc<RwLock<Progress>>,
    ) -> Result<HashMap<String, Vec<MediaFile>>> {
        let mut hash_map: HashMap<String, Vec<MediaFile>> = HashMap::new();
        let mut cache_updated = false;

        // Update progress for hash calculation
        {
            let mut prog = progress.write().await;
            prog.total = files.len();
            prog.current = 0;
            prog.message = "Calculating file hashes...".to_string();
        }

        for (idx, file) in files.iter_mut().enumerate() {
            // Calculate hash if not already present
            if file.hash.is_none() {
                // Check if hash exists in cache
                let cached_hash = {
                    let cache = self.cache.lock().await;
                    cache
                        .get(&file.path, file.size, &file.modified)
                        .and_then(|entry| entry.hash.clone())
                };

                if let Some(hash) = cached_hash {
                    file.hash = Some(hash.clone());
                    hash_map.entry(hash).or_default().push(file.clone());
                } else {
                    // Calculate hash and update cache
                    match self.calculate_file_hash(&file.path).await {
                        Ok(hash) => {
                            file.hash = Some(hash.clone());

                            // Update cache with hash
                            {
                                let mut cache = self.cache.lock().await;
                                if let Some(entry) = cache.get_mut(&file.path, file.size, &file.modified) {
                                    entry.hash = Some(hash.clone());
                                    cache_updated = true;
                                }
                            }

                            hash_map.entry(hash).or_default().push(file.clone());
                        }
                        Err(e) => {
                            tracing::warn!("Failed to calculate hash for {}: {}", file.path.display(), e);
                            continue;
                        }
                    }
                }
            } else if let Some(hash) = &file.hash {
                hash_map.entry(hash.clone()).or_default().push(file.clone());
            }

            // Update progress
            {
                let mut prog = progress.write().await;
                prog.current = idx + 1;
                prog.message = format!("Checking duplicates: {}", file.name);
            }
        }

        // Save cache if updated
        if cache_updated {
            if let Err(e) = self.cache.lock().await.save().await {
                tracing::warn!("Failed to save cache: {}", e);
            }
        }

        // Keep only duplicates (more than one file with same hash)
        hash_map.retain(|_, files| files.len() > 1);

        Ok(hash_map)
    }

    pub async fn scan_directory_with_duplicates(
        &self,
        path: &Path,
        recursive: bool,
        progress: Arc<RwLock<Progress>>,
        settings: &Settings,
    ) -> Result<(Vec<MediaFile>, HashMap<String, Vec<MediaFile>>)> {
        // First, scan all files
        let mut files = self.scan_directory(path, recursive, progress.clone(), settings).await?;

        // Then detect duplicates if needed
        let duplicates = if settings.rename_duplicates {
            // If rename_duplicates is enabled, we don't need to detect duplicates
            HashMap::new()
        } else {
            // Calculate hashes and find duplicates
            self.find_duplicates(&mut files, progress).await?
        };

        Ok((files, duplicates))
    }

    async fn calculate_file_hash(&self, path: &Path) -> Result<String> {
        use tokio::io::AsyncReadExt;

        let file_size = tokio::fs::metadata(path).await?.len();

        // For small files (< 1MB), hash the entire file
        if file_size < 1024 * 1024 {
            return self.calculate_full_hash(path).await;
        }

        // For larger files, use a sampling approach
        let mut file = tokio::fs::File::open(path).await?;
        let mut hasher = Sha256::new();

        // Hash the first 4KB
        let mut buffer = vec![0; 4096];
        let n = file.read(&mut buffer).await?;
        hasher.update(&buffer[..n]);

        // Hash 4KB from the middle
        if file_size > 8192 {
            file.seek(std::io::SeekFrom::Start(file_size / 2)).await?;
            let n = file.read(&mut buffer).await?;
            hasher.update(&buffer[..n]);
        }

        // Hash the last 4KB
        if file_size > 4096 {
            file.seek(std::io::SeekFrom::End(-4096)).await?;
            let n = file.read(&mut buffer).await?;
            hasher.update(&buffer[..n]);
        }

        // Include file size in hash to ensure different sized files have different hashes
        hasher.update(file_size.to_le_bytes());

        Ok(format!("{:x}", hasher.finalize()))
    }

    async fn calculate_full_hash(&self, path: &Path) -> Result<String> {
        // Original full hash implementation
        use tokio::io::AsyncReadExt;

        let mut file = tokio::fs::File::open(path).await?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0; 8192];

        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    async fn process_file(&self, path: &Path) -> Result<MediaFile> {
        let metadata = tokio::fs::metadata(path).await?;
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();

        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

        let file_type = Self::determine_file_type(&extension);

        // Use modified time as the primary timestamp
        let modified =
            Self::system_time_to_datetime(metadata.modified()).map_or_else(Local::now, |dt| dt.with_timezone(&Local));

        // Still get created time but it's secondary
        let created =
            Self::system_time_to_datetime(metadata.created()).map_or_else(|| modified, |dt| dt.with_timezone(&Local)); // Fallback to modified if created is not available

        let mut media_file = MediaFile {
            path: path.to_path_buf(),
            name,
            extension: extension.clone(),
            file_type: file_type.clone(),
            size: metadata.len(),
            created,
            modified,
            hash: None,
            metadata: None,
        };

        // Only extract EXIF for images, and make it optional
        if file_type == FileType::Image && Self::should_extract_metadata() {
            // Don't fail if EXIF extraction fails
            if let Ok(Ok(metadata)) =
                tokio::time::timeout(std::time::Duration::from_secs(3), self.extract_image_metadata(path)).await
            {
                media_file.metadata = Some(metadata);
            }
        }

        Ok(media_file)
    }

    fn should_extract_metadata() -> bool {
        // Make metadata extraction optional or configurable
        false // For now, disable it for faster scanning
    }

    fn is_media_file(path: &Path) -> bool {
        const MEDIA_EXTENSIONS: &[&str] = &[
            // Images
            "jpg", "jpeg", "png", "gif", "bmp", "tiff", "webp", "raw", "heic", "heif", // Videos
            "mp4", "avi", "mkv", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", // Audio
            "mp3", "wav", "flac", "aac", "ogg", "wma", "m4a",
        ];

        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| MEDIA_EXTENSIONS.contains(&e.to_lowercase().as_str()))
    }

    fn determine_file_type(extension: &str) -> FileType {
        match extension {
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff" | "raw" | "heic" | "heif" => FileType::Image,
            "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg" => FileType::Video,
            "pdf" | "doc" | "docx" | "txt" | "odt" | "rtf" => FileType::Document,
            _ => FileType::Other,
        }
    }

    async fn extract_image_metadata(&self, path: &Path) -> Result<MediaMetadata> {
        let path = path.to_path_buf();

        tokio::task::spawn_blocking(move || {
            if let Ok(img) = image::open(&path) {
                let (width, height) = img.dimensions();
                let color = format!("{:?}", img.color());

                Ok(MediaMetadata::Image(ImageMetadata {
                    width,
                    height,
                    format: "unknown".to_string(),
                    color_type: color,
                }))
            } else {
                Err(color_eyre::eyre::eyre!("Failed to open image"))
            }
        })
        .await?
        .map_err(|e| color_eyre::eyre::eyre!(e))
    }
    pub async fn is_complete(&self) -> bool {
        !*self.is_scanning.lock().await
    }

    #[allow(clippy::cast_possible_wrap)]
    fn system_time_to_datetime(time: std::io::Result<SystemTime>) -> Option<DateTime<Utc>> {
        time.ok().and_then(|t| {
            t.duration_since(SystemTime::UNIX_EPOCH)
                .ok()
                .and_then(|d| DateTime::from_timestamp(d.as_secs() as i64, d.subsec_nanos()))
        })
    }
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
}
