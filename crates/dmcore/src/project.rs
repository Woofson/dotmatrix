//! Project definition - a logical grouping of scattered files
//!
//! A project is a named collection of files that may be scattered anywhere
//! on disk. Each file can have its own tracking mode (git, backup, or both).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::config::expand_path;

/// A project is a logical grouping of files that may be scattered across disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Optional description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Git remote URL for this project (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    /// Path to the file (may contain ~ for home directory)
    pub path: String,

    /// How this file should be tracked
    #[serde(default)]
    pub track: TrackMode,

    /// Whether this file should be encrypted in backups
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub encrypted: bool,
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

impl TrackMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            TrackMode::Git => "git",
            TrackMode::Backup => "backup",
            TrackMode::Both => "both",
        }
    }
}

impl std::fmt::Display for TrackMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl TrackedFile {
    /// Create a new tracked file with default settings
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            track: TrackMode::default(),
            encrypted: false,
        }
    }

    /// Create a new tracked file with a specific track mode
    pub fn with_mode(path: impl Into<String>, track: TrackMode) -> Self {
        Self {
            path: path.into(),
            track,
            encrypted: false,
        }
    }

    /// Get the expanded absolute path
    pub fn absolute_path(&self) -> PathBuf {
        expand_path(&self.path)
    }

    /// Check if the file exists on disk
    pub fn exists(&self) -> bool {
        self.absolute_path().exists()
    }

    /// Check if this file should be tracked via git
    pub fn uses_git(&self) -> bool {
        matches!(self.track, TrackMode::Git | TrackMode::Both)
    }

    /// Check if this file should be backed up
    pub fn uses_backup(&self) -> bool {
        matches!(self.track, TrackMode::Backup | TrackMode::Both)
    }
}

impl Project {
    /// Create a new empty project
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new project with a description
    pub fn with_description(description: impl Into<String>) -> Self {
        Self {
            description: Some(description.into()),
            ..Self::default()
        }
    }

    /// Add a file to the project
    pub fn add_file(&mut self, file: TrackedFile) -> bool {
        // Check if already tracked
        if self.files.iter().any(|f| f.path == file.path) {
            return false;
        }
        self.files.push(file);
        true
    }

    /// Add a file path with default settings
    pub fn add_path(&mut self, path: impl Into<String>) -> bool {
        self.add_file(TrackedFile::new(path))
    }

    /// Add a file path with a specific track mode
    pub fn add_path_with_mode(&mut self, path: impl Into<String>, track: TrackMode) -> bool {
        self.add_file(TrackedFile::with_mode(path, track))
    }

    /// Remove a file from the project by path
    pub fn remove_file(&mut self, path: &str) -> bool {
        let initial_len = self.files.len();
        self.files.retain(|f| f.path != path);
        self.files.len() < initial_len
    }

    /// Get a file by path
    pub fn get_file(&self, path: &str) -> Option<&TrackedFile> {
        self.files.iter().find(|f| f.path == path)
    }

    /// Get a mutable reference to a file by path
    pub fn get_file_mut(&mut self, path: &str) -> Option<&mut TrackedFile> {
        self.files.iter_mut().find(|f| f.path == path)
    }

    /// Get all files in the project
    pub fn list_files(&self) -> &[TrackedFile] {
        &self.files
    }

    /// Get the number of tracked files
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Set the git remote
    pub fn set_remote(&mut self, remote: impl Into<String>) {
        self.remote = Some(remote.into());
    }

    /// Check if any files use git tracking
    pub fn has_git_files(&self) -> bool {
        self.files.iter().any(|f| f.uses_git())
    }

    /// Check if any files use backup tracking
    pub fn has_backup_files(&self) -> bool {
        self.files.iter().any(|f| f.uses_backup())
    }
}
