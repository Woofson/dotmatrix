//! Application state and logic for the TUI
//!
//! Manages the core state shared between UI rendering and input handling.

use age::secrecy::SecretString;
use dmcore::{
    backup_project_incremental_encrypted_with_message, contract_path, expand_path,
    get_remote_status, hash_file, init_project_repo, project_needs_password, recent_commits,
    retrieve_file_from, retrieve_file_from_encrypted, scan_project, CommitInfo, Config,
    FileStatus, Index, Manifest, ProjectSummary, RemoteStatus, TrackMode,
};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::ListState;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use syntect::highlighting::{Style as SyntectStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::easy::HighlightLines;

/// V2 index format (entries field)
#[derive(Debug, Deserialize)]
struct V2Index {
    #[serde(default)]
    entries: HashMap<PathBuf, V2FileEntry>,
}

/// V2 file entry format
#[derive(Debug, Deserialize)]
struct V2FileEntry {
    hash: String,
    size: u64,
    #[serde(default)]
    encrypted: bool,
}

/// Legacy v1 index format (files field)
#[derive(Debug, Deserialize)]
struct LegacyIndex {
    #[serde(default)]
    files: HashMap<PathBuf, LegacyFileEntry>,
}

/// Legacy v1 file entry format
#[derive(Debug, Deserialize)]
struct LegacyFileEntry {
    #[allow(dead_code)]
    path: PathBuf,
    hash: String,
    #[allow(dead_code)]
    last_modified: u64,
    size: u64,
    #[serde(default)]
    encrypted: bool,
}

/// Spinner frames for busy indicator
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// TUI mode (tab)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
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
}

/// Restore view state (three-level: projects then commits then files)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreView {
    Projects, // Viewing available backup projects (scanned from disk)
    Commits,  // Viewing commit list for selected project
    Files,    // Viewing files from selected commit
}

/// Information about a backup project found on disk
#[derive(Debug, Clone)]
pub struct BackupProject {
    pub name: String,
    pub path: PathBuf,
    pub commit_count: usize,
    pub last_backup: Option<String>, // Date of most recent commit
}

/// Purpose of the password prompt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PasswordPurpose {
    #[default]
    Backup,
    Restore,
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

/// A displayable project entry (name used for target_project cycling)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DisplayProject {
    pub name: String,
    pub file_count: usize,
    pub summary: ProjectSummary,
    pub expanded: bool,
    pub files: Vec<DisplayFile>,
}

/// An item in the project view (either a project header or a file)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ProjectViewItem {
    Project {
        name: String,
        file_count: usize,
        summary: ProjectSummary,
        expanded: bool,
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

/// Result from background operation
pub struct OpResult {
    pub success: bool,
    pub message: String,
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
    pub preview_list_state: ListState,
}

/// A line in the file viewer with syntax highlighting
#[derive(Debug, Clone)]
pub struct ViewerLine {
    pub spans: Vec<(String, Style)>, // Text segments with ratatui styling
    pub file_header: bool,           // True if this is a file separator line
}

/// Application state
pub struct App {
    pub mode: Mode,
    pub config: Config,
    pub manifest: Manifest,
    pub index: Index,

    // Project view state
    pub projects: Vec<DisplayProject>,
    pub visible_items: Vec<ProjectViewItem>,
    pub project_list_state: ListState,
    pub expanded_projects: HashSet<String>,

    // Add mode state
    pub browse_dir: PathBuf,
    pub browse_files: Vec<BrowseFile>,
    pub browse_list_state: ListState,
    pub target_project: Option<String>,

    // Restore state (three-level view: projects -> commits -> files)
    pub restore_view: RestoreView,
    pub backup_projects: Vec<BackupProject>,      // Available backup projects on disk
    pub backup_project_list_state: ListState,     // Selection state for project list
    pub selected_backup_project: Option<String>,  // Currently selected backup project name
    pub commits: Vec<CommitInfo>,
    pub commit_list_state: ListState,
    pub selected_commit: Option<usize>,
    pub restore_files: Vec<RestoreFile>,
    pub restore_list_state: ListState,
    pub restore_selected: HashSet<usize>, // Multi-select for restore

    // UI state
    pub message: Option<(String, bool)>, // (message, is_error)
    pub should_quit: bool,
    pub show_help: bool,
    pub show_about: bool,
    pub help_scroll: u16,

    // Busy state
    pub busy: bool,
    pub busy_message: String,
    pub spinner_frame: usize,
    pub op_receiver: Option<Receiver<OpResult>>,

    // Dirty flags
    pub manifest_dirty: bool,
    pub index_dirty: bool,

    // Project creation state
    pub creating_project: bool,
    pub project_input: String,

    // Delete confirmation state
    pub confirm_delete: bool,
    pub delete_target: Option<String>, // Project name to delete

    // Git remote configuration state
    pub setting_remote: bool,
    pub remote_input: String,

    // Custom commit message state
    pub entering_commit_msg: bool,
    pub commit_msg_input: String,

    // Recursive add state
    pub recursive_preview: Option<RecursivePreviewState>,

    // Track mode for adding files
    #[allow(dead_code)]
    pub default_track_mode: TrackMode,

    // Password prompt state
    pub password_prompt_visible: bool,
    pub password_input: String,
    pub password_purpose: PasswordPurpose,
    pub encryption_password: Option<SecretString>,

    // Git remote status per project
    pub project_remote_status: HashMap<String, RemoteStatus>,

    // File viewer state
    pub viewer_visible: bool,
    pub viewer_content: Vec<ViewerLine>,
    pub viewer_scroll: usize,
    pub viewer_title: String,
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
}

/// File entry for browsing
#[derive(Debug, Clone)]
pub struct BrowseFile {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub is_tracked: bool,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let config = Config::load()?;
        let manifest = Manifest::load()?;
        let index = Index::load()?;

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));

        let mut app = App {
            mode: Mode::Projects,
            config,
            manifest,
            index,
            projects: Vec::new(),
            visible_items: Vec::new(),
            project_list_state: ListState::default(),
            expanded_projects: HashSet::new(),
            browse_dir: home,
            browse_files: Vec::new(),
            browse_list_state: ListState::default(),
            target_project: None,
            restore_view: RestoreView::Projects,
            backup_projects: Vec::new(),
            backup_project_list_state: ListState::default(),
            selected_backup_project: None,
            commits: Vec::new(),
            commit_list_state: ListState::default(),
            selected_commit: None,
            restore_files: Vec::new(),
            restore_list_state: ListState::default(),
            restore_selected: HashSet::new(),
            message: None,
            should_quit: false,
            show_help: false,
            show_about: false,
            help_scroll: 0,
            busy: false,
            busy_message: String::new(),
            spinner_frame: 0,
            op_receiver: None,
            manifest_dirty: false,
            index_dirty: false,
            creating_project: false,
            project_input: String::new(),
            confirm_delete: false,
            delete_target: None,
            setting_remote: false,
            remote_input: String::new(),
            entering_commit_msg: false,
            commit_msg_input: String::new(),
            recursive_preview: None,
            default_track_mode: TrackMode::Both,
            password_prompt_visible: false,
            password_input: String::new(),
            password_purpose: PasswordPurpose::default(),
            encryption_password: None,
            project_remote_status: HashMap::new(),
            viewer_visible: false,
            viewer_content: Vec::new(),
            viewer_scroll: 0,
            viewer_title: String::new(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        };

        app.refresh_projects();
        app.refresh_browse();
        app.scan_backup_projects();

        Ok(app)
    }

    /// Scan the data directory for all available backup projects
    pub fn scan_backup_projects(&mut self) {
        self.backup_projects.clear();

        // Get the projects directory
        let projects_dir = match self.config.data_dir() {
            Ok(d) => d.join("projects"),
            Err(_) => return,
        };

        if !projects_dir.exists() {
            return;
        }

        // Scan for directories with .git
        if let Ok(entries) = fs::read_dir(&projects_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    let git_dir = path.join(".git");
                    if git_dir.exists() {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();

                        // Get commit count and last backup date
                        let (commit_count, last_backup) = self.get_project_backup_info(&path);

                        self.backup_projects.push(BackupProject {
                            name,
                            path,
                            commit_count,
                            last_backup,
                        });
                    }
                }
            }
        }

        // Sort by name
        self.backup_projects.sort_by(|a, b| a.name.cmp(&b.name));

        // Select first if available
        if !self.backup_projects.is_empty() {
            self.backup_project_list_state.select(Some(0));
        }
    }

    /// Get backup info for a project (commit count and last backup date)
    fn get_project_backup_info(&self, project_dir: &Path) -> (usize, Option<String>) {
        // Get commit count
        let count_output = std::process::Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .current_dir(project_dir)
            .output();

        let commit_count = count_output
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
            .unwrap_or(0);

        // Get last commit date
        let date_output = std::process::Command::new("git")
            .args(["log", "-1", "--format=%ai"])
            .current_dir(project_dir)
            .output();

        let last_backup = date_output
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                let date = String::from_utf8_lossy(&o.stdout).trim().to_string();
                // Truncate to just date and time (no timezone)
                if date.len() > 19 {
                    date[..19].to_string()
                } else {
                    date
                }
            });

        (commit_count, last_backup)
    }

    /// Load git commit history for a specific backup project
    pub fn load_commits_for_project(&mut self, project_name: &str) {
        self.commits.clear();

        if let Ok(project_dir) = self.config.project_dir(project_name) {
            if let Ok(commits) = recent_commits(&project_dir, 100) {
                self.commits = commits;
            }
        }

        // Select first commit if available
        if !self.commits.is_empty() {
            self.commit_list_state.select(Some(0));
        }
    }

    /// Select a backup project and load its commits
    pub fn select_backup_project(&mut self) {
        if let Some(idx) = self.backup_project_list_state.selected() {
            if let Some(project) = self.backup_projects.get(idx) {
                let name = project.name.clone();
                self.selected_backup_project = Some(name.clone());
                self.load_commits_for_project(&name);
                self.restore_view = RestoreView::Commits;
            }
        }
    }

    /// Go back from commits to project list
    pub fn back_to_backup_projects(&mut self) {
        self.restore_view = RestoreView::Projects;
        self.selected_backup_project = None;
        self.commits.clear();
        self.selected_commit = None;
    }

    /// Refresh the projects list and build visible items
    pub fn refresh_projects(&mut self) {
        self.projects.clear();
        self.visible_items.clear();

        let mut names: Vec<_> = self.manifest.projects.keys().cloned().collect();
        names.sort();

        for name in names {
            if let Some(project) = self.manifest.get_project(&name) {
                let results = scan_project(project, &self.index);
                let summary = ProjectSummary::from_results(&results);
                let expanded = self.expanded_projects.contains(&name);

                // Build file list with encryption status
                let files: Vec<DisplayFile> = results
                    .iter()
                    .zip(project.files.iter())
                    .map(|(r, tracked)| DisplayFile {
                        path: r.path.clone(),
                        abs_path: expand_path(&r.path),
                        status: r.status,
                        size: r.current_size,
                        track_mode: r.track_mode,
                        encrypted: tracked.encrypted,
                    })
                    .collect();

                // Add project header to visible items
                self.visible_items.push(ProjectViewItem::Project {
                    name: name.clone(),
                    file_count: project.file_count(),
                    summary: summary.clone(),
                    expanded,
                });

                // Add files if expanded
                if expanded {
                    for file in &files {
                        self.visible_items.push(ProjectViewItem::File {
                            project_name: name.clone(),
                            path: file.path.clone(),
                            abs_path: file.abs_path.clone(),
                            status: file.status,
                            size: file.size,
                            track_mode: file.track_mode,
                            encrypted: file.encrypted,
                        });
                    }
                }

                self.projects.push(DisplayProject {
                    name: name.clone(),
                    file_count: project.file_count(),
                    summary,
                    expanded,
                    files,
                });
            }
        }

        // Select first item if nothing selected
        if self.project_list_state.selected().is_none() && !self.visible_items.is_empty() {
            self.project_list_state.select(Some(0));
        }
    }

    /// Refresh the browse file list
    pub fn refresh_browse(&mut self) {
        self.browse_files.clear();

        // Add parent directory entry
        if self.browse_dir.parent().is_some() {
            self.browse_files.push(BrowseFile {
                path: self.browse_dir.join(".."),
                name: "..".to_string(),
                is_dir: true,
                size: None,
                is_tracked: false,
            });
        }

        // Read directory contents
        if let Ok(entries) = std::fs::read_dir(&self.browse_dir) {
            let mut files: Vec<BrowseFile> = entries
                .filter_map(|e| e.ok())
                .filter_map(|entry| {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();

                    // Show dotfiles - they're what we want to track!
                    // Only skip . and ..
                    if name == "." || name == ".." {
                        return None;
                    }

                    let is_dir = path.is_dir();
                    let size = if is_dir {
                        None
                    } else {
                        std::fs::metadata(&path).ok().map(|m| m.len())
                    };

                    // Check if tracked in any project
                    let contracted = contract_path(&path);
                    let is_tracked = self
                        .manifest
                        .projects
                        .values()
                        .any(|p| p.files.iter().any(|f| f.path == contracted));

                    Some(BrowseFile {
                        path,
                        name,
                        is_dir,
                        size,
                        is_tracked,
                    })
                })
                .collect();

            // Sort: directories first, then by name
            files.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            });

            self.browse_files.extend(files);
        }

        // Reset selection
        if !self.browse_files.is_empty() {
            self.browse_list_state.select(Some(0));
        }
    }

    /// Navigate into a directory
    pub fn enter_directory(&mut self, path: &PathBuf) {
        if path.is_dir() {
            let previous_dir = self.browse_dir.clone();
            self.browse_dir = if path.ends_with("..") {
                self.browse_dir
                    .parent()
                    .unwrap_or(&self.browse_dir)
                    .to_path_buf()
            } else {
                path.clone()
            };
            self.refresh_browse();

            // If we went up to parent, find and select the directory we came from
            if path.ends_with("..") {
                if let Some(idx) = self
                    .browse_files
                    .iter()
                    .position(|f| f.path == previous_dir)
                {
                    self.browse_list_state.select(Some(idx));
                }
            }
        }
    }

    /// Get the currently selected item in projects view
    pub fn selected_item(&self) -> Option<&ProjectViewItem> {
        self.project_list_state
            .selected()
            .and_then(|i| self.visible_items.get(i))
    }

    /// Get project name for the current selection (either project or file's parent project)
    pub fn selected_project_name(&self) -> Option<String> {
        match self.selected_item()? {
            ProjectViewItem::Project { name, .. } => Some(name.clone()),
            ProjectViewItem::File { project_name, .. } => Some(project_name.clone()),
        }
    }

    /// Toggle expansion for selected project (or parent project if file selected)
    pub fn toggle_selected_project(&mut self) {
        let name = match self.selected_project_name() {
            Some(n) => n,
            None => return,
        };

        if self.expanded_projects.contains(&name) {
            self.expanded_projects.remove(&name);
        } else {
            self.expanded_projects.insert(name);
        }

        // Remember current selection index
        let current_idx = self.project_list_state.selected().unwrap_or(0);
        self.refresh_projects();

        // Try to keep selection at same index, clamped to new list size
        let new_idx = current_idx.min(self.visible_items.len().saturating_sub(1));
        if !self.visible_items.is_empty() {
            self.project_list_state.select(Some(new_idx));
        }
    }

    /// Collapse selected project (if it's expanded)
    pub fn collapse_selected_project(&mut self) {
        let name = match self.selected_project_name() {
            Some(n) => n,
            None => return,
        };

        if self.expanded_projects.contains(&name) {
            self.expanded_projects.remove(&name);
            let current_idx = self.project_list_state.selected().unwrap_or(0);
            self.refresh_projects();
            // Find the project header and select it
            if let Some(idx) = self.visible_items.iter().position(|item| {
                matches!(item, ProjectViewItem::Project { name: n, .. } if n == &name)
            }) {
                self.project_list_state.select(Some(idx));
            } else {
                let new_idx = current_idx.min(self.visible_items.len().saturating_sub(1));
                self.project_list_state.select(Some(new_idx));
            }
        }
    }

    /// Toggle encryption for the selected file
    pub fn toggle_encryption(&mut self) {
        let (project_name, file_path) = match self.selected_item() {
            Some(ProjectViewItem::File {
                project_name,
                path,
                ..
            }) => (project_name.clone(), path.clone()),
            _ => {
                self.message = Some(("Select a file to toggle encryption".to_string(), true));
                return;
            }
        };

        if let Some(project) = self.manifest.get_project_mut(&project_name) {
            if let Some(file) = project.files.iter_mut().find(|f| f.path == file_path) {
                file.encrypted = !file.encrypted;
                self.manifest_dirty = true;
                let state = if file.encrypted { "enabled" } else { "disabled" };
                self.message = Some((format!("Encryption {} (saves on exit)", state), false));
                self.refresh_projects();
            }
        }
    }

    /// Toggle encryption for all files in the selected project
    pub fn toggle_project_encryption(&mut self) {
        let project_name = match self.selected_project_name() {
            Some(name) => name,
            None => {
                self.message = Some(("Select a project to toggle encryption".to_string(), true));
                return;
            }
        };

        if let Some(project) = self.manifest.get_project_mut(&project_name) {
            // Check current state - if any file is unencrypted, encrypt all; else decrypt all
            let any_unencrypted = project.files.iter().any(|f| !f.encrypted);
            let new_state = any_unencrypted;
            let count = project.files.len();

            for file in &mut project.files {
                file.encrypted = new_state;
            }

            self.manifest_dirty = true;
            let state = if new_state { "enabled" } else { "disabled" };
            self.message = Some((
                format!("Encryption {} for {} files (saves on exit)", state, count),
                false,
            ));
            self.refresh_projects();
        }
    }

    /// Toggle track mode for the selected file (Git -> Backup -> Both -> Git)
    pub fn toggle_track_mode(&mut self) {
        let (project_name, file_path) = match self.selected_item() {
            Some(ProjectViewItem::File {
                project_name,
                path,
                ..
            }) => (project_name.clone(), path.clone()),
            _ => {
                self.message = Some(("Select a file to toggle track mode".to_string(), true));
                return;
            }
        };

        if let Some(project) = self.manifest.get_project_mut(&project_name) {
            if let Some(file) = project.files.iter_mut().find(|f| f.path == file_path) {
                file.track = match file.track {
                    TrackMode::Git => TrackMode::Backup,
                    TrackMode::Backup => TrackMode::Both,
                    TrackMode::Both => TrackMode::Git,
                };
                self.manifest_dirty = true;
                let mode_name = match file.track {
                    TrackMode::Git => "Git",
                    TrackMode::Backup => "Backup",
                    TrackMode::Both => "Both",
                };
                self.message = Some((format!("Track mode: {} (saves on exit)", mode_name), false));
                self.refresh_projects();
            }
        }
    }

    /// Add a file to the target project
    pub fn add_file_to_project(&mut self, path: &PathBuf) -> bool {
        let project_name = match &self.target_project {
            Some(name) => name.clone(),
            None => {
                // Use first project or show error
                if let Some(p) = self.projects.first() {
                    p.name.clone()
                } else {
                    self.message = Some((
                        "No project selected. Create one first.".to_string(),
                        true,
                    ));
                    return false;
                }
            }
        };

        if let Some(project) = self.manifest.get_project_mut(&project_name) {
            let contracted = contract_path(path);
            if project.add_path_with_mode(&contracted, self.default_track_mode) {
                self.manifest_dirty = true;
                self.message = Some((
                    format!("Added {} to {}", contracted, project_name),
                    false,
                ));
                self.refresh_projects();
                // Preserve cursor position when refreshing browse
                let saved_selection = self.browse_list_state.selected();
                self.refresh_browse();
                if let Some(idx) = saved_selection {
                    let max_idx = self.browse_files.len().saturating_sub(1);
                    self.browse_list_state.select(Some(idx.min(max_idx)));
                }
                return true;
            } else {
                self.message = Some(("File already tracked".to_string(), true));
            }
        }
        false
    }

    /// Remove a file from all projects (untrack)
    pub fn untrack_file(&mut self, path: &PathBuf) -> bool {
        let contracted = contract_path(path);
        let mut removed_from = Vec::new();

        // Check all projects for this file
        for (name, project) in self.manifest.projects.iter_mut() {
            if let Some(pos) = project.files.iter().position(|f| f.path == contracted) {
                project.files.remove(pos);
                removed_from.push(name.clone());
            }
        }

        if !removed_from.is_empty() {
            self.manifest_dirty = true;
            self.message = Some((
                format!("Removed {} from {}", contracted, removed_from.join(", ")),
                false,
            ));
            self.refresh_projects();
            // Preserve cursor position when refreshing browse
            let saved_selection = self.browse_list_state.selected();
            self.refresh_browse();
            if let Some(idx) = saved_selection {
                let max_idx = self.browse_files.len().saturating_sub(1);
                self.browse_list_state.select(Some(idx.min(max_idx)));
            }
            return true;
        }

        self.message = Some(("File not tracked".to_string(), true));
        false
    }

    /// Backup the selected project
    pub fn backup_project(&mut self) {
        let project_name = match self.selected_project_name() {
            Some(name) => name,
            None => {
                self.message = Some(("No project selected".to_string(), true));
                return;
            }
        };

        let project = match self.manifest.get_project(&project_name) {
            Some(p) => p.clone(),
            None => return,
        };

        // Check if project needs encryption password
        if project_needs_password(&project) && self.encryption_password.is_none() {
            self.show_password_prompt(PasswordPurpose::Backup);
            return;
        }

        // Perform backup with or without password
        self.backup_project_with_password(project_name, project);
    }

    /// Backup with custom commit message prompt
    pub fn backup_project_with_message(&mut self) {
        let project_name = match self.selected_project_name() {
            Some(name) => name,
            None => {
                self.message = Some(("No project selected".to_string(), true));
                return;
            }
        };

        let project = match self.manifest.get_project(&project_name) {
            Some(p) => p.clone(),
            None => return,
        };

        // Check if project needs encryption password first
        if project_needs_password(&project) && self.encryption_password.is_none() {
            self.show_password_prompt(PasswordPurpose::Backup);
            return;
        }

        // Show commit message prompt
        self.entering_commit_msg = true;
        self.commit_msg_input.clear();
    }

    /// Cancel commit message prompt
    pub fn cancel_commit_msg(&mut self) {
        self.entering_commit_msg = false;
        self.commit_msg_input.clear();
    }

    /// Confirm commit message and run backup
    pub fn confirm_commit_msg(&mut self) {
        let custom_msg = self.commit_msg_input.trim().to_string();
        self.entering_commit_msg = false;

        let project_name = match self.selected_project_name() {
            Some(name) => name,
            None => {
                self.commit_msg_input.clear();
                return;
            }
        };

        let project = match self.manifest.get_project(&project_name) {
            Some(p) => p.clone(),
            None => {
                self.commit_msg_input.clear();
                return;
            }
        };

        self.commit_msg_input.clear();

        // Run backup with custom message
        let msg = if custom_msg.is_empty() {
            None
        } else {
            Some(custom_msg)
        };
        self.backup_project_internal(project_name, project, msg);
    }

    /// Internal backup with optional password and custom message
    fn backup_project_with_password(&mut self, project_name: String, project: dmcore::Project) {
        self.backup_project_internal(project_name, project, None);
    }

    /// Internal backup implementation
    fn backup_project_internal(
        &mut self,
        project_name: String,
        project: dmcore::Project,
        custom_message: Option<String>,
    ) {
        let config = self.config.clone();
        let password = self.encryption_password.clone();
        let name = project_name.clone();

        let (tx, rx) = mpsc::channel();
        self.op_receiver = Some(rx);
        self.busy = true;
        self.busy_message = format!("Backing up {}...", project_name);

        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<String> {
                // Initialize project-specific git repo
                init_project_repo(&config, &name)?;

                // Backup with encryption support to project-specific store
                let result = backup_project_incremental_encrypted_with_message(
                    &config,
                    &name,
                    &project,
                    password.as_ref(),
                    custom_message.as_deref(),
                )?;

                Ok(format!(
                    "Backed up {} files ({} new, {} unchanged)",
                    result.backed_up + result.unchanged,
                    result.backed_up,
                    result.unchanged
                ))
            })();

            let op_result = match result {
                Ok(msg) => OpResult {
                    success: true,
                    message: msg,
                },
                Err(e) => OpResult {
                    success: false,
                    message: e.to_string(),
                },
            };

            let _ = tx.send(op_result);
        });
    }

    /// Check if selected project needs a password
    pub fn selected_project_needs_password(&self) -> bool {
        let project_name = match self.selected_project_name() {
            Some(name) => name,
            None => return false,
        };
        match self.manifest.get_project(&project_name) {
            Some(p) => project_needs_password(p),
            None => false,
        }
    }

    /// Show password prompt for backup or restore
    pub fn show_password_prompt(&mut self, purpose: PasswordPurpose) {
        self.password_prompt_visible = true;
        self.password_input.clear();
        self.password_purpose = purpose;
    }

    /// Cancel password prompt
    pub fn cancel_password(&mut self) {
        self.password_prompt_visible = false;
        self.password_input.clear();
    }

    /// Confirm password entry
    pub fn confirm_password(&mut self) {
        if self.password_input.is_empty() {
            self.message = Some(("Password cannot be empty".to_string(), true));
            return;
        }

        // Store the password
        self.encryption_password = Some(SecretString::from(self.password_input.clone()));
        self.password_input.clear();
        self.password_prompt_visible = false;

        // Continue with the operation
        match self.password_purpose {
            PasswordPurpose::Backup => {
                // Re-trigger backup now that we have the password
                let project_name = match self.selected_project_name() {
                    Some(name) => name,
                    None => return,
                };
                let project = match self.manifest.get_project(&project_name) {
                    Some(p) => p.clone(),
                    None => return,
                };
                self.backup_project_with_password(project_name, project);
            }
            PasswordPurpose::Restore => {
                // Re-trigger restore
                self.perform_restore_with_password();
            }
        }
    }

    /// Refresh git remote status for all projects
    pub fn refresh_remote_status(&mut self) {
        self.project_remote_status.clear();

        // Each project has its own git repo
        for name in self.manifest.projects.keys() {
            if let Ok(project_dir) = self.config.project_dir(name) {
                if dmcore::is_git_repo(&project_dir) {
                    if let Ok(status) = get_remote_status(&project_dir) {
                        self.project_remote_status.insert(name.clone(), status);
                    }
                }
            }
        }

        self.message = Some(("Git status refreshed".to_string(), false));
    }

    /// Get remote status for a project
    pub fn get_project_remote_status(&self, project_name: &str) -> Option<&RemoteStatus> {
        self.project_remote_status.get(project_name)
    }

    /// Poll for background operation completion
    pub fn poll_operation(&mut self) {
        if let Some(ref rx) = self.op_receiver {
            match rx.try_recv() {
                Ok(result) => {
                    self.busy = false;
                    self.op_receiver = None;
                    self.message = Some((result.message, !result.success));

                    // Reload project-specific index after backup
                    if result.success {
                        // Load index for the selected project
                        if let Some(name) = self.selected_project_name() {
                            if let Ok(index) = Index::load_for_project(&self.config, &name) {
                                self.index = index;
                            }
                        }
                        self.refresh_projects();
                        // Refresh backup projects list in case new backup was created
                        self.scan_backup_projects();
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Still running, advance spinner
                    self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.busy = false;
                    self.op_receiver = None;
                }
            }
        }
    }

    /// Sync a project (mark all as synced)
    pub fn sync_project(&mut self) {
        let project_name = match self.selected_project_name() {
            Some(name) => name,
            None => {
                self.message = Some(("No project selected".to_string(), true));
                return;
            }
        };

        let project = match self.manifest.get_project(&project_name) {
            Some(p) => p,
            None => return,
        };

        let mut synced = 0;
        for file in &project.files {
            let abs_path = file.absolute_path();
            if abs_path.exists() {
                if let Ok(hash) = hash_file(&abs_path) {
                    if let Ok((size, modified)) = dmcore::file_metadata(&abs_path) {
                        let entry = dmcore::FileEntry::with_sync_now(hash, size, modified);
                        self.index.upsert(abs_path, entry);
                        synced += 1;
                    }
                }
            }
        }

        if synced > 0 {
            self.index_dirty = true;
            self.message = Some((format!("Synced {} files", synced), false));
            self.refresh_projects();
        } else {
            self.message = Some(("Nothing to sync".to_string(), false));
        }
    }

    /// Select a commit and load its files for restore
    pub fn select_commit(&mut self) {
        if let Some(i) = self.commit_list_state.selected() {
            if i < self.commits.len() {
                self.selected_commit = Some(i);
                self.load_commit_files(&self.commits[i].hash.clone());
                self.restore_view = RestoreView::Files;
                self.restore_selected.clear();
                if !self.restore_files.is_empty() {
                    self.restore_list_state.select(Some(0));
                }
            }
        }
    }

    /// Go back to commit list from file view
    pub fn back_to_commits(&mut self) {
        self.restore_view = RestoreView::Commits;
        self.restore_files.clear();
        self.restore_selected.clear();
        // Restore selection to the commit we were viewing
        if let Some(idx) = self.selected_commit {
            self.commit_list_state.select(Some(idx));
        }
        self.selected_commit = None;
    }

    /// Load files from a specific commit's index
    pub fn load_commit_files(&mut self, commit_hash: &str) {
        self.restore_files.clear();

        // Get project directory for the selected backup project
        let project_name = match &self.selected_backup_project {
            Some(n) => n.clone(),
            None => return,
        };

        let project_dir = match self.config.project_dir(&project_name) {
            Ok(d) => d,
            Err(_) => return,
        };

        // Get index.json content at this commit
        let output = std::process::Command::new("git")
            .args(["show", &format!("{}:index.json", commit_hash)])
            .current_dir(&project_dir)
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let content = String::from_utf8_lossy(&output.stdout);

                // Try V2 format first (entries field)
                if let Ok(v2_index) = serde_json::from_str::<V2Index>(&content) {
                    if !v2_index.entries.is_empty() {
                        for (path, entry) in v2_index.entries {
                            self.add_restore_file(path, entry.hash, entry.size, entry.encrypted);
                        }
                        self.restore_files
                            .sort_by(|a, b| a.display_path.cmp(&b.display_path));
                        return;
                    }
                }

                // Fall back to legacy v1 format (files field)
                if let Ok(legacy_index) = serde_json::from_str::<LegacyIndex>(&content) {
                    for (path, entry) in legacy_index.files {
                        self.add_restore_file(path, entry.hash, entry.size, entry.encrypted);
                    }
                    self.restore_files
                        .sort_by(|a, b| a.display_path.cmp(&b.display_path));
                }
            } else {
                self.message = Some(("Failed to load commit index".to_string(), true));
            }
        }
    }

    /// Helper to add a file to the restore list
    fn add_restore_file(&mut self, path: PathBuf, hash: String, size: u64, encrypted: bool) {
        // Remap path to current home directory if needed
        let restore_path = Self::remap_path_to_current_home(&path);

        let display_path = if let Some(home) = dirs::home_dir() {
            if let Ok(rel) = restore_path.strip_prefix(home) {
                format!("~/{}", rel.display())
            } else {
                restore_path.display().to_string()
            }
        } else {
            restore_path.display().to_string()
        };

        // Check if file exists locally and if it differs (using restore_path)
        let exists_locally = restore_path.exists();
        let local_differs = if exists_locally {
            hash_file(&restore_path)
                .map(|h| h != hash)
                .unwrap_or(true)
        } else {
            true
        };

        self.restore_files.push(RestoreFile {
            path,
            restore_path,
            display_path,
            hash,
            size,
            exists_locally,
            local_differs,
            encrypted,
        });
    }

    /// Remap a path from an old home directory to the current one
    ///
    /// This handles cases where backups were made on a different machine
    /// or with a different username.
    fn remap_path_to_current_home(path: &PathBuf) -> PathBuf {
        let current_home = match dirs::home_dir() {
            Some(h) => h,
            None => return path.clone(),
        };

        let path_str = path.to_string_lossy();

        // Handle ~ prefix (expand to current home)
        if path_str.starts_with("~/") {
            return current_home.join(&path_str[2..]);
        }

        // Handle /home/username/ pattern
        if path_str.starts_with("/home/") {
            // Extract the username from the path
            if let Some(rest) = path_str.strip_prefix("/home/") {
                if let Some(slash_pos) = rest.find('/') {
                    let old_user = &rest[..slash_pos];
                    let rel_path = &rest[slash_pos + 1..];

                    // Get current username from home dir
                    if let Some(current_user) = current_home.file_name() {
                        let current_user = current_user.to_string_lossy();
                        if old_user != current_user.as_ref() {
                            // Remap to current home
                            return current_home.join(rel_path);
                        }
                    }
                }
            }
        }

        // Handle /Users/username/ pattern (macOS)
        if path_str.starts_with("/Users/") {
            if let Some(rest) = path_str.strip_prefix("/Users/") {
                if let Some(slash_pos) = rest.find('/') {
                    let old_user = &rest[..slash_pos];
                    let rel_path = &rest[slash_pos + 1..];

                    if let Some(current_user) = current_home.file_name() {
                        let current_user = current_user.to_string_lossy();
                        if old_user != current_user.as_ref() {
                            return current_home.join(rel_path);
                        }
                    }
                }
            }
        }

        // No remapping needed
        path.clone()
    }

    /// Restore selected files from the selected commit
    pub fn perform_restore(&mut self) {
        let indices: Vec<usize> = if self.restore_selected.is_empty() {
            // If nothing selected, restore the currently highlighted file
            self.restore_list_state.selected().into_iter().collect()
        } else {
            self.restore_selected.iter().cloned().collect()
        };

        if indices.is_empty() {
            self.message = Some(("No files selected for restore".to_string(), true));
            return;
        }

        // Check if any selected files are encrypted
        let needs_password = indices
            .iter()
            .any(|&i| self.restore_files.get(i).map(|f| f.encrypted).unwrap_or(false));

        if needs_password && self.encryption_password.is_none() {
            self.show_password_prompt(PasswordPurpose::Restore);
            return;
        }

        self.perform_restore_with_password();
    }

    /// Internal restore with optional password
    fn perform_restore_with_password(&mut self) {
        let indices: Vec<usize> = if self.restore_selected.is_empty() {
            self.restore_list_state.selected().into_iter().collect()
        } else {
            self.restore_selected.iter().cloned().collect()
        };

        // Get project-specific store directory from selected backup project
        let project_name = match &self.selected_backup_project {
            Some(n) => n.clone(),
            None => {
                self.message = Some(("No backup project selected".to_string(), true));
                return;
            }
        };

        let store_dir = match self.config.project_store_dir(&project_name) {
            Ok(d) => d,
            Err(e) => {
                self.message = Some((format!("Failed to get store dir: {}", e), true));
                return;
            }
        };

        let mut restored = 0;
        let mut errors = 0;

        for i in indices {
            if i >= self.restore_files.len() {
                continue;
            }

            let file = &self.restore_files[i];

            // Use encrypted or regular retrieve from project-specific store
            // Use restore_path which may be remapped to current home directory
            let result = if file.encrypted {
                retrieve_file_from_encrypted(
                    &store_dir,
                    &file.hash,
                    &file.restore_path,
                    self.encryption_password.as_ref(),
                    true,
                )
            } else {
                retrieve_file_from(&store_dir, &file.hash, &file.restore_path)
            };

            match result {
                Ok(true) => restored += 1,
                Ok(false) => errors += 1,
                Err(_) => errors += 1,
            }
        }

        self.restore_selected.clear();

        if errors > 0 {
            self.message = Some((
                format!("Restored {} files ({} errors)", restored, errors),
                true,
            ));
        } else {
            self.message = Some((format!("Restored {} files", restored), false));
        }

        // Refresh to update local_differs status
        if let Some(commit_idx) = self.selected_commit {
            let hash = self.commits[commit_idx].hash.clone();
            self.load_commit_files(&hash);
        }
    }

    /// Toggle file selection for restore
    pub fn toggle_restore_select(&mut self) {
        if let Some(i) = self.restore_list_state.selected() {
            if self.restore_selected.contains(&i) {
                self.restore_selected.remove(&i);
            } else {
                self.restore_selected.insert(i);
            }
        }
    }

    /// Save dirty state (called on exit)
    pub fn save_if_dirty(&mut self) -> anyhow::Result<()> {
        if self.manifest_dirty {
            self.manifest.save()?;
            self.manifest_dirty = false;
        }
        if self.index_dirty {
            self.index.save()?;
            self.index_dirty = false;
        }
        Ok(())
    }

    /// Save immediately and reload (for live changes like encryption settings)
    pub fn save_and_reload(&mut self) {
        let mut saved = Vec::new();

        if self.manifest_dirty {
            match self.manifest.save() {
                Ok(_) => {
                    self.manifest_dirty = false;
                    saved.push("manifest");
                }
                Err(e) => {
                    self.message = Some((format!("Failed to save manifest: {}", e), true));
                    return;
                }
            }
        }

        if self.index_dirty {
            match self.index.save() {
                Ok(_) => {
                    self.index_dirty = false;
                    saved.push("index");
                }
                Err(e) => {
                    self.message = Some((format!("Failed to save index: {}", e), true));
                    return;
                }
            }
        }

        if saved.is_empty() {
            self.message = Some(("Nothing to save".to_string(), false));
        } else {
            // Reload from disk
            if let Ok(manifest) = dmcore::Manifest::load() {
                self.manifest = manifest;
            }
            if let Ok(index) = Index::load() {
                self.index = index;
            }
            self.scan_backup_projects();
            self.refresh_projects();
            self.message = Some((format!("Saved {} and reloaded", saved.join(" and ")), false));
        }
    }

    /// Get the current spinner frame
    pub fn spinner(&self) -> &'static str {
        SPINNER_FRAMES[self.spinner_frame]
    }

    /// Start creating a new project
    pub fn start_create_project(&mut self) {
        self.creating_project = true;
        self.project_input.clear();
    }

    /// Cancel project creation
    pub fn cancel_create_project(&mut self) {
        self.creating_project = false;
        self.project_input.clear();
    }

    /// Confirm and create the new project
    pub fn confirm_create_project(&mut self) {
        let name = self.project_input.trim().to_string();
        if name.is_empty() {
            self.message = Some(("Project name cannot be empty".to_string(), true));
            return;
        }

        // Check if project already exists
        if self.manifest.projects.contains_key(&name) {
            self.message = Some((format!("Project '{}' already exists", name), true));
            return;
        }

        // Create new project
        use dmcore::Project;
        let project = Project::new();
        self.manifest.projects.insert(name.clone(), project);
        self.manifest_dirty = true;

        self.message = Some((format!("Created project '{}'", name), false));
        self.creating_project = false;
        self.project_input.clear();

        // Set as target project and refresh
        self.target_project = Some(name);
        self.refresh_projects();
    }

    /// Start setting git remote for selected project
    pub fn start_set_remote(&mut self) {
        let project_name = match self.selected_project_name() {
            Some(n) => n,
            None => {
                self.message = Some(("No project selected".to_string(), true));
                return;
            }
        };

        // Pre-fill with existing remote if any
        if let Some(project) = self.manifest.get_project(&project_name) {
            self.remote_input = project.remote.clone().unwrap_or_default();
        } else {
            self.remote_input.clear();
        }
        self.setting_remote = true;
    }

    /// Cancel remote configuration
    pub fn cancel_set_remote(&mut self) {
        self.setting_remote = false;
        self.remote_input.clear();
    }

    /// Confirm and set the git remote URL
    pub fn confirm_set_remote(&mut self) {
        let url = self.remote_input.trim().to_string();
        let project_name = match self.selected_project_name() {
            Some(n) => n,
            None => {
                self.setting_remote = false;
                return;
            }
        };

        // Update manifest with remote URL
        if let Some(project) = self.manifest.get_project_mut(&project_name) {
            if url.is_empty() {
                project.remote = None;
                self.message = Some(("Remote cleared".to_string(), false));
            } else {
                project.remote = Some(url.clone());
                self.manifest_dirty = true;
                self.message = Some((format!("Remote set to: {}", url), false));
            }
        }

        // Also set it in the project's git repo if it exists
        if let Ok(project_dir) = self.config.project_dir(&project_name) {
            if dmcore::is_git_repo(&project_dir) && !url.is_empty() {
                if let Err(e) = dmcore::set_remote_url(&project_dir, &url) {
                    self.message = Some((format!("Warning: {}", e), true));
                }
            }
        }

        self.setting_remote = false;
        self.remote_input.clear();
        self.refresh_remote_status();
    }

    /// Show delete confirmation for the selected project
    pub fn start_delete_project(&mut self) {
        let name = match self.selected_project_name() {
            Some(n) => n,
            None => {
                self.message = Some(("No project selected".to_string(), true));
                return;
            }
        };

        self.delete_target = Some(name);
        self.confirm_delete = true;
    }

    /// Cancel delete confirmation
    pub fn cancel_delete(&mut self) {
        self.confirm_delete = false;
        self.delete_target = None;
    }

    /// Confirm and delete the project
    pub fn confirm_delete_project(&mut self) {
        let name = match self.delete_target.take() {
            Some(n) => n,
            None => return,
        };
        self.confirm_delete = false;

        self.manifest.projects.remove(&name);
        self.manifest_dirty = true;
        self.expanded_projects.remove(&name);

        self.message = Some((format!("Deleted project '{}'", name), false));

        // Clear target if it was the deleted project
        if self.target_project.as_ref() == Some(&name) {
            self.target_project = None;
        }

        self.refresh_projects();

        // Select first item if any remain
        if !self.visible_items.is_empty() {
            self.project_list_state.select(Some(0));
        }
    }

    /// Cycle track mode for adding files (Git -> Backup -> Both -> Git)
    pub fn cycle_add_track_mode(&mut self) {
        self.default_track_mode = match self.default_track_mode {
            TrackMode::Git => TrackMode::Backup,
            TrackMode::Backup => TrackMode::Both,
            TrackMode::Both => TrackMode::Git,
        };
        let mode_name = match self.default_track_mode {
            TrackMode::Git => "Git",
            TrackMode::Backup => "Backup",
            TrackMode::Both => "Both",
        };
        self.message = Some((format!("Track mode: {}", mode_name), false));
    }

    /// Cycle target project for Add mode
    pub fn cycle_target_project(&mut self) {
        if self.projects.is_empty() {
            self.message = Some((
                "No projects. Press 'n' to create one.".to_string(),
                true,
            ));
            return;
        }

        let current_idx = self
            .target_project
            .as_ref()
            .and_then(|name| self.projects.iter().position(|p| &p.name == name))
            .unwrap_or(0);

        let next_idx = (current_idx + 1) % self.projects.len();
        self.target_project = Some(self.projects[next_idx].name.clone());
        self.message = Some((format!("Target: {}", self.projects[next_idx].name), false));
    }

    /// Start recursive preview for the selected directory
    pub fn start_recursive_preview(&mut self) {
        if self.mode != Mode::Add {
            return;
        }

        let idx = match self.browse_list_state.selected() {
            Some(i) => i,
            None => {
                self.message = Some(("No directory selected".to_string(), true));
                return;
            }
        };

        let file = match self.browse_files.get(idx) {
            Some(f) => f,
            None => return,
        };

        if !file.is_dir {
            self.message = Some((
                "Select a directory to add recursively".to_string(),
                true,
            ));
            return;
        }

        let dir = file.path.clone();

        // Scan directory recursively
        let mut preview_files = Vec::new();
        self.scan_dir_recursive(&dir, &mut preview_files);

        if preview_files.is_empty() {
            self.message = Some(("No files found in directory".to_string(), true));
            return;
        }

        // Sort by path
        preview_files.sort_by(|a, b| a.display_path.cmp(&b.display_path));

        // Select all files by default
        let selected_files: HashSet<usize> = (0..preview_files.len()).collect();

        let mut preview_list_state = ListState::default();
        preview_list_state.select(Some(0));

        self.recursive_preview = Some(RecursivePreviewState {
            source_dir: dir,
            preview_files,
            selected_files,
            preview_list_state,
        });
    }

    /// Recursively scan a directory for files
    fn scan_dir_recursive(&self, dir: &PathBuf, files: &mut Vec<PreviewFile>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Skip hidden files and common ignored dirs
            if name.starts_with('.') {
                continue;
            }
            if matches!(
                name,
                "node_modules" | "__pycache__" | "target" | ".git"
            ) {
                continue;
            }

            if path.is_dir() {
                self.scan_dir_recursive(&path, files);
            } else if path.is_file() {
                let display_path = if let Some(home) = dirs::home_dir() {
                    if let Ok(rel) = path.strip_prefix(&home) {
                        format!("~/{}", rel.display())
                    } else {
                        path.display().to_string()
                    }
                } else {
                    path.display().to_string()
                };

                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

                files.push(PreviewFile {
                    path,
                    display_path,
                    size,
                    track_mode: self.default_track_mode,
                });
            }
        }
    }

    /// Cancel recursive preview
    pub fn cancel_recursive_preview(&mut self) {
        self.recursive_preview = None;
    }

    /// Confirm and add files from recursive preview
    pub fn confirm_recursive_add(&mut self) {
        let preview = match self.recursive_preview.take() {
            Some(p) => p,
            None => return,
        };

        let project_name = match &self.target_project {
            Some(name) => name.clone(),
            None => {
                if let Some(p) = self.projects.first() {
                    p.name.clone()
                } else {
                    self.message = Some(("No project selected".to_string(), true));
                    return;
                }
            }
        };

        let project = match self.manifest.get_project_mut(&project_name) {
            Some(p) => p,
            None => {
                self.message = Some(("Project not found".to_string(), true));
                return;
            }
        };

        let mut added = 0;
        for idx in &preview.selected_files {
            if let Some(file) = preview.preview_files.get(*idx) {
                let contracted = contract_path(&file.path);
                // Create a TrackedFile with the file's track mode
                let tracked = dmcore::TrackedFile::with_mode(&contracted, file.track_mode);
                if project.add_file(tracked) {
                    added += 1;
                }
            }
        }

        if added > 0 {
            self.manifest_dirty = true;
            self.message = Some((
                format!("Added {} files to {}", added, project_name),
                false,
            ));
            self.refresh_projects();
            // Preserve cursor position when refreshing browse
            let saved_selection = self.browse_list_state.selected();
            self.refresh_browse();
            if let Some(idx) = saved_selection {
                let max_idx = self.browse_files.len().saturating_sub(1);
                self.browse_list_state.select(Some(idx.min(max_idx)));
            }
        } else {
            self.message = Some(("No new files added".to_string(), false));
        }
    }

    /// Toggle file selection in recursive preview
    pub fn toggle_preview_file(&mut self) {
        if let Some(ref mut preview) = self.recursive_preview {
            if let Some(idx) = preview.preview_list_state.selected() {
                if preview.selected_files.contains(&idx) {
                    preview.selected_files.remove(&idx);
                } else {
                    preview.selected_files.insert(idx);
                }
            }
        }
    }

    /// Toggle all files in recursive preview
    pub fn toggle_all_preview_files(&mut self) {
        if let Some(ref mut preview) = self.recursive_preview {
            if preview.selected_files.len() == preview.preview_files.len() {
                preview.selected_files.clear();
            } else {
                preview.selected_files = (0..preview.preview_files.len()).collect();
            }
        }
    }

    /// Toggle track mode for selected file in recursive preview
    pub fn toggle_preview_track_mode(&mut self) {
        if let Some(ref mut preview) = self.recursive_preview {
            if let Some(idx) = preview.preview_list_state.selected() {
                if let Some(file) = preview.preview_files.get_mut(idx) {
                    file.track_mode = match file.track_mode {
                        TrackMode::Git => TrackMode::Backup,
                        TrackMode::Backup => TrackMode::Both,
                        TrackMode::Both => TrackMode::Git,
                    };
                }
            }
        }
    }

    /// Set track mode for all files in recursive preview
    pub fn set_all_preview_track_mode(&mut self) {
        if let Some(ref mut preview) = self.recursive_preview {
            // Cycle through modes: Git -> Backup -> Both -> Git
            let first_mode = preview
                .preview_files
                .first()
                .map(|f| f.track_mode)
                .unwrap_or(TrackMode::Both);
            let new_mode = match first_mode {
                TrackMode::Git => TrackMode::Backup,
                TrackMode::Backup => TrackMode::Both,
                TrackMode::Both => TrackMode::Git,
            };
            for file in &mut preview.preview_files {
                file.track_mode = new_mode;
            }
        }
    }

    /// Navigate in preview
    pub fn preview_next(&mut self) {
        if let Some(ref mut preview) = self.recursive_preview {
            let len = preview.preview_files.len();
            if len == 0 {
                return;
            }
            let i = preview.preview_list_state.selected().unwrap_or(0);
            let next = if i >= len - 1 { 0 } else { i + 1 };
            preview.preview_list_state.select(Some(next));
        }
    }

    pub fn preview_previous(&mut self) {
        if let Some(ref mut preview) = self.recursive_preview {
            let len = preview.preview_files.len();
            if len == 0 {
                return;
            }
            let i = preview.preview_list_state.selected().unwrap_or(0);
            let prev = if i == 0 { len - 1 } else { i - 1 };
            preview.preview_list_state.select(Some(prev));
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // File Viewer Methods
    // ─────────────────────────────────────────────────────────────────────────

    /// Open the file viewer for the currently selected item
    pub fn open_viewer(&mut self) {
        // Get the path to view based on current mode
        let path = match self.mode {
            Mode::Projects => {
                // Get path from selected item in project view
                if let Some(i) = self.project_list_state.selected() {
                    if i < self.visible_items.len() {
                        match &self.visible_items[i] {
                            ProjectViewItem::File { abs_path, .. } => abs_path.clone(),
                            _ => return,
                        }
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            }
            Mode::Add => {
                // Get path from browse files
                if let Some(i) = self.browse_list_state.selected() {
                    if i < self.browse_files.len() {
                        self.browse_files[i].path.clone()
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            }
            Mode::Restore => {
                // Get path from restore files
                if self.restore_view == RestoreView::Files {
                    if let Some(i) = self.restore_list_state.selected() {
                        if i < self.restore_files.len() {
                            self.restore_files[i].restore_path.clone()
                        } else {
                            return;
                        }
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            }
        };

        // Check if path exists
        if !path.exists() {
            self.message = Some(("File not found".to_string(), true));
            return;
        }

        // Set viewer title
        self.viewer_title = if let Some(home) = dirs::home_dir() {
            if let Ok(rel) = path.strip_prefix(&home) {
                format!("~/{}", rel.display())
            } else {
                path.display().to_string()
            }
        } else {
            path.display().to_string()
        };

        // Load content based on whether it's a file or directory
        if path.is_dir() {
            self.viewer_content = self.load_folder_content(&path);
        } else {
            self.viewer_content = self.load_file_content(&path);
        }

        self.viewer_visible = true;
        self.viewer_scroll = 0;
    }

    /// Load and highlight a single file's content
    fn load_file_content(&self, path: &Path) -> Vec<ViewerLine> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return vec![ViewerLine {
                spans: vec![("Unable to read file".to_string(), Style::default().fg(Color::Red))],
                file_header: false,
            }],
        };

        self.highlight_content(&content, path)
    }

    /// Load folder as concatenated files with headers (conf.d style)
    fn load_folder_content(&self, path: &Path) -> Vec<ViewerLine> {
        let entries = match fs::read_dir(path) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        // Collect files only (skip subdirectories)
        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();

        // Sort files: numeric prefix first, then alphabetically
        sort_config_files(&mut files);

        let mut result = Vec::new();

        for file_path in files {
            // Add file header
            let file_name = file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "???".to_string());
            let header = format!("─────────────── {} ───────────────", file_name);
            result.push(ViewerLine {
                spans: vec![(header, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))],
                file_header: true,
            });

            // Add file content
            if let Ok(content) = fs::read_to_string(&file_path) {
                let highlighted = self.highlight_content(&content, &file_path);
                result.extend(highlighted);
            }

            // Add blank line between files
            result.push(ViewerLine {
                spans: vec![("".to_string(), Style::default())],
                file_header: false,
            });
        }

        result
    }

    /// Highlight content using syntect
    fn highlight_content(&self, content: &str, path: &Path) -> Vec<ViewerLine> {
        let syntax = self.syntax_set
            .find_syntax_for_file(path)
            .ok()
            .flatten()
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        content
            .lines()
            .map(|line| {
                let ranges = highlighter
                    .highlight_line(line, &self.syntax_set)
                    .unwrap_or_default();
                ViewerLine {
                    spans: ranges
                        .iter()
                        .map(|(style, text)| {
                            (text.to_string(), syntect_to_ratatui_style(style))
                        })
                        .collect(),
                    file_header: false,
                }
            })
            .collect()
    }

    /// Close the file viewer
    pub fn close_viewer(&mut self) {
        self.viewer_visible = false;
        self.viewer_content.clear();
        self.viewer_scroll = 0;
        self.viewer_title.clear();
    }

    /// Scroll viewer down
    pub fn viewer_scroll_down(&mut self, lines: usize) {
        let max_scroll = self.viewer_content.len().saturating_sub(10);
        self.viewer_scroll = (self.viewer_scroll + lines).min(max_scroll);
    }

    /// Scroll viewer up
    pub fn viewer_scroll_up(&mut self, lines: usize) {
        self.viewer_scroll = self.viewer_scroll.saturating_sub(lines);
    }

    /// Scroll viewer to top
    pub fn viewer_scroll_top(&mut self) {
        self.viewer_scroll = 0;
    }

    /// Scroll viewer to bottom
    pub fn viewer_scroll_bottom(&mut self) {
        self.viewer_scroll = self.viewer_content.len().saturating_sub(10);
    }
}

/// Sort config files: numeric prefix first, then alphabetically
pub fn sort_config_files(files: &mut Vec<PathBuf>) {
    files.sort_by(|a, b| {
        let a_name = a.file_name().unwrap_or_default().to_string_lossy();
        let b_name = b.file_name().unwrap_or_default().to_string_lossy();

        // Extract leading numbers
        let a_num: Option<u32> = a_name
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .ok();
        let b_num: Option<u32> = b_name
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .ok();

        match (a_num, b_num) {
            (Some(an), Some(bn)) => an.cmp(&bn).then(a_name.cmp(&b_name)),
            (Some(_), None) => std::cmp::Ordering::Less,    // Numbers first
            (None, Some(_)) => std::cmp::Ordering::Greater, // Numbers first
            (None, None) => a_name.cmp(&b_name),            // Alphabetic
        }
    });
}

/// Convert syntect style to ratatui style
fn syntect_to_ratatui_style(style: &SyntectStyle) -> Style {
    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
    Style::default().fg(fg)
}

/// Format file size for display
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}
