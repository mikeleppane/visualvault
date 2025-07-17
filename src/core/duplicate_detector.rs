use color_eyre::eyre::Result;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, BufReader};
use tracing::{debug, error};

use crate::models::MediaFile;

pub struct DuplicateDetector {
    cache: HashMap<String, String>,
}

impl DuplicateDetector {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub async fn find_duplicates(
        &mut self,
        files: &mut Vec<MediaFile>,
    ) -> HashMap<String, Vec<MediaFile>> {
        // First, compute hashes for all files
        for file in files.iter_mut() {
            if file.hash.is_none() {
                match self.compute_file_hash(&file.path).await {
                    Ok(hash) => {
                        file.hash = Some(hash.clone());
                        self.cache
                            .insert(file.path.to_string_lossy().to_string(), hash);
                    }
                    Err(e) => {
                        error!("Failed to compute hash for {:?}: {}", file.path, e);
                    }
                }
            }
        }

        // Group files by hash
        let mut hash_groups: HashMap<String, Vec<MediaFile>> = HashMap::new();

        for file in files {
            if let Some(hash) = &file.hash {
                hash_groups
                    .entry(hash.clone())
                    .or_default()
                    .push(file.clone());
            }
        }

        // Filter out non-duplicates
        hash_groups.retain(|_, files| files.len() > 1);

        debug!("Found {} groups of duplicates", hash_groups.len());
        hash_groups
    }

    async fn compute_file_hash(&self, path: &Path) -> Result<String> {
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

    pub fn get_cached_hash(&self, path: &str) -> Option<&String> {
        self.cache.get(path)
    }
}
