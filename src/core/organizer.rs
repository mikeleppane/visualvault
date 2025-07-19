use color_eyre::eyre::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{Mutex, RwLock};
use tracing::error;

use crate::app::OrganizeResult;
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

    pub async fn organize_files_with_duplicates(
        &self,
        files: Vec<MediaFile>,
        duplicates: HashMap<String, Vec<MediaFile>>,
        settings: &Settings,
        progress: Arc<RwLock<Progress>>,
    ) -> Result<OrganizeResult> {
        let dest_folder = settings
            .destination_folder
            .as_ref()
            .ok_or_else(|| color_eyre::eyre::eyre!("Destination folder not configured"))?;

        // Track which files have been processed to avoid duplicates
        let mut processed_hashes: HashSet<String> = HashSet::new();
        let mut files_to_organize: Vec<MediaFile> = Vec::new();
        let mut skipped_duplicates = 0;
        let files_total = files.len();

        // If rename_duplicates is false, filter out duplicates
        if !settings.rename_duplicates && !duplicates.is_empty() {
            for file in files {
                if let Some(hash) = &file.hash {
                    if let Some(duplicate_group) = duplicates.get(hash) {
                        // This file is part of a duplicate group
                        if processed_hashes.contains(hash) {
                            // Already processed one file from this group, skip this one
                            skipped_duplicates += 1;
                            continue;
                        }
                        // First file from this group, process it
                        processed_hashes.insert(hash.clone());
                        // Choose the oldest file (by modified date) or first in list
                        let chosen_file = duplicate_group.iter().min_by_key(|f| f.modified).unwrap_or(&file);
                        files_to_organize.push(chosen_file.clone());
                    } else {
                        // Not a duplicate, process normally
                        files_to_organize.push(file);
                    }
                } else {
                    // No hash, process normally
                    files_to_organize.push(file);
                }
            }
        } else {
            // If rename_duplicates is true, organize all files
            files_to_organize = files;
        }

        // Update progress
        {
            let mut prog = progress.write().await;
            prog.total = files_to_organize.len();
            prog.current = 0;
            prog.message = "Organizing files...".to_string();
        }

        // Now organize the filtered files
        let mut moved_files = 0;
        let mut errors = Vec::new();

        for (idx, file) in files_to_organize.iter().enumerate() {
            match self.organize_file(file, dest_folder, settings).await {
                Ok(dest_path) => {
                    moved_files += 1;
                    tracing::info!("Organized {} to {}", file.name, dest_path.display());
                }
                Err(e) => {
                    tracing::error!("Failed to organize {}: {}", file.name, e);
                    errors.push(format!("{}: {}", file.name, e));
                }
            }

            // Update progress
            {
                let mut prog = progress.write().await;
                prog.current = idx + 1;
            }
        }

        Ok(OrganizeResult {
            files_organized: moved_files,
            files_total,
            destination: dest_folder.clone(),
            success: errors.is_empty(),
            timestamp: chrono::Local::now(),
            skipped_duplicates,
            errors,
        })
    }

    async fn organize_file(&self, file: &MediaFile, destination: &Path, settings: &Settings) -> Result<PathBuf> {
        let target_dir = Self::determine_target_directory(file, destination, settings)?;

        // Create target directory if it doesn't exist
        fs::create_dir_all(&target_dir).await?;

        // Handle file naming
        let file_name = if settings.rename_duplicates {
            // Check if file exists in target directory
            if target_dir.join(&file.name).exists() {
                Self::generate_unique_name(&target_dir, &file.name)?
            } else {
                file.name.clone()
            }
        } else {
            file.name.clone()
        };

        // Apply lowercase extension if configured
        let final_name = if settings.lowercase_extensions {
            let stem = Path::new(&file_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&file_name);
            let ext = Path::new(&file_name).extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext.is_empty() {
                file_name
            } else {
                format!("{}.{}", stem, ext.to_lowercase())
            }
        } else {
            file_name
        };

        let target_path = target_dir.join(&final_name);

        // Move the file
        fs::rename(&file.path, &target_path).await?;

        Ok(target_path)
    }

    fn determine_target_directory(file: &MediaFile, destination: &Path, settings: &Settings) -> Result<PathBuf> {
        let mut path = destination.to_path_buf();

        if settings.separate_videos && file.file_type == FileType::Video {
            path.push("Videos");
        }

        match OrganizationMode::from_str(&settings.organize_by) {
            Ok(OrganizationMode::Yearly) => {
                path.push(file.modified.format("%Y").to_string());
            }
            Ok(OrganizationMode::Monthly) => {
                path.push(file.modified.format("%Y").to_string());
                path.push(file.modified.format("%m-%B").to_string());
            }
            Ok(OrganizationMode::ByType) => {
                path.push(Self::get_type_folder(file, settings));
            }
            Ok(OrganizationMode::Daily) => {
                path.push(file.modified.format("%Y").to_string());
                path.push(file.modified.format("%m-%B").to_string());
                path.push(file.modified.format("%d-%A").to_string());
            }
            Ok(OrganizationMode::TypeAndDate) => {
                // Combine type and date
                path.push(Self::get_type_folder(file, settings));
                path.push(file.modified.format("%Y").to_string());
                path.push(file.modified.format("%m-%B").to_string());
                path.push(file.modified.format("%d-%A").to_string());
            }
            Err(e) => {
                error!("Invalid organization mode: {}", e);
                return Err(color_eyre::eyre::eyre!("Invalid organization mode"));
            }
        }
        Ok(path)
    }

    fn get_type_folder(file: &MediaFile, settings: &Settings) -> String {
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

    fn generate_unique_name(dir: &Path, original_name: &str) -> Result<String> {
        let mut counter = 1;
        let stem = Path::new(original_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let extension = Path::new(original_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        loop {
            let new_name = if extension.is_empty() {
                format!("{stem} ({counter})")
            } else {
                format!("{stem} ({counter}).{extension}")
            };

            if !dir.join(&new_name).exists() {
                return Ok(new_name);
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
