use color_eyre::eyre::Result;
use image::GenericImageView;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::info;
use walkdir::WalkDir;

use crate::{
    models::{FileType, ImageMetadata, MediaFile, MediaMetadata},
    utils::Progress,
};

pub struct Scanner {
    is_scanning: Arc<Mutex<bool>>,
}

impl Scanner {
    pub fn new() -> Self {
        Self {
            is_scanning: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn scan_directory(
        &self,
        path: &Path,
        recurse: bool,
        progress: Arc<RwLock<Progress>>,
    ) -> Result<Vec<MediaFile>> {
        let mut scanning = self.is_scanning.lock().await;
        *scanning = true;
        drop(scanning);

        info!("Starting scan of {:?}", path);
        let mut files = Vec::new();

        let walker = if recurse {
            WalkDir::new(path)
        } else {
            WalkDir::new(path).max_depth(1)
        };

        let entries: Vec<_> = walker
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();

        let total = entries.len();
        progress.write().await.set_total(total);

        for (idx, entry) in entries.into_iter().enumerate() {
            if let Ok(file) = self.process_file(entry.path()).await {
                files.push(file);
            }

            progress.write().await.set_current(idx + 1);
            progress
                .write()
                .await
                .set_message(format!("Scanned {} of {} files", idx + 1, total));
        }

        let mut scanning = self.is_scanning.lock().await;
        *scanning = false;

        info!("Scan complete: {} files found", files.len());
        Ok(files)
    }

    fn system_time_to_datetime(
        time: std::io::Result<std::time::SystemTime>,
    ) -> chrono::DateTime<chrono::Local> {
        time.ok()
            .and_then(|t| t.try_into().ok())
            .unwrap_or_else(chrono::Local::now)
    }

    async fn process_file(&self, path: &Path) -> Result<MediaFile> {
        let metadata = tokio::fs::metadata(path).await?;
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

        let file_type = self.determine_file_type(&extension);

        // Much cleaner!
        let created = Self::system_time_to_datetime(metadata.created());
        info!("Processing file: {} (type: {:?})", name, file_type);
        info!("Created: {}", created);

        let modified = Self::system_time_to_datetime(metadata.modified());
        info!("Modified: {}", modified);

        let mut media_file = MediaFile {
            path: path.to_path_buf(),
            name,
            extension: extension.clone(),
            file_type: file_type.clone(),
            size: metadata.len(),
            created,
            modified,
            hash: None,
            metadata: None,
        };

        // Extract metadata for images
        if file_type == FileType::Image {
            if let Ok(metadata) = self.extract_image_metadata(path).await {
                media_file.metadata = Some(metadata);
            }
        }

        Ok(media_file)
    }

    fn determine_file_type(&self, extension: &str) -> FileType {
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

    async fn extract_image_metadata(&self, path: &Path) -> Result<MediaMetadata> {
        let path = path.to_path_buf();

        tokio::task::spawn_blocking(move || {
            if let Ok(img) = image::open(&path) {
                let (width, height) = img.dimensions();
                let color = format!("{:?}", img.color());

                Ok(MediaMetadata::Image(ImageMetadata {
                    width,
                    height,
                    format: "unknown".to_string(),
                    color_type: color,
                }))
            } else {
                Err(color_eyre::eyre::eyre!("Failed to open image"))
            }
        })
        .await?
    }

    pub async fn is_complete(&self) -> bool {
        !*self.is_scanning.lock().await
    }
}
