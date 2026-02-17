//! Terminal UI frontend using ratatui + crossterm.
//!
//! This module contains only the rendering code. The shared application state
//! and logic is in the `app` module.

use crate::app::{
    App, AddSubMode, RestoreView, TuiMode,
    format_size, SPINNER_FRAMES,
};
use crate::config::{BackupMode, Config, TrackedPattern};
use crate::index::Index;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame, Terminal,
};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

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

        // Check for background task completion
        app.poll_backup();

        // Poll for events with timeout (allows spinner to animate)
        let poll_timeout = if app.busy {
            Duration::from_millis(80) // Fast polling for spinner animation
        } else {
            Duration::from_millis(250) // Slower polling when idle
        };

        if !event::poll(poll_timeout)? {
            continue; // No event, loop again (updates spinner)
        }

        if let Event::Key(key) = event::read()? {
            // Only handle key press events (Windows sends both Press and Release)
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // Clear message on any keypress (but not while busy)
            if !app.busy {
                app.message = None;
            }

            // Ignore most keys while busy (allow quit)
            if app.busy {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    // Allow quitting while busy (backup continues in background)
                    app.should_quit = true;
                }
                continue;
            }

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

            // Handle file viewer mode
            if app.viewer_visible {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        app.close_viewer();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let max_scroll = app.viewer_content.len().saturating_sub(1);
                        if app.viewer_scroll < max_scroll {
                            app.viewer_scroll += 1;
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.viewer_scroll = app.viewer_scroll.saturating_sub(1);
                    }
                    KeyCode::PageDown => {
                        let max_scroll = app.viewer_content.len().saturating_sub(1);
                        app.viewer_scroll = (app.viewer_scroll + 20).min(max_scroll);
                    }
                    KeyCode::PageUp => {
                        app.viewer_scroll = app.viewer_scroll.saturating_sub(20);
                    }
                    KeyCode::Char('g') | KeyCode::Home => {
                        app.viewer_scroll = 0;
                    }
                    KeyCode::Char('G') | KeyCode::End => {
                        app.viewer_scroll = app.viewer_content.len().saturating_sub(1);
                    }
                    _ => {}
                }
                continue;
            }

            // Handle recursive preview mode
            if app.add_sub_mode == AddSubMode::RecursivePreview {
                match key.code {
                    KeyCode::Esc => {
                        app.cancel_recursive_preview();
                    }
                    KeyCode::Enter => {
                        app.confirm_recursive_add();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.preview_next();
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.preview_previous();
                    }
                    KeyCode::Char(' ') => {
                        app.toggle_preview_file();
                        app.preview_next();
                    }
                    KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.toggle_all_preview_files();
                    }
                    KeyCode::PageDown => {
                        app.preview_page_down();
                    }
                    KeyCode::PageUp => {
                        app.preview_page_up();
                    }
                    KeyCode::Char('q') => {
                        app.cancel_recursive_preview();
                    }
                    _ => {}
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

            if app.backup_message_mode {
                match key.code {
                    KeyCode::Enter => {
                        let msg = if app.backup_message_input.is_empty() {
                            None
                        } else {
                            Some(app.backup_message_input.clone())
                        };
                        app.backup_message_input.clear();
                        app.backup_message_mode = false;
                        app.perform_backup(msg);
                    }
                    KeyCode::Esc => {
                        app.backup_message_input.clear();
                        app.backup_message_mode = false;
                        app.message = Some("Backup cancelled".to_string());
                    }
                    KeyCode::Backspace => {
                        app.backup_message_input.pop();
                    }
                    KeyCode::Char(c) => {
                        app.backup_message_input.push(c);
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
                            // In Status mode, Right/Enter expands folder
                            if let Some(i) = app.list_state.selected() {
                                if i < app.files.len() && app.files[i].is_folder_node {
                                    app.expand_folder();
                                } else {
                                    app.message = Some("Press 'b' to backup, 'd' to remove from tracking".to_string());
                                }
                            }
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
                        app.perform_backup(None);
                    } else {
                        app.message = Some("Switch to Tracked Files tab to run backup".to_string());
                    }
                }
                KeyCode::Char('B') => {
                    // Backup with custom message - only in Status mode
                    if app.mode == TuiMode::Status {
                        app.backup_message_mode = true;
                    } else {
                        app.message = Some("Switch to Tracked Files tab to run backup".to_string());
                    }
                }
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
                    // In Add mode, go to parent directory
                    // In Restore mode files view, go back to commits
                    // In Status mode, collapse folder
                    if app.mode == TuiMode::Add {
                        app.parent_directory();
                    } else if app.mode == TuiMode::Status {
                        app.collapse_folder();
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
                KeyCode::Char('A') => {
                    // Add folder as pattern (with /**)
                    if app.mode == TuiMode::Add {
                        app.add_folder_pattern();
                    }
                }
                KeyCode::Char('d') | KeyCode::Delete => {
                    // In Status mode, 'd' removes from tracking config
                    // In Add mode, 'd' removes folder/file patterns from tracking
                    // In other modes, removes from index
                    if app.mode == TuiMode::Status {
                        app.toggle_tracking();  // This removes tracked files
                    } else if app.mode == TuiMode::Add {
                        app.remove_from_tracking_in_browser();
                    } else {
                        app.remove_from_index();
                    }
                }
                KeyCode::Char('r') => {
                    app.reload_index();
                    app.refresh_files();
                    app.message = Some("Refreshed".to_string());
                }
                KeyCode::Char('R') => {
                    // Start recursive add preview in Add mode
                    if app.mode == TuiMode::Add {
                        app.start_recursive_preview();
                    } else {
                        app.message = Some("Switch to Add Files tab to add recursively".to_string());
                    }
                }
                KeyCode::Char('g') => {
                    app.list_state.select(Some(0));
                }
                KeyCode::Char('G') => {
                    if !app.files.is_empty() {
                        app.list_state.select(Some(app.files.len() - 1));
                    }
                }
                KeyCode::PageDown => {
                    app.page_down();
                }
                KeyCode::PageUp => {
                    app.page_up();
                }
                KeyCode::Char('e') => {
                    // Expand all folders (Status mode)
                    app.expand_all_folders();
                }
                KeyCode::Char('E') => {
                    // Collapse all folders (Status mode)
                    app.collapse_all_folders();
                }
                KeyCode::Char('v') => {
                    // View file contents
                    app.open_viewer();
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
    } else if app.viewer_visible {
        render_viewer(f, chunks[1], app);
    } else if app.add_mode {
        render_add_input(f, chunks[1], app);
    } else if app.backup_message_mode {
        render_backup_input(f, chunks[1], app);
    } else if app.add_sub_mode == AddSubMode::RecursivePreview {
        render_recursive_preview(f, chunks[1], app);
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
                // Status mode - show tree view
                if file.is_folder_node {
                    // Folder node
                    let expanded = app.expanded_folders.contains(&file.path);
                    let expand_icon = if expanded { "▼" } else { "▶" };

                    // Status indicator for folder
                    let (folder_status, status_color) = if file.modified_count > 0 {
                        ("M", Color::Yellow)
                    } else if file.new_count > 0 {
                        ("+", Color::Cyan)
                    } else {
                        (" ", Color::Green)
                    };

                    let count_str = format!("({} files", file.child_count);
                    let mod_str = if file.modified_count > 0 {
                        format!(", {} modified", file.modified_count)
                    } else {
                        String::new()
                    };
                    let new_str = if file.new_count > 0 {
                        format!(", {} new", file.new_count)
                    } else {
                        String::new()
                    };
                    let stats = format!("{}{}{})", count_str, mod_str, new_str);

                    let line = Line::from(vec![
                        Span::raw(format!("{} ", selected_marker)),
                        Span::styled(
                            format!("{} ", folder_status),
                            Style::default().fg(status_color),
                        ),
                        Span::styled(
                            format!("{} ", expand_icon),
                            Style::default().fg(Color::Blue),
                        ),
                        Span::styled(
                            file.display_path.clone(),
                            Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("  {}", stats),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]);

                    return ListItem::new(line);
                }

                // Regular file (nested under folder)
                let indent = "    ".repeat(file.depth);
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
                    Span::raw(format!("{}{} ", indent, mode_indicator)),
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

fn render_recursive_preview(f: &mut Frame, area: Rect, app: &App) {
    let preview = match &app.recursive_preview {
        Some(p) => p,
        None => return,
    };

    // Split area for header info and file list
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Header with stats
            Constraint::Min(0),    // File list
        ])
        .split(area);

    // Header with directory info and stats
    let source_display = if let Some(home) = dirs::home_dir() {
        if let Ok(rel) = preview.source_dir.strip_prefix(&home) {
            format!("~/{}", rel.display())
        } else {
            preview.source_dir.display().to_string()
        }
    } else {
        preview.source_dir.display().to_string()
    };

    let selected_count = preview.selected_files.len();
    let total_count = preview.preview_files.len();

    let header_lines = vec![
        Line::from(vec![
            Span::raw("Adding recursively: "),
            Span::styled(&source_display, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(format!("{}", selected_count), Style::default().fg(Color::Green)),
            Span::raw(" files selected | "),
            Span::styled(format!("{}", preview.gitignore_excluded), Style::default().fg(Color::DarkGray)),
            Span::raw(" excluded by .gitignore | "),
            Span::styled(format!("{}", preview.config_excluded), Style::default().fg(Color::DarkGray)),
            Span::raw(" excluded by config"),
        ]),
        Line::from(vec![
            Span::styled("Space", Style::default().fg(Color::Cyan)),
            Span::raw(": toggle | "),
            Span::styled("Ctrl+A", Style::default().fg(Color::Cyan)),
            Span::raw(": select all | "),
            Span::styled("Enter", Style::default().fg(Color::Green)),
            Span::raw(": add selected | "),
            Span::styled("Esc", Style::default().fg(Color::Red)),
            Span::raw(": cancel"),
        ]),
    ];

    let header = Paragraph::new(header_lines)
        .block(Block::default().borders(Borders::ALL).title(" Recursive Add Preview "))
        .style(Style::default().fg(Color::White));

    f.render_widget(header, chunks[0]);

    // File list
    let items: Vec<ListItem> = preview
        .preview_files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let selected_marker = if preview.selected_files.contains(&i) { "[x]" } else { "[ ]" };
            let size_str = format_size(file.size);

            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", selected_marker),
                    Style::default().fg(if preview.selected_files.contains(&i) {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::raw(format!("{}  ", size_str)),
                Span::styled(
                    file.display_path.clone(),
                    Style::default().fg(if preview.selected_files.contains(&i) {
                        Color::White
                    } else {
                        Color::DarkGray
                    }),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list_title = format!(" Files ({}/{} selected) ", selected_count, total_count);
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(list_title))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(list, chunks[1], &mut preview.preview_list_state.clone());
}

fn render_help(f: &mut Frame, area: Rect, scroll: u16) {
    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let key_style = Style::default().fg(Color::Cyan);
    let dim_style = Style::default().fg(Color::DarkGray);

    let help_lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(Span::styled("  WHAT EACH TAB DOES", header_style)),
        Line::from(Span::styled("  ==================", dim_style)),
        Line::from(vec![
            Span::styled("  Tracked Files  ", key_style),
            Span::raw("View files you're backing up and their status"),
        ]),
        Line::from(vec![
            Span::styled("  Add Files      ", key_style),
            Span::raw("Browse your computer to add files to backup"),
        ]),
        Line::from(vec![
            Span::styled("  Restore        ", key_style),
            Span::raw("Recover files from previous backups"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  STATUS SYMBOLS (Tracked Files tab)", header_style)),
        Line::from(Span::styled("  ===================================", dim_style)),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("(space)", Style::default().fg(Color::Green)),
            Span::raw(" = Backed up and unchanged"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("M", Style::default().fg(Color::Yellow)),
            Span::raw("       = Modified since last backup"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("+", Style::default().fg(Color::Cyan)),
            Span::raw("       = New, not yet backed up"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("-", Style::default().fg(Color::DarkGray)),
            Span::raw("       = Deleted from your system"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  MODE INDICATORS", header_style)),
        Line::from(Span::styled("  ===============", dim_style)),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("[I]", Style::default().fg(Color::Blue)),
            Span::raw("    = Incremental backup (content-addressed, deduped)"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("[A]", Style::default().fg(Color::Magenta)),
            Span::raw("    = Archive backup (compressed tarball)"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  NAVIGATION", header_style)),
        Line::from(Span::styled("  ==========", dim_style)),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("j/Down", key_style),
            Span::raw("      Move down          "),
            Span::styled("Tab", key_style),
            Span::raw("         Next tab"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("k/Up", key_style),
            Span::raw("        Move up            "),
            Span::styled("Shift+Tab", key_style),
            Span::raw("   Previous tab"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("g", key_style),
            Span::raw("           Go to top          "),
            Span::styled("?/F1", key_style),
            Span::raw("        Show this help"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("G", key_style),
            Span::raw("           Go to bottom       "),
            Span::styled("q", key_style),
            Span::raw("           Quit (saves changes)"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("PgDn/PgUp", key_style),
            Span::raw("   Page down/up       "),
            Span::styled("v", key_style),
            Span::raw("           View file contents"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  TRACKED FILES TAB", header_style)),
        Line::from(Span::styled("  =================", dim_style)),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("b", key_style),
            Span::raw("           Run backup now (auto message)"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("B", key_style),
            Span::raw("           Run backup with custom message"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("d/Delete", key_style),
            Span::raw("    Stop tracking this file"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("r", key_style),
            Span::raw("           Refresh list"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Right/l", key_style),
            Span::raw("     Expand folder"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Left/h", key_style),
            Span::raw("      Collapse folder"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("e", key_style),
            Span::raw("           Expand all folders"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("E", key_style),
            Span::raw("           Collapse all folders"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  ADD FILES TAB", header_style)),
        Line::from(Span::styled("  =============", dim_style)),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Enter/l", key_style),
            Span::raw("     Open folder / Add file to tracking"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("A", key_style),
            Span::raw("           Add folder as pattern (with /**)"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("R", key_style),
            Span::raw("           Recursive add (select individual files)"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Backspace/h", key_style),
            Span::raw(" Go back to parent directory"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("~", key_style),
            Span::raw("           Go to home folder"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("a", key_style),
            Span::raw("           Type a path manually"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("d", key_style),
            Span::raw("           Untrack file/folder (no files deleted)"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  RECURSIVE ADD PREVIEW", header_style)),
        Line::from(Span::styled("  =====================", dim_style)),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Space", key_style),
            Span::raw("       Toggle file selection"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Ctrl+A", key_style),
            Span::raw("      Select/deselect all"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Enter", key_style),
            Span::raw("       Confirm and add selected files"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Esc", key_style),
            Span::raw("         Cancel and return to browser"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  RESTORE TAB", header_style)),
        Line::from(Span::styled("  ===========", dim_style)),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Enter", key_style),
            Span::raw("       Select backup / Restore file(s)"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Backspace", key_style),
            Span::raw("   Go back to backup list"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Space", key_style),
            Span::raw("       Select multiple files"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  RESTORE SYMBOLS", header_style)),
        Line::from(Span::styled("  ===============", dim_style)),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("NEW", Style::default().fg(Color::Cyan)),
            Span::raw("     = File missing locally (will be created)"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("CHG", Style::default().fg(Color::Yellow)),
            Span::raw("     = Local file differs from backup"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("OK", Style::default().fg(Color::Green)),
            Span::raw("      = Local file matches backup"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Note: Changes are saved when you quit (q)", dim_style)),
        Line::from(""),
        Line::from(Span::styled("  Scroll: Up/Down/j/k  |  Press any other key to close", dim_style)),
    ];

    let paragraph = Paragraph::new(help_lines)
        .block(Block::default().borders(Borders::ALL).title(" Help (scroll with arrows) "))
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

fn render_backup_input(f: &mut Frame, area: Rect, app: &App) {
    let input_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    let input = Paragraph::new(app.backup_message_input.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Backup commit message (Enter to confirm, Esc to cancel) "),
        )
        .style(Style::default().fg(Color::Yellow));

    f.render_widget(input, input_area[0]);

    let hints = vec![
        Line::from(""),
        Line::from("  Enter a commit message for this backup:"),
        Line::from(""),
        Line::from(vec![
            Span::raw("    "),
            Span::styled("Enter", Style::default().fg(Color::Green)),
            Span::raw("  Run backup with this message"),
        ]),
        Line::from(vec![
            Span::raw("    "),
            Span::styled("Esc", Style::default().fg(Color::Red)),
            Span::raw("    Cancel backup"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Leave empty for auto-generated timestamp message", Style::default().fg(Color::DarkGray))),
        Line::from(""),
    ];

    let hint_para = Paragraph::new(hints)
        .block(Block::default().borders(Borders::ALL).title(" Hints "))
        .style(Style::default().fg(Color::White));

    f.render_widget(hint_para, input_area[1]);
}

fn render_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let (status, style) = if app.busy {
        // Show spinner and busy message
        let spinner = SPINNER_FRAMES[app.spinner_frame];
        let msg = format!(" {} {}", spinner, app.busy_message);
        (msg, Style::default().fg(Color::Yellow))
    } else if let Some(ref msg) = app.message {
        (msg.clone(), Style::default().fg(Color::Cyan))
    } else {
        let selected_count = app.selected.len();

        // Get total count based on current mode/view
        let (total, mode_hint) = match app.mode {
            TuiMode::Status => (app.files.len(), "Right: expand | Left: collapse | b: backup | d: remove | v: view"),
            TuiMode::Browse => {
                match app.restore_view {
                    RestoreView::Commits => (app.commits.len(), "Enter: select backup"),
                    RestoreView::Files => (app.restore_files.len(), "Enter: restore | Backspace: back"),
                }
            }
            TuiMode::Add => (app.files.len(), "Enter: add/open | A: folder | R: recursive | d: untrack"),
        };

        let msg = if selected_count > 0 {
            format!(
                " {} selected | {} total | {} | Tab: switch tab | ?: help | q: quit",
                selected_count, total, mode_hint
            )
        } else {
            format!(
                " {} items | {} | Tab: switch tab | ?: help | q: quit",
                total, mode_hint
            )
        };
        (msg, Style::default().fg(Color::Cyan))
    };

    let version = env!("CARGO_PKG_VERSION");
    let status_bar = Paragraph::new(status)
        .block(Block::default().borders(Borders::ALL).title(format!(" v{} ", version)))
        .style(style);

    f.render_widget(status_bar, area);
}

fn render_viewer(f: &mut Frame, area: Rect, app: &App) {
    // Calculate visible height (subtract 2 for borders)
    let visible_height = area.height.saturating_sub(2) as usize;

    let lines: Vec<Line> = app.viewer_content
        .iter()
        .skip(app.viewer_scroll)
        .take(visible_height)
        .map(|vl| {
            if vl.file_header {
                // File header line - use first span's text
                let text = vl.spans.first()
                    .map(|(t, _)| t.clone())
                    .unwrap_or_default();
                Line::from(Span::styled(
                    text,
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ))
            } else {
                // Regular content line - convert all spans
                Line::from(
                    vl.spans
                        .iter()
                        .map(|(text, style)| Span::styled(text.clone(), *style))
                        .collect::<Vec<_>>(),
                )
            }
        })
        .collect();

    let total_lines = app.viewer_content.len();
    let scroll_pos = if total_lines > 0 {
        format!(" {}/{} ", app.viewer_scroll + 1, total_lines)
    } else {
        " 0/0 ".to_string()
    };

    let title = format!(" {} (q:close  j/k:scroll  g/G:top/bottom) {} ", app.viewer_title, scroll_pos);

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(paragraph, area);
}
