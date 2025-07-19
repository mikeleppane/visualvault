use crate::models::{MediaFile, MediaMetadata};
use chrono::{DateTime, Local};
use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCache {
    pub entries: HashMap<PathBuf, CacheEntry>,
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
            entries: HashMap::new(),
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

    pub fn get_mut(
        &mut self,
        path: &Path,
        size: u64,
        modified: &DateTime<Local>,
    ) -> Option<&mut CacheEntry> {
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
    pub fn to_media_file(
        &self,
        file_type: crate::models::FileType,
        created: DateTime<Local>,
    ) -> MediaFile {
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
