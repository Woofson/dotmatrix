//! Widget modules for the GUI
//!
//! Each widget renders a specific part of the UI.

mod dialogs;
mod file_browser;
mod file_viewer;
mod project_tree;
mod restore_view;

pub use dialogs::render_dialogs;
pub use file_browser::render_file_browser;
pub use file_viewer::render_file_viewer;
pub use project_tree::render_project_tree;
pub use restore_view::render_restore_view;

use crate::app::GuiApp;
use crate::state::Mode;
use crate::theme::Colors;
use egui::{self, RichText};

/// Render the tab bar at the top
pub fn render_tabs(app: &mut GuiApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        for (i, title) in Mode::titles().iter().enumerate() {
            let is_selected = app.mode.index() == i;

            let bg_color = if is_selected {
                Colors::TAB_SELECTED_BG
            } else {
                Colors::TAB_BG
            };

            let text_color = if is_selected {
                Colors::YELLOW
            } else {
                Colors::DARK_GRAY
            };

            let rounding = egui::Rounding {
                nw: 6.0,
                ne: 6.0,
                sw: 0.0,
                se: 0.0,
            };

            let frame_response = egui::Frame::none()
                .fill(bg_color)
                .rounding(rounding)
                .inner_margin(egui::Margin::symmetric(16.0, 8.0))
                .stroke(if is_selected {
                    egui::Stroke::new(1.0, Colors::YELLOW)
                } else {
                    egui::Stroke::NONE
                })
                .show(ui, |ui| {
                    let text = RichText::new(*title).color(text_color);
                    let text = if is_selected { text.strong() } else { text };

                    if ui
                        .add(egui::Label::new(text).sense(egui::Sense::click()))
                        .clicked()
                    {
                        app.mode = Mode::from_index(i);
                        app.message = None;
                    }
                });

            if frame_response.response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            ui.add_space(2.0);
        }

        // Push help/about buttons to the right
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // About button
            if ui
                .add(
                    egui::Button::new(RichText::new("!").color(Colors::DARK_GRAY))
                        .frame(false),
                )
                .on_hover_text("About")
                .clicked()
            {
                app.show_about = !app.show_about;
            }

            // Help button
            if ui
                .add(
                    egui::Button::new(RichText::new("?").color(Colors::DARK_GRAY))
                        .frame(false),
                )
                .on_hover_text("Help")
                .clicked()
            {
                app.show_help = !app.show_help;
            }
        });
    });
}

/// Render the status bar at the bottom
pub fn render_status_bar(app: &mut GuiApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        // Busy indicator
        if app.busy {
            let spinner = crate::theme::SPINNER_FRAMES[app.spinner_frame];
            ui.label(RichText::new(spinner).color(Colors::CYAN));
            ui.label(RichText::new(&app.busy_message).color(Colors::YELLOW));
        } else if let Some((msg, is_error)) = &app.message {
            let color = if *is_error { Colors::RED } else { Colors::GREEN };
            ui.label(RichText::new(msg).color(color));
        }

        // Push keyboard hints to the right
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            match app.mode {
                Mode::Projects => {
                    ui.label(RichText::new("a: backup | A: silent backup | s: sync | n: new project | ?: help").color(Colors::DARK_GRAY).small());
                }
                Mode::Add => {
                    let target = app.target_project.as_deref().unwrap_or("none");
                    ui.label(RichText::new(format!("Target: {} | p: cycle project | t: track mode | R: recursive", target)).color(Colors::DARK_GRAY).small());
                }
                Mode::Restore => {
                    ui.label(RichText::new("Space: select | Enter: restore | v: view").color(Colors::DARK_GRAY).small());
                }
            }
        });
    });
}

/// Render the main content area
pub fn render_main_content(app: &mut GuiApp, ui: &mut egui::Ui) {
    // Handle recursive preview mode
    if app.recursive_preview.is_some() {
        render_recursive_preview(app, ui);
        return;
    }

    match app.mode {
        Mode::Projects => render_project_tree(app, ui),
        Mode::Add => render_file_browser(app, ui),
        Mode::Restore => render_restore_view(app, ui),
    }
}

/// Render the recursive add preview
fn render_recursive_preview(app: &mut GuiApp, ui: &mut egui::Ui) {
    let preview = match &app.recursive_preview {
        Some(p) => p,
        None => return,
    };

    // Header
    ui.horizontal(|ui| {
        ui.label("Adding recursively: ");
        let display = if let Some(home) = dirs::home_dir() {
            if let Ok(rel) = preview.source_dir.strip_prefix(&home) {
                format!("~/{}", rel.display())
            } else {
                preview.source_dir.display().to_string()
            }
        } else {
            preview.source_dir.display().to_string()
        };
        ui.label(RichText::new(display).color(Colors::YELLOW).strong());
    });

    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}", preview.selected_files.len())).color(Colors::GREEN));
        ui.label(format!(" / {} files selected", preview.preview_files.len()));
    });

    ui.horizontal(|ui| {
        ui.label(RichText::new("Space").color(Colors::CYAN));
        ui.label(": toggle | ");
        ui.label(RichText::new("a").color(Colors::CYAN));
        ui.label(": select all | ");
        ui.label(RichText::new("Enter").color(Colors::GREEN));
        ui.label(": add | ");
        ui.label(RichText::new("Esc").color(Colors::RED));
        ui.label(": cancel");
    });

    ui.separator();

    // File list
    let preview_files: Vec<_> = preview
        .preview_files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let is_selected = preview.selected_idx == i;
            let is_checked = preview.selected_files.contains(&i);
            (i, f.display_path.clone(), f.size, is_selected, is_checked)
        })
        .collect();

    let selected_idx = preview.selected_idx;

    egui::ScrollArea::vertical()
        .id_salt("recursive_preview_scroll")
        .show(ui, |ui| {
            for (i, display_path, size, is_selected, is_checked) in &preview_files {
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
                    egui::Id::new(("preview_row", *i)),
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

                // Checkbox
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

                // Size
                let size_str = crate::theme::format_size(*size);
                ui.painter().text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    &size_str,
                    font.clone(),
                    Colors::DARK_GRAY,
                );
                x += 70.0;

                // Path
                let text_color = if *is_checked {
                    Colors::WHITE
                } else {
                    Colors::DARK_GRAY
                };
                ui.painter().text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    display_path,
                    font,
                    text_color,
                );

                // Scroll to selected
                if selected_idx == *i {
                    row_response.scroll_to_me(Some(egui::Align::Center));
                }
            }
        });
}
