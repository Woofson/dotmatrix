//! File viewer widget with syntax highlighting
//!
//! Displays file content with syntax highlighting as an overlay.

use crate::app::GuiApp;
use crate::theme::Colors;
use egui::{self, RichText};

/// Render the file viewer overlay
pub fn render_file_viewer(app: &mut GuiApp, ctx: &egui::Context) {
    if !app.viewer_visible {
        return;
    }

    egui::Window::new(&app.viewer_title)
        .collapsible(false)
        .resizable(true)
        .default_size([800.0, 600.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            // Toolbar
            ui.horizontal(|ui| {
                let line_btn_text = if app.viewer_line_numbers {
                    "Hide Line Numbers"
                } else {
                    "Show Line Numbers"
                };
                if ui.button(line_btn_text).clicked() {
                    app.viewer_line_numbers = !app.viewer_line_numbers;
                }

                ui.separator();

                ui.label(
                    RichText::new(format!("{} lines", app.viewer_content.len()))
                        .color(Colors::DARK_GRAY),
                );

                // Push close button to right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        app.close_viewer();
                    }
                });
            });

            ui.separator();

            // Content area with scroll
            egui::ScrollArea::both()
                .id_salt("file_viewer_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let _line_number_width = if app.viewer_line_numbers {
                        // Calculate width based on number of lines
                        let digits = (app.viewer_content.len() as f32).log10().floor() as usize + 1;
                        (digits.max(3) as f32 * 8.0) + 16.0
                    } else {
                        0.0
                    };

                    for (idx, line) in app.viewer_content.iter().enumerate() {
                        ui.horizontal(|ui| {
                            // Line number
                            if app.viewer_line_numbers {
                                let line_num = format!("{:>4}", idx + 1);
                                ui.label(
                                    RichText::new(line_num)
                                        .color(Colors::DARK_GRAY)
                                        .monospace(),
                                );
                                ui.add_space(8.0);
                            }

                            // File header styling
                            if line.file_header {
                                for (text, color) in &line.spans {
                                    ui.label(RichText::new(text).color(*color).strong().monospace());
                                }
                            } else if line.spans.is_empty() {
                                // Empty line
                                ui.label(RichText::new(" ").monospace());
                            } else {
                                // Regular line with syntax highlighting
                                for (text, color) in &line.spans {
                                    ui.label(RichText::new(text).color(*color).monospace());
                                }
                            }
                        });
                    }
                });
        });

    // Handle keyboard shortcuts for viewer
    ctx.input(|i| {
        if i.key_pressed(egui::Key::Escape)
            || i.key_pressed(egui::Key::Q)
            || i.key_pressed(egui::Key::V)
        {
            app.close_viewer();
        }

        // Navigation
        if i.key_pressed(egui::Key::J) || i.key_pressed(egui::Key::ArrowDown) {
            app.viewer_scroll = (app.viewer_scroll + 1).min(app.viewer_content.len().saturating_sub(1));
        }
        if i.key_pressed(egui::Key::K) || i.key_pressed(egui::Key::ArrowUp) {
            app.viewer_scroll = app.viewer_scroll.saturating_sub(1);
        }
        if i.key_pressed(egui::Key::PageDown) {
            app.viewer_scroll = (app.viewer_scroll + 20).min(app.viewer_content.len().saturating_sub(1));
        }
        if i.key_pressed(egui::Key::PageUp) {
            app.viewer_scroll = app.viewer_scroll.saturating_sub(20);
        }
        if i.key_pressed(egui::Key::Home) || (i.key_pressed(egui::Key::G) && !i.modifiers.shift) {
            app.viewer_scroll = 0;
        }
        if i.key_pressed(egui::Key::End) || (i.key_pressed(egui::Key::G) && i.modifiers.shift) {
            app.viewer_scroll = app.viewer_content.len().saturating_sub(1);
        }
        if i.key_pressed(egui::Key::N) {
            app.viewer_line_numbers = !app.viewer_line_numbers;
        }
    });
}
