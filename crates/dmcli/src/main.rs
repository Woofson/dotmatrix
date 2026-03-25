//! dmcli - CLI for dotmatrix
//!
//! Full-featured command-line interface for project management.
//! Designed for automation, scripting, and power users.

use clap::{Parser, Subcommand, ValueEnum};
use dmcore::{
    contract_path, expand_path, scan_project, Config, FileStatus, Index, Manifest, Project,
    ProjectSummary, TrackMode, TrackedFile,
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

    /// Backup project files
    Backup {
        /// Project name (or all if not specified)
        project: Option<String>,
    },

    /// Restore files from backup
    Restore {
        /// Project name
        project: String,

        /// Specific files to restore (or all if not specified)
        files: Vec<String>,
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
        Commands::Backup { project } => cmd_backup(project)?,
        Commands::Restore { project, files } => cmd_restore(project, files)?,
        Commands::List { verbose } => cmd_list(verbose)?,
        Commands::Info { project } => cmd_info(project)?,
        Commands::Delete { project, force } => cmd_delete(project, force)?,
        Commands::Tui => cmd_tui()?,
        Commands::Gui => cmd_gui()?,
    }

    Ok(())
}

fn cmd_init() -> anyhow::Result<()> {
    println!("dotmatrix 2.0 - project compositor with git versioning");
    println!();

    let config = Config::load()?;
    config.save()?;

    let manifest = Manifest::load()?;
    manifest.save()?;

    let index = Index::load()?;
    index.save()?;

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

fn cmd_backup(_project_name: Option<String>) -> anyhow::Result<()> {
    println!("Backup functionality coming soon.");
    println!("For now, use 'dotmatrix sync' to track file states.");
    Ok(())
}

fn cmd_restore(_project_name: String, _files: Vec<String>) -> anyhow::Result<()> {
    println!("Restore functionality coming soon.");
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
