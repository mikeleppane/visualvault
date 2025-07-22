use super::App;

impl App {
    pub fn next_tab(&mut self) {
        let max_tabs = self.get_tab_count();
        self.selected_tab = (self.selected_tab + 1) % max_tabs;
        self.selected_setting = 0;
    }

    pub fn previous_tab(&mut self) {
        let max_tabs = self.get_tab_count();
        if self.selected_tab > 0 {
            self.selected_tab -= 1;
        } else {
            self.selected_tab = max_tabs - 1;
        }
        self.selected_setting = 0;
    }

    pub fn move_selection_up(&mut self) {
        if self.selected_file_index > 0 {
            self.selected_file_index -= 1;
            if self.selected_file_index < self.scroll_offset {
                self.scroll_offset = self.selected_file_index;
            }
        }
    }

    pub fn move_selection_down(&mut self) {
        let file_count = self.cached_files.len();
        if self.selected_file_index < file_count.saturating_sub(1) {
            self.selected_file_index += 1;
            if self.selected_file_index >= self.scroll_offset + 20 {
                self.scroll_offset = self.selected_file_index - 19;
            }
        }
    }

    pub fn page_up(&mut self) {
        if self.selected_file_index >= 10 {
            self.selected_file_index -= 10;
        } else {
            self.selected_file_index = 0;
        }
        if self.selected_file_index < self.scroll_offset {
            self.scroll_offset = self.selected_file_index;
        }
    }

    pub fn page_down(&mut self) {
        let file_count = self.cached_files.len();
        self.selected_file_index = std::cmp::min(self.selected_file_index + 10, file_count.saturating_sub(1));
        if self.selected_file_index >= self.scroll_offset + 20 {
            self.scroll_offset = self.selected_file_index.saturating_sub(19);
        }
    }

    pub fn handle_search_keys(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match self.input_mode {
            super::InputMode::Normal => match key.code {
                KeyCode::Enter | KeyCode::Char('/') => {
                    self.input_mode = super::InputMode::Insert;
                }
                KeyCode::Esc => {
                    self.state = super::AppState::Dashboard;
                    self.search_input.clear();
                    self.search_results.clear();
                    self.selected_file_index = 0;
                    self.scroll_offset = 0;
                }
                KeyCode::Up => {
                    if !self.search_results.is_empty() && self.selected_file_index > 0 {
                        self.selected_file_index -= 1;
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
                        if self.selected_file_index >= self.scroll_offset + 20 {
                            self.scroll_offset = self.selected_file_index - 19;
                        }
                    }
                }
                _ => {}
            },
            super::InputMode::Insert => match key.code {
                KeyCode::Enter => {
                    self.perform_search();
                    self.input_mode = super::InputMode::Normal;
                }
                KeyCode::Esc => {
                    self.input_mode = super::InputMode::Normal;
                }
                KeyCode::Char(c) => {
                    self.search_input.push(c);
                    self.perform_search();
                }
                KeyCode::Backspace => {
                    self.search_input.pop();
                    self.perform_search();
                }
                KeyCode::Delete => {
                    self.search_input.clear();
                    self.search_results.clear();
                    self.selected_file_index = 0;
                    self.scroll_offset = 0;
                }
                _ => {}
            },
            super::InputMode::Editing => {
                if key.code == KeyCode::Esc {
                    self.input_mode = super::InputMode::Normal;
                } else {
                    self.input_mode = super::InputMode::Insert;
                    self.handle_search_keys(key);
                }
            }
        }
    }

    pub fn perform_search(&mut self) {
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
                file.name.to_lowercase().contains(&search_term)
                    || file.path.to_string_lossy().to_lowercase().contains(&search_term)
            })
            .cloned()
            .collect();
        self.selected_file_index = 0;
        self.scroll_offset = 0;
    }
}
