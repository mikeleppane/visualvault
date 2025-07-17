use super::{FileType, MediaFile};
use std::collections::HashMap;


#[derive(Debug, Clone, Default)]
pub struct FileStatistics {
    pub total_files: usize,
    pub total_size: u64,
    pub by_type: HashMap<FileType, TypeStatistics>,
    pub duplicates_count: usize,
    pub duplicates_size: u64,
}

#[derive(Debug, Clone, Default)]
pub struct TypeStatistics {
    pub count: usize,
    pub total_size: u64,
    pub extensions: HashMap<String, usize>,
}

impl FileStatistics {
    pub fn from_files(files: &[MediaFile]) -> Self {
        let mut stats = Self::default();
        let mut by_type: HashMap<FileType, TypeStatistics> = HashMap::new();

        for file in files {
            stats.total_files += 1;
            stats.total_size += file.size;

            let type_stats = by_type.entry(file.file_type.clone()).or_default();
            type_stats.count += 1;
            type_stats.total_size += file.size;
            *type_stats
                .extensions
                .entry(file.extension.clone())
                .or_default() += 1;
        }

        stats.by_type = by_type;
        stats
    }
}
