use chrono::Datelike;
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::models::{FileType, MediaFile};

#[derive(Debug, Clone, Default)]
pub struct Statistics {
    pub total_files: usize,
    pub total_size: u64,
    pub duplicate_count: usize,
    pub organized_size: u64,
    pub media_types: HashMap<String, usize>,
    pub type_sizes: HashMap<String, u64>,
    pub files_by_date: HashMap<String, usize>,
    pub files_by_year: HashMap<u32, usize>,
    pub files_by_extension: HashMap<String, usize>,
    pub largest_files: Vec<(PathBuf, u64)>,
    pub most_recent_files: Vec<(PathBuf, DateTime<Local>)>,
    pub duplicate_size: u64, // Total size of duplicate files (excluding one copy of each)
    pub file_types: HashMap<FileType, usize>,
}

impl Statistics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_from_files(&mut self, files: &[MediaFile]) {
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
            *self
                .media_types
                .entry(file.file_type.to_string())
                .or_insert(0) += 1;
            *self
                .type_sizes
                .entry(file.file_type.to_string())
                .or_insert(0) += file.size;

            // Count by date (YYYY-MM format)
            let date_key = file.modified.format("%Y-%m").to_string();
            *self.files_by_date.entry(date_key).or_insert(0) += 1;

            // Count by year
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
        let mut sorted_by_date: Vec<_> =
            files.iter().map(|f| (f.path.clone(), f.modified)).collect();
        sorted_by_date.sort_by(|a, b| b.1.cmp(&a.1));
        self.most_recent_files = sorted_by_date.into_iter().take(10).collect();
    }

    pub fn update_duplicate_count(&mut self, count: usize) {
        self.duplicate_count = count;
    }

    pub fn update_organized_size(&mut self, size: u64) {
        self.organized_size = size;
    }

    pub fn update_from_scan_results(
        &mut self,
        files: &[MediaFile],
        duplicates: &HashMap<String, Vec<MediaFile>>,
    ) {
        // Reset statistics
        self.total_files = files.len();
        self.total_size = files.iter().map(|f| f.size).sum();
        self.duplicate_count = 0;
        self.duplicate_size = 0;
        self.file_types.clear();
        self.media_types.clear();
        self.type_sizes.clear();

        // Count file types
        for file in files {
            *self.file_types.entry(file.file_type.clone()).or_insert(0) += 1;
            *self
                .media_types
                .entry(file.file_type.to_string())
                .or_insert(0) += 1;
            *self
                .type_sizes
                .entry(file.file_type.to_string())
                .or_insert(0) += file.size;
        }

        // Count duplicates and calculate duplicate size
        for (_, group) in duplicates {
            if group.len() > 1 {
                // Count all duplicates except one (the one we'd keep)
                self.duplicate_count += group.len() - 1;

                // Calculate size of duplicates (excluding one copy)
                if let Some(file_size) = group.first().map(|f| f.size) {
                    self.duplicate_size += file_size * (group.len() - 1) as u64;
                }
            }
        }
    }
}
