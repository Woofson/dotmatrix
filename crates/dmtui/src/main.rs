//! dmtui - TUI for dotmatrix
//!
//! Terminal user interface built with ratatui.
//! Keyboard-driven interface for managing projects.

mod app;

use app::{format_size, App, Mode};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dmcore::FileStatus;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame, Terminal,
};
use std::io;
use std::time::Duration;

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new()?;

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

    // Save any dirty state
    app.save_if_dirty()?;

    res
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        // Poll background operations
        app.poll_operation();

        // Poll for events with timeout
        let poll_timeout = if app.busy {
            Duration::from_millis(80)
        } else {
            Duration::from_millis(250)
        };

        if !event::poll(poll_timeout)? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // Clear message on keypress
            if !app.busy {
                app.message = None;
            }

            // Ignore keys while busy (except quit)
            if app.busy {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    app.should_quit = true;
                }
                continue;
            }

            // Help mode
            if app.show_help {
                match key.code {
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.help_scroll = app.help_scroll.saturating_add(1);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.help_scroll = app.help_scroll.saturating_sub(1);
                    }
                    _ => {
                        app.show_help = false;
                        app.help_scroll = 0;
                    }
                }
                continue;
            }

            // Global keys
            match key.code {
                KeyCode::Char('q') => app.should_quit = true,
                KeyCode::Char('?') => app.show_help = true,
                KeyCode::Tab => {
                    let next = (app.mode.index() + 1) % 3;
                    app.mode = Mode::from_index(next);
                }
                KeyCode::BackTab => {
                    let prev = (app.mode.index() + 2) % 3;
                    app.mode = Mode::from_index(prev);
                }
                KeyCode::Char('1') => app.mode = Mode::Projects,
                KeyCode::Char('2') => app.mode = Mode::Add,
                KeyCode::Char('3') => app.mode = Mode::Restore,
                _ => {
                    // Mode-specific keys
                    match app.mode {
                        Mode::Projects => handle_projects_keys(app, key.code),
                        Mode::Add => handle_add_keys(app, key.code),
                        Mode::Restore => handle_restore_keys(app, key.code),
                    }
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn handle_projects_keys(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.projects.is_empty() {
                let i = app.project_list_state.selected().unwrap_or(0);
                let next = (i + 1).min(app.projects.len() - 1);
                app.project_list_state.select(Some(next));
                app.selected_project = Some(next);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.projects.is_empty() {
                let i = app.project_list_state.selected().unwrap_or(0);
                let prev = i.saturating_sub(1);
                app.project_list_state.select(Some(prev));
                app.selected_project = Some(prev);
            }
        }
        KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
            if let Some(idx) = app.selected_project {
                app.toggle_project(idx);
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if let Some(idx) = app.selected_project {
                if app.projects.get(idx).map(|p| p.expanded).unwrap_or(false) {
                    app.toggle_project(idx);
                }
            }
        }
        KeyCode::Char('b') => {
            app.backup_project();
        }
        KeyCode::Char('s') => {
            app.sync_project();
        }
        KeyCode::Char('r') => {
            app.refresh_projects();
            app.message = Some(("Refreshed".to_string(), false));
        }
        _ => {}
    }
}

fn handle_add_keys(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.browse_files.is_empty() {
                let i = app.browse_list_state.selected().unwrap_or(0);
                let next = (i + 1).min(app.browse_files.len() - 1);
                app.browse_list_state.select(Some(next));
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.browse_files.is_empty() {
                let i = app.browse_list_state.selected().unwrap_or(0);
                let prev = i.saturating_sub(1);
                app.browse_list_state.select(Some(prev));
            }
        }
        KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
            if let Some(idx) = app.browse_list_state.selected() {
                if let Some(file) = app.browse_files.get(idx) {
                    let path = file.path.clone();
                    if file.is_dir {
                        app.enter_directory(&path);
                    } else {
                        app.add_file_to_project(&path);
                    }
                }
            }
        }
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
            let parent = app.browse_dir.parent().map(|p| p.to_path_buf());
            if let Some(p) = parent {
                app.browse_dir = p;
                app.refresh_browse();
            }
        }
        KeyCode::Char('a') => {
            // Add selected file
            if let Some(idx) = app.browse_list_state.selected() {
                if let Some(file) = app.browse_files.get(idx) {
                    if !file.is_dir {
                        let path = file.path.clone();
                        app.add_file_to_project(&path);
                    }
                }
            }
        }
        KeyCode::Char('~') => {
            // Go to home
            if let Some(home) = dirs::home_dir() {
                app.browse_dir = home;
                app.refresh_browse();
            }
        }
        _ => {}
    }
}

fn handle_restore_keys(_app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Down | KeyCode::Char('j') => {
            // TODO: Implement restore navigation
        }
        KeyCode::Up | KeyCode::Char('k') => {
            // TODO: Implement restore navigation
        }
        _ => {}
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tabs
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Status bar
        ])
        .split(f.area());

    // Tabs
    let titles: Vec<Line> = Mode::titles()
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let style = if i == app.mode.index() {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::from(Span::styled(format!(" {} ", t), style))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" dotmatrix "))
        .highlight_style(Style::default().fg(Color::Yellow))
        .select(app.mode.index());

    f.render_widget(tabs, chunks[0]);

    // Content based on mode
    match app.mode {
        Mode::Projects => render_projects(f, app, chunks[1]),
        Mode::Add => render_add(f, app, chunks[1]),
        Mode::Restore => render_restore(f, app, chunks[1]),
    }

    // Status bar
    render_status_bar(f, app, chunks[2]);

    // Help overlay
    if app.show_help {
        render_help(f, app);
    }
}

fn render_projects(f: &mut Frame, app: &mut App, area: Rect) {
    if app.projects.is_empty() {
        let msg = Paragraph::new("No projects. Create one with: dotmatrix new <name>")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title(" Projects "));
        f.render_widget(msg, area);
        return;
    }

    let mut items: Vec<ListItem> = Vec::new();

    for project in app.projects.iter() {
        let expand_char = if project.expanded { "▼" } else { "▶" };

        // Project header
        let status_icon = if project.summary.is_clean() { "✓" } else { "⚠" };
        let status_color = if project.summary.is_clean() {
            Color::Green
        } else {
            Color::Yellow
        };

        let header = Line::from(vec![
            Span::raw(format!("{} ", expand_char)),
            Span::styled(status_icon, Style::default().fg(status_color)),
            Span::raw(" "),
            Span::styled(
                &project.name,
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({} files)", project.file_count),
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        items.push(ListItem::new(header));

        // Expanded files
        if project.expanded {
            for file in &project.files {
                let (icon, color) = match file.status {
                    FileStatus::Synced => ("✓", Color::Green),
                    FileStatus::Drifted => ("⚠", Color::Yellow),
                    FileStatus::New => ("+", Color::Cyan),
                    FileStatus::Missing => ("✗", Color::Red),
                    FileStatus::Error => ("!", Color::Red),
                };

                let size_str = file
                    .size
                    .map(|s| format_size(s))
                    .unwrap_or_else(|| "-".to_string());

                let line = Line::from(vec![
                    Span::raw("    "),
                    Span::styled(icon, Style::default().fg(color)),
                    Span::raw(" "),
                    Span::styled(
                        format!("{:>8}", size_str),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw("  "),
                    Span::raw(&file.path),
                ]);

                items.push(ListItem::new(line));
            }
        }
    }

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Projects "))
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_stateful_widget(list, area, &mut app.project_list_state);
}

fn render_add(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    // Current directory
    let dir_line = Paragraph::new(format!(" {} ", app.browse_dir.display()))
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(dir_line, chunks[0]);

    // File list
    let items: Vec<ListItem> = app
        .browse_files
        .iter()
        .map(|file| {
            let icon = if file.is_dir { "📁" } else { "📄" };
            let tracked = if file.is_tracked { " ✓" } else { "" };
            let size_str = file
                .size
                .map(|s| format!(" {:>8}", format_size(s)))
                .unwrap_or_default();

            let style = if file.is_tracked {
                Style::default().fg(Color::DarkGray)
            } else if file.is_dir {
                Style::default().fg(Color::Blue)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(vec![
                Span::raw(format!("{} ", icon)),
                Span::styled(&file.name, style),
                Span::styled(size_str, Style::default().fg(Color::DarkGray)),
                Span::styled(tracked, Style::default().fg(Color::Green)),
            ]))
        })
        .collect();

    let target = app
        .target_project
        .as_ref()
        .or_else(|| app.projects.first().map(|p| &p.name))
        .map(|n| format!(" → {} ", n))
        .unwrap_or_default();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Add Files{}", target)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_stateful_widget(list, chunks[1], &mut app.browse_list_state);
}

fn render_restore(f: &mut Frame, _app: &mut App, area: Rect) {
    let msg = Paragraph::new("Restore view - coming soon\n\nUse CLI: dotmatrix restore <project>")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).title(" Restore "));
    f.render_widget(msg, area);
}

fn render_status_bar(f: &mut Frame, app: &mut App, area: Rect) {
    let (msg, style) = if app.busy {
        (
            format!("{} {}", app.spinner(), app.busy_message),
            Style::default().fg(Color::Yellow),
        )
    } else if let Some((ref message, is_error)) = app.message {
        let color = if is_error { Color::Red } else { Color::Green };
        (message.clone(), Style::default().fg(color))
    } else {
        let help = match app.mode {
            Mode::Projects => "↑↓:select  Enter:expand  b:backup  s:sync  ?:help  q:quit",
            Mode::Add => "↑↓:select  Enter:open/add  h:parent  ~:home  ?:help  q:quit",
            Mode::Restore => "?:help  q:quit",
        };
        (help.to_string(), Style::default().fg(Color::DarkGray))
    };

    let status = Paragraph::new(msg)
        .style(style)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(status, area);
}

fn render_help(f: &mut Frame, _app: &App) {
    let area = centered_rect(60, 70, f.area());

    let help_text = r#"
 GLOBAL KEYS
 ───────────────────────────
 Tab/1-3    Switch tabs
 ?          Show/hide help
 q          Quit

 PROJECTS TAB
 ───────────────────────────
 ↑/k ↓/j    Navigate projects
 Enter/→    Expand/collapse
 ←/h        Collapse
 b          Backup project
 s          Sync project
 r          Refresh

 ADD FILES TAB
 ───────────────────────────
 ↑/k ↓/j    Navigate files
 Enter/→    Open dir / Add file
 ←/h        Parent directory
 a          Add selected file
 ~          Go to home

 RESTORE TAB
 ───────────────────────────
 (Coming soon)
"#;

    let help = Paragraph::new(help_text)
        .style(Style::default())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help ")
                .style(Style::default().bg(Color::Black)),
        );

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(help, area);
}

/// Create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
