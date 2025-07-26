use chrono::{Duration, Local, NaiveDate, TimeZone};
use crossterm::event::{KeyCode, KeyEvent};
use visualvault_models::{FilterFocus, InputMode, filters::RegexTarget};

use super::{App, AppState};

impl App {
    pub fn handle_filter_keys(&mut self, key: KeyEvent) {
        if self.input_mode == InputMode::Editing {
            self.handle_filter_editing_mode(key);
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
                let max_index = self.get_max_filter_index();
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
                self.toggle_filter_active();
            }
            KeyCode::Enter => {
                self.apply_filters();
                self.state = AppState::Dashboard;
            }
            _ => {}
        }
    }

    fn handle_filter_editing_mode(&mut self, key: KeyEvent) {
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
    }

    pub fn update_filter_focus(&mut self) {
        self.filter_focus = match self.filter_tab {
            1 => FilterFocus::SizeRange,
            2 => FilterFocus::MediaType,
            3 => FilterFocus::RegexPattern,
            _ => FilterFocus::DateRange,
        };
    }

    fn get_max_filter_index(&self) -> usize {
        match self.filter_focus {
            FilterFocus::DateRange => self.filter_set.date_ranges.len(),
            FilterFocus::SizeRange => self.filter_set.size_ranges.len(),
            FilterFocus::MediaType => self.filter_set.media_types.len(),
            FilterFocus::RegexPattern => self.filter_set.regex_patterns.len(),
        }
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
            FilterFocus::MediaType => {
                // Media types are handled differently (toggle-based)
            }
        }
    }

    fn parse_date_range(
        input: &str,
    ) -> Option<(
        Option<chrono::DateTime<chrono::Local>>,
        Option<chrono::DateTime<chrono::Local>>,
    )> {
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
            FilterFocus::MediaType => {
                // Media types cannot be deleted, only toggled
            }
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
            FilterFocus::DateRange => {
                if let Some(dr) = self.filter_set.date_ranges.get_mut(self.selected_filter_index) {
                    self.success_message = Some(format!("Date range '{}'", dr.name));
                }
            }
            FilterFocus::SizeRange => {
                if let Some(sr) = self.filter_set.size_ranges.get_mut(self.selected_filter_index) {
                    self.success_message = Some(format!("Size range '{}'", sr.name));
                }
            }
        }
    }

    fn toggle_filter_active(&mut self) {
        self.filter_set.is_active = !self.filter_set.is_active;
        let status = if self.filter_set.is_active {
            "enabled"
        } else {
            "disabled"
        };
        self.success_message = Some(format!("Filters {status}"));
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
