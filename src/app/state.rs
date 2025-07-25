use ahash::AHashMap;
use chrono::{DateTime, Local};
use color_eyre::eyre::Result;
use ratatui::widgets::ListState;
use std::{collections::HashSet, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

use crate::{
    config::Settings,
    core::{DuplicateDetector, DuplicateStats, FileManager, FileOrganizer, Scanner},
    models::{MediaFile, Statistics, filters::FilterSet},
    utils::{FolderStats, Progress},
};

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

pub struct App {
    // Core state
    pub state: AppState,
    pub input_mode: InputMode,
    pub should_quit: bool,

    // UI state
    pub show_help: bool,
    pub error_message: Option<String>,
    pub success_message: Option<String>,
    pub selected_tab: usize,
    pub selected_setting: usize,
    pub selected_file_index: usize,
    pub scroll_offset: usize,
    pub help_scroll: usize,

    // Components
    pub settings: Arc<RwLock<Settings>>,
    pub settings_cache: Settings,
    pub scanner: Arc<Scanner>,
    pub file_manager: Arc<RwLock<FileManager>>,
    pub organizer: Arc<FileOrganizer>,
    pub duplicate_detector: DuplicateDetector,

    // Data
    pub statistics: Statistics,
    pub progress: Arc<RwLock<Progress>>,
    pub cached_files: Vec<Arc<MediaFile>>,
    pub search_results: Vec<MediaFile>,
    pub duplicate_groups: Option<Vec<Vec<MediaFile>>>,
    pub duplicate_stats: Option<DuplicateStats>,
    pub folder_stats_cache: AHashMap<PathBuf, FolderStats>,

    // Search state
    pub search_input: String,

    // Input state
    pub input_buffer: String,
    pub editing_field: Option<EditingField>,

    // Results
    pub last_scan_result: Option<ScanResult>,
    pub last_organize_result: Option<OrganizeResult>,

    // Duplicate state
    pub selected_duplicate_group: usize,
    pub selected_duplicate_items: HashSet<usize>,
    pub duplicate_list_state: ListState,
    pub duplicate_focus: DuplicateFocus,
    pub selected_file_in_group: usize,
    pub pending_bulk_delete: bool,

    // Filter state
    pub filter_set: FilterSet,
    pub filter_tab: usize,
    pub filter_focus: FilterFocus,
    pub selected_filter_index: usize,
    pub filter_input: String,

    // Undo state
    pub last_undo_result: Option<String>,
}

impl App {
    /// Initializes a new `App` instance with default settings and components.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Settings cannot be loaded from the configuration file
    /// - Scanner cache initialization fails
    pub async fn init() -> Result<Self> {
        let mut duplicate_list_state = ListState::default();
        duplicate_list_state.select(Some(0));

        let settings = Settings::load().await?;
        let settings_cache = settings.clone();
        let settings = Arc::new(RwLock::new(settings));
        let file_manager = Arc::new(RwLock::new(FileManager::new()));
        let scanner = Arc::new(Scanner::with_cache().await?);
        let config_dir =
            dirs::config_dir().ok_or_else(|| color_eyre::eyre::eyre!("Could not find config directory"))?;
        let organizer = Arc::new(FileOrganizer::new(config_dir).await?);
        let statistics = Statistics::new();
        let progress = Arc::new(RwLock::new(Progress::new()));

        Ok(Self {
            state: AppState::Dashboard,
            input_mode: InputMode::Normal,
            should_quit: false,
            show_help: false,
            error_message: None,
            success_message: None,
            selected_tab: 0,
            selected_setting: 0,
            selected_file_index: 0,
            scroll_offset: 0,
            help_scroll: 0,
            settings,
            settings_cache,
            scanner,
            file_manager,
            organizer,
            duplicate_detector: DuplicateDetector::new(),
            statistics,
            progress,
            cached_files: Vec::new(),
            search_results: Vec::new(),
            duplicate_groups: None,
            duplicate_stats: None,
            folder_stats_cache: AHashMap::new(),
            search_input: String::new(),
            input_buffer: String::new(),
            editing_field: None,
            last_scan_result: None,
            last_organize_result: None,
            selected_duplicate_group: 0,
            selected_duplicate_items: HashSet::new(),
            duplicate_list_state,
            duplicate_focus: DuplicateFocus::GroupList,
            selected_file_in_group: 0,
            pending_bulk_delete: false,
            filter_set: FilterSet::new(),
            filter_tab: 0,
            filter_focus: FilterFocus::DateRange,
            selected_filter_index: 0,
            filter_input: String::new(),
            last_undo_result: None,
        })
    }

    pub fn clear_messages(&mut self) {
        self.error_message = None;
        self.success_message = None;
    }

    #[must_use]
    pub fn get_tab_count(&self) -> usize {
        match self.state {
            AppState::Dashboard => 4,
            AppState::Settings => 3,
            _ => 1,
        }
    }

    /// Updates the cached settings from the shared settings instance.
    ///
    /// # Errors
    ///
    /// This function currently does not return any errors, but the `Result` type
    /// is used for future compatibility.
    pub async fn update_settings_cache(&mut self) -> Result<()> {
        let settings = self.settings.read().await;
        self.settings_cache = settings.clone();
        drop(settings);
        Ok(())
    }
}
