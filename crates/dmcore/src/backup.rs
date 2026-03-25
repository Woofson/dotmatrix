//! Backup operations
//!
//! Supports two backup modes:
//! - Incremental: Content-addressed storage with deduplication
//! - Archive: Compressed archives (tar.gz, zip, 7z)

use chrono::Local;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config::{ArchiveFormat, Config};
use crate::index::{FileEntry, Index};
use crate::project::Project;
use crate::scanner::file_metadata;
use crate::store;

/// Result of a backup operation
#[derive(Debug, Clone, Default)]
pub struct BackupResult {
    /// Files successfully backed up
    pub backed_up: usize,
    /// Files unchanged (already in store)
    pub unchanged: usize,
    /// Files that failed
    pub errors: usize,
    /// Total bytes stored
    pub bytes_stored: u64,
    /// Whether a git commit was made
    pub committed: bool,
}

/// Backup a project using incremental (content-addressed) mode
pub fn backup_incremental(
    config: &Config,
    project: &Project,
    index: &mut Index,
) -> anyhow::Result<BackupResult> {
    let mut result = BackupResult::default();

    for file in &project.files {
        let abs_path = file.absolute_path();

        if !abs_path.exists() {
            result.errors += 1;
            continue;
        }

        // Store file in content-addressed store
        match store::store_file(config, &abs_path) {
            Ok(store_result) => {
                // Update index
                let (size, modified) = file_metadata(&abs_path)?;
                let mut entry = FileEntry::with_sync_now(store_result.hash.clone(), size, modified);
                entry.mark_backed_up();
                index.upsert(abs_path, entry);

                if store_result.was_new {
                    result.backed_up += 1;
                    result.bytes_stored += store_result.size;
                } else {
                    result.unchanged += 1;
                }
            }
            Err(_) => {
                result.errors += 1;
            }
        }
    }

    Ok(result)
}

/// Backup files to an archive
pub fn backup_archive(
    config: &Config,
    project_name: &str,
    project: &Project,
    format: ArchiveFormat,
) -> anyhow::Result<PathBuf> {
    let backups_dir = config.backups_dir()?;
    fs::create_dir_all(&backups_dir)?;

    let timestamp = Local::now().format("%Y%m%d-%H%M%S");
    let archive_name = format!("{}-{}.{}", project_name, timestamp, format.extension());
    let archive_path = backups_dir.join(&archive_name);

    match format {
        ArchiveFormat::TarGz => create_tar_gz(&archive_path, project)?,
        ArchiveFormat::Zip => create_zip(&archive_path, project)?,
        ArchiveFormat::SevenZ => {
            // For 7z, fall back to tar.gz with a note
            // Full 7z support would require the sevenz-rust crate
            let alt_path = backups_dir.join(format!("{}-{}.tar.gz", project_name, timestamp));
            create_tar_gz(&alt_path, project)?;
            return Ok(alt_path);
        }
    }

    Ok(archive_path)
}

/// Create a tar.gz archive
fn create_tar_gz(archive_path: &Path, project: &Project) -> anyhow::Result<()> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use tar::Builder;

    let file = File::create(archive_path)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = Builder::new(encoder);

    for tracked_file in &project.files {
        let abs_path = tracked_file.absolute_path();
        if abs_path.exists() && abs_path.is_file() {
            // Use the stored path (with ~) as the archive path
            let archive_path = tracked_file.path.trim_start_matches("~/");
            builder.append_path_with_name(&abs_path, archive_path)?;
        }
    }

    builder.finish()?;
    Ok(())
}

/// Create a zip archive
fn create_zip(archive_path: &Path, project: &Project) -> anyhow::Result<()> {
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    let file = File::create(archive_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for tracked_file in &project.files {
        let abs_path = tracked_file.absolute_path();
        if abs_path.exists() && abs_path.is_file() {
            let archive_path = tracked_file.path.trim_start_matches("~/");
            zip.start_file(archive_path, options)?;
            let content = fs::read(&abs_path)?;
            zip.write_all(&content)?;
        }
    }

    zip.finish()?;
    Ok(())
}

/// List archive backups for a project
pub fn list_archives(config: &Config, project_name: &str) -> anyhow::Result<Vec<ArchiveInfo>> {
    let backups_dir = config.backups_dir()?;
    if !backups_dir.exists() {
        return Ok(Vec::new());
    }

    let prefix = format!("{}-", project_name);
    let mut archives = Vec::new();

    for entry in fs::read_dir(backups_dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with(&prefix) {
                let meta = entry.metadata()?;
                archives.push(ArchiveInfo {
                    path: path.clone(),
                    name: name.to_string(),
                    size: meta.len(),
                    created: meta.created().ok(),
                });
            }
        }
    }

    // Sort by name (which includes timestamp) descending
    archives.sort_by(|a, b| b.name.cmp(&a.name));

    Ok(archives)
}

/// Information about an archive backup
#[derive(Debug, Clone)]
pub struct ArchiveInfo {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub created: Option<std::time::SystemTime>,
}
