//! Content-addressed file store
//!
//! Files are stored by their SHA256 hash, enabling deduplication.
//! Structure: store/ab/cdef1234... (first 2 chars as subdirectory)

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::scanner::hash_file;

/// Result of storing a file
#[derive(Debug, Clone)]
pub struct StoreResult {
    /// Original file path
    pub source: PathBuf,
    /// SHA256 hash
    pub hash: String,
    /// File size in bytes
    pub size: u64,
    /// Whether file was newly stored (vs already existed)
    pub was_new: bool,
}

/// Get the storage path for a hash
pub fn hash_to_path(store_dir: &Path, hash: &str) -> PathBuf {
    // Use first 2 characters as subdirectory for better filesystem performance
    let (prefix, rest) = hash.split_at(2.min(hash.len()));
    store_dir.join(prefix).join(rest)
}

/// Store a file in the content-addressed store
pub fn store_file(config: &Config, source: &Path) -> anyhow::Result<StoreResult> {
    let store_dir = config.store_dir()?;
    fs::create_dir_all(&store_dir)?;

    // Hash the file
    let hash = hash_file(source)?;
    let size = fs::metadata(source)?.len();

    // Determine storage path
    let storage_path = hash_to_path(&store_dir, &hash);

    // Check if already stored (deduplication)
    let was_new = if storage_path.exists() {
        false
    } else {
        // Create parent directory
        if let Some(parent) = storage_path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Copy file to store
        fs::copy(source, &storage_path)?;
        true
    };

    Ok(StoreResult {
        source: source.to_path_buf(),
        hash,
        size,
        was_new,
    })
}

/// Retrieve a file from the store by hash
pub fn retrieve_file(config: &Config, hash: &str, dest: &Path) -> anyhow::Result<bool> {
    let store_dir = config.store_dir()?;
    let storage_path = hash_to_path(&store_dir, hash);

    if !storage_path.exists() {
        return Ok(false);
    }

    // Create parent directory for destination
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(&storage_path, dest)?;
    Ok(true)
}

/// Check if a hash exists in the store
pub fn exists_in_store(config: &Config, hash: &str) -> anyhow::Result<bool> {
    let store_dir = config.store_dir()?;
    let storage_path = hash_to_path(&store_dir, hash);
    Ok(storage_path.exists())
}

/// Get the path to a stored file (for reading)
pub fn get_stored_path(config: &Config, hash: &str) -> anyhow::Result<Option<PathBuf>> {
    let store_dir = config.store_dir()?;
    let storage_path = hash_to_path(&store_dir, hash);
    if storage_path.exists() {
        Ok(Some(storage_path))
    } else {
        Ok(None)
    }
}

/// Calculate total store size
pub fn store_size(config: &Config) -> anyhow::Result<(u64, usize)> {
    let store_dir = config.store_dir()?;
    if !store_dir.exists() {
        return Ok((0, 0));
    }

    let mut total_size = 0u64;
    let mut file_count = 0usize;

    for entry in walkdir(store_dir)? {
        if entry.is_file() {
            total_size += entry.metadata()?.len();
            file_count += 1;
        }
    }

    Ok((total_size, file_count))
}

/// Simple directory walker
fn walkdir(dir: PathBuf) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !dir.exists() {
        return Ok(files);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(walkdir(path)?);
        } else {
            files.push(path);
        }
    }

    Ok(files)
}
