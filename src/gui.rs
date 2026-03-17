//! GUI frontend using egui/eframe.
//!
//! This module provides a cross-platform graphical interface that mirrors
//! the TUI functionality. It uses the shared `App` state from the `app` module.

use crate::app::{
    App, AddSubMode, PasswordPurpose, RestoreView, TuiMode,
    format_size, SPINNER_FRAMES,
};
use crate::config::{BackupMode, Config, TrackedPattern};
use crate::index::Index;
use anyhow::Result;
use eframe::egui::{self, Color32, Key, RichText, ScrollArea, TextEdit};
use std::path::PathBuf;

/// Color scheme matching TUI theme
struct Colors;

impl Colors {
    const GREEN: Color32 = Color32::from_rgb(0, 200, 0);
    const YELLOW: Color32 = Color32::from_rgb(230, 200, 0);
    const CYAN: Color32 = Color32::from_rgb(0, 200, 200);
    const RED: Color32 = Color32::from_rgb(200, 50, 50);
    const BLUE: Color32 = Color32::from_rgb(100, 150, 255);
    const DARK_GRAY: Color32 = Color32::from_rgb(128, 128, 128);
    const WHITE: Color32 = Color32::from_rgb(220, 220, 220);
    const MAGENTA: Color32 = Color32::from_rgb(200, 100, 200);
    const SELECTION_BG: Color32 = Color32::from_rgb(60, 60, 80);
}

/// GUI wrapper around the shared App state
pub struct GuiApp {
    app: App,
    // GUI-specific state
    text_input_focus: bool,
}

impl GuiApp {
    pub fn new(config: Config, index: Index, config_path: PathBuf, index_path: PathBuf, data_dir: PathBuf) -> Self {
        Self {
            app: App::new(config, index, config_path, index_path, data_dir),
            text_input_focus: false,
        }
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        // Don't process keyboard shortcuts while typing in text input
        if self.text_input_focus {
            return;
        }

        ctx.input(|i| {
            // Clear message on any key press (so notifications don't persist forever)
            if i.keys_down.len() > 0 && self.app.message.is_some() && !self.app.busy {
                self.app.message = None;
            }

            // Ctrl+Q quits the application
            if i.key_pressed(Key::Q) && i.modifiers.ctrl {
                if !self.app.busy {
                    self.app.should_quit = true;
                }
            }

            // Q without Ctrl and Escape both close overlays/go back (but don't quit)
            let close_or_back = (i.key_pressed(Key::Q) && !i.modifiers.ctrl) || i.key_pressed(Key::Escape);
            if close_or_back {
                // First: close any open message/notification
                if self.app.message.is_some() {
                    self.app.message = None;
                }
                // Then: close help overlay
                else if self.app.show_help {
                    self.app.show_help = false;
                }
                // Then: close file viewer
                else if self.app.viewer_visible {
                    self.app.close_viewer();
                }
                // Then: cancel recursive preview
                else if self.app.add_sub_mode == AddSubMode::RecursivePreview {
                    self.app.cancel_recursive_preview();
                }
                // Then: close add input dialog
                else if self.app.add_mode {
                    self.app.add_input.clear();
                    self.app.add_mode = false;
                }
                // Then: close backup message dialog
                else if self.app.backup_message_mode {
                    self.app.backup_message_input.clear();
                    self.app.backup_message_mode = false;
                }
                // Then: close password prompt
                else if self.app.password_prompt_visible {
                    self.app.cancel_password();
                    self.app.message = Some("Password required for encrypted files".to_string());
                }
                // Then: close remote dialog
                else if self.app.remote_dialog_visible {
                    self.app.cancel_remote_dialog();
                }
                // Then: go back in Add mode (but don't quit at home)
                else if self.app.mode == TuiMode::Add {
                    let home = dirs::home_dir().unwrap_or_default();
                    if self.app.browse_dir != home {
                        self.app.parent_directory();
                    }
                    // At home directory, do nothing - user must use Ctrl+Q to quit
                }
                // Then: go back in Restore files view
                else if self.app.mode == TuiMode::Browse && self.app.restore_view == RestoreView::Files {
                    self.app.back_to_commits();
                }
                // At top level: do nothing - user must use Ctrl+Q to quit
            }

            // Help toggle
            if i.key_pressed(Key::F1) {
                self.app.show_help = !self.app.show_help;
            }

            // Navigation (if not in overlay mode)
            if !self.app.show_help && !self.app.viewer_visible && !self.app.add_mode && !self.app.backup_message_mode && !self.app.password_prompt_visible && !self.app.remote_dialog_visible {
                // Tab switching
                if i.key_pressed(Key::Tab) && !i.modifiers.shift {
                    self.app.next_mode();
                }
                if i.key_pressed(Key::Tab) && i.modifiers.shift {
                    self.app.prev_mode();
                }

                // Recursive preview mode navigation
                if self.app.add_sub_mode == AddSubMode::RecursivePreview {
                    if i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::J) {
                        self.app.preview_next();
                    }
                    if i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::K) {
                        self.app.preview_previous();
                    }
                    if i.key_pressed(Key::Space) {
                        self.app.toggle_preview_file();
                        self.app.preview_next();
                    }
                    if i.key_pressed(Key::A) && i.modifiers.ctrl {
                        self.app.toggle_all_preview_files();
                    }
                    if i.key_pressed(Key::Enter) {
                        self.app.confirm_recursive_add();
                    }
                    if i.key_pressed(Key::PageDown) {
                        self.app.preview_page_down();
                    }
                    if i.key_pressed(Key::PageUp) {
                        self.app.preview_page_up();
                    }
                    return;
                }

                // List navigation
                if i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::J) {
                    self.app.next();
                }
                if i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::K) {
                    self.app.previous();
                }
                if i.key_pressed(Key::PageDown) {
                    self.app.page_down();
                }
                if i.key_pressed(Key::PageUp) {
                    self.app.page_up();
                }
                if i.key_pressed(Key::Home) || (i.key_pressed(Key::G) && !i.modifiers.shift) {
                    self.app.list_state.select(Some(0));
                }
                if i.key_pressed(Key::End) || (i.key_pressed(Key::G) && i.modifiers.shift) {
                    if !self.app.files.is_empty() {
                        self.app.list_state.select(Some(self.app.files.len() - 1));
                    }
                }

                // Selection
                if i.key_pressed(Key::Space) {
                    self.app.toggle_select();
                    self.app.next();
                }
                if i.key_pressed(Key::A) && i.modifiers.ctrl {
                    self.app.select_all();
                }

                // Save and reload
                if i.key_pressed(Key::S) && i.modifiers.shift {
                    self.app.save_and_reload();
                }

                // Mode-specific actions
                match self.app.mode {
                    TuiMode::Status => {
                        if i.key_pressed(Key::Enter) || i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::L) {
                            if let Some(idx) = self.app.list_state.selected() {
                                if idx < self.app.files.len() && self.app.files[idx].is_folder_node {
                                    self.app.expand_folder();
                                }
                            }
                        }
                        if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::H) || i.key_pressed(Key::Backspace) {
                            self.app.collapse_folder();
                        }
                        if i.key_pressed(Key::B) && !i.modifiers.shift {
                            self.app.perform_backup(None);
                        }
                        if i.key_pressed(Key::B) && i.modifiers.shift {
                            self.app.backup_message_mode = true;
                        }
                        if i.key_pressed(Key::D) || i.key_pressed(Key::Delete) {
                            self.app.toggle_tracking();
                        }
                        if i.key_pressed(Key::E) && !i.modifiers.shift {
                            self.app.expand_all_folders();
                        }
                        if i.key_pressed(Key::E) && i.modifiers.shift {
                            self.app.collapse_all_folders();
                        }
                    }
                    TuiMode::Add => {
                        if i.key_pressed(Key::Enter) || i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::L) {
                            if let Some(idx) = self.app.list_state.selected() {
                                if idx < self.app.files.len() && self.app.files[idx].is_dir {
                                    self.app.enter_directory();
                                } else {
                                    self.app.toggle_tracking();
                                }
                            }
                        }
                        if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::H) || i.key_pressed(Key::Backspace) {
                            self.app.parent_directory();
                        }
                        if i.key_pressed(Key::A) && i.modifiers.shift {
                            self.app.add_folder_pattern();
                        }
                        if i.key_pressed(Key::A) && !i.modifiers.ctrl && !i.modifiers.shift {
                            self.app.add_mode = true;
                        }
                        if i.key_pressed(Key::R) && i.modifiers.shift {
                            self.app.start_recursive_preview();
                        }
                        if i.key_pressed(Key::D) || i.key_pressed(Key::Delete) {
                            self.app.remove_from_tracking_in_browser();
                        }
                    }
                    TuiMode::Browse => {
                        if i.key_pressed(Key::Enter) || i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::L) {
                            match self.app.restore_view {
                                RestoreView::Commits => self.app.select_commit(),
                                RestoreView::Files => self.app.perform_restore(),
                            }
                        }
                        if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::H) || i.key_pressed(Key::Backspace) {
                            if self.app.restore_view == RestoreView::Files {
                                self.app.back_to_commits();
                            }
                        }
                    }
                }

                // View file
                if i.key_pressed(Key::V) {
                    self.app.open_viewer();
                }

                // Refresh
                if i.key_pressed(Key::R) && !i.modifiers.shift {
                    self.app.reload_index();
                    self.app.refresh_files();
                    self.app.message = Some("Refreshed".to_string());
                }
            }

            // Viewer mode navigation
            if self.app.viewer_visible {
                if i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::J) {
                    let max_scroll = self.app.viewer_content.len().saturating_sub(1);
                    if self.app.viewer_scroll < max_scroll {
                        self.app.viewer_scroll += 1;
                    }
                }
                if i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::K) {
                    self.app.viewer_scroll = self.app.viewer_scroll.saturating_sub(1);
                }
                if i.key_pressed(Key::PageDown) {
                    let max_scroll = self.app.viewer_content.len().saturating_sub(1);
                    self.app.viewer_scroll = (self.app.viewer_scroll + 20).min(max_scroll);
                }
                if i.key_pressed(Key::PageUp) {
                    self.app.viewer_scroll = self.app.viewer_scroll.saturating_sub(20);
                }
                if i.key_pressed(Key::Home) || (i.key_pressed(Key::G) && !i.modifiers.shift) {
                    self.app.viewer_scroll = 0;
                }
                if i.key_pressed(Key::End) || (i.key_pressed(Key::G) && i.modifiers.shift) {
                    self.app.viewer_scroll = self.app.viewer_content.len().saturating_sub(1);
                }
            }
        });
    }

    fn render_tabs(&mut self, ui: &mut egui::Ui) {
        // Track actions for burger menu (to avoid borrow issues)
        let mut action_open_config = false;
        let mut action_open_folder = false;
        let mut action_quit = false;

        ui.horizontal(|ui| {
            for (i, title) in TuiMode::titles().iter().enumerate() {
                let is_selected = self.app.mode.index() == i;

                // Create a tab-like appearance with frame
                let bg_color = if is_selected {
                    Color32::from_rgb(50, 50, 70)
                } else {
                    Color32::from_rgb(35, 35, 45)
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

                        if ui.add(egui::Label::new(text).sense(egui::Sense::click())).clicked() {
                            self.app.save_current_tab_state();
                            self.app.mode = TuiMode::from_index(i);
                            self.app.selected.clear();
                            self.app.refresh_files();
                            self.app.restore_tab_state();
                        }
                    });

                // Change cursor to pointer when hovering tabs
                if frame_response.response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                ui.add_space(2.0);
            }

            // Push the burger menu to the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Burger menu button
                let burger_id = ui.make_persistent_id("burger_menu");
                let burger_response = ui.add(
                    egui::Button::new(RichText::new("☰").size(18.0).color(Colors::DARK_GRAY))
                        .frame(false)
                );

                if burger_response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                // Open menu on left click
                if burger_response.clicked() {
                    ui.memory_mut(|mem| mem.toggle_popup(burger_id));
                }

                // Show popup menu below the button
                egui::popup_below_widget(ui, burger_id, &burger_response, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                    ui.set_min_width(180.0);
                    if ui.button("📁 Open Config File").clicked() {
                        action_open_config = true;
                        ui.memory_mut(|mem| mem.close_popup());
                    }
                    if ui.button("📂 Open Backup Folder").clicked() {
                        action_open_folder = true;
                        ui.memory_mut(|mem| mem.close_popup());
                    }
                    ui.separator();
                    if ui.button("🚪 Quit").clicked() {
                        action_quit = true;
                        ui.memory_mut(|mem| mem.close_popup());
                    }
                });
            });
        });

        // Handle burger menu actions
        if action_open_config {
            let _ = open::that(&self.app.config_path);
        }
        if action_open_folder {
            let _ = open::that(&self.app.data_dir);
        }
        if action_quit {
            self.app.should_quit = true;
        }
    }

    fn render_status_tab(&mut self, ui: &mut egui::Ui) {
        // Title
        ui.label(RichText::new("Your Tracked Files - Shows backup status and changes").color(Colors::DARK_GRAY));
        ui.add_space(5.0);

        // Collect items to display (avoid borrowing issues)
        let items: Vec<_> = self.app.files.iter().enumerate().map(|(i, file)| {
            let is_selected = self.app.list_state.selected() == Some(i);
            let is_multi_selected = self.app.selected.contains(&i);
            let expanded = self.app.expanded_folders.contains(&file.path);
            (i, file.clone(), is_selected, is_multi_selected, expanded)
        }).collect();

        let _selected_idx = self.app.list_state.selected();

        // Track which item was clicked/right-clicked
        let mut new_selection: Option<usize> = None;
        let mut double_clicked_folder: Option<usize> = None;

        // Actions to perform after the loop (to avoid borrow issues)
        let _action_toggle_folder = false;
        let mut action_open_viewer = false;
        let mut action_backup = false;
        let mut action_remove_tracking = false;
        let mut action_expand_folder = false;
        let mut action_collapse_folder = false;
        let mut action_toggle_encryption = false;
        let mut action_toggle_backup_mode = false;

        ScrollArea::vertical()
            .id_salt("status_tab_scroll")
            .show(ui, |ui| {
                for (i, file, is_selected, is_multi_selected, expanded) in &items {
                    let bg_color = if *is_selected {
                        Colors::SELECTION_BG
                    } else {
                        Color32::TRANSPARENT
                    };

                    // Allocate space for the row
                    let (row_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 20.0),
                        egui::Sense::hover(),
                    );

                    // Interact with the full row rect (must use unique ID)
                    let row_response = ui.interact(
                        row_rect,
                        egui::Id::new(("status_row", *i)),
                        egui::Sense::click(),
                    );

                    // Draw background
                    if *is_selected || row_response.hovered() {
                        let color = if *is_selected { bg_color } else { Color32::from_rgb(45, 45, 55) };
                        ui.painter().rect_filled(row_rect, 0.0, color);
                    }

                    // Draw the row content using painter (non-interactive)
                    let mut x = row_rect.left() + 4.0;
                    let y = row_rect.center().y;
                    let font = egui::FontId::monospace(13.0);

                    // Selection marker
                    let marker = if *is_multi_selected { "*" } else { " " };
                    ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, marker, font.clone(), Colors::CYAN);
                    x += 12.0;

                    if file.is_folder_node {
                        // Folder row
                        let expand_icon = if *expanded { "▼" } else { "▶" };

                        // Status indicator
                        let (status, color) = if file.modified_count > 0 {
                            ("M", Colors::YELLOW)
                        } else if file.new_count > 0 {
                            ("+", Colors::CYAN)
                        } else {
                            (" ", Colors::GREEN)
                        };
                        ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, status, font.clone(), color);
                        x += 14.0;
                        ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, expand_icon, font.clone(), Colors::BLUE);
                        x += 16.0;

                        // Encryption indicator for folders
                        if file.encrypted {
                            ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, "[E] ", font.clone(), Colors::MAGENTA);
                            x += 32.0;
                        }

                        ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, &file.display_path, font.clone(), Colors::BLUE);
                        x += file.display_path.len() as f32 * 8.0 + 8.0;

                        // Stats
                        let stats = format!(
                            "({} files{}{})",
                            file.child_count,
                            if file.modified_count > 0 { format!(", {} modified", file.modified_count) } else { String::new() },
                            if file.new_count > 0 { format!(", {} new", file.new_count) } else { String::new() }
                        );
                        ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, stats, font.clone(), Colors::DARK_GRAY);
                    } else {
                        // File row
                        let status_color = file.status.color();
                        let status_symbol = file.status.symbol();

                        ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, status_symbol, font.clone(), egui_color(status_color));
                        x += 14.0;

                        // Indentation
                        let indent_width = file.depth as f32 * 32.0;
                        x += indent_width;

                        // Mode indicator
                        let mode_str = match file.backup_mode {
                            Some(BackupMode::Archive) => "[A]",
                            Some(BackupMode::Incremental) => "[I]",
                            None => "   ",
                        };
                        ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, mode_str, font.clone(), Colors::BLUE);
                        x += 28.0;

                        // Encryption indicator
                        let enc_str = if file.encrypted { "[E]" } else { "   " };
                        let enc_color = if file.encrypted { Colors::MAGENTA } else { Colors::DARK_GRAY };
                        ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, enc_str, font.clone(), enc_color);
                        x += 28.0;

                        let file_color = if file.is_tracked { Colors::WHITE } else { Colors::DARK_GRAY };
                        ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, &file.display_path, font.clone(), file_color);
                        x += file.display_path.len() as f32 * 8.0 + 8.0;

                        // Size
                        if let Some(size) = file.size {
                            let size_str = format!("  {}", format_size(size));
                            ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, size_str, font.clone(), Colors::DARK_GRAY);
                        }
                    }

                    // Handle clicks on this row
                    if row_response.clicked() {
                        new_selection = Some(*i);
                    }
                    if row_response.double_clicked() && file.is_folder_node {
                        double_clicked_folder = Some(*i);
                    }

                    // Right-click selects and shows context menu
                    if row_response.secondary_clicked() {
                        new_selection = Some(*i);
                    }

                    // Context menu (must be called in same scope as response)
                    row_response.context_menu(|ui| {
                        if file.is_folder_node {
                            if *expanded {
                                if ui.button("Collapse Folder").clicked() {
                                    action_collapse_folder = true;
                                    ui.close_menu();
                                }
                            } else {
                                if ui.button("Expand Folder").clicked() {
                                    action_expand_folder = true;
                                    ui.close_menu();
                                }
                            }
                        } else {
                            if ui.button("View File").clicked() {
                                action_open_viewer = true;
                                ui.close_menu();
                            }
                            if ui.button("Backup Now").clicked() {
                                action_backup = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            let enc_label = if file.encrypted { "Disable Encryption" } else { "Enable Encryption" };
                            if ui.button(enc_label).clicked() {
                                action_toggle_encryption = true;
                                ui.close_menu();
                            }
                            let mode_label = match file.backup_mode {
                                Some(BackupMode::Archive) => "Switch to Incremental",
                                _ => "Switch to Archive",
                            };
                            if ui.button(mode_label).clicked() {
                                action_toggle_backup_mode = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Remove from Tracking").clicked() {
                                action_remove_tracking = true;
                                ui.close_menu();
                            }
                        }
                    });
                }
            });

        // Apply selection change
        if let Some(i) = new_selection {
            self.app.list_state.select(Some(i));
        }

        // Handle double-click on folder
        if let Some(i) = double_clicked_folder {
            self.app.list_state.select(Some(i));
            self.app.toggle_folder();
        }

        // Handle context menu actions
        if action_collapse_folder {
            self.app.collapse_folder();
        }
        if action_expand_folder {
            self.app.expand_folder();
        }
        if action_open_viewer {
            self.app.open_viewer();
        }
        if action_backup {
            self.app.perform_backup(None);
        }
        if action_toggle_encryption {
            self.app.toggle_encryption();
        }
        if action_toggle_backup_mode {
            self.app.toggle_backup_mode();
        }
        if action_remove_tracking {
            self.app.toggle_tracking();
        }
    }

    fn render_add_tab(&mut self, ui: &mut egui::Ui) {
        // Navigation toolbar
        ui.horizontal(|ui| {
            // Back button - always works regardless of selection
            if ui.button("⬅ Back").clicked() {
                self.app.selected.clear(); // Clear selection when navigating
                self.app.parent_directory();
            }

            // Home button
            if ui.button("🏠 Home").clicked() {
                self.app.selected.clear();
                self.app.home_directory();
            }

            ui.separator();

            // Path display
            let path_display = if let Some(home) = dirs::home_dir() {
                if let Ok(rel) = self.app.browse_dir.strip_prefix(&home) {
                    format!("~/{}", rel.display())
                } else {
                    self.app.browse_dir.display().to_string()
                }
            } else {
                self.app.browse_dir.display().to_string()
            };
            ui.label(RichText::new(&path_display).color(Colors::CYAN).strong());
        });
        ui.add_space(5.0);

        // Collect items to avoid borrowing issues
        let items: Vec<_> = self.app.files.iter().enumerate().map(|(i, file)| {
            let is_selected = self.app.list_state.selected() == Some(i);
            let is_multi_selected = self.app.selected.contains(&i);
            (i, file.clone(), is_selected, is_multi_selected)
        }).collect();

        let _selected_idx = self.app.list_state.selected();

        // Track which item was clicked/right-clicked
        let mut new_selection: Option<usize> = None;
        let mut double_clicked_item: Option<usize> = None;

        // Actions to perform after the loop
        let mut action_enter_dir = false;
        let mut action_add_folder_pattern = false;
        let mut action_recursive_preview = false;
        let mut action_remove_tracking = false;
        let mut action_add_tracking = false;
        let mut action_open_viewer = false;

        ScrollArea::vertical()
            .id_salt("add_tab_scroll")
            .show(ui, |ui| {
                for (i, file, is_selected, is_multi_selected) in &items {
                    let bg_color = if *is_selected {
                        Colors::SELECTION_BG
                    } else {
                        Color32::TRANSPARENT
                    };

                    // Allocate space for the row
                    let (row_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 20.0),
                        egui::Sense::hover(),
                    );

                    // Interact with the full row rect (must use unique ID)
                    let row_response = ui.interact(
                        row_rect,
                        egui::Id::new(("add_row", *i)),
                        egui::Sense::click(),
                    );

                    // Draw background
                    if *is_selected || row_response.hovered() {
                        let color = if *is_selected { bg_color } else { Color32::from_rgb(45, 45, 55) };
                        ui.painter().rect_filled(row_rect, 0.0, color);
                    }

                    // Draw the row content using painter (non-interactive)
                    let mut x = row_rect.left() + 4.0;
                    let y = row_rect.center().y;
                    let font = egui::FontId::monospace(13.0);

                    let marker = if *is_multi_selected { "*" } else { " " };
                    ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, marker, font.clone(), Colors::CYAN);
                    x += 12.0;

                    let icon = if file.is_dir { "/" } else { " " };
                    ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, icon, font.clone(), Colors::BLUE);
                    x += 12.0;

                    let color = if file.is_dir {
                        Colors::BLUE
                    } else if file.is_tracked {
                        Colors::GREEN
                    } else {
                        Colors::WHITE
                    };

                    ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, &file.display_path, font.clone(), color);
                    x += file.display_path.len() as f32 * 8.0 + 4.0;

                    if file.is_tracked {
                        ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, " [tracked]", font.clone(), Colors::GREEN);
                        x += 80.0;
                    }

                    if let Some(size) = file.size {
                        let size_str = format!("  {}", format_size(size));
                        ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, size_str, font.clone(), Colors::DARK_GRAY);
                    }

                    // Handle clicks on this row
                    if row_response.clicked() {
                        new_selection = Some(*i);
                    }
                    if row_response.double_clicked() {
                        double_clicked_item = Some(*i);
                    }
                    if row_response.secondary_clicked() {
                        new_selection = Some(*i);
                    }

                    // Context menu (must be in same scope as response)
                    row_response.context_menu(|ui| {
                        if file.is_dir {
                            if ui.button("Open Folder").clicked() {
                                action_enter_dir = true;
                                ui.close_menu();
                            }
                            if ui.button("Add Folder Pattern (/**)").clicked() {
                                action_add_folder_pattern = true;
                                ui.close_menu();
                            }
                            if ui.button("Recursive Add Preview").clicked() {
                                action_recursive_preview = true;
                                ui.close_menu();
                            }
                        } else {
                            if file.is_tracked {
                                if ui.button("Remove from Tracking").clicked() {
                                    action_remove_tracking = true;
                                    ui.close_menu();
                                }
                            } else {
                                if ui.button("Add to Tracking").clicked() {
                                    action_add_tracking = true;
                                    ui.close_menu();
                                }
                            }
                            ui.separator();
                            if ui.button("View File").clicked() {
                                action_open_viewer = true;
                                ui.close_menu();
                            }
                        }
                    });
                }
            });

        // Apply selection change
        if let Some(i) = new_selection {
            self.app.list_state.select(Some(i));
        }

        // Handle double-click (opens folders or adds files)
        if let Some(i) = double_clicked_item {
            self.app.list_state.select(Some(i));
            if i < self.app.files.len() {
                if self.app.files[i].is_dir {
                    self.app.selected.clear();
                    self.app.enter_directory();
                } else {
                    self.app.toggle_tracking();
                }
            }
        }

        // Handle context menu actions
        if action_enter_dir {
            self.app.enter_directory();
        }
        if action_add_folder_pattern {
            self.app.add_folder_pattern();
        }
        if action_recursive_preview {
            self.app.start_recursive_preview();
        }
        if action_remove_tracking {
            self.app.remove_from_tracking_in_browser();
        }
        if action_add_tracking {
            self.app.toggle_tracking();
        }
        if action_open_viewer {
            self.app.open_viewer();
        }
    }

    fn render_restore_tab(&mut self, ui: &mut egui::Ui) {
        match self.app.restore_view {
            RestoreView::Commits => {
                ui.label(RichText::new("Backup History - Select a backup to restore from (Enter to select)").color(Colors::DARK_GRAY));
                ui.add_space(5.0);

                let items: Vec<_> = self.app.commits.iter().enumerate().map(|(i, commit)| {
                    let is_selected = self.app.restore_list_state.selected() == Some(i);
                    let is_multi_selected = self.app.selected.contains(&i);
                    (i, commit.clone(), is_selected, is_multi_selected)
                }).collect();

                let _selected_idx = self.app.restore_list_state.selected();

                // Track which item was clicked/right-clicked
                let mut new_selection: Option<usize> = None;
                let mut double_clicked_item: Option<usize> = None;
                let mut action_select_commit = false;

                ScrollArea::vertical()
                    .id_salt("restore_commits_scroll")
                    .show(ui, |ui| {
                        for (i, commit, is_selected, is_multi_selected) in &items {
                            let bg_color = if *is_selected {
                                Colors::SELECTION_BG
                            } else {
                                Color32::TRANSPARENT
                            };

                            // Allocate space for the row
                            let (row_rect, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 20.0),
                                egui::Sense::hover(),
                            );

                            // Interact with the full row rect (must use unique ID)
                            let row_response = ui.interact(
                                row_rect,
                                egui::Id::new(("restore_commit_row", *i)),
                                egui::Sense::click(),
                            );

                            // Draw background
                            if *is_selected || row_response.hovered() {
                                let color = if *is_selected { bg_color } else { Color32::from_rgb(45, 45, 55) };
                                ui.painter().rect_filled(row_rect, 0.0, color);
                            }

                            // Draw the row content using painter (non-interactive)
                            let mut x = row_rect.left() + 4.0;
                            let y = row_rect.center().y;
                            let font = egui::FontId::monospace(13.0);

                            let marker = if *is_multi_selected { "*" } else { " " };
                            ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, marker, font.clone(), Colors::CYAN);
                            x += 12.0;

                            ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, &commit.short_hash, font.clone(), Colors::YELLOW);
                            x += 70.0;

                            let date_short = if commit.date.len() > 19 {
                                &commit.date[..19]
                            } else {
                                &commit.date
                            };
                            ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, date_short, font.clone(), Colors::CYAN);
                            x += 160.0;

                            ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, &commit.message, font.clone(), Colors::WHITE);

                            // Handle clicks on this row
                            if row_response.clicked() {
                                new_selection = Some(*i);
                            }
                            if row_response.double_clicked() {
                                double_clicked_item = Some(*i);
                            }
                            if row_response.secondary_clicked() {
                                new_selection = Some(*i);
                            }

                            // Context menu (must be in same scope as response)
                            row_response.context_menu(|ui| {
                                if ui.button("Select This Backup").clicked() {
                                    action_select_commit = true;
                                    ui.close_menu();
                                }
                            });
                        }
                    });

                // Apply selection change
                if let Some(i) = new_selection {
                    self.app.restore_list_state.select(Some(i));
                }

                // Handle double-click to select commit immediately
                if let Some(i) = double_clicked_item {
                    self.app.restore_list_state.select(Some(i));
                    self.app.select_commit();
                }

                // Handle context menu action
                if action_select_commit {
                    self.app.select_commit();
                }
            }
            RestoreView::Files => {
                // Navigation toolbar for restore files view
                ui.horizontal(|ui| {
                    if ui.button("⬅ Back to Backups").clicked() {
                        self.app.back_to_commits();
                    }

                    ui.separator();

                    let commit_info = self.app.selected_commit
                        .and_then(|i| self.app.commits.get(i))
                        .map(|c| format!("{} - {}", c.short_hash, c.message))
                        .unwrap_or_else(|| "Unknown".to_string());
                    ui.label(RichText::new(&commit_info).color(Colors::CYAN).strong());
                });
                ui.add_space(5.0);

                let items: Vec<_> = self.app.restore_files.iter().enumerate().map(|(i, file)| {
                    let is_selected = self.app.restore_list_state.selected() == Some(i);
                    let is_multi_selected = self.app.selected.contains(&i);
                    (i, file.clone(), is_selected, is_multi_selected)
                }).collect();

                let _selected_idx = self.app.restore_list_state.selected();

                // Track which item was clicked/right-clicked
                let mut new_selection: Option<usize> = None;
                let mut action_restore = false;
                let mut action_view = false;
                let mut action_back = false;

                ScrollArea::vertical()
                    .id_salt("restore_files_scroll")
                    .show(ui, |ui| {
                        for (i, file, is_selected, is_multi_selected) in &items {
                            let bg_color = if *is_selected {
                                Colors::SELECTION_BG
                            } else {
                                Color32::TRANSPARENT
                            };

                            // Allocate space for the row
                            let (row_rect, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 20.0),
                                egui::Sense::hover(),
                            );

                            // Interact with the full row rect (must use unique ID)
                            let row_response = ui.interact(
                                row_rect,
                                egui::Id::new(("restore_file_row", *i)),
                                egui::Sense::click(),
                            );

                            // Draw background
                            if *is_selected || row_response.hovered() {
                                let color = if *is_selected { bg_color } else { Color32::from_rgb(45, 45, 55) };
                                ui.painter().rect_filled(row_rect, 0.0, color);
                            }

                            // Draw the row content using painter (non-interactive)
                            let mut x = row_rect.left() + 4.0;
                            let y = row_rect.center().y;
                            let font = egui::FontId::monospace(13.0);

                            let marker = if *is_multi_selected { "*" } else { " " };
                            ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, marker, font.clone(), Colors::CYAN);
                            x += 12.0;

                            let (status, color) = if !file.exists_locally {
                                ("NEW", Colors::CYAN)
                            } else if file.local_differs {
                                ("CHG", Colors::YELLOW)
                            } else {
                                ("OK ", Colors::GREEN)
                            };
                            ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, status, font.clone(), color);
                            x += 35.0;

                            let size_str = format_size(file.size);
                            ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, &size_str, font.clone(), Colors::DARK_GRAY);
                            x += 70.0;

                            let file_color = if file.local_differs { Colors::WHITE } else { Colors::DARK_GRAY };
                            ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, &file.display_path, font.clone(), file_color);

                            // Handle clicks on this row
                            if row_response.clicked() {
                                new_selection = Some(*i);
                            }
                            if row_response.secondary_clicked() {
                                new_selection = Some(*i);
                            }

                            // Context menu (must be in same scope as response)
                            row_response.context_menu(|ui| {
                                if ui.button("Restore File").clicked() {
                                    action_restore = true;
                                    ui.close_menu();
                                }
                                if ui.button("View File").clicked() {
                                    action_view = true;
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui.button("Back to Backups").clicked() {
                                    action_back = true;
                                    ui.close_menu();
                                }
                            });
                        }
                    });

                // Apply selection change
                if let Some(i) = new_selection {
                    self.app.restore_list_state.select(Some(i));
                }

                // Handle context menu actions
                if action_restore {
                    self.app.perform_restore();
                }
                if action_view {
                    self.app.open_viewer();
                }
                if action_back {
                    self.app.back_to_commits();
                }
            }
        }
    }

    fn render_recursive_preview(&mut self, ui: &mut egui::Ui) {
        let (source_display, selected_count, _total_count, gitignore_excluded, config_excluded, preview_selected_idx) = {
            let preview = match &self.app.recursive_preview {
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

            (source_display, preview.selected_files.len(), preview.preview_files.len(),
             preview.gitignore_excluded, preview.config_excluded, preview.preview_list_state.selected())
        };

        // Header
        ui.horizontal(|ui| {
            ui.label("Adding recursively: ");
            ui.label(RichText::new(&source_display).color(Colors::YELLOW).strong());
        });

        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}", selected_count)).color(Colors::GREEN));
            ui.label(" files selected | ");
            ui.label(RichText::new(format!("{}", gitignore_excluded)).color(Colors::DARK_GRAY));
            ui.label(" excluded by .gitignore | ");
            ui.label(RichText::new(format!("{}", config_excluded)).color(Colors::DARK_GRAY));
            ui.label(" excluded by config");
        });

        ui.horizontal(|ui| {
            ui.label(RichText::new("Space").color(Colors::CYAN));
            ui.label(": toggle | ");
            ui.label(RichText::new("Ctrl+A").color(Colors::CYAN));
            ui.label(": select all | ");
            ui.label(RichText::new("Enter").color(Colors::GREEN));
            ui.label(": add selected | ");
            ui.label(RichText::new("Esc").color(Colors::RED));
            ui.label(": cancel");
        });

        ui.separator();

        // Collect items
        let items: Vec<_> = if let Some(preview) = &self.app.recursive_preview {
            preview.preview_files.iter().enumerate().map(|(i, file)| {
                let is_selected = preview.preview_list_state.selected() == Some(i);
                let is_checked = preview.selected_files.contains(&i);
                (i, file.display_path.clone(), file.size, is_selected, is_checked)
            }).collect()
        } else {
            Vec::new()
        };

        ScrollArea::vertical()
            .id_salt("recursive_preview_scroll")
            .show(ui, |ui| {
                for (i, display_path, size, is_selected, is_checked) in &items {
                    let bg_color = if *is_selected {
                        Colors::SELECTION_BG
                    } else {
                        Color32::TRANSPARENT
                    };

                    let frame_response = egui::Frame::none()
                        .fill(bg_color)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let marker = if *is_checked { "[x]" } else { "[ ]" };
                                let marker_color = if *is_checked { Colors::GREEN } else { Colors::DARK_GRAY };
                                ui.label(RichText::new(marker).color(marker_color).monospace());

                                ui.label(RichText::new(format_size(*size)).color(Colors::DARK_GRAY).monospace());

                                let text_color = if *is_checked { Colors::WHITE } else { Colors::DARK_GRAY };
                                ui.label(RichText::new(display_path).color(text_color).monospace());
                            });
                        });

                    // Scroll to selected item
                    if preview_selected_idx == Some(*i) {
                        frame_response.response.scroll_to_me(Some(egui::Align::Center));
                    }
                }
            });
    }

    fn render_add_input_dialog(&mut self, ctx: &egui::Context) {
        egui::Window::new("Add files/folders to backup")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Enter a path to add to your backup:");
                ui.add_space(10.0);

                let response = ui.add(
                    TextEdit::singleline(&mut self.app.add_input)
                        .desired_width(400.0)
                        .hint_text("e.g., ~/.bashrc or ~/.config/nvim/**")
                );

                self.text_input_focus = response.has_focus();
                response.request_focus();

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Add").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))) {
                        if !self.app.add_input.is_empty() {
                            let pattern = self.app.add_input.clone();
                            self.app.config.tracked_files.push(TrackedPattern::simple(&pattern));
                            self.app.config_dirty = true;
                            self.app.message = Some(format!("Added: {} (saves on exit)", pattern));
                            self.app.add_input.clear();
                            self.app.refresh_files();
                        }
                        self.app.add_mode = false;
                        self.text_input_focus = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.app.add_input.clear();
                        self.app.add_mode = false;
                        self.text_input_focus = false;
                    }
                });

                ui.add_space(10.0);
                ui.label(RichText::new("Hints:").color(Colors::DARK_GRAY));
                ui.label(RichText::new("  ~/.bashrc             Add a single file").color(Colors::DARK_GRAY));
                ui.label(RichText::new("  ~/.config/nvim/**     Add all files in folder (recursive)").color(Colors::DARK_GRAY));
                ui.label(RichText::new("  ~/.config/nvim/*      Add files in folder (not recursive)").color(Colors::DARK_GRAY));
            });
    }

    fn render_backup_message_dialog(&mut self, ctx: &egui::Context) {
        egui::Window::new("Backup commit message")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Enter a commit message for this backup:");
                ui.add_space(10.0);

                let response = ui.add(
                    TextEdit::singleline(&mut self.app.backup_message_input)
                        .desired_width(400.0)
                        .hint_text("Leave empty for auto-generated timestamp")
                );

                self.text_input_focus = response.has_focus();
                response.request_focus();

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Backup").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))) {
                        let msg = if self.app.backup_message_input.is_empty() {
                            None
                        } else {
                            Some(self.app.backup_message_input.clone())
                        };
                        self.app.backup_message_input.clear();
                        self.app.backup_message_mode = false;
                        self.text_input_focus = false;
                        self.app.perform_backup(msg);
                    }
                    if ui.button("Cancel").clicked() {
                        self.app.backup_message_input.clear();
                        self.app.backup_message_mode = false;
                        self.text_input_focus = false;
                        self.app.message = Some("Backup cancelled".to_string());
                    }
                });
            });
    }

    fn render_password_dialog(&mut self, ctx: &egui::Context) {
        let title = match self.app.password_purpose {
            PasswordPurpose::Backup => "Enter Encryption Password (Backup)",
            PasswordPurpose::Restore => "Enter Encryption Password (Restore)",
        };

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Enter the password for encrypted files:");
                ui.add_space(10.0);

                let response = ui.add(
                    TextEdit::singleline(&mut self.app.password_input)
                        .password(true)
                        .desired_width(300.0)
                        .hint_text("Password")
                );

                self.text_input_focus = response.has_focus();
                response.request_focus();

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Confirm").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))) {
                        self.app.confirm_password();
                        self.text_input_focus = false;
                        // Continue with the operation that needed the password
                        match self.app.password_purpose {
                            PasswordPurpose::Backup => {
                                let msg = self.app.pending_backup_message.take();
                                self.app.perform_backup(msg);
                            }
                            PasswordPurpose::Restore => {
                                self.app.perform_restore();
                            }
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        self.app.cancel_password();
                        self.app.pending_backup_message = None;
                        self.text_input_focus = false;
                        self.app.message = Some("Password required for encrypted files".to_string());
                    }
                });

                ui.add_space(5.0);
                ui.label(RichText::new("Files marked with encrypted=true require a password").color(Colors::DARK_GRAY).small());
            });
    }

    fn render_remote_dialog(&mut self, ctx: &egui::Context) {
        egui::Window::new("Git Remote URL")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Enter the URL of your git remote:");
                ui.add_space(10.0);

                let response = ui.add(
                    TextEdit::singleline(&mut self.app.remote_url_input)
                        .desired_width(400.0)
                        .hint_text("https://github.com/user/dotfiles.git")
                );

                self.text_input_focus = response.has_focus();
                response.request_focus();

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))) {
                        self.app.confirm_remote_url();
                        self.text_input_focus = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.app.cancel_remote_dialog();
                        self.text_input_focus = false;
                    }
                });

                ui.add_space(5.0);
                ui.label(RichText::new("Leave empty to remove remote configuration").color(Colors::DARK_GRAY).small());
            });
    }

    fn render_help_overlay(&mut self, ctx: &egui::Context) {
        egui::Window::new("Help & About")
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_size([600.0, 500.0])
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    // ABOUT section
                    ui.label(RichText::new("ABOUT DOT MATRIX").color(Colors::YELLOW).strong());
                    ui.horizontal(|ui| {
                        ui.label("Version:");
                        ui.label(RichText::new(env!("CARGO_PKG_VERSION")).color(Colors::CYAN));
                    });
                    ui.label("Dotfile management and versioning tool");
                    ui.label("Track configuration files in-place with git-based versioning.");
                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        ui.label("GitHub:");
                        ui.hyperlink_to("github.com/Woofson/dotmatrix", "https://github.com/Woofson/dotmatrix");
                    });
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);

                    ui.label(RichText::new("WHAT EACH TAB DOES").color(Colors::YELLOW).strong());
                    ui.label("Tracked Files - View files you're backing up and their status");
                    ui.label("Add Files - Browse your computer to add files to backup");
                    ui.label("Restore - Recover files from previous backups");
                    ui.add_space(10.0);

                    ui.label(RichText::new("STATUS SYMBOLS").color(Colors::YELLOW).strong());
                    ui.horizontal(|ui| { ui.label(RichText::new("(space)").color(Colors::GREEN)); ui.label("= Backed up and unchanged"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("M").color(Colors::YELLOW)); ui.label("= Modified since last backup"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("+").color(Colors::CYAN)); ui.label("= New, not yet backed up"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("-").color(Colors::DARK_GRAY)); ui.label("= Deleted from your system"); });
                    ui.add_space(10.0);

                    ui.label(RichText::new("FILE INDICATORS").color(Colors::YELLOW).strong());
                    ui.horizontal(|ui| { ui.label(RichText::new("[I]").color(Colors::BLUE)); ui.label("= Incremental backup (content-addressed)"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("[A]").color(Colors::BLUE)); ui.label("= Archive backup (compressed tarball)"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("[E]").color(Colors::MAGENTA)); ui.label("= Encrypted (requires password)"); });
                    ui.add_space(10.0);

                    ui.label(RichText::new("KEYBOARD SHORTCUTS").color(Colors::YELLOW).strong());
                    ui.horizontal(|ui| { ui.label(RichText::new("j/k").color(Colors::CYAN)); ui.label("Move down/up"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("Tab").color(Colors::CYAN)); ui.label("Switch tabs"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("Space").color(Colors::CYAN)); ui.label("Toggle selection"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("v").color(Colors::CYAN)); ui.label("View file contents"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("S").color(Colors::CYAN)); ui.label("Save and reload"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("?").color(Colors::CYAN)); ui.label("Show this help"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("q / Ctrl+Q").color(Colors::CYAN)); ui.label("Quit (saves changes)"); });
                    ui.add_space(10.0);

                    ui.label(RichText::new("TRACKED FILES TAB").color(Colors::YELLOW).strong());
                    ui.horizontal(|ui| { ui.label(RichText::new("b").color(Colors::CYAN)); ui.label("Run backup now"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("B").color(Colors::CYAN)); ui.label("Backup with custom message"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("d").color(Colors::CYAN)); ui.label("Stop tracking this file"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("Right/l").color(Colors::CYAN)); ui.label("Expand folder"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("Left/h").color(Colors::CYAN)); ui.label("Collapse folder"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("e").color(Colors::CYAN)); ui.label("Expand all folders"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("E").color(Colors::CYAN)); ui.label("Collapse all folders"); });
                    ui.add_space(10.0);

                    ui.label(RichText::new("ADD FILES TAB").color(Colors::YELLOW).strong());
                    ui.horizontal(|ui| { ui.label(RichText::new("Enter/l").color(Colors::CYAN)); ui.label("Open folder / Add file"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("A").color(Colors::CYAN)); ui.label("Add folder as pattern (/**)"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("R").color(Colors::CYAN)); ui.label("Recursive add preview"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("Backspace/h").color(Colors::CYAN)); ui.label("Go to parent directory"); });
                    ui.horizontal(|ui| { ui.label(RichText::new("a").color(Colors::CYAN)); ui.label("Type a path manually"); });
                    ui.add_space(10.0);

                    if ui.button("Close (Esc)").clicked() {
                        self.app.show_help = false;
                    }
                });
            });
    }

    fn render_viewer_overlay(&mut self, ctx: &egui::Context) {
        let title = self.app.viewer_title.clone();
        let total_lines = self.app.viewer_content.len();
        let scroll_pos = if total_lines > 0 {
            format!("{}/{}", self.app.viewer_scroll + 1, total_lines)
        } else {
            "0/0".to_string()
        };

        // Clone the content we need
        let content: Vec<_> = self.app.viewer_content.iter().cloned().collect();

        egui::Window::new(title)
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_size([800.0, 600.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("q: close | j/k: scroll | g/G: top/bottom").color(Colors::DARK_GRAY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(&scroll_pos).color(Colors::DARK_GRAY));
                    });
                });
                ui.separator();

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for line in &content {
                            if line.file_header {
                                let text = line.spans.first()
                                    .map(|(t, _)| t.clone())
                                    .unwrap_or_default();
                                ui.label(RichText::new(text).color(Colors::CYAN).strong().monospace());
                            } else {
                                ui.horizontal(|ui| {
                                    for (text, style) in &line.spans {
                                        let color = egui_color(style.fg.unwrap_or(ratatui::style::Color::White));
                                        ui.label(RichText::new(text).color(color).monospace());
                                    }
                                });
                            }
                        }
                    });

                if ui.button("Close").clicked() {
                    self.app.close_viewer();
                }
            });
    }

    fn render_status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Left: app name and version
            ui.label(RichText::new("Dot Matrix").color(Colors::WHITE).strong());
            let version = env!("CARGO_PKG_VERSION");
            ui.label(RichText::new(format!("v{}", version)).color(Colors::DARK_GRAY));
            ui.separator();

            // Mode-specific action buttons (unless busy)
            if !self.app.busy {
                match self.app.mode {
                    TuiMode::Status => {
                        if ui.button("Backup").clicked() {
                            self.app.perform_backup(None);
                        }
                        if ui.button("Remove").clicked() {
                            self.app.toggle_tracking();
                        }
                        if ui.button("View").clicked() {
                            self.app.open_viewer();
                        }
                        ui.separator();
                        // Git sync buttons
                        if self.app.config.git_remote_url.is_some() {
                            if ui.button("Pull").clicked() {
                                match self.app.git_pull() {
                                    Ok(msg) => self.app.message = Some(msg),
                                    Err(e) => self.app.message = Some(format!("Pull failed: {}", e)),
                                }
                            }
                            if ui.button("Push").clicked() {
                                match self.app.git_push() {
                                    Ok(msg) => self.app.message = Some(msg),
                                    Err(e) => self.app.message = Some(format!("Push failed: {}", e)),
                                }
                            }
                        } else {
                            if ui.button("Set Remote...").clicked() {
                                self.app.show_remote_dialog();
                            }
                        }
                    }
                    TuiMode::Add => {
                        if ui.button("Add").clicked() {
                            self.app.toggle_tracking();
                        }
                        if ui.button("Add Folder").clicked() {
                            self.app.add_folder_pattern();
                        }
                        if ui.button("Recursive...").clicked() {
                            self.app.start_recursive_preview();
                        }
                    }
                    TuiMode::Browse => {
                        match self.app.restore_view {
                            RestoreView::Commits => {
                                if ui.button("Select").clicked() {
                                    self.app.select_commit();
                                }
                            }
                            RestoreView::Files => {
                                if ui.button("Restore").clicked() {
                                    self.app.perform_restore();
                                }
                                if ui.button("Back").clicked() {
                                    self.app.back_to_commits();
                                }
                            }
                        }
                    }
                }

                // Save button (always available, shows * when dirty)
                ui.separator();
                let save_label = if self.app.config_dirty || self.app.index_dirty {
                    "Save*"
                } else {
                    "Save"
                };
                if ui.button(save_label).clicked() {
                    self.app.save_and_reload();
                }
            }

            ui.separator();

            // Show status message (busy spinner OR notification) in the middle
            if self.app.busy {
                let spinner = SPINNER_FRAMES[self.app.spinner_frame];
                ui.label(RichText::new(format!("{} {}", spinner, self.app.busy_message)).color(Colors::YELLOW));
            } else if let Some(ref msg) = self.app.message.clone() {
                ui.label(RichText::new(msg).color(Colors::CYAN));
            }

            // Right-aligned: item count, Help, Quit
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Quit").clicked() {
                    self.app.should_quit = true;
                }
                if ui.button("Help").clicked() {
                    self.app.show_help = true;
                }
                ui.separator();

                let (total, selected) = match self.app.mode {
                    TuiMode::Status => (self.app.files.len(), self.app.selected.len()),
                    TuiMode::Browse => match self.app.restore_view {
                        RestoreView::Commits => (self.app.commits.len(), self.app.selected.len()),
                        RestoreView::Files => (self.app.restore_files.len(), self.app.selected.len()),
                    },
                    TuiMode::Add => (self.app.files.len(), self.app.selected.len()),
                };

                if selected > 0 {
                    ui.label(RichText::new(format!("{} selected / {} items", selected, total)).color(Colors::CYAN));
                } else {
                    ui.label(RichText::new(format!("{} items", total)).color(Colors::DARK_GRAY));
                }
            });
        });
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle keyboard input
        self.handle_keyboard(ctx);

        // Poll for backup completion
        self.app.poll_backup();

        // Request repaint if busy (for spinner animation)
        if self.app.busy {
            ctx.request_repaint();
        }

        // Quit handling
        if self.app.should_quit {
            let _ = self.app.save_if_dirty();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Bottom panel FIRST (renders at bottom, must be declared before CentralPanel)
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            self.render_status_bar(ui);
        });

        // Top panel for tabs only (no heading - app name moved to status bar)
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            self.render_tabs(ui);
        });

        // Central panel fills remaining space
        egui::CentralPanel::default().show(ctx, |ui| {
            // Main content
            if self.app.add_sub_mode == AddSubMode::RecursivePreview {
                self.render_recursive_preview(ui);
            } else {
                match self.app.mode {
                    TuiMode::Status => self.render_status_tab(ui),
                    TuiMode::Add => self.render_add_tab(ui),
                    TuiMode::Browse => self.render_restore_tab(ui),
                }
            }
        });

        // Overlays/dialogs
        if self.app.show_help {
            self.render_help_overlay(ctx);
        }

        if self.app.viewer_visible {
            self.render_viewer_overlay(ctx);
        }

        if self.app.add_mode {
            self.render_add_input_dialog(ctx);
        }

        if self.app.backup_message_mode {
            self.render_backup_message_dialog(ctx);
        }

        if self.app.password_prompt_visible {
            self.render_password_dialog(ctx);
        }

        if self.app.remote_dialog_visible {
            self.render_remote_dialog(ctx);
        }
    }
}

/// Convert ratatui Color to egui Color32
fn egui_color(color: ratatui::style::Color) -> Color32 {
    match color {
        ratatui::style::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
        ratatui::style::Color::Green => Colors::GREEN,
        ratatui::style::Color::Yellow => Colors::YELLOW,
        ratatui::style::Color::Cyan => Colors::CYAN,
        ratatui::style::Color::Red => Colors::RED,
        ratatui::style::Color::Blue => Colors::BLUE,
        ratatui::style::Color::Magenta => Colors::MAGENTA,
        ratatui::style::Color::DarkGray | ratatui::style::Color::Gray => Colors::DARK_GRAY,
        ratatui::style::Color::White | ratatui::style::Color::Reset => Colors::WHITE,
        _ => Colors::WHITE,
    }
}

/// Run the GUI application
pub fn run(config: Config, index: Index, config_path: PathBuf, index_path: PathBuf, data_dir: PathBuf) -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Dot Matrix")
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([600.0, 400.0])
            .with_app_id("ac.arf.dev.dotmatrix"),
        ..Default::default()
    };

    eframe::run_native(
        "dotmatrix",
        options,
        Box::new(|_cc| {
            Ok(Box::new(GuiApp::new(config, index, config_path, index_path, data_dir)))
        }),
    ).map_err(|e| anyhow::anyhow!("GUI error: {}", e))?;

    Ok(())
}
