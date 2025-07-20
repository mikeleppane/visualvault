use chrono::{DateTime, Local};
use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::RwLock;
use tracing::{error, info};
use walkdir::WalkDir;

use crate::models::Statistics;

use crate::{
    config::Settings,
    core::{FileManager, FileOrganizer, Scanner},
    models::{FileType, ImageMetadata, MediaFile, MediaMetadata},
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Insert,
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
    pub skipped_duplicates: usize, // Add this field
    pub errors: Vec<String>,
}

pub struct App {
    pub state: AppState,
    pub input_mode: InputMode,
    pub settings: Arc<RwLock<Settings>>,
    pub settings_cache: Settings, // Cached settings for UI rendering
    pub scanner: Arc<Scanner>,
    pub file_manager: Arc<RwLock<FileManager>>,
    pub organizer: Arc<FileOrganizer>,
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
    pub search_results: Vec<MediaFile>,
    pub input_buffer: String,                // Add this field for text input
    pub editing_field: Option<EditingField>, // Add this to track what we're editing
    pub last_scan_result: Option<ScanResult>,
    pub last_organize_result: Option<OrganizeResult>,
    pub should_quit: bool,                             // Add this field to control quitting
    pub duplicate_groups: Option<Vec<Vec<MediaFile>>>, // Add this field// Add this for navigating duplicate groups
    pub folder_stats_cache: HashMap<PathBuf, FolderStats>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditingField {
    SourceFolder,
    DestinationFolder,
    WorkerThreads,
    BufferSize,
}

impl App {
    pub async fn new() -> Result<Self> {
        let settings = Settings::load().await?;
        let settings_cache = settings.clone();
        let settings = Arc::new(RwLock::new(settings));
        let file_manager = Arc::new(RwLock::new(FileManager::new()));
        let scanner = Arc::new(Scanner::with_cache().await?);
        let organizer = Arc::new(FileOrganizer::new());
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
            search_results: Vec::new(),
            input_buffer: String::new(),
            editing_field: None,
            last_scan_result: None,
            last_organize_result: None,
            should_quit: false,
            duplicate_groups: None,
            folder_stats_cache: HashMap::new(),
        })
    }

    pub async fn update_settings_cache(&mut self) -> Result<()> {
        let settings = self.settings.read().await;
        self.settings_cache = settings.clone();
        drop(settings);
        Ok(())
    }

    pub async fn on_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.state == AppState::Search {
            return {
                self.handle_search_keys(key);
                Ok(())
            };
        }

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

        match self.state {
            AppState::FileDetails(_) => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.state = AppState::Dashboard;
                    return Ok(());
                }
                _ => return Ok(()),
            },
            AppState::Search => {
                // For Search state, delegate ALL key handling to handle_search_keys
                return {
                    self.handle_search_keys(key);
                    Ok(())
                };
            }
            _ => {}
        }

        match key.code {
            KeyCode::Char('q') => {
                // Only quit if we're in Dashboard state on the main tab
                if self.state == AppState::Dashboard && [0, 1, 2, 3].contains(&self.selected_tab) {
                    self.should_quit = true;
                }
                // In other states/tabs, 'q' doesn't quit
            }
            KeyCode::Esc => {
                // ESC behavior depends on the state
                match self.state {
                    AppState::Dashboard => {
                        // On dashboard, ESC can quit if on main tab
                        if [0, 1, 2, 3].contains(&self.selected_tab) {
                            self.should_quit = true;
                        }
                    }
                    AppState::Settings => {
                        // Go back to dashboard
                        self.state = AppState::Dashboard;
                    }
                    _ => {
                        // In other states, just go back to dashboard
                        self.state = AppState::Dashboard;
                    }
                }
            }
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
            KeyCode::Char('r') => {
                self.start_scan().await?;
            }
            KeyCode::Char('o') => {
                self.start_organize().await?;
            }
            KeyCode::Char('u') => {
                // Update folder stats
                self.update_folder_stats().await?;
            }
            KeyCode::Char('f' | '/') => {
                // Switch to search mode
                self.state = AppState::Search;
                self.search_input.clear();
                self.search_results.clear();
                self.selected_file_index = 0;
                self.scroll_offset = 0;
                self.input_mode = InputMode::Normal;
            }
            _ => {
                // Handle state-specific keys
                match self.state {
                    AppState::Settings => self.handle_settings_keys(key).await?,
                    AppState::Dashboard => self.handle_dashboard_keys(key).await?,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    async fn start_editing_field(&mut self, field: EditingField) -> Result<()> {
        self.input_mode = InputMode::Insert;
        self.editing_field = Some(field.clone());

        // Pre-populate the input buffer with current value
        match field {
            EditingField::SourceFolder => {
                let settings = self.settings.read().await;
                self.input_buffer = settings
                    .source_folder
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
            }
            EditingField::DestinationFolder => {
                let settings = self.settings.read().await;
                self.input_buffer = settings
                    .destination_folder
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
            }
            EditingField::WorkerThreads => {
                let settings = self.settings.read().await;
                self.input_buffer = settings.worker_threads.to_string();
            }
            EditingField::BufferSize => {
                let settings = self.settings.read().await;
                // Convert bytes back to MB for display
                self.input_buffer = (settings.buffer_size / (1024 * 1024)).to_string();
            }
        }

        Ok(())
    }

    async fn handle_settings_keys(&mut self, key: KeyEvent) -> Result<()> {
        use crossterm::event::KeyCode;

        match self.selected_tab {
            0 => {
                // General settings tab
                match key.code {
                    KeyCode::Up => {
                        if self.selected_setting > 0 {
                            self.selected_setting -= 1;
                        }
                    }
                    KeyCode::Down => {
                        // 4 settings in general tab (source, dest, recurse, verbose)
                        if self.selected_setting < 3 {
                            self.selected_setting += 1;
                        }
                    }
                    KeyCode::Enter => match self.selected_setting {
                        0 => self.start_editing_field(EditingField::SourceFolder).await?,
                        1 => {
                            self.start_editing_field(EditingField::DestinationFolder).await?;
                        }
                        _ => {}
                    },
                    KeyCode::Char(' ') => match self.selected_setting {
                        2 => self.toggle_selected_setting().await?,
                        3 => self.toggle_selected_setting().await?,
                        _ => {}
                    },
                    _ => {}
                }
            }
            1 => {
                // Organization settings tab
                match key.code {
                    KeyCode::Up => {
                        if self.selected_setting > 0 {
                            self.selected_setting -= 1;
                        }
                    }
                    KeyCode::Down => {
                        // 10 settings in organization tab (5 modes + 5 options)
                        if self.selected_setting < 9 {
                            self.selected_setting += 1;
                        }
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if self.selected_setting < 5 {
                            // Organization mode selection (0-4)
                            self.change_organization_mode(self.selected_setting).await?;
                        } else {
                            // Toggle options (5-9)
                            self.toggle_selected_setting().await?;
                        }
                    }
                    _ => {}
                }
            }
            2 => {
                // Performance settings tab
                match key.code {
                    KeyCode::Up => {
                        if self.selected_setting > 0 {
                            self.selected_setting -= 1;
                        }
                    }
                    KeyCode::Down => {
                        // 6 settings in performance tab
                        if self.selected_setting < 5 {
                            self.selected_setting += 1;
                        }
                    }
                    KeyCode::Enter => match self.selected_setting {
                        0 => {
                            self.start_editing_field(EditingField::WorkerThreads).await?;
                        }
                        1 => self.start_editing_field(EditingField::BufferSize).await?,
                        _ => {}
                    },
                    KeyCode::Char(' ') => {
                        if self.selected_setting >= 2 {
                            self.toggle_selected_setting().await?;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        // Common keys for all tabs
        match key.code {
            KeyCode::Char('s' | 'S') => {
                self.save_settings().await?;
            }
            KeyCode::Char('r' | 'R') => {
                self.reset_settings().await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn change_organization_mode(&mut self, mode_index: usize) -> Result<()> {
        let modes = ["yearly", "monthly", "daily", "type", "type-date"];

        if let Some(mode) = modes.get(mode_index) {
            {
                let mut settings = self.settings.write().await;
                settings.organize_by = (*mode).to_string();
            }

            // Update the cache
            self.update_settings_cache().await?;

            self.success_message = Some(format!("Organization mode changed to: {mode}"));
        }

        Ok(())
    }

    async fn toggle_selected_setting(&mut self) -> Result<()> {
        match self.selected_tab {
            0 => {
                // General tab
                match self.selected_setting {
                    2 => {
                        let mut settings = self.settings.write().await;
                        settings.recurse_subfolders = !settings.recurse_subfolders;
                    }
                    3 => {
                        let mut settings = self.settings.write().await;
                        settings.verbose_output = !settings.verbose_output;
                    }
                    _ => {}
                }
            }
            1 => {
                // Organization tab - toggle options (indices 5-9)
                match self.selected_setting {
                    5 => {
                        let mut settings = self.settings.write().await;
                        settings.separate_videos = !settings.separate_videos;
                    }
                    6 => {
                        let mut settings = self.settings.write().await;
                        settings.keep_original_structure = !settings.keep_original_structure;
                    }
                    7 => {
                        let mut settings = self.settings.write().await;
                        settings.rename_duplicates = !settings.rename_duplicates;
                    }
                    8 => {
                        let mut settings = self.settings.write().await;
                        settings.lowercase_extensions = !settings.lowercase_extensions;
                    }
                    9 => {
                        let mut settings = self.settings.write().await;
                        settings.create_thumbnails = !settings.create_thumbnails;
                    }
                    _ => {}
                }
            }
            2 => {
                // Performance tab
                match self.selected_setting {
                    2 => {
                        let mut settings = self.settings.write().await;
                        settings.enable_cache = !settings.enable_cache;
                    }
                    3 => {
                        let mut settings = self.settings.write().await;
                        settings.parallel_processing = !settings.parallel_processing;
                    }
                    4 => {
                        let mut settings = self.settings.write().await;
                        settings.skip_hidden_files = !settings.skip_hidden_files;
                    }
                    5 => {
                        let mut settings = self.settings.write().await;
                        settings.optimize_for_ssd = !settings.optimize_for_ssd;
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        // Update the cache after any change
        self.update_settings_cache().await?;

        Ok(())
    }

    async fn save_settings(&mut self) -> Result<()> {
        let _ = self.settings.read().await;
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

    async fn handle_dashboard_keys(&mut self, key: KeyEvent) -> Result<()> {
        use crossterm::event::KeyCode;

        // Only handle these keys when in Files tab (tab index 1)
        if self.selected_tab == 1 {
            match key.code {
                KeyCode::Up => {
                    self.move_selection_up();
                }
                KeyCode::Down => {
                    self.move_selection_down();
                }
                KeyCode::PageUp => {
                    self.page_up();
                }
                KeyCode::PageDown => {
                    self.page_down();
                }
                KeyCode::Home => {
                    self.selected_file_index = 0;
                    self.scroll_offset = 0;
                }
                KeyCode::End => {
                    let file_count = self.cached_files.len();
                    if file_count > 0 {
                        self.selected_file_index = file_count - 1;
                        // Adjust scroll to show the last item
                        if self.selected_file_index >= 20 {
                            self.scroll_offset = self.selected_file_index - 19;
                        }
                    }
                }
                KeyCode::Enter => {
                    // Load metadata if not already loaded, then show file details modal
                    if !self.cached_files.is_empty() && self.selected_file_index < self.cached_files.len() {
                        // Check if we need to load metadata
                        let needs_metadata = self
                            .cached_files
                            .get(self.selected_file_index)
                            .is_some_and(|f| f.file_type == FileType::Image && f.metadata.is_none());

                        if needs_metadata {
                            // Show loading message in status bar
                            self.success_message = Some("Loading image metadata...".to_string());

                            // Force a UI update to show the loading message
                            // (You might need to trigger a render here depending on your main loop)

                            // Load metadata
                            // Extract the path before mutable borrow
                            let path = self.cached_files.get(self.selected_file_index).map(|f| f.path.clone());

                            if let Some(path) = path {
                                match self.load_image_metadata(&path).await {
                                    Ok(metadata) => {
                                        if let Some(file) = self.cached_files.get_mut(self.selected_file_index) {
                                            file.metadata = Some(metadata);
                                        }
                                        self.success_message = None;
                                    }
                                    Err(e) => {
                                        tracing::warn!("Failed to load metadata for {}: {}", path.display(), e);
                                        // Don't block opening the modal, just show what we have
                                        self.error_message = Some(format!("Metadata unavailable: {e}"));
                                    }
                                }
                            }
                        }

                        // Clear any loading messages
                        if self.success_message == Some("Loading image metadata...".to_string()) {
                            self.success_message = None;
                        }

                        // Show the file details
                        self.state = AppState::FileDetails(self.selected_file_index);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn load_image_metadata(&self, path: &Path) -> Result<MediaMetadata> {
        use image::GenericImageView;

        // Read file asynchronously
        let file_data = tokio::fs::read(path).await?;

        // Clone path data that we need inside the closure
        let path_owned = path.to_path_buf();

        // Parse image in a blocking task to avoid blocking the async runtime
        let metadata = tokio::task::spawn_blocking(move || -> Result<MediaMetadata> {
            let img = image::load_from_memory(&file_data)?;
            let (width, height) = img.dimensions();
            let color_type = match img.color() {
                image::ColorType::L8 => "Grayscale 8-bit",
                image::ColorType::La8 => "Grayscale + Alpha 8-bit",
                image::ColorType::Rgb8 => "RGB 8-bit",
                image::ColorType::Rgba8 => "RGBA 8-bit",
                image::ColorType::L16 => "Grayscale 16-bit",
                image::ColorType::La16 => "Grayscale + Alpha 16-bit",
                image::ColorType::Rgb16 => "RGB 16-bit",
                image::ColorType::Rgba16 => "RGBA 16-bit",
                _ => "Unknown",
            };

            // Detect format from extension or file header
            let format = path_owned
                .extension()
                .and_then(|e| e.to_str())
                .map_or_else(|| "Unknown".to_string(), str::to_uppercase);

            Ok(MediaMetadata::Image(ImageMetadata {
                width,
                height,
                format,
                color_type: color_type.to_string(),
            }))
        })
        .await??;

        Ok(metadata)
    }

    fn handle_search_keys(&mut self, key: KeyEvent) {
        use crossterm::event::KeyCode;
        match self.input_mode {
            InputMode::Normal => {
                match key.code {
                    KeyCode::Enter | KeyCode::Char('/') => {
                        // Enter insert mode to type search
                        self.input_mode = InputMode::Insert;
                        // Don't clear the search input here if you want to edit existing search
                    }
                    KeyCode::Esc => {
                        // Go back to dashboard
                        self.state = AppState::Dashboard;
                        self.search_input.clear();
                        self.search_results.clear();
                        self.selected_file_index = 0;
                        self.scroll_offset = 0;
                    }
                    KeyCode::Up => {
                        if !self.search_results.is_empty() && self.selected_file_index > 0 {
                            self.selected_file_index -= 1;
                            // Adjust scroll if needed
                            if self.selected_file_index < self.scroll_offset {
                                self.scroll_offset = self.selected_file_index;
                            }
                        }
                    }
                    KeyCode::Down => {
                        if !self.search_results.is_empty()
                            && self.selected_file_index < self.search_results.len().saturating_sub(1)
                        {
                            self.selected_file_index += 1;
                            // Adjust scroll if needed
                            if self.selected_file_index >= self.scroll_offset + 20 {
                                self.scroll_offset = self.selected_file_index - 19;
                            }
                        }
                    }
                    _ => {}
                }
            }
            InputMode::Insert => {
                match key.code {
                    KeyCode::Enter => {
                        // Exit insert mode and perform final search
                        self.perform_search();
                        self.input_mode = InputMode::Normal;
                    }
                    KeyCode::Esc => {
                        // Exit insert mode
                        self.input_mode = InputMode::Normal;
                    }
                    KeyCode::Char(c) => {
                        // Add character to search input
                        self.search_input.push(c);
                        // Perform live search as user types
                        self.perform_search();
                    }
                    KeyCode::Backspace => {
                        // Remove last character
                        self.search_input.pop();
                        // Update search results
                        self.perform_search();
                    }
                    KeyCode::Delete => {
                        // Clear the entire search
                        self.search_input.clear();
                        self.search_results.clear();
                        self.selected_file_index = 0;
                        self.scroll_offset = 0;
                    }
                    _ => {}
                }
            }
        }
    }

    fn perform_search(&mut self) {
        if self.search_input.is_empty() {
            self.search_results.clear();
            self.selected_file_index = 0;
            self.scroll_offset = 0;
            return;
        }
        let search_term = self.search_input.to_lowercase();
        self.search_results = self
            .cached_files
            .iter()
            .filter(|file| {
                // Search in file name
                file.name.to_lowercase().contains(&search_term) ||
                // Also search in path
                file.path.to_string_lossy().to_lowercase().contains(&search_term)
            })
            .cloned()
            .collect();
        self.selected_file_index = 0;
        self.scroll_offset = 0;
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
                        self.error_message = Some(format!("Invalid directory: {}", self.input_buffer));
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
                        self.error_message =
                            Some(format!("Worker threads must be between 1 and {}", num_cpus::get() * 2));
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
                        self.error_message = Some("Buffer size must be between 1 and 1024 MB".to_string());
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
            let _ = self.progress.write().await;
        }

        self.update_folder_stats_if_needed();

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
                            self.success_message = Some(format!("Successfully organized {count} files"));
                        }
                        Some(Err(e)) => {
                            self.error_message = Some(format!("Organization failed: {e}"));
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

    async fn start_scan(&mut self) -> Result<()> {
        let settings = self.settings.read().await;
        let source = settings
            .source_folder
            .clone()
            .ok_or_else(|| color_eyre::eyre::eyre!("Source folder not configured"))?;
        info!("Scanner: Starting scan of {:?}", source);
        let recursive = settings.recurse_subfolders;
        let settings_clone = settings.clone();
        drop(settings);
        self.state = AppState::Scanning;
        self.progress.write().await.reset();

        let start_time = std::time::Instant::now();
        let scanner = Arc::clone(&self.scanner);
        let progress = Arc::clone(&self.progress);

        // Instead of using tokio::spawn, directly await the scan
        let scan_result = scanner
            .scan_directory_with_duplicates(&source, recursive, progress, &settings_clone)
            .await;

        match scan_result {
            Ok((files, duplicates)) => {
                info!("=== SCAN RESULTS ===");
                info!("Scanner returned {} files", files.len());
                info!("Scanner returned {} duplicate groups", duplicates.len());
                let files_found = files.len();
                let duplicate_count = duplicates
                    .values()
                    .map(|group| group.len().saturating_sub(1))
                    .sum::<usize>();

                info!(
                    "Scan complete: {} files found, {} duplicates",
                    files_found, duplicate_count
                );

                // Update statistics with duplicate information
                self.statistics.update_from_scan_results(&files, &duplicates);

                // Store files in both file_manager and cached_files
                self.file_manager.write().await.set_files(files.clone());
                self.cached_files = files;

                info!("App cached_files now has {} entries", self.cached_files.len());

                // Store duplicate groups if any
                self.duplicate_groups = if duplicates.is_empty() {
                    None
                } else {
                    Some(duplicates.into_values().collect())
                };

                // Store scan result
                self.last_scan_result = Some(ScanResult {
                    files_found,
                    duration: start_time.elapsed(),
                    timestamp: Local::now(),
                });

                self.success_message = if duplicate_count > 0 {
                    Some(format!(
                        "Scan complete: {files_found} files found ({duplicate_count} duplicates)"
                    ))
                } else {
                    Some(format!("Scan complete: {files_found} files found"))
                };

                self.state = AppState::Dashboard;
            }
            Err(e) => {
                error!("Scan failed: {}", e);
                self.error_message = Some(format!("Scan failed: {e}"));
                self.state = AppState::Dashboard;
            }
        }

        Ok(())
    }

    async fn start_organize(&mut self) -> Result<()> {
        if self.cached_files.is_empty() {
            self.error_message = Some("No files to organize. Run a scan first.".to_string());
            return Ok(());
        }
        info!("Starting file organization");
        self.state = AppState::Organizing;
        self.progress.write().await.reset();

        let start_time = Local::now();
        let organizer = self.organizer.clone();
        let progress = self.progress.clone();
        let scanner = self.scanner.clone();
        let settings = self.settings.read().await;
        let settings_clone = settings.clone();
        let destination = settings
            .destination_folder
            .clone()
            .ok_or_else(|| color_eyre::eyre::eyre!("No destination folder configured"))?;
        drop(settings);

        let files = self.cached_files.clone();
        let files_total = files.len();

        // Find duplicates if rename_duplicates is false
        let duplicates = if settings_clone.rename_duplicates {
            HashMap::new()
        } else {
            let mut files_for_hash = files.clone();
            scanner.find_duplicates(&mut files_for_hash, progress.clone()).await?
        };

        // Directly await the organize operation
        let organize_result = organizer
            .organize_files_with_duplicates(files, duplicates, &settings_clone, progress)
            .await;

        match organize_result {
            Ok(result) => {
                info!("Organization complete: {} files organized", result.files_organized);

                // Store organize result
                let has_errors = !result.errors.is_empty();
                let error_count = result.errors.len();

                self.last_organize_result = Some(OrganizeResult {
                    files_organized: result.files_organized,
                    files_total,
                    destination,
                    success: result.success,
                    timestamp: start_time,
                    skipped_duplicates: result.skipped_duplicates,
                    errors: result.errors,
                });

                let message = if result.skipped_duplicates > 0 {
                    format!(
                        "Organization complete: {} files organized, {} duplicates skipped",
                        result.files_organized, result.skipped_duplicates
                    )
                } else {
                    format!("Organization complete: {} files organized", result.files_organized)
                };

                if has_errors {
                    self.error_message = Some(format!("{message} (with {error_count} errors)"));
                } else {
                    self.success_message = Some(message);
                }

                self.state = AppState::Dashboard;

                // Clear cached files after organizing
                self.cached_files.clear();
                self.duplicate_groups = None;
            }
            Err(e) => {
                error!("Organization failed: {}", e);

                // Store failed result
                self.last_organize_result = Some(OrganizeResult {
                    files_organized: 0,
                    files_total,
                    destination,
                    success: false,
                    timestamp: start_time,
                    skipped_duplicates: 0,
                    errors: vec![e.to_string()],
                });

                self.error_message = Some(format!("Organization failed: {e}"));
                self.state = AppState::Dashboard;
            }
        }

        Ok(())
    }

    async fn update_statistics(&mut self) -> Result<()> {
        let files = self.file_manager.read().await.get_files();
        self.statistics.update_from_files(&files);
        self.cached_files = files; // Update cache for UI
        Ok(())
    }

    fn move_selection_up(&mut self) {
        if self.selected_file_index > 0 {
            self.selected_file_index -= 1;
            if self.selected_file_index < self.scroll_offset {
                self.scroll_offset = self.selected_file_index;
            }
        }
    }

    fn move_selection_down(&mut self) {
        let file_count = self.cached_files.len();
        if self.selected_file_index < file_count.saturating_sub(1) {
            self.selected_file_index += 1;
            // Adjust scroll if needed - keep 20 items visible
            if self.selected_file_index >= self.scroll_offset + 20 {
                self.scroll_offset = self.selected_file_index - 19;
            }
        }
    }

    fn page_up(&mut self) {
        if self.selected_file_index >= 10 {
            self.selected_file_index -= 10;
        } else {
            self.selected_file_index = 0;
        }
        if self.selected_file_index < self.scroll_offset {
            self.scroll_offset = self.selected_file_index;
        }
    }

    fn page_down(&mut self) {
        let file_count = self.cached_files.len();
        self.selected_file_index = std::cmp::min(self.selected_file_index + 10, file_count.saturating_sub(1));
        // Adjust scroll if needed
        if self.selected_file_index >= self.scroll_offset + 20 {
            self.scroll_offset = self.selected_file_index.saturating_sub(19);
        }
    }

    pub fn update_folder_stats_if_needed(&mut self) {
        let mut paths_to_update = Vec::new();

        // Extract paths from settings and immediately drop the lock
        {
            let Ok(settings) = self.settings.try_read() else { return };

            // Check which paths need updating
            if let Some(source) = &settings.source_folder {
                if !self.folder_stats_cache.contains_key(source) {
                    paths_to_update.push(source.clone());
                }
            }

            if let Some(dest) = &settings.destination_folder {
                if !self.folder_stats_cache.contains_key(dest) {
                    paths_to_update.push(dest.clone());
                }
            }
        } // settings lock is dropped here

        // Calculate stats for paths that need it
        for path in paths_to_update {
            let path_clone = path.clone();
            let stats_result = std::thread::spawn(move || calculate_folder_stats_sync(&path_clone));

            // Get the result immediately (blocking, but walkdir is fast for reasonable directories)
            if let Ok(stats) = stats_result.join() {
                self.folder_stats_cache.insert(path, stats);
            }
        }
    }

    async fn update_folder_stats(&mut self) -> Result<()> {
        info!("Updating folder statistics...");

        // Clear existing cache to force recalculation
        self.folder_stats_cache.clear();

        // Show a progress message
        self.success_message = Some("Updating folder statistics...".to_string());

        // Get the settings
        let settings = self.settings.read().await;

        let mut paths_to_update = Vec::new();
        if let Some(source) = &settings.source_folder {
            paths_to_update.push(source.clone());
        }
        if let Some(dest) = &settings.destination_folder {
            paths_to_update.push(dest.clone());
        }

        drop(settings); // Release the lock

        // Calculate stats for each path
        let mut update_count = 0;
        for path in paths_to_update {
            let path_clone = path.clone();

            // Use tokio's spawn_blocking for async compatibility
            let stats_result = tokio::task::spawn_blocking(move || calculate_folder_stats_sync(&path_clone)).await;

            match stats_result {
                Ok(stats) => {
                    self.folder_stats_cache.insert(path, stats);
                    update_count += 1;
                }
                Err(e) => {
                    error!("Failed to calculate stats for {:?}: {}", path, e);
                }
            }
        }

        // Update success message
        if update_count > 0 {
            self.success_message = Some(format!("Updated statistics for {update_count} folder(s)"));
        } else {
            self.error_message = Some("No folders configured to update".to_string());
        }

        // Update success message
        if update_count > 0 {
            self.success_message = Some(format!("Updated statistics for {update_count} folder(s)"));
        } else {
            self.error_message = Some("No folders configured to update".to_string());
        }

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

fn calculate_folder_stats_sync(path: &std::path::Path) -> FolderStats {
    let mut stats = FolderStats { ..Default::default() };

    for entry in WalkDir::new(path).follow_links(false).into_iter().flatten() {
        if let Ok(metadata) = entry.metadata() {
            if metadata.is_file() {
                stats.total_files += 1;
                stats.total_size += metadata.len();

                // Check if it's a media file
                if let Some(ext) = entry.path().extension() {
                    if let Some(ext_str) = ext.to_str() {
                        if is_media_extension(ext_str) {
                            stats.media_files += 1;
                        }
                    }
                }
            } else if metadata.is_dir() {
                stats.total_dirs += 1;
            }
        }
    }

    stats
}

fn is_media_extension(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "jpg"
            | "jpeg"
            | "png"
            | "gif"
            | "bmp"
            | "webp"
            | "tiff"
            | "tif"
            | "svg"
            | "ico"
            | "heic"
            | "heif"
            | "mp4"
            | "avi"
            | "mkv"
            | "mov"
            | "wmv"
            | "flv"
            | "webm"
            | "m4v"
            | "mpg"
            | "mpeg"
            | "3gp"
            | "mp3"
            | "wav"
            | "flac"
            | "aac"
            | "ogg"
            | "wma"
            | "m4a"
            | "opus"
            | "ape"
    )
}
