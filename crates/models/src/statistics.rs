use ahash::AHashMap;
use chrono::Datelike;
use chrono::{DateTime, Local};
use std::path::PathBuf;
use std::sync::Arc;

use crate::MediaFile;
use crate::{DuplicateStats, FileType};

#[derive(Debug, Clone, Default)]
pub struct Statistics {
    pub total_files: usize,
    pub total_size: u64,
    pub duplicate_count: usize,
    pub media_types: AHashMap<String, usize>,
    pub type_sizes: AHashMap<String, u64>,
    pub files_by_date: AHashMap<String, usize>,
    pub files_by_year: AHashMap<u32, usize>,
    pub files_by_extension: AHashMap<String, usize>,
    pub largest_files: Vec<(PathBuf, u64)>,
    pub most_recent_files: Vec<(PathBuf, DateTime<Local>)>,
    pub duplicate_size: u64, // Total size of duplicate files (excluding one copy of each)
    pub file_types: AHashMap<FileType, usize>,
}

impl Statistics {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_from_files(&mut self, files: &[Arc<MediaFile>]) {
        // Reset statistics
        self.total_files = files.len();
        self.total_size = 0;
        self.media_types.clear();
        self.type_sizes.clear();
        self.files_by_date.clear();
        self.files_by_year.clear();
        self.files_by_extension.clear();
        self.largest_files.clear();
        self.most_recent_files.clear();

        // Calculate statistics
        for file in files {
            self.total_size += file.size;

            // Count by media type
            *self.media_types.entry(file.file_type.to_string()).or_insert(0) += 1;
            *self.type_sizes.entry(file.file_type.to_string()).or_insert(0) += file.size;

            #[allow(clippy::cast_sign_loss)]
            let date_key = file.modified.format("%Y-%m").to_string();
            *self.files_by_date.entry(date_key).or_insert(0) += 1;

            #[allow(clippy::cast_sign_loss)]
            let year = file.modified.year() as u32;
            *self.files_by_year.entry(year).or_insert(0) += 1;

            // Count by extension
            if let Some(ext) = file.path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                *self.files_by_extension.entry(ext_str).or_insert(0) += 1;
            }
        }

        // Find largest files
        let mut sorted_by_size: Vec<_> = files.iter().map(|f| (f.path.clone(), f.size)).collect();
        sorted_by_size.sort_by(|a, b| b.1.cmp(&a.1));
        self.largest_files = sorted_by_size.into_iter().take(10).collect();

        // Find most recent files
        let mut sorted_by_date: Vec<_> = files.iter().map(|f| (f.path.clone(), f.modified)).collect();
        sorted_by_date.sort_by(|a, b| b.1.cmp(&a.1));
        self.most_recent_files = sorted_by_date.into_iter().take(10).collect();
    }

    pub fn update_from_scan_results(&mut self, files: &[Arc<MediaFile>], duplicates: &DuplicateStats) {
        // Reset statistics
        self.total_files = files.len();
        self.total_size = files.iter().map(|f| f.size).sum();
        self.file_types.clear();
        self.media_types.clear();
        self.type_sizes.clear();

        // Count duplicates and calculate duplicate size
        for group in &duplicates.groups {
            if group.files.len() > 1 {
                // Count all duplicates except one (the one we'd keep)
                self.duplicate_count += group.files.len() - 1;

                // Calculate size of duplicates (excluding one copy)
                if let Some(file_size) = group.files.first().map(|f| f.size) {
                    self.duplicate_size += file_size * (group.files.len() - 1) as u64;
                }
            }
        }

        // Count file types
        for file in files {
            *self.file_types.entry(file.file_type.clone()).or_insert(0) += 1;
            *self.media_types.entry(file.file_type.to_string()).or_insert(0) += 1;
            *self.type_sizes.entry(file.file_type.to_string()).or_insert(0) += file.size;
        }
    }
}

// ... existing code ...

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::float_cmp)] // For comparing floats in tests
    #![allow(clippy::panic)]
    #![allow(clippy::cognitive_complexity)] // For test convenience
    use super::*;
    use crate::{DuplicateGroup, FileType, MediaFile};
    use chrono::{Local, TimeZone};
    use std::path::PathBuf;

    fn create_test_media_file(path: &str, size: u64, file_type: FileType, modified: DateTime<Local>) -> Arc<MediaFile> {
        let path_buf = PathBuf::from(path);
        let name = path_buf.file_name().unwrap().to_string_lossy().to_string();
        let extension = path_buf.extension().unwrap_or_default().to_string_lossy().to_string();

        Arc::new(MediaFile {
            path: path_buf,
            name: name.into(),
            extension: extension.into(),
            file_type,
            size,
            created: modified,
            modified,
            hash: None,
            metadata: None,
        })
    }

    fn create_test_files() -> Vec<Arc<MediaFile>> {
        vec![
            create_test_media_file(
                "/test/image1.jpg",
                1024 * 1024 * 5, // 5MB
                FileType::Image,
                Local.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap(),
            ),
            create_test_media_file(
                "/test/image2.png",
                1024 * 1024 * 3, // 3MB
                FileType::Image,
                Local.with_ymd_and_hms(2024, 2, 20, 14, 30, 0).unwrap(),
            ),
            create_test_media_file(
                "/test/video1.mp4",
                1024 * 1024 * 100, // 100MB
                FileType::Video,
                Local.with_ymd_and_hms(2024, 3, 10, 9, 15, 0).unwrap(),
            ),
            create_test_media_file(
                "/test/video2.avi",
                1024 * 1024 * 50, // 50MB
                FileType::Video,
                Local.with_ymd_and_hms(2023, 12, 25, 18, 45, 0).unwrap(),
            ),
            create_test_media_file(
                "/test/document.pdf",
                1024 * 512, // 512KB
                FileType::Document,
                Local.with_ymd_and_hms(2023, 11, 5, 11, 20, 0).unwrap(),
            ),
        ]
    }

    #[test]
    fn test_new_statistics() {
        let stats = Statistics::new();

        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_size, 0);
        assert_eq!(stats.duplicate_count, 0);
        assert_eq!(stats.duplicate_size, 0);
        assert!(stats.media_types.is_empty());
        assert!(stats.type_sizes.is_empty());
        assert!(stats.files_by_date.is_empty());
        assert!(stats.files_by_year.is_empty());
        assert!(stats.files_by_extension.is_empty());
        assert!(stats.largest_files.is_empty());
        assert!(stats.most_recent_files.is_empty());
        assert!(stats.file_types.is_empty());
    }

    #[test]
    fn test_update_from_files() {
        let mut stats = Statistics::new();
        let files = create_test_files();

        stats.update_from_files(&files);

        // Test total counts
        assert_eq!(stats.total_files, 5);
        assert_eq!(stats.total_size, 1024 * 1024 * 158 + 1024 * 512); // 158.5MB

        // Test media types
        assert_eq!(stats.media_types.get("Image"), Some(&2));
        assert_eq!(stats.media_types.get("Video"), Some(&2));
        assert_eq!(stats.media_types.get("Document"), Some(&1));

        // Test type sizes
        assert_eq!(stats.type_sizes.get("Image"), Some(&(1024 * 1024 * 8))); // 8MB
        assert_eq!(stats.type_sizes.get("Video"), Some(&(1024 * 1024 * 150))); // 150MB
        assert_eq!(stats.type_sizes.get("Document"), Some(&(1024 * 512))); // 512KB

        // Test files by date
        assert_eq!(stats.files_by_date.get("2024-01"), Some(&1));
        assert_eq!(stats.files_by_date.get("2024-02"), Some(&1));
        assert_eq!(stats.files_by_date.get("2024-03"), Some(&1));
        assert_eq!(stats.files_by_date.get("2023-12"), Some(&1));
        assert_eq!(stats.files_by_date.get("2023-11"), Some(&1));

        // Test files by year
        assert_eq!(stats.files_by_year.get(&2024), Some(&3));
        assert_eq!(stats.files_by_year.get(&2023), Some(&2));

        // Test files by extension
        assert_eq!(stats.files_by_extension.get("jpg"), Some(&1));
        assert_eq!(stats.files_by_extension.get("png"), Some(&1));
        assert_eq!(stats.files_by_extension.get("mp4"), Some(&1));
        assert_eq!(stats.files_by_extension.get("avi"), Some(&1));
        assert_eq!(stats.files_by_extension.get("pdf"), Some(&1));
    }

    #[test]
    fn test_largest_files() {
        let mut stats = Statistics::new();
        let files = create_test_files();

        stats.update_from_files(&files);

        // Should have 5 files (less than 10)
        assert_eq!(stats.largest_files.len(), 5);

        // Check order (largest first)
        assert_eq!(stats.largest_files[0].0, PathBuf::from("/test/video1.mp4"));
        assert_eq!(stats.largest_files[0].1, 1024 * 1024 * 100); // 100MB

        assert_eq!(stats.largest_files[1].0, PathBuf::from("/test/video2.avi"));
        assert_eq!(stats.largest_files[1].1, 1024 * 1024 * 50); // 50MB

        assert_eq!(stats.largest_files[2].0, PathBuf::from("/test/image1.jpg"));
        assert_eq!(stats.largest_files[2].1, 1024 * 1024 * 5); // 5MB
    }

    #[test]
    fn test_most_recent_files() {
        let mut stats = Statistics::new();
        let files = create_test_files();

        stats.update_from_files(&files);

        // Should have 5 files (less than 10)
        assert_eq!(stats.most_recent_files.len(), 5);

        // Check order (most recent first)
        assert_eq!(stats.most_recent_files[0].0, PathBuf::from("/test/video1.mp4"));
        assert_eq!(
            stats.most_recent_files[0].1,
            Local.with_ymd_and_hms(2024, 3, 10, 9, 15, 0).unwrap()
        );

        assert_eq!(stats.most_recent_files[1].0, PathBuf::from("/test/image2.png"));
        assert_eq!(
            stats.most_recent_files[1].1,
            Local.with_ymd_and_hms(2024, 2, 20, 14, 30, 0).unwrap()
        );
    }

    #[test]
    fn test_update_from_scan_results_no_duplicates() {
        let mut stats = Statistics::new();
        let files = create_test_files();
        let duplicates = DuplicateStats::new(); // No duplicates

        stats.update_from_scan_results(&files, &duplicates);

        assert_eq!(stats.total_files, 5);
        assert_eq!(stats.total_size, 1024 * 1024 * 158 + 1024 * 512);
        assert_eq!(stats.duplicate_count, 0);
        assert_eq!(stats.duplicate_size, 0);

        // Check file types
        assert_eq!(stats.file_types.get(&FileType::Image), Some(&2));
        assert_eq!(stats.file_types.get(&FileType::Video), Some(&2));
        assert_eq!(stats.file_types.get(&FileType::Document), Some(&1));
    }

    #[test]
    fn test_update_from_scan_results_with_duplicates() {
        let mut stats = Statistics::new();
        let files = create_test_files();

        // Create duplicate groups
        let mut duplicates = DuplicateStats::new();

        // Group 1: 3 identical 5MB images
        let duplicate_image =
            create_test_media_file("/test/dup_image.jpg", 1024 * 1024 * 5, FileType::Image, Local::now());
        duplicates.groups.push(DuplicateGroup::new(
            vec![duplicate_image.clone(), duplicate_image.clone(), duplicate_image],
            1024 * 1024 * 10, // 10MB wasted space
        ));

        // Group 2: 2 identical 10MB videos
        let duplicate_video =
            create_test_media_file("/test/dup_video.mp4", 1024 * 1024 * 10, FileType::Video, Local::now());
        duplicates.groups.push(DuplicateGroup::new(
            vec![duplicate_video.clone(), duplicate_video],
            1024 * 1024 * 10, // 10MB wasted space
        ));

        stats.update_from_scan_results(&files, &duplicates);

        // Should count 3 duplicates (3-1 + 2-1 = 2+1 = 3)
        assert_eq!(stats.duplicate_count, 3);

        // Duplicate size: 2 extra images (5MB each) + 1 extra video (10MB)
        assert_eq!(stats.duplicate_size, 1024 * 1024 * 20); // 20MB
    }

    #[test]
    fn test_empty_files() {
        let mut stats = Statistics::new();
        let files: Vec<Arc<MediaFile>> = vec![];

        stats.update_from_files(&files);

        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_size, 0);
        assert!(stats.largest_files.is_empty());
        assert!(stats.most_recent_files.is_empty());
    }

    #[test]
    fn test_files_without_extension() {
        let mut stats = Statistics::new();
        let file = create_test_media_file("/test/noextension", 1024, FileType::Other, Local::now());

        stats.update_from_files(&[file]);

        // Should handle files without extensions gracefully
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.files_by_extension.len(), 0); // No extension to count
    }

    #[test]
    fn test_case_insensitive_extensions() {
        let mut stats = Statistics::new();
        let files = vec![
            create_test_media_file("/test/image1.JPG", 1024, FileType::Image, Local::now()),
            create_test_media_file("/test/image2.jpg", 1024, FileType::Image, Local::now()),
            create_test_media_file("/test/image3.Jpg", 1024, FileType::Image, Local::now()),
        ];

        stats.update_from_files(&files);

        // All should be counted as "jpg" (lowercase)
        assert_eq!(stats.files_by_extension.get("jpg"), Some(&3));
        assert_eq!(stats.files_by_extension.len(), 1);
    }

    #[test]
    fn test_more_than_10_files() {
        let mut stats = Statistics::new();
        let mut files = Vec::new();

        // Create 15 files with different sizes and dates
        for i in 0..15 {
            files.push(create_test_media_file(
                &format!("/test/file{i}.jpg"),
                1024 * (u64::from(i) + 1), // Increasing sizes
                FileType::Image,
                Local.with_ymd_and_hms(2024, 1, i + 1, 10, 0, 0).unwrap(),
            ));
        }

        stats.update_from_files(&files);

        // Should only keep top 10 largest and most recent
        assert_eq!(stats.largest_files.len(), 10);
        assert_eq!(stats.most_recent_files.len(), 10);

        // Verify largest files are correct (should be files 14, 13, 12, ... 5)
        assert_eq!(stats.largest_files[0].1, 1024 * 15); // Largest
        assert_eq!(stats.largest_files[9].1, 1024 * 6); // 10th largest
    }

    #[test]
    fn test_reset_on_update() {
        let mut stats = Statistics::new();
        let files1 = create_test_files();

        // First update
        stats.update_from_files(&files1);
        assert_eq!(stats.total_files, 5);

        // Second update with different files
        let files2 = vec![create_test_media_file(
            "/test/single.jpg",
            1024,
            FileType::Image,
            Local::now(),
        )];

        stats.update_from_files(&files2);

        // Should reset and only have new data
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.total_size, 1024);
        assert_eq!(stats.media_types.get("Image"), Some(&1));
        assert_eq!(stats.media_types.get("Video"), None); // Old data cleared
    }

    #[test]
    fn test_duplicate_groups_edge_cases() {
        let mut stats = Statistics::new();
        let files = vec![];
        let mut duplicates: DuplicateStats = DuplicateStats::new();

        // Group with only 1 file (not a duplicate)
        duplicates.groups.push(DuplicateGroup::new(
            vec![create_test_media_file(
                "/test/single.jpg",
                1024,
                FileType::Image,
                Local::now(),
            )],
            0, // No wasted space since it's a single file
        ));

        // Empty group (edge case)
        duplicates.groups.push(DuplicateGroup::new(vec![], 0));

        stats.update_from_scan_results(&files, &duplicates);

        // Should not count single files or empty groups as duplicates
        assert_eq!(stats.duplicate_count, 0);
        assert_eq!(stats.duplicate_size, 0);
    }
}
