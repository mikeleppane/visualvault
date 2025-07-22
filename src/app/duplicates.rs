use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::{app::state::DuplicateFocus, utils::format_bytes};

use super::{App, AppState};

impl App {
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
    pub async fn handle_duplicate_keys(&mut self, key: KeyEvent) -> Result<()> {
        // Handle bulk delete confirmation first
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
                self.exit_duplicate_review();
            }
            KeyCode::Char('s') => {
                self.start_duplicate_scan().await?;
            }
            KeyCode::Up => {
                self.move_duplicate_selection_up();
            }
            KeyCode::Down => {
                self.move_duplicate_selection_down();
            }
            KeyCode::Left => {
                self.switch_to_group_list();
            }
            KeyCode::Right => {
                self.switch_to_file_list();
            }
            KeyCode::Char(' ') => {
                self.toggle_file_selection();
            }
            KeyCode::Char('a') => {
                self.select_all_except_first();
            }
            KeyCode::Char('d') => {
                self.handle_delete_key().await?;
            }
            KeyCode::Char('D') => {
                self.initiate_bulk_delete();
            }
            _ => {}
        }
        Ok(())
    }

    fn exit_duplicate_review(&mut self) {
        self.state = AppState::Dashboard;
        self.selected_duplicate_items.clear();
    }

    fn move_duplicate_selection_up(&mut self) {
        match self.duplicate_focus {
            DuplicateFocus::GroupList => {
                if self.selected_duplicate_group > 0 {
                    self.selected_duplicate_group -= 1;
                    self.duplicate_list_state.select(Some(self.selected_duplicate_group));
                    self.selected_duplicate_items.clear();
                }
            }
            DuplicateFocus::FileList => {
                if let Some(stats) = &self.duplicate_stats {
                    if stats.groups.get(self.selected_duplicate_group).is_some() && self.selected_file_in_group > 0 {
                        self.selected_file_in_group -= 1;
                    }
                }
            }
        }
    }

    fn move_duplicate_selection_down(&mut self) {
        match self.duplicate_focus {
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
        }
    }

    fn switch_to_group_list(&mut self) {
        self.duplicate_focus = DuplicateFocus::GroupList;
        self.selected_file_in_group = 0;
    }

    fn switch_to_file_list(&mut self) {
        if let Some(stats) = &self.duplicate_stats {
            if !stats.groups.is_empty() {
                self.duplicate_focus = DuplicateFocus::FileList;
                self.selected_file_in_group = 0;
            }
        }
    }

    fn toggle_file_selection(&mut self) {
        if self.duplicate_focus == DuplicateFocus::FileList {
            if self.selected_duplicate_items.contains(&self.selected_file_in_group) {
                self.selected_duplicate_items.remove(&self.selected_file_in_group);
            } else {
                self.selected_duplicate_items.insert(self.selected_file_in_group);
            }
        }
    }

    fn select_all_except_first(&mut self) {
        // Select all but the first file in the current group
        if let Some(stats) = &self.duplicate_stats {
            if let Some(group) = stats.groups.get(self.selected_duplicate_group) {
                self.selected_duplicate_items.clear();
                for i in 1..group.files.len() {
                    self.selected_duplicate_items.insert(i);
                }
                self.success_message = Some(format!(
                    "Selected {} duplicate files (keeping the first as original)",
                    self.selected_duplicate_items.len()
                ));
            }
        }
    }

    async fn handle_delete_key(&mut self) -> Result<()> {
        // Delete selected files in current group
        if self.selected_duplicate_items.is_empty() {
            self.error_message = Some("No files selected for deletion".to_string());
        } else {
            self.delete_selected_duplicates().await?;
        }
        Ok(())
    }

    fn initiate_bulk_delete(&mut self) {
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
}
