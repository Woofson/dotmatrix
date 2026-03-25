//! Global configuration for dotmatrix
//!
//! Handles global settings that apply across all projects.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Global dotmatrix configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Custom data directory path (optional, defaults to system data dir)
    #[serde(default)]
    pub data_dir: Option<String>,

    /// Default backup mode for new files
    #[serde(default)]
    pub default_backup_mode: BackupMode,

    /// Default archive format
    #[serde(default)]
    pub default_archive_format: ArchiveFormat,

    /// Enable git tracking by default
    #[serde(default = "default_true")]
    pub git_enabled: bool,

    /// Global exclude patterns
    #[serde(default = "default_excludes")]
    pub exclude: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_excludes() -> Vec<String> {
    vec![
        "**/*.log".to_string(),
        "**/.DS_Store".to_string(),
        "**/node_modules/**".to_string(),
        "**/.git/**".to_string(),
        "**/target/**".to_string(),
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: None,
            default_backup_mode: BackupMode::default(),
            default_archive_format: ArchiveFormat::default(),
            git_enabled: true,
            exclude: default_excludes(),
        }
    }
}

/// Backup mode for tracked files
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BackupMode {
    /// Content-addressed incremental backups
    #[default]
    Incremental,
    /// Archive-based backups (tarball)
    Archive,
}

impl BackupMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackupMode::Incremental => "incremental",
            BackupMode::Archive => "archive",
        }
    }
}

/// Archive format for archive backups
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveFormat {
    TarGz,
    Zip,
    SevenZ,
}

impl Default for ArchiveFormat {
    fn default() -> Self {
        #[cfg(windows)]
        {
            ArchiveFormat::Zip
        }
        #[cfg(not(windows))]
        {
            ArchiveFormat::TarGz
        }
    }
}

impl ArchiveFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ArchiveFormat::TarGz => "tar.gz",
            ArchiveFormat::Zip => "zip",
            ArchiveFormat::SevenZ => "7z",
        }
    }
}

impl Config {
    /// Load config from the default location
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&contents)?)
        } else {
            Ok(Self::default())
        }
    }

    /// Save config to the default location
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        Ok(())
    }

    /// Get the config file path
    pub fn config_path() -> anyhow::Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("dotmatrix").join("config.toml"))
    }

    /// Get the config directory path
    pub fn config_dir() -> anyhow::Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("dotmatrix"))
    }

    /// Get the data directory path (where backups/store lives)
    pub fn data_dir(&self) -> anyhow::Result<PathBuf> {
        if let Some(custom) = &self.data_dir {
            Ok(expand_path(custom))
        } else {
            let data_dir = dirs::data_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
            Ok(data_dir.join("dotmatrix"))
        }
    }

    /// Get the store directory path (git-tracked file store)
    pub fn store_dir(&self) -> anyhow::Result<PathBuf> {
        Ok(self.data_dir()?.join("store"))
    }

    /// Get the backups directory path
    pub fn backups_dir(&self) -> anyhow::Result<PathBuf> {
        Ok(self.data_dir()?.join("backups"))
    }
}

/// Expand ~ to home directory
pub fn expand_path(path: &str) -> PathBuf {
    if path.starts_with("~/") || path == "~" {
        if let Some(home) = dirs::home_dir() {
            if path == "~" {
                return home;
            }
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

/// Contract home directory to ~
pub fn contract_path(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(stripped) = path.strip_prefix(&home) {
            return format!("~/{}", stripped.display());
        }
    }
    path.display().to_string()
}
