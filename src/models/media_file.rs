use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

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
