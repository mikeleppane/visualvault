use crate::MediaFile;
use chrono::{DateTime, Local};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{fmt, sync::Arc};

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
    #[must_use]
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

    #[must_use]
    pub fn matches_file(&self, file: &MediaFile) -> bool {
        if !self.is_active {
            return true;
        }

        self.matches_date(file)
            && self.matches_size(file)
            && self.matches_media_type(file)
            && self.matches_regex_patterns(file)
    }

    fn matches_date(&self, file: &MediaFile) -> bool {
        if self.date_ranges.is_empty() {
            return true;
        }

        let file_date = file.modified;
        self.date_ranges.iter().any(|range| {
            let after_from = range.from.is_none_or(|from| file_date >= from);
            let before_to = range.to.is_none_or(|to| file_date <= to);
            after_from && before_to
        })
    }

    fn matches_size(&self, file: &MediaFile) -> bool {
        if self.size_ranges.is_empty() {
            return true;
        }

        self.size_ranges.iter().any(|range| {
            let above_min = range.min_bytes.is_none_or(|min| file.size >= min);
            let below_max = range.max_bytes.is_none_or(|max| file.size <= max);
            above_min && below_max
        })
    }

    fn matches_media_type(&self, file: &MediaFile) -> bool {
        let enabled_types: Vec<_> = self.media_types.iter().filter(|mt| mt.enabled).collect();

        if enabled_types.is_empty() {
            return true;
        }

        let file_ext = Self::get_file_extension(file);

        enabled_types
            .iter()
            .any(|mt| mt.extensions.iter().any(|ext| ext.to_lowercase() == file_ext))
    }

    fn get_file_extension(file: &MediaFile) -> String {
        file.path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase()
    }

    fn matches_regex_patterns(&self, file: &MediaFile) -> bool {
        self.regex_patterns
            .iter()
            .filter(|pattern| pattern.enabled)
            .all(|pattern| Self::matches_single_regex(pattern, file))
    }

    fn matches_single_regex(pattern: &RegexPattern, file: &MediaFile) -> bool {
        let regex_result = Self::build_regex(pattern);

        let Ok(regex) = regex_result else {
            return true; // Skip invalid regex patterns
        };

        let text = Self::get_regex_target_text(pattern, file);
        regex.is_match(&text)
    }

    fn build_regex(pattern: &RegexPattern) -> Result<Regex, regex::Error> {
        if pattern.case_sensitive {
            Regex::new(&pattern.pattern)
        } else {
            Regex::new(&format!("(?i){}", pattern.pattern))
        }
    }

    fn get_regex_target_text(pattern: &RegexPattern, file: &MediaFile) -> Arc<str> {
        match pattern.target {
            RegexTarget::FileName => Arc::clone(&file.name),
            RegexTarget::FilePath => file.path.to_str().unwrap_or("").into(),
            RegexTarget::Extension => file.path.extension().and_then(|ext| ext.to_str()).unwrap_or("").into(),
        }
    }

    pub fn clear_all(&mut self) {
        self.date_ranges.clear();
        self.size_ranges.clear();
        self.regex_patterns.clear();
        // Reset media types to default
        self.media_types = Self::default_media_types();
        self.is_active = false;
    }

    #[must_use]
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

// ... existing code ...

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::float_cmp)] // For comparing floats in tests
    #![allow(clippy::panic)]
    #![allow(clippy::cognitive_complexity)] // For complex filter matching logic
    use super::*;
    use chrono::Local;
    use std::path::PathBuf;

    fn create_test_media_file() -> MediaFile {
        MediaFile {
            path: PathBuf::from("/test/path/image.jpg"),
            name: "image.jpg".into(),
            extension: "jpg".into(),
            file_type: crate::FileType::Image,
            size: 1024 * 1024 * 5, // 5MB
            created: Local::now(),
            modified: Local::now(),
            hash: None,
            metadata: None,
        }
    }

    #[test]
    fn test_filter_set_default() {
        let filter_set = FilterSet::default();

        assert!(filter_set.date_ranges.is_empty());
        assert!(filter_set.size_ranges.is_empty());
        assert!(!filter_set.media_types.is_empty());
        assert!(filter_set.regex_patterns.is_empty());
        assert!(!filter_set.is_active);

        // Check default media types
        assert_eq!(filter_set.media_types.len(), 5);

        // Images and Videos should be enabled by default
        assert!(filter_set.media_types[0].enabled); // Images
        assert!(filter_set.media_types[1].enabled); // Videos
        assert!(!filter_set.media_types[2].enabled); // Audio
        assert!(!filter_set.media_types[3].enabled); // Documents
        assert!(!filter_set.media_types[4].enabled); // Archives
    }

    #[test]
    fn test_filter_set_new() {
        let filter_set = FilterSet::new();
        assert_eq!(filter_set.date_ranges.len(), 0);
        assert_eq!(filter_set.active_filter_count(), 2); // Images and Videos enabled by default
    }

    #[test]
    fn test_matches_file_inactive_filter() {
        let filter_set = FilterSet::new();
        let file = create_test_media_file();

        // When filter is inactive, all files should match
        assert!(filter_set.matches_file(&file));
    }

    #[test]
    fn test_matches_file_date_range() {
        let mut filter_set = FilterSet::new();
        let mut file = create_test_media_file();

        let yesterday = Local::now() - chrono::Duration::days(1);
        let tomorrow = Local::now() + chrono::Duration::days(1);
        let last_week = Local::now() - chrono::Duration::days(7);

        // Test file within date range
        filter_set.add_date_range("Recent".to_string(), Some(yesterday), Some(tomorrow));
        assert!(filter_set.matches_file(&file));

        // Test file outside date range
        file.modified = last_week;
        assert!(!filter_set.matches_file(&file));

        // Test open-ended date range (no end date)
        filter_set.date_ranges.clear();
        filter_set.add_date_range(
            "After last month".to_string(),
            Some(Local::now() - chrono::Duration::days(30)),
            None,
        );
        file.modified = Local::now();
        assert!(filter_set.matches_file(&file));

        // Test open-ended date range (no start date)
        filter_set.date_ranges.clear();
        filter_set.add_date_range("Before today".to_string(), None, Some(Local::now()));
        assert!(filter_set.matches_file(&file));
    }

    #[test]
    fn test_matches_file_size_range() {
        let mut filter_set = FilterSet::new();
        let mut file = create_test_media_file();

        // Test file within size range (1MB - 10MB)
        filter_set.add_size_range("Medium files".to_string(), Some(1.0), Some(10.0));
        assert!(filter_set.matches_file(&file)); // 5MB file

        // Test file outside size range
        file.size = 1024 * 1024 * 20; // 20MB
        assert!(!filter_set.matches_file(&file));

        // Test open-ended size range (minimum only)
        filter_set.size_ranges.clear();
        filter_set.add_size_range("Large files".to_string(), Some(15.0), None);
        assert!(filter_set.matches_file(&file)); // 20MB > 15MB

        // Test open-ended size range (maximum only)
        filter_set.size_ranges.clear();
        filter_set.add_size_range("Small files".to_string(), None, Some(30.0));
        assert!(filter_set.matches_file(&file)); // 20MB < 30MB
    }

    #[test]
    fn test_matches_file_media_types() {
        let mut filter_set = FilterSet::new();
        let mut file = create_test_media_file();

        filter_set.is_active = true; // Activate filters

        // JPG should match enabled Image type
        assert!(filter_set.matches_file(&file));

        // Disable Images, enable only Videos
        filter_set.media_types[0].enabled = false; // Disable Images
        filter_set.media_types[1].enabled = true; // Enable Videos
        assert!(!filter_set.matches_file(&file));

        // Change file to video
        file.path = PathBuf::from("/test/video.mp4");
        file.extension = "mp4".into();
        assert!(filter_set.matches_file(&file));

        // Test case-insensitive extension matching
        file.extension = "MP4".into();
        assert!(filter_set.matches_file(&file));
    }

    #[test]
    fn test_matches_file_regex_patterns() {
        let mut filter_set = FilterSet::new();
        let file = create_test_media_file();

        filter_set.is_active = true; // Activate filters

        // Test filename regex
        filter_set.add_regex_pattern(r"^image.*\.jpg$".to_string(), RegexTarget::FileName, false);
        assert!(filter_set.matches_file(&file));

        // Test case-sensitive regex
        filter_set.regex_patterns.clear();
        filter_set.add_regex_pattern(r"IMAGE".to_string(), RegexTarget::FileName, true);
        assert!(!filter_set.matches_file(&file)); // "image.jpg" doesn't contain "IMAGE"

        // Test case-insensitive regex
        filter_set.regex_patterns.clear();
        filter_set.add_regex_pattern(r"IMAGE".to_string(), RegexTarget::FileName, false);
        assert!(filter_set.matches_file(&file));

        // Test file path regex
        filter_set.regex_patterns.clear();
        filter_set.add_regex_pattern(r"/test/path/".to_string(), RegexTarget::FilePath, false);
        assert!(filter_set.matches_file(&file));

        // Test extension regex
        filter_set.regex_patterns.clear();
        filter_set.add_regex_pattern(r"^jpg$".to_string(), RegexTarget::Extension, false);
        assert!(filter_set.matches_file(&file));

        // Test disabled pattern
        filter_set.regex_patterns[0].enabled = false;
        assert!(filter_set.matches_file(&file)); // Should match when pattern is disabled
    }

    #[test]
    fn test_matches_file_combined_filters() {
        let mut filter_set = FilterSet::new();
        let file = create_test_media_file();

        // Add multiple filters - file must match ALL active filters
        filter_set.add_date_range(
            "Recent".to_string(),
            Some(Local::now() - chrono::Duration::days(1)),
            Some(Local::now() + chrono::Duration::days(1)),
        );
        filter_set.add_size_range("Medium".to_string(), Some(1.0), Some(10.0));
        filter_set.add_regex_pattern(r"\.jpg$".to_string(), RegexTarget::FileName, false);

        // File matches all filters
        assert!(filter_set.matches_file(&file));

        // Change one filter to not match
        filter_set.size_ranges[0].max_bytes = Some(1024 * 1024); // 1MB max
        assert!(!filter_set.matches_file(&file)); // 5MB file doesn't match
    }

    #[test]
    fn test_clear_all() {
        let mut filter_set = FilterSet::new();

        // Add various filters
        filter_set.add_date_range("Test".to_string(), None, None);
        filter_set.add_size_range("Test".to_string(), Some(1.0), None);
        filter_set.add_regex_pattern("test".to_string(), RegexTarget::FileName, false);

        assert!(filter_set.is_active);
        assert!(!filter_set.date_ranges.is_empty());
        assert!(!filter_set.size_ranges.is_empty());
        assert!(!filter_set.regex_patterns.is_empty());

        // Clear all filters
        filter_set.clear_all();

        assert!(!filter_set.is_active);
        assert!(filter_set.date_ranges.is_empty());
        assert!(filter_set.size_ranges.is_empty());
        assert!(filter_set.regex_patterns.is_empty());

        // Media types should be reset to defaults
        assert_eq!(filter_set.media_types.len(), 5);
        assert!(filter_set.media_types[0].enabled); // Images
        assert!(filter_set.media_types[1].enabled); // Videos
    }

    #[test]
    fn test_active_filter_count() {
        let mut filter_set = FilterSet::new();

        // Default: 2 media types enabled
        assert_eq!(filter_set.active_filter_count(), 2);

        // Add filters
        filter_set.add_date_range("Test".to_string(), None, None);
        assert_eq!(filter_set.active_filter_count(), 3);

        filter_set.add_size_range("Test".to_string(), Some(1.0), None);
        assert_eq!(filter_set.active_filter_count(), 4);

        filter_set.add_regex_pattern("test".to_string(), RegexTarget::FileName, false);
        assert_eq!(filter_set.active_filter_count(), 5);

        // Disable a media type
        filter_set.media_types[0].enabled = false;
        assert_eq!(filter_set.active_filter_count(), 4);

        // Disable a regex pattern
        filter_set.regex_patterns[0].enabled = false;
        assert_eq!(filter_set.active_filter_count(), 3);
    }

    #[test]
    fn test_media_type_display() {
        assert_eq!(MediaType::Image.to_string(), "Images");
        assert_eq!(MediaType::Video.to_string(), "Videos");
        assert_eq!(MediaType::Audio.to_string(), "Audio");
        assert_eq!(MediaType::Document.to_string(), "Documents");
        assert_eq!(MediaType::Archive.to_string(), "Archives");
        assert_eq!(MediaType::Other.to_string(), "Other");
    }

    #[test]
    fn test_regex_target_display() {
        assert_eq!(RegexTarget::FileName.to_string(), "File Name");
        assert_eq!(RegexTarget::FilePath.to_string(), "Full Path");
        assert_eq!(RegexTarget::Extension.to_string(), "Extension");
    }

    #[test]
    fn test_default_media_types() {
        let media_types = FilterSet::default_media_types();

        assert_eq!(media_types.len(), 5);

        // Check Image extensions
        let image_filter = &media_types[0];
        assert_eq!(image_filter.media_type, MediaType::Image);
        assert!(image_filter.extensions.contains(&"jpg".to_string()));
        assert!(image_filter.extensions.contains(&"png".to_string()));
        assert!(image_filter.extensions.contains(&"gif".to_string()));
        assert!(image_filter.enabled);

        // Check Video extensions
        let video_filter = &media_types[1];
        assert_eq!(video_filter.media_type, MediaType::Video);
        assert!(video_filter.extensions.contains(&"mp4".to_string()));
        assert!(video_filter.extensions.contains(&"avi".to_string()));
        assert!(video_filter.extensions.contains(&"mkv".to_string()));
        assert!(video_filter.enabled);

        // Check Audio is disabled by default
        assert!(!media_types[2].enabled);
    }

    #[test]
    fn test_size_range_conversion() {
        let mut filter_set = FilterSet::new();

        // Test exact conversion
        filter_set.add_size_range("Test".to_string(), Some(1.5), Some(2.5));
        let range = &filter_set.size_ranges[0];

        assert_eq!(range.min_bytes, Some(1024 * 1024 + 512 * 1024)); // 1.5MB
        assert_eq!(range.max_bytes, Some(2 * 1024 * 1024 + 512 * 1024)); // 2.5MB

        // Test with None values
        filter_set.size_ranges.clear();
        filter_set.add_size_range("Test".to_string(), None, Some(10.0));
        let range = &filter_set.size_ranges[0];

        assert_eq!(range.min_bytes, None);
        assert_eq!(range.max_bytes, Some(10 * 1024 * 1024));
    }

    #[test]
    fn test_file_with_no_extension() {
        let mut filter_set = FilterSet::new();
        filter_set.is_active = true; // Activate filtering

        let mut file = create_test_media_file();
        file.path = PathBuf::from("/test/noextension");
        file.extension = "".into();

        // Should not match any media type filter when extension is empty
        assert!(!filter_set.matches_file(&file));

        // Test with all media types disabled
        for media_type in &mut filter_set.media_types {
            media_type.enabled = false;
        }
        // When no media types are enabled, it should match (no media type filtering)
        assert!(filter_set.matches_file(&file));
    }

    #[test]
    fn test_invalid_regex_pattern() {
        let mut filter_set = FilterSet::new();

        // Add an invalid regex pattern
        filter_set.add_regex_pattern(r"[invalid regex".to_string(), RegexTarget::FileName, false);

        // Should still match files (invalid regex is ignored)
        let file = create_test_media_file();
        assert!(filter_set.matches_file(&file));
    }

    #[test]
    fn test_edge_cases() {
        let mut filter_set = FilterSet::new();
        let mut file = create_test_media_file();

        // Make filter active to test filtering behavior
        filter_set.is_active = true;

        // Test file with no extension
        file.path = PathBuf::from("/test/noextension");
        file.extension = String::new().into();

        // Should not match any media type filter
        assert!(!filter_set.matches_file(&file));

        // Reset for next test
        filter_set = FilterSet::new();

        // Test invalid regex pattern
        filter_set.add_regex_pattern(r"[invalid regex".to_string(), RegexTarget::FileName, false);
        // Should still match (invalid regex is ignored)
        assert!(filter_set.matches_file(&create_test_media_file()));

        // Test file with None path in to_str()
        // This is harder to test directly, but the code handles it
    }

    #[test]
    fn test_serialization() {
        let mut filter_set = FilterSet::new();
        filter_set.add_date_range("Test".to_string(), Some(Local::now()), None);
        filter_set.add_size_range("Test".to_string(), Some(1.0), Some(10.0));
        filter_set.add_regex_pattern("test".to_string(), RegexTarget::FileName, true);

        // Serialize to JSON
        let json = serde_json::to_string(&filter_set).unwrap();

        // Deserialize back
        let deserialized: FilterSet = serde_json::from_str(&json).unwrap();

        assert_eq!(filter_set.date_ranges.len(), deserialized.date_ranges.len());
        assert_eq!(filter_set.size_ranges.len(), deserialized.size_ranges.len());
        assert_eq!(filter_set.regex_patterns.len(), deserialized.regex_patterns.len());
        assert_eq!(filter_set.is_active, deserialized.is_active);
    }
}
