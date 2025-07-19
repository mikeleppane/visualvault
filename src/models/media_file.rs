use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub enum FileType {
    Image,
    Video,
    Document,
    Other,
}
// Display implementation for FileType
impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileType::Image => write!(f, "Image"),
            FileType::Video => write!(f, "Video"),
            FileType::Document => write!(f, "Document"),
            FileType::Other => write!(f, "Other"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFile {
    pub path: PathBuf,
    pub name: String,
    pub extension: String,
    pub file_type: FileType,
    pub size: u64,
    pub created: DateTime<Local>,
    pub modified: DateTime<Local>,
    pub hash: Option<String>,
    pub metadata: Option<MediaMetadata>,
}

impl MediaFile {
    pub fn new(
        path: PathBuf,
        name: String,
        extension: String,
        file_type: FileType,
        size: u64,
    ) -> Self {
        let now = Local::now();
        Self {
            path,
            name,
            extension,
            file_type,
            size,
            created: now,
            modified: now,
            hash: None,
            metadata: None,
        }
    }

    pub fn from_path(path: &Path) -> Result<Option<MediaFile>, std::io::Error> {
        // Check if it's a media file
        if !Self::is_media_file(path) {
            return Ok(None);
        }

        let metadata = std::fs::metadata(path)?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let file_type = Self::determine_file_type(&extension);

        // Convert system time to chrono DateTime
        let created = metadata
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
            .and_then(|d| DateTime::from_timestamp(d.as_secs() as i64, d.subsec_nanos()))
            .map(|dt| dt.with_timezone(&Local))
            .unwrap_or_else(Local::now);

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
            .and_then(|d| DateTime::from_timestamp(d.as_secs() as i64, d.subsec_nanos()))
            .map(|dt| dt.with_timezone(&Local))
            .unwrap_or_else(Local::now);

        Ok(Some(MediaFile {
            path: path.to_path_buf(),
            name,
            extension: extension.clone(),
            file_type,
            size: metadata.len(),
            created,
            modified,
            hash: None,
            metadata: None,
        }))
    }

    fn is_media_file(path: &Path) -> bool {
        const MEDIA_EXTENSIONS: &[&str] = &[
            // Images
            "jpg", "jpeg", "png", "gif", "bmp", "tiff", "webp", "raw", "heic", // Videos
            "mp4", "avi", "mkv", "mov", "wmv", "flv", "webm", "m4v", // Audio
            "mp3", "wav", "flac", "aac", "ogg", "wma", "m4a",
        ];

        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| MEDIA_EXTENSIONS.contains(&e.to_lowercase().as_str()))
            .unwrap_or(false)
    }

    fn determine_file_type(extension: &str) -> FileType {
        match extension {
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff" | "raw" | "heic" | "heif" => {
                FileType::Image
            }
            "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg" => {
                FileType::Video
            }
            "pdf" | "doc" | "docx" | "txt" | "odt" | "rtf" => FileType::Document,
            _ => FileType::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaMetadata {
    Image(ImageMetadata),
    Video(VideoMetadata),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub codec: String,
}

impl MediaFile {
    pub fn resolution_string(&self) -> Option<String> {
        match &self.metadata {
            Some(MediaMetadata::Image(meta)) => Some(format!("{}x{}", meta.width, meta.height)),
            Some(MediaMetadata::Video(meta)) => Some(format!("{}x{}", meta.width, meta.height)),
            None => None,
        }
    }

    pub fn is_duplicate_of(&self, other: &MediaFile) -> bool {
        if let (Some(hash1), Some(hash2)) = (&self.hash, &other.hash) {
            hash1 == hash2
        } else {
            self.name == other.name && self.size == other.size
        }
    }
}
