//! Project tree widget for the Projects tab
//!
//! Displays projects with expand/collapse and file status indicators.

use crate::app::GuiApp;
use crate::state::ProjectViewItem;
use crate::theme::{format_size, Colors};
use dmcore::{FileStatus, TrackMode};
use egui::{self, RichText};

/// Render the project tree
pub fn render_project_tree(app: &mut GuiApp, ui: &mut egui::Ui) {
    ui.label(
        RichText::new("Your Projects - Press Enter to expand/collapse, a to backup")
            .color(Colors::DARK_GRAY),
    );
    ui.add_space(5.0);

    // Collect items to avoid borrow issues
    let items: Vec<_> = app
        .visible_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = app.project_selected == Some(i);
            (i, item.clone(), is_selected)
        })
        .collect();

    let mut new_selection: Option<usize> = None;
    let mut double_clicked: Option<usize> = None;

    // Actions to perform after rendering
    let action_toggle = false;
    let mut action_expand = false;
    let mut action_collapse = false;
    let mut action_backup = false;
    let mut action_backup_msg = false;
    let mut action_sync = false;
    let mut action_delete = false;
    let mut action_toggle_enc = false;
    let mut action_toggle_mode = false;
    let mut action_view = false;
    let mut action_set_remote = false;
    let mut action_push = false;
    let mut action_pull = false;
    let mut action_refresh_git = false;

    egui::ScrollArea::vertical()
        .id_salt("project_tree_scroll")
        .show(ui, |ui| {
            for (i, item, is_selected) in &items {
                let bg_color = if *is_selected {
                    Colors::SELECTION_BG
                } else {
                    egui::Color32::TRANSPARENT
                };

                let (row_rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 22.0),
                    egui::Sense::hover(),
                );

                let row_response = ui.interact(
                    row_rect,
                    egui::Id::new(("project_row", *i)),
                    egui::Sense::click(),
                );

                if *is_selected || row_response.hovered() {
                    let color = if *is_selected {
                        bg_color
                    } else {
                        Colors::HOVER_BG
                    };
                    ui.painter().rect_filled(row_rect, 0.0, color);
                }

                let mut x = row_rect.left() + 4.0;
                let y = row_rect.center().y;
                let font = egui::FontId::monospace(13.0);

                match item {
                    ProjectViewItem::Project {
                        name,
                        file_count,
                        synced,
                        drifted,
                        new_files,
                        missing,
                        expanded,
                        remote_status,
                    } => {
                        // Expand/collapse icon
                        let expand_icon = if *expanded { "▼" } else { "▶" };
                        ui.painter().text(
                            egui::pos2(x, y),
                            egui::Align2::LEFT_CENTER,
                            expand_icon,
                            font.clone(),
                            Colors::BLUE,
                        );
                        x += 16.0;

                        // Project name
                        ui.painter().text(
                            egui::pos2(x, y),
                            egui::Align2::LEFT_CENTER,
                            name,
                            font.clone(),
                            Colors::YELLOW,
                        );
                        x += (name.len() as f32 * 8.0).max(100.0) + 8.0;

                        // File count and status summary
                        let status_text = format!(
                            "({} files: {} synced, {} drifted, {} new, {} missing)",
                            file_count, synced, drifted, new_files, missing
                        );
                        ui.painter().text(
                            egui::pos2(x, y),
                            egui::Align2::LEFT_CENTER,
                            &status_text,
                            font.clone(),
                            Colors::DARK_GRAY,
                        );
                        x += status_text.len() as f32 * 7.0 + 16.0;

                        // Git remote status
                        if let Some(status) = remote_status {
                            let (git_text, git_color) = if !status.has_remote {
                                ("no remote".to_string(), Colors::DARK_GRAY)
                            } else if status.ahead == 0 && status.behind == 0 {
                                ("synced".to_string(), Colors::GREEN)
                            } else if status.ahead > 0 && status.behind == 0 {
                                (format!("ahead {}", status.ahead), Colors::CYAN)
                            } else if status.behind > 0 && status.ahead == 0 {
                                (format!("behind {}", status.behind), Colors::YELLOW)
                            } else {
                                (format!("{}↑ {}↓", status.ahead, status.behind), Colors::RED)
                            };
                            ui.painter().text(
                                egui::pos2(x, y),
                                egui::Align2::LEFT_CENTER,
                                &format!("[{}]", git_text),
                                font.clone(),
                                git_color,
                            );
                        }

                        // Context menu for projects
                        row_response.context_menu(|ui| {
                            if *expanded {
                                if ui.button("Collapse").clicked() {
                                    action_collapse = true;
                                    ui.close_menu();
                                }
                            } else {
                                if ui.button("Expand").clicked() {
                                    action_expand = true;
                                    ui.close_menu();
                                }
                            }
                            ui.separator();
                            if ui.button("Backup").clicked() {
                                action_backup = true;
                                ui.close_menu();
                            }
                            if ui.button("Backup with message").clicked() {
                                action_backup_msg = true;
                                ui.close_menu();
                            }
                            if ui.button("Sync (mark all synced)").clicked() {
                                action_sync = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Set Git Remote").clicked() {
                                action_set_remote = true;
                                ui.close_menu();
                            }
                            if ui.button("Push").clicked() {
                                action_push = true;
                                ui.close_menu();
                            }
                            if ui.button("Pull").clicked() {
                                action_pull = true;
                                ui.close_menu();
                            }
                            if ui.button("Refresh Git Status").clicked() {
                                action_refresh_git = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Toggle Encryption (all files)").clicked() {
                                action_toggle_enc = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui
                                .button(RichText::new("Delete Project").color(Colors::RED))
                                .clicked()
                            {
                                action_delete = true;
                                ui.close_menu();
                            }
                        });
                    }
                    ProjectViewItem::File {
                        project_name: _,
                        path,
                        abs_path: _,
                        status,
                        size,
                        track_mode,
                        encrypted,
                    } => {
                        // Indentation for files
                        x += 20.0;

                        // Status indicator
                        let (status_char, status_color) = match status {
                            FileStatus::Synced => ("✓", Colors::GREEN),
                            FileStatus::Drifted => ("⚠", Colors::YELLOW),
                            FileStatus::New => ("+", Colors::CYAN),
                            FileStatus::Missing => ("✗", Colors::RED),
                            FileStatus::Error => ("!", Colors::RED),
                        };
                        ui.painter().text(
                            egui::pos2(x, y),
                            egui::Align2::LEFT_CENTER,
                            status_char,
                            font.clone(),
                            status_color,
                        );
                        x += 16.0;

                        // Track mode indicator
                        let (mode_text, mode_color) = match track_mode {
                            TrackMode::Git => ("[G]", Colors::CYAN),
                            TrackMode::Backup => ("[B]", Colors::MAGENTA),
                            TrackMode::Both => ("[+]", Colors::GREEN),
                        };
                        ui.painter().text(
                            egui::pos2(x, y),
                            egui::Align2::LEFT_CENTER,
                            mode_text,
                            font.clone(),
                            mode_color,
                        );
                        x += 28.0;

                        // Encryption indicator
                        if *encrypted {
                            ui.painter().text(
                                egui::pos2(x, y),
                                egui::Align2::LEFT_CENTER,
                                "[E]",
                                font.clone(),
                                Colors::YELLOW,
                            );
                        }
                        x += 28.0;

                        // File path
                        ui.painter().text(
                            egui::pos2(x, y),
                            egui::Align2::LEFT_CENTER,
                            path,
                            font.clone(),
                            Colors::WHITE,
                        );
                        x += path.len() as f32 * 8.0 + 8.0;

                        // Size
                        if let Some(s) = size {
                            ui.painter().text(
                                egui::pos2(x, y),
                                egui::Align2::LEFT_CENTER,
                                &format_size(*s),
                                font.clone(),
                                Colors::DARK_GRAY,
                            );
                        }

                        // Context menu for files
                        row_response.context_menu(|ui| {
                            if ui.button("View File").clicked() {
                                action_view = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Toggle Track Mode (G/B/+)").clicked() {
                                action_toggle_mode = true;
                                ui.close_menu();
                            }
                            let enc_label = if *encrypted {
                                "Disable Encryption"
                            } else {
                                "Enable Encryption"
                            };
                            if ui.button(enc_label).clicked() {
                                action_toggle_enc = true;
                                ui.close_menu();
                            }
                        });
                    }
                }

                // Handle clicks
                if row_response.clicked() {
                    new_selection = Some(*i);
                }
                if row_response.double_clicked() {
                    double_clicked = Some(*i);
                }
                if row_response.secondary_clicked() {
                    new_selection = Some(*i);
                }

                // Scroll to selected
                if app.project_selected == Some(*i) {
                    row_response.scroll_to_me(Some(egui::Align::Center));
                }
            }
        });

    // Apply selection
    if let Some(i) = new_selection {
        app.project_selected = Some(i);
    }

    // Handle double-click to toggle
    if double_clicked.is_some() {
        app.toggle_selected_project();
    }

    // Handle context menu actions
    if action_toggle || action_expand {
        app.toggle_selected_project();
    }
    if action_collapse {
        app.collapse_selected_project();
    }
    if action_backup {
        app.backup_project();
    }
    if action_backup_msg {
        app.backup_project_with_message();
    }
    if action_sync {
        app.sync_project();
    }
    if action_delete {
        if let Some(name) = app.selected_project_name() {
            app.delete_target = Some(name);
            app.confirm_delete = true;
        }
    }
    if action_toggle_enc {
        // Check if project or file
        if let Some(item) = app.selected_item() {
            match item {
                ProjectViewItem::Project { .. } => app.toggle_project_encryption(),
                ProjectViewItem::File { .. } => app.toggle_encryption(),
            }
        }
    }
    if action_toggle_mode {
        app.toggle_track_mode();
    }
    if action_view {
        if let Some(ProjectViewItem::File { abs_path, path, .. }) = app.selected_item() {
            let title = path.clone();
            let abs = abs_path.clone();
            app.load_file_into_viewer(&abs, &title);
        }
    }
    if action_set_remote {
        app.setting_remote = true;
        app.remote_input.clear();
    }
    if action_push {
        app.push_project();
    }
    if action_pull {
        app.pull_project();
    }
    if action_refresh_git {
        app.refresh_remote_status();
    }
}
