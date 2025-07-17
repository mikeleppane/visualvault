mod media_file;
mod statistics;

pub use media_file::{FileType, ImageMetadata, MediaFile, MediaMetadata};

#[derive(Debug, Clone)]
pub enum OrganizeMode {
    Move,
    Copy,
}
