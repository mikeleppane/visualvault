use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::{
    config::Settings,
    core::{DuplicateDetector, FileManager, FileOrganizer, Scanner, Statistics},
    models::MediaFile,
    utils::Progress,
};

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Dashboard,
    Settings,
    Scanning,
    Organizing,
    DuplicateReview,
    Search,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Insert,
}

pub struct App {
    pub state: AppState,
    pub input_mode: InputMode,
    pub settings: Arc<RwLock<Settings>>,
    pub settings_cache: Settings, // Cached settings for UI rendering
    pub scanner: Arc<Scanner>,
    pub file_manager: Arc<RwLock<FileManager>>,
    pub organizer: Arc<FileOrganizer>,
    pub duplicate_detector: Arc<DuplicateDetector>,
    pub statistics: Statistics,
    pub progress: Arc<RwLock<Progress>>,
    pub show_help: bool,
    pub error_message: Option<String>,
    pub success_message: Option<String>,
    pub selected_tab: usize,
    pub selected_setting: usize, // Currently selected setting in settings view
    pub search_input: String,
    pub selected_file_index: usize,
    pub scroll_offset: usize,
    pub cached_files: Vec<MediaFile>,
    pub search_filters: SearchFilters,
    pub search_results: Vec<MediaFile>,
    pub input_buffer: String,                // Add this field for text input
    pub editing_field: Option<EditingField>, // Add this to track what we're editing
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditingField {
    SourceFolder,
    DestinationFolder,
    WorkerThreads,
    BufferSize,
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub filename_pattern: String,
    pub file_types: Vec<String>,
    pub date_from: Option<chrono::DateTime<chrono::Local>>,
    pub date_to: Option<chrono::DateTime<chrono::Local>>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
}

impl App {
    pub async fn new() -> Result<Self> {
        let settings = Settings::load().await?;
        let settings_cache = settings.clone();
        let settings = Arc::new(RwLock::new(settings));
        let file_manager = Arc::new(RwLock::new(FileManager::new()));
        let scanner = Arc::new(Scanner::new());
        let organizer = Arc::new(FileOrganizer::new());
        let duplicate_detector = Arc::new(DuplicateDetector::new());
        let statistics = Statistics::new();
        let progress = Arc::new(RwLock::new(Progress::new()));

        Ok(Self {
            state: AppState::Dashboard,
            input_mode: InputMode::Normal,
            settings,
            settings_cache,
            scanner,
            file_manager,
            organizer,
            duplicate_detector,
            statistics,
            progress,
            show_help: false,
            error_message: None,
            success_message: None,
            selected_tab: 0,
            selected_setting: 0,
            search_input: String::new(),
            selected_file_index: 0,
            scroll_offset: 0,
            cached_files: Vec::new(),
            search_filters: SearchFilters::default(),
            search_results: Vec::new(),
            input_buffer: String::new(),
            editing_field: None,
        })
    }

    pub async fn update_settings_cache(&mut self) -> Result<()> {
        let settings = self.settings.read().await;
        self.settings_cache = settings.clone();
        Ok(())
    }

    pub async fn on_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_mode(key).await,
            InputMode::Insert => self.handle_insert_mode(key).await,
        }
    }

    async fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<()> {
        use crossterm::event::KeyCode;

        // Clear messages on any key press
        self.error_message = None;
        self.success_message = None;

        match key.code {
            KeyCode::Tab => {
                self.next_tab();
            }
            KeyCode::BackTab => {
                self.previous_tab();
            }
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.show_help = !self.show_help;
            }
            KeyCode::Char('d') => {
                self.state = AppState::Dashboard;
            }
            KeyCode::Char('s') => {
                self.state = AppState::Settings;
                self.update_settings_cache().await?;
            }
            KeyCode::Char('f') => {
                self.state = AppState::Search;
            }
            KeyCode::Char('r') => {
                self.start_scan().await?;
            }
            KeyCode::Char('o') => {
                self.start_organize().await?;
            }
            KeyCode::Char('u') => {
                self.state = AppState::DuplicateReview;
            }
            _ => {
                // Handle state-specific keys
                match self.state {
                    AppState::Settings => self.handle_settings_keys(key).await?,
                    AppState::Search => self.handle_search_keys(key).await?,
                    AppState::DuplicateReview => self.handle_duplicate_keys(key).await?,
                    AppState::Dashboard => self.handle_dashboard_keys(key).await?,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    async fn handle_settings_keys(&mut self, key: KeyEvent) -> Result<()> {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Up => {
                if self.selected_setting > 0 {
                    self.selected_setting -= 1;
                }
            }
            KeyCode::Down => {
                // Adjust max based on current settings tab
                let max_settings = match self.selected_tab {
                    0 => 5, // General settings
                    1 => 9, // Organization settings
                    2 => 5, // Performance settings
                    _ => 0,
                };
                if self.selected_setting < max_settings {
                    self.selected_setting += 1;
                }
            }
            KeyCode::Enter => {
                // Handle editing the selected setting
                match (self.selected_tab, self.selected_setting) {
                    (0, 0) => {
                        // Edit source folder
                        self.input_mode = InputMode::Insert;
                        self.editing_field = Some(EditingField::SourceFolder);
                        self.input_buffer = self
                            .settings_cache
                            .source_folder
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                    }
                    (0, 1) => {
                        // Edit destination folder
                        self.input_mode = InputMode::Insert;
                        self.editing_field = Some(EditingField::DestinationFolder);
                        self.input_buffer = self
                            .settings_cache
                            .destination_folder
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                    }
                    (2, 0) => {
                        // Edit worker threads
                        self.input_mode = InputMode::Insert;
                        self.editing_field = Some(EditingField::WorkerThreads);
                        self.input_buffer = self.settings_cache.worker_threads.to_string();
                    }
                    (2, 1) => {
                        // Edit buffer size
                        self.input_mode = InputMode::Insert;
                        self.editing_field = Some(EditingField::BufferSize);
                        self.input_buffer =
                            (self.settings_cache.buffer_size / (1024 * 1024)).to_string();
                    }
                    _ => {}
                }
            }
            KeyCode::Char(' ') => {
                // Toggle boolean settings
                self.toggle_selected_setting().await?;
            }
            KeyCode::Char('S') | KeyCode::Char('s') => {
                // Save settings
                self.save_settings().await?;
            }
            KeyCode::Char('R') | KeyCode::Char('r') => {
                // Reset to defaults
                self.reset_settings().await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn toggle_selected_setting(&mut self) -> Result<()> {
        let mut settings = self.settings.write().await;

        match (self.selected_tab, self.selected_setting) {
            // General settings tab
            (0, 2) => settings.recurse_subfolders = !settings.recurse_subfolders,
            (0, 3) => settings.verbose_output = !settings.verbose_output,

            _ => {}
        }

        drop(settings);
        self.update_settings_cache().await?;
        Ok(())
    }

    async fn save_settings(&mut self) -> Result<()> {
        let settings = self.settings.read().await;
        settings.save().await?;
        self.success_message = Some("Settings saved successfully".to_string());
        Ok(())
    }

    async fn reset_settings(&mut self) -> Result<()> {
        let mut settings = self.settings.write().await;
        *settings = Settings::default();
        drop(settings);
        self.update_settings_cache().await?;
        self.success_message = Some("Settings reset to defaults".to_string());
        Ok(())
    }

    async fn handle_dashboard_keys(&mut self, _key: KeyEvent) -> Result<()> {
        // Dashboard-specific key handling
        Ok(())
    }

    async fn handle_search_keys(&mut self, _key: KeyEvent) -> Result<()> {
        // Search-specific key handling
        Ok(())
    }

    async fn handle_duplicate_keys(&mut self, _key: KeyEvent) -> Result<()> {
        // Duplicate review-specific key handling
        Ok(())
    }

    async fn handle_insert_mode(&mut self, key: KeyEvent) -> Result<()> {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.editing_field = None;
                self.input_buffer.clear();
            }
            KeyCode::Enter => {
                // Apply the input based on what we're editing
                if let Some(field) = &self.editing_field {
                    self.apply_edited_value(field.clone()).await?;
                }
                self.input_mode = InputMode::Normal;
                self.editing_field = None;
                self.input_buffer.clear();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Left => {
                // Move cursor left (if implementing cursor position)
            }
            KeyCode::Right => {
                // Move cursor right (if implementing cursor position)
            }
            KeyCode::Home => {
                // Move to beginning (if implementing cursor position)
            }
            KeyCode::End => {
                // Move to end (if implementing cursor position)
            }
            _ => {}
        }
        Ok(())
    }

    async fn apply_edited_value(&mut self, field: EditingField) -> Result<()> {
        let mut settings = self.settings.write().await;

        match field {
            EditingField::SourceFolder => {
                if self.input_buffer.is_empty() {
                    settings.source_folder = None;
                } else {
                    let path = PathBuf::from(&self.input_buffer);
                    if path.exists() && path.is_dir() {
                        settings.source_folder = Some(path);
                    } else {
                        self.error_message =
                            Some(format!("Invalid directory: {}", self.input_buffer));
                        return Ok(());
                    }
                }
            }
            EditingField::DestinationFolder => {
                if self.input_buffer.is_empty() {
                    settings.destination_folder = None;
                } else {
                    let path = PathBuf::from(&self.input_buffer);
                    // For destination, we can create it if it doesn't exist
                    settings.destination_folder = Some(path);
                }
            }
            EditingField::WorkerThreads => {
                if let Ok(threads) = self.input_buffer.parse::<usize>() {
                    if threads > 0 && threads <= num_cpus::get() * 2 {
                        settings.worker_threads = threads;
                    } else {
                        self.error_message = Some(format!(
                            "Worker threads must be between 1 and {}",
                            num_cpus::get() * 2
                        ));
                        return Ok(());
                    }
                } else {
                    self.error_message = Some("Invalid number for worker threads".to_string());
                    return Ok(());
                }
            }
            EditingField::BufferSize => {
                if let Ok(mb) = self.input_buffer.parse::<usize>() {
                    if mb > 0 && mb <= 1024 {
                        // Max 1GB
                        settings.buffer_size = mb * 1024 * 1024;
                    } else {
                        self.error_message =
                            Some("Buffer size must be between 1 and 1024 MB".to_string());
                        return Ok(());
                    }
                } else {
                    self.error_message = Some("Invalid number for buffer size".to_string());
                    return Ok(());
                }
            }
        }

        drop(settings);
        self.update_settings_cache().await?;
        self.success_message = Some("Setting updated".to_string());
        Ok(())
    }

    fn next_tab(&mut self) {
        let max_tabs = match self.state {
            AppState::Dashboard => 4,
            AppState::Settings => 3,
            _ => 1,
        };
        self.selected_tab = (self.selected_tab + 1) % max_tabs;
        self.selected_setting = 0; // Reset setting selection when changing tabs
    }

    fn previous_tab(&mut self) {
        let max_tabs = match self.state {
            AppState::Dashboard => 4,
            AppState::Settings => 3,
            _ => 1,
        };
        if self.selected_tab > 0 {
            self.selected_tab -= 1;
        } else {
            self.selected_tab = max_tabs - 1;
        }
        self.selected_setting = 0; // Reset setting selection when changing tabs
    }

    pub async fn on_tick(&mut self) -> Result<()> {
        // Update progress
        if matches!(self.state, AppState::Scanning | AppState::Organizing) {
            let mut progress = self.progress.write().await;
            progress.tick();
        }

        // Check for completed operations
        match self.state {
            AppState::Scanning => {
                if self.scanner.is_complete().await {
                    let count = self.file_manager.read().await.get_file_count();
                    self.success_message = Some(format!("Scan complete: {count} files found"));
                    self.state = AppState::Dashboard;
                    self.update_statistics().await?;
                }
            }
            AppState::Organizing => {
                if self.organizer.is_complete().await {
                    let result = self.organizer.get_result().await;
                    match result {
                        Some(Ok(count)) => {
                            self.success_message =
                                Some(format!("Successfully organized {} files", count));
                        }
                        Some(Err(e)) => {
                            self.error_message = Some(format!("Organization failed: {}", e));
                        }
                        None => {}
                    }
                    self.state = AppState::Dashboard;
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub fn should_quit(&self) -> bool {
        !matches!(self.state, AppState::Scanning | AppState::Organizing)
    }

    async fn start_scan(&mut self) -> Result<()> {
        let settings = self.settings.read().await;

        if let Some(source) = &settings.source_folder {
            info!("Starting scan of {:?}", source);
            self.state = AppState::Scanning;

            let mut progress = self.progress.write().await;
            progress.reset();
            progress.set_message("Scanning files...".to_owned());

            let scanner = self.scanner.clone();
            let file_manager = self.file_manager.clone();
            let source = source.clone();
            let recurse = settings.recurse_subfolders;
            let progress_handle = self.progress.clone();

            tokio::spawn(async move {
                match scanner
                    .scan_directory(&source, recurse, progress_handle)
                    .await
                {
                    Ok(files) => {
                        let mut fm = file_manager.write().await;
                        fm.set_files(files);
                    }
                    Err(e) => {
                        error!("Scan error: {}", e);
                    }
                }
            });
        } else {
            self.error_message = Some("No source folder configured".to_string());
        }

        Ok(())
    }

    async fn start_organize(&mut self) -> Result<()> {
        let settings = self.settings.read().await;

        if settings.source_folder.is_none() {
            self.error_message = Some("No source folder configured".to_string());
            return Ok(());
        }

        if settings.destination_folder.is_none() {
            self.error_message = Some("No destination folder configured".to_string());
            return Ok(());
        }

        info!("Starting file organization");
        self.state = AppState::Organizing;

        let mut progress = self.progress.write().await;
        progress.reset();
        progress.set_message("Organizing files...".to_owned());
        drop(progress);

        let files = self.file_manager.read().await.get_files();
        let organizer = self.organizer.clone();
        let settings = (*settings).clone();
        let progress_handle = self.progress.clone();

        tokio::spawn(async move {
            let _ = organizer
                .organize_files(files, settings, progress_handle)
                .await;
        });

        Ok(())
    }

    async fn update_statistics(&mut self) -> Result<()> {
        let files = self.file_manager.read().await.get_files();
        self.statistics.update_from_files(&files);
        self.cached_files = files; // Update cache for UI
        Ok(())
    }

    async fn move_selection_up(&mut self) {
        if self.selected_file_index > 0 {
            self.selected_file_index -= 1;
            if self.selected_file_index < self.scroll_offset {
                self.scroll_offset = self.selected_file_index;
            }
        }
    }

    async fn move_selection_down(&mut self) {
        let file_count = self.file_manager.read().await.get_file_count();
        if self.selected_file_index < file_count.saturating_sub(1) {
            self.selected_file_index += 1;
        }
    }

    async fn page_up(&mut self) {
        self.selected_file_index = self.selected_file_index.saturating_sub(10);
        if self.selected_file_index < self.scroll_offset {
            self.scroll_offset = self.selected_file_index;
        }
    }

    async fn page_down(&mut self) {
        let file_count = self.file_manager.read().await.get_file_count();
        self.selected_file_index =
            std::cmp::min(self.selected_file_index + 10, file_count.saturating_sub(1));
    }

    async fn handle_enter(&mut self) -> Result<()> {
        // Handle enter key based on current state
        Ok(())
    }

    async fn handle_delete(&mut self) -> Result<()> {
        // Handle delete key for file operations
        Ok(())
    }

    async fn apply_search(&mut self) -> Result<()> {
        let mut file_manager = self.file_manager.write().await;
        file_manager.filter_files(&self.search_input);
        self.selected_file_index = 0;
        self.scroll_offset = 0;
        Ok(())
    }

    pub fn get_tab_count(&self) -> usize {
        match self.state {
            AppState::Dashboard => 4,
            AppState::Settings => 3,
            _ => 1,
        }
    }
}
