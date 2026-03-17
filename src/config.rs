use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Backup mode for files
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum BackupMode {
    #[default]
    Incremental,
    Archive,
}

impl BackupMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackupMode::Incremental => "incremental",
            BackupMode::Archive => "archive",
        }
    }
}

/// Archive format for archive backup mode
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveFormat {
    Zip,
    TarGz,
    SevenZip,
}

impl Default for ArchiveFormat {
    fn default() -> Self {
        #[cfg(windows)]
        { ArchiveFormat::Zip }
        #[cfg(not(windows))]
        { ArchiveFormat::TarGz }
    }
}

impl ArchiveFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ArchiveFormat::Zip => "zip",
            ArchiveFormat::TarGz => "tar.gz",
            ArchiveFormat::SevenZip => "7z",
        }
    }
}

/// Preferred interface mode
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PreferredInterface {
    #[default]
    Auto,
    Gui,
    Tui,
}

/// A tracked file pattern with optional per-pattern settings
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum TrackedPattern {
    /// Simple string pattern (uses default backup mode)
    Simple(String),
    /// Pattern with explicit settings
    WithOptions {
        path: String,
        #[serde(default)]
        mode: Option<BackupMode>,
        #[serde(default)]
        encrypted: bool,
    },
}

impl TrackedPattern {
    /// Create a simple pattern from a string
    pub fn simple(path: impl Into<String>) -> Self {
        TrackedPattern::Simple(path.into())
    }

    /// Get the path pattern
    pub fn path(&self) -> &str {
        match self {
            TrackedPattern::Simple(p) => p,
            TrackedPattern::WithOptions { path, .. } => path,
        }
    }

    /// Get the backup mode (None means use default)
    pub fn mode(&self) -> Option<BackupMode> {
        match self {
            TrackedPattern::Simple(_) => None,
            TrackedPattern::WithOptions { mode, .. } => *mode,
        }
    }

    /// Check if this pattern requires encryption
    pub fn encrypted(&self) -> bool {
        match self {
            TrackedPattern::Simple(_) => false,
            TrackedPattern::WithOptions { encrypted, .. } => *encrypted,
        }
    }

    /// Check if this pattern matches a path string
    pub fn matches_path(&self, path: &str) -> bool {
        self.path() == path
    }

    /// Set the encrypted flag, converting Simple to WithOptions if needed
    pub fn set_encrypted(&mut self, encrypted: bool) {
        match self {
            TrackedPattern::Simple(path) => {
                if encrypted {
                    // Convert to WithOptions to enable encryption
                    *self = TrackedPattern::WithOptions {
                        path: path.clone(),
                        mode: None,
                        encrypted: true,
                    };
                }
                // If not encrypted, keep as Simple (default is unencrypted)
            }
            TrackedPattern::WithOptions { encrypted: enc, mode, path } => {
                if !encrypted && mode.is_none() {
                    // Convert back to Simple if no special options
                    *self = TrackedPattern::Simple(path.clone());
                } else {
                    *enc = encrypted;
                }
            }
        }
    }
}

impl std::fmt::Display for TrackedPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackedPattern::Simple(p) => write!(f, "{}", p),
            TrackedPattern::WithOptions { path, mode, encrypted } => {
                let mode_str = mode.map(|m| m.as_str()).unwrap_or("");
                let enc_str = if *encrypted { "encrypted" } else { "" };
                let flags: Vec<&str> = [mode_str, enc_str].iter()
                    .filter(|s| !s.is_empty())
                    .copied()
                    .collect();
                if flags.is_empty() {
                    write!(f, "{}", path)
                } else {
                    write!(f, "{} ({})", path, flags.join(", "))
                }
            }
        }
    }
}

fn default_backup_mode() -> BackupMode {
    BackupMode::Incremental
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    /// Custom data directory path (optional, defaults to system data dir)
    /// On Linux: ~/.local/share/dotmatrix
    /// On Windows: C:\Users\<User>\Documents\dotmatrix
    /// On macOS: ~/Library/Application Support/dotmatrix
    #[serde(default)]
    pub data_dir: Option<String>,
    pub git_enabled: bool,
    #[serde(default = "default_backup_mode")]
    pub backup_mode: BackupMode,
    /// Archive format for archive backup mode
    /// "zip" = ZIP archive (default on Windows)
    /// "targz" = tar.gz archive (default on Linux/macOS)
    /// "sevenzip" = 7z archive
    #[serde(default)]
    pub archive_format: ArchiveFormat,
    pub tracked_files: Vec<TrackedPattern>,
    pub exclude: Vec<String>,
    /// Preferred interface when running without arguments
    /// "auto" = platform default (GUI on Windows, TUI on Linux/macOS)
    /// "gui" = always use GUI
    /// "tui" = always use TUI
    #[serde(default)]
    pub preferred_interface: PreferredInterface,
    /// Git remote URL for push/pull operations (optional)
    /// Example: "https://github.com/user/dotfiles.git"
    #[serde(default)]
    pub git_remote_url: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        // Platform-specific default tracked files
        #[cfg(windows)]
        let default_tracked = vec![
            TrackedPattern::Simple("~/.gitconfig".to_string()),
            TrackedPattern::Simple("~/AppData/Local/dotmatrix/*".to_string()),
        ];

        #[cfg(not(windows))]
        let default_tracked = vec![
            TrackedPattern::Simple("~/.bashrc".to_string()),
            TrackedPattern::Simple("~/.zshrc".to_string()),
            TrackedPattern::Simple("~/.gitconfig".to_string()),
            TrackedPattern::Simple("~/.config/dotmatrix/*".to_string()),
        ];

        Config {
            data_dir: None,  // Use system default
            git_enabled: true,
            backup_mode: BackupMode::Incremental,
            archive_format: ArchiveFormat::default(),
            tracked_files: default_tracked,
            exclude: vec![
                "**/*.log".to_string(),
                "**/.DS_Store".to_string(),
                "**/node_modules/**".to_string(),
            ],
            preferred_interface: PreferredInterface::Auto,
            git_remote_url: None,
        }
    }
}

/// Expand ~ to home directory (works on all platforms)
pub fn expand_path(path: &str) -> PathBuf {
    if path.starts_with("~/") || path == "~" {
        if let Some(home) = dirs::home_dir() {
            if path == "~" {
                return home;
            }
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

impl Config {
    /// Load config from file
    pub fn load(path: &PathBuf) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save config to file
    pub fn save(&self, path: &PathBuf) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(&self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Get the expanded data directory path
    /// Returns None if using system default, Some(path) if custom
    pub fn get_data_dir(&self) -> Option<PathBuf> {
        self.data_dir.as_ref().map(|p| expand_path(p))
    }

    /// Get all pattern strings (for backward compatibility)
    pub fn pattern_strings(&self) -> Vec<String> {
        self.tracked_files.iter().map(|p| p.path().to_string()).collect()
    }

    /// Get the effective backup mode for a pattern
    pub fn mode_for_pattern(&self, pattern: &TrackedPattern) -> BackupMode {
        pattern.mode().unwrap_or(self.backup_mode)
    }
}
