//! Application state and logic for the TUI
//!
//! Manages the core state shared between UI rendering and input handling.

use dmcore::{
    backup_incremental, contract_path, expand_path, init_repo, scan_project, stage_all, commit,
    Config, FileStatus, Index, Manifest, ProjectSummary, TrackMode,
};
use ratatui::widgets::ListState;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

/// Spinner frames for busy indicator
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// TUI mode (tab)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Projects,  // View projects and their files
    Add,       // Add files to projects
    Restore,   // Restore from backup
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

/// A displayable file entry
#[derive(Debug, Clone)]
pub struct DisplayFile {
    pub path: String,
    pub abs_path: PathBuf,
    pub status: FileStatus,
    pub size: Option<u64>,
    pub track_mode: TrackMode,
    pub is_folder: bool,
    pub depth: usize,
    pub child_count: usize,
}

/// A displayable project entry
#[derive(Debug, Clone)]
pub struct DisplayProject {
    pub name: String,
    pub description: Option<String>,
    pub file_count: usize,
    pub summary: ProjectSummary,
    pub expanded: bool,
    pub files: Vec<DisplayFile>,
}

/// Result from background operation
pub struct OpResult {
    pub success: bool,
    pub message: String,
}

/// Application state
pub struct App {
    pub mode: Mode,
    pub config: Config,
    pub manifest: Manifest,
    pub index: Index,

    // Project view state
    pub projects: Vec<DisplayProject>,
    pub project_list_state: ListState,
    pub selected_project: Option<usize>,
    pub file_list_state: ListState,
    pub expanded_projects: HashSet<String>,

    // Add mode state
    pub browse_dir: PathBuf,
    pub browse_files: Vec<BrowseFile>,
    pub browse_list_state: ListState,
    pub target_project: Option<String>,

    // Restore state
    pub restore_files: Vec<DisplayFile>,
    pub restore_list_state: ListState,

    // UI state
    pub message: Option<(String, bool)>, // (message, is_error)
    pub should_quit: bool,
    pub show_help: bool,
    pub help_scroll: u16,

    // Busy state
    pub busy: bool,
    pub busy_message: String,
    pub spinner_frame: usize,
    pub op_receiver: Option<Receiver<OpResult>>,

    // Dirty flags
    pub manifest_dirty: bool,
    pub index_dirty: bool,
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
            project_list_state: ListState::default(),
            selected_project: None,
            file_list_state: ListState::default(),
            expanded_projects: HashSet::new(),
            browse_dir: home,
            browse_files: Vec::new(),
            browse_list_state: ListState::default(),
            target_project: None,
            restore_files: Vec::new(),
            restore_list_state: ListState::default(),
            message: None,
            should_quit: false,
            show_help: false,
            help_scroll: 0,
            busy: false,
            busy_message: String::new(),
            spinner_frame: 0,
            op_receiver: None,
            manifest_dirty: false,
            index_dirty: false,
        };

        app.refresh_projects();
        app.refresh_browse();

        Ok(app)
    }

    /// Refresh the projects list
    pub fn refresh_projects(&mut self) {
        self.projects.clear();

        let mut names: Vec<_> = self.manifest.projects.keys().cloned().collect();
        names.sort();

        for name in names {
            if let Some(project) = self.manifest.get_project(&name) {
                let results = scan_project(project, &self.index);
                let summary = ProjectSummary::from_results(&results);
                let expanded = self.expanded_projects.contains(&name);

                let files: Vec<DisplayFile> = results
                    .iter()
                    .map(|r| DisplayFile {
                        path: r.path.clone(),
                        abs_path: expand_path(&r.path),
                        status: r.status,
                        size: r.current_size,
                        track_mode: r.track_mode,
                        is_folder: false,
                        depth: 0,
                        child_count: 0,
                    })
                    .collect();

                self.projects.push(DisplayProject {
                    name: name.clone(),
                    description: project.description.clone(),
                    file_count: project.file_count(),
                    summary,
                    expanded,
                    files,
                });
            }
        }

        // Select first project if none selected
        if self.selected_project.is_none() && !self.projects.is_empty() {
            self.selected_project = Some(0);
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

                    // Skip hidden files
                    if name.starts_with('.') {
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
                    let is_tracked = self.manifest.projects.values().any(|p| {
                        p.files.iter().any(|f| f.path == contracted)
                    });

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
            files.sort_by(|a, b| {
                match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                }
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
            self.browse_dir = if path.ends_with("..") {
                self.browse_dir.parent().unwrap_or(&self.browse_dir).to_path_buf()
            } else {
                path.clone()
            };
            self.refresh_browse();
        }
    }

    /// Toggle project expansion
    pub fn toggle_project(&mut self, idx: usize) {
        if let Some(project) = self.projects.get(idx) {
            let name = project.name.clone();
            if self.expanded_projects.contains(&name) {
                self.expanded_projects.remove(&name);
            } else {
                self.expanded_projects.insert(name);
            }
            self.refresh_projects();
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
                    self.message = Some(("No project selected. Create one first.".to_string(), true));
                    return false;
                }
            }
        };

        if let Some(project) = self.manifest.get_project_mut(&project_name) {
            let contracted = contract_path(path);
            if project.add_path(&contracted) {
                self.manifest_dirty = true;
                self.message = Some((format!("Added {} to {}", contracted, project_name), false));
                self.refresh_projects();
                self.refresh_browse();
                return true;
            } else {
                self.message = Some(("File already tracked".to_string(), true));
            }
        }
        false
    }

    /// Backup the selected project
    pub fn backup_project(&mut self) {
        let project_idx = match self.selected_project {
            Some(idx) => idx,
            None => {
                self.message = Some(("No project selected".to_string(), true));
                return;
            }
        };

        let project_name = self.projects[project_idx].name.clone();

        // Clone what we need for the background thread
        let config = self.config.clone();
        let project = match self.manifest.get_project(&project_name) {
            Some(p) => p.clone(),
            None => return,
        };
        let mut index = self.index.clone();

        let (tx, rx) = mpsc::channel();
        self.op_receiver = Some(rx);
        self.busy = true;
        self.busy_message = format!("Backing up {}...", project_name);

        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<String> {
                // Initialize git repo
                let data_dir = config.data_dir()?;
                init_repo(&data_dir)?;

                // Backup
                let result = backup_incremental(&config, &project, &mut index)?;

                // Save index
                index.save()?;

                // Git commit
                let store_dir = config.store_dir()?;
                stage_all(&store_dir)?;
                let msg = format!("Backup: {} files", result.backed_up + result.unchanged);
                commit(&store_dir, &msg)?;

                Ok(format!(
                    "Backed up {} files ({} new, {} unchanged)",
                    result.backed_up + result.unchanged,
                    result.backed_up,
                    result.unchanged
                ))
            })();

            let op_result = match result {
                Ok(msg) => OpResult { success: true, message: msg },
                Err(e) => OpResult { success: false, message: e.to_string() },
            };

            let _ = tx.send(op_result);
        });
    }

    /// Poll for background operation completion
    pub fn poll_operation(&mut self) {
        if let Some(ref rx) = self.op_receiver {
            match rx.try_recv() {
                Ok(result) => {
                    self.busy = false;
                    self.op_receiver = None;
                    self.message = Some((result.message, !result.success));

                    // Reload index after backup
                    if result.success {
                        if let Ok(index) = Index::load() {
                            self.index = index;
                        }
                        self.refresh_projects();
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
        let project_idx = match self.selected_project {
            Some(idx) => idx,
            None => {
                self.message = Some(("No project selected".to_string(), true));
                return;
            }
        };

        let project = match self.manifest.get_project(&self.projects[project_idx].name) {
            Some(p) => p,
            None => return,
        };

        let mut synced = 0;
        for file in &project.files {
            let abs_path = file.absolute_path();
            if abs_path.exists() {
                if let Ok(hash) = dmcore::hash_file(&abs_path) {
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

    /// Save dirty state
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

    /// Get the current spinner frame
    pub fn spinner(&self) -> &'static str {
        SPINNER_FRAMES[self.spinner_frame]
    }
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
