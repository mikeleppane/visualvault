use chrono::{DateTime, Local};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::models::MediaFile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterSet {
    pub date_ranges: Vec<DateRange>,
    pub size_ranges: Vec<SizeRange>,
    pub media_types: Vec<MediaTypeFilter>,
    pub regex_patterns: Vec<RegexPattern>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub from: Option<DateTime<Local>>,
    pub to: Option<DateTime<Local>>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeRange {
    pub min_bytes: Option<u64>,
    pub max_bytes: Option<u64>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaTypeFilter {
    pub media_type: MediaType,
    pub extensions: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexPattern {
    pub pattern: String,
    pub target: RegexTarget,
    pub case_sensitive: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    Image,
    Video,
    Audio,
    Document,
    Archive,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegexTarget {
    FileName,
    FilePath,
    Extension,
}

impl Default for FilterSet {
    fn default() -> Self {
        Self {
            date_ranges: vec![],
            size_ranges: vec![],
            media_types: Self::default_media_types(),
            regex_patterns: vec![],
            is_active: false,
        }
    }
}

impl FilterSet {
    pub fn new() -> Self {
        Self::default()
    }

    fn default_media_types() -> Vec<MediaTypeFilter> {
        vec![
            MediaTypeFilter {
                media_type: MediaType::Image,
                extensions: vec!["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "svg", "ico", "heic"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                enabled: true,
            },
            MediaTypeFilter {
                media_type: MediaType::Video,
                extensions: vec!["mp4", "avi", "mkv", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                enabled: true,
            },
            MediaTypeFilter {
                media_type: MediaType::Audio,
                extensions: vec!["mp3", "wav", "flac", "aac", "ogg", "wma", "m4a", "opus"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                enabled: false,
            },
            MediaTypeFilter {
                media_type: MediaType::Document,
                extensions: vec!["pdf", "doc", "docx", "txt", "odt", "rtf"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                enabled: false,
            },
            MediaTypeFilter {
                media_type: MediaType::Archive,
                extensions: vec!["zip", "rar", "7z", "tar", "gz", "bz2", "xz"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                enabled: false,
            },
        ]
    }

    pub fn matches_file(&self, file: &MediaFile) -> bool {
        if !self.is_active {
            return true;
        }

        // Check date ranges
        if !self.date_ranges.is_empty() {
            let file_date = file.modified;
            let matches_date = self.date_ranges.iter().any(|range| {
                let after_from = range.from.is_none_or(|from| file_date >= from);
                let before_to = range.to.is_none_or(|to| file_date <= to);
                after_from && before_to
            });
            if !matches_date {
                return false;
            }
        }

        // Check size ranges
        if !self.size_ranges.is_empty() {
            let matches_size = self.size_ranges.iter().any(|range| {
                let above_min = range.min_bytes.is_none_or(|min| file.size >= min);
                let below_max = range.max_bytes.is_none_or(|max| file.size <= max);
                above_min && below_max
            });
            if !matches_size {
                return false;
            }
        }

        // Check media types
        let enabled_types: Vec<_> = self.media_types.iter().filter(|mt| mt.enabled).collect();
        if !enabled_types.is_empty() {
            let file_ext = file
                .path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_lowercase();

            let matches_type = enabled_types
                .iter()
                .any(|mt| mt.extensions.iter().any(|ext| ext.to_lowercase() == file_ext));
            if !matches_type {
                return false;
            }
        }

        // Check regex patterns
        for pattern in &self.regex_patterns {
            if !pattern.enabled {
                continue;
            }

            let Ok(regex) = Regex::new(&pattern.pattern) else {
                continue;
            };

            let text = match pattern.target {
                RegexTarget::FileName => file.name.as_str(),
                RegexTarget::FilePath => file.path.to_str().unwrap_or(""),
                RegexTarget::Extension => file.path.extension().and_then(|ext| ext.to_str()).unwrap_or(""),
            };

            let matches = if pattern.case_sensitive {
                regex.is_match(text)
            } else {
                regex.is_match(&text.to_lowercase())
            };

            if !matches {
                return false;
            }
        }

        true
    }

    pub fn clear_all(&mut self) {
        self.date_ranges.clear();
        self.size_ranges.clear();
        self.regex_patterns.clear();
        // Reset media types to default
        self.media_types = Self::default_media_types();
        self.is_active = false;
    }

    pub fn active_filter_count(&self) -> usize {
        let mut count = 0;
        count += self.date_ranges.len();
        count += self.size_ranges.len();
        count += self.media_types.iter().filter(|mt| mt.enabled).count();
        count += self.regex_patterns.iter().filter(|rp| rp.enabled).count();
        count
    }
}

impl fmt::Display for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaType::Image => write!(f, "Images"),
            MediaType::Video => write!(f, "Videos"),
            MediaType::Audio => write!(f, "Audio"),
            MediaType::Document => write!(f, "Documents"),
            MediaType::Archive => write!(f, "Archives"),
            MediaType::Other => write!(f, "Other"),
        }
    }
}

impl fmt::Display for RegexTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegexTarget::FileName => write!(f, "File Name"),
            RegexTarget::FilePath => write!(f, "Full Path"),
            RegexTarget::Extension => write!(f, "Extension"),
        }
    }
}

// Helper functions for creating common filters
impl FilterSet {
    pub fn add_date_range(&mut self, name: String, from: Option<DateTime<Local>>, to: Option<DateTime<Local>>) {
        self.date_ranges.push(DateRange { from, to, name });
        self.is_active = true;
    }

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    pub fn add_size_range(&mut self, name: String, min_mb: Option<f64>, max_mb: Option<f64>) {
        let min_bytes = min_mb.map(|mb| (mb * 1024.0 * 1024.0) as u64);
        let max_bytes = max_mb.map(|mb| (mb * 1024.0 * 1024.0) as u64);
        self.size_ranges.push(SizeRange {
            min_bytes,
            max_bytes,
            name,
        });
        self.is_active = true;
    }

    pub fn add_regex_pattern(&mut self, pattern: String, target: RegexTarget, case_sensitive: bool) {
        self.regex_patterns.push(RegexPattern {
            pattern,
            target,
            case_sensitive,
            enabled: true,
        });
        self.is_active = true;
    }
}
