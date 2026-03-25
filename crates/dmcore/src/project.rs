//! Project definition - a logical grouping of scattered files

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A project is a logical grouping of files that may be scattered across disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Optional description
    #[serde(default)]
    pub description: Option<String>,

    /// Git remote URL for this project (optional)
    #[serde(default)]
    pub remote: Option<String>,

    /// Files tracked in this project
    #[serde(default)]
    pub files: Vec<TrackedFile>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            description: None,
            remote: None,
            files: Vec::new(),
        }
    }
}

/// A file tracked within a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedFile {
    /// Absolute path to the file on disk
    pub path: PathBuf,

    /// How this file should be tracked
    #[serde(default)]
    pub track: TrackMode,

    /// Last known SHA256 hash
    #[serde(default)]
    pub last_hash: Option<String>,

    /// Last sync timestamp
    #[serde(default)]
    pub last_sync: Option<chrono::DateTime<chrono::Utc>>,
}

/// How a file should be tracked
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TrackMode {
    /// Track via git (for text/diffable files)
    #[default]
    Git,
    /// Track via backup only (for binary files)
    Backup,
    /// Track via both git and backup
    Both,
}

impl Project {
    /// Create a new empty project
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a file to the project
    pub fn add_file(&mut self, path: PathBuf, track: TrackMode) {
        self.files.push(TrackedFile {
            path,
            track,
            last_hash: None,
            last_sync: None,
        });
    }

    /// Remove a file from the project by path
    pub fn remove_file(&mut self, path: &PathBuf) -> bool {
        let initial_len = self.files.len();
        self.files.retain(|f| &f.path != path);
        self.files.len() < initial_len
    }

    /// Get all files in the project
    pub fn list_files(&self) -> &[TrackedFile] {
        &self.files
    }

    /// Set the git remote
    pub fn set_remote(&mut self, remote: String) {
        self.remote = Some(remote);
    }
}
