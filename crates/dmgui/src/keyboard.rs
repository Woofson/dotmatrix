//! Keyboard input handling for the GUI
//!
//! Processes keyboard shortcuts and navigation.

use crate::app::GuiApp;
use crate::state::{Mode, RestoreView};
use egui::{Context, Key};

/// Handle keyboard input
pub fn handle_keyboard(app: &mut GuiApp, ctx: &Context) {
    // Don't process shortcuts while typing in text input
    if app.text_input_focus {
        return;
    }

    // Don't process while dialogs are open
    if app.creating_project
        || app.confirm_delete
        || app.setting_remote
        || app.entering_commit_msg
        || app.password_prompt_visible
        || app.restore_confirm.visible
        || app.viewer_visible
        || app.show_help
        || app.show_about
    {
        return;
    }

    // Handle recursive preview mode separately
    if app.recursive_preview.is_some() {
        handle_recursive_preview_keyboard(app, ctx);
        return;
    }

    ctx.input(|i| {
        // Clear message on key press
        if !i.keys_down.is_empty() && app.message.is_some() && !app.busy {
            app.message = None;
        }

        // Quit
        if i.key_pressed(Key::Q) && !i.modifiers.shift {
            app.should_quit = true;
        }

        // Tab switching
        if i.key_pressed(Key::Tab) {
            if i.modifiers.shift {
                app.mode = app.mode.prev();
            } else {
                app.mode = app.mode.next();
            }
            app.message = None;
        }
        if i.key_pressed(Key::Num1) {
            app.mode = Mode::Projects;
            app.message = None;
        }
        if i.key_pressed(Key::Num2) {
            app.mode = Mode::Add;
            app.message = None;
        }
        if i.key_pressed(Key::Num3) {
            app.mode = Mode::Restore;
            app.message = None;
        }

        // Help
        if i.key_pressed(Key::F1) || (i.key_pressed(Key::Slash) && i.modifiers.shift) {
            app.show_help = !app.show_help;
        }

        // About
        if i.key_pressed(Key::Num1) && i.modifiers.shift {
            app.show_about = !app.show_about;
        }

        // Mode-specific handling
        match app.mode {
            Mode::Projects => handle_projects_keyboard(app, i),
            Mode::Add => handle_add_keyboard(app, i),
            Mode::Restore => handle_restore_keyboard(app, i),
        }
    });
}

fn handle_projects_keyboard(app: &mut GuiApp, i: &egui::InputState) {
    let list_len = app.visible_items.len();

    // Navigation
    if i.key_pressed(Key::J) || i.key_pressed(Key::ArrowDown) {
        if let Some(sel) = app.project_selected {
            app.project_selected = Some((sel + 1).min(list_len.saturating_sub(1)));
        } else if list_len > 0 {
            app.project_selected = Some(0);
        }
    }
    if i.key_pressed(Key::K) || i.key_pressed(Key::ArrowUp) {
        if let Some(sel) = app.project_selected {
            app.project_selected = Some(sel.saturating_sub(1));
        } else if list_len > 0 {
            app.project_selected = Some(0);
        }
    }
    if i.key_pressed(Key::PageDown) {
        if let Some(sel) = app.project_selected {
            app.project_selected = Some((sel + 10).min(list_len.saturating_sub(1)));
        }
    }
    if i.key_pressed(Key::PageUp) {
        if let Some(sel) = app.project_selected {
            app.project_selected = Some(sel.saturating_sub(10));
        }
    }
    if i.key_pressed(Key::Home) || (i.key_pressed(Key::G) && !i.modifiers.shift) {
        if list_len > 0 {
            app.project_selected = Some(0);
        }
    }
    if i.key_pressed(Key::End) || (i.key_pressed(Key::G) && i.modifiers.shift) {
        if list_len > 0 {
            app.project_selected = Some(list_len - 1);
        }
    }

    // Expand/collapse
    if i.key_pressed(Key::Enter) || i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::L) {
        app.toggle_selected_project();
    }
    if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::H) {
        app.collapse_selected_project();
    }

    // Actions
    if i.key_pressed(Key::A) && !i.modifiers.shift {
        app.backup_project_with_message();
    }
    if i.key_pressed(Key::A) && i.modifiers.shift {
        app.backup_project();
    }
    if i.key_pressed(Key::S) && !i.modifiers.shift {
        app.sync_project();
    }
    if i.key_pressed(Key::S) && i.modifiers.shift {
        app.save_state();
        app.message = Some(("Saved".to_string(), false));
    }
    if i.key_pressed(Key::N) {
        app.creating_project = true;
        app.project_input.clear();
    }
    if i.key_pressed(Key::D) && i.modifiers.shift {
        if let Some(name) = app.selected_project_name() {
            app.delete_target = Some(name);
            app.confirm_delete = true;
        }
    }
    if i.key_pressed(Key::M) {
        app.toggle_track_mode();
    }
    if i.key_pressed(Key::X) && !i.modifiers.shift {
        app.toggle_encryption();
    }
    if i.key_pressed(Key::X) && i.modifiers.shift {
        app.toggle_project_encryption();
    }
    if i.key_pressed(Key::G) && i.modifiers.shift {
        app.setting_remote = true;
        app.remote_input.clear();
    }
    if i.key_pressed(Key::G) && !i.modifiers.shift && !i.modifiers.ctrl {
        // 'g' without shift for git refresh (but not 'gg' for go to top)
        // This is handled by Home key above, so this is safe
    }
    if i.key_pressed(Key::P) && !i.modifiers.shift {
        app.push_project();
    }
    if i.key_pressed(Key::P) && i.modifiers.shift {
        app.pull_project();
    }
    if i.key_pressed(Key::R) {
        app.refresh_projects();
        app.refresh_remote_status();
        app.message = Some(("Refreshed".to_string(), false));
    }
    if i.key_pressed(Key::V) {
        if let Some(crate::state::ProjectViewItem::File { abs_path, path, .. }) =
            app.selected_item()
        {
            let title = path.clone();
            let abs = abs_path.clone();
            app.load_file_into_viewer(&abs, &title);
        }
    }
}

fn handle_add_keyboard(app: &mut GuiApp, i: &egui::InputState) {
    let list_len = app.browse_files.len();

    // Navigation
    if i.key_pressed(Key::J) || i.key_pressed(Key::ArrowDown) {
        if let Some(sel) = app.browse_selected {
            app.browse_selected = Some((sel + 1).min(list_len.saturating_sub(1)));
        } else if list_len > 0 {
            app.browse_selected = Some(0);
        }
    }
    if i.key_pressed(Key::K) || i.key_pressed(Key::ArrowUp) {
        if let Some(sel) = app.browse_selected {
            app.browse_selected = Some(sel.saturating_sub(1));
        } else if list_len > 0 {
            app.browse_selected = Some(0);
        }
    }
    if i.key_pressed(Key::PageDown) {
        if let Some(sel) = app.browse_selected {
            app.browse_selected = Some((sel + 10).min(list_len.saturating_sub(1)));
        }
    }
    if i.key_pressed(Key::PageUp) {
        if let Some(sel) = app.browse_selected {
            app.browse_selected = Some(sel.saturating_sub(10));
        }
    }
    if i.key_pressed(Key::Home) {
        if list_len > 0 {
            app.browse_selected = Some(0);
        }
    }
    if i.key_pressed(Key::End) {
        if list_len > 0 {
            app.browse_selected = Some(list_len - 1);
        }
    }

    // Enter directory or add file
    if i.key_pressed(Key::Enter) || i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::L) {
        if let Some(idx) = app.browse_selected {
            if let Some(file) = app.browse_files.get(idx) {
                if file.is_dir {
                    let path = file.path.clone();
                    app.enter_directory(&path);
                } else if !file.is_tracked() {
                    let path = file.path.clone();
                    app.add_file_to_project(&path);
                }
            }
        }
    }

    // Parent directory
    if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::H) || i.key_pressed(Key::Backspace) {
        let parent = app.browse_dir.join("..");
        app.enter_directory(&parent);
    }

    // Home
    if i.key_pressed(Key::Backtick) {
        app.go_home();
    }

    // Add file
    if i.key_pressed(Key::A) && !i.modifiers.shift && !i.modifiers.ctrl {
        if let Some(idx) = app.browse_selected {
            if let Some(file) = app.browse_files.get(idx) {
                if !file.is_dir && !file.is_tracked() {
                    let path = file.path.clone();
                    app.add_file_to_project(&path);
                }
            }
        }
    }

    // Untrack
    if i.key_pressed(Key::U) {
        if let Some(idx) = app.browse_selected {
            if let Some(file) = app.browse_files.get(idx) {
                if file.is_tracked() {
                    let path = file.path.clone();
                    app.untrack_file(&path);
                }
            }
        }
    }

    // Cycle track mode
    if i.key_pressed(Key::T) {
        app.cycle_default_track_mode();
    }

    // Cycle target project
    if i.key_pressed(Key::P) {
        app.cycle_target_project();
    }

    // New project
    if i.key_pressed(Key::N) {
        app.creating_project = true;
        app.project_input.clear();
    }

    // Recursive preview
    if i.key_pressed(Key::R) && i.modifiers.shift {
        app.start_recursive_preview();
    }

    // View file
    if i.key_pressed(Key::V) {
        if let Some(idx) = app.browse_selected {
            if let Some(file) = app.browse_files.get(idx) {
                if !file.is_dir {
                    let path = file.path.clone();
                    let name = file.name.clone();
                    app.load_file_into_viewer(&path, &name);
                }
            }
        }
    }
}

fn handle_restore_keyboard(app: &mut GuiApp, i: &egui::InputState) {
    match app.restore_view {
        RestoreView::Projects => {
            let list_len = app.backup_projects.len();

            // Navigation
            if i.key_pressed(Key::J) || i.key_pressed(Key::ArrowDown) {
                if let Some(sel) = app.backup_project_selected {
                    app.backup_project_selected = Some((sel + 1).min(list_len.saturating_sub(1)));
                } else if list_len > 0 {
                    app.backup_project_selected = Some(0);
                }
            }
            if i.key_pressed(Key::K) || i.key_pressed(Key::ArrowUp) {
                if let Some(sel) = app.backup_project_selected {
                    app.backup_project_selected = Some(sel.saturating_sub(1));
                } else if list_len > 0 {
                    app.backup_project_selected = Some(0);
                }
            }

            // Select project
            if i.key_pressed(Key::Enter) || i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::L)
            {
                app.select_backup_project();
            }

            // Refresh
            if i.key_pressed(Key::R) {
                app.scan_backup_projects();
            }
        }
        RestoreView::Commits => {
            let list_len = app.commits.len();

            // Navigation
            if i.key_pressed(Key::J) || i.key_pressed(Key::ArrowDown) {
                if let Some(sel) = app.commit_selected {
                    app.commit_selected = Some((sel + 1).min(list_len.saturating_sub(1)));
                } else if list_len > 0 {
                    app.commit_selected = Some(0);
                }
            }
            if i.key_pressed(Key::K) || i.key_pressed(Key::ArrowUp) {
                if let Some(sel) = app.commit_selected {
                    app.commit_selected = Some(sel.saturating_sub(1));
                } else if list_len > 0 {
                    app.commit_selected = Some(0);
                }
            }

            // Select commit
            if i.key_pressed(Key::Enter) || i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::L)
            {
                app.select_commit();
            }

            // Back
            if i.key_pressed(Key::ArrowLeft)
                || i.key_pressed(Key::H)
                || i.key_pressed(Key::Backspace)
            {
                app.back_to_backup_projects();
            }
        }
        RestoreView::Files => {
            let list_len = app.restore_files.len();

            // Navigation
            if i.key_pressed(Key::J) || i.key_pressed(Key::ArrowDown) {
                if let Some(sel) = app.restore_file_selected {
                    app.restore_file_selected = Some((sel + 1).min(list_len.saturating_sub(1)));
                } else if list_len > 0 {
                    app.restore_file_selected = Some(0);
                }
            }
            if i.key_pressed(Key::K) || i.key_pressed(Key::ArrowUp) {
                if let Some(sel) = app.restore_file_selected {
                    app.restore_file_selected = Some(sel.saturating_sub(1));
                } else if list_len > 0 {
                    app.restore_file_selected = Some(0);
                }
            }

            // Toggle selection
            if i.key_pressed(Key::Space) {
                if let Some(sel) = app.restore_file_selected {
                    if app.restore_selected.contains(&sel) {
                        app.restore_selected.remove(&sel);
                    } else {
                        app.restore_selected.insert(sel);
                    }
                }
            }

            // Select all
            if i.key_pressed(Key::A) && !i.modifiers.shift {
                for i in 0..list_len {
                    app.restore_selected.insert(i);
                }
            }

            // Deselect all
            if i.key_pressed(Key::D) {
                app.restore_selected.clear();
            }

            // Restore
            if i.key_pressed(Key::Enter) || i.key_pressed(Key::R) {
                app.show_restore_confirm();
            }

            // Back
            if i.key_pressed(Key::ArrowLeft)
                || i.key_pressed(Key::H)
                || i.key_pressed(Key::Backspace)
            {
                app.back_to_commits();
            }
        }
    }
}

fn handle_recursive_preview_keyboard(app: &mut GuiApp, ctx: &Context) {
    // First, collect input state
    let (down, up, page_down, page_up, space, toggle_all, confirm, cancel) = ctx.input(|i| {
        (
            i.key_pressed(Key::J) || i.key_pressed(Key::ArrowDown),
            i.key_pressed(Key::K) || i.key_pressed(Key::ArrowUp),
            i.key_pressed(Key::PageDown),
            i.key_pressed(Key::PageUp),
            i.key_pressed(Key::Space),
            i.key_pressed(Key::A),
            i.key_pressed(Key::Enter),
            i.key_pressed(Key::Escape) || i.key_pressed(Key::Q),
        )
    });

    // Handle cancel first (can drop the preview)
    if cancel {
        app.cancel_recursive_preview();
        return;
    }

    // Handle confirm (can drop the preview)
    if confirm {
        app.confirm_recursive_add();
        return;
    }

    // Now handle navigation and selection
    let preview = match &mut app.recursive_preview {
        Some(p) => p,
        None => return,
    };

    let list_len = preview.preview_files.len();

    // Navigation
    if down {
        preview.selected_idx = (preview.selected_idx + 1).min(list_len.saturating_sub(1));
    }
    if up {
        preview.selected_idx = preview.selected_idx.saturating_sub(1);
    }
    if page_down {
        preview.selected_idx = (preview.selected_idx + 10).min(list_len.saturating_sub(1));
    }
    if page_up {
        preview.selected_idx = preview.selected_idx.saturating_sub(10);
    }

    // Toggle selection
    if space {
        let idx = preview.selected_idx;
        if preview.selected_files.contains(&idx) {
            preview.selected_files.remove(&idx);
        } else {
            preview.selected_files.insert(idx);
        }
        // Move to next
        preview.selected_idx = (preview.selected_idx + 1).min(list_len.saturating_sub(1));
    }

    // Toggle all
    if toggle_all {
        if preview.selected_files.len() == list_len {
            preview.selected_files.clear();
        } else {
            preview.selected_files = (0..list_len).collect();
        }
    }
}
