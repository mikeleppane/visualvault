mod actions;
mod duplicates;
mod filters;
mod handlers;
mod navigation;
pub mod state;

pub use state::{App, AppState, EditingField, InputMode, OrganizeResult, ScanResult};

use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;

impl App {
    /// Creates a new App instance with default settings and components.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Settings cannot be loaded from the configuration file
    /// - Scanner cache initialization fails
    /// - Any other component initialization fails
    pub async fn new() -> Result<Self> {
        state::App::init().await
    }

    /// Handles keyboard input events and updates application state accordingly.
    ///
    /// # Errors
    /// Returns an error if the key handling operation fails, such as when
    /// updating settings, performing file operations, or state transitions.
    pub async fn on_key(&mut self, key: KeyEvent) -> Result<()> {
        self.clear_messages();

        match self.state {
            AppState::Search => {
                self.handle_search_keys(key);
                Ok(())
            }
            AppState::Filters => {
                self.handle_filter_keys(key);
                Ok(())
            }
            AppState::FileDetails(_) => {
                self.handle_file_details_keys(key);
                Ok(())
            }
            AppState::DuplicateReview => self.handle_duplicate_keys(key).await,
            _ => self.handle_global_keys(key).await,
        }
    }

    /// Handles periodic updates and state transitions.
    ///
    /// # Errors
    /// Returns an error if statistics update fails.
    pub async fn on_tick(&mut self) -> Result<()> {
        self.update_progress().await?;
        self.update_folder_stats_if_needed();
        self.check_operation_completion().await?;
        Ok(())
    }
}
