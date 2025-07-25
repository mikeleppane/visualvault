use ahash::AHashMap;
use color_eyre::Result;
use sha2::{Digest, Sha256};
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

impl DuplicateGroup {
    #[allow(dead_code)]
    #[must_use]
    pub fn new(files: Vec<MediaFile>, wasted_space: u64) -> Self {
        Self { files, wasted_space }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DuplicateStats {
    pub total_groups: usize,
    pub total_duplicates: usize,
    pub total_wasted_space: u64,
    pub groups: Vec<DuplicateGroup>,
}

impl DuplicateStats {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn get_by_hash(&self, hash: &str) -> Option<&DuplicateGroup> {
        self.groups.iter().find(|g| g.files[0].hash.as_deref() == Some(hash))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.groups.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    #[must_use]
    pub fn total_size(&self) -> u64 {
        self.groups.iter().map(|g| g.wasted_space).sum()
    }

    #[must_use]
    pub fn total_files(&self) -> usize {
        self.groups.iter().map(|g| g.files.len()).sum()
    }
}

pub struct DuplicateDetector;

impl Default for DuplicateDetector {
    fn default() -> Self {
        Self
    }
}

impl DuplicateDetector {
    #[must_use]
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
    ///
    /// # Errors
    ///
    /// This function will return an error if file I/O operations fail while calculating hashes.
    pub async fn detect_duplicates(&self, files: &[MediaFile], use_quick_hash: bool) -> Result<DuplicateStats> {
        info!("Starting duplicate detection for {} files", files.len());

        // Group files by size first (fast pre-filter)
        let mut size_groups: AHashMap<u64, Vec<&MediaFile>> = AHashMap::new();
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
        let mut hash_groups: AHashMap<String, Vec<MediaFile>> = AHashMap::new();

        for (size, group) in potential_duplicates {
            for file in group {
                match if use_quick_hash {
                    Self::calculate_quick_hash(&file.path, size).await
                } else {
                    Self::calculate_file_hash(&file.path).await
                } {
                    Ok(hash) => {
                        let mut file = file.clone();
                        file.hash = Some(hash.clone());
                        hash_groups.entry(hash).or_default().push(file);
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
    ///
    /// # Errors
    ///
    /// This function will return an error if any file system operation fails,
    /// though it continues attempting to delete remaining files even after failures.
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::float_cmp)] // For comparing floats in tests
    #![allow(clippy::panic)]
    use super::*;
    use crate::models::FileType;
    use chrono::Local;
    use tempfile::TempDir;
    use tokio::fs;

    // Helper function to create a test media file
    fn create_test_media_file(path: PathBuf, size: u64, _content_id: u8) -> MediaFile {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let extension = path.extension().unwrap_or_default().to_string_lossy().to_string();

        MediaFile {
            path,
            name,
            extension,
            file_type: FileType::Image,
            size,
            created: Local::now(),
            modified: Local::now(),
            hash: None,
            metadata: None,
        }
    }

    // Helper function to create a file with specific content
    async fn create_file_with_content(path: &Path, content: Vec<u8>) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, content).await?;
        Ok(())
    }

    // Helper function to create a file with repeated byte pattern
    async fn create_file_with_size(path: &Path, size: usize, byte: u8) -> Result<()> {
        let content = vec![byte; size];
        create_file_with_content(path, content).await
    }

    #[test]
    fn test_duplicate_detector_new() {
        let detector = DuplicateDetector::new();
        // Just ensure it creates without panic
        let _ = detector;
    }

    #[test]
    fn test_duplicate_detector_default() {
        let detector = DuplicateDetector;
        // Just ensure it creates without panic
        let _ = detector;
    }

    #[tokio::test]
    async fn test_calculate_file_hash_empty_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let empty_file = temp_dir.path().join("empty.txt");
        create_file_with_content(&empty_file, vec![]).await?;

        let hash = DuplicateDetector::calculate_file_hash(&empty_file).await?;

        // SHA256 hash of empty file
        assert_eq!(hash, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");

        Ok(())
    }

    #[tokio::test]
    async fn test_calculate_file_hash_small_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.txt");
        create_file_with_content(&file_path, b"Hello, World!".to_vec()).await?;

        let hash = DuplicateDetector::calculate_file_hash(&file_path).await?;

        // SHA256 hash of "Hello, World!"
        assert_eq!(hash, "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f");

        Ok(())
    }

    #[tokio::test]
    async fn test_calculate_file_hash_large_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("large.bin");

        // Create a 1MB file with repeated pattern
        create_file_with_size(&file_path, 1024 * 1024, 0xAB).await?;

        let hash1 = DuplicateDetector::calculate_file_hash(&file_path).await?;
        let hash2 = DuplicateDetector::calculate_file_hash(&file_path).await?;

        // Hash should be consistent
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA256 produces 64 hex characters

        Ok(())
    }

    #[tokio::test]
    async fn test_calculate_quick_hash_empty_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let empty_file = temp_dir.path().join("empty.txt");
        create_file_with_content(&empty_file, vec![]).await?;

        let hash = DuplicateDetector::calculate_quick_hash(&empty_file, 0).await?;
        assert_eq!(hash, "empty");

        Ok(())
    }

    #[tokio::test]
    async fn test_calculate_quick_hash_small_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("small.txt");
        let content = b"Small file content".to_vec();
        create_file_with_content(&file_path, content.clone()).await?;

        let hash = DuplicateDetector::calculate_quick_hash(&file_path, content.len() as u64).await?;

        // Should produce a hash
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64);

        Ok(())
    }

    #[tokio::test]
    async fn test_calculate_quick_hash_large_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("large.bin");

        // Create a 10KB file (larger than 8KB threshold)
        let size = 10 * 1024;
        create_file_with_size(&file_path, size, 0xFF).await?;

        let hash = DuplicateDetector::calculate_quick_hash(&file_path, size as u64).await?;

        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64);

        // Quick hash should be consistent
        let hash2 = DuplicateDetector::calculate_quick_hash(&file_path, size as u64).await?;
        assert_eq!(hash, hash2);

        Ok(())
    }

    #[tokio::test]
    async fn test_calculate_quick_hash_different_files_same_size() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file1 = temp_dir.path().join("file1.bin");
        let file2 = temp_dir.path().join("file2.bin");

        let size = 10 * 1024;
        create_file_with_size(&file1, size, 0xAA).await?;
        create_file_with_size(&file2, size, 0xBB).await?;

        let hash1 = DuplicateDetector::calculate_quick_hash(&file1, size as u64).await?;
        let hash2 = DuplicateDetector::calculate_quick_hash(&file2, size as u64).await?;

        // Different content should produce different hashes
        assert_ne!(hash1, hash2);

        Ok(())
    }

    #[tokio::test]
    async fn test_detect_duplicates_empty_list() -> Result<()> {
        let detector = DuplicateDetector::new();
        let stats = detector.detect_duplicates(&[], false).await?;

        assert_eq!(stats.total_groups, 0);
        assert_eq!(stats.total_duplicates, 0);
        assert_eq!(stats.total_wasted_space, 0);
        assert!(stats.groups.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_detect_duplicates_no_duplicates() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create unique files
        let file1 = temp_dir.path().join("file1.jpg");
        let file2 = temp_dir.path().join("file2.jpg");
        let file3 = temp_dir.path().join("file3.jpg");

        create_file_with_size(&file1, 1024, 0x01).await?;
        create_file_with_size(&file2, 2048, 0x02).await?;
        create_file_with_size(&file3, 3072, 0x03).await?;

        let files = vec![
            create_test_media_file(file1, 1024, 1),
            create_test_media_file(file2, 2048, 2),
            create_test_media_file(file3, 3072, 3),
        ];

        let detector = DuplicateDetector::new();
        let stats = detector.detect_duplicates(&files, false).await?;

        assert_eq!(stats.total_groups, 0);
        assert_eq!(stats.total_duplicates, 0);
        assert_eq!(stats.total_wasted_space, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_detect_duplicates_with_duplicates() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create duplicate files
        let file1 = temp_dir.path().join("dup1.jpg");
        let file2 = temp_dir.path().join("dup2.jpg");
        let file3 = temp_dir.path().join("unique.jpg");

        create_file_with_content(&file1, b"duplicate content".to_vec()).await?;
        create_file_with_content(&file2, b"duplicate content".to_vec()).await?;
        create_file_with_content(&file3, b"unique content".to_vec()).await?;

        let files = vec![
            create_test_media_file(file1, 17, 1),
            create_test_media_file(file2, 17, 1),
            create_test_media_file(file3, 14, 2),
        ];

        let detector = DuplicateDetector::new();
        let stats = detector.detect_duplicates(&files, false).await?;

        assert_eq!(stats.total_groups, 1);
        assert_eq!(stats.total_duplicates, 1);
        assert_eq!(stats.total_wasted_space, 17);
        assert_eq!(stats.groups[0].files.len(), 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_detect_duplicates_multiple_groups() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create multiple duplicate groups
        let files_data = vec![
            ("group1_1.jpg", b"content A", 9),
            ("group1_2.jpg", b"content A", 9),
            ("group1_3.jpg", b"content A", 9),
            ("group2_1.jpg", b"content B", 9),
            ("group2_2.jpg", b"content B", 9),
            ("unique.jpg", b"content C", 9),
        ];

        let mut files = Vec::new();
        for (name, content, size) in files_data {
            let path = temp_dir.path().join(name);
            create_file_with_content(&path, content.to_vec()).await?;
            files.push(create_test_media_file(path, size, 1));
        }

        let detector = DuplicateDetector::new();
        let stats = detector.detect_duplicates(&files, false).await?;

        assert_eq!(stats.total_groups, 2);
        assert_eq!(stats.total_duplicates, 3); // 2 duplicates in group1, 1 in group2
        assert_eq!(stats.total_wasted_space, 27); // 18 + 9

        // Groups should be sorted by wasted space
        assert_eq!(stats.groups[0].wasted_space, 18); // Group with 3 files
        assert_eq!(stats.groups[1].wasted_space, 9); // Group with 2 files

        Ok(())
    }

    #[tokio::test]
    async fn test_detect_duplicates_quick_hash() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create large duplicate files
        let file1 = temp_dir.path().join("large1.bin");
        let file2 = temp_dir.path().join("large2.bin");

        let size = 100 * 1024; // 100KB
        create_file_with_size(&file1, size, 0xFF).await?;
        create_file_with_size(&file2, size, 0xFF).await?;

        let files = vec![
            create_test_media_file(file1, size as u64, 1),
            create_test_media_file(file2, size as u64, 1),
        ];

        let detector = DuplicateDetector::new();
        let stats = detector.detect_duplicates(&files, true).await?;

        assert_eq!(stats.total_groups, 1);
        assert_eq!(stats.total_duplicates, 1);
        assert_eq!(stats.total_wasted_space, size as u64);

        Ok(())
    }

    #[tokio::test]
    async fn test_detect_duplicates_different_sizes() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Files with same content but different sizes won't be duplicates
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");

        create_file_with_content(&file1, b"content".to_vec()).await?;
        create_file_with_content(&file2, b"content ".to_vec()).await?; // Extra space

        let files = vec![create_test_media_file(file1, 7, 1), create_test_media_file(file2, 8, 1)];

        let detector = DuplicateDetector::new();
        let stats = detector.detect_duplicates(&files, false).await?;

        assert_eq!(stats.total_groups, 0);
        assert_eq!(stats.total_duplicates, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_detect_duplicates_handles_missing_file() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create one real file and one non-existent
        let file1 = temp_dir.path().join("exists.jpg");
        create_file_with_content(&file1, b"content".to_vec()).await?;

        let files = vec![
            create_test_media_file(file1, 7, 1),
            create_test_media_file(PathBuf::from("/non/existent/file.jpg"), 7, 1),
        ];

        let detector = DuplicateDetector::new();
        let stats = detector.detect_duplicates(&files, false).await?;

        // Should handle the error gracefully
        assert_eq!(stats.total_groups, 0);
        assert_eq!(stats.total_duplicates, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_files_success() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create test files
        let file1 = temp_dir.path().join("delete1.txt");
        let file2 = temp_dir.path().join("delete2.txt");

        create_file_with_content(&file1, b"content1".to_vec()).await?;
        create_file_with_content(&file2, b"content2".to_vec()).await?;

        assert!(file1.exists());
        assert!(file2.exists());

        let detector = DuplicateDetector::new();
        let deleted = detector.delete_files(&[file1.clone(), file2.clone()]).await?;

        assert_eq!(deleted.len(), 2);
        assert!(!file1.exists());
        assert!(!file2.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_files_partial_success() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create one file and reference one non-existent
        let file1 = temp_dir.path().join("exists.txt");
        let file2 = PathBuf::from("/non/existent/file.txt");

        create_file_with_content(&file1, b"content".to_vec()).await?;

        let detector = DuplicateDetector::new();
        let deleted = detector.delete_files(&[file1.clone(), file2]).await?;

        // Should delete the existing file and continue
        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0], file1);
        assert!(!file1.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_files_empty_list() -> Result<()> {
        let detector = DuplicateDetector::new();
        let deleted = detector.delete_files(&[]).await?;

        assert_eq!(deleted.len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_duplicate_stats_sorting() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create files with different sizes to test sorting
        let small1 = temp_dir.path().join("small1.bin");
        let small2 = temp_dir.path().join("small2.bin");
        let large1 = temp_dir.path().join("large1.bin");
        let large2 = temp_dir.path().join("large2.bin");

        create_file_with_size(&small1, 1024, 0xAA).await?;
        create_file_with_size(&small2, 1024, 0xAA).await?;
        create_file_with_size(&large1, 10240, 0xBB).await?;
        create_file_with_size(&large2, 10240, 0xBB).await?;

        let files = vec![
            create_test_media_file(small1, 1024, 1),
            create_test_media_file(small2, 1024, 1),
            create_test_media_file(large1, 10240, 2),
            create_test_media_file(large2, 10240, 2),
        ];

        let detector = DuplicateDetector::new();
        let stats = detector.detect_duplicates(&files, false).await?;

        // Verify sorting by wasted space (largest first)
        assert_eq!(stats.groups[0].wasted_space, 10240);
        assert_eq!(stats.groups[1].wasted_space, 1024);

        Ok(())
    }
}
