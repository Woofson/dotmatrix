//! dmcli - CLI for dotmatrix
//!
//! Full-featured command-line interface for project management.
//! Designed for automation, scripting, and power users.

use clap::{Parser, Subcommand, ValueEnum};
use dmcore::{
    backup_archive, backup_incremental, contract_path, expand_path, init_repo, retrieve_file,
    scan_project, stage_all, commit, ArchiveFormat, Config, FileStatus, Index,
    Manifest, Project, ProjectSummary, TrackMode, TrackedFile,
};
use std::path::Path;

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

    /// Launch TUI
    Tui,

    /// Launch GUI
    Gui,
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
        Commands::Init => cmd_init()?,
        Commands::New { name, description } => cmd_new(name, description)?,
        Commands::Add {
            project,
            files,
            track,
            encrypted,
        } => cmd_add(project, files, track.into(), encrypted)?,
        Commands::Remove { project, files } => cmd_remove(project, files)?,
        Commands::Status { project, changes } => cmd_status(project, changes)?,
        Commands::Sync { project } => cmd_sync(project)?,
        Commands::Backup {
            project,
            message,
            archive,
            format,
        } => cmd_backup(project, message, archive, format.into())?,
        Commands::Restore {
            project,
            files,
            dry_run,
        } => cmd_restore(project, files, dry_run)?,
        Commands::List { verbose } => cmd_list(verbose)?,
        Commands::Info { project } => cmd_info(project)?,
        Commands::Delete { project, force } => cmd_delete(project, force)?,
        Commands::Tui => cmd_tui()?,
        Commands::Gui => cmd_gui()?,
    }

    Ok(())
}

fn cmd_init() -> anyhow::Result<()> {
    println!("dotmatrix 2.0.0 - project compositor with git versioning");
    println!();

    let config = Config::load()?;
    config.save()?;

    let manifest = Manifest::load()?;
    manifest.save()?;

    let index = Index::load()?;
    index.save()?;

    // Initialize git repo in data directory
    let data_dir = config.data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    init_repo(&data_dir)?;

    println!("Config:   {}", Config::config_path()?.display());
    println!("Manifest: {}", Manifest::manifest_path()?.display());
    println!("Index:    {}", Index::index_path()?.display());
    println!("Data:     {}", config.data_dir()?.display());
    println!();
    println!("Ready. Create a project with: dotmatrix new <name>");

    Ok(())
}

fn cmd_new(name: String, description: Option<String>) -> anyhow::Result<()> {
    let mut manifest = Manifest::load()?;

    if manifest.get_project(&name).is_some() {
        anyhow::bail!("Project '{}' already exists", name);
    }

    let project = match description {
        Some(desc) => Project::with_description(desc),
        None => Project::new(),
    };

    manifest.add_project(name.clone(), project);
    manifest.save()?;

    println!("Created project: {}", name);
    println!("Add files with: dotmatrix add {} <files...>", name);

    Ok(())
}

fn cmd_add(
    project_name: String,
    files: Vec<String>,
    track: TrackMode,
    encrypted: bool,
) -> anyhow::Result<()> {
    let mut manifest = Manifest::load()?;

    let project = manifest
        .get_project_mut(&project_name)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", project_name))?;

    let mut added = 0;
    let mut skipped = 0;

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
            println!("Warning: File not found: {}", abs_path.display());
            skipped += 1;
            continue;
        }

        if !abs_path.is_file() {
            println!("Warning: Not a file: {}", abs_path.display());
            skipped += 1;
            continue;
        }

        // Store with ~ for home directory paths
        let stored_path = contract_path(&abs_path);

        let mut tf = TrackedFile::with_mode(stored_path.clone(), track);
        tf.encrypted = encrypted;

        if project.add_file(tf) {
            println!("  + {} ({})", stored_path, track);
            added += 1;
        } else {
            println!("  ~ {} (already tracked)", stored_path);
            skipped += 1;
        }
    }

    manifest.save()?;

    println!();
    println!(
        "Added {} file(s) to '{}' ({} skipped)",
        added, project_name, skipped
    );

    Ok(())
}

fn cmd_remove(project_name: String, files: Vec<String>) -> anyhow::Result<()> {
    let mut manifest = Manifest::load()?;

    let project = manifest
        .get_project_mut(&project_name)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", project_name))?;

    let mut removed = 0;

    for file_path in files {
        // Try to match the path as-is first, then try expanded/contracted versions
        if project.remove_file(&file_path) {
            println!("  - {}", file_path);
            removed += 1;
        } else {
            // Try with expansion/contraction
            let abs_path = expand_path(&file_path);
            let contracted = contract_path(&abs_path);
            if project.remove_file(&contracted) {
                println!("  - {}", contracted);
                removed += 1;
            } else {
                println!("  ? {} (not found in project)", file_path);
            }
        }
    }

    manifest.save()?;

    println!();
    println!("Removed {} file(s) from '{}'", removed, project_name);

    Ok(())
}

fn cmd_status(project_name: Option<String>, changes_only: bool) -> anyhow::Result<()> {
    let manifest = Manifest::load()?;
    let index = Index::load()?;

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
        println!("No projects. Create one with: dotmatrix new <name>");
        return Ok(());
    }

    for (name, project) in projects {
        let results = scan_project(project, &index);
        let summary = ProjectSummary::from_results(&results);

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

    Ok(())
}

fn cmd_sync(project_name: Option<String>) -> anyhow::Result<()> {
    let manifest = Manifest::load()?;
    let mut index = Index::load()?;

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

    for (name, project) in projects {
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

        if synced > 0 {
            println!("{}: synced {} file(s)", name, synced);
            total_synced += synced;
        }
    }

    index.save()?;

    if total_synced == 0 {
        println!("Nothing to sync - all files are up to date");
    } else {
        println!();
        println!("Total: {} file(s) synced", total_synced);
    }

    Ok(())
}

fn cmd_backup(
    project_name: Option<String>,
    message: Option<String>,
    archive: bool,
    format: ArchiveFormat,
) -> anyhow::Result<()> {
    let config = Config::load()?;
    let manifest = Manifest::load()?;
    let mut index = Index::load()?;

    // Initialize git repo if needed
    let data_dir = config.data_dir()?;
    init_repo(&data_dir)?;

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
        println!("No projects to backup.");
        return Ok(());
    }

    let mut total_backed_up = 0;
    let mut total_unchanged = 0;
    let mut total_errors = 0;

    for (name, project) in &projects {
        if project.files.is_empty() {
            continue;
        }

        println!("Backing up {}...", name);

        if archive {
            // Archive backup
            let archive_path = backup_archive(&config, name, project, format)?;
            println!("  Created archive: {}", archive_path.display());
            total_backed_up += project.file_count();
        } else {
            // Incremental backup
            let result = backup_incremental(&config, project, &mut index)?;

            if result.backed_up > 0 {
                println!("  {} file(s) backed up", result.backed_up);
            }
            if result.unchanged > 0 {
                println!("  {} file(s) unchanged (deduplicated)", result.unchanged);
            }
            if result.errors > 0 {
                println!("  {} error(s)", result.errors);
            }

            total_backed_up += result.backed_up;
            total_unchanged += result.unchanged;
            total_errors += result.errors;
        }
    }

    // Save index
    index.save()?;

    // Git commit if incremental backup
    if !archive && (total_backed_up > 0 || total_unchanged > 0) {
        let store_dir = config.store_dir()?;
        stage_all(&store_dir)?;

        let msg = message.unwrap_or_else(|| {
            format!(
                "Backup: {} files ({} new, {} unchanged)",
                total_backed_up + total_unchanged,
                total_backed_up,
                total_unchanged
            )
        });

        if commit(&store_dir, &msg)? {
            println!();
            println!("Committed to git: {}", msg);
        }
    }

    println!();
    println!("Backup complete:");
    println!("  Backed up:  {}", total_backed_up);
    println!("  Unchanged:  {}", total_unchanged);
    if total_errors > 0 {
        println!("  Errors:     {}", total_errors);
    }

    Ok(())
}

fn cmd_restore(project_name: String, files: Vec<String>, dry_run: bool) -> anyhow::Result<()> {
    let config = Config::load()?;
    let manifest = Manifest::load()?;
    let index = Index::load()?;

    let project = manifest
        .get_project(&project_name)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", project_name))?;

    if project.files.is_empty() {
        println!("Project '{}' has no tracked files.", project_name);
        return Ok(());
    }

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
        println!("No matching files found to restore.");
        return Ok(());
    }

    println!(
        "Restoring {} file(s) from project '{}'{}",
        files_to_restore.len(),
        project_name,
        if dry_run { " (dry run)" } else { "" }
    );
    println!();

    let mut restored = 0;
    let mut not_found = 0;
    let mut errors = 0;

    for file in files_to_restore {
        let abs_path = file.absolute_path();

        // Look up in index to get hash
        let entry = match index.get(&abs_path) {
            Some(e) => e,
            None => {
                println!("  ? {} (not in index, run backup first)", file.path);
                not_found += 1;
                continue;
            }
        };

        if dry_run {
            let exists = abs_path.exists();
            let status = if exists { "overwrite" } else { "create" };
            println!("  {} {} (would {})", entry.hash[..8].to_string(), file.path, status);
            restored += 1;
        } else {
            match retrieve_file(&config, &entry.hash, &abs_path) {
                Ok(true) => {
                    println!("  ✓ {}", file.path);
                    restored += 1;
                }
                Ok(false) => {
                    println!("  ✗ {} (not in store)", file.path);
                    not_found += 1;
                }
                Err(e) => {
                    println!("  ✗ {} ({})", file.path, e);
                    errors += 1;
                }
            }
        }
    }

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

    Ok(())
}

fn cmd_list(verbose: bool) -> anyhow::Result<()> {
    let manifest = Manifest::load()?;
    let index = Index::load()?;

    let mut projects: Vec<_> = manifest.projects.iter().collect();
    projects.sort_by_key(|(name, _)| name.as_str());

    if projects.is_empty() {
        println!("No projects. Create one with: dotmatrix new <name>");
        return Ok(());
    }

    for (name, project) in projects {
        if verbose {
            let results = scan_project(project, &index);
            let summary = ProjectSummary::from_results(&results);

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
        } else {
            println!("{}", name);
        }
    }

    Ok(())
}

fn cmd_info(project_name: String) -> anyhow::Result<()> {
    let manifest = Manifest::load()?;
    let index = Index::load()?;

    let project = manifest
        .get_project(&project_name)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", project_name))?;

    println!("Project: {}", project_name);

    if let Some(desc) = &project.description {
        println!("Description: {}", desc);
    }

    if let Some(remote) = &project.remote {
        println!("Git remote: {}", remote);
    }

    println!("Files: {}", project.file_count());

    let results = scan_project(project, &index);
    let summary = ProjectSummary::from_results(&results);

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
            println!("  {} {} ({})", status, file.path, file.track);
        }
    }

    Ok(())
}

fn cmd_delete(project_name: String, force: bool) -> anyhow::Result<()> {
    let mut manifest = Manifest::load()?;

    if manifest.get_project(&project_name).is_none() {
        anyhow::bail!("Project '{}' not found", project_name);
    }

    if !force {
        println!("Delete project '{}'? This does not delete the actual files.", project_name);
        println!("Use --force to confirm.");
        return Ok(());
    }

    manifest.remove_project(&project_name);
    manifest.save()?;

    println!("Deleted project: {}", project_name);

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
