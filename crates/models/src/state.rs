use std::path::PathBuf;

use chrono::{DateTime, Local};

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Dashboard,
    Settings,
    Scanning,
    Organizing,
    Search,
    FileDetails(usize),
    DuplicateReview,
    Filters,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Insert,
    Editing,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditingField {
    SourceFolder,
    DestinationFolder,
    WorkerThreads,
    BufferSize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DuplicateFocus {
    GroupList,
    FileList,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub files_found: usize,
    pub duration: std::time::Duration,
    pub timestamp: DateTime<Local>,
}

#[derive(Debug, Clone)]
pub struct OrganizeResult {
    pub files_organized: usize,
    pub files_total: usize,
    pub destination: PathBuf,
    pub success: bool,
    pub timestamp: DateTime<Local>,
    pub skipped_duplicates: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterFocus {
    DateRange,
    SizeRange,
    MediaType,
    RegexPattern,
}
