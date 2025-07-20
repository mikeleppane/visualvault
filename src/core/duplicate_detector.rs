use color_eyre::Result;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, BufReader};
use tracing::{info, warn};

use crate::models::MediaFile;

#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub files: Vec<MediaFile>,
    pub wasted_space: u64, // Size that could be saved by keeping only one copy
}

#[derive(Debug, Clone)]
pub struct DuplicateStats {
    pub total_groups: usize,
    pub total_duplicates: usize,
    pub total_wasted_space: u64,
    pub groups: Vec<DuplicateGroup>,
}

pub struct DuplicateDetector;

impl DuplicateDetector {
    pub fn new() -> Self {
        Self
    }

    /// Calculate SHA256 hash of a file
    async fn calculate_file_hash(path: &Path) -> Result<String> {
        let file = File::open(path).await?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = vec![0; 8192];

        loop {
            let bytes_read = reader.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Quick hash based on file size and first/last bytes
    async fn calculate_quick_hash(path: &Path, size: u64) -> Result<String> {
        if size == 0 {
            return Ok("empty".to_string());
        }

        let mut file = File::open(path).await?;
        let mut hasher = Sha256::new();

        // Hash the size
        hasher.update(size.to_le_bytes());

        // Read first 4KB
        let mut buffer = vec![0; 4096];
        let bytes_read = file.read(&mut buffer).await?;
        hasher.update(&buffer[..bytes_read]);

        // Read last 4KB if file is large enough
        if size > 8192 {
            use tokio::io::AsyncSeekExt;
            file.seek(std::io::SeekFrom::End(-4096)).await?;
            let bytes_read = file.read(&mut buffer).await?;
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Detect duplicates in a collection of media files
    pub async fn detect_duplicates(&self, files: &[MediaFile], use_quick_hash: bool) -> Result<DuplicateStats> {
        info!("Starting duplicate detection for {} files", files.len());

        // Group files by size first (fast pre-filter)
        let mut size_groups: HashMap<u64, Vec<&MediaFile>> = HashMap::new();
        for file in files {
            size_groups.entry(file.size).or_default().push(file);
        }

        // Only process groups with more than one file
        let potential_duplicates: Vec<_> = size_groups.into_iter().filter(|(_, group)| group.len() > 1).collect();

        info!(
            "Found {} size groups with potential duplicates",
            potential_duplicates.len()
        );

        // Calculate hashes for potential duplicates
        let mut hash_groups: HashMap<String, Vec<MediaFile>> = HashMap::new();

        for (size, group) in potential_duplicates {
            for file in group {
                match if use_quick_hash {
                    Self::calculate_quick_hash(&file.path, size).await
                } else {
                    Self::calculate_file_hash(&file.path).await
                } {
                    Ok(hash) => {
                        hash_groups.entry(hash).or_default().push(file.clone());
                    }
                    Err(e) => {
                        warn!("Failed to hash file {:?}: {}", file.path, e);
                    }
                }
            }
        }

        // Build duplicate groups
        let mut groups = Vec::new();
        let mut total_duplicates = 0;
        let mut total_wasted_space = 0;

        for (_, files) in hash_groups {
            if files.len() > 1 {
                let wasted_space = files[0].size * (files.len() - 1) as u64;

                total_duplicates += files.len() - 1;
                total_wasted_space += wasted_space;

                groups.push(DuplicateGroup { files, wasted_space });
            }
        }

        // Sort groups by wasted space (largest first)
        groups.sort_by(|a, b| b.wasted_space.cmp(&a.wasted_space));

        info!(
            "Found {} duplicate groups with {} total duplicates wasting {} bytes",
            groups.len(),
            total_duplicates,
            total_wasted_space
        );

        Ok(DuplicateStats {
            total_groups: groups.len(),
            total_duplicates,
            total_wasted_space,
            groups,
        })
    }

    /// Delete specified files
    pub async fn delete_files(&self, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let mut deleted = Vec::new();

        for path in paths {
            match tokio::fs::remove_file(path).await {
                Ok(()) => {
                    info!("Deleted file: {:?}", path);
                    deleted.push(path.clone());
                }
                Err(e) => {
                    warn!("Failed to delete file {:?}: {}", path, e);
                }
            }
        }

        Ok(deleted)
    }
}
