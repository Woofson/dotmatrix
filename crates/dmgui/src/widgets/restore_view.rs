//! Restore view widget for the Restore tab
//!
//! Three-level navigation: projects -> commits -> files

use crate::app::GuiApp;
use crate::state::RestoreView;
use crate::theme::{format_size, Colors};
use egui::{self, RichText};

/// Render the restore view
pub fn render_restore_view(app: &mut GuiApp, ui: &mut egui::Ui) {
    match app.restore_view {
        RestoreView::Projects => render_backup_projects(app, ui),
        RestoreView::Commits => render_commits(app, ui),
        RestoreView::Files => render_restore_files(app, ui),
    }
}

fn render_backup_projects(app: &mut GuiApp, ui: &mut egui::Ui) {
    ui.label(
        RichText::new("Backup Projects - Select a project to view its backup history")
            .color(Colors::DARK_GRAY),
    );
    ui.add_space(5.0);

    if app.backup_projects.is_empty() {
        ui.label(RichText::new("No backup projects found.").color(Colors::DARK_GRAY));
        ui.label(
            RichText::new("Back up a project first to see it here.").color(Colors::DARK_GRAY),
        );
        return;
    }

    // Refresh button
    ui.horizontal(|ui| {
        if ui.button("🔄 Refresh").clicked() {
            app.scan_backup_projects();
        }
    });
    ui.add_space(5.0);

    let items: Vec<_> = app
        .backup_projects
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let is_selected = app.backup_project_selected == Some(i);
            (
                i,
                p.name.clone(),
                p.commit_count,
                p.last_backup.clone(),
                is_selected,
            )
        })
        .collect();

    let mut new_selection: Option<usize> = None;
    let mut double_clicked: Option<usize> = None;

    egui::ScrollArea::vertical()
        .id_salt("backup_projects_scroll")
        .show(ui, |ui| {
            for (i, name, commit_count, last_backup, is_selected) in &items {
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
                    egui::Id::new(("backup_project_row", *i)),
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

                // Project name
                ui.painter().text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    name,
                    font.clone(),
                    Colors::YELLOW,
                );
                x += (name.len() as f32 * 8.0).max(150.0) + 16.0;

                // Commit count
                ui.painter().text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    &format!("{} backups", commit_count),
                    font.clone(),
                    Colors::CYAN,
                );
                x += 100.0;

                // Last backup date
                if let Some(date) = last_backup {
                    ui.painter().text(
                        egui::pos2(x, y),
                        egui::Align2::LEFT_CENTER,
                        &format!("Last: {}", date),
                        font.clone(),
                        Colors::DARK_GRAY,
                    );
                }

                // Handle clicks
                if row_response.clicked() {
                    new_selection = Some(*i);
                }
                if row_response.double_clicked() {
                    double_clicked = Some(*i);
                }

                // Context menu
                row_response.context_menu(|ui| {
                    if ui.button("View Backups").clicked() {
                        double_clicked = Some(*i);
                        ui.close_menu();
                    }
                });

                // Scroll to selected
                if app.backup_project_selected == Some(*i) {
                    row_response.scroll_to_me(Some(egui::Align::Center));
                }
            }
        });

    // Apply selection
    if let Some(i) = new_selection {
        app.backup_project_selected = Some(i);
    }

    // Handle double-click
    if double_clicked.is_some() {
        app.select_backup_project();
    }
}

fn render_commits(app: &mut GuiApp, ui: &mut egui::Ui) {
    // Navigation toolbar
    ui.horizontal(|ui| {
        if ui.button("⬅ Back to Projects").clicked() {
            app.back_to_backup_projects();
        }

        ui.separator();

        if let Some(name) = &app.selected_backup_project {
            ui.label(RichText::new(format!("Project: {}", name)).color(Colors::YELLOW).strong());
        }
    });
    ui.add_space(5.0);

    ui.label(
        RichText::new("Backup History - Select a backup to view its files")
            .color(Colors::DARK_GRAY),
    );
    ui.add_space(5.0);

    if app.commits.is_empty() {
        ui.label(RichText::new("No backups found for this project.").color(Colors::DARK_GRAY));
        return;
    }

    let items: Vec<_> = app
        .commits
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let is_selected = app.commit_selected == Some(i);
            (
                i,
                c.short_hash.clone(),
                c.date.clone(),
                c.message.clone(),
                is_selected,
            )
        })
        .collect();

    let mut new_selection: Option<usize> = None;
    let mut double_clicked: Option<usize> = None;

    egui::ScrollArea::vertical()
        .id_salt("commits_scroll")
        .show(ui, |ui| {
            for (i, short_hash, date, message, is_selected) in &items {
                let bg_color = if *is_selected {
                    Colors::SELECTION_BG
                } else {
                    egui::Color32::TRANSPARENT
                };

                let (row_rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 20.0),
                    egui::Sense::hover(),
                );

                let row_response = ui.interact(
                    row_rect,
                    egui::Id::new(("commit_row", *i)),
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

                // Hash
                ui.painter().text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    short_hash,
                    font.clone(),
                    Colors::YELLOW,
                );
                x += 70.0;

                // Date (truncated to 19 chars)
                let date_short = if date.len() > 19 {
                    &date[..19]
                } else {
                    date
                };
                ui.painter().text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    date_short,
                    font.clone(),
                    Colors::CYAN,
                );
                x += 160.0;

                // Message
                ui.painter().text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    message,
                    font.clone(),
                    Colors::WHITE,
                );

                // Handle clicks
                if row_response.clicked() {
                    new_selection = Some(*i);
                }
                if row_response.double_clicked() {
                    double_clicked = Some(*i);
                }

                // Context menu
                row_response.context_menu(|ui| {
                    if ui.button("View Files").clicked() {
                        double_clicked = Some(*i);
                        ui.close_menu();
                    }
                });

                // Scroll to selected
                if app.commit_selected == Some(*i) {
                    row_response.scroll_to_me(Some(egui::Align::Center));
                }
            }
        });

    // Apply selection
    if let Some(i) = new_selection {
        app.commit_selected = Some(i);
    }

    // Handle double-click
    if double_clicked.is_some() {
        app.select_commit();
    }
}

fn render_restore_files(app: &mut GuiApp, ui: &mut egui::Ui) {
    // Navigation toolbar
    ui.horizontal(|ui| {
        if ui.button("⬅ Back to Backups").clicked() {
            app.back_to_commits();
        }

        ui.separator();

        // Show commit info
        let commit_info = app
            .selected_commit
            .and_then(|i| app.commits.get(i))
            .map(|c| format!("{} - {}", c.short_hash, c.message))
            .unwrap_or_else(|| "Unknown".to_string());
        ui.label(RichText::new(&commit_info).color(Colors::CYAN).strong());
    });
    ui.add_space(5.0);

    ui.label(
        RichText::new("Files in Backup - Space to select, Enter to restore")
            .color(Colors::DARK_GRAY),
    );
    ui.add_space(5.0);

    if app.restore_files.is_empty() {
        ui.label(RichText::new("No files in this backup.").color(Colors::DARK_GRAY));
        return;
    }

    // Action buttons
    ui.horizontal(|ui| {
        if ui.button("Select All").clicked() {
            for i in 0..app.restore_files.len() {
                app.restore_selected.insert(i);
            }
        }
        if ui.button("Deselect All").clicked() {
            app.restore_selected.clear();
        }

        ui.separator();

        let count = if app.restore_selected.is_empty() {
            if app.restore_file_selected.is_some() {
                1
            } else {
                0
            }
        } else {
            app.restore_selected.len()
        };
        if count > 0 {
            if ui
                .button(RichText::new(format!("Restore {} files", count)).color(Colors::GREEN))
                .clicked()
            {
                app.show_restore_confirm();
            }
        }
    });
    ui.add_space(5.0);

    let items: Vec<_> = app
        .restore_files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let is_selected = app.restore_file_selected == Some(i);
            let is_checked = app.restore_selected.contains(&i);
            (
                i,
                f.display_path.clone(),
                f.size,
                f.exists_locally,
                f.local_differs,
                f.encrypted,
                is_selected,
                is_checked,
            )
        })
        .collect();

    let mut new_selection: Option<usize> = None;
    let mut toggle_selection: Option<usize> = None;

    // Actions
    let mut action_restore = false;
    let mut action_view = false;

    egui::ScrollArea::vertical()
        .id_salt("restore_files_scroll")
        .show(ui, |ui| {
            for (i, display_path, size, exists_locally, local_differs, encrypted, is_selected, is_checked) in &items {
                let bg_color = if *is_selected {
                    Colors::SELECTION_BG
                } else {
                    egui::Color32::TRANSPARENT
                };

                let (row_rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 20.0),
                    egui::Sense::hover(),
                );

                let row_response = ui.interact(
                    row_rect,
                    egui::Id::new(("restore_file_row", *i)),
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

                // Selection checkbox
                let marker = if *is_checked { "[x]" } else { "[ ]" };
                let marker_color = if *is_checked {
                    Colors::GREEN
                } else {
                    Colors::DARK_GRAY
                };
                ui.painter().text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    marker,
                    font.clone(),
                    marker_color,
                );
                x += 30.0;

                // Status indicator
                let (status, status_color) = if !*exists_locally {
                    ("NEW", Colors::CYAN)
                } else if *local_differs {
                    ("CHG", Colors::YELLOW)
                } else {
                    ("OK ", Colors::GREEN)
                };
                ui.painter().text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    status,
                    font.clone(),
                    status_color,
                );
                x += 35.0;

                // Encryption indicator
                if *encrypted {
                    ui.painter().text(
                        egui::pos2(x, y),
                        egui::Align2::LEFT_CENTER,
                        "[E]",
                        font.clone(),
                        Colors::MAGENTA,
                    );
                }
                x += 28.0;

                // Size
                ui.painter().text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    &format_size(*size),
                    font.clone(),
                    Colors::DARK_GRAY,
                );
                x += 70.0;

                // Path
                let path_color = if *local_differs || !*exists_locally {
                    Colors::WHITE
                } else {
                    Colors::DARK_GRAY
                };
                ui.painter().text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    display_path,
                    font.clone(),
                    path_color,
                );

                // Handle clicks
                if row_response.clicked() {
                    new_selection = Some(*i);
                }
                if row_response.double_clicked() {
                    toggle_selection = Some(*i);
                }

                // Context menu
                row_response.context_menu(|ui| {
                    let select_label = if *is_checked { "Deselect" } else { "Select" };
                    if ui.button(select_label).clicked() {
                        toggle_selection = Some(*i);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("View File").clicked() {
                        action_view = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Restore This File").clicked() {
                        action_restore = true;
                        ui.close_menu();
                    }
                });

                // Scroll to selected
                if app.restore_file_selected == Some(*i) {
                    row_response.scroll_to_me(Some(egui::Align::Center));
                }
            }
        });

    // Apply selection
    if let Some(i) = new_selection {
        app.restore_file_selected = Some(i);
    }

    // Toggle selection on double-click
    if let Some(i) = toggle_selection {
        if app.restore_selected.contains(&i) {
            app.restore_selected.remove(&i);
        } else {
            app.restore_selected.insert(i);
        }
    }

    // Handle context menu actions
    if action_restore {
        app.perform_restore();
    }
    if action_view {
        // TODO: Implement view backup file
        app.message = Some(("View backup file not yet implemented".to_string(), false));
    }
}
