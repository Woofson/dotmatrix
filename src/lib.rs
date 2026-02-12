pub mod config;
pub mod index;
pub mod scanner;
pub mod tui;

use std::path::PathBuf;

/// Get the config directory path
/// - Linux: ~/.config/dotmatrix
/// - Windows: C:\Users\<User>\AppData\Roaming\dotmatrix
/// - macOS: ~/Library/Application Support/dotmatrix
pub fn get_config_dir() -> anyhow::Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
        .join("dotmatrix");
    Ok(config_dir)
}

/// Get the default data directory path (used when not configured)
/// - Linux: ~/.local/share/dotmatrix
/// - Windows: C:\Users\<User>\AppData\Local\dotmatrix
/// - macOS: ~/Library/Application Support/dotmatrix
pub fn get_default_data_dir() -> anyhow::Result<PathBuf> {
    let data_dir = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find data directory"))?
        .join("dotmatrix");
    Ok(data_dir)
}

/// Get the data directory path, using custom path from config if set
pub fn get_data_dir() -> anyhow::Result<PathBuf> {
    // Try to load config to check for custom data_dir
    let config_path = get_config_path()?;
    if config_path.exists() {
        if let Ok(config) = config::Config::load(&config_path) {
            if let Some(custom_dir) = config.get_data_dir() {
                return Ok(custom_dir);
            }
        }
    }
    get_default_data_dir()
}

/// Get the data directory path with explicit config (avoids re-loading config)
pub fn get_data_dir_with_config(config: &config::Config) -> anyhow::Result<PathBuf> {
    if let Some(custom_dir) = config.get_data_dir() {
        Ok(custom_dir)
    } else {
        get_default_data_dir()
    }
}

/// Get the config file path
pub fn get_config_path() -> anyhow::Result<PathBuf> {
    Ok(get_config_dir()?.join("config.toml"))
}

/// Get the index file path
pub fn get_index_path() -> anyhow::Result<PathBuf> {
    Ok(get_data_dir()?.join("index.json"))
}

/// Get the index file path with explicit config
pub fn get_index_path_with_config(config: &config::Config) -> anyhow::Result<PathBuf> {
    Ok(get_data_dir_with_config(config)?.join("index.json"))
}

/// Get the storage directory path (for incremental/content-addressed backups)
pub fn get_storage_path() -> anyhow::Result<PathBuf> {
    Ok(get_data_dir()?.join("storage"))
}

/// Get the storage directory path with explicit config
pub fn get_storage_path_with_config(config: &config::Config) -> anyhow::Result<PathBuf> {
    Ok(get_data_dir_with_config(config)?.join("storage"))
}

/// Get the archives directory path (for tarball backups)
pub fn get_archives_path() -> anyhow::Result<PathBuf> {
    Ok(get_data_dir()?.join("archives"))
}

/// Get the archives directory path with explicit config
pub fn get_archives_path_with_config(config: &config::Config) -> anyhow::Result<PathBuf> {
    Ok(get_data_dir_with_config(config)?.join("archives"))
}
