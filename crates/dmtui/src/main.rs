//! dmtui - TUI for dotmatrix
//!
//! Terminal user interface built with ratatui.
//! Keyboard-driven interface for managing projects.

mod app;

use app::{format_size, App, Mode, PasswordPurpose, RestoreDestination, RestoreView};
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

            // About mode
            if app.show_about {
                app.show_about = false;
                continue;
            }

            // File viewer mode
            if app.viewer_visible {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('v') => {
                        app.close_viewer();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.viewer_scroll_down(1);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.viewer_scroll_up(1);
                    }
                    KeyCode::PageDown => {
                        app.viewer_scroll_down(PAGE_SIZE);
                    }
                    KeyCode::PageUp => {
                        app.viewer_scroll_up(PAGE_SIZE);
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        app.viewer_scroll_top();
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        app.viewer_scroll_bottom();
                    }
                    KeyCode::Char('n') => {
                        app.toggle_viewer_line_numbers();
                    }
                    _ => {}
                }
                continue;
            }

            // Project creation input mode
            if app.creating_project {
                match key.code {
                    KeyCode::Enter => {
                        app.confirm_create_project();
                    }
                    KeyCode::Esc => {
                        app.cancel_create_project();
                    }
                    KeyCode::Backspace => {
                        app.project_input.pop();
                    }
                    KeyCode::Char(c) => {
                        app.project_input.push(c);
                    }
                    _ => {}
                }
                continue;
            }

            // Git remote configuration mode
            if app.setting_remote {
                match key.code {
                    KeyCode::Enter => {
                        app.confirm_set_remote();
                    }
                    KeyCode::Esc => {
                        app.cancel_set_remote();
                    }
                    KeyCode::Backspace => {
                        app.remote_input.pop();
                    }
                    KeyCode::Char(c) => {
                        app.remote_input.push(c);
                    }
                    _ => {}
                }
                continue;
            }

            // Custom commit message mode
            if app.entering_commit_msg {
                match key.code {
                    KeyCode::Enter => {
                        app.confirm_commit_msg();
                    }
                    KeyCode::Esc => {
                        app.cancel_commit_msg();
                    }
                    KeyCode::Backspace => {
                        app.commit_msg_input.pop();
                    }
                    KeyCode::Char(c) => {
                        app.commit_msg_input.push(c);
                    }
                    _ => {}
                }
                continue;
            }

            // Recursive preview mode
            if app.recursive_preview.is_some() {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
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
                    KeyCode::Char('a') => {
                        app.toggle_all_preview_files();
                    }
                    KeyCode::Char('t') => {
                        app.toggle_preview_track_mode();
                    }
                    KeyCode::Char('T') => {
                        app.set_all_preview_track_mode();
                    }
                    _ => {}
                }
                continue;
            }

            // Password prompt mode
            if app.password_prompt_visible {
                match key.code {
                    KeyCode::Enter => {
                        app.confirm_password();
                    }
                    KeyCode::Esc => {
                        app.cancel_password();
                    }
                    KeyCode::Backspace => {
                        app.password_input.pop();
                    }
                    KeyCode::Char(c) => {
                        app.password_input.push(c);
                    }
                    _ => {}
                }
                continue;
            }

            // Delete confirmation mode
            if app.confirm_delete {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                        app.confirm_delete_project();
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        app.cancel_delete();
                    }
                    _ => {}
                }
                continue;
            }

            // Restore confirmation mode
            if app.restore_confirm.visible {
                // If viewer is open (for backup/local/diff), handle viewer keys
                if app.viewer_visible {
                    match key.code {
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.viewer_scroll = app.viewer_scroll.saturating_add(1);
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.viewer_scroll = app.viewer_scroll.saturating_sub(1);
                        }
                        KeyCode::PageDown => {
                            app.viewer_scroll = app.viewer_scroll.saturating_add(20);
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
                        KeyCode::Char('n') => {
                            app.toggle_viewer_line_numbers();
                        }
                        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('v') => {
                            app.close_restore_preview();
                        }
                        _ => {}
                    }
                    continue;
                }

                if app.restore_confirm.entering_path {
                    // Path input mode
                    match key.code {
                        KeyCode::Enter => {
                            app.restore_confirm.entering_path = false;
                        }
                        KeyCode::Esc => {
                            app.restore_confirm.entering_path = false;
                            app.restore_confirm.custom_path.clear();
                            app.restore_confirm.destination = RestoreDestination::Original;
                        }
                        KeyCode::Backspace => {
                            app.restore_confirm.custom_path.pop();
                        }
                        KeyCode::Char(c) => {
                            app.restore_confirm.custom_path.push(c);
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                            app.confirm_restore();
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc | KeyCode::Char('q') => {
                            app.cancel_restore_confirm();
                        }
                        KeyCode::Char('o') | KeyCode::Char('O') => {
                            // Original location
                            app.restore_confirm.destination = RestoreDestination::Original;
                            app.restore_confirm.entering_path = false;
                        }
                        KeyCode::Char('c') | KeyCode::Char('C') => {
                            // Custom location - start entering path
                            app.restore_confirm.destination = RestoreDestination::Custom;
                            app.restore_confirm.entering_path = true;
                        }
                        KeyCode::Tab => {
                            app.toggle_restore_destination();
                        }
                        // Navigation in file list
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.restore_confirm_up();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.restore_confirm_down();
                        }
                        // View backup file
                        KeyCode::Char('b') | KeyCode::Char('B') => {
                            app.view_restore_backup();
                        }
                        // View local file
                        KeyCode::Char('l') | KeyCode::Char('L') => {
                            app.view_restore_local();
                        }
                        // View diff
                        KeyCode::Char('d') | KeyCode::Char('D') => {
                            app.view_restore_diff();
                        }
                        _ => {}
                    }
                }
                continue;
            }

            // Global keys
            match key.code {
                KeyCode::Char('q') => app.should_quit = true,
                KeyCode::Char('?') => app.show_help = true,
                KeyCode::Char('!') => app.show_about = true,
                KeyCode::Tab => {
                    let next = (app.mode.index() + 1) % 3;
                    app.mode = Mode::from_index(next);
                    // Reset restore view when entering Restore tab
                    if app.mode == Mode::Restore {
                        app.restore_view = RestoreView::Projects;
                        app.restore_selected.clear();
                        app.scan_backup_projects();
                    }
                }
                KeyCode::BackTab => {
                    let prev = (app.mode.index() + 2) % 3;
                    app.mode = Mode::from_index(prev);
                    if app.mode == Mode::Restore {
                        app.restore_view = RestoreView::Projects;
                        app.restore_selected.clear();
                        app.scan_backup_projects();
                    }
                }
                KeyCode::Char('1') => app.mode = Mode::Projects,
                KeyCode::Char('2') => app.mode = Mode::Add,
                KeyCode::Char('3') => {
                    app.mode = Mode::Restore;
                    app.restore_view = RestoreView::Projects;
                    app.restore_selected.clear();
                    app.scan_backup_projects();
                }
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

const PAGE_SIZE: usize = 10;

fn handle_projects_keys(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.visible_items.is_empty() {
                let i = app.project_list_state.selected().unwrap_or(0);
                let next = (i + 1).min(app.visible_items.len() - 1);
                app.project_list_state.select(Some(next));
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if !app.visible_items.is_empty() {
                let i = app.project_list_state.selected().unwrap_or(0);
                let prev = i.saturating_sub(1);
                app.project_list_state.select(Some(prev));
            }
        }
        KeyCode::PageDown => {
            if !app.visible_items.is_empty() {
                let i = app.project_list_state.selected().unwrap_or(0);
                let next = (i + PAGE_SIZE).min(app.visible_items.len() - 1);
                app.project_list_state.select(Some(next));
            }
        }
        KeyCode::PageUp => {
            if !app.visible_items.is_empty() {
                let i = app.project_list_state.selected().unwrap_or(0);
                let prev = i.saturating_sub(PAGE_SIZE);
                app.project_list_state.select(Some(prev));
            }
        }
        KeyCode::Home => {
            if !app.visible_items.is_empty() {
                app.project_list_state.select(Some(0));
            }
        }
        KeyCode::End => {
            if !app.visible_items.is_empty() {
                app.project_list_state.select(Some(app.visible_items.len() - 1));
            }
        }
        KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
            app.toggle_selected_project();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.collapse_selected_project();
        }
        KeyCode::Char('a') => {
            // Backup with custom commit message popup
            app.backup_project_with_message();
        }
        KeyCode::Char('A') => {
            // Silent backup (no popup)
            app.backup_project();
        }
        KeyCode::Char('s') => {
            app.sync_project();
        }
        KeyCode::Char('r') => {
            app.refresh_projects();
            app.message = Some(("Refreshed".to_string(), false));
        }
        KeyCode::Char('n') => {
            app.start_create_project();
        }
        KeyCode::Char('D') => {
            app.start_delete_project();
        }
        KeyCode::Char('x') => {
            app.toggle_encryption();
        }
        KeyCode::Char('X') => {
            // Toggle encryption for all files in project
            app.toggle_project_encryption();
        }
        KeyCode::Char('m') | KeyCode::Char('M') => {
            app.toggle_track_mode();
        }
        KeyCode::Char('S') => {
            app.save_and_reload();
        }
        KeyCode::Char('g') => {
            app.refresh_remote_status();
        }
        KeyCode::Char('G') => {
            // Set git remote URL
            app.start_set_remote();
        }
        KeyCode::Char('p') => {
            // Push to remote (project-specific)
            if let Some(name) = app.selected_project_name() {
                if let Ok(project_dir) = app.config.project_dir(&name) {
                    if dmcore::is_git_repo(&project_dir) {
                        match dmcore::push(&project_dir) {
                            Ok(msg) => app.message = Some((msg, false)),
                            Err(e) => app.message = Some((e.to_string(), true)),
                        }
                        app.refresh_remote_status();
                    } else {
                        app.message = Some(("No git repo for project. Backup first.".to_string(), true));
                    }
                }
            } else {
                app.message = Some(("No project selected".to_string(), true));
            }
        }
        KeyCode::Char('P') => {
            // Pull from remote (project-specific)
            if let Some(name) = app.selected_project_name() {
                if let Ok(project_dir) = app.config.project_dir(&name) {
                    if dmcore::is_git_repo(&project_dir) {
                        match dmcore::pull(&project_dir) {
                            Ok(msg) => app.message = Some((msg, false)),
                            Err(e) => app.message = Some((e.to_string(), true)),
                        }
                        app.refresh_remote_status();
                        app.scan_backup_projects();
                    } else {
                        app.message = Some(("No git repo for project. Backup first.".to_string(), true));
                    }
                }
            } else {
                app.message = Some(("No project selected".to_string(), true));
            }
        }
        KeyCode::Char('v') => {
            app.open_viewer();
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
        KeyCode::PageDown => {
            if !app.browse_files.is_empty() {
                let i = app.browse_list_state.selected().unwrap_or(0);
                let next = (i + PAGE_SIZE).min(app.browse_files.len() - 1);
                app.browse_list_state.select(Some(next));
            }
        }
        KeyCode::PageUp => {
            if !app.browse_files.is_empty() {
                let i = app.browse_list_state.selected().unwrap_or(0);
                let prev = i.saturating_sub(PAGE_SIZE);
                app.browse_list_state.select(Some(prev));
            }
        }
        KeyCode::Home => {
            if !app.browse_files.is_empty() {
                app.browse_list_state.select(Some(0));
            }
        }
        KeyCode::End => {
            if !app.browse_files.is_empty() {
                app.browse_list_state.select(Some(app.browse_files.len() - 1));
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
            let previous_dir = app.browse_dir.clone();
            if let Some(parent) = app.browse_dir.parent().map(|p| p.to_path_buf()) {
                app.browse_dir = parent;
                app.refresh_browse();
                // Find and select the directory we came from
                if let Some(idx) = app.browse_files.iter().position(|f| f.path == previous_dir) {
                    app.browse_list_state.select(Some(idx));
                }
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
        KeyCode::Char('p') => {
            // Cycle target project
            app.cycle_target_project();
        }
        KeyCode::Char('n') => {
            // Create new project
            app.start_create_project();
        }
        KeyCode::Char('R') => {
            // Recursive add
            app.start_recursive_preview();
        }
        KeyCode::Char('t') => {
            // Cycle track mode for adding files
            app.cycle_add_track_mode();
        }
        KeyCode::Char('u') => {
            // Untrack selected file
            if let Some(idx) = app.browse_list_state.selected() {
                if let Some(file) = app.browse_files.get(idx) {
                    if file.is_tracked() && !file.is_dir {
                        let path = file.path.clone();
                        app.untrack_file(&path);
                    }
                }
            }
        }
        KeyCode::Char('v') => {
            app.open_viewer();
        }
        _ => {}
    }
}

fn handle_restore_keys(app: &mut App, key: KeyCode) {
    match app.restore_view {
        RestoreView::Projects => {
            match key {
                KeyCode::Down | KeyCode::Char('j') => {
                    if !app.backup_projects.is_empty() {
                        let i = app.backup_project_list_state.selected().unwrap_or(0);
                        let next = (i + 1).min(app.backup_projects.len() - 1);
                        app.backup_project_list_state.select(Some(next));
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if !app.backup_projects.is_empty() {
                        let i = app.backup_project_list_state.selected().unwrap_or(0);
                        let prev = i.saturating_sub(1);
                        app.backup_project_list_state.select(Some(prev));
                    }
                }
                KeyCode::PageDown => {
                    if !app.backup_projects.is_empty() {
                        let i = app.backup_project_list_state.selected().unwrap_or(0);
                        let next = (i + PAGE_SIZE).min(app.backup_projects.len() - 1);
                        app.backup_project_list_state.select(Some(next));
                    }
                }
                KeyCode::PageUp => {
                    if !app.backup_projects.is_empty() {
                        let i = app.backup_project_list_state.selected().unwrap_or(0);
                        let prev = i.saturating_sub(PAGE_SIZE);
                        app.backup_project_list_state.select(Some(prev));
                    }
                }
                KeyCode::Home => {
                    if !app.backup_projects.is_empty() {
                        app.backup_project_list_state.select(Some(0));
                    }
                }
                KeyCode::End => {
                    if !app.backup_projects.is_empty() {
                        app.backup_project_list_state.select(Some(app.backup_projects.len() - 1));
                    }
                }
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                    // Select project and view its commits
                    app.select_backup_project();
                }
                KeyCode::Char('r') => {
                    // Refresh backup projects
                    app.scan_backup_projects();
                    app.message = Some(("Refreshed".to_string(), false));
                }
                _ => {}
            }
        }
        RestoreView::Commits => {
            match key {
                KeyCode::Down | KeyCode::Char('j') => {
                    if !app.commits.is_empty() {
                        let i = app.commit_list_state.selected().unwrap_or(0);
                        let next = (i + 1).min(app.commits.len() - 1);
                        app.commit_list_state.select(Some(next));
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if !app.commits.is_empty() {
                        let i = app.commit_list_state.selected().unwrap_or(0);
                        let prev = i.saturating_sub(1);
                        app.commit_list_state.select(Some(prev));
                    }
                }
                KeyCode::PageDown => {
                    if !app.commits.is_empty() {
                        let i = app.commit_list_state.selected().unwrap_or(0);
                        let next = (i + PAGE_SIZE).min(app.commits.len() - 1);
                        app.commit_list_state.select(Some(next));
                    }
                }
                KeyCode::PageUp => {
                    if !app.commits.is_empty() {
                        let i = app.commit_list_state.selected().unwrap_or(0);
                        let prev = i.saturating_sub(PAGE_SIZE);
                        app.commit_list_state.select(Some(prev));
                    }
                }
                KeyCode::Home => {
                    if !app.commits.is_empty() {
                        app.commit_list_state.select(Some(0));
                    }
                }
                KeyCode::End => {
                    if !app.commits.is_empty() {
                        app.commit_list_state.select(Some(app.commits.len() - 1));
                    }
                }
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                    // Select commit and view its files
                    app.select_commit();
                }
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
                    // Go back to projects
                    app.back_to_backup_projects();
                }
                KeyCode::Char('r') => {
                    // Refresh commits
                    if let Some(name) = app.selected_backup_project.clone() {
                        app.load_commits_for_project(&name);
                    }
                    app.message = Some(("Refreshed".to_string(), false));
                }
                _ => {}
            }
        }
        RestoreView::Files => {
            match key {
                KeyCode::Down | KeyCode::Char('j') => {
                    if !app.restore_files.is_empty() {
                        let i = app.restore_list_state.selected().unwrap_or(0);
                        let next = (i + 1).min(app.restore_files.len() - 1);
                        app.restore_list_state.select(Some(next));
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if !app.restore_files.is_empty() {
                        let i = app.restore_list_state.selected().unwrap_or(0);
                        let prev = i.saturating_sub(1);
                        app.restore_list_state.select(Some(prev));
                    }
                }
                KeyCode::PageDown => {
                    if !app.restore_files.is_empty() {
                        let i = app.restore_list_state.selected().unwrap_or(0);
                        let next = (i + PAGE_SIZE).min(app.restore_files.len() - 1);
                        app.restore_list_state.select(Some(next));
                    }
                }
                KeyCode::PageUp => {
                    if !app.restore_files.is_empty() {
                        let i = app.restore_list_state.selected().unwrap_or(0);
                        let prev = i.saturating_sub(PAGE_SIZE);
                        app.restore_list_state.select(Some(prev));
                    }
                }
                KeyCode::Home => {
                    if !app.restore_files.is_empty() {
                        app.restore_list_state.select(Some(0));
                    }
                }
                KeyCode::End => {
                    if !app.restore_files.is_empty() {
                        app.restore_list_state.select(Some(app.restore_files.len() - 1));
                    }
                }
                KeyCode::Enter | KeyCode::Char('R') => {
                    // Restore selected file(s)
                    app.perform_restore();
                }
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
                    // Go back to commits
                    app.back_to_commits();
                }
                KeyCode::Char(' ') => {
                    // Toggle selection
                    app.toggle_restore_select();
                    // Move to next
                    if !app.restore_files.is_empty() {
                        let i = app.restore_list_state.selected().unwrap_or(0);
                        let next = (i + 1).min(app.restore_files.len() - 1);
                        app.restore_list_state.select(Some(next));
                    }
                }
                KeyCode::Char('r') => {
                    // Refresh files
                    if let Some(idx) = app.selected_commit {
                        let hash = app.commits[idx].hash.clone();
                        app.load_commit_files(&hash);
                    }
                    app.message = Some(("Refreshed".to_string(), false));
                }
                KeyCode::Char('v') => {
                    app.open_viewer();
                }
                KeyCode::Char('a') => {
                    // Select all
                    app.select_all_restore();
                }
                KeyCode::Char('d') => {
                    // Deselect all
                    app.deselect_all_restore();
                }
                _ => {}
            }
        }
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
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(Span::styled(format!(" {} ", t), style))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Dot Matrix v{} ", env!("CARGO_PKG_VERSION"))),
        )
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

    // File viewer overlay (when not in restore confirm mode)
    if app.viewer_visible && !app.restore_confirm.visible {
        render_viewer(f, app);
    }

    // Help overlay
    if app.show_help {
        render_help(f, app);
    }

    // About overlay
    if app.show_about {
        render_about(f);
    }

    // Project creation overlay
    if app.creating_project {
        render_project_input(f, app);
    }

    // Git remote configuration overlay
    if app.setting_remote {
        render_remote_input(f, app);
    }

    // Custom commit message overlay
    if app.entering_commit_msg {
        render_commit_msg_input(f, app);
    }

    // Recursive preview overlay
    if app.recursive_preview.is_some() {
        render_recursive_preview(f, app);
    }

    // Delete confirmation overlay
    if app.confirm_delete {
        render_delete_confirm(f, app);
    }

    // Restore confirmation overlay
    if app.restore_confirm.visible {
        render_restore_confirm(f, app);
        // Render viewer ON TOP of restore confirm if viewing backup/local/diff
        if app.viewer_visible {
            render_viewer(f, app);
        }
    }

    // Password prompt overlay
    if app.password_prompt_visible {
        render_password_prompt(f, app);
    }
}

fn render_projects(f: &mut Frame, app: &mut App, area: Rect) {
    use app::ProjectViewItem;
    use dmcore::TrackMode;

    if app.visible_items.is_empty() {
        let msg = Paragraph::new("No projects. Press 'n' to create one.")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title(" Projects "));
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = app
        .visible_items
        .iter()
        .map(|item| match item {
            ProjectViewItem::Project {
                name,
                file_count,
                summary,
                expanded,
            } => {
                let expand_char = if *expanded { "▼" } else { "▶" };
                let status_icon = if summary.is_clean() { "✓" } else { "⚠" };
                let status_color = if summary.is_clean() {
                    Color::Green
                } else {
                    Color::Yellow
                };

                // Git remote status indicator
                let (git_status_str, git_status_color) =
                    if let Some(remote_status) = app.get_project_remote_status(name) {
                        if !remote_status.has_remote {
                            ("[no remote]".to_string(), Color::DarkGray)
                        } else if !remote_status.remote_reachable {
                            ("[offline]".to_string(), Color::Red)
                        } else if remote_status.ahead > 0 && remote_status.behind > 0 {
                            (
                                format!("[↑{} ↓{}]", remote_status.ahead, remote_status.behind),
                                Color::Yellow,
                            )
                        } else if remote_status.ahead > 0 {
                            (format!("[↑{}]", remote_status.ahead), Color::Cyan)
                        } else if remote_status.behind > 0 {
                            (format!("[↓{}]", remote_status.behind), Color::Magenta)
                        } else {
                            ("[synced]".to_string(), Color::Green)
                        }
                    } else {
                        (String::new(), Color::DarkGray)
                    };

                let mut spans = vec![
                    Span::raw(format!("{} ", expand_char)),
                    Span::styled(status_icon, Style::default().fg(status_color)),
                    Span::raw(" "),
                    Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(
                        format!(" ({} files)", file_count),
                        Style::default().fg(Color::DarkGray),
                    ),
                ];

                if !git_status_str.is_empty() {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        git_status_str,
                        Style::default().fg(git_status_color),
                    ));
                }

                ListItem::new(Line::from(spans))
            }
            ProjectViewItem::File {
                path,
                status,
                size,
                track_mode,
                encrypted,
                ..
            } => {
                let (icon, color) = match status {
                    FileStatus::Synced => ("✓", Color::Green),
                    FileStatus::Drifted => ("⚠", Color::Yellow),
                    FileStatus::New => ("+", Color::Cyan),
                    FileStatus::Missing => ("✗", Color::Red),
                    FileStatus::Error => ("!", Color::Red),
                };

                let size_str = size
                    .map(|s| format_size(s))
                    .unwrap_or_else(|| "-".to_string());

                // Track mode indicator: [G]=Git, [B]=Backup, [+]=Both
                let track_str = match track_mode {
                    TrackMode::Git => "[G]",
                    TrackMode::Backup => "[B]",
                    TrackMode::Both => "[+]",
                };
                let track_color = match track_mode {
                    TrackMode::Git => Color::Cyan,
                    TrackMode::Backup => Color::Magenta,
                    TrackMode::Both => Color::Green,
                };

                // Encryption indicator
                let enc_str = if *encrypted { "[E]" } else { "   " };

                ListItem::new(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(icon, Style::default().fg(color)),
                    Span::raw(" "),
                    Span::styled(track_str, Style::default().fg(track_color)),
                    Span::styled(
                        enc_str,
                        Style::default().fg(if *encrypted {
                            Color::Yellow
                        } else {
                            Color::DarkGray
                        }),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        format!("{:>8}", size_str),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw("  "),
                    Span::raw(path),
                ]))
            }
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Projects "))
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_stateful_widget(list, area, &mut app.project_list_state);
}

fn render_add(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    // Track mode badge and color (same as Projects view)
    let (mode_badge, mode_color) = match app.default_track_mode {
        dmcore::TrackMode::Git => ("[G]", Color::Cyan),
        dmcore::TrackMode::Backup => ("[B]", Color::Magenta),
        dmcore::TrackMode::Both => ("[+]", Color::Green),
    };

    // File list
    let items: Vec<ListItem> = app
        .browse_files
        .iter()
        .map(|file| {
            let size_str = file
                .size
                .map(|s| format!(" {:>8}", format_size(s)))
                .unwrap_or_default();

            if file.is_dir {
                // Directory - show with project info if it contains tracked files
                if file.is_tracked() {
                    let projects_str = if file.tracked_in.len() == 1 {
                        format!(" [{}]", file.tracked_in[0])
                    } else if file.tracked_in.len() <= 3 {
                        format!(" [{}]", file.tracked_in.join(", "))
                    } else {
                        format!(" [{}, +{}]", file.tracked_in[0], file.tracked_in.len() - 1)
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(" ✓ ", Style::default().fg(Color::Green)),
                        Span::styled(format!("{}/", &file.name), Style::default().fg(Color::Blue)),
                        Span::styled(projects_str, Style::default().fg(Color::Cyan)),
                    ]))
                } else {
                    ListItem::new(Line::from(vec![
                        Span::styled(" / ", Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("{}/", &file.name), Style::default().fg(Color::Blue)),
                    ]))
                }
            } else if file.is_tracked() {
                // File already tracked - show with checkmark and project name(s)
                let projects_str = if file.tracked_in.len() == 1 {
                    format!(" [{}]", file.tracked_in[0])
                } else {
                    format!(" [{}]", file.tracked_in.join(", "))
                };

                ListItem::new(Line::from(vec![
                    Span::styled(" ✓ ", Style::default().fg(Color::Green)),
                    Span::styled(&file.name, Style::default().fg(Color::Yellow)),
                    Span::styled(size_str, Style::default().fg(Color::DarkGray)),
                    Span::styled(projects_str, Style::default().fg(Color::Cyan)),
                ]))
            } else {
                // Regular file - show track mode badge
                ListItem::new(Line::from(vec![
                    Span::styled(mode_badge, Style::default().fg(mode_color)),
                    Span::styled(&file.name, Style::default().fg(Color::White)),
                    Span::styled(size_str, Style::default().fg(Color::DarkGray)),
                ]))
            }
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

    f.render_stateful_widget(list, chunks[0], &mut app.browse_list_state);

    // Current directory path at bottom
    let path_display = if let Some(home) = dirs::home_dir() {
        if let Ok(rel) = app.browse_dir.strip_prefix(&home) {
            format!(" ~/{}", rel.display())
        } else {
            format!(" {}", app.browse_dir.display())
        }
    } else {
        format!(" {}", app.browse_dir.display())
    };
    let dir_line = Paragraph::new(path_display).style(Style::default().fg(Color::Cyan));
    f.render_widget(dir_line, chunks[1]);
}

fn render_restore(f: &mut Frame, app: &mut App, area: Rect) {
    match app.restore_view {
        RestoreView::Projects => render_restore_projects(f, app, area),
        RestoreView::Commits => render_restore_commits(f, app, area),
        RestoreView::Files => render_restore_files(f, app, area),
    }
}

fn render_restore_projects(f: &mut Frame, app: &mut App, area: Rect) {
    if app.backup_projects.is_empty() {
        let msg = Paragraph::new("No backups found.\n\nCreate backups in the Projects tab with 'b'.")
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Available Backups "),
            );
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = app
        .backup_projects
        .iter()
        .map(|project| {
            let commits_str = format!("{} backups", project.commit_count);
            let last_backup_str = project
                .last_backup
                .as_ref()
                .map(|d| format!("  Last: {}", d))
                .unwrap_or_default();

            let line = Line::from(vec![
                Span::styled(
                    format!("{:<20}", project.name),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:>12}", commits_str),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(last_backup_str, Style::default().fg(Color::DarkGray)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Available Backups (Enter to select) "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut app.backup_project_list_state);
}

fn render_restore_commits(f: &mut Frame, app: &mut App, area: Rect) {
    // Get the selected backup project name for the title
    let project_name = app.selected_backup_project.clone().unwrap_or_default();

    if app.commits.is_empty() {
        let title = if project_name.is_empty() {
            " Backup History ".to_string()
        } else {
            format!(" {} - Backup History ", project_name)
        };
        let msg = Paragraph::new("No commits found in this backup.")
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title),
            );
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = app
        .commits
        .iter()
        .map(|commit| {
            // Parse date to show only date and time
            let date_short = if commit.date.len() > 19 {
                &commit.date[..19]
            } else {
                &commit.date
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", commit.short_hash),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(format!("{} ", date_short), Style::default().fg(Color::Cyan)),
                Span::raw(&commit.message),
            ]);

            ListItem::new(line)
        })
        .collect();

    let title = format!(
        " {} - {} backups (Enter=select, Backspace=back) ",
        project_name,
        app.commits.len()
    );

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut app.commit_list_state);
}

fn render_restore_files(f: &mut Frame, app: &mut App, area: Rect) {
    // Get the selected backup project name for the title
    let project_name = app.selected_backup_project.clone().unwrap_or_default();

    if app.restore_files.is_empty() {
        let title = if project_name.is_empty() {
            " Files ".to_string()
        } else {
            format!(" {} - Files ", project_name)
        };
        let msg = Paragraph::new("No files found in this backup.")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title(title));
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = app
        .restore_files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let selected_marker = if app.restore_selected.contains(&i) {
                "*"
            } else {
                " "
            };

            // Status indicator
            let (status, color) = if !file.exists_locally {
                ("NEW", Color::Cyan) // File doesn't exist locally
            } else if file.local_differs {
                ("CHG", Color::Yellow) // Local file is different
            } else {
                ("OK ", Color::Green) // File matches backup
            };

            let size_str = format_size(file.size);

            let line = Line::from(vec![
                Span::raw(format!("{} ", selected_marker)),
                Span::styled(format!("{} ", status), Style::default().fg(color)),
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

    let commit_info = app
        .selected_commit
        .and_then(|i| app.commits.get(i))
        .map(|c| format!("{} - {}", c.short_hash, c.message))
        .unwrap_or_else(|| "Unknown".to_string());

    let title = if project_name.is_empty() {
        format!(" {} (Enter=restore, Backspace=back) ", commit_info)
    } else {
        format!(" {} / {} (Enter=restore, Backspace=back) ", project_name, commit_info)
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut app.restore_list_state);
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
            Mode::Projects => "↑↓:nav  Enter:expand  b:backup  S:save  g:git  p:push  P:pull  ?:help",
            Mode::Add => "↑↓:select  Enter:open/add  h:parent  ~:home  ?:help  q:quit",
            Mode::Restore => match app.restore_view {
                RestoreView::Projects => "↑↓:select  Enter:view backups  r:refresh  ?:help  q:quit",
                RestoreView::Commits => "↑↓:select  Enter:view files  h:back  r:refresh  ?:help",
                RestoreView::Files => {
                    "↑↓:nav  Space:select  a:all  d:none  Enter:restore  v:view  h:back  ?:help"
                }
            },
        };
        (help.to_string(), Style::default().fg(Color::Cyan))
    };

    let status = Paragraph::new(msg)
        .style(style)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(status, area);
}

fn render_viewer(f: &mut Frame, app: &App) {
    use ratatui::widgets::{Clear, Wrap};

    let area = centered_rect(85, 85, f.area());

    // Clear the area completely (removes any content behind)
    f.render_widget(Clear, area);

    // Split area for content and footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let content_area = chunks[0];
    let footer_area = chunks[1];

    // Get visible lines based on scroll
    let visible_height = content_area.height.saturating_sub(2) as usize; // Account for borders
    let start = app.viewer_scroll;
    let end = (start + visible_height).min(app.viewer_content.len());
    let visible_lines = &app.viewer_content[start..end];

    // Calculate line number width (digits + separator)
    let total_lines = app.viewer_content.len();
    let line_num_digits = if total_lines > 0 {
        total_lines.to_string().len()
    } else {
        1
    };
    let gutter_width = line_num_digits + 3; // digits + " │ "

    // Build title with scroll info
    let scroll_info = format!(
        " {} [{}/{}] ",
        app.viewer_title,
        app.viewer_scroll + 1,
        app.viewer_content.len()
    );

    if app.viewer_line_numbers {
        // Split content area into line numbers gutter and content
        let inner_area = Block::default()
            .borders(Borders::ALL)
            .title(scroll_info.clone())
            .border_style(Style::default().fg(Color::Cyan))
            .inner(content_area);

        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(gutter_width as u16),
                Constraint::Min(1),
            ])
            .split(inner_area);

        let gutter_area = h_chunks[0];
        let text_area = h_chunks[1];

        // Render the border/background first
        let border_block = Block::default()
            .borders(Borders::ALL)
            .title(scroll_info)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));
        f.render_widget(border_block, content_area);

        // Build line numbers
        let line_nums: Vec<Line> = visible_lines
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                let actual_line = start + idx + 1;
                Line::from(Span::styled(
                    format!("{:>width$} │", actual_line, width = line_num_digits),
                    Style::default().fg(Color::DarkGray),
                ))
            })
            .collect();

        let gutter = Paragraph::new(line_nums)
            .style(Style::default().bg(Color::Black));
        f.render_widget(gutter, gutter_area);

        // Build content lines (without line numbers)
        let content_lines: Vec<Line> = visible_lines
            .iter()
            .map(|vl| {
                let spans: Vec<Span> = vl
                    .spans
                    .iter()
                    .map(|(text, style)| Span::styled(text.clone(), *style))
                    .collect();
                Line::from(spans)
            })
            .collect();

        let content = Paragraph::new(content_lines)
            .wrap(Wrap { trim: false })
            .style(Style::default().bg(Color::Black));
        f.render_widget(content, text_area);
    } else {
        // No line numbers - simple render with wrap
        let lines: Vec<Line> = visible_lines
            .iter()
            .map(|vl| {
                let spans: Vec<Span> = vl
                    .spans
                    .iter()
                    .map(|(text, style)| Span::styled(text.clone(), *style))
                    .collect();
                Line::from(spans)
            })
            .collect();

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(scroll_info)
                    .border_style(Style::default().fg(Color::Cyan))
                    .style(Style::default().bg(Color::Black)),
            )
            .wrap(Wrap { trim: false })
            .style(Style::default().bg(Color::Black));

        f.render_widget(paragraph, content_area);
    }

    // Render footer with hints
    let line_num_status = if app.viewer_line_numbers { "ON" } else { "OFF" };
    let footer = Line::from(vec![
        Span::styled(" ↑↓", Style::default().fg(Color::Cyan)),
        Span::raw(":scroll  "),
        Span::styled("g/G", Style::default().fg(Color::Cyan)),
        Span::raw(":top/bottom  "),
        Span::styled("n", Style::default().fg(Color::Cyan)),
        Span::styled(format!(":line# [{}]  ", line_num_status), Style::default().fg(Color::Gray)),
        Span::styled("q/Esc", Style::default().fg(Color::Cyan)),
        Span::raw(":close"),
    ]);
    let footer_para = Paragraph::new(footer)
        .style(Style::default().bg(Color::Black));
    f.render_widget(footer_para, footer_area);
}

fn render_help(f: &mut Frame, _app: &App) {
    let area = centered_rect(60, 70, f.area());

    let help_text = r#"
 NAVIGATION (All Tabs)
 ───────────────────────────
 ↑/k ↓/j    Move up/down
 PgUp/PgDn  Page up/down
 Home/End   Jump to start/end

 GLOBAL KEYS
 ───────────────────────────
 Tab/1-3    Switch tabs
 ?          Show/hide help
 A          About
 v          View file content
 q          Quit

 FILE VIEWER
 ───────────────────────────
 ↑/k ↓/j    Scroll up/down
 PgUp/PgDn  Page up/down
 g/Home     Go to top
 G/End      Go to bottom
 n          Toggle line numbers
 v/q/Esc    Close viewer

 PROJECTS TAB
 ───────────────────────────
 Enter/→/l  Expand/collapse
 ←/h        Collapse project
 m          Toggle track mode
 x          Toggle encryption
 X          Encrypt project
 b          Backup (incremental)
 B          Backup w/ message
 a          Archive backup
 s          Sync project
 S          Save now (live)
 n          New project
 D          Delete project
 r          Refresh
 g          Refresh git status
 G          Set git remote
 p          Push to remote
 P          Pull from remote

 ADD FILES TAB
 ───────────────────────────
 Enter/→/l  Open dir / Add file
 ←/h/Bksp   Parent directory
 a          Add selected file
 u          Untrack file
 R          Recursive add
 p          Cycle target project
 t          Cycle track mode
 n          New project
 ~          Go to home

 RECURSIVE ADD POPUP
 ───────────────────────────
 Space      Toggle selection
 a          Toggle all
 t          Cycle track mode
 T          Set all track mode
 Enter      Add selected files
 Esc/q      Cancel

 RESTORE - PROJECTS
 ───────────────────────────
 Enter/→/l  View project backups
 r          Refresh

 RESTORE - COMMITS
 ───────────────────────────
 Enter/→/l  View files in backup
 ←/h/Bksp   Back to projects
 r          Refresh

 RESTORE - FILES
 ───────────────────────────
 Space      Toggle selection
 a          Select all
 d          Deselect all
 Enter/R    Restore (confirm)
 v          View file content
 ←/h/Bksp   Back to commits
 r          Refresh

 RESTORE CONFIRMATION
 ───────────────────────────
 ↑/k ↓/j    Navigate files
 b          View backup file
 l          View local file
 d          View diff
 Y/Enter    Confirm restore
 N/Esc      Cancel
 O          Original location
 C          Custom location
 Tab        Toggle destination

 TRACK MODES
 ───────────────────────────
 [G] Git     Version control
 [B] Backup  Incremental
 [+] Both    Git + Backup
 [E] Encrypted file

 GIT STATUS
 ───────────────────────────
 [synced]   Up to date
 [↑N]       Ahead of remote
 [↓N]       Behind remote
 [no remote] No remote set

 RESTORE SYMBOLS
 ───────────────────────────
 NEW   File missing locally
 CHG   Local file differs
 OK    Local matches backup
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

fn render_about(f: &mut Frame) {
    let area = centered_rect(50, 40, f.area());

    let about_text = format!(
        r#"

         ██████╗  ██████╗ ████████╗
         ██╔══██╗██╔═══██╗╚══██╔══╝
         ██║  ██║██║   ██║   ██║
         ██║  ██║██║   ██║   ██║
         ██████╔╝╚██████╔╝   ██║
         ╚═════╝  ╚═════╝    ╚═╝

   ███╗   ███╗ █████╗ ████████╗██████╗ ██╗██╗  ██╗
   ████╗ ████║██╔══██╗╚══██╔══╝██╔══██╗██║╚██╗██╔╝
   ██╔████╔██║███████║   ██║   ██████╔╝██║ ╚███╔╝
   ██║╚██╔╝██║██╔══██║   ██║   ██╔══██╗██║ ██╔██╗
   ██║ ╚═╝ ██║██║  ██║   ██║   ██║  ██║██║██╔╝ ██╗
   ╚═╝     ╚═╝╚═╝  ╚═╝   ╚═╝   ╚═╝  ╚═╝╚═╝╚═╝  ╚═╝

                    v{}

    Project compositor with git versioning

    Author: Woofson
    License: MIT
    GitHub: https://github.com/Woofson/dotmatrix

             Press any key to close
"#,
        env!("CARGO_PKG_VERSION")
    );

    let about = Paragraph::new(about_text)
        .style(Style::default().fg(Color::Cyan))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" About ")
                .style(Style::default().bg(Color::Black)),
        );

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(about, area);
}

fn render_project_input(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 20, f.area());

    let input_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Enter project name:",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  > "),
            Span::styled(&app.project_input, Style::default().fg(Color::Yellow)),
            Span::styled(
                "_",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Enter", Style::default().fg(Color::Green)),
            Span::raw(": Create  "),
            Span::styled("Esc", Style::default().fg(Color::Red)),
            Span::raw(": Cancel"),
        ]),
    ];

    let input = Paragraph::new(input_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" New Project ")
            .style(Style::default().bg(Color::Black)),
    );

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(input, area);
}

fn render_remote_input(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 25, f.area());

    let project_name = app.selected_project_name().unwrap_or_default();

    let input_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  Git remote URL for '{}':", project_name),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  > "),
            Span::styled(&app.remote_input, Style::default().fg(Color::Yellow)),
            Span::styled(
                "_",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Example: git@github.com:user/repo.git",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Enter", Style::default().fg(Color::Green)),
            Span::raw(": Set  "),
            Span::styled("Esc", Style::default().fg(Color::Red)),
            Span::raw(": Cancel"),
        ]),
    ];

    let input = Paragraph::new(input_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Set Git Remote ")
            .style(Style::default().bg(Color::Black)),
    );

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(input, area);
}

fn render_commit_msg_input(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 30, f.area());

    let project_name = app.selected_project_name().unwrap_or_default();

    let input_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  Commit message for '{}':", project_name),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  > "),
            Span::styled(&app.commit_msg_input, Style::default().fg(Color::Yellow)),
            Span::styled(
                "_",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Date/time will be added automatically.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "  Leave empty for default message.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Enter", Style::default().fg(Color::Green)),
            Span::raw(": Backup  "),
            Span::styled("Esc", Style::default().fg(Color::Red)),
            Span::raw(": Cancel"),
        ]),
    ];

    let input = Paragraph::new(input_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Backup with Custom Message ")
            .style(Style::default().bg(Color::Black)),
    );

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(input, area);
}

fn render_recursive_preview(f: &mut Frame, app: &mut App) {
    let area = centered_rect(80, 80, f.area());

    // Split into header and list
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    // Build items from preview data (collect what we need first)
    let (source_display, selected_count, total_count, items) = {
        let preview = match &app.recursive_preview {
            Some(p) => p,
            None => return,
        };

        let source_display = if let Some(home) = dirs::home_dir() {
            if let Ok(rel) = preview.source_dir.strip_prefix(&home) {
                format!("~/{}", rel.display())
            } else {
                preview.source_dir.display().to_string()
            }
        } else {
            preview.source_dir.display().to_string()
        };

        let items: Vec<ListItem> = preview
            .preview_files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                let selected = if preview.selected_files.contains(&i) {
                    "[x]"
                } else {
                    "[ ]"
                };
                let (mode_badge, mode_color) = match file.track_mode {
                    dmcore::TrackMode::Git => ("[G]", Color::Cyan),
                    dmcore::TrackMode::Backup => ("[B]", Color::Magenta),
                    dmcore::TrackMode::Both => ("[+]", Color::Green),
                };
                let size_str = format_size(file.size);

                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{} ", selected),
                        Style::default().fg(if preview.selected_files.contains(&i) {
                            Color::Green
                        } else {
                            Color::DarkGray
                        }),
                    ),
                    Span::styled(format!("{} ", mode_badge), Style::default().fg(mode_color)),
                    Span::styled(
                        format!("{:>8}  ", size_str),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(file.display_path.clone()),
                ]))
            })
            .collect();

        (
            source_display,
            preview.selected_files.len(),
            preview.preview_files.len(),
            items,
        )
    };

    // Header
    let header_text = vec![
        Line::from(vec![
            Span::raw(" Adding from: "),
            Span::styled(source_display, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw(" Selected: "),
            Span::styled(
                format!("{}/{}", selected_count, total_count),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" files"),
        ]),
    ];

    let header = Paragraph::new(header_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Recursive Add "),
        )
        .style(Style::default().bg(Color::Black));

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(header, chunks[0]);

    // File list
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::DarkGray))
        .style(Style::default().bg(Color::Black));

    f.render_stateful_widget(
        list,
        chunks[1],
        &mut app
            .recursive_preview
            .as_mut()
            .unwrap()
            .preview_list_state,
    );

    // Footer with hints
    let footer = Paragraph::new(" Space: toggle  a: all  t: mode  T: all mode  Enter: add  Esc: cancel")
        .style(Style::default().fg(Color::Cyan).bg(Color::Black));
    f.render_widget(footer, chunks[2]);
}

fn render_delete_confirm(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 20, f.area());

    let project_name = app.delete_target.as_deref().unwrap_or("unknown");

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Are you sure you want to delete this project?",
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Project: "),
            Span::styled(project_name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  This will remove the project from the manifest.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "  Backup data will NOT be deleted.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Y/Enter", Style::default().fg(Color::Red)),
            Span::raw(": Delete  "),
            Span::styled("N/Esc", Style::default().fg(Color::Green)),
            Span::raw(": Cancel"),
        ]),
    ];

    let paragraph = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Confirm Delete ")
            .border_style(Style::default().fg(Color::Red))
            .style(Style::default().bg(Color::Black)),
    );

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(paragraph, area);
}

fn render_restore_confirm(f: &mut Frame, app: &App) {
    let area = centered_rect(75, 70, f.area());

    let file_count = app.restore_confirm.files_to_restore.len();
    let will_overwrite = app.restore_confirm.will_overwrite;

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Restore Confirmation",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    // File count summary
    lines.push(Line::from(vec![
        Span::raw("  Files to restore: "),
        Span::styled(
            format!("{}", file_count),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        if will_overwrite > 0 {
            Span::styled(
                format!("⚠ {} will overwrite", will_overwrite),
                Style::default().fg(Color::Yellow),
            )
        } else {
            Span::styled(
                "✓ No overwrites",
                Style::default().fg(Color::Green),
            )
        },
    ]));

    lines.push(Line::from(""));

    // File list header
    lines.push(Line::from(vec![
        Span::styled("  FILES ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled("(↑↓ select)", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("b", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("=view backup  ", Style::default().fg(Color::Gray)),
        Span::styled("l", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("=view local  ", Style::default().fg(Color::Gray)),
        Span::styled("d", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled("=view diff", Style::default().fg(Color::Gray)),
    ]));
    lines.push(Line::from(Span::styled(
        "  ─────────────────────────────────────────────────────────────",
        Style::default().fg(Color::DarkGray),
    )));

    // Show files (up to ~15 visible)
    let max_visible = 12;
    let selected = app.restore_confirm.selected_idx;
    let scroll = if selected >= max_visible {
        selected - max_visible + 1
    } else {
        0
    };

    for (display_idx, &file_idx) in app.restore_confirm.files_to_restore.iter().enumerate().skip(scroll).take(max_visible) {
        if let Some(file) = app.restore_files.get(file_idx) {
            let is_selected = display_idx == selected;
            let marker = if is_selected { "▶" } else { " " };

            // Status indicator
            let status = if !file.exists_locally {
                Span::styled("NEW ", Style::default().fg(Color::Cyan))
            } else if file.local_differs {
                Span::styled("CHG ", Style::default().fg(Color::Yellow))
            } else {
                Span::styled("OK  ", Style::default().fg(Color::Green))
            };

            // Path (truncate if too long)
            let max_path_len = 50;
            let path_display = if file.display_path.len() > max_path_len {
                format!("...{}", &file.display_path[file.display_path.len() - max_path_len + 3..])
            } else {
                file.display_path.clone()
            };

            let path_style = if is_selected {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else if file.exists_locally && file.local_differs {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Gray)
            };

            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", marker), Style::default().fg(Color::Cyan)),
                status,
                Span::styled(path_display, path_style),
            ]));
        }
    }

    // Show scroll indicator if needed
    if file_count > max_visible {
        lines.push(Line::from(Span::styled(
            format!("  ... ({}/{} shown, scroll with ↑↓)", max_visible.min(file_count), file_count),
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  ─────────────────────────────────────────────────────────────",
        Style::default().fg(Color::DarkGray),
    )));

    // Destination options
    lines.push(Line::from(Span::styled(
        "  DESTINATION:",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    )));

    // Original location option
    let orig_style = if app.restore_confirm.destination == RestoreDestination::Original {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let orig_marker = if app.restore_confirm.destination == RestoreDestination::Original {
        "●"
    } else {
        "○"
    };
    lines.push(Line::from(vec![
        Span::styled(format!("  {} ", orig_marker), orig_style),
        Span::styled("[O]", Style::default().fg(Color::Cyan)),
        Span::styled(" Original location", orig_style),
    ]));

    // Custom location option
    let custom_style = if app.restore_confirm.destination == RestoreDestination::Custom {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let custom_marker = if app.restore_confirm.destination == RestoreDestination::Custom {
        "●"
    } else {
        "○"
    };
    lines.push(Line::from(vec![
        Span::styled(format!("  {} ", custom_marker), custom_style),
        Span::styled("[C]", Style::default().fg(Color::Cyan)),
        Span::styled(" Custom location", custom_style),
    ]));

    // Show path input if custom is selected
    if app.restore_confirm.destination == RestoreDestination::Custom {
        let cursor = if app.restore_confirm.entering_path { "█" } else { "" };
        let path_display = if app.restore_confirm.custom_path.is_empty() {
            "~/...".to_string()
        } else {
            app.restore_confirm.custom_path.clone()
        };
        let input_style = if app.restore_confirm.entering_path {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::from(vec![
            Span::raw("      Path: "),
            Span::styled(format!("{}{}", path_display, cursor), input_style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  ─────────────────────────────────────────────────────────────",
        Style::default().fg(Color::DarkGray),
    )));

    // Actions
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("Y/Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(": Restore  "),
        Span::styled("N/Esc", Style::default().fg(Color::Red)),
        Span::raw(": Cancel  "),
        Span::styled("Tab", Style::default().fg(Color::Cyan)),
        Span::raw(": Toggle dest"),
    ]));

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Restore Files ")
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black)),
    );

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(paragraph, area);
}

fn render_password_prompt(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 25, f.area());

    let title = match app.password_purpose {
        PasswordPurpose::Backup => " Encryption Password ",
        PasswordPurpose::Restore => " Decryption Password ",
    };

    let description = match app.password_purpose {
        PasswordPurpose::Backup => "  Enter password to encrypt files:",
        PasswordPurpose::Restore => "  Enter password to decrypt files:",
    };

    // Mask the password with asterisks
    let masked: String = "*".repeat(app.password_input.len());

    let input_text = vec![
        Line::from(""),
        Line::from(Span::styled(description, Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(vec![
            Span::raw("  > "),
            Span::styled(masked, Style::default().fg(Color::Yellow)),
            Span::styled(
                "_",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Enter", Style::default().fg(Color::Green)),
            Span::raw(": Confirm  "),
            Span::styled("Esc", Style::default().fg(Color::Red)),
            Span::raw(": Cancel"),
        ]),
    ];

    let input = Paragraph::new(input_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().bg(Color::Black)),
    );

    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(input, area);
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
