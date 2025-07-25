use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{app::EditingField, config::Settings, models::FileType};

use super::{App, AppState, InputMode};
use std::path::PathBuf;

impl App {
    /// Handles global keyboard input events across all application states.
    ///
    /// # Errors
    ///
    /// Returns an error if handling keys in normal or insert mode fails,
    /// typically due to file system operations or configuration updates.
    pub async fn handle_global_keys(&mut self, key: KeyEvent) -> Result<()> {
        if self.show_help {
            return {
                self.handle_help_keys(key);
                Ok(())
            };
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
                return self.handle_undo().await;
            }
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                return self.handle_redo().await;
            }
            _ => {}
        }

        match key.code {
            KeyCode::F(1) | KeyCode::Char('?') => {
                self.show_help = !self.show_help;
                self.help_scroll = 0;
                Ok(())
            }
            _ => match self.input_mode {
                InputMode::Normal => self.handle_normal_mode(key).await,
                InputMode::Insert | InputMode::Editing => self.handle_insert_mode(key).await,
            },
        }
    }

    fn handle_help_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                if self.help_scroll > 0 {
                    self.help_scroll -= 1;
                }
            }
            KeyCode::Down => {
                let content_lines: usize = 70;
                let visible_lines: usize = 35;
                let max_scroll = content_lines.saturating_sub(visible_lines);
                if self.help_scroll < max_scroll {
                    self.help_scroll += 1;
                }
            }
            KeyCode::PageUp => {
                self.help_scroll = self.help_scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                let content_lines: usize = 70;
                let visible_lines: usize = 35;
                let max_scroll = content_lines.saturating_sub(visible_lines);
                self.help_scroll = (self.help_scroll + 10).min(max_scroll);
            }
            KeyCode::Home => {
                self.help_scroll = 0;
            }
            KeyCode::End => {
                let content_lines: usize = 70;
                let visible_lines: usize = 35;
                self.help_scroll = content_lines.saturating_sub(visible_lines);
            }
            _ => {
                self.show_help = false;
                self.help_scroll = 0;
            }
        }
    }

    /// Handles keyboard input events when viewing file details.
    ///
    /// # Errors
    ///
    /// This function currently does not return any errors, but returns a `Result`
    /// for consistency with other key handling methods.
    pub fn handle_file_details_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.state = AppState::Dashboard;
            }
            _ => {}
        }
    }

    async fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => {
                if self.state == AppState::Dashboard && [0, 1, 2, 3].contains(&self.selected_tab) {
                    self.should_quit = true;
                }
            }
            KeyCode::Esc => match self.state {
                AppState::Dashboard => {
                    if [0, 1, 2, 3].contains(&self.selected_tab) {
                        self.should_quit = true;
                    }
                }
                _ => {
                    self.state = AppState::Dashboard;
                }
            },
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.previous_tab(),
            KeyCode::Char('d') => self.state = AppState::Dashboard,
            KeyCode::Char('s') => {
                if self.state != AppState::DuplicateReview {
                    self.state = AppState::Settings;
                    self.update_settings_cache().await?;
                }
            }
            KeyCode::Char('r') => self.start_scan().await?,
            KeyCode::Char('o') => self.start_organize().await?,
            KeyCode::Char('u') => self.update_folder_stats().await?,
            KeyCode::Char('f' | '/') => {
                self.state = AppState::Search;
                self.search_input.clear();
                self.search_results.clear();
                self.selected_file_index = 0;
                self.scroll_offset = 0;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Char('D') => self.state = AppState::DuplicateReview,
            KeyCode::Char('F') => {
                self.state = AppState::Filters;
                self.filter_tab = 0;
                self.selected_filter_index = 0;
                self.update_filter_focus();
                self.input_mode = InputMode::Normal;
            }
            _ => match self.state {
                AppState::Settings => self.handle_settings_keys(key).await?,
                AppState::Dashboard => self.handle_dashboard_keys(key).await?,
                _ => {}
            },
        }
        Ok(())
    }

    async fn handle_insert_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.editing_field = None;
                self.input_buffer.clear();
            }
            KeyCode::Enter => {
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
}

impl App {
    /// Handles keyboard input events when in settings mode.
    ///
    /// # Errors
    ///
    /// Returns an error if saving settings fails, typically due to file system
    /// operations or configuration file write permissions.
    pub async fn handle_settings_keys(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('S' | 's') => {
                self.save_settings().await?;
            }
            KeyCode::Char('R' | 'r') => {
                self.settings_cache = Settings::default();
                self.success_message = Some("Settings reset to defaults (not saved)".to_string());
            }
            KeyCode::Enter => {
                self.handle_settings_enter();
            }
            KeyCode::Esc => {
                if self.input_mode == InputMode::Insert {
                    self.input_mode = InputMode::Normal;
                    self.editing_field = None;
                    self.input_buffer.clear();
                } else {
                    self.state = AppState::Dashboard;
                }
            }
            KeyCode::Char(' ') => {
                self.toggle_setting();
            }
            KeyCode::Up => {
                if self.selected_setting > 0 {
                    self.selected_setting -= 1;
                }
            }
            KeyCode::Down => {
                let max_setting = match self.selected_tab {
                    0 | 2 => 5,
                    1 => 7,
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

    fn handle_settings_enter(&mut self) {
        match self.selected_setting {
            0 if self.selected_tab == 0 => {
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
                    self.settings_cache.destination_folder = if self.input_buffer.is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(&self.input_buffer))
                    };
                    self.input_mode = InputMode::Normal;
                    self.editing_field = None;
                }
            }
            _ => {}
        }
    }

    fn toggle_setting(&mut self) {
        match (self.selected_tab, self.selected_setting) {
            (0, 2) => self.settings_cache.recurse_subfolders = !self.settings_cache.recurse_subfolders,
            (0, 3) => self.settings_cache.verbose_output = !self.settings_cache.verbose_output,
            (0, 4) => self.settings_cache.undo_enabled = !self.settings_cache.undo_enabled,
            (1, s) if s <= 2 => {
                self.settings_cache.organize_by = match s {
                    1 => "monthly",
                    2 => "type",
                    _ => "yearly", // fallback
                }
                .to_string();
            }
            (1, 3) => self.settings_cache.separate_videos = !self.settings_cache.separate_videos,
            (1, 4) => self.settings_cache.keep_original_structure = !self.settings_cache.keep_original_structure,
            (1, 5) => self.settings_cache.rename_duplicates = !self.settings_cache.rename_duplicates,
            (1, 6) => self.settings_cache.lowercase_extensions = !self.settings_cache.lowercase_extensions,
            (2, 2) => self.settings_cache.enable_cache = !self.settings_cache.enable_cache,
            (2, 3) => self.settings_cache.parallel_processing = !self.settings_cache.parallel_processing,
            (2, 4) => self.settings_cache.skip_hidden_files = !self.settings_cache.skip_hidden_files,
            (2, 5) => self.settings_cache.optimize_for_ssd = !self.settings_cache.optimize_for_ssd,
            _ => {}
        }
    }

    /// Saves the current settings cache to the configuration file.
    ///
    /// # Errors
    ///
    /// Returns an error if the settings file cannot be written to disk,
    /// typically due to file system permissions or I/O issues.
    pub async fn save_settings(&mut self) -> Result<()> {
        let mut settings = self.settings.write().await;
        *settings = self.settings_cache.clone();
        settings.save()?;
        drop(settings);
        self.success_message = Some("Settings saved successfully!".to_string());
        Ok(())
    }

    /// Applies the edited value from the input buffer to the specified setting field.
    ///
    /// # Errors
    ///
    /// Returns an error if the settings cannot be updated, typically due to
    /// invalid input values or file system issues when updating the configuration.
    pub async fn apply_edited_value(&mut self, field: EditingField) -> Result<()> {
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
}

impl App {
    /// Handles keyboard input events when in dashboard mode.
    ///
    /// # Errors
    ///
    /// Returns an error if loading image metadata fails or if there are issues
    /// with file system operations during navigation.
    pub async fn handle_dashboard_keys(&mut self, key: KeyEvent) -> Result<()> {
        if self.selected_tab == 1 {
            match key.code {
                KeyCode::Up => self.move_selection_up(),
                KeyCode::Down => self.move_selection_down(),
                KeyCode::PageUp => self.page_up(),
                KeyCode::PageDown => self.page_down(),
                KeyCode::Home => {
                    self.selected_file_index = 0;
                    self.scroll_offset = 0;
                }
                KeyCode::End => {
                    let file_count = self.cached_files.len();
                    if file_count > 0 {
                        self.selected_file_index = file_count - 1;
                        if self.selected_file_index >= 20 {
                            self.scroll_offset = self.selected_file_index - 19;
                        }
                    }
                }
                KeyCode::Enter => {
                    if !self.cached_files.is_empty() && self.selected_file_index < self.cached_files.len() {
                        let needs_metadata = self
                            .cached_files
                            .get(self.selected_file_index)
                            .is_some_and(|f| f.file_type == FileType::Image && f.metadata.is_none());

                        if needs_metadata {
                            self.success_message = Some("Loading image metadata...".to_string());

                            let path = self.cached_files.get(self.selected_file_index).map(|f| f.path.clone());

                            if let Some(path) = path {
                                match self.load_image_metadata(&path).await {
                                    Ok(metadata) => {
                                        if let Some(file) = self.cached_files.get_mut(self.selected_file_index) {
                                            // Replace the Arc with a new Arc containing the updated MediaFile
                                            let mut updated_file = (**file).clone();
                                            updated_file.metadata = Some(metadata);
                                            *file = std::sync::Arc::new(updated_file);
                                        }
                                        self.success_message = None;
                                    }
                                    Err(e) => {
                                        tracing::warn!("Failed to load metadata for {}: {}", path.display(), e);
                                        self.error_message = Some(format!("Metadata unavailable: {e}"));
                                    }
                                }
                            }
                        }

                        if self.success_message == Some("Loading image metadata...".to_string()) {
                            self.success_message = None;
                        }

                        self.state = AppState::FileDetails(self.selected_file_index);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Handles the undo operation
    ///
    /// # Errors
    ///
    /// Returns an error if the undo operation fails
    pub async fn handle_undo(&mut self) -> Result<()> {
        if let Some(message) = self.organizer.undo_manager().undo().await? {
            self.last_undo_result = Some(format!("✓ {message}"));
            self.success_message = Some(format!("Undo successful: {message}"));
            self.success_message = Some(format!("✓ Undone: {message}"));
        } else {
            self.success_message = Some("Nothing to undo".to_string());
            self.error_message = Some("Nothing to undo".to_string());
        }
        Ok(())
    }

    /// Handles the redo operation
    ///
    /// # Errors
    ///
    /// Returns an error if the redo operation fails
    pub async fn handle_redo(&mut self) -> Result<()> {
        if let Some(message) = self.organizer.undo_manager().redo().await? {
            self.last_undo_result = Some(format!("↻ {message}"));
            self.success_message = Some(format!("Redo successful: {message}"));
            self.success_message = Some(format!("↻ Redone: {message}"));
        } else {
            self.success_message = Some("Nothing to redo".to_string());
            self.error_message = Some("Nothing to redo".to_string());
        }
        Ok(())
    }

    /* /// Updates the undo history UI
    ///
    /// # Errors
    ///
    /// Returns an error if fetching history fails
    pub async fn update_undo_history(&mut self) -> Result<()> {
        let operations = self.organizer.undo_manager().get_history().await;
        self.undo_history_ui.update_operations(operations);
        Ok(())
    } */
}
