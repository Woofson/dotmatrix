//! File index for tracking backup state
//!
//! The index stores the last known state of each tracked file,
//! used for drift detection and sync operations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Index of all tracked files and their backup state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Index {
    /// Map of absolute file path to entry
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

    /// Last modified timestamp (Unix epoch seconds)
    pub modified: u64,

    /// Last sync timestamp
    #[serde(default)]
    pub last_sync: Option<chrono::DateTime<chrono::Utc>>,

    /// Last backup timestamp
    #[serde(default)]
    pub last_backup: Option<chrono::DateTime<chrono::Utc>>,
}

impl Index {
    /// Create a new empty index
    pub fn new() -> Self {
        Self::default()
    }

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

    /// Get entry for a file by absolute path
    pub fn get(&self, path: &PathBuf) -> Option<&FileEntry> {
        self.entries.get(path)
    }

    /// Get mutable entry for a file
    pub fn get_mut(&mut self, path: &PathBuf) -> Option<&mut FileEntry> {
        self.entries.get_mut(path)
    }

    /// Update or insert entry for a file
    pub fn upsert(&mut self, path: PathBuf, entry: FileEntry) {
        self.entries.insert(path, entry);
    }

    /// Remove entry for a file
    pub fn remove(&mut self, path: &PathBuf) -> Option<FileEntry> {
        self.entries.remove(path)
    }

    /// Check if a file is tracked in the index
    pub fn contains(&self, path: &PathBuf) -> bool {
        self.entries.contains_key(path)
    }

    /// Get the number of tracked files
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all entries
    pub fn iter(&self) -> impl Iterator<Item = (&PathBuf, &FileEntry)> {
        self.entries.iter()
    }
}

impl FileEntry {
    /// Create a new file entry
    pub fn new(hash: String, size: u64, modified: u64) -> Self {
        Self {
            hash,
            size,
            modified,
            last_sync: None,
            last_backup: None,
        }
    }

    /// Create a file entry with current sync time
    pub fn with_sync_now(hash: String, size: u64, modified: u64) -> Self {
        Self {
            hash,
            size,
            modified,
            last_sync: Some(chrono::Utc::now()),
            last_backup: None,
        }
    }

    /// Mark as synced now
    pub fn mark_synced(&mut self) {
        self.last_sync = Some(chrono::Utc::now());
    }

    /// Mark as backed up now
    pub fn mark_backed_up(&mut self) {
        self.last_backup = Some(chrono::Utc::now());
    }
}
