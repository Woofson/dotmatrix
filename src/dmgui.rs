// GUI-only entry point for dotmatrix
// On Windows, this binary runs without a console window

#![windows_subsystem = "windows"]

use dotmatrix::config::Config;
use dotmatrix::index::Index;
use dotmatrix::gui;

fn main() {
    if let Err(_e) = run() {
        // On Windows without console, errors aren't visible
        // Exit with error code
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let config_path = dotmatrix::get_config_path()?;
    let index_path = dotmatrix::get_index_path()?;
    let data_dir = dotmatrix::get_data_dir()?;

    // If no config exists, create default config
    let config = if config_path.exists() {
        Config::load(&config_path)?
    } else {
        // Create default config for first-time GUI users
        let config = Config::default();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        config.save(&config_path)?;
        config
    };

    let index = if index_path.exists() {
        Index::load(&index_path)?
    } else {
        Index::new()
    };

    // Ensure data directory exists
    std::fs::create_dir_all(&data_dir)?;

    gui::run(config, index, config_path, index_path, data_dir)
}
