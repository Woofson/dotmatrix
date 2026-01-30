use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub hash: String,
    pub last_modified: u64,
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Index {
    pub files: HashMap<PathBuf, FileEntry>,
}

impl Index {
    pub fn new() -> Self {
        Index {
            files: HashMap::new(),
        }
    }

    /// Load index from file
    pub fn load(path: &PathBuf) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let index: Index = serde_json::from_str(&content)?;
        Ok(index)
    }

    /// Save index to file
    pub fn save(&self, path: &PathBuf) -> anyhow::Result<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let content = serde_json::to_string_pretty(&self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Add a file to the index
    pub fn add_file(&mut self, path: PathBuf, entry: FileEntry) {
        self.files.insert(path, entry);
    }

    /// Remove a file from the index
    pub fn remove_file(&mut self, path: &PathBuf) -> Option<FileEntry> {
        self.files.remove(path)
    }

    /// Get a file entry
    pub fn get_file(&self, path: &PathBuf) -> Option<&FileEntry> {
        self.files.get(path)
    }
}

impl Default for Index {
    fn default() -> Self {
        Self::new()
    }
}
