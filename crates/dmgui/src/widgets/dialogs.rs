//! Dialog widgets for the GUI
//!
//! Modal dialogs for various actions.

use crate::app::GuiApp;
use crate::theme::Colors;
use egui::{self, RichText, TextEdit};

/// Render all modal dialogs
pub fn render_dialogs(app: &mut GuiApp, ctx: &egui::Context) {
    render_help_dialog(app, ctx);
    render_about_dialog(app, ctx);
    render_new_project_dialog(app, ctx);
    render_delete_confirm_dialog(app, ctx);
    render_git_remote_dialog(app, ctx);
    render_commit_message_dialog(app, ctx);
    render_password_dialog(app, ctx);
    render_restore_confirm_dialog(app, ctx);
}

fn render_help_dialog(app: &mut GuiApp, ctx: &egui::Context) {
    if !app.show_help {
        return;
    }

    egui::Window::new("Help")
        .collapsible(false)
        .resizable(true)
        .default_size([600.0, 500.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Dot Matrix - Keyboard Shortcuts");
                ui.add_space(10.0);

                ui.label(RichText::new("Global").color(Colors::YELLOW).strong());
                ui.label("Tab / 1-3    Switch tabs");
                ui.label("?            Show/hide help");
                ui.label("!            About dialog");
                ui.label("v            View file content");
                ui.label("q            Quit");
                ui.add_space(10.0);

                ui.label(RichText::new("Navigation").color(Colors::YELLOW).strong());
                ui.label("↑/k ↓/j      Move up/down");
                ui.label("PageUp/Down  Page up/down");
                ui.label("Home/End     Jump to start/end");
                ui.add_space(10.0);

                ui.label(RichText::new("Projects Tab").color(Colors::YELLOW).strong());
                ui.label("Enter/→/l    Expand/collapse project");
                ui.label("←/h          Collapse project");
                ui.label("m            Toggle track mode (G/B/+)");
                ui.label("x            Toggle encryption on file");
                ui.label("X            Toggle encryption for all");
                ui.label("a            Backup with message");
                ui.label("A            Silent backup");
                ui.label("s            Sync project");
                ui.label("S            Save manifest now");
                ui.label("g            Refresh git status");
                ui.label("G            Set git remote URL");
                ui.label("p            Push to remote");
                ui.label("P            Pull from remote");
                ui.label("n            New project");
                ui.label("D            Delete project");
                ui.label("r            Refresh");
                ui.add_space(10.0);

                ui.label(RichText::new("Add Files Tab").color(Colors::YELLOW).strong());
                ui.label("Enter/→/l    Open directory or add file");
                ui.label("←/h/Bksp     Parent directory");
                ui.label("a            Add selected file");
                ui.label("u            Untrack file");
                ui.label("t            Cycle track mode");
                ui.label("R            Recursive add preview");
                ui.label("p            Cycle target project");
                ui.label("n            New project");
                ui.label("~            Go to home");
                ui.add_space(10.0);

                ui.label(RichText::new("Restore Tab").color(Colors::YELLOW).strong());
                ui.label("Enter/→/l    Select item / Enter view");
                ui.label("←/h/Bksp     Back to previous view");
                ui.label("Space        Toggle file selection");
                ui.label("a            Select all files");
                ui.label("d            Deselect all files");
                ui.label("Enter/R      Restore selected files");
                ui.label("v            View file content");
                ui.add_space(10.0);

                ui.label(RichText::new("File Viewer").color(Colors::YELLOW).strong());
                ui.label("↑/k ↓/j      Scroll up/down");
                ui.label("PageUp/Down  Page up/down");
                ui.label("g/Home       Go to top");
                ui.label("G/End        Go to bottom");
                ui.label("n            Toggle line numbers");
                ui.label("v/q/Esc      Close viewer");
            });

            ui.add_space(10.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        app.show_help = false;
                    }
                });
            });
        });
}

fn render_about_dialog(app: &mut GuiApp, ctx: &egui::Context) {
    if !app.show_about {
        return;
    }

    egui::Window::new("About Dot Matrix")
        .collapsible(false)
        .resizable(false)
        .default_size([400.0, 300.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Dot Matrix");
                ui.add_space(10.0);
                ui.label("Version 2.0.1");
                ui.add_space(20.0);
                ui.label("Project compositor with git versioning.");
                ui.label("Files stay native to their tools on disk.");
                ui.label("Dotmatrix tracks, versions, and backs them up.");
                ui.add_space(20.0);
                ui.label(RichText::new("Track Modes").color(Colors::YELLOW));
                ui.label("[G] Git - For text/diffable files");
                ui.label("[B] Backup - For binary/large files");
                ui.label("[+] Both - Track via git and backup");
                ui.add_space(20.0);
                ui.label(RichText::new("Status Indicators").color(Colors::YELLOW));
                ui.horizontal(|ui| {
                    ui.label(RichText::new("✓").color(Colors::GREEN));
                    ui.label("Synced");
                    ui.add_space(10.0);
                    ui.label(RichText::new("⚠").color(Colors::YELLOW));
                    ui.label("Drifted");
                    ui.add_space(10.0);
                    ui.label(RichText::new("+").color(Colors::CYAN));
                    ui.label("New");
                    ui.add_space(10.0);
                    ui.label(RichText::new("✗").color(Colors::RED));
                    ui.label("Missing");
                });
            });

            ui.add_space(20.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        app.show_about = false;
                    }
                });
            });
        });
}

fn render_new_project_dialog(app: &mut GuiApp, ctx: &egui::Context) {
    if !app.creating_project {
        return;
    }

    egui::Window::new("New Project")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("Enter project name:");
            ui.add_space(10.0);

            let response = ui.add(
                TextEdit::singleline(&mut app.project_input)
                    .desired_width(300.0)
                    .hint_text("my-project"),
            );

            // Focus the text input
            if response.gained_focus() || app.project_input.is_empty() {
                response.request_focus();
            }
            app.text_input_focus = response.has_focus();

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Create").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    app.create_project();
                }
                if ui.button("Cancel").clicked() {
                    app.creating_project = false;
                    app.project_input.clear();
                }
            });
        });

    // Handle Escape to cancel
    ctx.input(|i| {
        if i.key_pressed(egui::Key::Escape) {
            app.creating_project = false;
            app.project_input.clear();
        }
    });
}

fn render_delete_confirm_dialog(app: &mut GuiApp, ctx: &egui::Context) {
    if !app.confirm_delete {
        return;
    }

    let project_name = app.delete_target.clone().unwrap_or_default();

    egui::Window::new("Confirm Delete")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!("Delete project '{}'?", project_name));
            ui.add_space(5.0);
            ui.label(
                RichText::new("This will remove the project from tracking.")
                    .color(Colors::YELLOW),
            );
            ui.label(
                RichText::new("Files on disk will NOT be deleted.")
                    .color(Colors::DARK_GRAY),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("Delete").color(Colors::RED))
                    .clicked()
                {
                    app.delete_project();
                }
                if ui.button("Cancel").clicked() {
                    app.confirm_delete = false;
                    app.delete_target = None;
                }
            });
        });

    // Handle keyboard
    ctx.input(|i| {
        if i.key_pressed(egui::Key::Y) || i.key_pressed(egui::Key::Enter) {
            app.delete_project();
        }
        if i.key_pressed(egui::Key::N) || i.key_pressed(egui::Key::Escape) {
            app.confirm_delete = false;
            app.delete_target = None;
        }
    });
}

fn render_git_remote_dialog(app: &mut GuiApp, ctx: &egui::Context) {
    if !app.setting_remote {
        return;
    }

    egui::Window::new("Set Git Remote")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("Enter remote URL:");
            ui.add_space(10.0);

            let response = ui.add(
                TextEdit::singleline(&mut app.remote_input)
                    .desired_width(400.0)
                    .hint_text("git@github.com:user/repo.git"),
            );

            if response.gained_focus() || app.remote_input.is_empty() {
                response.request_focus();
            }
            app.text_input_focus = response.has_focus();

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Set Remote").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    app.set_git_remote();
                }
                if ui.button("Cancel").clicked() {
                    app.setting_remote = false;
                    app.remote_input.clear();
                }
            });
        });

    ctx.input(|i| {
        if i.key_pressed(egui::Key::Escape) {
            app.setting_remote = false;
            app.remote_input.clear();
        }
    });
}

fn render_commit_message_dialog(app: &mut GuiApp, ctx: &egui::Context) {
    if !app.entering_commit_msg {
        return;
    }

    egui::Window::new("Backup Message")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("Enter commit message (optional):");
            ui.add_space(10.0);

            let response = ui.add(
                TextEdit::singleline(&mut app.commit_msg_input)
                    .desired_width(400.0)
                    .hint_text("Describe your changes..."),
            );

            if response.gained_focus() || app.commit_msg_input.is_empty() {
                response.request_focus();
            }
            app.text_input_focus = response.has_focus();

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Backup").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    app.confirm_commit_msg();
                }
                if ui.button("Cancel").clicked() {
                    app.cancel_commit_msg();
                }
            });
        });

    ctx.input(|i| {
        if i.key_pressed(egui::Key::Escape) {
            app.cancel_commit_msg();
        }
    });
}

fn render_password_dialog(app: &mut GuiApp, ctx: &egui::Context) {
    if !app.password_prompt_visible {
        return;
    }

    let title = match app.password_purpose {
        crate::state::PasswordPurpose::Backup => "Encryption Password",
        crate::state::PasswordPurpose::Restore => "Decryption Password",
    };

    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("Enter password for encrypted files:");
            ui.add_space(10.0);

            let response = ui.add(
                TextEdit::singleline(&mut app.password_input)
                    .password(true)
                    .desired_width(300.0),
            );

            if response.gained_focus() || app.password_input.is_empty() {
                response.request_focus();
            }
            app.text_input_focus = response.has_focus();

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Confirm").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    app.confirm_password();
                }
                if ui.button("Cancel").clicked() {
                    app.cancel_password();
                }
            });
        });

    ctx.input(|i| {
        if i.key_pressed(egui::Key::Escape) {
            app.cancel_password();
        }
    });
}

fn render_restore_confirm_dialog(app: &mut GuiApp, ctx: &egui::Context) {
    if !app.restore_confirm.visible {
        return;
    }

    let file_count = app.restore_confirm.files_to_restore.len();
    let will_overwrite = app.restore_confirm.will_overwrite;

    egui::Window::new("Confirm Restore")
        .collapsible(false)
        .resizable(true)
        .default_size([500.0, 400.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!("Restore {} files?", file_count));
            if will_overwrite > 0 {
                ui.label(
                    RichText::new(format!("{} files will be overwritten", will_overwrite))
                        .color(Colors::YELLOW),
                );
            }
            ui.add_space(10.0);

            // File list
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for (idx, &file_idx) in app.restore_confirm.files_to_restore.iter().enumerate() {
                        if let Some(file) = app.restore_files.get(file_idx) {
                            let is_selected = idx == app.restore_confirm.selected_idx;
                            let bg_color = if is_selected {
                                Colors::SELECTION_BG
                            } else {
                                egui::Color32::TRANSPARENT
                            };

                            let (status, status_color) = if !file.exists_locally {
                                ("NEW", Colors::CYAN)
                            } else if file.local_differs {
                                ("CHG", Colors::YELLOW)
                            } else {
                                ("OK ", Colors::GREEN)
                            };

                            egui::Frame::none().fill(bg_color).show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(status).color(status_color).monospace());
                                    ui.label(RichText::new(&file.display_path).monospace());
                                });
                            });
                        }
                    }
                });

            ui.add_space(10.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("Restore").color(Colors::GREEN))
                    .clicked()
                {
                    app.restore_confirm.visible = false;
                    app.perform_restore();
                }
                if ui.button("Cancel").clicked() {
                    app.restore_confirm.visible = false;
                }
            });
        });

    ctx.input(|i| {
        if i.key_pressed(egui::Key::Y) || i.key_pressed(egui::Key::Enter) {
            app.restore_confirm.visible = false;
            app.perform_restore();
        }
        if i.key_pressed(egui::Key::N) || i.key_pressed(egui::Key::Escape) {
            app.restore_confirm.visible = false;
        }
    });
}
