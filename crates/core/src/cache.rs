use crate::database_cache::{CacheEntry, CacheStats};
use async_trait::async_trait;
use chrono::{DateTime, Local};
use color_eyre::Result;
use std::path::{Path, PathBuf};

/// Cache trait for abstracting different cache implementations
#[async_trait]
pub trait Cache: Send + Sync {
    async fn get(&self, path: &Path, size: u64, modified: &DateTime<Local>) -> Result<Option<CacheEntry>>;
    async fn insert(&self, path: PathBuf, entry: CacheEntry) -> Result<()>;
    async fn update_hash(&self, path: &Path, hash: &str) -> Result<()>;
    async fn get_stats(&self) -> Result<CacheStats>;
    async fn remove_stale_entries(&self) -> Result<usize>;
    async fn len(&self) -> Result<usize>;
    async fn is_empty(&self) -> Result<bool>;
}

/// Implement the Cache trait for DatabaseCache
#[async_trait]
impl Cache for crate::DatabaseCache {
    async fn get(&self, path: &Path, size: u64, modified: &DateTime<Local>) -> Result<Option<CacheEntry>> {
        self.get(path, size, modified).await
    }

    async fn insert(&self, path: PathBuf, entry: CacheEntry) -> Result<()> {
        self.insert(path, entry).await
    }

    async fn update_hash(&self, path: &Path, hash: &str) -> Result<()> {
        self.update_hash(path, hash).await
    }

    async fn get_stats(&self) -> Result<CacheStats> {
        self.get_stats().await
    }

    async fn remove_stale_entries(&self) -> Result<usize> {
        self.remove_stale_entries().await
    }
    async fn len(&self) -> Result<usize> {
        self.len().await
    }

    async fn is_empty(&self) -> Result<bool> {
        Ok(self.len().await? == 0)
    }
}
