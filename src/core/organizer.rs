use color_eyre::eyre::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info};

use crate::config::settings::OrganizationMode;
use crate::{
    config::Settings,
    models::{FileType, MediaFile},
    utils::Progress,
};

pub struct FileOrganizer {
    is_organizing: Arc<Mutex<bool>>,
    result: Arc<Mutex<Option<Result<usize>>>>,
}

impl FileOrganizer {
    pub fn new() -> Self {
        Self {
            is_organizing: Arc::new(Mutex::new(false)),
            result: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn organize_files(
        &self,
        files: Vec<MediaFile>,
        settings: Settings,
        progress: Arc<RwLock<Progress>>,
    ) -> Result<usize> {
        let mut organizing = self.is_organizing.lock().await;
        *organizing = true;
        drop(organizing);

        let destination = settings
            .destination_folder
            .as_ref()
            .ok_or_else(|| color_eyre::eyre::eyre!("No destination folder set"))?;

        info!("Starting organization of {} files", files.len());

        let total = files.len();
        progress.write().await.set_total(total);

        let mut organized_count = 0;

        for (idx, file) in files.iter().enumerate() {
            match self.organize_file(file, destination, &settings).await {
                Ok(_) => organized_count += 1,
                Err(e) => {
                    error!("Failed to organize {}: {}", file.path.display(), e);
                }
            }

            progress.write().await.set_current(idx + 1);
            progress
                .write()
                .await
                .set_message(format!("Organized {} of {} files", idx + 1, total));
        }

        let mut organizing = self.is_organizing.lock().await;
        *organizing = false;

        let mut result = self.result.lock().await;
        *result = Some(Ok(organized_count));

        info!("Organization complete: {} files organized", organized_count);
        Ok(organized_count)
    }

    async fn organize_file(
        &self,
        file: &MediaFile,
        destination: &Path,
        settings: &Settings,
    ) -> Result<()> {
        let target_dir = self.determine_target_directory(file, destination, settings)?;

        // Create target directory if it doesn't exist
        fs::create_dir_all(&target_dir).await?;

        let target_path = target_dir.join(&file.name);

        // Handle existing files
        if target_path.exists() {
            let target_path = self.handle_existing_file(&target_path).await?;
            fs::rename(&file.path, &target_path).await?;
        } else {
            fs::rename(&file.path, &target_path).await?;
        }

        Ok(())
    }

    fn determine_target_directory(
        &self,
        file: &MediaFile,
        destination: &Path,
        settings: &Settings,
    ) -> Result<PathBuf> {
        let mut path = destination.to_path_buf();

        match OrganizationMode::from_string(&settings.organize_by) {
            OrganizationMode::Yearly => {
                path.push(file.created.format("%Y").to_string());
            }
            OrganizationMode::Monthly => {
                path.push(file.created.format("%Y").to_string());
                path.push(file.created.format("%m-%B").to_string());
            }
            OrganizationMode::ByType => {
                path.push(self.get_type_folder(file, settings));
            }
            OrganizationMode::Daily => {
                path.push(file.created.format("%Y").to_string());
                path.push(file.created.format("%m-%B").to_string());
                path.push(file.created.format("%d-%A").to_string());
            }
            OrganizationMode::TypeAndDate => {
                path.push(self.get_type_folder(file, settings));
                path.push(file.created.format("%Y").to_string());
                path.push(file.created.format("%m-%B").to_string());
                path.push(file.created.format("%d-%A").to_string());
            }
        }

        Ok(path)
    }

    fn get_type_folder(&self, file: &MediaFile, settings: &Settings) -> String {
        match file.file_type {
            FileType::Image => "Images".to_string(),
            FileType::Video => {
                if settings.separate_videos {
                    "Videos".to_string()
                } else {
                    "Media".to_string()
                }
            }
            FileType::Document => "Documents".to_string(),
            FileType::Other => "Other".to_string(),
        }
    }

    async fn handle_existing_file(&self, path: &Path) -> Result<PathBuf> {
        let mut counter = 1;
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let parent = path.parent().unwrap_or(Path::new(""));

        loop {
            let new_name = if extension.is_empty() {
                format!("{stem} ({counter})")
            } else {
                format!("{stem} ({counter}).{extension}")
            };

            let new_path = parent.join(new_name);
            if !new_path.exists() {
                return Ok(new_path);
            }

            counter += 1;
            if counter > 999 {
                return Err(color_eyre::eyre::eyre!("Too many duplicate filenames"));
            }
        }
    }

    pub async fn is_complete(&self) -> bool {
        !*self.is_organizing.lock().await
    }

    pub async fn get_result(&self) -> Option<Result<usize>> {
        self.result.lock().await.take()
    }
}
