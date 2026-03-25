//! File index for tracking backup state

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Index of all tracked files and their backup state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Index {
    /// Map of file path to entry
    #[serde(default)]
    pub entries: HashMap<PathBuf, FileEntry>,
}

/// State of a tracked file in the index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// SHA256 hash of the file contents
    pub hash: String,

    /// File size in bytes
    pub size: u64,

    /// Last modified timestamp
    pub modified: chrono::DateTime<chrono::Utc>,

    /// Last backup timestamp
    #[serde(default)]
    pub last_backup: Option<chrono::DateTime<chrono::Utc>>,
}

impl Index {
    /// Load index from the default location
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::index_path()?;
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&contents)?)
        } else {
            Ok(Self::default())
        }
    }

    /// Save index to the default location
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::index_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        Ok(())
    }

    /// Get the index file path
    pub fn index_path() -> anyhow::Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("dotmatrix").join("index.json"))
    }

    /// Get entry for a file
    pub fn get(&self, path: &PathBuf) -> Option<&FileEntry> {
        self.entries.get(path)
    }

    /// Update or insert entry for a file
    pub fn upsert(&mut self, path: PathBuf, entry: FileEntry) {
        self.entries.insert(path, entry);
    }

    /// Remove entry for a file
    pub fn remove(&mut self, path: &PathBuf) -> Option<FileEntry> {
        self.entries.remove(path)
    }
}
