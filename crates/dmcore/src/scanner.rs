//! File scanning and drift detection
//!
//! Provides SHA256-based change detection for tracked files.

use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::Path;

use crate::index::Index;
use crate::project::{Project, TrackedFile};

/// Result of scanning a file for drift
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// The path as stored in the project
    pub path: String,

    /// Current state on disk
    pub status: FileStatus,

    /// Current hash (if file exists)
    pub current_hash: Option<String>,

    /// Current size (if file exists)
    pub current_size: Option<u64>,

    /// Track mode
    pub track_mode: crate::project::TrackMode,
}

/// Status of a tracked file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// File is synced (hash matches)
    Synced,
    /// File has changed since last sync
    Drifted,
    /// File is new (never synced)
    New,
    /// File is missing from disk
    Missing,
    /// Error reading file
    Error,
}

impl FileStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            FileStatus::Synced => "✓",
            FileStatus::Drifted => "⚠",
            FileStatus::New => "+",
            FileStatus::Missing => "✗",
            FileStatus::Error => "!",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            FileStatus::Synced => "synced",
            FileStatus::Drifted => "drifted",
            FileStatus::New => "new",
            FileStatus::Missing => "missing",
            FileStatus::Error => "error",
        }
    }
}

/// Calculate SHA256 hash of a file
pub fn hash_file(path: &Path) -> anyhow::Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}

/// Get file metadata
pub fn file_metadata(path: &Path) -> anyhow::Result<(u64, u64)> {
    let meta = fs::metadata(path)?;
    let size = meta.len();
    let modified = meta
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Ok((size, modified))
}

/// Scan a tracked file and determine its status
pub fn scan_file(file: &TrackedFile, index: &Index) -> ScanResult {
    let abs_path = file.absolute_path();

    if !abs_path.exists() {
        return ScanResult {
            path: file.path.clone(),
            status: FileStatus::Missing,
            current_hash: None,
            current_size: None,
            track_mode: file.track,
        };
    }

    let (current_hash, current_size) = match hash_file(&abs_path) {
        Ok(h) => {
            let size = fs::metadata(&abs_path).map(|m| m.len()).ok();
            (Some(h), size)
        }
        Err(_) => {
            return ScanResult {
                path: file.path.clone(),
                status: FileStatus::Error,
                current_hash: None,
                current_size: None,
                track_mode: file.track,
            };
        }
    };

    let status = match index.get(&abs_path) {
        Some(entry) if Some(&entry.hash) == current_hash.as_ref() => FileStatus::Synced,
        Some(_) => FileStatus::Drifted,
        None => FileStatus::New,
    };

    ScanResult {
        path: file.path.clone(),
        status,
        current_hash,
        current_size,
        track_mode: file.track,
    }
}

/// Scan all files in a project
pub fn scan_project(project: &Project, index: &Index) -> Vec<ScanResult> {
    project
        .files
        .iter()
        .map(|f| scan_file(f, index))
        .collect()
}

/// Summary of project status
#[derive(Debug, Clone, Default)]
pub struct ProjectSummary {
    pub total: usize,
    pub synced: usize,
    pub drifted: usize,
    pub new: usize,
    pub missing: usize,
    pub errors: usize,
}

impl ProjectSummary {
    pub fn from_results(results: &[ScanResult]) -> Self {
        let mut summary = Self::default();
        summary.total = results.len();
        for r in results {
            match r.status {
                FileStatus::Synced => summary.synced += 1,
                FileStatus::Drifted => summary.drifted += 1,
                FileStatus::New => summary.new += 1,
                FileStatus::Missing => summary.missing += 1,
                FileStatus::Error => summary.errors += 1,
            }
        }
        summary
    }

    pub fn is_clean(&self) -> bool {
        self.drifted == 0 && self.new == 0 && self.missing == 0 && self.errors == 0
    }

    pub fn needs_attention(&self) -> bool {
        !self.is_clean()
    }
}
