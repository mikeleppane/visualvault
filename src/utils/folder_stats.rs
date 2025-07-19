#[derive(Debug, Clone)]
pub struct FolderStats {
    pub total_files: usize,
    pub total_dirs: usize,
    pub media_files: usize,
    pub total_size: u64,
}

impl Default for FolderStats {
    fn default() -> Self {
        Self {
            total_files: 0,
            total_dirs: 0,
            media_files: 0,
            total_size: 0,
        }
    }
}
