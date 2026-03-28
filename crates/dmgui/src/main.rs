//! dmgui - GUI for dotmatrix
//!
//! Graphical user interface built with egui.
//! For beginners and visual workflow users.
//! Simple and pragmatic - like bvckup2.

mod app;
mod keyboard;
mod state;
mod theme;
mod widgets;

use app::GuiApp;
use eframe::egui;
use widgets::{render_dialogs, render_file_viewer, render_main_content, render_status_bar, render_tabs};

fn main() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Dot Matrix"),
        ..Default::default()
    };

    eframe::run_native(
        "Dot Matrix",
        options,
        Box::new(|cc| {
            // Set dark theme
            cc.egui_ctx.set_visuals(egui::Visuals::dark());

            // Load custom fonts if needed
            let mut style = (*cc.egui_ctx.style()).clone();
            style.spacing.item_spacing = egui::vec2(8.0, 4.0);
            cc.egui_ctx.set_style(style);

            match GuiApp::new() {
                Ok(app) => Ok(Box::new(GuiAppWrapper { app })),
                Err(e) => {
                    eprintln!("Failed to initialize: {}", e);
                    std::process::exit(1);
                }
            }
        }),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run GUI: {}", e))
}

struct GuiAppWrapper {
    app: GuiApp,
}

impl eframe::App for GuiAppWrapper {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll for background operations
        self.app.poll_operation();

        // Request repaint if busy (for spinner animation)
        if self.app.busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        // Handle keyboard input
        keyboard::handle_keyboard(&mut self.app, ctx);

        // Check for quit
        if self.app.should_quit {
            self.app.save_state();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        // Top panel with tabs
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            render_tabs(&mut self.app, ui);
        });

        // Bottom panel with status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            render_status_bar(&mut self.app, ui);
        });

        // Central panel with main content
        egui::CentralPanel::default().show(ctx, |ui| {
            render_main_content(&mut self.app, ui);
        });

        // Render overlays (dialogs, viewer, help)
        render_dialogs(&mut self.app, ctx);
        render_file_viewer(&mut self.app, ctx);
    }
}
