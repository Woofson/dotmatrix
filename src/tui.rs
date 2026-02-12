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
    Browse,   // Browse and restore from backup
    Add,      // Add new files to tracking
}

impl TuiMode {
    fn titles() -> Vec<&'static str> {
        vec!["Backup Status", "Restore", "Add Files"]
    }

    fn index(&self) -> usize {
        match self {
            TuiMode::Status => 0,
            TuiMode::Browse => 1,
            TuiMode::Add => 2,
        }
    }

    fn from_index(i: usize) -> Self {
        match i {
            0 => TuiMode::Status,
            1 => TuiMode::Browse,
            _ => TuiMode::Add,
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
    pub message: Option<String>,
    pub should_quit: bool,
    pub show_help: bool,
    pub add_input: String,
    pub add_mode: bool,
    pub browse_dir: PathBuf,  // Current directory for Add mode file browser
}

impl App {
    pub fn new(config: Config, index: Index, config_path: PathBuf, index_path: PathBuf) -> Self {
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
            message: None,
            should_quit: false,
            show_help: false,
            add_input: String::new(),
            add_mode: false,
            browse_dir: home,
        };
        app.refresh_files();
        if !app.files.is_empty() {
            app.list_state.select(Some(0));
        }
        app
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
        if self.files.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.files.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.files.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn toggle_select(&mut self) {
        if let Some(i) = self.list_state.selected() {
            if self.selected.contains(&i) {
                self.selected.remove(&i);
            } else {
                self.selected.insert(i);
            }
        }
    }

    pub fn select_all(&mut self) {
        if self.selected.len() == self.files.len() {
            self.selected.clear();
        } else {
            self.selected = (0..self.files.len()).collect();
        }
    }

    pub fn next_mode(&mut self) {
        let next = (self.mode.index() + 1) % 3;
        self.mode = TuiMode::from_index(next);
        self.refresh_files();
    }

    pub fn prev_mode(&mut self) {
        let prev = if self.mode.index() == 0 {
            2
        } else {
            self.mode.index() - 1
        };
        self.mode = TuiMode::from_index(prev);
        self.refresh_files();
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
                        if self.config.save(&self.config_path).is_ok() {
                            self.message = Some(format!("Removed: {}", pattern));
                        }
                    }
                } else {
                    // Add to tracking
                    self.config
                        .tracked_files
                        .push(TrackedPattern::simple(&pattern));
                    if self.config.save(&self.config_path).is_ok() {
                        self.message = Some(format!("Added: {}", pattern));
                    }
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
            if self.index.save(&self.index_path).is_ok() {
                self.message = Some(format!("Removed {} file(s) from index", removed));
            }
            self.selected.clear();
            self.refresh_files();
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
pub fn run(config: Config, index: Index, config_path: PathBuf, index_path: PathBuf) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(config, index, config_path, index_path);

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
                app.show_help = false;
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
                            if app.config.save(&app.config_path).is_ok() {
                                app.message = Some(format!("Added: {}", pattern));
                            }
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
                    // In Add mode, Enter/Right/l enters directories
                    if app.mode == TuiMode::Add {
                        if let Some(i) = app.list_state.selected() {
                            if i < app.files.len() && app.files[i].is_dir {
                                app.enter_directory();
                            } else {
                                app.toggle_tracking();
                            }
                        }
                    } else {
                        app.toggle_tracking();
                    }
                }
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
                    // In Add mode, go to parent directory
                    if app.mode == TuiMode::Add {
                        app.parent_directory();
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
                    app.remove_from_index();
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
        render_help(f, chunks[1]);
    } else if app.add_mode {
        render_add_input(f, chunks[1], app);
    } else {
        render_file_list(f, chunks[1], app);
    }

    // Status bar
    render_status_bar(f, chunks[2], app);
}

fn render_file_list(f: &mut Frame, area: Rect, app: &App) {
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
                // Status/Browse mode - show full info
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
        TuiMode::Status => " Your Monitored Files - Shows what's backed up and any changes ".to_string(),
        TuiMode::Browse => " Restore Files - Select files to restore from backup ".to_string(),
        TuiMode::Add => {
            // Show current path in Add mode with hint
            let path_display = if let Some(home) = dirs::home_dir() {
                if let Ok(rel) = app.browse_dir.strip_prefix(&home) {
                    format!(" Browse: ~/{} - Press Enter to add files to backup ", rel.display())
                } else {
                    format!(" Browse: {} - Press Enter to add files to backup ", app.browse_dir.display())
                }
            } else {
                format!(" Browse: {} - Press Enter to add files to backup ", app.browse_dir.display())
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

fn render_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        "",
        "  WHAT EACH TAB DOES",
        "  ==================",
        "  Backup Status  See your monitored files and their backup state",
        "  Restore        Browse backups and restore files to your system",
        "  Add Files      Browse your system to add new files to backup",
        "",
        "  STATUS SYMBOLS",
        "  ==============",
        "  (space) = File is backed up and unchanged",
        "  M       = File was Modified since last backup",
        "  +       = New file, not yet backed up",
        "  -       = File was Deleted from your system",
        "",
        "  NAVIGATION",
        "  ==========",
        "  j/Down      Move down          Tab         Next tab",
        "  k/Up        Move up            Shift+Tab   Previous tab",
        "  g           Go to top          ?/F1        Show this help",
        "  G           Go to bottom       q           Quit",
        "",
        "  ADD FILES TAB",
        "  =============",
        "  Enter/l     Enter directory    a           Add pattern manually",
        "  Backspace/h Parent directory   ~           Go to home",
        "",
        "  ACTIONS",
        "  =======",
        "  Space       Select/deselect file(s)",
        "  Ctrl+a      Select all / Deselect all",
        "  Enter       Add to backup (Add tab) / Enter dir",
        "  d/Delete    Remove from backup tracking",
        "  r           Refresh file list",
        "",
        "  Press any key to close",
    ];

    let text: Vec<Line> = help_text.iter().map(|s| Line::from(*s)).collect();
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(" Help "))
        .style(Style::default().fg(Color::White));

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
        let total = app.files.len();

        let mode_hint = match app.mode {
            TuiMode::Status => "Viewing backup status",
            TuiMode::Browse => "Select files to restore",
            TuiMode::Add => "Browse and add files to backup",
        };

        if selected_count > 0 {
            format!(
                " {} selected | {} total | {} | Tab: switch tab | ?: help | q: quit",
                selected_count, total, mode_hint
            )
        } else {
            format!(
                " {} files | {} | Tab: switch tab | ?: help | q: quit",
                total, mode_hint
            )
        }
    };

    let status_bar = Paragraph::new(status)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan));

    f.render_widget(status_bar, area);
}
