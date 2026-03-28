//! File browser widget for the Add Files tab
//!
//! Displays directory contents with tracking status indicators.

use crate::app::GuiApp;
use crate::theme::{format_size, Colors};
use egui::{self, RichText};

/// Render the file browser
pub fn render_file_browser(app: &mut GuiApp, ui: &mut egui::Ui) {
    // Navigation toolbar
    ui.horizontal(|ui| {
        // Back button
        if ui.button("⬅ Back").clicked() {
            if let Some(parent) = app.browse_dir.parent() {
                let _parent = parent.to_path_buf();
                app.enter_directory(&app.browse_dir.join(".."));
            }
        }

        // Home button
        if ui.button("🏠 Home").clicked() {
            app.go_home();
        }

        ui.separator();

        // Path display
        let path_display = if let Some(home) = dirs::home_dir() {
            if let Ok(rel) = app.browse_dir.strip_prefix(&home) {
                format!("~/{}", rel.display())
            } else {
                app.browse_dir.display().to_string()
            }
        } else {
            app.browse_dir.display().to_string()
        };
        ui.label(RichText::new(&path_display).color(Colors::CYAN).strong());
    });
    ui.add_space(5.0);

    // Collect items
    let items: Vec<_> = app
        .browse_files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let is_selected = app.browse_selected == Some(i);
            (
                i,
                file.path.clone(),
                file.name.clone(),
                file.is_dir,
                file.size,
                file.tracked_in.clone(),
                is_selected,
            )
        })
        .collect();

    let mut new_selection: Option<usize> = None;
    let mut double_clicked: Option<usize> = None;

    // Actions
    let mut action_enter = false;
    let mut action_add = false;
    let mut action_untrack = false;
    let mut action_view = false;
    let mut action_recursive = false;

    egui::ScrollArea::vertical()
        .id_salt("file_browser_scroll")
        .show(ui, |ui| {
            for (i, _path, name, is_dir, size, tracked_in, is_selected) in &items {
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
                    egui::Id::new(("browse_row", *i)),
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

                // Tracked indicator
                let is_tracked = !tracked_in.is_empty();
                if is_tracked {
                    ui.painter().text(
                        egui::pos2(x, y),
                        egui::Align2::LEFT_CENTER,
                        "✓",
                        font.clone(),
                        Colors::YELLOW,
                    );
                } else if *is_dir && name != ".." {
                    ui.painter().text(
                        egui::pos2(x, y),
                        egui::Align2::LEFT_CENTER,
                        "/",
                        font.clone(),
                        Colors::DARK_GRAY,
                    );
                } else {
                    ui.painter().text(
                        egui::pos2(x, y),
                        egui::Align2::LEFT_CENTER,
                        " ",
                        font.clone(),
                        Colors::DARK_GRAY,
                    );
                }
                x += 14.0;

                // Name with color
                let name_color = if *is_dir {
                    Colors::BLUE
                } else if is_tracked {
                    Colors::YELLOW
                } else {
                    Colors::WHITE
                };

                let display_name = if *is_dir && name != ".." {
                    format!("{}/", name)
                } else {
                    name.clone()
                };

                ui.painter().text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    &display_name,
                    font.clone(),
                    name_color,
                );
                x += (display_name.len() as f32 * 8.0).max(150.0) + 8.0;

                // Tracked in info
                if !tracked_in.is_empty() {
                    let tracked_text = if tracked_in.len() == 1 {
                        format!("[{}]", tracked_in[0])
                    } else if tracked_in.len() <= 3 {
                        format!("[{}]", tracked_in.join(", "))
                    } else {
                        format!("[{}, +{}]", tracked_in[0], tracked_in.len() - 1)
                    };
                    ui.painter().text(
                        egui::pos2(x, y),
                        egui::Align2::LEFT_CENTER,
                        &tracked_text,
                        font.clone(),
                        Colors::CYAN,
                    );
                    x += tracked_text.len() as f32 * 7.0 + 8.0;
                }

                // Size (for files)
                if !*is_dir {
                    if let Some(s) = size {
                        ui.painter().text(
                            egui::pos2(x, y),
                            egui::Align2::LEFT_CENTER,
                            &format_size(*s),
                            font.clone(),
                            Colors::DARK_GRAY,
                        );
                    }
                }

                // Context menu
                row_response.context_menu(|ui| {
                    if *is_dir {
                        if name != ".." {
                            if ui.button("Open Folder").clicked() {
                                action_enter = true;
                                ui.close_menu();
                            }
                            if ui.button("Recursive Add Preview").clicked() {
                                action_recursive = true;
                                ui.close_menu();
                            }
                        }
                    } else {
                        if is_tracked {
                            if ui.button("Untrack File").clicked() {
                                action_untrack = true;
                                ui.close_menu();
                            }
                        } else {
                            if ui.button("Add to Project").clicked() {
                                action_add = true;
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        if ui.button("View File").clicked() {
                            action_view = true;
                            ui.close_menu();
                        }
                    }
                });

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
                if app.browse_selected == Some(*i) {
                    row_response.scroll_to_me(Some(egui::Align::Center));
                }
            }
        });

    // Apply selection
    if let Some(i) = new_selection {
        app.browse_selected = Some(i);
    }

    // Handle double-click
    if let Some(i) = double_clicked {
        app.browse_selected = Some(i);
        if let Some(file) = app.browse_files.get(i) {
            if file.is_dir {
                let path = file.path.clone();
                app.enter_directory(&path);
            } else if file.is_tracked() {
                // Already tracked, do nothing or show message
            } else {
                let path = file.path.clone();
                app.add_file_to_project(&path);
            }
        }
    }

    // Handle context menu actions
    if action_enter {
        if let Some(idx) = app.browse_selected {
            if let Some(file) = app.browse_files.get(idx) {
                let path = file.path.clone();
                app.enter_directory(&path);
            }
        }
    }
    if action_add {
        if let Some(idx) = app.browse_selected {
            if let Some(file) = app.browse_files.get(idx) {
                let path = file.path.clone();
                app.add_file_to_project(&path);
            }
        }
    }
    if action_untrack {
        if let Some(idx) = app.browse_selected {
            if let Some(file) = app.browse_files.get(idx) {
                let path = file.path.clone();
                app.untrack_file(&path);
            }
        }
    }
    if action_view {
        if let Some(idx) = app.browse_selected {
            if let Some(file) = app.browse_files.get(idx) {
                let path = file.path.clone();
                let name = file.name.clone();
                app.load_file_into_viewer(&path, &name);
            }
        }
    }
    if action_recursive {
        app.start_recursive_preview();
    }
}
