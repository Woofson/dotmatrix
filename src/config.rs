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

    /// Check if this pattern matches a path string
    pub fn matches_path(&self, path: &str) -> bool {
        self.path() == path
    }
}

impl std::fmt::Display for TrackedPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackedPattern::Simple(p) => write!(f, "{}", p),
            TrackedPattern::WithOptions { path, mode } => {
                if let Some(m) = mode {
                    write!(f, "{} ({})", path, m.as_str())
                } else {
                    write!(f, "{}", path)
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
    pub git_enabled: bool,
    #[serde(default = "default_backup_mode")]
    pub backup_mode: BackupMode,
    pub tracked_files: Vec<TrackedPattern>,
    pub exclude: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            git_enabled: true,
            backup_mode: BackupMode::Incremental,
            tracked_files: vec![
                TrackedPattern::Simple("~/.bashrc".to_string()),
                TrackedPattern::Simple("~/.zshrc".to_string()),
                TrackedPattern::Simple("~/.gitconfig".to_string()),
                TrackedPattern::Simple("~/.config/dotmatrix/*".to_string()),
            ],
            exclude: vec![
                "**/*.log".to_string(),
                "**/.DS_Store".to_string(),
                "**/node_modules/**".to_string(),
            ],
        }
    }
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

    /// Get all pattern strings (for backward compatibility)
    pub fn pattern_strings(&self) -> Vec<String> {
        self.tracked_files.iter().map(|p| p.path().to_string()).collect()
    }

    /// Get the effective backup mode for a pattern
    pub fn mode_for_pattern(&self, pattern: &TrackedPattern) -> BackupMode {
        pattern.mode().unwrap_or(self.backup_mode)
    }
}
