#![allow(clippy::unwrap_used)]
#![allow(clippy::field_reassign_with_default)]

use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf, str::FromStr};
use tracing::info;

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub source_folder: Option<PathBuf>,
    pub destination_folder: Option<PathBuf>,
    #[serde(default = "default_recurse_subfolders")]
    pub recurse_subfolders: bool,
    #[serde(default)]
    pub verbose_output: bool,
    #[serde(default = "default_organize_by")]
    pub organize_by: String,
    #[serde(default)]
    pub separate_videos: bool,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub keep_original_structure: bool,
    #[serde(default = "default_rename_duplicates")]
    pub rename_duplicates: bool,
    #[serde(default = "default_lowercase_extensions")]
    pub lowercase_extensions: bool,
    #[serde(default = "default_preserve_metadata")]
    pub preserve_metadata: bool,
    #[serde(default)]
    pub create_thumbnails: bool,
    #[serde(default = "default_worker_threads")]
    pub worker_threads: usize,
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,
    #[serde(default = "default_enable_cache")]
    pub enable_cache: bool,
    #[serde(default = "default_parallel_processing")]
    pub parallel_processing: bool,
    #[serde(default)]
    pub skip_hidden_files: bool,
    #[serde(default)]
    pub optimize_for_ssd: bool,
}

// Default value functions for serde
fn default_recurse_subfolders() -> bool {
    true
}
fn default_organize_by() -> String {
    "monthly".to_string()
}
fn default_rename_duplicates() -> bool {
    true
}
fn default_lowercase_extensions() -> bool {
    true
}
fn default_preserve_metadata() -> bool {
    true
}
fn default_worker_threads() -> usize {
    num_cpus::get()
}
fn default_buffer_size() -> usize {
    8 * 1024 * 1024
}
fn default_enable_cache() -> bool {
    true
}
fn default_parallel_processing() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            source_folder: None,
            destination_folder: None,
            recurse_subfolders: default_recurse_subfolders(),
            verbose_output: false,
            organize_by: default_organize_by(),
            separate_videos: false,
            dry_run: false,
            keep_original_structure: false,
            rename_duplicates: default_rename_duplicates(),
            lowercase_extensions: default_lowercase_extensions(),
            preserve_metadata: default_preserve_metadata(),
            create_thumbnails: false,
            worker_threads: default_worker_threads(),
            buffer_size: default_buffer_size(),
            enable_cache: default_enable_cache(),
            parallel_processing: default_parallel_processing(),
            skip_hidden_files: false,
            optimize_for_ssd: false,
        }
    }
}

impl Settings {
    /// Loads the settings from the configuration file, or returns defaults if the file does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration directory cannot be found, or if reading or parsing the configuration file fails.
    pub async fn load() -> Result<Self> {
        let config_dir =
            dirs::config_dir().ok_or_else(|| color_eyre::eyre::eyre!("Could not find config directory"))?;
        let config_path = config_dir.join("visualvault").join("config.toml");

        if config_path.exists() {
            let content = tokio::fs::read_to_string(&config_path).await?;
            let settings: Settings = toml::from_str(&content)?;
            Ok(settings)
        } else {
            Ok(Self::default())
        }
    }

    /// Saves the current settings to the configuration file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration directory cannot be found
    /// - The parent directory cannot be created
    /// - The settings cannot be serialized to TOML
    /// - The configuration file cannot be written to disk
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Serialize to TOML
        let toml_string = toml::to_string_pretty(self)?;

        // Write to file
        std::fs::write(&config_path, toml_string)?;

        info!("Settings saved to {:?}", config_path);
        Ok(())
    }

    fn config_path() -> Result<PathBuf> {
        let config_dir =
            dirs::config_dir().ok_or_else(|| color_eyre::eyre::eyre!("Could not find config directory"))?;
        Ok(config_dir.join("visualvault").join("config.toml"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationMode {
    Yearly,
    Monthly,
    Daily,
    ByType,
    TypeAndDate,
}

impl FromStr for OrganizationMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "yearly" => Ok(Self::Yearly),
            "monthly" => Ok(Self::Monthly),
            "daily" => Ok(Self::Daily),
            "type" => Ok(Self::ByType),
            "type-date" => Ok(Self::TypeAndDate),
            _ => Err(format!("Unknown organization mode: {s}")),
        }
    }
}

impl Default for OrganizationMode {
    fn default() -> Self {
        Self::Monthly
    }
}

impl fmt::Display for OrganizationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yearly => write!(f, "yearly"),
            Self::Monthly => write!(f, "monthly"),
            Self::Daily => write!(f, "daily"),
            Self::ByType => write!(f, "type"),
            Self::TypeAndDate => write!(f, "type-date"),
        }
    }
}

// ... existing code ...

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();

        assert_eq!(settings.source_folder, None);
        assert_eq!(settings.destination_folder, None);
        assert!(settings.recurse_subfolders);
        assert!(!settings.verbose_output);
        assert_eq!(settings.organize_by, "monthly");
        assert!(!settings.separate_videos);
        assert!(!settings.dry_run);
        assert!(!settings.keep_original_structure);
        assert!(settings.rename_duplicates);
        assert!(settings.lowercase_extensions);
        assert!(settings.preserve_metadata);
        assert!(!settings.create_thumbnails);
        assert_eq!(settings.worker_threads, num_cpus::get());
        assert_eq!(settings.buffer_size, 8 * 1024 * 1024);
        assert!(settings.enable_cache);
        assert!(settings.parallel_processing);
        assert!(!settings.skip_hidden_files);
        assert!(!settings.optimize_for_ssd);
    }

    #[test]
    fn test_organization_mode_from_str() {
        // Valid cases
        assert_eq!(OrganizationMode::from_str("yearly").unwrap(), OrganizationMode::Yearly);
        assert_eq!(
            OrganizationMode::from_str("monthly").unwrap(),
            OrganizationMode::Monthly
        );
        assert_eq!(OrganizationMode::from_str("daily").unwrap(), OrganizationMode::Daily);
        assert_eq!(OrganizationMode::from_str("type").unwrap(), OrganizationMode::ByType);
        assert_eq!(
            OrganizationMode::from_str("type-date").unwrap(),
            OrganizationMode::TypeAndDate
        );

        // Case insensitive
        assert_eq!(OrganizationMode::from_str("YEARLY").unwrap(), OrganizationMode::Yearly);
        assert_eq!(
            OrganizationMode::from_str("Monthly").unwrap(),
            OrganizationMode::Monthly
        );
        assert_eq!(
            OrganizationMode::from_str("Type-Date").unwrap(),
            OrganizationMode::TypeAndDate
        );

        // Invalid cases
        assert!(OrganizationMode::from_str("invalid").is_err());
        assert!(OrganizationMode::from_str("").is_err());
        assert!(OrganizationMode::from_str("year").is_err());
    }

    #[test]
    fn test_organization_mode_display() {
        assert_eq!(OrganizationMode::Yearly.to_string(), "yearly");
        assert_eq!(OrganizationMode::Monthly.to_string(), "monthly");
        assert_eq!(OrganizationMode::Daily.to_string(), "daily");
        assert_eq!(OrganizationMode::ByType.to_string(), "type");
        assert_eq!(OrganizationMode::TypeAndDate.to_string(), "type-date");
    }

    #[test]
    fn test_organization_mode_default() {
        assert_eq!(OrganizationMode::default(), OrganizationMode::Monthly);
    }

    #[test]
    fn test_organization_mode_serialization() {
        // Test that serialization and deserialization work correctly
        let mode = OrganizationMode::TypeAndDate;
        let serialized = serde_json::to_string(&mode).unwrap();
        let deserialized: OrganizationMode = serde_json::from_str(&serialized).unwrap();
        assert_eq!(mode, deserialized);
    }

    #[tokio::test]
    async fn test_load_nonexistent_config() {
        // Create a temporary directory and set it as config dir
        let temp_dir = TempDir::new().unwrap();
        unsafe {
            env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }

        // Should return default settings when config doesn't exist
        let settings = Settings::load().await.unwrap();
        assert_eq!(settings.organize_by, "monthly");
        assert!(settings.enable_cache);
    }

    #[tokio::test]
    async fn test_save_and_load_settings() {
        // Create a temporary directory for config
        let temp_dir = TempDir::new().unwrap();
        unsafe {
            env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }
        // Create custom settings
        let mut settings = Settings::default();
        settings.source_folder = Some(PathBuf::from("/test/source"));
        settings.destination_folder = Some(PathBuf::from("/test/dest"));
        settings.verbose_output = true;
        settings.organize_by = "yearly".to_string();
        settings.separate_videos = true;
        settings.dry_run = true;
        settings.worker_threads = 4;
        settings.buffer_size = 1024 * 1024;
        settings.skip_hidden_files = true;

        // Save settings
        settings.save().unwrap();

        // Verify file was created
        let config_path = temp_dir.path().join("visualvault").join("config.toml");
        assert!(config_path.exists());

        // Load settings
        let loaded = Settings::load().await.unwrap();

        // Verify all fields match
        assert_eq!(loaded.source_folder, Some(PathBuf::from("/test/source")));
        assert_eq!(loaded.destination_folder, Some(PathBuf::from("/test/dest")));
        assert!(loaded.verbose_output);
        assert_eq!(loaded.organize_by, "yearly");
        assert!(loaded.separate_videos);
        assert!(loaded.dry_run);
        assert_eq!(loaded.worker_threads, 4);
        assert_eq!(loaded.buffer_size, 1024 * 1024);
        assert!(loaded.skip_hidden_files);
    }

    #[test]
    fn test_settings_serialization_deserialization() {
        let settings = Settings {
            source_folder: Some(PathBuf::from("/source")),
            destination_folder: Some(PathBuf::from("/dest")),
            recurse_subfolders: false,
            verbose_output: true,
            organize_by: "daily".to_string(),
            separate_videos: true,
            dry_run: false,
            keep_original_structure: true,
            rename_duplicates: false,
            lowercase_extensions: false,
            preserve_metadata: false,
            create_thumbnails: true,
            worker_threads: 8,
            buffer_size: 4 * 1024 * 1024,
            enable_cache: false,
            parallel_processing: false,
            skip_hidden_files: true,
            optimize_for_ssd: true,
        };

        // Serialize to TOML
        let toml_str = toml::to_string(&settings).unwrap();

        // Deserialize back
        let deserialized: Settings = toml::from_str(&toml_str).unwrap();

        // Check all fields
        assert_eq!(settings.source_folder, deserialized.source_folder);
        assert_eq!(settings.destination_folder, deserialized.destination_folder);
        assert_eq!(settings.recurse_subfolders, deserialized.recurse_subfolders);
        assert_eq!(settings.verbose_output, deserialized.verbose_output);
        assert_eq!(settings.organize_by, deserialized.organize_by);
        assert_eq!(settings.separate_videos, deserialized.separate_videos);
        assert_eq!(settings.dry_run, deserialized.dry_run);
        assert_eq!(settings.keep_original_structure, deserialized.keep_original_structure);
        assert_eq!(settings.rename_duplicates, deserialized.rename_duplicates);
        assert_eq!(settings.lowercase_extensions, deserialized.lowercase_extensions);
        assert_eq!(settings.preserve_metadata, deserialized.preserve_metadata);
        assert_eq!(settings.create_thumbnails, deserialized.create_thumbnails);
        assert_eq!(settings.worker_threads, deserialized.worker_threads);
        assert_eq!(settings.buffer_size, deserialized.buffer_size);
        assert_eq!(settings.enable_cache, deserialized.enable_cache);
        assert_eq!(settings.parallel_processing, deserialized.parallel_processing);
        assert_eq!(settings.skip_hidden_files, deserialized.skip_hidden_files);
        assert_eq!(settings.optimize_for_ssd, deserialized.optimize_for_ssd);
    }

    #[test]
    fn test_partial_settings_deserialization() {
        // Test that partial TOML still works with defaults
        let toml_str = r#"
            source_folder = "/custom/source"
            verbose_output = true
            organize_by = "yearly"
        "#;

        let settings: Settings = toml::from_str(toml_str).unwrap();

        assert_eq!(settings.source_folder, Some(PathBuf::from("/custom/source")));
        assert!(settings.verbose_output);
        assert_eq!(settings.organize_by, "yearly");

        // Check defaults are applied
        assert!(settings.recurse_subfolders);
        assert!(settings.enable_cache);
        assert_eq!(settings.worker_threads, num_cpus::get());
    }

    #[test]
    fn test_config_path() {
        let temp_dir = TempDir::new().unwrap();
        unsafe { env::set_var("XDG_CONFIG_HOME", temp_dir.path()) };

        let config_path = Settings::config_path().unwrap();
        assert!(config_path.ends_with("visualvault/config.toml"));
    }

    #[test]
    fn test_default_functions() {
        assert!(default_recurse_subfolders());
        assert_eq!(default_organize_by(), "monthly");
        assert!(default_rename_duplicates());
        assert!(default_lowercase_extensions());
        assert!(default_preserve_metadata());
        assert_eq!(default_worker_threads(), num_cpus::get());
        assert_eq!(default_buffer_size(), 8 * 1024 * 1024);
        assert!(default_enable_cache());
        assert!(default_parallel_processing());
    }

    #[test]
    fn test_settings_clone() {
        let settings = Settings {
            source_folder: Some(PathBuf::from("/test")),
            verbose_output: true,
            worker_threads: 16,
            ..Default::default()
        };

        let cloned = settings.clone();

        assert_eq!(settings.source_folder, cloned.source_folder);
        assert_eq!(settings.verbose_output, cloned.verbose_output);
        assert_eq!(settings.worker_threads, cloned.worker_threads);
    }

    #[test]
    fn test_settings_debug_format() {
        let settings = Settings::default();
        let debug_str = format!("{settings:?}");

        // Just ensure it can be formatted without panic
        assert!(debug_str.contains("Settings"));
        assert!(debug_str.contains("recurse_subfolders"));
    }

    #[test]
    fn test_organization_mode_equality() {
        assert_eq!(OrganizationMode::Monthly, OrganizationMode::Monthly);
        assert_ne!(OrganizationMode::Monthly, OrganizationMode::Yearly);

        // Test all variants
        let modes = [
            OrganizationMode::Yearly,
            OrganizationMode::Monthly,
            OrganizationMode::Daily,
            OrganizationMode::ByType,
            OrganizationMode::TypeAndDate,
        ];

        for (i, mode1) in modes.iter().enumerate() {
            for (j, mode2) in modes.iter().enumerate() {
                if i == j {
                    assert_eq!(mode1, mode2);
                } else {
                    assert_ne!(mode1, mode2);
                }
            }
        }
    }

    #[test]
    fn test_edge_case_worker_threads() {
        // Test with 0 threads (should probably be validated in real code)
        let mut settings = Settings {
            worker_threads: 0,
            ..Default::default()
        };
        let toml_str = toml::to_string(&settings).unwrap();
        let deserialized: Settings = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.worker_threads, 0);

        // Test with very large but reasonable number
        // Use a large value that's safe for TOML serialization
        settings.worker_threads = 1024;
        let toml_str = toml::to_string(&settings).unwrap();
        let deserialized: Settings = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.worker_threads, 1024);

        // Test with typical max threads for a system
        settings.worker_threads = 256;
        let toml_str = toml::to_string(&settings).unwrap();
        let deserialized: Settings = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.worker_threads, 256);
    }
}
