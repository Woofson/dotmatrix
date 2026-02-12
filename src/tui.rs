use crate::config::{BackupMode, Config, TrackedPattern};
use crate::index::Index;
use crate::scanner::{self, Verbosity};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame, Terminal,
};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// TUI application mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiMode {
    Status,   // View status of tracked files
    Add,      // Add new files to tracking
    Browse,   // Browse and restore from backup
}

impl TuiMode {
    fn titles() -> Vec<&'static str> {
        vec!["Tracked Files", "Add Files", "Restore"]
    }

    fn index(&self) -> usize {
        match self {
            TuiMode::Status => 0,
            TuiMode::Add => 1,
            TuiMode::Browse => 2,
        }
    }

    fn from_index(i: usize) -> Self {
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
    fn symbol(&self) -> &'static str {
        match self {
            FileStatus::Unchanged => " ",
            FileStatus::Modified => "M",
            FileStatus::New => "+",
            FileStatus::Deleted => "-",
            FileStatus::Untracked => "?",
        }
    }

    fn color(&self) -> Color {
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

/// TUI application state
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
    pub browse_dir: PathBuf,  // Current directory for Add mode file browser
    pub config_dirty: bool,   // Track if config needs saving on exit
    pub index_dirty: bool,    // Track if index needs saving on exit
    pub commits: Vec<GitCommit>,  // Git commit history for restore
    // Restore state
    pub restore_view: RestoreView,
    pub selected_commit: Option<usize>,  // Index into commits
    pub restore_files: Vec<RestoreFile>, // Files from selected commit
    pub restore_list_state: ListState,   // Separate list state for restore
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
            browse_dir: home,
            config_dirty: false,
            index_dirty: false,
            commits: Vec::new(),
            restore_view: RestoreView::Commits,
            selected_commit: None,
            restore_files: Vec::new(),
            restore_list_state: ListState::default(),
        };
        app.refresh_files();
        app.load_commits();
        if !app.files.is_empty() {
            app.list_state.select(Some(0));
        }
        app
    }

    /// Load git commit history
    fn load_commits(&mut self) {
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

            self.files.push(DisplayFile {
                path: file.clone(),
                display_path: self.display_path(file),
                status,
                size,
                backup_size,
                is_tracked: true,
                backup_mode: Some(backup_mode),
                is_dir: false,
            });
        }

        // Add deleted files from index
        for (path, entry) in &self.index.files {
            if !current_set.contains(path) {
                self.files.push(DisplayFile {
                    path: path.clone(),
                    display_path: self.display_path(path),
                    status: FileStatus::Deleted,
                    size: None,
                    backup_size: Some(entry.size),
                    is_tracked: true,
                    backup_mode: None,
                    is_dir: false,
                });
            }
        }

        // Sort by path
        self.files.sort_by(|a, b| a.path.cmp(&b.path));
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
                    expanded == path || path.starts_with(&expanded)
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
        if let Some(parent) = self.browse_dir.parent() {
            self.browse_dir = parent.to_path_buf();
            self.list_state.select(Some(0));
            self.refresh_files();
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

    fn display_path(&self, path: &Path) -> String {
        if let Some(home) = dirs::home_dir() {
            if let Ok(rel) = path.strip_prefix(&home) {
                return format!("~/{}", rel.display());
            }
        }
        path.display().to_string()
    }

    fn get_file_mode(&self, file: &PathBuf) -> BackupMode {
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

    /// Get current list length and state based on mode
    fn get_current_list_info(&self) -> (usize, &ListState) {
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
    fn set_current_selection(&mut self, i: usize) {
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
        let next = (self.mode.index() + 1) % 3;
        self.mode = TuiMode::from_index(next);
        self.selected.clear();
        self.reset_mode_state();
        self.refresh_files();
    }

    pub fn prev_mode(&mut self) {
        let prev = if self.mode.index() == 0 {
            2
        } else {
            self.mode.index() - 1
        };
        self.mode = TuiMode::from_index(prev);
        self.selected.clear();
        self.reset_mode_state();
        self.refresh_files();
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
        if let Some(i) = self.list_state.selected() {
            if i < self.files.len() {
                let file = &self.files[i];
                let pattern = file.display_path.clone();

                if file.is_tracked {
                    // Remove from tracking
                    if let Some(pos) = self
                        .config
                        .tracked_files
                        .iter()
                        .position(|p| p.path() == pattern)
                    {
                        self.config.tracked_files.remove(pos);
                        self.config_dirty = true;
                        self.message = Some(format!("Removed: {} (saves on exit)", pattern));
                    }
                } else {
                    // Add to tracking
                    self.config
                        .tracked_files
                        .push(TrackedPattern::simple(&pattern));
                    self.config_dirty = true;
                    self.message = Some(format!("Added: {} (saves on exit)", pattern));
                }

                self.refresh_files();
            }
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

    /// Perform backup of selected or all tracked files
    pub fn perform_backup(&mut self) {
        use chrono::Local;

        self.message = Some("Running backup...".to_string());

        // Generate timestamp-based commit message
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let commit_msg = format!("Backup {}", timestamp);

        // Get current executable path to run backup command
        let exe_path = match std::env::current_exe() {
            Ok(path) => path,
            Err(e) => {
                self.message = Some(format!("Cannot find executable: {}", e));
                return;
            }
        };

        // Run backup command using the current executable
        let output = std::process::Command::new(&exe_path)
            .args(["backup", "--message", &commit_msg])
            .output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    self.message = Some(format!("Backup complete: {}", commit_msg));
                    // Reload commits after backup
                    self.load_commits();
                    self.refresh_files();
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    self.message = Some(format!("Backup failed: {}", stderr.trim()));
                }
            }
            Err(e) => {
                self.message = Some(format!("Backup error: {}", e));
            }
        }
    }
}

/// Format file size
fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1}M", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1}K", bytes as f64 / 1024.0)
    } else {
        format!("{}B", bytes)
    }
}

/// Run the TUI application
pub fn run(config: Config, index: Index, config_path: PathBuf, index_path: PathBuf, data_dir: PathBuf) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(config, index, config_path, index_path, data_dir);

    // Main loop
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            // Clear message on any keypress
            app.message = None;

            if app.show_help {
                match key.code {
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.help_scroll = app.help_scroll.saturating_add(1);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.help_scroll = app.help_scroll.saturating_sub(1);
                    }
                    KeyCode::PageDown => {
                        app.help_scroll = app.help_scroll.saturating_add(10);
                    }
                    KeyCode::PageUp => {
                        app.help_scroll = app.help_scroll.saturating_sub(10);
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        app.help_scroll = 0;
                    }
                    _ => {
                        // Any other key closes help
                        app.show_help = false;
                        app.help_scroll = 0;
                    }
                }
                continue;
            }

            if app.add_mode {
                match key.code {
                    KeyCode::Enter => {
                        if !app.add_input.is_empty() {
                            let pattern = app.add_input.clone();
                            app.config
                                .tracked_files
                                .push(TrackedPattern::simple(&pattern));
                            app.config_dirty = true;
                            app.message = Some(format!("Added: {} (saves on exit)", pattern));
                            app.add_input.clear();
                            app.refresh_files();
                        }
                        app.add_mode = false;
                    }
                    KeyCode::Esc => {
                        app.add_input.clear();
                        app.add_mode = false;
                    }
                    KeyCode::Backspace => {
                        app.add_input.pop();
                    }
                    KeyCode::Char(c) => {
                        app.add_input.push(c);
                    }
                    _ => {}
                }
                continue;
            }

            match key.code {
                KeyCode::Char('q') => {
                    app.should_quit = true;
                }
                KeyCode::Esc => {
                    // In Add mode, Esc goes to parent; at home, quits
                    if app.mode == TuiMode::Add {
                        let home = dirs::home_dir().unwrap_or_default();
                        if app.browse_dir == home {
                            app.should_quit = true;
                        } else {
                            app.parent_directory();
                        }
                    } else {
                        app.should_quit = true;
                    }
                }
                KeyCode::Char('?') | KeyCode::F(1) => {
                    app.show_help = true;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    app.next();
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    app.previous();
                }
                KeyCode::Tab => {
                    app.next_mode();
                }
                KeyCode::BackTab => {
                    app.prev_mode();
                }
                KeyCode::Char(' ') => {
                    app.toggle_select();
                    app.next();
                }
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                    // Mode-specific Enter behavior
                    match app.mode {
                        TuiMode::Add => {
                            // In Add mode, Enter enters directories or adds files to tracking
                            if let Some(i) = app.list_state.selected() {
                                if i < app.files.len() && app.files[i].is_dir {
                                    app.enter_directory();
                                } else {
                                    app.toggle_tracking();
                                }
                            }
                        }
                        TuiMode::Status => {
                            // In Status mode, Enter does nothing (use 'b' to backup)
                            app.message = Some("Press 'b' to backup, 'd' to remove from tracking".to_string());
                        }
                        TuiMode::Browse => {
                            // In Restore mode, Enter selects commit or restores files
                            match app.restore_view {
                                RestoreView::Commits => {
                                    // Select commit and show its files
                                    app.select_commit();
                                }
                                RestoreView::Files => {
                                    // Restore selected files
                                    app.perform_restore();
                                }
                            }
                        }
                    }
                }
                KeyCode::Char('b') => {
                    // Backup - only in Status mode
                    if app.mode == TuiMode::Status {
                        app.perform_backup();
                    } else {
                        app.message = Some("Switch to Tracked Files tab to run backup".to_string());
                    }
                }
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
                    // In Add mode, go to parent directory
                    // In Restore mode files view, go back to commits
                    if app.mode == TuiMode::Add {
                        app.parent_directory();
                    } else if app.mode == TuiMode::Browse && app.restore_view == RestoreView::Files {
                        app.back_to_commits();
                    }
                }
                KeyCode::Char('~') => {
                    // Go to home directory
                    app.home_directory();
                }
                KeyCode::Char('a') => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        app.select_all();
                    } else {
                        app.add_mode = true;
                    }
                }
                KeyCode::Char('d') | KeyCode::Delete => {
                    // In Status mode, 'd' removes from tracking config
                    // In other modes, removes from index
                    if app.mode == TuiMode::Status {
                        app.toggle_tracking();  // This removes tracked files
                    } else {
                        app.remove_from_index();
                    }
                }
                KeyCode::Char('r') => {
                    app.refresh_files();
                    app.message = Some("Refreshed".to_string());
                }
                KeyCode::Char('g') => {
                    app.list_state.select(Some(0));
                }
                KeyCode::Char('G') => {
                    if !app.files.is_empty() {
                        app.list_state.select(Some(app.files.len() - 1));
                    }
                }
                _ => {}
            }
        }

        if app.should_quit {
            // Save any pending changes before quitting
            app.save_if_dirty()?;
            return Ok(());
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tabs
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Status bar
        ])
        .split(f.area());

    // Tabs
    let titles: Vec<Line> = TuiMode::titles()
        .iter()
        .map(|t| Line::from(*t))
        .collect();
    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Dot Matrix "),
        )
        .select(app.mode.index())
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, chunks[0]);

    // Main content
    if app.show_help {
        render_help(f, chunks[1], app.help_scroll);
    } else if app.add_mode {
        render_add_input(f, chunks[1], app);
    } else {
        render_file_list(f, chunks[1], app);
    }

    // Status bar
    render_status_bar(f, chunks[2], app);
}

fn render_file_list(f: &mut Frame, area: Rect, app: &App) {
    // Handle Browse/Restore mode specially
    if app.mode == TuiMode::Browse {
        render_restore_view(f, area, app);
        return;
    }

    let items: Vec<ListItem> = app
        .files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let selected_marker = if app.selected.contains(&i) { "*" } else { " " };

            // In Add mode, show simpler view for file browser
            if app.mode == TuiMode::Add {
                let icon = if file.is_dir { "/" } else { " " };
                let color = if file.is_dir {
                    Color::Blue
                } else if file.is_tracked {
                    Color::Green
                } else {
                    Color::White
                };

                let size_str = file
                    .size
                    .map(format_size)
                    .unwrap_or_else(|| "".to_string());

                let tracked_marker = if file.is_tracked { " [tracked]" } else { "" };

                let line = Line::from(vec![
                    Span::raw(format!("{} ", selected_marker)),
                    Span::styled(icon, Style::default().fg(Color::Blue)),
                    Span::styled(
                        file.display_path.clone(),
                        Style::default().fg(color).add_modifier(if file.is_dir {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                    ),
                    Span::styled(tracked_marker, Style::default().fg(Color::Green)),
                    Span::raw(format!("  {}", size_str)),
                ]);

                ListItem::new(line)
            } else {
                // Status mode - show full info
                let status_symbol = file.status.symbol();
                let mode_indicator = match file.backup_mode {
                    Some(BackupMode::Archive) => "[A]",
                    Some(BackupMode::Incremental) => "[I]",
                    None => "   ",
                };

                let size_str = file
                    .size
                    .map(format_size)
                    .unwrap_or_else(|| "---".to_string());

                let line = Line::from(vec![
                    Span::raw(format!("{} ", selected_marker)),
                    Span::styled(
                        format!("{} ", status_symbol),
                        Style::default().fg(file.status.color()),
                    ),
                    Span::raw(format!("{} ", mode_indicator)),
                    Span::styled(
                        file.display_path.clone(),
                        Style::default().fg(if file.is_tracked {
                            Color::White
                        } else {
                            Color::DarkGray
                        }),
                    ),
                    Span::raw(format!("  {}", size_str)),
                ]);

                ListItem::new(line)
            }
        })
        .collect();

    let title = match app.mode {
        TuiMode::Status => " Your Tracked Files - Shows backup status and changes ".to_string(),
        TuiMode::Browse => " Restore ".to_string(), // Won't be reached
        TuiMode::Add => {
            // Show current path in Add mode with hint
            let path_display = if let Some(home) = dirs::home_dir() {
                if let Ok(rel) = app.browse_dir.strip_prefix(&home) {
                    format!(" ~/{} - Select a file and press Enter to track it ", rel.display())
                } else {
                    format!(" {} - Select a file and press Enter to track it ", app.browse_dir.display())
                }
            } else {
                format!(" {} - Select a file and press Enter to track it ", app.browse_dir.display())
            };
            path_display
        }
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut app.list_state.clone());
}

fn render_restore_view(f: &mut Frame, area: Rect, app: &App) {
    match app.restore_view {
        RestoreView::Commits => {
            // Show commit history
            let items: Vec<ListItem> = app
                .commits
                .iter()
                .enumerate()
                .map(|(i, commit)| {
                    let selected_marker = if app.selected.contains(&i) { "*" } else { " " };

                    // Parse date to show only date and time
                    let date_short = if commit.date.len() > 19 {
                        &commit.date[..19]
                    } else {
                        &commit.date
                    };

                    let line = Line::from(vec![
                        Span::raw(format!("{} ", selected_marker)),
                        Span::styled(
                            format!("{} ", commit.short_hash),
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::styled(
                            format!("{} ", date_short),
                            Style::default().fg(Color::Cyan),
                        ),
                        Span::raw(commit.message.clone()),
                    ]);

                    ListItem::new(line)
                })
                .collect();

            let title = " Backup History - Select a backup to restore from (Enter to select) ";

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(title))
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            f.render_stateful_widget(list, area, &mut app.restore_list_state.clone());
        }
        RestoreView::Files => {
            // Show files from selected commit
            let items: Vec<ListItem> = app
                .restore_files
                .iter()
                .enumerate()
                .map(|(i, file)| {
                    let selected_marker = if app.selected.contains(&i) { "*" } else { " " };

                    // Status indicator
                    let (status, color) = if !file.exists_locally {
                        ("NEW", Color::Cyan)  // File doesn't exist locally
                    } else if file.local_differs {
                        ("CHG", Color::Yellow)  // Local file is different
                    } else {
                        ("OK ", Color::Green)  // File matches backup
                    };

                    let size_str = format_size(file.size);

                    let line = Line::from(vec![
                        Span::raw(format!("{} ", selected_marker)),
                        Span::styled(
                            format!("{} ", status),
                            Style::default().fg(color),
                        ),
                        Span::raw(format!("{}  ", size_str)),
                        Span::styled(
                            file.display_path.clone(),
                            Style::default().fg(if file.local_differs {
                                Color::White
                            } else {
                                Color::DarkGray
                            }),
                        ),
                    ]);

                    ListItem::new(line)
                })
                .collect();

            let commit_info = app.selected_commit
                .and_then(|i| app.commits.get(i))
                .map(|c| format!("{} - {}", c.short_hash, c.message))
                .unwrap_or_else(|| "Unknown".to_string());

            let title = format!(
                " Files in backup: {} - Enter to restore, Backspace to go back ",
                commit_info
            );

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(title))
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            f.render_stateful_widget(list, area, &mut app.restore_list_state.clone());
        }
    }
}

fn render_help(f: &mut Frame, area: Rect, scroll: u16) {
    let help_text = vec![
        "",
        "  WHAT EACH TAB DOES",
        "  ==================",
        "  Tracked Files  View files you're backing up and their status",
        "  Add Files      Browse your computer to add files to backup",
        "  Restore        Recover files from previous backups",
        "",
        "  STATUS SYMBOLS (Tracked Files tab)",
        "  ===================================",
        "  (space) = Backed up and unchanged",
        "  M       = Modified since last backup",
        "  +       = New, not yet backed up",
        "  -       = Deleted from your system",
        "",
        "  MODE INDICATORS",
        "  ===============",
        "  [I]      = Incremental backup mode (content-addressed, deduped)",
        "  [A]      = Archive backup mode (compressed tarball)",
        "",
        "  NAVIGATION",
        "  ==========",
        "  j/Down      Move down          Tab         Next tab",
        "  k/Up        Move up            Shift+Tab   Previous tab",
        "  g           Go to top          ?/F1        Show this help",
        "  G           Go to bottom       q           Quit (saves changes)",
        "",
        "  TRACKED FILES TAB",
        "  =================",
        "  b           Run backup now",
        "  d/Delete    Stop tracking this file",
        "  r           Refresh list",
        "",
        "  ADD FILES TAB",
        "  =============",
        "  Enter/l     Open folder / Add file to tracking",
        "  Backspace/h Go back             a           Type a path manually",
        "  ~           Go to home folder",
        "",
        "  RESTORE TAB",
        "  ===========",
        "  Enter       Select backup / Restore file(s)",
        "  Backspace   Go back to backup list",
        "  Space       Select multiple files",
        "",
        "  RESTORE SYMBOLS",
        "  ===============",
        "  NEW       = File missing locally (will be created)",
        "  CHG       = Local file differs from backup",
        "  OK        = Local file matches backup",
        "",
        "  Note: Changes are saved when you quit (q)",
        "",
        "  Scroll: Up/Down/j/k  |  Press any other key to close",
    ];

    let text: Vec<Line> = help_text.iter().map(|s| Line::from(*s)).collect();
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(" Help (scroll with arrows) "))
        .style(Style::default().fg(Color::White))
        .scroll((scroll, 0));

    f.render_widget(paragraph, area);
}

fn render_add_input(f: &mut Frame, area: Rect, app: &App) {
    let input_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let input = Paragraph::new(app.add_input.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Add files/folders to backup (Enter to confirm, Esc to cancel) "),
        )
        .style(Style::default().fg(Color::Yellow));

    f.render_widget(input, input_area[0]);

    let hints = [
        "",
        "  Type a path to add to your backup:",
        "",
        "    ~/.bashrc             Add a single file",
        "    ~/.config/nvim/**     Add all files in folder (recursive)",
        "    ~/.config/nvim/*      Add files in folder (not recursive)",
        "    /etc/nginx/*.conf     Add all .conf files in a folder",
        "",
    ];

    let hint_text: Vec<Line> = hints.iter().map(|s| Line::from(*s)).collect();
    let hint_para = Paragraph::new(hint_text)
        .block(Block::default().borders(Borders::ALL).title(" Hints "))
        .style(Style::default().fg(Color::DarkGray));

    f.render_widget(hint_para, input_area[1]);
}

fn render_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let status = if let Some(ref msg) = app.message {
        msg.clone()
    } else {
        let selected_count = app.selected.len();

        // Get total count based on current mode/view
        let (total, mode_hint) = match app.mode {
            TuiMode::Status => (app.files.len(), "b: backup | d: remove"),
            TuiMode::Browse => {
                match app.restore_view {
                    RestoreView::Commits => (app.commits.len(), "Enter: select backup"),
                    RestoreView::Files => (app.restore_files.len(), "Enter: restore | Backspace: back"),
                }
            }
            TuiMode::Add => (app.files.len(), "Enter: add file | a: type path"),
        };

        if selected_count > 0 {
            format!(
                " {} selected | {} total | {} | Tab: switch tab | ?: help | q: quit",
                selected_count, total, mode_hint
            )
        } else {
            format!(
                " {} items | {} | Tab: switch tab | ?: help | q: quit",
                total, mode_hint
            )
        }
    };

    let status_bar = Paragraph::new(status)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan));

    f.render_widget(status_bar, area);
}
