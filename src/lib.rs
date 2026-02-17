pub mod app;
pub mod config;
pub mod gui;
pub mod index;
pub mod scanner;
pub mod tui;

use std::path::PathBuf;

/// Check for portable config directory (Windows only)
/// Returns Some(dir) if config.toml exists next to the executable
#[cfg(windows)]
fn get_portable_config_dir() -> Option<PathBuf> {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let portable_config = exe_dir.join("config.toml");
            if portable_config.exists() {
                return Some(exe_dir.to_path_buf());
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn get_portable_config_dir() -> Option<PathBuf> {
    None
}

/// Get the config directory path
/// - Windows (portable): Directory containing the executable (if config.toml exists there)
/// - Linux: ~/.config/dotmatrix
/// - Windows: C:\Users\<User>\AppData\Roaming\dotmatrix
/// - macOS: ~/Library/Application Support/dotmatrix
pub fn get_config_dir() -> anyhow::Result<PathBuf> {
    // Check for portable mode first (Windows only)
    if let Some(portable_dir) = get_portable_config_dir() {
        return Ok(portable_dir);
    }

    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
        .join("dotmatrix");
    Ok(config_dir)
}

/// Get the default data directory path (used when not configured)
/// - Windows (portable): "data" folder next to the executable (if config.toml exists there)
/// - Linux: ~/.local/share/dotmatrix
/// - Windows: C:\Users\<User>\Documents\dotmatrix (for better discoverability)
/// - macOS: ~/Library/Application Support/dotmatrix
pub fn get_default_data_dir() -> anyhow::Result<PathBuf> {
    // In portable mode, use "data" folder next to executable
    if let Some(portable_dir) = get_portable_config_dir() {
        return Ok(portable_dir.join("data"));
    }

    #[cfg(target_os = "windows")]
    {
        // Use Documents folder on Windows for better discoverability
        let docs = dirs::document_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find Documents directory"))?;
        return Ok(docs.join("dotmatrix"));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find data directory"))?
            .join("dotmatrix");
        Ok(data_dir)
    }
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
