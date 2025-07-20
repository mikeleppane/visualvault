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
