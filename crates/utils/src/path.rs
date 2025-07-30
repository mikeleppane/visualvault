use std::path::PathBuf;

use color_eyre::Result;

/// Creates a cache path for the database.
///
/// If `use_in_memory` is true, returns ":memory:" for in-memory database.
/// Otherwise, creates the cache directory if it doesn't exist and returns the path to cache.db.
///
/// # Errors
///
/// Returns an error if:
/// - The cache directory cannot be determined
/// - The cache directory cannot be created
/// - The cache path cannot be converted to a string
pub async fn create_cache_path(app_name: &str, filename: &str) -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| color_eyre::eyre::eyre!("Failed to get cache directory"))?
        .join(app_name)
        .join(filename);
    let cache_path = cache_dir
        .parent()
        .ok_or_else(|| color_eyre::eyre::eyre!("Invalid cache path"))?;

    tokio::fs::create_dir_all(cache_path).await?;
    Ok(cache_dir)
}
