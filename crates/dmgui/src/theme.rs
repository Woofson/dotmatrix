//! Theme and color constants for the GUI
//!
//! Matches the TUI color scheme for consistency.

use egui::Color32;

/// Color scheme matching TUI theme
pub struct Colors;

impl Colors {
    pub const GREEN: Color32 = Color32::from_rgb(0, 200, 0);
    pub const YELLOW: Color32 = Color32::from_rgb(230, 200, 0);
    pub const CYAN: Color32 = Color32::from_rgb(0, 200, 200);
    pub const RED: Color32 = Color32::from_rgb(200, 50, 50);
    pub const BLUE: Color32 = Color32::from_rgb(100, 150, 255);
    pub const DARK_GRAY: Color32 = Color32::from_rgb(128, 128, 128);
    pub const WHITE: Color32 = Color32::from_rgb(220, 220, 220);
    pub const MAGENTA: Color32 = Color32::from_rgb(200, 100, 200);
    pub const SELECTION_BG: Color32 = Color32::from_rgb(60, 60, 80);
    pub const HOVER_BG: Color32 = Color32::from_rgb(45, 45, 55);
    pub const TAB_SELECTED_BG: Color32 = Color32::from_rgb(50, 50, 70);
    pub const TAB_BG: Color32 = Color32::from_rgb(35, 35, 45);
}

/// Spinner frames for busy indicator
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Format a file size in human-readable form
pub fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.1} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.1} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.1} KB", size as f64 / KB as f64)
    } else {
        format!("{} B", size)
    }
}
