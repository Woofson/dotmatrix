//! File scanning and drift detection

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::index::FileEntry;
use crate::project::TrackedFile;

/// Result of scanning a file for drift
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// The tracked file
    pub file: TrackedFile,

    /// Current state on disk
    pub status: FileStatus,

    /// Current hash (if file exists)
    pub current_hash: Option<String>,
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

/// Scan a tracked file and determine its status
pub fn scan_file(file: &TrackedFile, last_entry: Option<&FileEntry>) -> ScanResult {
    if !file.path.exists() {
        return ScanResult {
            file: file.clone(),
            status: FileStatus::Missing,
            current_hash: None,
        };
    }

    let current_hash = match hash_file(&file.path) {
        Ok(h) => h,
        Err(_) => {
            return ScanResult {
                file: file.clone(),
                status: FileStatus::Error,
                current_hash: None,
            };
        }
    };

    let status = match last_entry {
        Some(entry) if entry.hash == current_hash => FileStatus::Synced,
        Some(_) => FileStatus::Drifted,
        None => FileStatus::New,
    };

    ScanResult {
        file: file.clone(),
        status,
        current_hash: Some(current_hash),
    }
}
