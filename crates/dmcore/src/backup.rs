//! Backup operations
//!
//! Supports two backup modes:
//! - Incremental: Content-addressed storage with deduplication
//! - Archive: Compressed archives (tar.gz, zip, 7z)

use age::secrecy::SecretString;
use chrono::Local;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config::{ArchiveFormat, Config};
use crate::git;
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

/// Check if a project has any files marked for encryption
pub fn project_needs_password(project: &Project) -> bool {
    project.files.iter().any(|f| f.encrypted)
}

/// Backup a project using incremental mode with encryption support
///
/// Files marked as encrypted will be encrypted with the provided password
/// before being stored. Non-encrypted files are stored normally.
pub fn backup_incremental_encrypted(
    config: &Config,
    project: &Project,
    index: &mut Index,
    password: Option<&SecretString>,
) -> anyhow::Result<BackupResult> {
    let mut result = BackupResult::default();

    for file in &project.files {
        let abs_path = file.absolute_path();

        if !abs_path.exists() {
            result.errors += 1;
            continue;
        }

        // Determine if this file should be encrypted
        let should_encrypt = file.encrypted && password.is_some();

        // Store file in content-addressed store
        let store_result = if should_encrypt {
            store::store_file_encrypted(config, &abs_path, password)?
        } else {
            store::store_file(config, &abs_path)?
        };

        // Update index
        let (size, modified) = file_metadata(&abs_path)?;
        let mut entry = if should_encrypt {
            FileEntry::with_sync_now_encrypted(store_result.hash.clone(), size, modified)
        } else {
            FileEntry::with_sync_now(store_result.hash.clone(), size, modified)
        };
        entry.mark_backed_up();
        index.upsert(abs_path, entry);

        if store_result.was_new {
            result.backed_up += 1;
            result.bytes_stored += store_result.size;
        } else {
            result.unchanged += 1;
        }
    }

    Ok(result)
}

/// Backup a project using its isolated store and index
///
/// Each project gets its own:
/// - Git repository (.git/)
/// - Content-addressed store (store/)
/// - File index (index.json)
pub fn backup_project_incremental(
    config: &Config,
    project_name: &str,
    project: &Project,
) -> anyhow::Result<BackupResult> {
    let store_dir = config.project_store_dir(project_name)?;
    let mut index = Index::load_for_project(config, project_name)?;

    let mut result = BackupResult::default();

    for file in &project.files {
        let abs_path = file.absolute_path();

        if !abs_path.exists() {
            result.errors += 1;
            continue;
        }

        // Store file in project-specific store
        match store::store_file_to(&store_dir, &abs_path) {
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

    // Save the project-specific index
    index.save_for_project(config, project_name)?;

    // Commit to project git repo if there are changes
    let project_dir = config.project_dir(project_name)?;
    if git::is_git_repo(&project_dir) {
        git::stage_all(&project_dir)?;
        if git::has_staged_changes(&project_dir)? {
            let msg = format!("Backup: {} files", result.backed_up + result.unchanged);
            git::commit(&project_dir, &msg)?;
            result.committed = true;
        }
    }

    Ok(result)
}

/// Backup a project with encryption support using isolated store and index
pub fn backup_project_incremental_encrypted(
    config: &Config,
    project_name: &str,
    project: &Project,
    password: Option<&SecretString>,
) -> anyhow::Result<BackupResult> {
    backup_project_incremental_encrypted_with_message(config, project_name, project, password, None)
}

/// Backup a project with encryption support and custom commit message
///
/// If custom_message is provided, it will be used as the commit message.
/// Date/time is always appended to the commit message.
pub fn backup_project_incremental_encrypted_with_message(
    config: &Config,
    project_name: &str,
    project: &Project,
    password: Option<&SecretString>,
    custom_message: Option<&str>,
) -> anyhow::Result<BackupResult> {
    let store_dir = config.project_store_dir(project_name)?;
    let mut index = Index::load_for_project(config, project_name)?;

    let mut result = BackupResult::default();

    for file in &project.files {
        let abs_path = file.absolute_path();

        if !abs_path.exists() {
            result.errors += 1;
            continue;
        }

        // Determine if this file should be encrypted
        let should_encrypt = file.encrypted && password.is_some();

        // Store file in project-specific store
        let store_result = if should_encrypt {
            store::store_file_to_encrypted(&store_dir, &abs_path, password)?
        } else {
            store::store_file_to(&store_dir, &abs_path)?
        };

        // Update index
        let (size, modified) = file_metadata(&abs_path)?;
        let mut entry = if should_encrypt {
            FileEntry::with_sync_now_encrypted(store_result.hash.clone(), size, modified)
        } else {
            FileEntry::with_sync_now(store_result.hash.clone(), size, modified)
        };
        entry.mark_backed_up();
        index.upsert(abs_path, entry);

        if store_result.was_new {
            result.backed_up += 1;
            result.bytes_stored += store_result.size;
        } else {
            result.unchanged += 1;
        }
    }

    // Save the project-specific index
    index.save_for_project(config, project_name)?;

    // Commit to project git repo if there are changes
    let project_dir = config.project_dir(project_name)?;
    if git::is_git_repo(&project_dir) {
        git::stage_all(&project_dir)?;
        if git::has_staged_changes(&project_dir)? {
            // Build commit message with date/time
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
            let msg = match custom_message {
                Some(custom) if !custom.is_empty() => {
                    format!("{} [{}]", custom, timestamp)
                }
                _ => {
                    format!("Backup: {} files [{}]", result.backed_up + result.unchanged, timestamp)
                }
            };
            git::commit(&project_dir, &msg)?;
            result.committed = true;
        }
    }

    Ok(result)
}
