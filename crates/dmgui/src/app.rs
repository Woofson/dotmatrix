//! Application state and logic for the GUI
//!
//! Manages the core state shared between UI rendering and input handling.

use age::secrecy::SecretString;
use dmcore::{
    backup_project_incremental_encrypted_with_message, contract_path, expand_path,
    get_remote_status, hash_file, init_project_repo, project_needs_password,
    recent_commits, retrieve_file_from, retrieve_file_from_encrypted, scan_project,
    Config, Index, Manifest, ProjectSummary, RemoteStatus, TrackMode,
};
use egui::Color32;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use syntect::highlighting::{Style as SyntectStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::easy::HighlightLines;

use crate::state::*;
use crate::theme::Colors;

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

/// Application state
pub struct GuiApp {
    pub mode: Mode,
    pub config: Config,
    pub manifest: Manifest,
    pub index: Index,

    // Project view state
    pub projects: Vec<DisplayProject>,
    pub visible_items: Vec<ProjectViewItem>,
    pub project_selected: Option<usize>,
    pub expanded_projects: HashSet<String>,

    // Add mode state
    pub browse_dir: PathBuf,
    pub browse_files: Vec<BrowseFile>,
    pub browse_selected: Option<usize>,
    pub target_project: Option<String>,
    pub default_track_mode: TrackMode,

    // Restore state (three-level view: projects -> commits -> files)
    pub restore_view: RestoreView,
    pub backup_projects: Vec<BackupProject>,
    pub backup_project_selected: Option<usize>,
    pub selected_backup_project: Option<String>,
    pub commits: Vec<CommitInfo>,
    pub commit_selected: Option<usize>,
    pub selected_commit: Option<usize>,
    pub restore_files: Vec<RestoreFile>,
    pub restore_selected: HashSet<usize>,
    pub restore_file_selected: Option<usize>,

    // UI state
    pub message: Option<(String, bool)>, // (message, is_error)
    pub should_quit: bool,
    pub show_help: bool,
    pub show_about: bool,

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
    pub delete_target: Option<String>,

    // Git remote configuration state
    pub setting_remote: bool,
    pub remote_input: String,

    // Custom commit message state
    pub entering_commit_msg: bool,
    pub commit_msg_input: String,

    // Recursive add state
    pub recursive_preview: Option<RecursivePreviewState>,

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
    pub viewer_line_numbers: bool,
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,

    // Restore confirmation state
    pub restore_confirm: RestoreConfirmState,

    // GUI-specific state
    pub text_input_focus: bool,
}

impl GuiApp {
    pub fn new() -> anyhow::Result<Self> {
        let config = Config::load()?;
        let manifest = Manifest::load()?;
        let index = Index::load()?;

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));

        let mut app = GuiApp {
            mode: Mode::Projects,
            config,
            manifest,
            index,
            projects: Vec::new(),
            visible_items: Vec::new(),
            project_selected: None,
            expanded_projects: HashSet::new(),
            browse_dir: home,
            browse_files: Vec::new(),
            browse_selected: None,
            target_project: None,
            default_track_mode: TrackMode::Both,
            restore_view: RestoreView::Projects,
            backup_projects: Vec::new(),
            backup_project_selected: None,
            selected_backup_project: None,
            commits: Vec::new(),
            commit_selected: None,
            selected_commit: None,
            restore_files: Vec::new(),
            restore_selected: HashSet::new(),
            restore_file_selected: None,
            message: None,
            should_quit: false,
            show_help: false,
            show_about: false,
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
            password_prompt_visible: false,
            password_input: String::new(),
            password_purpose: PasswordPurpose::default(),
            encryption_password: None,
            project_remote_status: HashMap::new(),
            viewer_visible: false,
            viewer_content: Vec::new(),
            viewer_scroll: 0,
            viewer_title: String::new(),
            viewer_line_numbers: true,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            restore_confirm: RestoreConfirmState::default(),
            text_input_focus: false,
        };

        app.refresh_projects();
        app.refresh_browse();
        app.scan_backup_projects();

        Ok(app)
    }

    /// Scan the data directory for all available backup projects
    pub fn scan_backup_projects(&mut self) {
        self.backup_projects.clear();

        let projects_dir = match self.config.data_dir() {
            Ok(d) => d.join("projects"),
            Err(_) => return,
        };

        if !projects_dir.exists() {
            return;
        }

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

        self.backup_projects.sort_by(|a, b| a.name.cmp(&b.name));

        if !self.backup_projects.is_empty() {
            self.backup_project_selected = Some(0);
        }
    }

    fn get_project_backup_info(&self, project_dir: &Path) -> (usize, Option<String>) {
        let count_output = std::process::Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .current_dir(project_dir)
            .output();

        let commit_count = count_output
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
            .unwrap_or(0);

        let date_output = std::process::Command::new("git")
            .args(["log", "-1", "--format=%ai"])
            .current_dir(project_dir)
            .output();

        let last_backup = date_output
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                let date = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if date.len() > 19 {
                    date[..19].to_string()
                } else {
                    date
                }
            });

        (commit_count, last_backup)
    }

    pub fn load_commits_for_project(&mut self, project_name: &str) {
        self.commits.clear();

        if let Ok(project_dir) = self.config.project_dir(project_name) {
            if let Ok(commits) = recent_commits(&project_dir, 100) {
                self.commits = commits.into_iter().map(CommitInfo::from).collect();
            }
        }

        if !self.commits.is_empty() {
            self.commit_selected = Some(0);
        }
    }

    pub fn select_backup_project(&mut self) {
        if let Some(idx) = self.backup_project_selected {
            if let Some(project) = self.backup_projects.get(idx) {
                let name = project.name.clone();
                self.selected_backup_project = Some(name.clone());
                self.load_commits_for_project(&name);
                self.restore_view = RestoreView::Commits;
            }
        }
    }

    pub fn back_to_backup_projects(&mut self) {
        self.restore_view = RestoreView::Projects;
        self.selected_backup_project = None;
        self.commits.clear();
        self.selected_commit = None;
    }

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
                let remote_status = self.project_remote_status.get(&name).cloned();

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

                self.visible_items.push(ProjectViewItem::Project {
                    name: name.clone(),
                    file_count: project.file_count(),
                    synced: summary.synced,
                    drifted: summary.drifted,
                    new_files: summary.new,
                    missing: summary.missing,
                    expanded,
                    remote_status: remote_status.clone(),
                });

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
                    synced: summary.synced,
                    drifted: summary.drifted,
                    new_files: summary.new,
                    missing: summary.missing,
                    expanded,
                    files,
                    remote_status,
                });
            }
        }

        if self.project_selected.is_none() && !self.visible_items.is_empty() {
            self.project_selected = Some(0);
        }
    }

    pub fn refresh_browse(&mut self) {
        self.browse_files.clear();

        if self.browse_dir.parent().is_some() {
            self.browse_files.push(BrowseFile {
                path: self.browse_dir.join(".."),
                name: "..".to_string(),
                is_dir: true,
                size: None,
                tracked_in: Vec::new(),
            });
        }

        if let Ok(entries) = std::fs::read_dir(&self.browse_dir) {
            let mut files: Vec<BrowseFile> = entries
                .filter_map(|e| e.ok())
                .filter_map(|entry| {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();

                    if name == "." || name == ".." {
                        return None;
                    }

                    let is_dir = path.is_dir();
                    let size = if is_dir {
                        None
                    } else {
                        std::fs::metadata(&path).ok().map(|m| m.len())
                    };

                    let contracted = contract_path(&path);
                    let tracked_in: Vec<String> = if is_dir {
                        let dir_prefix = format!("{}/", contracted);
                        self.manifest
                            .projects
                            .iter()
                            .filter(|(_, p)| {
                                p.files.iter().any(|f| {
                                    f.path.starts_with(&dir_prefix) || f.path == contracted
                                })
                            })
                            .map(|(name, _)| name.clone())
                            .collect()
                    } else {
                        self.manifest
                            .projects
                            .iter()
                            .filter(|(_, p)| p.files.iter().any(|f| f.path == contracted))
                            .map(|(name, _)| name.clone())
                            .collect()
                    };

                    Some(BrowseFile {
                        path,
                        name,
                        is_dir,
                        size,
                        tracked_in,
                    })
                })
                .collect();

            files.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            });

            self.browse_files.extend(files);
        }

        if !self.browse_files.is_empty() {
            self.browse_selected = Some(0);
        }
    }

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

            if path.ends_with("..") {
                if let Some(idx) = self
                    .browse_files
                    .iter()
                    .position(|f| f.path == previous_dir)
                {
                    self.browse_selected = Some(idx);
                }
            }
        }
    }

    pub fn go_home(&mut self) {
        self.browse_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        self.refresh_browse();
    }

    pub fn selected_item(&self) -> Option<&ProjectViewItem> {
        self.project_selected
            .and_then(|i| self.visible_items.get(i))
    }

    pub fn selected_project_name(&self) -> Option<String> {
        match self.selected_item()? {
            ProjectViewItem::Project { name, .. } => Some(name.clone()),
            ProjectViewItem::File { project_name, .. } => Some(project_name.clone()),
        }
    }

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

        let current_idx = self.project_selected.unwrap_or(0);
        self.refresh_projects();

        let new_idx = current_idx.min(self.visible_items.len().saturating_sub(1));
        if !self.visible_items.is_empty() {
            self.project_selected = Some(new_idx);
        }
    }

    pub fn collapse_selected_project(&mut self) {
        let name = match self.selected_project_name() {
            Some(n) => n,
            None => return,
        };

        if self.expanded_projects.contains(&name) {
            self.expanded_projects.remove(&name);
            let current_idx = self.project_selected.unwrap_or(0);
            self.refresh_projects();
            if let Some(idx) = self.visible_items.iter().position(|item| {
                matches!(item, ProjectViewItem::Project { name: n, .. } if n == &name)
            }) {
                self.project_selected = Some(idx);
            } else {
                let new_idx = current_idx.min(self.visible_items.len().saturating_sub(1));
                self.project_selected = Some(new_idx);
            }
        }
    }

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

    pub fn toggle_project_encryption(&mut self) {
        let project_name = match self.selected_project_name() {
            Some(name) => name,
            None => {
                self.message = Some(("Select a project to toggle encryption".to_string(), true));
                return;
            }
        };

        if let Some(project) = self.manifest.get_project_mut(&project_name) {
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

    pub fn add_file_to_project(&mut self, path: &PathBuf) -> bool {
        let project_name = match &self.target_project {
            Some(name) => name.clone(),
            None => {
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
                let saved_selection = self.browse_selected;
                self.refresh_browse();
                if let Some(idx) = saved_selection {
                    let max_idx = self.browse_files.len().saturating_sub(1);
                    self.browse_selected = Some(idx.min(max_idx));
                }
                return true;
            } else {
                self.message = Some(("File already tracked".to_string(), true));
            }
        }
        false
    }

    pub fn untrack_file(&mut self, path: &PathBuf) -> bool {
        let contracted = contract_path(path);
        let mut removed_from = Vec::new();

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
            let saved_selection = self.browse_selected;
            self.refresh_browse();
            if let Some(idx) = saved_selection {
                let max_idx = self.browse_files.len().saturating_sub(1);
                self.browse_selected = Some(idx.min(max_idx));
            }
            return true;
        }

        self.message = Some(("File not tracked".to_string(), true));
        false
    }

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

        if project_needs_password(&project) && self.encryption_password.is_none() {
            self.show_password_prompt(PasswordPurpose::Backup);
            return;
        }

        self.backup_project_with_password(project_name, project);
    }

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

        if project_needs_password(&project) && self.encryption_password.is_none() {
            self.show_password_prompt(PasswordPurpose::Backup);
            return;
        }

        self.entering_commit_msg = true;
        self.commit_msg_input.clear();
    }

    pub fn cancel_commit_msg(&mut self) {
        self.entering_commit_msg = false;
        self.commit_msg_input.clear();
    }

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

        let msg = if custom_msg.is_empty() {
            None
        } else {
            Some(custom_msg)
        };
        self.backup_project_internal(project_name, project, msg);
    }

    fn backup_project_with_password(&mut self, project_name: String, project: dmcore::Project) {
        self.backup_project_internal(project_name, project, None);
    }

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
                init_project_repo(&config, &name)?;

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

    pub fn show_password_prompt(&mut self, purpose: PasswordPurpose) {
        self.password_prompt_visible = true;
        self.password_input.clear();
        self.password_purpose = purpose;
    }

    pub fn cancel_password(&mut self) {
        self.password_prompt_visible = false;
        self.password_input.clear();
    }

    pub fn confirm_password(&mut self) {
        if self.password_input.is_empty() {
            self.message = Some(("Password cannot be empty".to_string(), true));
            return;
        }

        self.encryption_password = Some(SecretString::from(self.password_input.clone()));
        self.password_input.clear();
        self.password_prompt_visible = false;

        match self.password_purpose {
            PasswordPurpose::Backup => {
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
                self.perform_restore_with_password();
            }
        }
    }

    pub fn refresh_remote_status(&mut self) {
        self.project_remote_status.clear();

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
        self.refresh_projects();
    }

    pub fn poll_operation(&mut self) {
        if let Some(ref rx) = self.op_receiver {
            match rx.try_recv() {
                Ok(result) => {
                    self.busy = false;
                    self.op_receiver = None;
                    self.message = Some((result.message, !result.success));

                    if result.success {
                        if let Some(name) = self.selected_project_name() {
                            if let Ok(index) = Index::load_for_project(&self.config, &name) {
                                self.index = index;
                            }
                        }
                        self.refresh_projects();
                        self.scan_backup_projects();
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.spinner_frame = (self.spinner_frame + 1) % crate::theme::SPINNER_FRAMES.len();
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.busy = false;
                    self.op_receiver = None;
                }
            }
        }
    }

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

    pub fn select_commit(&mut self) {
        if let Some(i) = self.commit_selected {
            if i < self.commits.len() {
                self.selected_commit = Some(i);
                self.load_commit_files(&self.commits[i].hash.clone());
                self.restore_view = RestoreView::Files;
                self.restore_selected.clear();
                if !self.restore_files.is_empty() {
                    self.restore_file_selected = Some(0);
                }
            }
        }
    }

    pub fn back_to_commits(&mut self) {
        self.restore_view = RestoreView::Commits;
        self.restore_files.clear();
        self.restore_selected.clear();
        if let Some(idx) = self.selected_commit {
            self.commit_selected = Some(idx);
        }
        self.selected_commit = None;
    }

    pub fn load_commit_files(&mut self, commit_hash: &str) {
        self.restore_files.clear();

        let project_name = match &self.selected_backup_project {
            Some(n) => n.clone(),
            None => return,
        };

        let project_dir = match self.config.project_dir(&project_name) {
            Ok(d) => d,
            Err(_) => return,
        };

        let output = std::process::Command::new("git")
            .args(["show", &format!("{}:index.json", commit_hash)])
            .current_dir(&project_dir)
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let content = String::from_utf8_lossy(&output.stdout);

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

    fn add_restore_file(&mut self, path: PathBuf, hash: String, size: u64, encrypted: bool) {
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

    fn remap_path_to_current_home(path: &PathBuf) -> PathBuf {
        let current_home = match dirs::home_dir() {
            Some(h) => h,
            None => return path.clone(),
        };

        let path_str = path.to_string_lossy();

        if path_str.starts_with("~/") {
            return current_home.join(&path_str[2..]);
        }

        if path_str.starts_with("/home/") {
            if let Some(rest) = path_str.strip_prefix("/home/") {
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

        path.clone()
    }

    pub fn show_restore_confirm(&mut self) {
        let indices: Vec<usize> = if self.restore_selected.is_empty() {
            self.restore_file_selected.into_iter().collect()
        } else {
            self.restore_selected.iter().cloned().collect()
        };

        if indices.is_empty() {
            self.message = Some(("No files selected for restore".to_string(), true));
            return;
        }

        let will_overwrite = indices
            .iter()
            .filter(|&&i| {
                self.restore_files
                    .get(i)
                    .map(|f| f.exists_locally)
                    .unwrap_or(false)
            })
            .count();

        self.restore_confirm = RestoreConfirmState {
            visible: true,
            destination: RestoreDestination::Original,
            custom_path: String::new(),
            entering_path: false,
            files_to_restore: indices,
            will_overwrite,
            selected_idx: 0,
            scroll_offset: 0,
            preview_mode: RestorePreviewMode::FileList,
        };
    }

    pub fn perform_restore(&mut self) {
        let indices: Vec<usize> = if self.restore_selected.is_empty() {
            self.restore_file_selected.into_iter().collect()
        } else {
            self.restore_selected.iter().cloned().collect()
        };

        if indices.is_empty() {
            self.message = Some(("No files selected for restore".to_string(), true));
            return;
        }

        // Check if any file needs password
        let needs_password = indices
            .iter()
            .any(|&i| self.restore_files.get(i).map(|f| f.encrypted).unwrap_or(false));

        if needs_password && self.encryption_password.is_none() {
            self.show_password_prompt(PasswordPurpose::Restore);
            return;
        }

        self.perform_restore_with_password();
    }

    fn perform_restore_with_password(&mut self) {
        let indices: Vec<usize> = if self.restore_selected.is_empty() {
            self.restore_file_selected.into_iter().collect()
        } else {
            self.restore_selected.iter().cloned().collect()
        };

        let project_name = match &self.selected_backup_project {
            Some(n) => n.clone(),
            None => return,
        };

        let store_dir = match self.config.project_store_dir(&project_name) {
            Ok(d) => d,
            Err(_) => return,
        };

        let mut restored = 0;
        let mut errors = 0;

        for &idx in &indices {
            if let Some(file) = self.restore_files.get(idx) {
                // Create parent directory if needed
                if let Some(parent) = file.restore_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }

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
                    Ok(_) => restored += 1,
                    Err(_) => errors += 1,
                }
            }
        }

        if errors > 0 {
            self.message = Some((
                format!("Restored {} files, {} errors", restored, errors),
                true,
            ));
        } else {
            self.message = Some((format!("Restored {} files", restored), false));
        }

        self.restore_selected.clear();
    }

    pub fn create_project(&mut self) {
        let name = self.project_input.trim().to_string();
        self.creating_project = false;
        self.project_input.clear();

        if name.is_empty() {
            self.message = Some(("Project name cannot be empty".to_string(), true));
            return;
        }

        if self.manifest.projects.contains_key(&name) {
            self.message = Some((format!("Project '{}' already exists", name), true));
            return;
        }

        self.manifest.add_project(name.clone(), dmcore::Project::default());
        self.manifest_dirty = true;
        self.target_project = Some(name.clone());
        self.message = Some((format!("Created project '{}'", name), false));
        self.refresh_projects();
    }

    pub fn delete_project(&mut self) {
        let name = match &self.delete_target {
            Some(n) => n.clone(),
            None => return,
        };

        self.confirm_delete = false;
        self.delete_target = None;

        if self.manifest.remove_project(&name).is_some() {
            self.manifest_dirty = true;
            if self.target_project.as_ref() == Some(&name) {
                self.target_project = self.manifest.projects.keys().next().cloned();
            }
            self.message = Some((format!("Deleted project '{}'", name), false));
            self.refresh_projects();
        }
    }

    pub fn set_git_remote(&mut self) {
        let project_name = match self.selected_project_name() {
            Some(name) => name,
            None => return,
        };

        let remote_url = self.remote_input.trim().to_string();
        self.setting_remote = false;
        self.remote_input.clear();

        if let Ok(project_dir) = self.config.project_dir(&project_name) {
            if dmcore::is_git_repo(&project_dir) {
                if let Err(e) = dmcore::set_remote_url(&project_dir, &remote_url) {
                    self.message = Some((format!("Failed to set remote: {}", e), true));
                    return;
                }
                self.message = Some((format!("Remote set to {}", remote_url), false));
                self.refresh_remote_status();
            } else {
                self.message = Some(("Project has no git repository yet".to_string(), true));
            }
        }
    }

    pub fn push_project(&mut self) {
        let project_name = match self.selected_project_name() {
            Some(name) => name,
            None => {
                self.message = Some(("No project selected".to_string(), true));
                return;
            }
        };

        let config = self.config.clone();
        let name = project_name.clone();

        let (tx, rx) = mpsc::channel();
        self.op_receiver = Some(rx);
        self.busy = true;
        self.busy_message = format!("Pushing {}...", project_name);

        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<String> {
                let project_dir = config.project_dir(&name)?;
                dmcore::push(&project_dir)?;
                Ok("Push successful".to_string())
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

    pub fn pull_project(&mut self) {
        let project_name = match self.selected_project_name() {
            Some(name) => name,
            None => {
                self.message = Some(("No project selected".to_string(), true));
                return;
            }
        };

        let config = self.config.clone();
        let name = project_name.clone();

        let (tx, rx) = mpsc::channel();
        self.op_receiver = Some(rx);
        self.busy = true;
        self.busy_message = format!("Pulling {}...", project_name);

        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<String> {
                let project_dir = config.project_dir(&name)?;
                dmcore::pull(&project_dir)?;
                Ok("Pull successful".to_string())
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

    pub fn cycle_target_project(&mut self) {
        let names: Vec<_> = self.manifest.projects.keys().cloned().collect();
        if names.is_empty() {
            return;
        }

        let current_idx = self
            .target_project
            .as_ref()
            .and_then(|t| names.iter().position(|n| n == t))
            .unwrap_or(0);

        let next_idx = (current_idx + 1) % names.len();
        self.target_project = Some(names[next_idx].clone());
        self.message = Some((format!("Target: {}", names[next_idx]), false));
    }

    pub fn cycle_default_track_mode(&mut self) {
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
        self.message = Some((format!("Default track mode: {}", mode_name), false));
    }

    pub fn load_file_into_viewer(&mut self, path: &Path, title: &str) {
        self.viewer_content.clear();
        self.viewer_scroll = 0;
        self.viewer_title = title.to_string();

        // Check if it's a directory (conf.d style)
        if path.is_dir() {
            self.load_directory_into_viewer(path);
            self.viewer_visible = true;
            return;
        }

        // Read file content
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => {
                // Try reading as binary
                match fs::read(path) {
                    Ok(_bytes) => {
                        self.viewer_content.push(ViewerLine {
                            spans: vec![("[Binary file]".to_string(), Colors::DARK_GRAY)],
                            file_header: false,
                        });
                        self.viewer_visible = true;
                        return;
                    }
                    Err(e) => {
                        self.message = Some((format!("Failed to read file: {}", e), true));
                        return;
                    }
                }
            }
        };

        // Get syntax for highlighting
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let syntax = self
            .syntax_set
            .find_syntax_by_extension(extension)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        for line in content.lines() {
            let ranges = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();

            let spans: Vec<(String, Color32)> = ranges
                .into_iter()
                .map(|(style, text)| {
                    let color = syntect_to_egui_color(style);
                    (text.to_string(), color)
                })
                .collect();

            self.viewer_content.push(ViewerLine {
                spans,
                file_header: false,
            });
        }

        self.viewer_visible = true;
    }

    fn load_directory_into_viewer(&mut self, dir: &Path) {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .map(|rd| rd.filter_map(|e| e.ok()).collect())
            .unwrap_or_default();

        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            if path.is_file() {
                // Add file header
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                self.viewer_content.push(ViewerLine {
                    spans: vec![(format!("=== {} ===", file_name), Colors::CYAN)],
                    file_header: true,
                });

                // Read and highlight file content
                if let Ok(content) = fs::read_to_string(&path) {
                    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let syntax = self
                        .syntax_set
                        .find_syntax_by_extension(extension)
                        .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

                    let theme = &self.theme_set.themes["base16-ocean.dark"];
                    let mut highlighter = HighlightLines::new(syntax, theme);

                    for line in content.lines() {
                        let ranges = highlighter
                            .highlight_line(line, &self.syntax_set)
                            .unwrap_or_default();

                        let spans: Vec<(String, Color32)> = ranges
                            .into_iter()
                            .map(|(style, text)| {
                                let color = syntect_to_egui_color(style);
                                (text.to_string(), color)
                            })
                            .collect();

                        self.viewer_content.push(ViewerLine {
                            spans,
                            file_header: false,
                        });
                    }
                }

                // Add blank line between files
                self.viewer_content.push(ViewerLine {
                    spans: vec![],
                    file_header: false,
                });
            }
        }
    }

    pub fn close_viewer(&mut self) {
        self.viewer_visible = false;
        self.viewer_content.clear();
    }

    pub fn save_state(&mut self) {
        if self.manifest_dirty {
            if let Err(e) = self.manifest.save() {
                self.message = Some((format!("Failed to save manifest: {}", e), true));
            } else {
                self.manifest_dirty = false;
            }
        }

        if self.index_dirty {
            if let Err(e) = self.index.save() {
                self.message = Some((format!("Failed to save index: {}", e), true));
            } else {
                self.index_dirty = false;
            }
        }
    }

    pub fn start_recursive_preview(&mut self) {
        if let Some(idx) = self.browse_selected {
            if let Some(file) = self.browse_files.get(idx) {
                if file.is_dir && file.name != ".." {
                    let source_dir = file.path.clone();
                    let mut preview_files = Vec::new();

                    self.scan_directory_recursive(&source_dir, &mut preview_files);

                    if preview_files.is_empty() {
                        self.message = Some(("No files found in directory".to_string(), true));
                        return;
                    }

                    let selected_files: HashSet<usize> = (0..preview_files.len()).collect();

                    self.recursive_preview = Some(RecursivePreviewState {
                        source_dir,
                        preview_files,
                        selected_files,
                        selected_idx: 0,
                    });
                }
            }
        }
    }

    fn scan_directory_recursive(&self, dir: &Path, files: &mut Vec<PreviewFile>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();

                // Skip hidden files and common ignored directories
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name == "node_modules" || name == "target" {
                    continue;
                }

                if path.is_dir() {
                    self.scan_directory_recursive(&path, files);
                } else if path.is_file() {
                    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    let display_path = if let Some(home) = dirs::home_dir() {
                        if let Ok(rel) = path.strip_prefix(&home) {
                            format!("~/{}", rel.display())
                        } else {
                            path.display().to_string()
                        }
                    } else {
                        path.display().to_string()
                    };

                    files.push(PreviewFile {
                        path,
                        display_path,
                        size,
                        track_mode: self.default_track_mode,
                    });
                }
            }
        }
    }

    pub fn cancel_recursive_preview(&mut self) {
        self.recursive_preview = None;
    }

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

        let mut added = 0;
        if let Some(project) = self.manifest.get_project_mut(&project_name) {
            for idx in &preview.selected_files {
                if let Some(file) = preview.preview_files.get(*idx) {
                    let contracted = contract_path(&file.path);
                    if project.add_path_with_mode(&contracted, file.track_mode) {
                        added += 1;
                    }
                }
            }
        }

        if added > 0 {
            self.manifest_dirty = true;
            self.message = Some((format!("Added {} files to {}", added, project_name), false));
            self.refresh_projects();
            self.refresh_browse();
        } else {
            self.message = Some(("No new files added".to_string(), false));
        }
    }
}

/// Convert syntect style to egui Color32
fn syntect_to_egui_color(style: SyntectStyle) -> Color32 {
    Color32::from_rgb(style.foreground.r, style.foreground.g, style.foreground.b)
}
