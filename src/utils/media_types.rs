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

pub fn determine_file_type(extension: &str) -> FileType {
    match extension {
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff" | "raw" | "heic" | "heif" => FileType::Image,
        "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg" => FileType::Video,
        "pdf" | "doc" | "docx" | "txt" | "odt" | "rtf" => FileType::Document,
        _ => FileType::Other,
    }
}
