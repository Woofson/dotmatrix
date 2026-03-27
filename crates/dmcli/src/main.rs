//! dmcli - CLI for dotmatrix
//!
//! Full-featured command-line interface for project management.
//! Designed for automation, scripting, and power users.

use age::secrecy::SecretString;
use clap::{Parser, Subcommand, ValueEnum};
use dmcore::{
    backup_archive, backup_project_incremental_encrypted_with_message, contract_path, expand_path,
    fetch, get_remote_status, get_remote_url, init_project_repo, list_archives,
    project_needs_password, pull, push, recent_commits, retrieve_file_from_encrypted,
    scan_project, set_remote_url, ArchiveFormat, Config, FileStatus, Index, Manifest, Project,
    ProjectSummary, TrackMode, TrackedFile,
};
use std::io::BufRead;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "dotmatrix")]
#[command(author, version, about = "Project compositor with git versioning")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output as JSON (for scripting)
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize dotmatrix
    Init,

    /// Create a new project
    New {
        /// Project name
        name: String,

        /// Project description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Add files to a project
    Add {
        /// Project name
        project: String,

        /// File paths to add
        files: Vec<String>,

        /// Track mode for added files
        #[arg(short, long, value_enum, default_value = "git")]
        track: TrackModeArg,

        /// Mark files as encrypted
        #[arg(short, long)]
        encrypted: bool,
    },

    /// Remove files from a project
    Remove {
        /// Project name
        project: String,

        /// File paths to remove
        files: Vec<String>,
    },

    /// Show project status
    Status {
        /// Project name (or all if not specified)
        project: Option<String>,

        /// Show only files needing attention
        #[arg(short, long)]
        changes: bool,
    },

    /// Sync drifted files to index (mark as synced)
    Sync {
        /// Project name (or all if not specified)
        project: Option<String>,
    },

    /// Backup project files to content-addressed store
    Backup {
        /// Project name (or all if not specified)
        project: Option<String>,

        /// Commit message for git
        #[arg(short, long)]
        message: Option<String>,

        /// Create archive backup instead of incremental
        #[arg(short, long)]
        archive: bool,

        /// Archive format (tar.gz, zip, 7z)
        #[arg(long, value_enum, default_value = "tar-gz")]
        format: ArchiveFormatArg,

        /// Read encryption password from file
        #[arg(long)]
        password_file: Option<PathBuf>,

        /// Read encryption password from stdin
        #[arg(long)]
        password_stdin: bool,
    },

    /// Restore files from backup store
    Restore {
        /// Project name
        project: String,

        /// Specific files to restore (or all if not specified)
        files: Vec<String>,

        /// Show what would be restored without making changes
        #[arg(long)]
        dry_run: bool,

        /// Read decryption password from file
        #[arg(long)]
        password_file: Option<PathBuf>,

        /// Read decryption password from stdin
        #[arg(long)]
        password_stdin: bool,
    },

    /// List all projects
    List {
        /// Show detailed info
        #[arg(short, long)]
        verbose: bool,
    },

    /// Show project info
    Info {
        /// Project name
        project: String,
    },

    /// Delete a project (does not delete files)
    Delete {
        /// Project name
        project: String,

        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Git operations for a project
    Git {
        /// Project name
        project: String,

        #[command(subcommand)]
        action: GitAction,
    },

    /// List archive backups for a project
    Archives {
        /// Project name
        project: String,
    },

    /// Show store statistics
    Store {
        /// Project name (optional, shows global if not specified)
        project: Option<String>,
    },

    /// Launch TUI
    Tui,

    /// Launch GUI
    Gui,
}

#[derive(Subcommand)]
enum GitAction {
    /// Show or set git remote URL
    Remote {
        /// Set remote URL
        #[arg(long)]
        set: Option<String>,
    },
    /// Push to remote
    Push,
    /// Pull from remote
    Pull,
    /// Fetch from remote
    Fetch,
    /// Show recent commits
    Log {
        /// Number of commits to show
        #[arg(short, long, default_value = "10")]
        count: usize,
    },
    /// Show ahead/behind status
    Status,
}

#[derive(Clone, Copy, ValueEnum)]
enum TrackModeArg {
    Git,
    Backup,
    Both,
}

impl From<TrackModeArg> for TrackMode {
    fn from(arg: TrackModeArg) -> Self {
        match arg {
            TrackModeArg::Git => TrackMode::Git,
            TrackModeArg::Backup => TrackMode::Backup,
            TrackModeArg::Both => TrackMode::Both,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum ArchiveFormatArg {
    TarGz,
    Zip,
    SevenZ,
}

impl From<ArchiveFormatArg> for ArchiveFormat {
    fn from(arg: ArchiveFormatArg) -> Self {
        match arg {
            ArchiveFormatArg::TarGz => ArchiveFormat::TarGz,
            ArchiveFormatArg::Zip => ArchiveFormat::Zip,
            ArchiveFormatArg::SevenZ => ArchiveFormat::SevenZ,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => cmd_init(cli.json)?,
        Commands::New { name, description } => cmd_new(name, description, cli.json)?,
        Commands::Add {
            project,
            files,
            track,
            encrypted,
        } => cmd_add(project, files, track.into(), encrypted, cli.json)?,
        Commands::Remove { project, files } => cmd_remove(project, files, cli.json)?,
        Commands::Status { project, changes } => cmd_status(project, changes, cli.json)?,
        Commands::Sync { project } => cmd_sync(project, cli.json)?,
        Commands::Backup {
            project,
            message,
            archive,
            format,
            password_file,
            password_stdin,
        } => cmd_backup(project, message, archive, format.into(), password_file, password_stdin, cli.json)?,
        Commands::Restore {
            project,
            files,
            dry_run,
            password_file,
            password_stdin,
        } => cmd_restore(project, files, dry_run, password_file, password_stdin, cli.json)?,
        Commands::List { verbose } => cmd_list(verbose, cli.json)?,
        Commands::Info { project } => cmd_info(project, cli.json)?,
        Commands::Delete { project, force } => cmd_delete(project, force, cli.json)?,
        Commands::Git { project, action } => cmd_git(project, action, cli.json)?,
        Commands::Archives { project } => cmd_archives(project, cli.json)?,
        Commands::Store { project } => cmd_store(project, cli.json)?,
        Commands::Tui => cmd_tui()?,
        Commands::Gui => cmd_gui()?,
    }

    Ok(())
}

fn cmd_init(json: bool) -> anyhow::Result<()> {
    let config = Config::load()?;
    config.save()?;

    let manifest = Manifest::load()?;
    manifest.save()?;

    let index = Index::load()?;
    index.save()?;

    // Create data directory
    let data_dir = config.data_dir()?;
    std::fs::create_dir_all(&data_dir)?;

    if json {
        let output = serde_json::json!({
            "config": Config::config_path()?.to_string_lossy(),
            "manifest": Manifest::manifest_path()?.to_string_lossy(),
            "index": Index::index_path()?.to_string_lossy(),
            "data": data_dir.to_string_lossy(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("dotmatrix 2.0.0 - project compositor with git versioning");
        println!();
        println!("Config:   {}", Config::config_path()?.display());
        println!("Manifest: {}", Manifest::manifest_path()?.display());
        println!("Index:    {}", Index::index_path()?.display());
        println!("Data:     {}", data_dir.display());
        println!();
        println!("Ready. Create a project with: dotmatrix new <name>");
    }

    Ok(())
}

fn cmd_new(name: String, description: Option<String>, json: bool) -> anyhow::Result<()> {
    let config = Config::load()?;
    let mut manifest = Manifest::load()?;

    if manifest.get_project(&name).is_some() {
        anyhow::bail!("Project '{}' already exists", name);
    }

    let project = match &description {
        Some(desc) => Project::with_description(desc.clone()),
        None => Project::new(),
    };

    manifest.add_project(name.clone(), project);
    manifest.save()?;

    // Initialize project-specific git repo
    init_project_repo(&config, &name)?;

    if json {
        let output = serde_json::json!({
            "name": name,
            "description": description,
            "created": true,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Created project: {}", name);
        println!("Add files with: dotmatrix add {} <files...>", name);
    }

    Ok(())
}

fn cmd_add(
    project_name: String,
    files: Vec<String>,
    track: TrackMode,
    encrypted: bool,
    json: bool,
) -> anyhow::Result<()> {
    let mut manifest = Manifest::load()?;

    let project = manifest
        .get_project_mut(&project_name)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", project_name))?;

    let mut added = 0;
    let mut skipped = 0;
    let mut added_files = Vec::new();
    let mut skipped_files = Vec::new();

    for file_path in files {
        // Expand and validate path
        let abs_path = if file_path.starts_with("~/") {
            expand_path(&file_path)
        } else {
            let p = Path::new(&file_path);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                std::env::current_dir()?.join(p).canonicalize()?
            }
        };

        if !abs_path.exists() {
            if !json {
                println!("Warning: File not found: {}", abs_path.display());
            }
            skipped_files.push(serde_json::json!({"path": file_path, "reason": "not found"}));
            skipped += 1;
            continue;
        }

        if !abs_path.is_file() {
            if !json {
                println!("Warning: Not a file: {}", abs_path.display());
            }
            skipped_files.push(serde_json::json!({"path": file_path, "reason": "not a file"}));
            skipped += 1;
            continue;
        }

        // Store with ~ for home directory paths
        let stored_path = contract_path(&abs_path);

        let mut tf = TrackedFile::with_mode(stored_path.clone(), track);
        tf.encrypted = encrypted;

        if project.add_file(tf) {
            if !json {
                println!("  + {} ({})", stored_path, track);
            }
            added_files.push(serde_json::json!({"path": stored_path, "track": track.to_string(), "encrypted": encrypted}));
            added += 1;
        } else {
            if !json {
                println!("  ~ {} (already tracked)", stored_path);
            }
            skipped_files.push(serde_json::json!({"path": stored_path, "reason": "already tracked"}));
            skipped += 1;
        }
    }

    manifest.save()?;

    if json {
        let output = serde_json::json!({
            "project": project_name,
            "added": added,
            "skipped": skipped,
            "files": added_files,
            "skipped_files": skipped_files,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!();
        println!(
            "Added {} file(s) to '{}' ({} skipped)",
            added, project_name, skipped
        );
    }

    Ok(())
}

fn cmd_remove(project_name: String, files: Vec<String>, json: bool) -> anyhow::Result<()> {
    let mut manifest = Manifest::load()?;

    let project = manifest
        .get_project_mut(&project_name)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", project_name))?;

    let mut removed = 0;
    let mut removed_files = Vec::new();
    let mut not_found = Vec::new();

    for file_path in files {
        // Try to match the path as-is first, then try expanded/contracted versions
        if project.remove_file(&file_path) {
            if !json {
                println!("  - {}", file_path);
            }
            removed_files.push(file_path.clone());
            removed += 1;
        } else {
            // Try with expansion/contraction
            let abs_path = expand_path(&file_path);
            let contracted = contract_path(&abs_path);
            if project.remove_file(&contracted) {
                if !json {
                    println!("  - {}", contracted);
                }
                removed_files.push(contracted);
                removed += 1;
            } else {
                if !json {
                    println!("  ? {} (not found in project)", file_path);
                }
                not_found.push(file_path);
            }
        }
    }

    manifest.save()?;

    if json {
        let output = serde_json::json!({
            "project": project_name,
            "removed": removed,
            "files": removed_files,
            "not_found": not_found,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!();
        println!("Removed {} file(s) from '{}'", removed, project_name);
    }

    Ok(())
}

fn cmd_status(project_name: Option<String>, changes_only: bool, json: bool) -> anyhow::Result<()> {
    let config = Config::load()?;
    let manifest = Manifest::load()?;

    let projects: Vec<(&str, &Project)> = match &project_name {
        Some(name) => {
            let p = manifest
                .get_project(name)
                .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", name))?;
            vec![(name.as_str(), p)]
        }
        None => manifest
            .projects
            .iter()
            .map(|(k, v)| (k.as_str(), v))
            .collect(),
    };

    if projects.is_empty() {
        if json {
            println!("{}", serde_json::json!({"projects": []}));
        } else {
            println!("No projects. Create one with: dotmatrix new <name>");
        }
        return Ok(());
    }

    let mut json_projects = Vec::new();

    for (name, project) in projects {
        // Use project-specific index
        let index = Index::load_for_project(&config, name).unwrap_or_default();
        let results = scan_project(project, &index);
        let summary = ProjectSummary::from_results(&results);

        if json {
            let files: Vec<_> = results
                .iter()
                .filter(|r| !changes_only || r.status != FileStatus::Synced)
                .map(|r| {
                    serde_json::json!({
                        "path": r.path,
                        "status": r.status.description(),
                        "size": r.current_size,
                        "hash": r.current_hash,
                    })
                })
                .collect();

            json_projects.push(serde_json::json!({
                "name": name,
                "total": summary.total,
                "synced": summary.synced,
                "drifted": summary.drifted,
                "new": summary.new,
                "missing": summary.missing,
                "files": files,
            }));
        } else {
            println!("{}/ ({} files)", name, summary.total);

            if project.files.is_empty() {
                println!("  (empty - add files with: dotmatrix add {} <files...>)", name);
                println!();
                continue;
            }

            for r in &results {
                if changes_only && r.status == FileStatus::Synced {
                    continue;
                }

                let size_str = r
                    .current_size
                    .map(|s| format_size(s))
                    .unwrap_or_else(|| "-".to_string());

                println!(
                    "  {} {:12} {:>8}  {}",
                    r.status.symbol(),
                    r.status.description(),
                    size_str,
                    r.path
                );
            }

            // Summary line
            if summary.needs_attention() {
                let mut parts = Vec::new();
                if summary.drifted > 0 {
                    parts.push(format!("{} drifted", summary.drifted));
                }
                if summary.new > 0 {
                    parts.push(format!("{} new", summary.new));
                }
                if summary.missing > 0 {
                    parts.push(format!("{} missing", summary.missing));
                }
                println!("  ({}: {})", name, parts.join(", "));
            }

            println!();
        }
    }

    if json {
        let output = serde_json::json!({"projects": json_projects});
        println!("{}", serde_json::to_string_pretty(&output)?);
    }

    Ok(())
}

fn cmd_sync(project_name: Option<String>, json: bool) -> anyhow::Result<()> {
    let config = Config::load()?;
    let manifest = Manifest::load()?;

    let projects: Vec<(&str, &Project)> = match &project_name {
        Some(name) => {
            let p = manifest
                .get_project(name)
                .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", name))?;
            vec![(name.as_str(), p)]
        }
        None => manifest
            .projects
            .iter()
            .map(|(k, v)| (k.as_str(), v))
            .collect(),
    };

    let mut total_synced = 0;
    let mut json_results = Vec::new();

    for (name, project) in projects {
        // Use project-specific index
        let mut index = Index::load_for_project(&config, name).unwrap_or_default();
        let results = scan_project(project, &index);
        let mut synced = 0;

        for r in results {
            match r.status {
                FileStatus::New | FileStatus::Drifted => {
                    if let Some(hash) = r.current_hash {
                        let abs_path = expand_path(&r.path);
                        let (size, modified) = dmcore::scanner::file_metadata(&abs_path)?;

                        let entry = dmcore::FileEntry::with_sync_now(hash, size, modified);
                        index.upsert(abs_path, entry);
                        synced += 1;
                    }
                }
                _ => {}
            }
        }

        // Save project-specific index
        index.save_for_project(&config, name)?;

        if synced > 0 {
            if !json {
                println!("{}: synced {} file(s)", name, synced);
            }
            json_results.push(serde_json::json!({"project": name, "synced": synced}));
            total_synced += synced;
        }
    }

    if json {
        let output = serde_json::json!({
            "total_synced": total_synced,
            "projects": json_results,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        if total_synced == 0 {
            println!("Nothing to sync - all files are up to date");
        } else {
            println!();
            println!("Total: {} file(s) synced", total_synced);
        }
    }

    Ok(())
}

fn cmd_backup(
    project_name: Option<String>,
    message: Option<String>,
    archive: bool,
    format: ArchiveFormat,
    password_file: Option<PathBuf>,
    password_stdin: bool,
    json: bool,
) -> anyhow::Result<()> {
    let config = Config::load()?;
    let manifest = Manifest::load()?;

    let projects: Vec<(&str, &Project)> = match &project_name {
        Some(name) => {
            let p = manifest
                .get_project(name)
                .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", name))?;
            vec![(name.as_str(), p)]
        }
        None => manifest
            .projects
            .iter()
            .map(|(k, v)| (k.as_str(), v))
            .collect(),
    };

    if projects.is_empty() {
        if json {
            println!("{}", serde_json::json!({"backed_up": 0, "unchanged": 0, "errors": 0}));
        } else {
            println!("No projects to backup.");
        }
        return Ok(());
    }

    let mut total_backed_up = 0;
    let mut total_unchanged = 0;
    let mut total_errors = 0;
    let mut json_results = Vec::new();

    for (name, project) in &projects {
        if project.files.is_empty() {
            continue;
        }

        // Initialize project-specific git repo
        init_project_repo(&config, name)?;

        // Get password if needed for this project
        let password = if project_needs_password(project) {
            Some(get_password(&password_file, password_stdin)?)
        } else {
            None
        };

        if !json {
            println!("Backing up {}...", name);
        }

        if archive {
            // Archive backup
            let archive_path = backup_archive(&config, name, project, format)?;
            if !json {
                println!("  Created archive: {}", archive_path.display());
            }
            json_results.push(serde_json::json!({
                "project": name,
                "type": "archive",
                "path": archive_path.to_string_lossy(),
                "files": project.file_count(),
            }));
            total_backed_up += project.file_count();
        } else {
            // Incremental backup with per-project store
            let result = backup_project_incremental_encrypted_with_message(
                &config,
                name,
                project,
                password.as_ref(),
                message.as_deref(),
            )?;

            if !json {
                if result.backed_up > 0 {
                    println!("  {} file(s) backed up", result.backed_up);
                }
                if result.unchanged > 0 {
                    println!("  {} file(s) unchanged (deduplicated)", result.unchanged);
                }
                if result.errors > 0 {
                    println!("  {} error(s)", result.errors);
                }
                if result.committed {
                    println!("  Committed to git");
                }
            }

            json_results.push(serde_json::json!({
                "project": name,
                "type": "incremental",
                "backed_up": result.backed_up,
                "unchanged": result.unchanged,
                "errors": result.errors,
                "committed": result.committed,
            }));

            total_backed_up += result.backed_up;
            total_unchanged += result.unchanged;
            total_errors += result.errors;
        }
    }

    if json {
        let output = serde_json::json!({
            "backed_up": total_backed_up,
            "unchanged": total_unchanged,
            "errors": total_errors,
            "projects": json_results,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!();
        println!("Backup complete:");
        println!("  Backed up:  {}", total_backed_up);
        println!("  Unchanged:  {}", total_unchanged);
        if total_errors > 0 {
            println!("  Errors:     {}", total_errors);
        }
    }

    Ok(())
}

fn cmd_restore(
    project_name: String,
    files: Vec<String>,
    dry_run: bool,
    password_file: Option<PathBuf>,
    password_stdin: bool,
    json: bool,
) -> anyhow::Result<()> {
    let config = Config::load()?;
    let manifest = Manifest::load()?;

    let project = manifest
        .get_project(&project_name)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", project_name))?;

    if project.files.is_empty() {
        if json {
            println!("{}", serde_json::json!({"restored": 0, "not_found": 0, "errors": 0}));
        } else {
            println!("Project '{}' has no tracked files.", project_name);
        }
        return Ok(());
    }

    // Load project-specific index
    let index = Index::load_for_project(&config, &project_name).unwrap_or_default();
    let store_dir = config.project_store_dir(&project_name)?;

    // Get password if needed for this project
    let password = if project_needs_password(project) {
        Some(get_password(&password_file, password_stdin)?)
    } else {
        None
    };

    // Filter files if specific ones requested
    let files_to_restore: Vec<_> = if files.is_empty() {
        project.files.iter().collect()
    } else {
        project
            .files
            .iter()
            .filter(|f| {
                files.iter().any(|req| {
                    f.path == *req || f.path.ends_with(req) || f.absolute_path().to_string_lossy().ends_with(req)
                })
            })
            .collect()
    };

    if files_to_restore.is_empty() {
        if json {
            println!("{}", serde_json::json!({"restored": 0, "not_found": 0, "errors": 0, "message": "no matching files"}));
        } else {
            println!("No matching files found to restore.");
        }
        return Ok(());
    }

    if !json {
        println!(
            "Restoring {} file(s) from project '{}'{}",
            files_to_restore.len(),
            project_name,
            if dry_run { " (dry run)" } else { "" }
        );
        println!();
    }

    let mut restored = 0;
    let mut not_found = 0;
    let mut errors = 0;
    let mut json_files = Vec::new();

    for file in files_to_restore {
        let abs_path = file.absolute_path();

        // Look up in index to get hash
        let entry = match index.get(&abs_path) {
            Some(e) => e,
            None => {
                if !json {
                    println!("  ? {} (not in index, run backup first)", file.path);
                }
                json_files.push(serde_json::json!({"path": file.path, "status": "not_indexed"}));
                not_found += 1;
                continue;
            }
        };

        if dry_run {
            let exists = abs_path.exists();
            let status = if exists { "overwrite" } else { "create" };
            if !json {
                println!("  {} {} (would {})", &entry.hash[..8], file.path, status);
            }
            json_files.push(serde_json::json!({
                "path": file.path,
                "status": "would_restore",
                "action": status,
                "hash": &entry.hash[..8],
            }));
            restored += 1;
        } else {
            // Use project-specific store with encryption support
            match retrieve_file_from_encrypted(
                &store_dir,
                &entry.hash,
                &abs_path,
                password.as_ref(),
                file.encrypted,
            ) {
                Ok(true) => {
                    if !json {
                        println!("  ✓ {}", file.path);
                    }
                    json_files.push(serde_json::json!({"path": file.path, "status": "restored"}));
                    restored += 1;
                }
                Ok(false) => {
                    if !json {
                        println!("  ✗ {} (not in store)", file.path);
                    }
                    json_files.push(serde_json::json!({"path": file.path, "status": "not_in_store"}));
                    not_found += 1;
                }
                Err(e) => {
                    if !json {
                        println!("  ✗ {} ({})", file.path, e);
                    }
                    json_files.push(serde_json::json!({"path": file.path, "status": "error", "error": e.to_string()}));
                    errors += 1;
                }
            }
        }
    }

    if json {
        let output = serde_json::json!({
            "project": project_name,
            "dry_run": dry_run,
            "restored": restored,
            "not_found": not_found,
            "errors": errors,
            "files": json_files,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!();
        if dry_run {
            println!("Dry run: {} file(s) would be restored", restored);
        } else {
            println!("Restored: {} file(s)", restored);
        }
        if not_found > 0 {
            println!("Not found: {} file(s)", not_found);
        }
        if errors > 0 {
            println!("Errors: {} file(s)", errors);
        }
    }

    Ok(())
}

fn cmd_list(verbose: bool, json: bool) -> anyhow::Result<()> {
    let config = Config::load()?;
    let manifest = Manifest::load()?;

    let mut projects: Vec<_> = manifest.projects.iter().collect();
    projects.sort_by_key(|(name, _)| name.as_str());

    if projects.is_empty() {
        if json {
            println!("{}", serde_json::json!({"projects": []}));
        } else {
            println!("No projects. Create one with: dotmatrix new <name>");
        }
        return Ok(());
    }

    let mut json_projects = Vec::new();

    for (name, project) in projects {
        if json || verbose {
            let index = Index::load_for_project(&config, name).unwrap_or_default();
            let results = scan_project(project, &index);
            let summary = ProjectSummary::from_results(&results);

            if json {
                json_projects.push(serde_json::json!({
                    "name": name,
                    "description": project.description,
                    "remote": project.remote,
                    "file_count": project.file_count(),
                    "synced": summary.synced,
                    "drifted": summary.drifted,
                    "new": summary.new,
                    "missing": summary.missing,
                }));
            } else {
                let status = if summary.is_clean() {
                    "✓"
                } else {
                    "⚠"
                };

                print!("{} {:20} {:3} files", status, name, project.file_count());

                if let Some(desc) = &project.description {
                    print!("  # {}", desc);
                }

                if summary.needs_attention() {
                    let mut parts = Vec::new();
                    if summary.drifted > 0 {
                        parts.push(format!("{} drifted", summary.drifted));
                    }
                    if summary.new > 0 {
                        parts.push(format!("{} new", summary.new));
                    }
                    print!("  ({})", parts.join(", "));
                }

                println!();
            }
        } else {
            println!("{}", name);
        }
    }

    if json {
        let output = serde_json::json!({"projects": json_projects});
        println!("{}", serde_json::to_string_pretty(&output)?);
    }

    Ok(())
}

fn cmd_info(project_name: String, json: bool) -> anyhow::Result<()> {
    let config = Config::load()?;
    let manifest = Manifest::load()?;

    let project = manifest
        .get_project(&project_name)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", project_name))?;

    // Use project-specific index
    let index = Index::load_for_project(&config, &project_name).unwrap_or_default();
    let results = scan_project(project, &index);
    let summary = ProjectSummary::from_results(&results);

    // Get git status for project
    let project_dir = config.project_dir(&project_name)?;
    let git_remote = get_remote_url(&project_dir).ok().flatten();
    let git_status = get_remote_status(&project_dir).ok();

    if json {
        let files: Vec<_> = project
            .list_files()
            .iter()
            .map(|f| {
                let status = results
                    .iter()
                    .find(|r| r.path == f.path)
                    .map(|r| r.status.description())
                    .unwrap_or("unknown");
                serde_json::json!({
                    "path": f.path,
                    "track": f.track.to_string(),
                    "encrypted": f.encrypted,
                    "status": status,
                })
            })
            .collect();

        let output = serde_json::json!({
            "name": project_name,
            "description": project.description,
            "remote": git_remote,
            "file_count": project.file_count(),
            "status": {
                "synced": summary.synced,
                "drifted": summary.drifted,
                "new": summary.new,
                "missing": summary.missing,
            },
            "git": git_status.map(|s| serde_json::json!({
                "has_remote": s.has_remote,
                "reachable": s.remote_reachable,
                "ahead": s.ahead,
                "behind": s.behind,
            })),
            "files": files,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Project: {}", project_name);

        if let Some(desc) = &project.description {
            println!("Description: {}", desc);
        }

        if let Some(remote) = &git_remote {
            println!("Git remote: {}", remote);
        }

        if let Some(status) = git_status {
            if status.has_remote {
                if status.ahead > 0 || status.behind > 0 {
                    println!("Git status: {} ahead, {} behind", status.ahead, status.behind);
                } else if status.remote_reachable {
                    println!("Git status: synced");
                }
            }
        }

        println!("Files: {}", project.file_count());

        println!();
        println!("Status:");
        println!("  Synced:  {}", summary.synced);
        println!("  Drifted: {}", summary.drifted);
        println!("  New:     {}", summary.new);
        println!("  Missing: {}", summary.missing);

        if !project.files.is_empty() {
            println!();
            println!("Files:");
            for file in project.list_files() {
                let status = results
                    .iter()
                    .find(|r| r.path == file.path)
                    .map(|r| r.status.symbol())
                    .unwrap_or("?");
                let enc = if file.encrypted { " [E]" } else { "" };
                println!("  {} {} ({}){}", status, file.path, file.track, enc);
            }
        }
    }

    Ok(())
}

fn cmd_delete(project_name: String, force: bool, json: bool) -> anyhow::Result<()> {
    let mut manifest = Manifest::load()?;

    if manifest.get_project(&project_name).is_none() {
        anyhow::bail!("Project '{}' not found", project_name);
    }

    if !force {
        if json {
            println!("{}", serde_json::json!({
                "project": project_name,
                "deleted": false,
                "message": "use --force to confirm deletion"
            }));
        } else {
            println!("Delete project '{}'? This does not delete the actual files.", project_name);
            println!("Use --force to confirm.");
        }
        return Ok(());
    }

    manifest.remove_project(&project_name);
    manifest.save()?;

    if json {
        println!("{}", serde_json::json!({
            "project": project_name,
            "deleted": true
        }));
    } else {
        println!("Deleted project: {}", project_name);
    }

    Ok(())
}

fn cmd_git(project_name: String, action: GitAction, json: bool) -> anyhow::Result<()> {
    let config = Config::load()?;
    let manifest = Manifest::load()?;

    // Verify project exists
    if manifest.get_project(&project_name).is_none() {
        anyhow::bail!("Project '{}' not found", project_name);
    }

    // Initialize project repo if needed
    let project_dir = init_project_repo(&config, &project_name)?;

    match action {
        GitAction::Remote { set } => {
            if let Some(url) = set {
                set_remote_url(&project_dir, &url)?;
                if json {
                    println!("{}", serde_json::json!({
                        "project": project_name,
                        "remote": url,
                        "action": "set"
                    }));
                } else {
                    println!("Set remote URL: {}", url);
                }
            } else {
                let remote = get_remote_url(&project_dir)?;
                if json {
                    println!("{}", serde_json::json!({
                        "project": project_name,
                        "remote": remote
                    }));
                } else if let Some(url) = remote {
                    println!("{}", url);
                } else {
                    println!("No remote configured");
                }
            }
        }
        GitAction::Push => {
            let result = push(&project_dir)?;
            if json {
                println!("{}", serde_json::json!({
                    "project": project_name,
                    "action": "push",
                    "message": result
                }));
            } else {
                println!("{}", result);
            }
        }
        GitAction::Pull => {
            let result = pull(&project_dir)?;
            if json {
                println!("{}", serde_json::json!({
                    "project": project_name,
                    "action": "pull",
                    "message": result
                }));
            } else {
                println!("{}", result);
            }
        }
        GitAction::Fetch => {
            fetch(&project_dir)?;
            if json {
                println!("{}", serde_json::json!({
                    "project": project_name,
                    "action": "fetch",
                    "success": true
                }));
            } else {
                println!("Fetched from remote");
            }
        }
        GitAction::Log { count } => {
            let commits = recent_commits(&project_dir, count)?;
            if json {
                let json_commits: Vec<_> = commits
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "hash": c.hash,
                            "short_hash": c.short_hash,
                            "message": c.message,
                            "date": c.date
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "project": project_name,
                    "commits": json_commits
                }))?);
            } else if commits.is_empty() {
                println!("No commits yet");
            } else {
                for c in commits {
                    println!("{} {} ({})", c.short_hash, c.message, c.date);
                }
            }
        }
        GitAction::Status => {
            let status = get_remote_status(&project_dir)?;
            if json {
                println!("{}", serde_json::json!({
                    "project": project_name,
                    "has_remote": status.has_remote,
                    "remote_reachable": status.remote_reachable,
                    "ahead": status.ahead,
                    "behind": status.behind,
                    "synced": status.is_synced()
                }));
            } else if !status.has_remote {
                println!("No remote configured");
            } else if !status.remote_reachable {
                println!("Remote not reachable");
            } else if status.is_synced() {
                println!("Up to date with remote");
            } else {
                if status.ahead > 0 {
                    println!("{} commit(s) ahead", status.ahead);
                }
                if status.behind > 0 {
                    println!("{} commit(s) behind", status.behind);
                }
            }
        }
    }

    Ok(())
}

fn cmd_archives(project_name: String, json: bool) -> anyhow::Result<()> {
    let config = Config::load()?;
    let manifest = Manifest::load()?;

    // Verify project exists
    if manifest.get_project(&project_name).is_none() {
        anyhow::bail!("Project '{}' not found", project_name);
    }

    let archives = list_archives(&config, &project_name)?;

    if json {
        let json_archives: Vec<_> = archives
            .iter()
            .map(|a| {
                serde_json::json!({
                    "name": a.name,
                    "path": a.path.to_string_lossy(),
                    "size": a.size,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "project": project_name,
            "count": archives.len(),
            "archives": json_archives
        }))?);
    } else if archives.is_empty() {
        println!("No archive backups for '{}'", project_name);
    } else {
        println!("Archive backups for '{}':", project_name);
        println!();
        for a in archives {
            println!("  {:>10}  {}", format_size(a.size), a.name);
        }
    }

    Ok(())
}

fn cmd_store(project_name: Option<String>, json: bool) -> anyhow::Result<()> {
    let config = Config::load()?;

    if let Some(name) = project_name {
        let manifest = Manifest::load()?;
        if manifest.get_project(&name).is_none() {
            anyhow::bail!("Project '{}' not found", name);
        }

        let store_dir = config.project_store_dir(&name)?;
        let (size, count) = calculate_store_size(&store_dir)?;

        if json {
            println!("{}", serde_json::json!({
                "project": name,
                "path": store_dir.to_string_lossy(),
                "size": size,
                "file_count": count
            }));
        } else {
            println!("Project: {}", name);
            println!("Store:   {}", store_dir.display());
            println!("Size:    {}", format_size(size));
            println!("Files:   {}", count);
        }
    } else {
        // Show stats for all projects
        let manifest = Manifest::load()?;
        let mut total_size = 0u64;
        let mut total_count = 0usize;
        let mut json_stores = Vec::new();

        for (name, _) in &manifest.projects {
            let store_dir = config.project_store_dir(name)?;
            let (size, count) = calculate_store_size(&store_dir)?;
            total_size += size;
            total_count += count;

            if json {
                json_stores.push(serde_json::json!({
                    "project": name,
                    "size": size,
                    "file_count": count
                }));
            } else if count > 0 {
                println!("{:20} {:>10}  {} files", name, format_size(size), count);
            }
        }

        if json {
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "total_size": total_size,
                "total_files": total_count,
                "projects": json_stores
            }))?);
        } else {
            println!();
            println!("Total: {} in {} files", format_size(total_size), total_count);
        }
    }

    Ok(())
}

fn cmd_tui() -> anyhow::Result<()> {
    println!("TUI not yet integrated. Run dotmatrix-tui separately.");
    Ok(())
}

fn cmd_gui() -> anyhow::Result<()> {
    println!("GUI not yet integrated. Run dotmatrix-gui separately.");
    Ok(())
}

/// Get password from file, stdin, environment, or interactive prompt
fn get_password(
    password_file: &Option<PathBuf>,
    password_stdin: bool,
) -> anyhow::Result<SecretString> {
    // Priority: --password-stdin > --password-file > DOTMATRIX_PASSWORD env > interactive prompt
    if password_stdin {
        let mut pass = String::new();
        std::io::stdin().lock().read_line(&mut pass)?;
        return Ok(SecretString::from(pass.trim().to_string()));
    }

    if let Some(path) = password_file {
        let pass = std::fs::read_to_string(path)?;
        return Ok(SecretString::from(pass.trim().to_string()));
    }

    if let Ok(pass) = std::env::var("DOTMATRIX_PASSWORD") {
        return Ok(SecretString::from(pass));
    }

    // Interactive prompt
    let pass = rpassword::prompt_password("Encryption password: ")?;
    Ok(SecretString::from(pass))
}

/// Calculate total size and file count for a store directory
fn calculate_store_size(store_dir: &Path) -> anyhow::Result<(u64, usize)> {
    if !store_dir.exists() {
        return Ok((0, 0));
    }

    let mut total_size = 0u64;
    let mut file_count = 0usize;

    fn walk(dir: &Path, size: &mut u64, count: &mut usize) -> anyhow::Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk(&path, size, count)?;
            } else {
                *size += entry.metadata()?.len();
                *count += 1;
            }
        }
        Ok(())
    }

    walk(store_dir, &mut total_size, &mut file_count)?;
    Ok((total_size, file_count))
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}
