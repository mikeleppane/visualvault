use crate::models::{MediaFile, MediaMetadata};
use ahash::AHashMap;
use chrono::{DateTime, Local};
use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCache {
    pub entries: AHashMap<PathBuf, CacheEntry>,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub path: PathBuf,
    pub name: String,
    pub extension: String,
    pub size: u64,
    pub modified: DateTime<Local>,
    pub hash: Option<String>,
    pub metadata: Option<MediaMetadata>,
}

impl FileCache {
    const CURRENT_VERSION: u32 = 1;

    pub fn new() -> Self {
        Self {
            entries: AHashMap::new(),
            version: Self::CURRENT_VERSION,
        }
    }

    pub async fn load() -> Result<Self> {
        let cache_path = Self::get_cache_path()?;

        if cache_path.exists() {
            let data = tokio::fs::read_to_string(&cache_path).await?;
            let cache: Self = serde_json::from_str(&data)?;

            // Check version compatibility
            if cache.version != Self::CURRENT_VERSION {
                tracing::info!("Cache version mismatch, creating new cache");
                return Ok(Self::new());
            }

            Ok(cache)
        } else {
            Ok(Self::new())
        }
    }

    pub async fn save(&self) -> Result<()> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| color_eyre::eyre::eyre!("Failed to get cache directory"))?
            .join("visualvault");

        tokio::fs::create_dir_all(&cache_dir).await?;

        let cache_path = cache_dir.join("file_cache.json");
        let data = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&cache_path, data).await?;

        Ok(())
    }

    pub fn get(&self, path: &Path, size: u64, modified: &DateTime<Local>) -> Option<&CacheEntry> {
        self.entries
            .get(path)
            .filter(|entry| entry.size == size && entry.modified == *modified)
    }

    pub fn get_mut(&mut self, path: &Path, size: u64, modified: &DateTime<Local>) -> Option<&mut CacheEntry> {
        self.entries
            .get_mut(path)
            .filter(|entry| entry.size == size && entry.modified == *modified)
    }

    pub fn insert(&mut self, path: PathBuf, entry: CacheEntry) {
        self.entries.insert(path, entry);
    }

    pub async fn remove_stale_entries(&mut self) {
        let mut to_remove = Vec::new();

        for path in self.entries.keys() {
            if !tokio::fs::try_exists(path).await.unwrap_or(false) {
                to_remove.push(path.clone());
            }
        }

        for path in to_remove {
            self.entries.remove(&path);
            tracing::debug!("Removed stale cache entry: {}", path.display());
        }
    }

    fn get_cache_path() -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| color_eyre::eyre::eyre!("Failed to get cache directory"))?
            .join("visualvault");

        Ok(cache_dir.join("file_cache.json"))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl From<&MediaFile> for CacheEntry {
    fn from(file: &MediaFile) -> Self {
        Self {
            path: file.path.clone(),
            name: file.name.clone(),
            extension: file.extension.clone(),
            size: file.size,
            modified: file.modified,
            hash: file.hash.clone(),
            metadata: file.metadata.clone(),
        }
    }
}

impl CacheEntry {
    pub fn to_media_file(&self, file_type: crate::models::FileType, created: DateTime<Local>) -> MediaFile {
        MediaFile {
            path: self.path.clone(),
            name: self.name.clone(),
            extension: self.extension.clone(),
            file_type,
            size: self.size,
            created,
            modified: self.modified,
            hash: self.hash.clone(),
            metadata: self.metadata.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::float_cmp)] // For comparing floats in tests
    #![allow(clippy::panic)]
    use super::*;
    use crate::models::FileType;
    use chrono::TimeZone;
    use tempfile::TempDir;

    // Helper function to create a test cache entry
    fn create_test_cache_entry(path: &str, size: u64, hash: Option<String>) -> CacheEntry {
        CacheEntry {
            path: PathBuf::from(path),
            name: PathBuf::from(path).file_name().unwrap().to_string_lossy().to_string(),
            extension: PathBuf::from(path)
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            size,
            modified: Local.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
            hash,
            metadata: None,
        }
    }

    // Helper function to create a test media file
    fn create_test_media_file(path: &str, size: u64, modified: DateTime<Local>) -> MediaFile {
        MediaFile {
            path: PathBuf::from(path),
            name: PathBuf::from(path).file_name().unwrap().to_string_lossy().to_string(),
            extension: PathBuf::from(path)
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            file_type: FileType::Image,
            size,
            created: modified,
            modified,
            hash: Some("test_hash".to_string()),
            metadata: None,
        }
    }

    #[test]
    fn test_file_cache_new() {
        let cache = FileCache::new();
        assert_eq!(cache.entries.len(), 0);
        assert_eq!(cache.version, FileCache::CURRENT_VERSION);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_insert_and_get() {
        let mut cache = FileCache::new();
        let path = PathBuf::from("/test/image.jpg");
        let size = 1024;
        let modified = Local.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        let entry = create_test_cache_entry("/test/image.jpg", size, Some("hash123".to_string()));
        cache.insert(path.clone(), entry);

        // Test successful get
        let retrieved = cache.get(&path, size, &modified);
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.path, path);
        assert_eq!(retrieved.size, size);
        assert_eq!(retrieved.modified, modified);
        assert_eq!(retrieved.hash, Some("hash123".to_string()));

        // Test get with different size - should return None
        let retrieved = cache.get(&path, 2048, &modified);
        assert!(retrieved.is_none());

        // Test get with different modified time - should return None
        let different_time = Local.with_ymd_and_hms(2024, 1, 16, 10, 30, 0).unwrap();
        let retrieved = cache.get(&path, size, &different_time);
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_cache_get_mut() {
        let mut cache = FileCache::new();
        let path = PathBuf::from("/test/image.jpg");
        let size = 1024;
        let modified = Local.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        let entry = create_test_cache_entry("/test/image.jpg", size, None);
        cache.insert(path.clone(), entry);

        // Get mutable reference and modify
        let entry_mut = cache.get_mut(&path, size, &modified);
        assert!(entry_mut.is_some());

        let entry_mut = entry_mut.unwrap();
        entry_mut.hash = Some("new_hash".to_string());

        // Verify modification
        let retrieved = cache.get(&path, size, &modified).unwrap();
        assert_eq!(retrieved.hash, Some("new_hash".to_string()));
    }

    #[test]
    fn test_from_media_file() {
        let modified = Local.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let media_file = create_test_media_file("/test/image.jpg", 2048, modified);

        let cache_entry = CacheEntry::from(&media_file);

        assert_eq!(cache_entry.path, media_file.path);
        assert_eq!(cache_entry.name, media_file.name);
        assert_eq!(cache_entry.extension, media_file.extension);
        assert_eq!(cache_entry.size, media_file.size);
        assert_eq!(cache_entry.modified, media_file.modified);
        assert_eq!(cache_entry.hash, media_file.hash);
    }

    #[test]
    fn test_to_media_file() {
        let modified = Local.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let created = Local.with_ymd_and_hms(2024, 1, 10, 8, 0, 0).unwrap();

        let cache_entry = CacheEntry {
            path: PathBuf::from("/test/video.mp4"),
            name: "video.mp4".to_string(),
            extension: "mp4".to_string(),
            size: 1024 * 1024 * 10,
            modified,
            hash: Some("video_hash".to_string()),
            metadata: None,
        };

        let media_file = cache_entry.to_media_file(FileType::Video, created);

        assert_eq!(media_file.path, cache_entry.path);
        assert_eq!(media_file.name, cache_entry.name);
        assert_eq!(media_file.extension, cache_entry.extension);
        assert_eq!(media_file.file_type, FileType::Video);
        assert_eq!(media_file.size, cache_entry.size);
        assert_eq!(media_file.created, created);
        assert_eq!(media_file.modified, cache_entry.modified);
        assert_eq!(media_file.hash, cache_entry.hash);
    }

    #[tokio::test]
    async fn test_save_and_load() -> Result<()> {
        // Set up a temporary directory for cache
        let temp_dir = TempDir::new()?;
        unsafe {
            std::env::set_var("HOME", temp_dir.path());
        }

        // Create cache directory structure
        let cache_dir = temp_dir.path().join(".cache").join("visualvault");
        tokio::fs::create_dir_all(&cache_dir).await?;

        // Create a cache with some entries
        let mut cache = FileCache::new();
        cache.insert(
            PathBuf::from("/test/image1.jpg"),
            create_test_cache_entry("/test/image1.jpg", 1024, Some("hash1".to_string())),
        );
        cache.insert(
            PathBuf::from("/test/image2.png"),
            create_test_cache_entry("/test/image2.png", 2048, Some("hash2".to_string())),
        );

        // Save cache
        cache.save().await?;

        // Load cache
        let loaded_cache = FileCache::load().await?;

        assert_eq!(loaded_cache.version, FileCache::CURRENT_VERSION);
        assert_eq!(loaded_cache.entries.len(), 2);
        assert!(loaded_cache.entries.contains_key(&PathBuf::from("/test/image1.jpg")));
        assert!(loaded_cache.entries.contains_key(&PathBuf::from("/test/image2.png")));

        Ok(())
    }

    #[tokio::test]
    async fn test_load_non_existent_cache() -> Result<()> {
        // Set up a temporary directory without cache file
        let temp_dir = TempDir::new()?;
        unsafe {
            std::env::set_var("HOME", temp_dir.path());
        }

        let cache = FileCache::load().await?;

        assert_eq!(cache.version, FileCache::CURRENT_VERSION);
        assert_eq!(cache.entries.len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_load_incompatible_version() -> Result<()> {
        // Set up a temporary directory
        let temp_dir = TempDir::new()?;
        unsafe {
            std::env::set_var("HOME", temp_dir.path());
        }

        let cache_dir = temp_dir.path().join(".cache").join("visualvault");
        tokio::fs::create_dir_all(&cache_dir).await?;

        // Create a cache with different version
        let incompatible_cache = serde_json::json!({
            "entries": {},
            "version": 999
        });

        let cache_path = cache_dir.join("file_cache.json");
        tokio::fs::write(&cache_path, incompatible_cache.to_string()).await?;

        // Load should return a new cache due to version mismatch
        let loaded_cache = FileCache::load().await?;

        assert_eq!(loaded_cache.version, FileCache::CURRENT_VERSION);
        assert_eq!(loaded_cache.entries.len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_stale_entries() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let mut cache = FileCache::new();

        // Create actual files
        let existing_file = temp_dir.path().join("existing.jpg");
        tokio::fs::write(&existing_file, b"data").await?;

        // Add entries for both existing and non-existing files
        cache.insert(
            existing_file.clone(),
            create_test_cache_entry(existing_file.to_str().unwrap(), 1024, Some("hash1".to_string())),
        );
        cache.insert(
            PathBuf::from("/non/existent/file.jpg"),
            create_test_cache_entry("/non/existent/file.jpg", 2048, Some("hash2".to_string())),
        );

        assert_eq!(cache.len(), 2);

        // Remove stale entries
        cache.remove_stale_entries().await;

        // Only the existing file should remain
        assert_eq!(cache.len(), 1);
        assert!(cache.entries.contains_key(&existing_file));
        assert!(!cache.entries.contains_key(&PathBuf::from("/non/existent/file.jpg")));

        Ok(())
    }

    #[test]
    fn test_multiple_entries_with_same_path() {
        let mut cache = FileCache::new();
        let path = PathBuf::from("/test/image.jpg");

        // Insert first entry
        let entry1 = create_test_cache_entry("/test/image.jpg", 1024, Some("hash1".to_string()));
        cache.insert(path.clone(), entry1);
        assert_eq!(cache.len(), 1);

        // Insert second entry with same path (should replace)
        let entry2 = create_test_cache_entry("/test/image.jpg", 2048, Some("hash2".to_string()));
        cache.insert(path.clone(), entry2);
        assert_eq!(cache.len(), 1);

        // Verify the second entry replaced the first
        let modified = Local.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let retrieved = cache.get(&path, 2048, &modified);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().hash, Some("hash2".to_string()));
    }

    #[test]
    fn test_cache_entry_without_extension() {
        let entry = CacheEntry {
            path: PathBuf::from("/test/README"),
            name: "README".to_string(),
            extension: String::new(),
            size: 1024,
            modified: Local::now(),
            hash: None,
            metadata: None,
        };

        let media_file = entry.to_media_file(FileType::Document, Local::now());
        assert_eq!(media_file.extension, "");
        assert_eq!(media_file.name, "README");
    }
}
