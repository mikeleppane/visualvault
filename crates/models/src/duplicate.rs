use std::sync::Arc;

use smallvec::SmallVec;

use crate::media_file::MediaFile;

#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub files: SmallVec<[Arc<MediaFile>; 4]>,
    pub wasted_space: u64, // Size that could be saved by keeping only one copy
}

impl DuplicateGroup {
    #[allow(dead_code)]
    #[must_use]
    pub fn new(files: impl Into<SmallVec<[Arc<MediaFile>; 4]>>, wasted_space: u64) -> Self {
        Self {
            files: files.into(),
            wasted_space,
        }
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
