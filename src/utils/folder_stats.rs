#[derive(Debug, Clone, Default)]
pub struct FolderStats {
    pub total_files: usize,
    pub total_dirs: usize,
    pub media_files: usize,
    pub total_size: u64,
}
