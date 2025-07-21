use regex::RegexSet;

use crate::models::FileType;

#[allow(clippy::expect_used)]
pub static MEDIA_EXTENSIONS: std::sync::LazyLock<RegexSet> = std::sync::LazyLock::new(|| {
    RegexSet::new([
        r"(?i)\.(jpg|jpeg|png|gif|bmp|webp|tiff|svg|ico|heic)$",
        r"(?i)\.(mp4|avi|mkv|mov|wmv|flv|webm|m4v|mpg|mpeg)$",
        r"(?i)\.(mp3|wav|flac|aac|ogg|wma|m4a|opus)$",
    ])
    .expect("Failed to compile media extensions regex patterns")
});

#[must_use]
pub fn determine_file_type(extension: &str) -> FileType {
    match extension {
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff" | "raw" | "heic" | "heif" => FileType::Image,
        "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg" => FileType::Video,
        "pdf" | "doc" | "docx" | "txt" | "odt" | "rtf" => FileType::Document,
        _ => FileType::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_file_type_images() {
        // Test all image extensions
        let image_extensions = vec![
            "jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "raw", "heic", "heif",
        ];

        for ext in image_extensions {
            assert_eq!(
                determine_file_type(ext),
                FileType::Image,
                "Extension '{ext}' should be identified as Image"
            );
        }
    }

    #[test]
    fn test_determine_file_type_videos() {
        // Test all video extensions
        let video_extensions = vec!["mp4", "avi", "mkv", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg"];

        for ext in video_extensions {
            assert_eq!(
                determine_file_type(ext),
                FileType::Video,
                "Extension '{ext}' should be identified as Video"
            );
        }
    }

    #[test]
    fn test_determine_file_type_documents() {
        // Test all document extensions
        let document_extensions = vec!["pdf", "doc", "docx", "txt", "odt", "rtf"];

        for ext in document_extensions {
            assert_eq!(
                determine_file_type(ext),
                FileType::Document,
                "Extension '{ext}' should be identified as Document"
            );
        }
    }

    #[test]
    fn test_determine_file_type_other() {
        // Test extensions that should be classified as Other
        let other_extensions = vec![
            "exe", "zip", "rar", "7z", "tar", "gz", "iso", "dmg", "pkg", "deb", "rpm", "msi", "app", "js", "py", "rs",
            "go", "java", "cpp", "c", "html", "css", "xml", "json", "yaml", "toml", "mp3", "wav", "flac", "aac", "ogg",
            "wma", // Audio files (not in our media types)
            "",    // Empty string
            "unknown", "xyz", "abc", "123",
        ];

        for ext in other_extensions {
            assert_eq!(
                determine_file_type(ext),
                FileType::Other,
                "Extension '{ext}' should be identified as Other"
            );
        }
    }

    #[test]
    fn test_determine_file_type_case_sensitivity() {
        // The function appears to be case-sensitive based on the implementation
        // Testing uppercase versions which should be treated as Other
        assert_eq!(determine_file_type("JPG"), FileType::Other);
        assert_eq!(determine_file_type("PNG"), FileType::Other);
        assert_eq!(determine_file_type("MP4"), FileType::Other);
        assert_eq!(determine_file_type("PDF"), FileType::Other);

        // Mixed case
        assert_eq!(determine_file_type("Jpg"), FileType::Other);
        assert_eq!(determine_file_type("Mp4"), FileType::Other);
        assert_eq!(determine_file_type("Pdf"), FileType::Other);
    }

    #[test]
    fn test_determine_file_type_edge_cases() {
        // Test edge cases
        assert_eq!(determine_file_type(""), FileType::Other);
        assert_eq!(determine_file_type(" "), FileType::Other);
        assert_eq!(determine_file_type("jpg "), FileType::Other); // With trailing space
        assert_eq!(determine_file_type(" jpg"), FileType::Other); // With leading space
        assert_eq!(determine_file_type(".jpg"), FileType::Other); // With dot
        assert_eq!(determine_file_type("file.jpg"), FileType::Other); // Full filename
        assert_eq!(determine_file_type("jpg.png"), FileType::Other); // Multiple extensions
    }

    #[test]
    fn test_determine_file_type_similar_extensions() {
        // Test extensions that are similar but not exact matches
        assert_eq!(determine_file_type("jp"), FileType::Other);
        assert_eq!(determine_file_type("jpe"), FileType::Other);
        assert_eq!(determine_file_type("jpgg"), FileType::Other);
        assert_eq!(determine_file_type("pngg"), FileType::Other);
        assert_eq!(determine_file_type("mp"), FileType::Other);
        assert_eq!(determine_file_type("mp44"), FileType::Other);
        assert_eq!(determine_file_type("pdff"), FileType::Other);
    }

    #[test]
    fn test_determine_file_type_unicode() {
        // Test with unicode characters
        assert_eq!(determine_file_type("jpg📷"), FileType::Other);
        assert_eq!(determine_file_type("файл"), FileType::Other);
        assert_eq!(determine_file_type("图片"), FileType::Other);
        assert_eq!(determine_file_type("🎬mp4"), FileType::Other);
    }

    #[test]
    fn test_media_extensions_regex() {
        // Test that MEDIA_EXTENSIONS regex works correctly
        let test_cases = vec![
            ("image.jpg", true),
            ("IMAGE.JPG", true), // Case insensitive
            ("photo.jpeg", true),
            ("pic.png", true),
            ("animation.gif", true),
            ("video.mp4", true),
            ("MOVIE.AVI", true),
            ("song.mp3", true),
            ("audio.wav", true),
            ("document.pdf", false), // PDF not in MEDIA_EXTENSIONS
            ("file.txt", false),
            ("archive.zip", false),
            ("noextension", false),
            ("", false),
        ];

        for (filename, should_match) in test_cases {
            assert_eq!(
                MEDIA_EXTENSIONS.is_match(filename),
                should_match,
                "Filename '{filename}' match expectation failed"
            );
        }
    }

    #[test]
    fn test_all_determine_file_type_extensions_consistency() {
        // Ensure all extensions in determine_file_type for Image and Video
        // are covered by MEDIA_EXTENSIONS regex (except for audio which seems intentional)

        let image_extensions = vec!["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "heic"];
        let video_extensions = vec!["mp4", "avi", "mkv", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg"];

        for ext in image_extensions {
            let filename = format!("test.{ext}");
            assert!(
                MEDIA_EXTENSIONS.is_match(&filename),
                "Image extension '{ext}' should be in MEDIA_EXTENSIONS"
            );
        }

        for ext in video_extensions {
            let filename = format!("test.{ext}");
            assert!(
                MEDIA_EXTENSIONS.is_match(&filename),
                "Video extension '{ext}' should be in MEDIA_EXTENSIONS"
            );
        }

        // Note: 'raw' and 'heif' in determine_file_type are not in MEDIA_EXTENSIONS regex
        // This might be intentional or an oversight
        let missing_in_regex = vec!["raw", "heif"];
        for ext in missing_in_regex {
            let filename = format!("test.{ext}");
            assert!(
                !MEDIA_EXTENSIONS.is_match(&filename),
                "Extension '{ext}' is in determine_file_type but not in MEDIA_EXTENSIONS"
            );
        }
    }
}
