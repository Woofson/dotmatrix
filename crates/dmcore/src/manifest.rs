//! Project manifest - maps logical projects to scattered disk paths

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::project::Project;

/// The manifest tracks all projects and their file mappings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Manifest {
    /// All tracked projects
    #[serde(default)]
    pub projects: HashMap<String, Project>,
}

impl Manifest {
    /// Load manifest from the default location
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::manifest_path()?;
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&contents)?)
        } else {
            Ok(Self::default())
        }
    }

    /// Save manifest to the default location
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::manifest_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        Ok(())
    }

    /// Get the manifest file path
    pub fn manifest_path() -> anyhow::Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("dotmatrix").join("manifest.toml"))
    }

    /// Add a new project
    pub fn add_project(&mut self, name: String, project: Project) {
        self.projects.insert(name, project);
    }

    /// Remove a project
    pub fn remove_project(&mut self, name: &str) -> Option<Project> {
        self.projects.remove(name)
    }

    /// Get a project by name
    pub fn get_project(&self, name: &str) -> Option<&Project> {
        self.projects.get(name)
    }

    /// Get a mutable reference to a project
    pub fn get_project_mut(&mut self, name: &str) -> Option<&mut Project> {
        self.projects.get_mut(name)
    }

    /// List all project names
    pub fn list_projects(&self) -> Vec<&str> {
        self.projects.keys().map(|s| s.as_str()).collect()
    }
}
