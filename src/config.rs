use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub git_enabled: bool,
    pub tracked_files: Vec<String>,
    pub exclude: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            git_enabled: true,
            tracked_files: vec![
                "~/.bashrc".to_string(),
                "~/.zshrc".to_string(),
                "~/.gitconfig".to_string(),
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
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let content = toml::to_string_pretty(&self)?;
        fs::write(path, content)?;
        Ok(())
    }
}
