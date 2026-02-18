//! Shared application state and logic for TUI and GUI frontends.
//!
//! This module contains the core `App` struct and all methods that manipulate
//! application state. Both the TUI (ratatui) and GUI (egui) frontends use this
//! shared module for consistent behavior.

use crate::config::{BackupMode, Config, TrackedPattern};
use crate::index::Index;
use crate::scanner::{self, RecursiveScanOptions, Verbosity};
use anyhow::Result;
use ratatui::widgets::ListState;
use ratatui::style::{Color, Style, Modifier};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use syntect::highlighting::{ThemeSet, Style as SyntectStyle};
use syntect::parsing::SyntaxSet;
use syntect::easy::HighlightLines;

/// Spinner frames for busy indicator
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// A single line in the viewer with syntax highlighting spans
#[derive(Debug, Clone)]
pub struct ViewerLine {
    pub spans: Vec<(String, Style)>,  // Text segments with ratatui styling
    pub file_header: bool,            // True if this is a file separator line
}

/// TUI application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiMode {
    Status,   // View status of tracked files
    Add,      // Add new files to tracking
    Browse,   // Browse and restore from backup
}

impl TuiMode {
    pub fn titles() -> Vec<&'static str> {
        vec!["Tracked Files", "Add Files", "Restore"]
    }

    pub fn index(&self) -> usize {
        match self {
            TuiMode::Status => 0,
            TuiMode::Add => 1,
            TuiMode::Browse => 2,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => TuiMode::Status,
            1 => TuiMode::Add,
            _ => TuiMode::Browse,
        }
    }
}

/// File entry for display
#[derive(Debug, Clone)]
pub struct DisplayFile {
    pub path: PathBuf,
    pub display_path: String,
    pub status: FileStatus,
    pub size: Option<u64>,
    pub backup_size: Option<u64>,
    pub is_tracked: bool,
    pub backup_mode: Option<BackupMode>,
    pub is_dir: bool,
    // Tree view fields (for Status tab)
    pub depth: usize,           // Indentation level in tree
    pub is_folder_node: bool,   // True if this is a virtual folder node
    pub child_count: usize,     // Number of files in folder (for folder nodes)
    pub modified_count: usize,  // Number of modified files in folder
    pub new_count: usize,       // Number of new files in folder
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Unchanged,
    Modified,
    New,
    Deleted,
    Untracked,
}

/// Git commit info for restore view
#[derive(Debug, Clone)]
pub struct GitCommit {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub date: String,
}

impl FileStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            FileStatus::Unchanged => " ",
            FileStatus::Modified => "M",
            FileStatus::New => "+",
            FileStatus::Deleted => "-",
            FileStatus::Untracked => "?",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            FileStatus::Unchanged => Color::Green,
            FileStatus::Modified => Color::Yellow,
            FileStatus::New => Color::Cyan,
            FileStatus::Deleted => Color::Red,
            FileStatus::Untracked => Color::DarkGray,
        }
    }
}

/// Restore view state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreView {
    Commits,     // Viewing commit list
    Files,       // Viewing files from selected commit
}

/// Add mode sub-state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AddSubMode {
    #[default]
    Browse,           // Normal file browser
    RecursivePreview, // Previewing recursive add
}

/// File entry for recursive preview
#[derive(Debug, Clone)]
pub struct PreviewFile {
    pub path: PathBuf,
    pub display_path: String,
    pub size: u64,
    pub is_excluded: bool,
    pub exclude_reason: Option<String>,
}

/// State for recursive add preview
#[derive(Debug, Clone)]
pub struct RecursivePreviewState {
    pub source_dir: PathBuf,
    pub preview_files: Vec<PreviewFile>,
    pub gitignore_excluded: usize,
    pub config_excluded: usize,
    pub selected_files: HashSet<usize>,
    pub preview_list_state: ListState,
}

/// File entry for restore (from a specific commit)
#[derive(Debug, Clone)]
pub struct RestoreFile {
    pub path: PathBuf,
    pub display_path: String,
    pub hash: String,
    pub size: u64,
    pub exists_locally: bool,
    pub local_differs: bool,  // True if local file has different hash
}

/// Result from background backup operation
pub struct BackupResult {
    pub success: bool,
    pub message: String,
}

/// Per-tab state for remembering cursor and scroll position
#[derive(Debug, Clone, Default)]
pub struct TabState {
    pub cursor_index: Option<usize>,
    pub scroll_offset: f32,
}

/// TUI/GUI application state
pub struct App {
    pub mode: TuiMode,
    pub files: Vec<DisplayFile>,
    pub list_state: ListState,
    pub selected: HashSet<usize>,
    pub config: Config,
    pub index: Index,
    pub config_path: PathBuf,
    pub index_path: PathBuf,
    pub data_dir: PathBuf,
    pub message: Option<String>,
    pub should_quit: bool,
    pub show_help: bool,
    pub help_scroll: u16,  // Scroll position for help window
    pub add_input: String,
    pub add_mode: bool,
    pub backup_message_input: String,
    pub backup_message_mode: bool,
    pub browse_dir: PathBuf,  // Current directory for Add mode file browser
    pub config_dirty: bool,   // Track if config needs saving on exit
    pub index_dirty: bool,    // Track if index needs saving on exit
    pub commits: Vec<GitCommit>,  // Git commit history for restore
    // Restore state
    pub restore_view: RestoreView,
    pub selected_commit: Option<usize>,  // Index into commits
    pub restore_files: Vec<RestoreFile>, // Files from selected commit
    pub restore_list_state: ListState,   // Separate list state for restore
    // Recursive add state
    pub add_sub_mode: AddSubMode,
    pub recursive_preview: Option<RecursivePreviewState>,
    // Busy/spinner state
    pub busy: bool,
    pub busy_message: String,
    pub spinner_frame: usize,
    pub backup_receiver: Option<Receiver<BackupResult>>,
    // Tree view state (for Status tab)
    pub expanded_folders: HashSet<PathBuf>,
    // File viewer state
    pub viewer_visible: bool,
    pub viewer_content: Vec<ViewerLine>,
    pub viewer_scroll: usize,
    pub viewer_title: String,
    // Syntax highlighting resources
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
    // Per-tab state for remembering cursor/scroll position
    pub status_tab_state: TabState,
    pub add_tab_state: TabState,
    pub restore_commits_state: TabState,
    pub restore_files_state: TabState,
}

impl App {
    pub fn new(config: Config, index: Index, config_path: PathBuf, index_path: PathBuf, data_dir: PathBuf) -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let mut app = App {
            mode: TuiMode::Status,
            files: Vec::new(),
            list_state: ListState::default(),
            selected: HashSet::new(),
            config,
            index,
            config_path,
            index_path,
            data_dir,
            message: None,
            should_quit: false,
            show_help: false,
            help_scroll: 0,
            add_input: String::new(),
            add_mode: false,
            backup_message_input: String::new(),
            backup_message_mode: false,
            browse_dir: home,
            config_dirty: false,
            index_dirty: false,
            commits: Vec::new(),
            restore_view: RestoreView::Commits,
            selected_commit: None,
            restore_files: Vec::new(),
            restore_list_state: ListState::default(),
            add_sub_mode: AddSubMode::Browse,
            recursive_preview: None,
            busy: false,
            busy_message: String::new(),
            spinner_frame: 0,
            backup_receiver: None,
            expanded_folders: HashSet::new(),
            viewer_visible: false,
            viewer_content: Vec::new(),
            viewer_scroll: 0,
            viewer_title: String::new(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            status_tab_state: TabState::default(),
            add_tab_state: TabState::default(),
            restore_commits_state: TabState::default(),
            restore_files_state: TabState::default(),
        };
        app.refresh_files();
        app.load_commits();
        if !app.files.is_empty() {
            app.list_state.select(Some(0));
        }
        app
    }

    /// Load git commit history
    pub fn load_commits(&mut self) {
        self.commits.clear();

        let output = std::process::Command::new("git")
            .args(["log", "--pretty=format:%H|%h|%s|%ci", "-20"])
            .current_dir(&self.data_dir)
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let parts: Vec<&str> = line.splitn(4, '|').collect();
                    if parts.len() == 4 {
                        self.commits.push(GitCommit {
                            hash: parts[0].to_string(),
                            short_hash: parts[1].to_string(),
                            message: parts[2].to_string(),
                            date: parts[3].to_string(),
                        });
                    }
                }
            }
        }
    }

    /// Save config and index if dirty (call on exit)
    pub fn save_if_dirty(&mut self) -> Result<()> {
        if self.config_dirty {
            self.config.save(&self.config_path)?;
            self.config_dirty = false;
        }
        if self.index_dirty {
            self.index.save(&self.index_path)?;
            self.index_dirty = false;
        }
        Ok(())
    }

    /// Reload index from disk (after external changes like backup)
    pub fn reload_index(&mut self) {
        if let Ok(index) = Index::load(&self.index_path) {
            self.index = index;
            self.index_dirty = false;
        }
    }

    /// Select a commit and load its files for restore
    pub fn select_commit(&mut self) {
        if let Some(i) = self.restore_list_state.selected() {
            if i < self.commits.len() {
                self.selected_commit = Some(i);
                self.load_commit_files(&self.commits[i].hash.clone());
                self.restore_view = RestoreView::Files;
                self.restore_list_state.select(Some(0));
            }
        }
    }

    /// Go back to commit list
    pub fn back_to_commits(&mut self) {
        self.restore_view = RestoreView::Commits;
        self.selected_commit = None;
        self.restore_files.clear();
        self.selected.clear();
        // Restore selection to the commit we were viewing
        if !self.commits.is_empty() {
            self.restore_list_state.select(Some(0));
        }
    }

    /// Load files from a specific commit's index
    fn load_commit_files(&mut self, commit_hash: &str) {
        self.restore_files.clear();

        // Get index.json content at this commit
        let output = std::process::Command::new("git")
            .args(["show", &format!("{}:index.json", commit_hash)])
            .current_dir(&self.data_dir)
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let content = String::from_utf8_lossy(&output.stdout);
                if let Ok(index) = serde_json::from_str::<Index>(&content) {
                    for (path, entry) in index.files {
                        let display_path = if let Some(home) = dirs::home_dir() {
                            if let Ok(rel) = path.strip_prefix(&home) {
                                format!("~/{}", rel.display())
                            } else {
                                path.display().to_string()
                            }
                        } else {
                            path.display().to_string()
                        };

                        // Check if file exists locally and if it differs
                        let exists_locally = path.exists();
                        let local_differs = if exists_locally {
                            // Calculate local file hash
                            if let Ok(local_hash) = self.hash_file(&path) {
                                local_hash != entry.hash
                            } else {
                                true // Can't read = differs
                            }
                        } else {
                            true // Doesn't exist = differs
                        };

                        self.restore_files.push(RestoreFile {
                            path,
                            display_path,
                            hash: entry.hash,
                            size: entry.size,
                            exists_locally,
                            local_differs,
                        });
                    }

                    // Sort by path
                    self.restore_files.sort_by(|a, b| a.display_path.cmp(&b.display_path));
                }
            } else {
                self.message = Some("Failed to load commit index".to_string());
            }
        }
    }

    /// Hash a file (for comparison)
    fn hash_file(&self, path: &Path) -> Result<String> {
        use sha2::{Sha256, Digest};
        use std::io::Read;

        let mut file = fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Restore selected files from the selected commit
    pub fn perform_restore(&mut self) {
        let indices: Vec<usize> = if self.selected.is_empty() {
            // If nothing selected, restore the currently highlighted file
            self.restore_list_state.selected().into_iter().collect()
        } else {
            self.selected.iter().cloned().collect()
        };

        if indices.is_empty() {
            self.message = Some("No files selected for restore".to_string());
            return;
        }

        let storage_path = match crate::get_storage_path() {
            Ok(p) => p,
            Err(e) => {
                self.message = Some(format!("Error getting storage path: {}", e));
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

            // Get backup file from storage
            let hash = &file.hash;
            let backup_path = storage_path.join(&hash[0..2]).join(hash);

            if !backup_path.exists() {
                errors += 1;
                continue;
            }

            // Create parent directory if needed
            if let Some(parent) = file.path.parent() {
                if !parent.exists() {
                    if let Err(_) = fs::create_dir_all(parent) {
                        errors += 1;
                        continue;
                    }
                }
            }

            // Copy from storage to destination
            match fs::copy(&backup_path, &file.path) {
                Ok(_) => restored += 1,
                Err(_) => errors += 1,
            }
        }

        self.selected.clear();

        if errors > 0 {
            self.message = Some(format!("Restored {} files ({} errors)", restored, errors));
        } else {
            self.message = Some(format!("Restored {} files", restored));
        }

        // Refresh to update local_differs status
        if let Some(commit_idx) = self.selected_commit {
            let hash = self.commits[commit_idx].hash.clone();
            self.load_commit_files(&hash);
        }
    }

    /// Refresh file list based on current mode
    pub fn refresh_files(&mut self) {
        self.files.clear();
        self.selected.clear();

        match self.mode {
            TuiMode::Status | TuiMode::Browse => {
                self.load_status_files();
            }
            TuiMode::Add => {
                self.load_addable_files();
            }
        }

        // Reset selection
        if !self.files.is_empty() {
            if self.list_state.selected().is_none() {
                self.list_state.select(Some(0));
            } else if let Some(i) = self.list_state.selected() {
                if i >= self.files.len() {
                    self.list_state.select(Some(self.files.len() - 1));
                }
            }
        } else {
            self.list_state.select(None);
        }
    }

    fn load_status_files(&mut self) {
        // Get all tracked files
        let pattern_strings = self.config.pattern_strings();
        let files = scanner::scan_patterns_with_verbosity(
            &pattern_strings,
            &self.config.exclude,
            Verbosity::Quiet,
        )
        .unwrap_or_default();

        let current_set: HashSet<_> = files.iter().cloned().collect();
        let mut all_files = Vec::new();

        // Check files in current patterns
        for file in &files {
            let (status, backup_size) = if let Some(entry) = self.index.get_file(file) {
                if !file.exists() {
                    (FileStatus::Deleted, Some(entry.size))
                } else {
                    let current_hash = scanner::hash_file(file).ok();
                    if current_hash.as_ref() == Some(&entry.hash) {
                        (FileStatus::Unchanged, Some(entry.size))
                    } else {
                        (FileStatus::Modified, Some(entry.size))
                    }
                }
            } else {
                (FileStatus::New, None)
            };

            let size = fs::metadata(file).map(|m| m.len()).ok();
            let backup_mode = self.get_file_mode(file);

            all_files.push(DisplayFile {
                path: file.clone(),
                display_path: self.display_path(file),
                status,
                size,
                backup_size,
                is_tracked: true,
                backup_mode: Some(backup_mode),
                is_dir: false,
                depth: 0,
                is_folder_node: false,
                child_count: 0,
                modified_count: 0,
                new_count: 0,
            });
        }

        // Add deleted files from index
        for (path, entry) in &self.index.files {
            if !current_set.contains(path) {
                all_files.push(DisplayFile {
                    path: path.clone(),
                    display_path: self.display_path(path),
                    status: FileStatus::Deleted,
                    size: None,
                    backup_size: Some(entry.size),
                    is_tracked: true,
                    backup_mode: None,
                    is_dir: false,
                    depth: 0,
                    is_folder_node: false,
                    child_count: 0,
                    modified_count: 0,
                    new_count: 0,
                });
            }
        }

        // Sort by path
        all_files.sort_by(|a, b| a.path.cmp(&b.path));

        // Build tree view
        self.build_tree_view(all_files);
    }

    /// Build tree view from flat file list
    fn build_tree_view(&mut self, all_files: Vec<DisplayFile>) {
        use std::collections::BTreeMap;

        // Group files by their parent folder
        let mut folders: BTreeMap<PathBuf, Vec<DisplayFile>> = BTreeMap::new();

        for file in all_files {
            let parent = file.path.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("/"));
            folders.entry(parent).or_default().push(file);
        }

        // Build the display list with folder nodes
        for (folder_path, files) in folders {
            // Calculate folder stats
            let child_count = files.len();
            let modified_count = files.iter().filter(|f| f.status == FileStatus::Modified).count();
            let new_count = files.iter().filter(|f| f.status == FileStatus::New).count();

            // Create folder display name
            let folder_display = self.display_path(&folder_path);

            // Add folder node
            self.files.push(DisplayFile {
                path: folder_path.clone(),
                display_path: folder_display,
                status: if modified_count > 0 || new_count > 0 {
                    FileStatus::Modified
                } else {
                    FileStatus::Unchanged
                },
                size: None,
                backup_size: None,
                is_tracked: true,
                backup_mode: None,
                is_dir: true,
                depth: 0,
                is_folder_node: true,
                child_count,
                modified_count,
                new_count,
            });

            // Add files if folder is expanded
            if self.expanded_folders.contains(&folder_path) {
                for mut file in files {
                    // Show just filename when nested under folder
                    file.display_path = file.path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| file.display_path.clone());
                    file.depth = 1;
                    self.files.push(file);
                }
            }
        }
    }

    /// Toggle folder expansion in tree view
    pub fn toggle_folder(&mut self) {
        if self.mode != TuiMode::Status {
            return;
        }

        if let Some(i) = self.list_state.selected() {
            if i < self.files.len() {
                let file = &self.files[i];
                if file.is_folder_node {
                    let path = file.path.clone();
                    if self.expanded_folders.contains(&path) {
                        self.expanded_folders.remove(&path);
                    } else {
                        self.expanded_folders.insert(path);
                    }
                    self.refresh_files();
                    // Try to keep selection on the same folder
                    self.list_state.select(Some(i.min(self.files.len().saturating_sub(1))));
                }
            }
        }
    }

    /// Expand selected folder in tree view (right arrow)
    pub fn expand_folder(&mut self) {
        if self.mode != TuiMode::Status {
            return;
        }

        if let Some(i) = self.list_state.selected() {
            if i < self.files.len() {
                let file = &self.files[i];
                if file.is_folder_node && !self.expanded_folders.contains(&file.path) {
                    let path = file.path.clone();
                    self.expanded_folders.insert(path);
                    self.refresh_files();
                    self.list_state.select(Some(i.min(self.files.len().saturating_sub(1))));
                }
            }
        }
    }

    /// Collapse selected folder in tree view (left arrow)
    pub fn collapse_folder(&mut self) {
        if self.mode != TuiMode::Status {
            return;
        }

        if let Some(i) = self.list_state.selected() {
            if i < self.files.len() {
                let file = &self.files[i];
                if file.is_folder_node && self.expanded_folders.contains(&file.path) {
                    let path = file.path.clone();
                    self.expanded_folders.remove(&path);
                    self.refresh_files();
                    self.list_state.select(Some(i.min(self.files.len().saturating_sub(1))));
                }
            }
        }
    }

    /// Expand all folders in tree view
    pub fn expand_all_folders(&mut self) {
        if self.mode != TuiMode::Status {
            return;
        }
        for file in &self.files {
            if file.is_folder_node {
                self.expanded_folders.insert(file.path.clone());
            }
        }
        self.refresh_files();
    }

    /// Collapse all folders in tree view
    pub fn collapse_all_folders(&mut self) {
        if self.mode != TuiMode::Status {
            return;
        }
        self.expanded_folders.clear();
        self.refresh_files();
    }

    fn load_addable_files(&mut self) {
        // Directory browser for Add mode
        let tracked: HashSet<_> = self.config.pattern_strings().into_iter().collect();

        // Read directory contents
        let entries = match fs::read_dir(&self.browse_dir) {
            Ok(entries) => entries,
            Err(_) => {
                self.message = Some(format!("Cannot read: {}", self.browse_dir.display()));
                return;
            }
        };

        let mut items: Vec<(PathBuf, bool)> = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            let is_dir = path.is_dir();

            // Skip hidden files unless we're in a hidden directory or it's a dotfile
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Show dotfiles (they're what we want to track!) but skip some system dirs
            if file_name == "." || file_name == ".." {
                continue;
            }

            // Skip some directories that are never useful
            if is_dir && matches!(file_name, "node_modules" | ".git" | "__pycache__" | ".cache" | "Cache" | "CacheStorage") {
                continue;
            }

            items.push((path, is_dir));
        }

        // Sort: directories first, then alphabetically
        items.sort_by(|a, b| {
            match (a.1, b.1) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.0.cmp(&b.0),
            }
        });

        for (path, is_dir) in items {
            let display = if is_dir {
                format!("{}/", path.file_name().and_then(|n| n.to_str()).unwrap_or("?"))
            } else {
                path.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string()
            };

            let is_tracked = tracked.iter().any(|p| {
                if let Ok(expanded) = scanner::expand_tilde(p) {
                    // Direct match or path is within tracked directory
                    if expanded == path || path.starts_with(&expanded) {
                        return true;
                    }
                    // For folders: check if any pattern covers this folder
                    // e.g., ~/.config/nvim/** means ~/.config/nvim/ is tracked
                    if is_dir {
                        let pattern_str = expanded.to_string_lossy();
                        let path_str = path.to_string_lossy();
                        // Check if pattern starts with this folder path
                        if pattern_str.starts_with(&*path_str) {
                            return true;
                        }
                        // Check patterns like "folder/**" or "folder/*"
                        let folder_pattern = format!("{}/**", path_str);
                        let folder_pattern2 = format!("{}/*", path_str);
                        if pattern_str == folder_pattern || pattern_str == folder_pattern2 {
                            return true;
                        }
                    }
                    false
                } else {
                    false
                }
            });

            let size = if is_dir {
                None
            } else {
                fs::metadata(&path).map(|m| m.len()).ok()
            };

            self.files.push(DisplayFile {
                path: path.clone(),
                display_path: display,
                status: if is_tracked {
                    FileStatus::Unchanged
                } else {
                    FileStatus::Untracked
                },
                size,
                backup_size: None,
                is_tracked,
                backup_mode: None,
                is_dir,
                depth: 0,
                is_folder_node: false,
                child_count: 0,
                modified_count: 0,
                new_count: 0,
            });
        }
    }

    /// Enter a directory in Add mode
    pub fn enter_directory(&mut self) {
        if self.mode != TuiMode::Add {
            return;
        }
        if let Some(i) = self.list_state.selected() {
            if i < self.files.len() && self.files[i].is_dir {
                self.browse_dir = self.files[i].path.clone();
                self.list_state.select(Some(0));
                self.refresh_files();
            }
        }
    }

    /// Go to parent directory in Add mode
    pub fn parent_directory(&mut self) {
        if self.mode != TuiMode::Add {
            return;
        }
        let current_dir = self.browse_dir.clone();
        if let Some(parent) = self.browse_dir.parent() {
            self.browse_dir = parent.to_path_buf();
            self.refresh_files();
            // Find and select the directory we came from
            if let Some(idx) = self.files.iter().position(|f| f.path == current_dir) {
                self.list_state.select(Some(idx));
            } else {
                self.list_state.select(Some(0));
            }
        }
    }

    /// Go to home directory in Add mode
    pub fn home_directory(&mut self) {
        if self.mode != TuiMode::Add {
            return;
        }
        self.browse_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        self.list_state.select(Some(0));
        self.refresh_files();
    }

    pub fn display_path(&self, path: &Path) -> String {
        if let Some(home) = dirs::home_dir() {
            if let Ok(rel) = path.strip_prefix(&home) {
                return format!("~/{}", rel.display());
            }
        }
        path.display().to_string()
    }

    pub fn get_file_mode(&self, file: &PathBuf) -> BackupMode {
        for pattern in self.config.tracked_files.iter().rev() {
            if let Ok(expanded) = scanner::expand_tilde(pattern.path()) {
                if file.starts_with(&expanded) || *file == expanded {
                    return self.config.mode_for_pattern(pattern);
                }
            }
        }
        self.config.backup_mode
    }

    pub fn next(&mut self) {
        // Get the appropriate list length and state based on mode
        let (len, list_state) = self.get_current_list_info();
        if len == 0 {
            return;
        }
        let i = match list_state.selected() {
            Some(i) => {
                if i >= len - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.set_current_selection(i);
    }

    pub fn previous(&mut self) {
        let (len, list_state) = self.get_current_list_info();
        if len == 0 {
            return;
        }
        let i = match list_state.selected() {
            Some(i) => {
                if i == 0 {
                    len - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.set_current_selection(i);
    }

    pub fn page_down(&mut self) {
        let (len, list_state) = self.get_current_list_info();
        if len == 0 {
            return;
        }
        let page_size = 10;
        let i = match list_state.selected() {
            Some(i) => (i + page_size).min(len - 1),
            None => 0,
        };
        self.set_current_selection(i);
    }

    pub fn page_up(&mut self) {
        let (len, list_state) = self.get_current_list_info();
        if len == 0 {
            return;
        }
        let page_size = 10;
        let i = match list_state.selected() {
            Some(i) => i.saturating_sub(page_size),
            None => 0,
        };
        self.set_current_selection(i);
    }

    /// Get current list length and state based on mode
    pub fn get_current_list_info(&self) -> (usize, &ListState) {
        if self.mode == TuiMode::Browse {
            match self.restore_view {
                RestoreView::Commits => (self.commits.len(), &self.restore_list_state),
                RestoreView::Files => (self.restore_files.len(), &self.restore_list_state),
            }
        } else {
            (self.files.len(), &self.list_state)
        }
    }

    /// Set selection for current list
    pub fn set_current_selection(&mut self, i: usize) {
        if self.mode == TuiMode::Browse {
            self.restore_list_state.select(Some(i));
        } else {
            self.list_state.select(Some(i));
        }
    }

    pub fn toggle_select(&mut self) {
        let selected_idx = if self.mode == TuiMode::Browse {
            self.restore_list_state.selected()
        } else {
            self.list_state.selected()
        };

        if let Some(i) = selected_idx {
            if self.selected.contains(&i) {
                self.selected.remove(&i);
            } else {
                self.selected.insert(i);
            }
        }
    }

    pub fn select_all(&mut self) {
        let len = if self.mode == TuiMode::Browse {
            match self.restore_view {
                RestoreView::Commits => self.commits.len(),
                RestoreView::Files => self.restore_files.len(),
            }
        } else {
            self.files.len()
        };

        if self.selected.len() == len {
            self.selected.clear();
        } else {
            self.selected = (0..len).collect();
        }
    }

    pub fn next_mode(&mut self) {
        self.save_current_tab_state();
        let next = (self.mode.index() + 1) % 3;
        self.mode = TuiMode::from_index(next);
        self.selected.clear();
        self.reset_mode_state();
        self.refresh_files();
        self.restore_tab_state();
    }

    pub fn prev_mode(&mut self) {
        self.save_current_tab_state();
        let prev = if self.mode.index() == 0 {
            2
        } else {
            self.mode.index() - 1
        };
        self.mode = TuiMode::from_index(prev);
        self.selected.clear();
        self.reset_mode_state();
        self.refresh_files();
        self.restore_tab_state();
    }

    /// Save current tab's cursor state before switching
    pub fn save_current_tab_state(&mut self) {
        match self.mode {
            TuiMode::Status => {
                self.status_tab_state.cursor_index = self.list_state.selected();
            }
            TuiMode::Add => {
                self.add_tab_state.cursor_index = self.list_state.selected();
            }
            TuiMode::Browse => {
                match self.restore_view {
                    RestoreView::Commits => {
                        self.restore_commits_state.cursor_index = self.restore_list_state.selected();
                    }
                    RestoreView::Files => {
                        self.restore_files_state.cursor_index = self.restore_list_state.selected();
                    }
                }
            }
        }
    }

    /// Restore tab's cursor state after switching
    pub fn restore_tab_state(&mut self) {
        match self.mode {
            TuiMode::Status => {
                if let Some(idx) = self.status_tab_state.cursor_index {
                    let max_idx = self.files.len().saturating_sub(1);
                    self.list_state.select(Some(idx.min(max_idx)));
                }
            }
            TuiMode::Add => {
                if let Some(idx) = self.add_tab_state.cursor_index {
                    let max_idx = self.files.len().saturating_sub(1);
                    self.list_state.select(Some(idx.min(max_idx)));
                }
            }
            TuiMode::Browse => {
                match self.restore_view {
                    RestoreView::Commits => {
                        if let Some(idx) = self.restore_commits_state.cursor_index {
                            let max_idx = self.commits.len().saturating_sub(1);
                            self.restore_list_state.select(Some(idx.min(max_idx)));
                        }
                    }
                    RestoreView::Files => {
                        if let Some(idx) = self.restore_files_state.cursor_index {
                            let max_idx = self.restore_files.len().saturating_sub(1);
                            self.restore_list_state.select(Some(idx.min(max_idx)));
                        }
                    }
                }
            }
        }
    }

    /// Reset mode-specific state when switching modes
    fn reset_mode_state(&mut self) {
        // Reset restore state when entering Browse mode
        if self.mode == TuiMode::Browse {
            self.restore_view = RestoreView::Commits;
            self.selected_commit = None;
            self.restore_files.clear();
            self.load_commits();
            if !self.commits.is_empty() {
                self.restore_list_state.select(Some(0));
            } else {
                self.restore_list_state.select(None);
            }
        }
    }

    pub fn toggle_tracking(&mut self) {
        // Get indices to operate on: selected items or current item
        let indices: Vec<usize> = if self.selected.is_empty() {
            self.list_state.selected().into_iter().collect()
        } else {
            self.selected.iter().cloned().collect()
        };

        if indices.is_empty() {
            return;
        }

        let mut added = 0;
        let mut removed = 0;

        // Collect patterns to add/remove (avoid borrowing issues)
        let mut patterns_to_add = Vec::new();
        let mut patterns_to_remove = Vec::new();

        for i in &indices {
            if *i >= self.files.len() {
                continue;
            }
            let file = &self.files[*i];

            // Skip directories in bulk operations (use A for folders)
            if file.is_dir {
                continue;
            }

            // Build pattern from path
            let pattern = if self.mode == TuiMode::Add {
                if let Some(home) = dirs::home_dir() {
                    if let Ok(rel) = file.path.strip_prefix(&home) {
                        format!("~/{}", rel.display())
                    } else {
                        file.path.to_string_lossy().to_string()
                    }
                } else {
                    file.path.to_string_lossy().to_string()
                }
            } else {
                file.display_path.clone()
            };

            if file.is_tracked {
                patterns_to_remove.push(pattern);
            } else {
                patterns_to_add.push(pattern);
            }
        }

        // Remove patterns
        for pattern in patterns_to_remove {
            if let Some(pos) = self
                .config
                .tracked_files
                .iter()
                .position(|p| p.path() == pattern)
            {
                self.config.tracked_files.remove(pos);
                removed += 1;
            }
        }

        // Add patterns
        for pattern in patterns_to_add {
            self.config
                .tracked_files
                .push(TrackedPattern::simple(&pattern));
            added += 1;
        }

        if added > 0 || removed > 0 {
            self.config_dirty = true;
            let msg = match (added, removed) {
                (a, 0) if a == 1 => "Added 1 file (saves on exit)".to_string(),
                (a, 0) => format!("Added {} files (saves on exit)", a),
                (0, r) if r == 1 => "Removed 1 file (saves on exit)".to_string(),
                (0, r) => format!("Removed {} files (saves on exit)", r),
                (a, r) => format!("Added {}, removed {} files (saves on exit)", a, r),
            };
            self.message = Some(msg);
            self.selected.clear();
            self.refresh_files();
        }
    }

    /// Start recursive preview for the selected directory
    pub fn start_recursive_preview(&mut self) {
        if self.mode != TuiMode::Add {
            return;
        }

        // Get selected item
        let selected_idx = self.list_state.selected();
        if selected_idx.is_none() {
            self.message = Some("No directory selected".to_string());
            return;
        }

        let idx = selected_idx.unwrap();
        if idx >= self.files.len() {
            return;
        }

        let file = &self.files[idx];
        if !file.is_dir {
            self.message = Some("Select a directory to add recursively".to_string());
            return;
        }

        let dir = file.path.clone();
        self.message = Some(format!("Scanning {}...", file.display_path));

        // Perform recursive scan
        let options = RecursiveScanOptions::new().with_gitignore(true);
        let result = match scanner::scan_directory_recursive(&dir, &self.config.exclude, &options) {
            Ok(r) => r,
            Err(e) => {
                self.message = Some(format!("Scan error: {}", e));
                return;
            }
        };

        // Build preview files
        let mut preview_files = Vec::new();
        for path in &result.files {
            let display_path = if let Some(home) = dirs::home_dir() {
                if let Ok(rel) = path.strip_prefix(&home) {
                    format!("~/{}", rel.display())
                } else {
                    path.display().to_string()
                }
            } else {
                path.display().to_string()
            };

            let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);

            preview_files.push(PreviewFile {
                path: path.clone(),
                display_path,
                size,
                is_excluded: false,
                exclude_reason: None,
            });
        }

        // Select all files by default
        let selected_files: HashSet<usize> = (0..preview_files.len()).collect();

        let mut preview_list_state = ListState::default();
        if !preview_files.is_empty() {
            preview_list_state.select(Some(0));
        }

        self.recursive_preview = Some(RecursivePreviewState {
            source_dir: dir,
            preview_files,
            gitignore_excluded: result.gitignore_excluded,
            config_excluded: result.config_excluded,
            selected_files,
            preview_list_state,
        });

        self.add_sub_mode = AddSubMode::RecursivePreview;
        self.message = None;
    }

    /// Confirm and add files from recursive preview
    pub fn confirm_recursive_add(&mut self) {
        if let Some(ref preview) = self.recursive_preview {
            let mut added = 0;

            for idx in &preview.selected_files {
                if *idx >= preview.preview_files.len() {
                    continue;
                }

                let file = &preview.preview_files[*idx];
                let pattern = file.display_path.clone();

                // Check if already tracked
                let already_tracked = self.config.tracked_files.iter().any(|p| p.path() == pattern);
                if !already_tracked {
                    self.config.tracked_files.push(TrackedPattern::simple(&pattern));
                    added += 1;
                }
            }

            if added > 0 {
                self.config_dirty = true;
                self.message = Some(format!("Added {} files (saves on exit)", added));
            } else {
                self.message = Some("All files already tracked".to_string());
            }
        }

        self.cancel_recursive_preview();
        self.refresh_files();
    }

    /// Cancel recursive preview and return to browse mode
    pub fn cancel_recursive_preview(&mut self) {
        self.add_sub_mode = AddSubMode::Browse;
        self.recursive_preview = None;
    }

    /// Toggle file selection in recursive preview
    pub fn toggle_preview_file(&mut self) {
        if let Some(ref mut preview) = self.recursive_preview {
            if let Some(i) = preview.preview_list_state.selected() {
                if preview.selected_files.contains(&i) {
                    preview.selected_files.remove(&i);
                } else {
                    preview.selected_files.insert(i);
                }
            }
        }
    }

    /// Select/deselect all files in recursive preview
    pub fn toggle_all_preview_files(&mut self) {
        if let Some(ref mut preview) = self.recursive_preview {
            if preview.selected_files.len() == preview.preview_files.len() {
                preview.selected_files.clear();
            } else {
                preview.selected_files = (0..preview.preview_files.len()).collect();
            }
        }
    }

    /// Navigate in recursive preview
    pub fn preview_next(&mut self) {
        if let Some(ref mut preview) = self.recursive_preview {
            let len = preview.preview_files.len();
            if len == 0 {
                return;
            }
            let i = match preview.preview_list_state.selected() {
                Some(i) => if i >= len - 1 { 0 } else { i + 1 },
                None => 0,
            };
            preview.preview_list_state.select(Some(i));
        }
    }

    pub fn preview_previous(&mut self) {
        if let Some(ref mut preview) = self.recursive_preview {
            let len = preview.preview_files.len();
            if len == 0 {
                return;
            }
            let i = match preview.preview_list_state.selected() {
                Some(i) => if i == 0 { len - 1 } else { i - 1 },
                None => 0,
            };
            preview.preview_list_state.select(Some(i));
        }
    }

    pub fn preview_page_down(&mut self) {
        if let Some(ref mut preview) = self.recursive_preview {
            let len = preview.preview_files.len();
            if len == 0 {
                return;
            }
            let page_size = 10;
            let i = match preview.preview_list_state.selected() {
                Some(i) => (i + page_size).min(len - 1),
                None => 0,
            };
            preview.preview_list_state.select(Some(i));
        }
    }

    pub fn preview_page_up(&mut self) {
        if let Some(ref mut preview) = self.recursive_preview {
            let len = preview.preview_files.len();
            if len == 0 {
                return;
            }
            let page_size = 10;
            let i = match preview.preview_list_state.selected() {
                Some(i) => i.saturating_sub(page_size),
                None => 0,
            };
            preview.preview_list_state.select(Some(i));
        }
    }

    /// Add selected folder(s) as pattern(s) (with /** suffix)
    pub fn add_folder_pattern(&mut self) {
        if self.mode != TuiMode::Add {
            return;
        }

        // Get indices to operate on: selected items or current item
        let indices: Vec<usize> = if self.selected.is_empty() {
            self.list_state.selected().into_iter().collect()
        } else {
            self.selected.iter().cloned().collect()
        };

        if indices.is_empty() {
            return;
        }

        // If single non-dir item selected, delegate to toggle_tracking
        if indices.len() == 1 {
            let i = indices[0];
            if i < self.files.len() && !self.files[i].is_dir {
                self.toggle_tracking();
                return;
            }
        }

        let mut added = 0;
        let mut skipped = 0;

        for i in &indices {
            if *i >= self.files.len() {
                continue;
            }
            let file = &self.files[*i];

            if !file.is_dir {
                skipped += 1;
                continue;
            }

            // Build pattern with /** suffix
            let pattern = if let Some(home) = dirs::home_dir() {
                if let Ok(rel) = file.path.strip_prefix(&home) {
                    format!("~/{}/**", rel.display())
                } else {
                    format!("{}/**", file.path.display())
                }
            } else {
                format!("{}/**", file.path.display())
            };

            // Check if already tracked
            let already_tracked = self.config.tracked_files.iter().any(|p| p.path() == pattern);
            if already_tracked {
                skipped += 1;
                continue;
            }

            self.config.tracked_files.push(TrackedPattern::simple(&pattern));
            added += 1;
        }

        if added > 0 {
            self.config_dirty = true;
            let msg = if added == 1 {
                "Added 1 folder (saves on exit)".to_string()
            } else {
                format!("Added {} folders (saves on exit)", added)
            };
            self.message = Some(msg);
            self.selected.clear();
            self.refresh_files();
        } else if skipped > 0 {
            self.message = Some("No new folders to add (already tracked or not folders)".to_string());
        }
    }

    /// Remove file/folder from tracking (in Add Files browser)
    pub fn remove_from_tracking_in_browser(&mut self) {
        if self.mode != TuiMode::Add {
            return;
        }

        // Get indices to operate on: selected items or current item
        let indices: Vec<usize> = if self.selected.is_empty() {
            self.list_state.selected().into_iter().collect()
        } else {
            self.selected.iter().cloned().collect()
        };

        if indices.is_empty() {
            return;
        }

        // Collect all patterns to check for removal
        let mut all_patterns_to_check: Vec<String> = Vec::new();

        for i in &indices {
            if *i >= self.files.len() {
                continue;
            }

            let file = &self.files[*i];
            if !file.is_tracked {
                continue;
            }

            // Build possible patterns for this path
            let path_str = if let Some(home) = dirs::home_dir() {
                if let Ok(rel) = file.path.strip_prefix(&home) {
                    format!("~/{}", rel.display())
                } else {
                    file.path.to_string_lossy().to_string()
                }
            } else {
                file.path.to_string_lossy().to_string()
            };

            // Patterns to look for
            if file.is_dir {
                all_patterns_to_check.push(format!("{}/**", path_str));
                all_patterns_to_check.push(format!("{}/*", path_str));
                all_patterns_to_check.push(path_str);
            } else {
                all_patterns_to_check.push(path_str);
            }
        }

        if all_patterns_to_check.is_empty() {
            self.message = Some("No tracked items selected".to_string());
            return;
        }

        // Find and remove matching patterns
        let mut removed = Vec::new();
        self.config.tracked_files.retain(|p| {
            let dominated = all_patterns_to_check.iter().any(|check| p.path() == check);
            if dominated {
                removed.push(p.path().to_string());
            }
            !dominated
        });

        if !removed.is_empty() {
            self.config_dirty = true;
            let msg = if removed.len() == 1 {
                format!("Untracked: {} (no files deleted, saves on exit)", removed[0])
            } else {
                format!("Untracked {} patterns (no files deleted, saves on exit)", removed.len())
            };
            self.message = Some(msg);
            self.selected.clear();
            self.refresh_files();
        } else {
            self.message = Some("Files tracked via folder patterns - remove the folder pattern instead".to_string());
        }
    }

    pub fn remove_from_index(&mut self) {
        let indices: Vec<_> = if self.selected.is_empty() {
            self.list_state.selected().into_iter().collect()
        } else {
            self.selected.iter().cloned().collect()
        };

        let mut removed = 0;
        for i in indices {
            if i < self.files.len() {
                let file = &self.files[i];
                if self.index.files.remove(&file.path).is_some() {
                    removed += 1;
                }
            }
        }

        if removed > 0 {
            self.index_dirty = true;
            self.message = Some(format!("Removed {} file(s) from index (saves on exit)", removed));
            self.selected.clear();
            self.refresh_files();
        }
    }

    /// Open file viewer for the selected item
    pub fn open_viewer(&mut self) {
        // Get the path to view based on current mode
        let path = match self.mode {
            TuiMode::Status => {
                if let Some(i) = self.list_state.selected() {
                    if i < self.files.len() {
                        self.files[i].path.clone()
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            }
            TuiMode::Add => {
                if let Some(i) = self.list_state.selected() {
                    if i < self.files.len() {
                        self.files[i].path.clone()
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            }
            TuiMode::Browse => {
                if self.restore_view == RestoreView::Files {
                    if let Some(i) = self.restore_list_state.selected() {
                        if i < self.restore_files.len() {
                            self.restore_files[i].path.clone()
                        } else {
                            return;
                        }
                    } else {
                        return;
                    }
                } else {
                    self.message = Some("Select a backup first, then select a file to view".to_string());
                    return;
                }
            }
        };

        // Load content based on whether it's a file or directory
        if path.is_dir() {
            self.viewer_content = self.load_folder_content(&path);
            self.viewer_title = self.display_path(&path);
        } else if path.is_file() {
            self.viewer_content = self.load_file_content(&path);
            self.viewer_title = self.display_path(&path);
        } else {
            self.message = Some("File not found".to_string());
            return;
        }

        if self.viewer_content.is_empty() {
            self.message = Some("File is empty or could not be read".to_string());
            return;
        }

        self.viewer_visible = true;
        self.viewer_scroll = 0;
    }

    /// Load and highlight a single file's content
    fn load_file_content(&self, path: &Path) -> Vec<ViewerLine> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        self.highlight_content(&content, path)
    }

    /// Load folder as concatenated files with headers
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

    /// Perform backup of selected or all tracked files (async)
    pub fn perform_backup(&mut self, custom_message: Option<String>) {
        use chrono::Local;

        // Don't start another backup if one is already running
        if self.busy {
            self.message = Some("Backup already in progress...".to_string());
            return;
        }

        // Use custom message or generate timestamp-based commit message
        let commit_msg = custom_message.unwrap_or_else(|| {
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            format!("Backup {}", timestamp)
        });

        // Get dotmatrix executable path to run backup command
        // Note: We need "dotmatrix" not "dmgui" since dmgui doesn't have CLI commands
        let exe_path = match std::env::current_exe() {
            Ok(current) => {
                // Get directory containing current executable
                if let Some(dir) = current.parent() {
                    #[cfg(windows)]
                    let dotmatrix_exe = dir.join("dotmatrix.exe");
                    #[cfg(not(windows))]
                    let dotmatrix_exe = dir.join("dotmatrix");

                    if dotmatrix_exe.exists() {
                        dotmatrix_exe
                    } else {
                        // Fall back to hoping it's in PATH
                        #[cfg(windows)]
                        { std::path::PathBuf::from("dotmatrix.exe") }
                        #[cfg(not(windows))]
                        { std::path::PathBuf::from("dotmatrix") }
                    }
                } else {
                    #[cfg(windows)]
                    { std::path::PathBuf::from("dotmatrix.exe") }
                    #[cfg(not(windows))]
                    { std::path::PathBuf::from("dotmatrix") }
                }
            }
            Err(_) => {
                #[cfg(windows)]
                { std::path::PathBuf::from("dotmatrix.exe") }
                #[cfg(not(windows))]
                { std::path::PathBuf::from("dotmatrix") }
            }
        };

        // Set busy state
        self.busy = true;
        self.busy_message = format!("Backing up: {}", commit_msg);
        self.spinner_frame = 0;

        // Create channel for result
        let (tx, rx) = mpsc::channel();
        self.backup_receiver = Some(rx);

        // Spawn backup thread
        let commit_msg_clone = commit_msg.clone();
        thread::spawn(move || {
            let output = std::process::Command::new(&exe_path)
                .args(["backup", "--message", &commit_msg_clone])
                .output();

            let result = match output {
                Ok(output) => {
                    if output.status.success() {
                        BackupResult {
                            success: true,
                            message: format!("Backup complete: {}", commit_msg_clone),
                        }
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        BackupResult {
                            success: false,
                            message: format!("Backup failed: {}", stderr.trim()),
                        }
                    }
                }
                Err(e) => BackupResult {
                    success: false,
                    message: format!("Backup error: {}", e),
                },
            };

            let _ = tx.send(result);
        });
    }

    /// Poll for backup completion - returns true if backup completed this poll
    pub fn poll_backup(&mut self) -> bool {
        if let Some(ref receiver) = self.backup_receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    self.busy = false;
                    self.busy_message.clear();
                    self.backup_receiver = None;
                    if result.success {
                        self.message = Some(result.message);
                        self.reload_index();
                        self.load_commits();
                        self.refresh_files();
                    } else {
                        self.message = Some(result.message);
                    }
                    return true;
                }
                Err(TryRecvError::Empty) => {
                    // Still running, update spinner
                    self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
                }
                Err(TryRecvError::Disconnected) => {
                    // Thread died unexpectedly
                    self.busy = false;
                    self.busy_message.clear();
                    self.backup_receiver = None;
                    self.message = Some("Backup process disconnected unexpectedly".to_string());
                    return true;
                }
            }
        }
        false
    }
}

/// Format file size
pub fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1}M", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1}K", bytes as f64 / 1024.0)
    } else {
        format!("{}B", bytes)
    }
}

/// Sort files for conf.d style directories: numeric prefix first, then alphabetically
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
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a_name.cmp(&b_name),
        }
    });
}

/// Convert syntect style to ratatui style
pub fn syntect_to_ratatui_style(style: &SyntectStyle) -> Style {
    Style::default().fg(Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    ))
}
