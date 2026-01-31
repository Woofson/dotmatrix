pub mod config;
pub mod index;
pub mod scanner;
pub mod tui;

use std::path::PathBuf;

/// Get the config directory path (~/.config/dotmatrix)
pub fn get_config_dir() -> anyhow::Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
        .join("dotmatrix");
    Ok(config_dir)
}

/// Get the data directory path (~/.local/share/dotmatrix)
pub fn get_data_dir() -> anyhow::Result<PathBuf> {
    let data_dir = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find data directory"))?
        .join("dotmatrix");
    Ok(data_dir)
}

/// Get the config file path
pub fn get_config_path() -> anyhow::Result<PathBuf> {
    Ok(get_config_dir()?.join("config.toml"))
}

/// Get the index file path
pub fn get_index_path() -> anyhow::Result<PathBuf> {
    Ok(get_data_dir()?.join("index.json"))
}

/// Get the storage directory path (for incremental/content-addressed backups)
pub fn get_storage_path() -> anyhow::Result<PathBuf> {
    Ok(get_data_dir()?.join("storage"))
}

/// Get the archives directory path (for tarball backups)
pub fn get_archives_path() -> anyhow::Result<PathBuf> {
    Ok(get_data_dir()?.join("archives"))
}
