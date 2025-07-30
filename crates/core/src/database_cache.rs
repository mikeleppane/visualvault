#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
use chrono::{DateTime, Local};
use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool, sqlite::SqlitePoolOptions};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tracing::{debug, info};
use visualvault_models::{FileType, MediaFile, MediaMetadata};

#[derive(Debug, Clone)]
pub struct DatabaseCache {
    pool: SqlitePool,
}

impl DatabaseCache {
    const SCHEMA_VERSION: i32 = 1;
    const MAX_DB_SIZE_MB: u64 = 500; // Maximum database size in MB
    const MAX_ENTRIES: usize = 1_000_000; // Maximum number of entries
    const CLEANUP_THRESHOLD_MB: u64 = 400; // Trigger cleanup when reaching this size
    const TARGET_SIZE_AFTER_CLEANUP_MB: u64 = 300; // Target size after cleanup

    /// Creates an uninitialized database cache instance.
    ///
    /// This is a temporary placeholder that should be replaced with a real cache via `init_cache()`.
    ///
    /// # Panics
    ///
    /// Panics if the dummy `SQLite` memory pool cannot be created.
    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn new_uninit() -> Self {
        // This is a temporary placeholder that will panic if used
        // It should be replaced with a real cache via init_cache()
        Self {
            pool: SqlitePool::connect_lazy("sqlite::memory:").expect("Failed to create dummy pool"),
        }
    }

    /// Create a new database cache
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The cache directory cannot be created
    /// - The database connection cannot be established
    /// - The database schema initialization fails
    pub async fn new(cache_path: &str) -> Result<Self> {
        // Create connection string
        let db_url = format!("sqlite:{cache_path}");
        info!("Initializing database cache at: {}", db_url);
        // Create connection pool with optimizations
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::from_str(&db_url)?
                    .create_if_missing(true)
                    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                    .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
                    .busy_timeout(std::time::Duration::from_secs(10)),
            )
            .await?;

        let cache = Self { pool };
        cache.init_schema().await?;

        info!("Database cache initialized successfully at: {}", cache_path);

        Ok(cache)
    }

    async fn set_size_limits(&self) -> Result<()> {
        // Set maximum database size (in pages, 1 page = 4096 bytes by default)
        let max_pages = (Self::MAX_DB_SIZE_MB * 1024 * 1024) / 4096;
        sqlx::query(&format!("PRAGMA max_page_count = {max_pages}"))
            .execute(&self.pool)
            .await?;

        // Enable auto-vacuum to reclaim space automatically
        sqlx::query("PRAGMA auto_vacuum = INCREMENTAL")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Initialize database schema
    async fn init_schema(&self) -> Result<()> {
        // Enable WAL mode and other optimizations
        sqlx::query("PRAGMA journal_mode = WAL").execute(&self.pool).await?;

        sqlx::query("PRAGMA temp_store = MEMORY").execute(&self.pool).await?;

        sqlx::query("PRAGMA cache_size = -64000") // 64MB cache
            .execute(&self.pool)
            .await?;

        // Create version table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            )",
        )
        .execute(&self.pool)
        .await?;

        // Check current version
        let current_version: Option<i32> = sqlx::query_scalar("SELECT version FROM schema_version LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;

        if current_version != Some(Self::SCHEMA_VERSION) {
            // Create tables
            sqlx::query("DROP TABLE IF EXISTS file_cache")
                .execute(&self.pool)
                .await?;

            sqlx::query(
                "CREATE TABLE file_cache (
                    path TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    extension TEXT NOT NULL,
                    size INTEGER NOT NULL,
                    modified INTEGER NOT NULL,
                    hash TEXT,
                    metadata TEXT,
                    last_accessed INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                    access_count INTEGER NOT NULL DEFAULT 0
                )",
            )
            .execute(&self.pool)
            .await?;

            // Create indexes
            sqlx::query("CREATE INDEX idx_size ON file_cache(size)")
                .execute(&self.pool)
                .await?;

            sqlx::query("CREATE INDEX idx_hash ON file_cache(hash)")
                .execute(&self.pool)
                .await?;

            sqlx::query("CREATE INDEX idx_modified ON file_cache(modified)")
                .execute(&self.pool)
                .await?;

            sqlx::query("CREATE INDEX idx_last_accessed ON file_cache(last_accessed)")
                .execute(&self.pool)
                .await?;

            // Update version
            sqlx::query("DELETE FROM schema_version").execute(&self.pool).await?;

            sqlx::query("INSERT INTO schema_version (version) VALUES (?)")
                .bind(Self::SCHEMA_VERSION)
                .execute(&self.pool)
                .await?;

            info!("Database schema initialized to version {}", Self::SCHEMA_VERSION);
        }

        self.set_size_limits().await?;

        // Add a trigger to limit total entries
        let trigger_query = format!(
            "CREATE TRIGGER IF NOT EXISTS limit_entries
             BEFORE INSERT ON file_cache
             WHEN (SELECT COUNT(*) FROM file_cache) >= {}
             BEGIN
                 DELETE FROM file_cache 
                 WHERE path IN (
                     SELECT path FROM file_cache 
                     ORDER BY last_accessed ASC 
                     LIMIT 1000
                 );
             END",
            Self::MAX_ENTRIES
        );

        sqlx::query(&trigger_query).execute(&self.pool).await?;

        Ok(())
    }

    /// Get entry from cache
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails or if there's a database connection issue.
    pub async fn get(&self, path: &Path, size: u64, modified: &DateTime<Local>) -> Result<Option<CacheEntry>> {
        let path_str = path.to_string_lossy();
        let modified_ts = modified.timestamp();

        let row = sqlx::query(
            "SELECT path, name, extension, size, modified, hash, metadata 
             FROM file_cache 
             WHERE path = ? AND size = ? AND modified = ?",
        )
        .bind(path_str.as_ref())
        .bind(size as i64)
        .bind(modified_ts)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            // Update access statistics
            sqlx::query(
                "UPDATE file_cache 
                 SET last_accessed = strftime('%s', 'now'), 
                     access_count = access_count + 1 
                 WHERE path = ?",
            )
            .bind(path_str.as_ref())
            .execute(&self.pool)
            .await?;

            let path: String = row.get("path");
            let modified_ts: i64 = row.get("modified");
            let metadata_json: Option<String> = row.get("metadata");

            Ok(Some(CacheEntry {
                path: PathBuf::from(path),
                name: row.get("name"),
                extension: row.get("extension"),
                size: row.get::<i64, _>("size") as u64,
                modified: DateTime::from_timestamp(modified_ts, 0)
                    .unwrap_or_else(|| Local::now().into())
                    .into(),
                hash: row.get("hash"),
                metadata: metadata_json.and_then(|json| serde_json::from_str(&json).ok()),
            }))
        } else {
            Ok(None)
        }
    }

    /// Insert or update entry in cache
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The metadata cannot be serialized to JSON
    /// - The database query fails or there's a database connection issue
    pub async fn insert(&self, path: PathBuf, entry: CacheEntry) -> Result<()> {
        let path_str = path.to_string_lossy();
        let modified_ts = entry.modified.timestamp();
        let metadata_json = entry.metadata.as_ref().map(serde_json::to_string).transpose()?;

        sqlx::query(
            "INSERT OR REPLACE INTO file_cache 
             (path, name, extension, size, modified, hash, metadata) 
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(path_str.as_ref())
        .bind(&entry.name)
        .bind(&entry.extension)
        .bind(entry.size as i64)
        .bind(modified_ts)
        .bind(&entry.hash)
        .bind(&metadata_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update only the hash for an entry
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails or there's a database connection issue.
    pub async fn update_hash(&self, path: &Path, hash: &str) -> Result<()> {
        let path_str = path.to_string_lossy();

        sqlx::query("UPDATE file_cache SET hash = ? WHERE path = ?")
            .bind(hash)
            .bind(path_str.as_ref())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Remove stale entries that no longer exist on disk
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The database query to fetch file paths fails
    /// - File system operations to check file existence fail
    /// - The database transaction to delete entries fails
    pub async fn remove_stale_entries(&self) -> Result<usize> {
        let paths: Vec<String> = sqlx::query_scalar("SELECT path FROM file_cache")
            .fetch_all(&self.pool)
            .await?;

        let mut to_remove = Vec::new();
        for path_str in paths {
            let path = PathBuf::from(&path_str);
            if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
                to_remove.push(path_str);
            }
        }

        if to_remove.is_empty() {
            return Ok(0);
        }

        let count = to_remove.len();

        // Use a transaction for batch deletion
        let mut tx = self.pool.begin().await?;
        for path in to_remove {
            sqlx::query("DELETE FROM file_cache WHERE path = ?")
                .bind(&path)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;

        debug!("Removed {} stale cache entries", count);
        Ok(count)
    }

    /// Get cache statistics
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails or there's a database connection issue.
    pub async fn get_stats(&self) -> Result<CacheStats> {
        let row = sqlx::query(
            "SELECT 
                COUNT(*) as total_entries,
                COUNT(hash) as entries_with_hash,
                COALESCE(SUM(size), 0) as total_size,
                COALESCE(CAST(AVG(access_count) AS REAL), 0.0) as avg_access_count
             FROM file_cache",
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(CacheStats {
            total_entries: row.get::<i64, _>("total_entries") as usize,
            entries_with_hash: row.get::<i64, _>("entries_with_hash") as usize,
            total_size: row.get::<i64, _>("total_size") as u64,
            avg_access_count: row.get::<f64, _>("avg_access_count"),
        })
    }

    /// Clean up old entries based on last access time
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails or there's a database connection issue.
    pub async fn cleanup_old_entries(&self, days: i64) -> Result<usize> {
        let cutoff = chrono::Local::now().timestamp() - (days * 24 * 60 * 60);

        let result = sqlx::query("DELETE FROM file_cache WHERE last_accessed < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;

        let count = result.rows_affected() as usize;
        info!("Cleaned up {} old cache entries", count);
        Ok(count)
    }

    /// Get the number of entries in cache
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails or there's a database connection issue.
    pub async fn len(&self) -> Result<usize> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM file_cache")
            .fetch_one(&self.pool)
            .await?;

        Ok(count as usize)
    }

    /// Check if cache is empty
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails or there's a database connection issue.
    pub async fn is_empty(&self) -> Result<bool> {
        Ok(self.len().await? == 0)
    }

    /// Get multiple entries by their hashes (useful for duplicate detection)
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails or there's a database connection issue.
    pub async fn get_by_hashes(&self, hashes: &[String]) -> Result<Vec<CacheEntry>> {
        if hashes.is_empty() {
            return Ok(Vec::new());
        }

        // Build query with placeholders
        let placeholders = hashes.iter().map(|_| "?").collect::<Vec<_>>().join(",");

        let query = format!(
            "SELECT path, name, extension, size, modified, hash, metadata 
             FROM file_cache 
             WHERE hash IN ({placeholders})"
        );

        let mut query_builder = sqlx::query(&query);
        for hash in hashes {
            query_builder = query_builder.bind(hash);
        }

        let rows = query_builder.fetch_all(&self.pool).await?;

        let entries = rows
            .into_iter()
            .map(|row| {
                let modified_ts: i64 = row.get("modified");
                let metadata_json: Option<String> = row.get("metadata");

                CacheEntry {
                    path: PathBuf::from(row.get::<String, _>("path")),
                    name: row.get("name"),
                    extension: row.get("extension"),
                    size: row.get::<i64, _>("size") as u64,
                    modified: DateTime::from_timestamp(modified_ts, 0)
                        .unwrap_or_else(|| Local::now().into())
                        .into(),
                    hash: row.get("hash"),
                    metadata: metadata_json.and_then(|json| serde_json::from_str(&json).ok()),
                }
            })
            .collect();

        Ok(entries)
    }

    fn get_cache_path() -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| color_eyre::eyre::eyre!("Failed to get cache directory"))?
            .join("visualvault");

        Ok(cache_dir.join("cache.db"))
    }

    /// Check database size and perform cleanup if needed
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The database file size cannot be determined
    /// - The cleanup operation fails
    pub async fn check_and_cleanup(&self) -> Result<()> {
        let db_size = self.get_db_file_size().await?;
        let db_size_mb = db_size / (1024 * 1024);

        if db_size_mb > Self::CLEANUP_THRESHOLD_MB {
            info!(
                "Database size ({} MB) exceeds threshold, performing cleanup",
                db_size_mb
            );
            self.perform_automatic_cleanup().await?;
        }

        Ok(())
    }

    /// Get the size of the database file in bytes
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The cache path cannot be determined
    /// - The database file metadata cannot be read
    pub async fn get_db_file_size(&self) -> Result<u64> {
        let path = Self::get_cache_path()?;
        let metadata = tokio::fs::metadata(&path).await?;
        Ok(metadata.len())
    }

    async fn perform_automatic_cleanup(&self) -> Result<()> {
        let start = std::time::Instant::now();

        // 1. Remove entries that haven't been accessed in 90 days
        let removed_old = self.cleanup_old_entries(90).await?;

        // 2. Remove entries for files that no longer exist
        let removed_stale = self.remove_stale_entries().await?;

        // 3. Remove entries without hashes that are older than 30 days
        let removed_no_hash = self.remove_old_entries_without_hash(30).await?;

        // 4. If still too large, remove least recently used entries
        let current_size = self.get_db_file_size().await? / (1024 * 1024);
        if current_size > Self::TARGET_SIZE_AFTER_CLEANUP_MB {
            let entries_to_remove = self.calculate_entries_to_remove().await?;
            let removed_lru = self.remove_least_recently_used(entries_to_remove).await?;
            info!("Removed {} least recently used entries", removed_lru);
        }

        // 5. Vacuum the database to reclaim space
        self.vacuum().await?;

        let final_size = self.get_db_file_size().await? / (1024 * 1024);
        info!(
            "Cleanup completed in {:?}: removed {} old, {} stale, {} without hash entries. Size: {} MB -> {} MB",
            start.elapsed(),
            removed_old,
            removed_stale,
            removed_no_hash,
            current_size,
            final_size
        );

        Ok(())
    }

    async fn remove_old_entries_without_hash(&self, days: i64) -> Result<usize> {
        let cutoff = chrono::Local::now().timestamp() - (days * 24 * 60 * 60);

        let result = sqlx::query(
            "DELETE FROM file_cache 
             WHERE hash IS NULL 
             AND last_accessed < ?",
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as usize)
    }

    async fn calculate_entries_to_remove(&self) -> Result<usize> {
        let stats = self.get_stats().await?;
        let current_size_mb = self.get_db_file_size().await? / (1024 * 1024);

        if current_size_mb <= Self::TARGET_SIZE_AFTER_CLEANUP_MB {
            return Ok(0);
        }

        // Estimate average entry size
        let avg_entry_size = if stats.total_entries > 0 {
            (current_size_mb * 1024 * 1024) / stats.total_entries as u64
        } else {
            1024 // Default 1KB per entry
        };

        let bytes_to_remove = (current_size_mb - Self::TARGET_SIZE_AFTER_CLEANUP_MB) * 1024 * 1024;
        let entries_to_remove = (bytes_to_remove / avg_entry_size) as usize;

        Ok(entries_to_remove.min(stats.total_entries / 4)) // Never remove more than 25% at once
    }

    async fn remove_least_recently_used(&self, count: usize) -> Result<usize> {
        if count == 0 {
            return Ok(0);
        }

        // Get paths of least recently used entries
        let paths: Vec<String> = sqlx::query_scalar(
            "SELECT path FROM file_cache 
             ORDER BY last_accessed ASC, access_count ASC 
             LIMIT ?",
        )
        .bind(count as i64)
        .fetch_all(&self.pool)
        .await?;

        if paths.is_empty() {
            return Ok(0);
        }

        // Delete in batches
        let mut tx = self.pool.begin().await?;
        for chunk in paths.chunks(1000) {
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!("DELETE FROM file_cache WHERE path IN ({placeholders})");

            let mut query_builder = sqlx::query(&query);
            for path in chunk {
                query_builder = query_builder.bind(path);
            }

            query_builder.execute(&mut *tx).await?;
        }
        tx.commit().await?;

        Ok(paths.len())
    }

    async fn vacuum(&self) -> Result<()> {
        sqlx::query("VACUUM").execute(&self.pool).await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub total_entries: usize,
    pub entries_with_hash: usize,
    pub total_size: u64,
    pub avg_access_count: f64,
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

impl CacheEntry {
    /// Convert cache entry to `MediaFile`
    pub fn to_media_file(&self, file_type: FileType, created: DateTime<Local>) -> MediaFile {
        MediaFile {
            path: self.path.clone(),
            name: self.name.clone().into(),
            extension: self.extension.clone().into(),
            file_type,
            size: self.size,
            created,
            modified: self.modified,
            hash: self.hash.as_ref().map(|h| std::sync::Arc::<str>::from(h.as_str())),
            metadata: self.metadata.clone(),
        }
    }
}

impl From<&MediaFile> for CacheEntry {
    fn from(file: &MediaFile) -> Self {
        Self {
            path: file.path.clone(),
            name: file.name.to_string(),
            extension: file.extension.to_string(),
            size: file.size,
            modified: file.modified,
            hash: file.hash.as_ref().map(std::string::ToString::to_string),
            metadata: file.metadata.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::float_cmp)] // For comparing floats in tests
    #![allow(clippy::panic)]
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::panic_in_result_fn)]
    use std::sync::Arc;

    use visualvault_utils::create_cache_path;

    use super::*;

    /// Helper to create a test database cache
    async fn create_test_cache() -> Result<DatabaseCache> {
        DatabaseCache::new(":memory:").await // Use in-memory database for tests
    }

    /// Helper to create a test cache entry
    fn create_test_entry(name: &str, size: u64, hash: Option<String>) -> CacheEntry {
        let path = PathBuf::from(format!("/test/{name}"));
        CacheEntry {
            path,
            name: name.to_string(),
            extension: "jpg".to_string(),
            size,
            modified: Local::now(),
            hash,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_new_uninit() {
        let cache = DatabaseCache::new_uninit();
        // But trying to use it should fail
        let result = cache.len().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_new_in_memory() -> Result<()> {
        let cache = DatabaseCache::new(":memory:").await?;
        assert_eq!(cache.len().await?, 0);
        assert!(cache.is_empty().await?);
        Ok(())
    }

    #[tokio::test]
    async fn test_insert_and_get() -> Result<()> {
        let cache = create_test_cache().await?;
        let entry = create_test_entry("test.jpg", 1024, Some("hash123".to_string()));

        // Insert entry
        cache.insert(entry.path.clone(), entry.clone()).await?;

        // Get entry
        let retrieved = cache.get(&entry.path, entry.size, &entry.modified).await?;
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, entry.name);
        assert_eq!(retrieved.extension, entry.extension);
        assert_eq!(retrieved.size, entry.size);
        assert_eq!(retrieved.hash, entry.hash);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_nonexistent() -> Result<()> {
        let cache = create_test_cache().await?;
        let path = PathBuf::from("/nonexistent/file.jpg");
        let result = cache.get(&path, 1024, &Local::now()).await?;
        assert!(result.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn test_update_hash() -> Result<()> {
        let cache = create_test_cache().await?;
        let entry = create_test_entry("test.jpg", 1024, None);

        // Insert without hash
        cache.insert(entry.path.clone(), entry.clone()).await?;

        // Update hash
        cache.update_hash(&entry.path, "new_hash").await?;

        // Verify hash was updated
        let retrieved = cache.get(&entry.path, entry.size, &entry.modified).await?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().hash, Some("new_hash".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_cache_stats() -> Result<()> {
        let cache = create_test_cache().await?;

        // Insert entries with and without hashes
        for i in 0..5 {
            let hash = if i < 3 { Some(format!("hash{i}")) } else { None };
            let entry = create_test_entry(&format!("file{i}.jpg"), 1024 * (i + 1), hash);
            cache.insert(entry.path.clone(), entry).await?;
        }

        let stats = cache.get_stats().await?;
        assert_eq!(stats.total_entries, 5);
        assert_eq!(stats.entries_with_hash, 3);
        assert_eq!(stats.total_size, 1024 + 2048 + 3072 + 4096 + 5120); // Sum of sizes

        Ok(())
    }

    #[tokio::test]
    async fn test_get_by_hashes() -> Result<()> {
        let cache = create_test_cache().await?;

        // Insert entries
        for i in 0..5 {
            let entry = create_test_entry(&format!("file{i}.jpg"), 1024, Some(format!("hash{i}")));
            cache.insert(entry.path.clone(), entry).await?;
        }

        // Get multiple by hashes
        let hashes = vec!["hash1".to_string(), "hash3".to_string()];
        let entries = cache.get_by_hashes(&hashes).await?;

        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.name == "file1.jpg"));
        assert!(entries.iter().any(|e| e.name == "file3.jpg"));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_by_empty_hashes() -> Result<()> {
        let cache = create_test_cache().await?;
        let entries = cache.get_by_hashes(&[]).await?;
        assert!(entries.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_old_entries() -> Result<()> {
        let cache = create_test_cache().await?;

        // Insert an entry and manually set its last_accessed to 100 days ago
        let entry = create_test_entry("old.jpg", 1024, Some("hash".to_string()));
        cache.insert(entry.path.clone(), entry).await?;

        // Update last_accessed to 100 days ago
        let old_timestamp = Local::now().timestamp() - (100 * 24 * 60 * 60);
        sqlx::query("UPDATE file_cache SET last_accessed = ? WHERE path = ?")
            .bind(old_timestamp)
            .bind("/test/old.jpg")
            .execute(&cache.pool)
            .await?;

        // Clean up entries older than 90 days
        let removed = cache.cleanup_old_entries(90).await?;
        assert_eq!(removed, 1);

        // Verify entry is gone
        assert_eq!(cache.len().await?, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_old_entries_without_hash() -> Result<()> {
        let cache = create_test_cache().await?;

        // Insert entries with and without hashes
        let entry1 = create_test_entry("with_hash.jpg", 1024, Some("hash".to_string()));
        let entry2 = create_test_entry("without_hash.jpg", 1024, None);

        cache.insert(entry1.path.clone(), entry1).await?;
        cache.insert(entry2.path.clone(), entry2).await?;

        // Set last_accessed to 40 days ago for both
        let old_timestamp = Local::now().timestamp() - (40 * 24 * 60 * 60);
        sqlx::query("UPDATE file_cache SET last_accessed = ?")
            .bind(old_timestamp)
            .execute(&cache.pool)
            .await?;

        // Remove entries without hash older than 30 days
        let removed = cache.remove_old_entries_without_hash(30).await?;
        assert_eq!(removed, 1); // Only the one without hash should be removed

        let stats = cache.get_stats().await?;
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.entries_with_hash, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_least_recently_used() -> Result<()> {
        let cache = create_test_cache().await?;

        // Insert 10 entries
        for i in 0..10 {
            let entry = create_test_entry(&format!("file{i}.jpg"), 1024, Some(format!("hash{i}")));
            cache.insert(entry.path.clone(), entry).await?;

            // Set different access times
            let timestamp = Local::now().timestamp() - (i64::from(i) * 60);
            sqlx::query("UPDATE file_cache SET last_accessed = ?, access_count = ? WHERE path = ?")
                .bind(timestamp)
                .bind(i)
                .bind(format!("/test/file{i}.jpg"))
                .execute(&cache.pool)
                .await?;
        }

        // Remove 3 least recently used
        let removed = cache.remove_least_recently_used(3).await?;
        assert_eq!(removed, 3);

        // Verify correct entries were removed (file7, file8, file9 - oldest)
        let remaining = cache.len().await?;
        assert_eq!(remaining, 7);

        Ok(())
    }

    #[tokio::test]
    async fn test_cache_entry_conversions() -> Result<()> {
        // Test CacheEntry to MediaFile conversion
        let cache_entry = create_test_entry("test.jpg", 1024, Some("hash".to_string()));
        let created = Local::now();
        let media_file = cache_entry.to_media_file(FileType::Image, created);

        assert_eq!(media_file.path, cache_entry.path);
        assert_eq!(media_file.name.as_ref(), cache_entry.name);
        assert_eq!(media_file.extension.as_ref(), cache_entry.extension);
        assert_eq!(media_file.size, cache_entry.size);
        assert_eq!(
            media_file.hash.as_ref().map(std::convert::AsRef::as_ref),
            cache_entry.hash.as_deref()
        );

        // Test MediaFile to CacheEntry conversion
        let cache_entry_from = CacheEntry::from(&media_file);
        assert_eq!(cache_entry_from.path, media_file.path);
        assert_eq!(cache_entry_from.name, media_file.name.as_ref());
        assert_eq!(cache_entry_from.extension, media_file.extension.as_ref());
        assert_eq!(cache_entry_from.size, media_file.size);

        Ok(())
    }

    #[tokio::test]
    async fn test_trigger_entry_limit() -> Result<()> {
        let cache = create_test_cache().await?;

        // We need to test the trigger, but MAX_ENTRIES is 1_000_000 which is too many for a test
        // So we'll create a custom trigger with a lower limit for testing
        sqlx::query("DROP TRIGGER IF EXISTS limit_entries")
            .execute(&cache.pool)
            .await?;

        let trigger_query = "CREATE TRIGGER limit_entries
             BEFORE INSERT ON file_cache
             WHEN (SELECT COUNT(*) FROM file_cache) >= 5
             BEGIN
                 DELETE FROM file_cache 
                 WHERE path IN (
                     SELECT path FROM file_cache 
                     ORDER BY last_accessed ASC 
                     LIMIT 1
                 );
             END";

        sqlx::query(trigger_query).execute(&cache.pool).await?;

        // Insert 6 entries
        for i in 0..6 {
            let entry = create_test_entry(&format!("file{i}.jpg"), 1024, None);
            cache.insert(entry.path.clone(), entry).await?;

            // Set different access times
            #[allow(clippy::cast_lossless)]
            let timestamp = Local::now().timestamp() - (10 - i as i64);
            sqlx::query("UPDATE file_cache SET last_accessed = ? WHERE path = ?")
                .bind(timestamp)
                .bind(format!("/test/file{i}.jpg"))
                .execute(&cache.pool)
                .await?;
        }

        // Should have exactly 5 entries due to trigger
        assert_eq!(cache.len().await?, 5);

        // file0.jpg should have been removed (oldest)
        let result = cache
            .get(&PathBuf::from("/test/file0.jpg"), 1024, &Local::now())
            .await?;
        assert!(result.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_check_and_cleanup() -> Result<()> {
        let cache_path = create_cache_path("visualvault", "cache.db").await?;
        let cache = DatabaseCache::new(cache_path.to_str().unwrap()).await?;

        // Since we're using in-memory DB, we can't really test file size limits
        // But we can test that the function doesn't error
        cache.check_and_cleanup().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_access() -> Result<()> {
        let cache = Arc::new(create_test_cache().await?);
        let mut handles = vec![];

        // Spawn multiple tasks that insert and read concurrently
        for i in 0..10 {
            let cache_clone = Arc::clone(&cache);
            let handle = tokio::spawn(async move {
                let entry = create_test_entry(&format!("file{i}.jpg"), 1024 * i, Some(format!("hash{i}")));
                cache_clone.insert(entry.path.clone(), entry.clone()).await?;

                // Try to read it back
                let retrieved = cache_clone.get(&entry.path, entry.size, &entry.modified).await?;
                assert!(retrieved.is_some());

                Result::<(), color_eyre::eyre::Error>::Ok(())
            });
            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            handle.await??;
        }

        // Verify all entries are there
        assert_eq!(cache.len().await?, 10);

        Ok(())
    }

    #[tokio::test]
    async fn test_vacuum() -> Result<()> {
        let cache = create_test_cache().await?;

        // Insert and delete entries to create fragmentation
        for i in 0..10 {
            let entry = create_test_entry(&format!("file{i}.jpg"), 1024, None);
            cache.insert(entry.path.clone(), entry).await?;
        }

        // Delete half of them
        for i in 0..5 {
            sqlx::query("DELETE FROM file_cache WHERE path = ?")
                .bind(format!("/test/file{i}.jpg"))
                .execute(&cache.pool)
                .await?;
        }

        // Vacuum should work without error
        cache.vacuum().await?;

        // Verify remaining entries
        assert_eq!(cache.len().await?, 5);

        Ok(())
    }

    #[tokio::test]
    async fn test_schema_version_upgrade() -> Result<()> {
        let cache = create_test_cache().await?;

        // Manually change schema version
        sqlx::query("UPDATE schema_version SET version = 0")
            .execute(&cache.pool)
            .await?;

        // Re-initialize schema (simulating upgrade)
        cache.init_schema().await?;

        // Verify version is updated
        let version: i32 = sqlx::query_scalar("SELECT version FROM schema_version")
            .fetch_one(&cache.pool)
            .await?;

        assert_eq!(version, DatabaseCache::SCHEMA_VERSION);

        Ok(())
    }

    #[tokio::test]
    async fn test_access_statistics_update() -> Result<()> {
        let cache = create_test_cache().await?;
        let entry = create_test_entry("test.jpg", 1024, Some("hash".to_string()));

        // Insert entry
        cache.insert(entry.path.clone(), entry.clone()).await?;

        // Get initial access count
        let initial_count: i64 = sqlx::query_scalar("SELECT access_count FROM file_cache WHERE path = ?")
            .bind("/test/test.jpg")
            .fetch_one(&cache.pool)
            .await?;

        assert_eq!(initial_count, 0);

        // Access the entry multiple times
        for _ in 0..5 {
            let _ = cache.get(&entry.path, entry.size, &entry.modified).await?;
        }

        // Verify access count increased
        let updated_count: i64 = sqlx::query_scalar("SELECT access_count FROM file_cache WHERE path = ?")
            .bind("/test/test.jpg")
            .fetch_one(&cache.pool)
            .await?;

        assert_eq!(updated_count, 5);

        Ok(())
    }

    #[tokio::test]
    async fn test_insert_or_replace() -> Result<()> {
        let cache = create_test_cache().await?;

        let mut entry = create_test_entry("test.jpg", 1024, Some("hash1".to_string()));
        cache.insert(entry.path.clone(), entry.clone()).await?;

        // Update the entry with new data
        entry.size = 2048;
        entry.hash = Some("hash2".to_string());
        cache.insert(entry.path.clone(), entry.clone()).await?;

        // Verify updated data
        let retrieved = cache.get(&entry.path, 2048, &entry.modified).await?;
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.size, 2048);
        assert_eq!(retrieved.hash, Some("hash2".to_string()));

        // Verify only one entry exists
        assert_eq!(cache.len().await?, 1);

        Ok(())
    }
}
