//! Global configuration for dotmatrix

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Global dotmatrix configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Default backup mode for new projects
    #[serde(default)]
    pub default_backup_mode: BackupMode,

    /// Default archive format
    #[serde(default)]
    pub default_archive_format: ArchiveFormat,

    /// Enable encryption for backups
    #[serde(default)]
    pub encrypt_backups: bool,
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

/// Archive format for archive backups
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveFormat {
    #[default]
    TarGz,
    Zip,
    SevenZ,
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

    /// Get the data directory path
    pub fn data_dir() -> anyhow::Result<PathBuf> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        Ok(data_dir.join("dotmatrix"))
    }
}
