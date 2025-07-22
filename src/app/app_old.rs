use ahash::AHashMap;
use chrono::{DateTime, Local, TimeZone};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::ListState;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::RwLock;
use tracing::{error, info};
use walkdir::WalkDir;

use crate::{
    core::{DuplicateDetector, DuplicateStats},
    models::{
        Statistics,
        filters::{FilterSet, RegexTarget},
    },
    utils::format_bytes,
};

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
    DuplicateReview,
    Filters,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Insert,
    Editing,
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
    pub input_buffer: String,
    pub editing_field: Option<EditingField>,
    pub last_scan_result: Option<ScanResult>,
    pub last_organize_result: Option<OrganizeResult>,
    pub should_quit: bool,
    pub duplicate_groups: Option<Vec<Vec<MediaFile>>>,
    pub folder_stats_cache: AHashMap<PathBuf, FolderStats>,
    pub duplicate_stats: Option<DuplicateStats>,
    pub duplicate_detector: DuplicateDetector,
    pub selected_duplicate_group: usize,
    pub selected_duplicate_items: HashSet<usize>,
    pub duplicate_list_state: ListState,
    pub duplicate_focus: DuplicateFocus,
    pub selected_file_in_group: usize,
    pub pending_bulk_delete: bool,
    pub filter_set: FilterSet,
    pub filter_tab: usize,
    pub filter_focus: FilterFocus,
    pub selected_filter_index: usize,
    pub filter_input: String,
    pub help_scroll: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterFocus {
    DateRange,
    SizeRange,
    MediaType,
    RegexPattern,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DuplicateFocus {
    GroupList,
    FileList,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditingField {
    SourceFolder,
    DestinationFolder,
    WorkerThreads,
    BufferSize,
}

impl App {
    /// Creates a new App instance with default settings and components.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Settings cannot be loaded from the configuration file
    /// - Scanner cache initialization fails
    /// - Any other component initialization fails
    pub async fn new() -> Result<Self> {
        let mut duplicate_list_state = ListState::default();
        duplicate_list_state.select(Some(0));
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
            folder_stats_cache: AHashMap::new(),
            duplicate_stats: None,
            duplicate_detector: DuplicateDetector::new(),
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
            help_scroll: 0,
        })
    }

    /// Updates the cached settings from the shared settings.
    ///
    /// # Errors
    /// This function currently cannot fail and always returns `Ok(())`.
    pub async fn update_settings_cache(&mut self) -> Result<()> {
        let settings = self.settings.read().await;
        self.settings_cache = settings.clone();
        drop(settings);
        Ok(())
    }

    /// Handles keyboard input events and updates application state accordingly.
    ///
    /// # Errors
    /// Returns an error if the key handling operation fails, such as when
    /// updating settings, performing file operations, or state transitions.
    pub async fn on_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.state == AppState::Search {
            return {
                self.handle_search_keys(key);
                Ok(())
            };
        }

        if self.state == AppState::Filters {
            return {
                self.handle_filter_keys(key);
                Ok(())
            };
        }

        if self.show_help {
            match key.code {
                KeyCode::Up => {
                    if self.help_scroll > 0 {
                        self.help_scroll -= 1;
                    }
                    return Ok(());
                }
                KeyCode::Down => {
                    // Calculate max scroll based on content
                    let content_lines: usize = 70; // Approximate number of help lines
                    let visible_lines: usize = 35; // Approximate visible lines in help area
                    let max_scroll = content_lines.saturating_sub(visible_lines);

                    if self.help_scroll < max_scroll {
                        self.help_scroll += 1;
                    }
                    return Ok(());
                }
                KeyCode::PageUp => {
                    self.help_scroll = self.help_scroll.saturating_sub(10);
                    return Ok(());
                }
                KeyCode::PageDown => {
                    let content_lines: usize = 70;
                    let visible_lines: usize = 35;
                    let max_scroll = content_lines.saturating_sub(visible_lines);

                    self.help_scroll = (self.help_scroll + 10).min(max_scroll);
                    return Ok(());
                }
                KeyCode::Home => {
                    self.help_scroll = 0;
                    return Ok(());
                }
                KeyCode::End => {
                    let content_lines: usize = 70;
                    let visible_lines: usize = 35;
                    self.help_scroll = content_lines.saturating_sub(visible_lines);
                    return Ok(());
                }
                _ => {
                    // Any other key closes help
                    self.show_help = false;
                    self.help_scroll = 0;
                    return Ok(());
                }
            }
        }

        match key.code {
            KeyCode::F(1) | KeyCode::Char('?') => {
                self.show_help = !self.show_help;
                self.help_scroll = 0; // Reset scroll when opening help
                return Ok(());
            }
            _ => {}
        }

        match self.input_mode {
            InputMode::Normal => self.handle_normal_mode(key).await,
            InputMode::Insert | InputMode::Editing => self.handle_insert_mode(key).await,
        }
    }

    #[allow(clippy::too_many_lines)]
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
            AppState::DuplicateReview => {
                // For DuplicateReview state, delegate to handle_duplicate_keys
                return self.handle_duplicate_keys(key).await;
            }
            AppState::Filters => {
                // For Filters state, delegate to handle_filter_keys
                return {
                    self.handle_filter_keys(key);
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
                // Only open settings if NOT in DuplicateReview state
                if self.state != AppState::DuplicateReview {
                    self.state = AppState::Settings;
                    self.update_settings_cache().await?;
                }
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
            KeyCode::Char('D') => {
                self.state = AppState::DuplicateReview;
            }
            KeyCode::Char('F') => {
                self.state = AppState::Filters;
                self.filter_tab = 0;
                self.selected_filter_index = 0;
                self.update_filter_focus();
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

    /// Handles keyboard input in settings mode.
    ///
    /// # Errors
    /// Returns an error if settings cannot be saved or updated.
    #[allow(clippy::too_many_lines)]
    pub async fn handle_settings_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('S' | 's') => {
                // Save settings
                self.save_settings().await?;
            }
            KeyCode::Char('R' | 'r') => {
                // Reset to defaults
                self.settings_cache = Settings::default();
                self.success_message = Some("Settings reset to defaults (not saved)".to_string());
            }
            KeyCode::Enter => {
                // Handle editing fields
                match self.selected_setting {
                    0 if self.selected_tab == 0 => {
                        // Source folder
                        if self.input_mode == InputMode::Normal {
                            self.input_mode = InputMode::Insert;
                            self.editing_field = Some(EditingField::SourceFolder);
                            self.input_buffer = self
                                .settings_cache
                                .source_folder
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default();
                        } else {
                            // Save the edited value
                            self.settings_cache.source_folder = if self.input_buffer.is_empty() {
                                None
                            } else {
                                Some(PathBuf::from(&self.input_buffer))
                            };
                            self.input_mode = InputMode::Normal;
                            self.editing_field = None;
                        }
                    }
                    1 if self.selected_tab == 0 => {
                        // Destination folder
                        if self.input_mode == InputMode::Normal {
                            self.input_mode = InputMode::Insert;
                            self.editing_field = Some(EditingField::DestinationFolder);
                            self.input_buffer = self
                                .settings_cache
                                .destination_folder
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default();
                        } else {
                            // Save the edited value
                            self.settings_cache.destination_folder = if self.input_buffer.is_empty() {
                                None
                            } else {
                                Some(PathBuf::from(&self.input_buffer))
                            };
                            self.input_mode = InputMode::Normal;
                            self.editing_field = None;
                        }
                    }
                    // Add other field handlers...
                    _ => {}
                }
            }
            KeyCode::Esc => {
                if self.input_mode == InputMode::Insert {
                    // Cancel editing
                    self.input_mode = InputMode::Normal;
                    self.editing_field = None;
                    self.input_buffer.clear();
                } else {
                    // Exit settings
                    self.state = AppState::Dashboard;
                }
            }
            KeyCode::Char(' ') => {
                // Toggle checkboxes
                match (self.selected_tab, self.selected_setting) {
                    (0, 2) => self.settings_cache.recurse_subfolders = !self.settings_cache.recurse_subfolders,
                    (0, 3) => self.settings_cache.verbose_output = !self.settings_cache.verbose_output,
                    (1, s) if s < 5 => {
                        // Radio buttons for organization mode
                        self.settings_cache.organize_by = match s {
                            1 => "monthly",
                            2 => "daily",
                            3 => "type",
                            4 => "type-date",
                            _ => "yearly",
                        }
                        .to_string();
                    }
                    (1, 5) => self.settings_cache.separate_videos = !self.settings_cache.separate_videos,
                    (1, 6) => {
                        self.settings_cache.keep_original_structure = !self.settings_cache.keep_original_structure;
                    }
                    (1, 7) => self.settings_cache.rename_duplicates = !self.settings_cache.rename_duplicates,
                    (1, 8) => self.settings_cache.lowercase_extensions = !self.settings_cache.lowercase_extensions,
                    (1, 9) => self.settings_cache.create_thumbnails = !self.settings_cache.create_thumbnails,
                    (2, 2) => self.settings_cache.enable_cache = !self.settings_cache.enable_cache,
                    (2, 3) => self.settings_cache.parallel_processing = !self.settings_cache.parallel_processing,
                    (2, 4) => self.settings_cache.skip_hidden_files = !self.settings_cache.skip_hidden_files,
                    (2, 5) => self.settings_cache.optimize_for_ssd = !self.settings_cache.optimize_for_ssd,
                    _ => {}
                }
            }
            KeyCode::Up => {
                if self.selected_setting > 0 {
                    self.selected_setting -= 1;
                }
            }
            KeyCode::Down => {
                let max_setting = match self.selected_tab {
                    0 => 3, // General tab: 2 folders + 2 checkboxes
                    1 => 9, // Organization tab: 5 radio + 5 checkboxes
                    2 => 5, // Performance tab: 2 inputs + 4 checkboxes
                    _ => 0,
                };
                if self.selected_setting < max_setting {
                    self.selected_setting += 1;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn save_settings(&mut self) -> Result<()> {
        // Update the actual settings from the cache
        let mut settings = self.settings.write().await;
        *settings = self.settings_cache.clone();
        settings.save()?;
        drop(settings);

        // Update success message
        self.success_message = Some("Settings saved successfully!".to_string());

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
            InputMode::Editing => {
                // Handle editing mode for search (unlikely to be used in search, but needs to be covered)
                if key.code == KeyCode::Esc {
                    // Exit editing mode and go back to normal mode
                    self.input_mode = InputMode::Normal;
                } else {
                    // In search context, treat Editing like Insert
                    self.input_mode = InputMode::Insert;
                    self.handle_search_keys(key);
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

    /// Handles periodic updates and state transitions.
    ///
    /// # Errors
    /// Returns an error if statistics update fails.
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
        self.success_message = Some("Starting scan...".to_string());
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

        let filter_set = if self.filter_set.is_active {
            Some(self.filter_set.clone())
        } else {
            None
        };

        // Instead of using tokio::spawn, directly await the scan
        let scan_result = scanner
            .scan_directory_with_duplicates(&source, recursive, progress, &settings_clone, filter_set)
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
        self.success_message = Some("Starting to organize files...".to_string());
        self.state = AppState::Organizing;
        self.progress.write().await.reset();

        let start_time = Local::now();
        let organizer = self.organizer.clone();
        let progress = self.progress.clone();
        let scanner = self.scanner.clone();
        let settings = self.settings.read().await;
        let destination = settings
            .destination_folder
            .clone()
            .ok_or_else(|| color_eyre::eyre::eyre!("No destination folder configured"))?;

        let mut files = self.cached_files.clone();
        let files_total = files.len();

        // Find duplicates if rename_duplicates is false
        let duplicates = if settings.rename_duplicates {
            AHashMap::new()
        } else {
            scanner.find_duplicates(&mut files, progress.clone()).await?
        };

        // Directly await the organize operation
        let organize_result = organizer
            .organize_files_with_duplicates(files, duplicates, &settings, progress)
            .await;
        drop(settings); // Release the lock early
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
        self.cached_files.clone_from(&(*files)); // Update cache for UI
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

    #[must_use]
    pub fn get_tab_count(&self) -> usize {
        match self.state {
            AppState::Dashboard => 4,
            AppState::Settings => 3,
            _ => 1,
        }
    }

    /// Starts a duplicate file scan operation.
    ///
    /// # Errors
    /// Returns an error if the duplicate detection process fails.
    pub async fn start_duplicate_scan(&mut self) -> Result<()> {
        self.error_message = None;
        self.success_message = Some("Scanning for duplicates...".to_string());

        // Make sure we have files to scan
        if self.cached_files.is_empty() {
            self.error_message = Some("No files to scan. Run a file scan first.".to_string());
            self.success_message = None;
            return Ok(());
        }

        // Use cached files for duplicate detection
        let stats = self
            .duplicate_detector
            .detect_duplicates(&self.cached_files, false)
            .await?;

        let message = if stats.total_groups > 0 {
            format!(
                "Found {} duplicate groups with {} files wasting {}",
                stats.total_groups,
                stats.total_duplicates,
                format_bytes(stats.total_wasted_space)
            )
        } else {
            "No duplicates found.".to_string()
        };

        let has_groups = stats.total_groups > 0;
        self.duplicate_stats = Some(stats);
        self.success_message = Some(message);
        self.state = AppState::DuplicateReview;

        // Reset selection states
        self.selected_duplicate_group = 0;
        self.selected_duplicate_items.clear();
        self.duplicate_list_state
            .select(if has_groups { Some(0) } else { None });

        Ok(())
    }

    /// Handles keyboard input in duplicate review mode.
    ///
    /// # Errors
    /// Returns an error if file operations (scanning, deleting) fail.
    #[allow(clippy::too_many_lines)]
    pub async fn handle_duplicate_keys(&mut self, key: KeyEvent) -> Result<()> {
        if self.pending_bulk_delete {
            match key.code {
                KeyCode::Char('y' | 'Y') => {
                    self.pending_bulk_delete = false;
                    self.perform_bulk_delete().await?;
                }
                KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                    self.pending_bulk_delete = false;
                    self.error_message = Some("Bulk delete cancelled".to_string());
                }
                _ => {}
            }
            return Ok(());
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.state = AppState::Dashboard;
                self.selected_duplicate_items.clear();
            }
            KeyCode::Char('s') => {
                self.start_duplicate_scan().await?;
            }
            KeyCode::Up => match self.duplicate_focus {
                DuplicateFocus::GroupList => {
                    if self.selected_duplicate_group > 0 {
                        self.selected_duplicate_group -= 1;
                        self.duplicate_list_state.select(Some(self.selected_duplicate_group));
                        self.selected_duplicate_items.clear();
                    }
                }
                DuplicateFocus::FileList => {
                    if let Some(stats) = &self.duplicate_stats {
                        if stats.groups.get(self.selected_duplicate_group).is_some() && self.selected_file_in_group > 0
                        {
                            self.selected_file_in_group -= 1;
                        }
                    }
                }
            },
            KeyCode::Down => match self.duplicate_focus {
                DuplicateFocus::GroupList => {
                    if let Some(stats) = &self.duplicate_stats {
                        if !stats.groups.is_empty() && self.selected_duplicate_group < stats.groups.len() - 1 {
                            self.selected_duplicate_group += 1;
                            self.duplicate_list_state.select(Some(self.selected_duplicate_group));
                            self.selected_duplicate_items.clear();
                        }
                    }
                }
                DuplicateFocus::FileList => {
                    if let Some(stats) = &self.duplicate_stats {
                        if let Some(group) = stats.groups.get(self.selected_duplicate_group) {
                            if self.selected_file_in_group < group.files.len() - 1 {
                                self.selected_file_in_group += 1;
                            }
                        }
                    }
                }
            },
            KeyCode::Left => {
                self.duplicate_focus = DuplicateFocus::GroupList;
                self.selected_file_in_group = 0;
            }
            KeyCode::Right => {
                if let Some(stats) = &self.duplicate_stats {
                    if !stats.groups.is_empty() {
                        self.duplicate_focus = DuplicateFocus::FileList;
                        self.selected_file_in_group = 0;
                    }
                }
            }
            KeyCode::Char(' ') => {
                if self.duplicate_focus == DuplicateFocus::FileList {
                    if self.selected_duplicate_items.contains(&self.selected_file_in_group) {
                        self.selected_duplicate_items.remove(&self.selected_file_in_group);
                    } else {
                        self.selected_duplicate_items.insert(self.selected_file_in_group);
                    }
                }
            }
            KeyCode::Char('a') => {
                // Select all but the first file in the current group
                if let Some(stats) = &self.duplicate_stats {
                    if let Some(group) = stats.groups.get(self.selected_duplicate_group) {
                        self.selected_duplicate_items.clear();
                        for i in 1..group.files.len() {
                            self.selected_duplicate_items.insert(i);
                        }
                    }
                }
            }
            KeyCode::Char('d') => {
                // Delete selected files in current group
                if !self.selected_duplicate_items.is_empty() {
                    self.delete_selected_duplicates().await?;
                }
            }
            KeyCode::Char('D') => {
                // Set pending and show confirmation message
                if let Some(stats) = &self.duplicate_stats {
                    if stats.total_duplicates > 0 {
                        self.pending_bulk_delete = true;
                        self.error_message = Some(format!(
                            "⚠️  Delete {} duplicates from {} groups? This will free {}. Press Y to confirm, N to cancel",
                            stats.total_duplicates,
                            stats.total_groups,
                            format_bytes(stats.total_wasted_space)
                        ));
                    } else {
                        self.error_message = Some("No duplicates to delete".to_string());
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn perform_bulk_delete(&mut self) -> Result<()> {
        if let Some(stats) = &self.duplicate_stats {
            let mut paths_to_delete = Vec::new();

            // Collect all duplicate files (skip first in each group)
            for group in &stats.groups {
                for (idx, file) in group.files.iter().enumerate() {
                    if idx > 0 {
                        // Skip the first file (keep it as original)
                        paths_to_delete.push(file.path.clone());
                    }
                }
            }

            if !paths_to_delete.is_empty() {
                let total_to_delete = paths_to_delete.len();
                let deleted = self.duplicate_detector.delete_files(&paths_to_delete).await?;

                self.success_message = Some(format!(
                    "✅ Successfully deleted {} of {} duplicate files, freed {}",
                    deleted.len(),
                    total_to_delete,
                    format_bytes(stats.total_wasted_space)
                ));

                // Clear selections and rescan
                self.selected_duplicate_items.clear();
                self.start_duplicate_scan().await?;
            }
        }
        Ok(())
    }

    async fn delete_selected_duplicates(&mut self) -> Result<()> {
        if let Some(stats) = &self.duplicate_stats {
            if let Some(group) = stats.groups.get(self.selected_duplicate_group) {
                let mut paths_to_delete = Vec::new();

                for &idx in &self.selected_duplicate_items {
                    if let Some(file) = group.files.get(idx) {
                        paths_to_delete.push(file.path.clone());
                    }
                }

                if !paths_to_delete.is_empty() {
                    let deleted = self.duplicate_detector.delete_files(&paths_to_delete).await?;
                    self.success_message = Some(format!("Deleted {} files", deleted.len()));

                    // Clear selections and rescan
                    self.selected_duplicate_items.clear();
                    self.start_duplicate_scan().await?;
                }
            }
        }
        Ok(())
    }

    pub fn handle_filter_keys(&mut self, key: KeyEvent) {
        use crossterm::event::KeyCode;
        if self.input_mode == InputMode::Editing {
            match key.code {
                KeyCode::Enter => {
                    self.save_current_filter();
                    self.input_mode = InputMode::Normal;
                    self.filter_input.clear();
                }
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                    self.filter_input.clear();
                }
                KeyCode::Char(c) => {
                    self.filter_input.push(c);
                }
                KeyCode::Backspace => {
                    self.filter_input.pop();
                }
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Esc => {
                self.state = AppState::Dashboard;
            }
            KeyCode::Tab => {
                self.filter_tab = (self.filter_tab + 1) % 4;
                self.selected_filter_index = 0;
                self.update_filter_focus();
            }
            KeyCode::BackTab => {
                self.filter_tab = if self.filter_tab == 0 { 3 } else { self.filter_tab - 1 };
                self.selected_filter_index = 0;
                self.update_filter_focus();
            }
            KeyCode::Up => {
                if self.selected_filter_index > 0 {
                    self.selected_filter_index -= 1;
                }
            }
            KeyCode::Down => {
                let max_index = match self.filter_focus {
                    FilterFocus::DateRange => self.filter_set.date_ranges.len(),
                    FilterFocus::SizeRange => self.filter_set.size_ranges.len(),
                    FilterFocus::MediaType => self.filter_set.media_types.len(),
                    FilterFocus::RegexPattern => self.filter_set.regex_patterns.len(),
                };
                if max_index > 0 && self.selected_filter_index < max_index - 1 {
                    self.selected_filter_index += 1;
                }
            }
            KeyCode::Char('a') => {
                self.input_mode = InputMode::Editing;
                self.filter_input.clear();
            }
            KeyCode::Char('d') => {
                self.delete_selected_filter();
            }
            KeyCode::Char(' ') => {
                self.toggle_selected_filter();
            }
            KeyCode::Char('c') => {
                self.filter_set.clear_all();
                self.success_message = Some("All filters cleared".to_string());
            }
            KeyCode::Char('t') => {
                self.filter_set.is_active = !self.filter_set.is_active;
                let status = if self.filter_set.is_active {
                    "enabled"
                } else {
                    "disabled"
                };
                self.success_message = Some(format!("Filters {status}"));
            }
            KeyCode::Enter => {
                self.apply_filters();
                self.state = AppState::Dashboard;
            }
            _ => {}
        }
    }

    fn update_filter_focus(&mut self) {
        self.filter_focus = match self.filter_tab {
            1 => FilterFocus::SizeRange,
            2 => FilterFocus::MediaType,
            3 => FilterFocus::RegexPattern,
            _ => FilterFocus::DateRange,
        };
    }

    fn save_current_filter(&mut self) {
        match self.filter_focus {
            FilterFocus::DateRange => {
                if let Some((from, to)) = Self::parse_date_range(&self.filter_input) {
                    let name = self.filter_input.clone();
                    self.filter_set.add_date_range(name, from, to);
                    self.success_message = Some("Date range added".to_string());
                } else {
                    self.error_message =
                        Some("Invalid date format. Use 'YYYY-MM-DD to YYYY-MM-DD' or 'last 7 days'".to_string());
                }
            }
            FilterFocus::SizeRange => {
                if let Some((min, max)) = Self::parse_size_range(&self.filter_input) {
                    let name = self.filter_input.clone();
                    self.filter_set.add_size_range(name, min, max);
                    self.success_message = Some("Size range added".to_string());
                } else {
                    self.error_message = Some("Invalid size format. Use '>10MB', '<1GB', or '10MB-100MB'".to_string());
                }
            }
            FilterFocus::RegexPattern => {
                if !self.filter_input.is_empty() {
                    self.filter_set
                        .add_regex_pattern(self.filter_input.clone(), RegexTarget::FileName, false);
                    self.success_message = Some("Regex pattern added".to_string());
                }
            }
            FilterFocus::MediaType => {}
        }
    }

    fn parse_date_range(
        input: &str,
    ) -> Option<(
        Option<chrono::DateTime<chrono::Local>>,
        Option<chrono::DateTime<chrono::Local>>,
    )> {
        use chrono::{Duration, Local, NaiveDate};

        let now = Local::now();
        let input_lower = input.to_lowercase();

        // Handle special cases
        match input_lower.as_str() {
            "today" => {
                let today_start = now.date_naive().and_hms_opt(0, 0, 0)?;
                let today_end = now.date_naive().and_hms_opt(23, 59, 59)?;
                Some((
                    Some(Local.from_local_datetime(&today_start).unwrap()),
                    Some(Local.from_local_datetime(&today_end).unwrap()),
                ))
            }
            "yesterday" => {
                let yesterday = now - Duration::days(1);
                let yesterday_start = yesterday.date_naive().and_hms_opt(0, 0, 0)?;
                let yesterday_end = yesterday.date_naive().and_hms_opt(23, 59, 59)?;
                Some((
                    Some(Local.from_local_datetime(&yesterday_start).unwrap()),
                    Some(Local.from_local_datetime(&yesterday_end).unwrap()),
                ))
            }
            "last 7 days" | "last week" => Some((Some(now - Duration::days(7)), Some(now))),
            "last 30 days" | "last month" => Some((Some(now - Duration::days(30)), Some(now))),
            "last year" | "last 365 days" => Some((Some(now - Duration::days(365)), Some(now))),
            _ => {
                // Try to parse "YYYY-MM-DD to YYYY-MM-DD"
                let parts: Vec<&str> = input.split(" to ").collect();
                if parts.len() == 2 {
                    let from = NaiveDate::parse_from_str(parts[0].trim(), "%Y-%m-%d")
                        .ok()
                        .and_then(|d| d.and_hms_opt(0, 0, 0))
                        .map(|dt| Local.from_local_datetime(&dt).unwrap());
                    let to = NaiveDate::parse_from_str(parts[1].trim(), "%Y-%m-%d")
                        .ok()
                        .and_then(|d| d.and_hms_opt(23, 59, 59))
                        .map(|dt| Local.from_local_datetime(&dt).unwrap());

                    if from.is_some() || to.is_some() {
                        Some((from, to))
                    } else {
                        None
                    }
                } else {
                    // Try single date
                    NaiveDate::parse_from_str(input.trim(), "%Y-%m-%d").ok().and_then(|d| {
                        let start = d.and_hms_opt(0, 0, 0)?;
                        let end = d.and_hms_opt(23, 59, 59)?;
                        Some((
                            Some(Local.from_local_datetime(&start).unwrap()),
                            Some(Local.from_local_datetime(&end).unwrap()),
                        ))
                    })
                }
            }
        }
    }

    fn parse_size_range(input: &str) -> Option<(Option<f64>, Option<f64>)> {
        let input = input.trim().to_lowercase();

        if let Some(stripped) = input.strip_prefix('>') {
            let size = Self::parse_size(stripped.trim())?;
            Some((Some(size), None))
        } else if let Some(stripped) = input.strip_prefix('<') {
            let size = Self::parse_size(stripped.trim())?;
            Some((None, Some(size)))
        } else if input.contains('-') {
            let parts: Vec<&str> = input.split('-').collect();
            if parts.len() == 2 {
                let min = Self::parse_size(parts[0].trim())?;
                let max = Self::parse_size(parts[1].trim())?;
                Some((Some(min), Some(max)))
            } else {
                None
            }
        } else {
            None
        }
    }

    fn parse_size(input: &str) -> Option<f64> {
        let input = input.trim().to_lowercase();

        let (number_str, multiplier) = if input.ends_with("tb") {
            (&input[..input.len() - 2], 1024.0 * 1024.0)
        } else if input.ends_with("gb") {
            (&input[..input.len() - 2], 1024.0)
        } else if input.ends_with("mb") {
            (&input[..input.len() - 2], 1.0)
        } else if input.ends_with("kb") {
            (&input[..input.len() - 2], 0.001)
        } else if input.ends_with('b') {
            (&input[..input.len() - 1], 0.000_001)
        } else {
            // Assume MB if no unit
            (input.as_str(), 1.0)
        };

        number_str.trim().parse::<f64>().ok().map(|n| n * multiplier)
    }

    fn delete_selected_filter(&mut self) {
        match self.filter_focus {
            FilterFocus::DateRange => {
                if self.selected_filter_index < self.filter_set.date_ranges.len() {
                    self.filter_set.date_ranges.remove(self.selected_filter_index);
                    if self.selected_filter_index > 0 && self.selected_filter_index >= self.filter_set.date_ranges.len()
                    {
                        self.selected_filter_index = self.filter_set.date_ranges.len().saturating_sub(1);
                    }
                }
            }
            FilterFocus::SizeRange => {
                if self.selected_filter_index < self.filter_set.size_ranges.len() {
                    self.filter_set.size_ranges.remove(self.selected_filter_index);
                    if self.selected_filter_index > 0 && self.selected_filter_index >= self.filter_set.size_ranges.len()
                    {
                        self.selected_filter_index = self.filter_set.size_ranges.len().saturating_sub(1);
                    }
                }
            }
            FilterFocus::RegexPattern => {
                if self.selected_filter_index < self.filter_set.regex_patterns.len() {
                    self.filter_set.regex_patterns.remove(self.selected_filter_index);
                    if self.selected_filter_index > 0
                        && self.selected_filter_index >= self.filter_set.regex_patterns.len()
                    {
                        self.selected_filter_index = self.filter_set.regex_patterns.len().saturating_sub(1);
                    }
                }
            }
            FilterFocus::MediaType => {}
        }
        self.success_message = Some("Filter deleted".to_string());
    }

    fn toggle_selected_filter(&mut self) {
        match self.filter_focus {
            FilterFocus::MediaType => {
                if let Some(mt) = self.filter_set.media_types.get_mut(self.selected_filter_index) {
                    mt.enabled = !mt.enabled;
                    let status = if mt.enabled { "enabled" } else { "disabled" };
                    self.success_message = Some(format!("{} {}", mt.media_type, status));
                }
            }
            FilterFocus::RegexPattern => {
                if let Some(rp) = self.filter_set.regex_patterns.get_mut(self.selected_filter_index) {
                    rp.enabled = !rp.enabled;
                    let status = if rp.enabled { "enabled" } else { "disabled" };
                    self.success_message = Some(format!("Pattern {status}"));
                }
            }
            _ => {}
        }
    }

    fn apply_filters(&mut self) {
        if self.filter_set.is_active {
            let filtered_count = self
                .cached_files
                .iter()
                .filter(|file| self.filter_set.matches_file(file))
                .count();

            self.success_message = Some(format!(
                "Filters applied: {} of {} files match",
                filtered_count,
                self.cached_files.len()
            ));
        } else {
            self.success_message = Some("Filters are inactive. Press 't' to toggle.".to_string());
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
