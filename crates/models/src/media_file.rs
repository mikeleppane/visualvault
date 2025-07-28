use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

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
            FileType::Other => write!(f, "Others"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaFile {
    pub path: PathBuf,
    pub name: Arc<str>,
    pub extension: Arc<str>,
    pub file_type: FileType,
    pub size: u64,
    pub created: DateTime<Local>,
    pub modified: DateTime<Local>,
    pub hash: Option<Arc<str>>,
    pub metadata: Option<MediaMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MediaMetadata {
    Image(ImageMetadata),
    Video(VideoMetadata),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageMetadata {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoMetadata {
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub codec: String,
}

// ... existing code ...

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::float_cmp)] // For comparing floats in tests
    #![allow(clippy::panic)]
    use super::*;
    use chrono::TimeZone;

    fn create_test_media_file() -> MediaFile {
        MediaFile {
            path: PathBuf::from("/test/path/image.jpg"),
            name: "image.jpg".into(),
            extension: "jpg".into(),
            file_type: FileType::Image,
            size: 1024 * 1024 * 5, // 5MB
            created: Local.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
            modified: Local.with_ymd_and_hms(2024, 1, 20, 14, 45, 0).unwrap(),
            hash: Some("abc123def456".into()),
            metadata: Some(MediaMetadata::Image(ImageMetadata {
                width: 1920,
                height: 1080,
                format: "JPEG".into(),
                color_type: "RGB".into(),
            })),
        }
    }

    #[test]
    fn test_file_type_display() {
        assert_eq!(FileType::Image.to_string(), "Image");
        assert_eq!(FileType::Video.to_string(), "Video");
        assert_eq!(FileType::Document.to_string(), "Document");
        assert_eq!(FileType::Other.to_string(), "Others");
    }

    #[test]
    fn test_file_type_equality() {
        assert_eq!(FileType::Image, FileType::Image);
        assert_ne!(FileType::Image, FileType::Video);

        // Test all combinations
        let types = [FileType::Image, FileType::Video, FileType::Document, FileType::Other];
        for (i, type1) in types.iter().enumerate() {
            for (j, type2) in types.iter().enumerate() {
                if i == j {
                    assert_eq!(type1, type2);
                } else {
                    assert_ne!(type1, type2);
                }
            }
        }
    }

    #[test]
    fn test_file_type_serialization() {
        let file_type = FileType::Video;

        // Serialize to JSON
        let json = serde_json::to_string(&file_type).unwrap();
        assert_eq!(json, "\"Video\"");

        // Deserialize back
        let deserialized: FileType = serde_json::from_str(&json).unwrap();
        assert_eq!(file_type, deserialized);
    }

    #[test]
    fn test_media_file_creation() {
        let file = create_test_media_file();

        assert_eq!(file.path, PathBuf::from("/test/path/image.jpg"));
        assert_eq!(file.name, "image.jpg".into());
        assert_eq!(file.extension, "jpg".into());
        assert_eq!(file.file_type, FileType::Image);
        assert_eq!(file.size, 5 * 1024 * 1024);
        assert_eq!(file.hash, Some("abc123def456".into()));
        assert!(file.metadata.is_some());
    }

    #[test]
    fn test_media_file_with_no_metadata() {
        let file = MediaFile {
            path: PathBuf::from("/test/document.pdf"),
            name: "document.pdf".to_string().into(),
            extension: "pdf".to_string().into(),
            file_type: FileType::Document,
            size: 2048,
            created: Local::now(),
            modified: Local::now(),
            hash: None,
            metadata: None,
        };

        assert_eq!(file.name, "document.pdf".into());
        assert!(file.hash.is_none());
        assert!(file.metadata.is_none());
    }

    #[test]
    fn test_media_file_serialization() {
        let file = create_test_media_file();

        // Serialize to JSON
        let json = serde_json::to_string(&file).unwrap();

        // Deserialize back
        let deserialized: MediaFile = serde_json::from_str(&json).unwrap();

        assert_eq!(file.path, deserialized.path);
        assert_eq!(file.name, deserialized.name);
        assert_eq!(file.extension, deserialized.extension);
        assert_eq!(file.file_type, deserialized.file_type);
        assert_eq!(file.size, deserialized.size);
        assert_eq!(file.hash, deserialized.hash);
    }

    #[test]
    fn test_image_metadata() {
        let metadata = ImageMetadata {
            width: 3840,
            height: 2160,
            format: "PNG".to_string(),
            color_type: "RGBA".to_string(),
        };

        assert_eq!(metadata.width, 3840);
        assert_eq!(metadata.height, 2160);
        assert_eq!(metadata.format, "PNG");
        assert_eq!(metadata.color_type, "RGBA");

        // Test serialization
        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: ImageMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(metadata.width, deserialized.width);
        assert_eq!(metadata.format, deserialized.format);
    }

    #[test]
    fn test_video_metadata() {
        let metadata = VideoMetadata {
            duration_seconds: 120.5,
            width: 1920,
            height: 1080,
            fps: 29.97,
            codec: "H.264".to_string(),
        };

        assert_eq!(metadata.duration_seconds, 120.5);
        assert_eq!(metadata.width, 1920);
        assert_eq!(metadata.height, 1080);
        assert_eq!(metadata.fps, 29.97);
        assert_eq!(metadata.codec, "H.264");

        // Test serialization
        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: VideoMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(metadata.duration_seconds, deserialized.duration_seconds);
        assert_eq!(metadata.codec, deserialized.codec);
    }

    #[test]
    fn test_media_metadata_enum() {
        // Test Image variant
        let image_meta = MediaMetadata::Image(ImageMetadata {
            width: 800,
            height: 600,
            format: "BMP".to_string(),
            color_type: "RGB".to_string(),
        });

        if let MediaMetadata::Image(meta) = &image_meta {
            assert_eq!(meta.width, 800);
            assert_eq!(meta.height, 600);
        } else {
            panic!("Expected Image metadata");
        }

        // Test Video variant
        let video_meta = MediaMetadata::Video(VideoMetadata {
            duration_seconds: 60.0,
            width: 640,
            height: 480,
            fps: 25.0,
            codec: "MPEG-4".to_string(),
        });

        if let MediaMetadata::Video(meta) = &video_meta {
            assert_eq!(meta.duration_seconds, 60.0);
            assert_eq!(meta.fps, 25.0);
        } else {
            panic!("Expected Video metadata");
        }
    }

    #[test]
    fn test_media_file_clone() {
        let original = create_test_media_file();
        let cloned = original.clone();

        assert_eq!(original.path, cloned.path);
        assert_eq!(original.name, cloned.name);
        assert_eq!(original.size, cloned.size);

        // Ensure it's a deep clone
        assert_eq!(format!("{original:?}"), format!("{:?}", cloned));
    }

    #[test]
    fn test_edge_cases() {
        // Test with empty strings
        let file = MediaFile {
            path: PathBuf::new(),
            name: String::new().into(),
            extension: String::new().into(),
            file_type: FileType::Other,
            size: 0,
            created: Local::now(),
            modified: Local::now(),
            hash: Some(String::new().into()),
            metadata: None,
        };

        assert_eq!(file.name, "".into());
        assert_eq!(file.extension, "".into());
        assert_eq!(file.size, 0);
        assert_eq!(file.hash, Some(String::new().into()));

        // Test with very large size
        let large_file = MediaFile {
            path: PathBuf::from("/large.bin"),
            name: "large.bin".to_string().into(),
            extension: "bin".to_string().into(),
            file_type: FileType::Other,
            size: u64::MAX,
            created: Local::now(),
            modified: Local::now(),
            hash: None,
            metadata: None,
        };

        assert_eq!(large_file.size, u64::MAX);
    }

    #[test]
    fn test_path_operations() {
        let file = create_test_media_file();

        // Test path components
        assert_eq!(file.path.file_name().unwrap(), "image.jpg");
        assert_eq!(file.path.extension().unwrap(), "jpg");

        // Test path manipulation
        let mut modified_file = file;
        modified_file.path = PathBuf::from("/new/path/renamed.png");
        assert_eq!(modified_file.path.file_name().unwrap(), "renamed.png");
    }

    #[test]
    fn test_datetime_handling() {
        let file = create_test_media_file();

        // Check date ordering
        assert!(file.modified > file.created);

        // Test with same dates
        let mut same_date_file = file;
        same_date_file.created = same_date_file.modified;
        assert_eq!(same_date_file.created, same_date_file.modified);
    }

    #[test]
    fn test_metadata_serialization_roundtrip() {
        let file = create_test_media_file();

        // Convert to JSON and back
        let json = serde_json::to_string_pretty(&file).unwrap();
        let parsed: MediaFile = serde_json::from_str(&json).unwrap();

        // Verify metadata survived the roundtrip
        match (&file.metadata, &parsed.metadata) {
            (Some(MediaMetadata::Image(orig)), Some(MediaMetadata::Image(parsed))) => {
                assert_eq!(orig.width, parsed.width);
                assert_eq!(orig.height, parsed.height);
                assert_eq!(orig.format, parsed.format);
                assert_eq!(orig.color_type, parsed.color_type);
            }
            _ => panic!("Metadata mismatch after serialization"),
        }
    }

    #[test]
    fn test_file_type_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(FileType::Image);
        set.insert(FileType::Video);
        set.insert(FileType::Document);
        set.insert(FileType::Other);

        // All types should be unique in the set
        assert_eq!(set.len(), 4);

        // Test that we can find items
        assert!(set.contains(&FileType::Image));
        assert!(set.contains(&FileType::Video));
    }
}
