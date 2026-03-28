//! State types for the GUI
//!
//! Contains enums and structs for managing GUI state.

use dmcore::{FileStatus, RemoteStatus, TrackMode};
use std::collections::HashSet;
use std::path::PathBuf;

/// GUI mode (tab)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Projects, // View projects and their files
    Add,      // Add files to projects
    Restore,  // Restore from backup
}

impl Mode {
    pub fn titles() -> Vec<&'static str> {
        vec!["Projects", "Add Files", "Restore"]
    }

    pub fn index(&self) -> usize {
        match self {
            Mode::Projects => 0,
            Mode::Add => 1,
            Mode::Restore => 2,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Mode::Projects,
            1 => Mode::Add,
            _ => Mode::Restore,
        }
    }

    pub fn next(&self) -> Self {
        Self::from_index((self.index() + 1) % 3)
    }

    pub fn prev(&self) -> Self {
        Self::from_index((self.index() + 2) % 3)
    }
}

/// Restore view state (three-level: projects then commits then files)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RestoreView {
    #[default]
    Projects, // Viewing available backup projects (scanned from disk)
    Commits,  // Viewing commit list for selected project
    Files,    // Viewing files from selected commit
}

/// Purpose of the password prompt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PasswordPurpose {
    #[default]
    Backup,
    Restore,
}

/// Restore destination mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RestoreDestination {
    #[default]
    Original, // Restore to original location
    Custom,   // Restore to custom location (user enters path)
}

/// What to view in restore preview
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RestorePreviewMode {
    #[default]
    FileList, // Show file list
    Backup,   // View backup file content
    Local,    // View local file content
    Diff,     // View diff between backup and local
}

/// Information about a backup project found on disk
#[derive(Debug, Clone)]
pub struct BackupProject {
    pub name: String,
    pub path: PathBuf,
    pub commit_count: usize,
    pub last_backup: Option<String>, // Date of most recent commit
}

/// Restore confirmation dialog state
#[derive(Debug, Clone, Default)]
pub struct RestoreConfirmState {
    pub visible: bool,
    pub destination: RestoreDestination,
    pub custom_path: String,
    pub entering_path: bool,
    pub files_to_restore: Vec<usize>, // Indices into restore_files
    pub will_overwrite: usize,        // Count of files that will be overwritten
    pub selected_idx: usize,          // Selected file in the list
    pub scroll_offset: usize,         // Scroll offset for file list
    pub preview_mode: RestorePreviewMode, // Current view mode
}

/// A displayable file entry
#[derive(Debug, Clone)]
pub struct DisplayFile {
    pub path: String,
    pub abs_path: PathBuf,
    pub status: FileStatus,
    pub size: Option<u64>,
    pub track_mode: TrackMode,
    pub encrypted: bool,
}

/// A displayable project entry
#[derive(Debug, Clone)]
pub struct DisplayProject {
    pub name: String,
    pub file_count: usize,
    pub synced: usize,
    pub drifted: usize,
    pub new_files: usize,
    pub missing: usize,
    pub expanded: bool,
    pub files: Vec<DisplayFile>,
    pub remote_status: Option<RemoteStatus>,
}

/// An item in the project view (either a project header or a file)
#[derive(Debug, Clone)]
pub enum ProjectViewItem {
    Project {
        name: String,
        file_count: usize,
        synced: usize,
        drifted: usize,
        new_files: usize,
        missing: usize,
        expanded: bool,
        remote_status: Option<RemoteStatus>,
    },
    File {
        project_name: String,
        path: String,
        abs_path: PathBuf,
        status: FileStatus,
        size: Option<u64>,
        track_mode: TrackMode,
        encrypted: bool,
    },
}

/// File entry for browsing
#[derive(Debug, Clone)]
pub struct BrowseFile {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub tracked_in: Vec<String>, // List of project names this file is tracked in
}

impl BrowseFile {
    /// Check if this file is tracked in any project
    pub fn is_tracked(&self) -> bool {
        !self.tracked_in.is_empty()
    }
}

/// A file that can be restored from a specific commit
#[derive(Debug, Clone)]
pub struct RestoreFile {
    pub path: PathBuf,         // Original path from backup
    pub restore_path: PathBuf, // Path to restore to (may be remapped)
    pub display_path: String,
    pub hash: String,
    pub size: u64,
    pub exists_locally: bool,
    pub local_differs: bool, // True if local file has different hash
    pub encrypted: bool,     // Whether file was stored encrypted
}

/// File entry for recursive preview
#[derive(Debug, Clone)]
pub struct PreviewFile {
    pub path: PathBuf,
    pub display_path: String,
    pub size: u64,
    pub track_mode: TrackMode,
}

/// State for recursive add preview
#[derive(Debug, Clone)]
pub struct RecursivePreviewState {
    pub source_dir: PathBuf,
    pub preview_files: Vec<PreviewFile>,
    pub selected_files: HashSet<usize>,
    pub selected_idx: usize,
}

/// Result from background operation
pub struct OpResult {
    pub success: bool,
    pub message: String,
}

/// A line in the file viewer with syntax highlighting
#[derive(Debug, Clone)]
pub struct ViewerLine {
    pub spans: Vec<(String, egui::Color32)>, // Text segments with color
    pub file_header: bool, // True if this is a file separator line
}

/// Commit information for restore view
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub date: String,
    pub message: String,
}

impl From<dmcore::CommitInfo> for CommitInfo {
    fn from(c: dmcore::CommitInfo) -> Self {
        Self {
            hash: c.hash,
            short_hash: c.short_hash,
            date: c.date,
            message: c.message,
        }
    }
}
