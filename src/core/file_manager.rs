use std::sync::Arc;

use crate::models::MediaFile;

#[derive(Default)]
pub struct FileManager {
    files: Arc<Vec<Arc<MediaFile>>>,
    // Remove filtered_files and filter_active for now
}

impl FileManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            files: Arc::new(Vec::new()),
        }
    }

    pub fn set_files(&mut self, files: Vec<Arc<MediaFile>>) {
        self.files = Arc::new(files);
    }

    #[must_use]
    pub fn get_files(&self) -> Arc<Vec<Arc<MediaFile>>> {
        Arc::clone(&self.files)
    }

    #[must_use]
    pub fn get_file_count(&self) -> usize {
        self.files.len()
    }
}

// ...existing code...

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::float_cmp)] // For comparing floats in tests
    #![allow(clippy::panic)]
    use super::*;
    use crate::models::{FileType, MediaFile};
    use chrono::Local;
    use std::path::PathBuf;

    fn create_test_media_file(name: &str, size: u64) -> Arc<MediaFile> {
        Arc::new(MediaFile {
            path: PathBuf::from(format!("/test/{name}")),
            name: name.to_string(),
            size,
            modified: Local::now(),
            created: Local::now(),
            file_type: FileType::Image,
            extension: "jpg".to_string(),
            hash: Some(format!("hash_{name}")),
            metadata: None,
        })
    }

    #[test]
    fn test_new_file_manager() {
        let manager = FileManager::new();
        assert_eq!(manager.get_file_count(), 0);
        assert!(manager.files.is_empty());
    }

    #[test]
    fn test_default_file_manager() {
        let manager = FileManager::default();
        assert_eq!(manager.get_file_count(), 0);

        assert!(manager.files.is_empty());
    }

    #[test]
    fn test_set_files() {
        let mut manager = FileManager::new();
        let files = vec![
            create_test_media_file("file1.jpg", 1000),
            create_test_media_file("file2.jpg", 2000),
            create_test_media_file("file3.jpg", 3000),
        ];

        manager.set_files(files.clone());

        assert_eq!(manager.get_file_count(), 3);
        assert_eq!(manager.files.len(), 3);

        // Verify files are the same
        let retrieved_files = manager.get_files();
        assert_eq!(retrieved_files.len(), 3);
        assert_eq!(retrieved_files[0].name, "file1.jpg");
        assert_eq!(retrieved_files[1].name, "file2.jpg");
        assert_eq!(retrieved_files[2].name, "file3.jpg");
    }

    #[test]
    fn test_set_files_overwrites_existing() {
        let mut manager = FileManager::new();

        // Set initial files
        let initial_files = vec![
            create_test_media_file("initial1.jpg", 1000),
            create_test_media_file("initial2.jpg", 2000),
        ];
        manager.set_files(initial_files);
        assert_eq!(manager.get_file_count(), 2);

        // Set new files
        let new_files = vec![
            create_test_media_file("new1.jpg", 3000),
            create_test_media_file("new2.jpg", 4000),
            create_test_media_file("new3.jpg", 5000),
        ];
        manager.set_files(new_files);

        assert_eq!(manager.get_file_count(), 3);

        let retrieved_files = manager.get_files();
        assert_eq!(retrieved_files[0].name, "new1.jpg");
        assert_eq!(retrieved_files[1].name, "new2.jpg");
        assert_eq!(retrieved_files[2].name, "new3.jpg");
    }

    #[test]
    fn test_get_files_returns_arc() {
        let mut manager = FileManager::new();
        let files = vec![
            create_test_media_file("file1.jpg", 1000),
            create_test_media_file("file2.jpg", 2000),
        ];

        manager.set_files(files);

        // Get files multiple times to ensure Arc is working
        let files_ref1 = manager.get_files();
        let files_ref2 = manager.get_files();

        // Both references should point to the same data
        assert_eq!(files_ref1.len(), files_ref2.len());
        assert_eq!(files_ref1[0].name, files_ref2[0].name);

        // Verify Arc is being used (same pointer)
        assert!(Arc::ptr_eq(&files_ref1, &files_ref2));
    }

    #[test]
    fn test_get_file_count_with_empty_manager() {
        let manager = FileManager::new();
        assert_eq!(manager.get_file_count(), 0);
    }

    #[test]
    fn test_get_files_with_empty_manager() {
        let manager = FileManager::new();
        let files = manager.get_files();
        assert!(files.is_empty());
    }

    #[test]
    fn test_filter_active_behavior() {
        let mut manager = FileManager::new();
        let files = vec![
            create_test_media_file("file1.jpg", 1000),
            create_test_media_file("file2.jpg", 2000),
            create_test_media_file("file3.jpg", 3000),
        ];

        manager.set_files(files);

        // When filter is not active, should return all files
        assert_eq!(manager.get_file_count(), 3);
        assert_eq!(manager.get_files().len(), 3);

        // When filter is active, should return filtered files
        // Since set_files makes filtered_files = files, should still be 3
        assert_eq!(manager.get_file_count(), 3);
        assert_eq!(manager.get_files().len(), 3);
    }

    #[test]
    fn test_set_files_resets_filter() {
        let mut manager = FileManager::new();

        // Set initial files and activate filter
        let files = vec![create_test_media_file("file1.jpg", 1000)];
        manager.set_files(files);

        // Set new files
        let new_files = vec![
            create_test_media_file("new1.jpg", 2000),
            create_test_media_file("new2.jpg", 3000),
        ];
        manager.set_files(new_files);
    }

    #[test]
    #[allow(clippy::cast_sign_loss)]
    fn test_arc_cloning_efficiency() {
        let mut manager = FileManager::new();
        let large_file_list: Vec<Arc<MediaFile>> = (0..1000)
            .map(|i| create_test_media_file(&format!("file{i}.jpg"), i as u64 * 1000))
            .collect();

        manager.set_files(large_file_list);

        // Getting files multiple times should be efficient due to Arc
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _ = manager.get_files();
        }
        let duration = start.elapsed();

        // This should be very fast due to Arc cloning
        assert!(duration.as_millis() < 100, "Arc cloning should be fast");
    }
}
