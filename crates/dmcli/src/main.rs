//! dmcli - CLI for dotmatrix
//!
//! Full-featured command-line interface for project management.
//! Designed for automation, scripting, and power users.

use clap::{Parser, Subcommand};

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
    },

    /// Add files to a project
    Add {
        /// Project name
        project: String,
        /// File paths to add
        files: Vec<String>,
    },

    /// Show project status
    Status {
        /// Project name (or all if not specified)
        project: Option<String>,
    },

    /// Sync drifted files to store
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
    List,

    /// Launch TUI
    Tui,

    /// Launch GUI
    Gui,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            println!("dotmatrix 2.0 - project compositor with git versioning");
            println!("Initializing...");
            let config = dmcore::Config::load()?;
            config.save()?;
            let manifest = dmcore::Manifest::load()?;
            manifest.save()?;
            println!("Initialized dotmatrix at {}", dmcore::Config::config_path()?.display());
        }
        Commands::New { name } => {
            let mut manifest = dmcore::Manifest::load()?;
            manifest.add_project(name.clone(), dmcore::Project::new());
            manifest.save()?;
            println!("Created project: {}", name);
        }
        Commands::List => {
            let manifest = dmcore::Manifest::load()?;
            let projects = manifest.list_projects();
            if projects.is_empty() {
                println!("No projects. Create one with: dotmatrix new <name>");
            } else {
                for name in projects {
                    println!("{}", name);
                }
            }
        }
        Commands::Status { project } => {
            println!("Status for {:?}", project.as_deref().unwrap_or("all projects"));
            // TODO: Implement status
        }
        Commands::Add { project, files } => {
            println!("Adding {:?} to project {}", files, project);
            // TODO: Implement add
        }
        Commands::Sync { project } => {
            println!("Syncing {:?}", project.as_deref().unwrap_or("all projects"));
            // TODO: Implement sync
        }
        Commands::Backup { project } => {
            println!("Backing up {:?}", project.as_deref().unwrap_or("all projects"));
            // TODO: Implement backup
        }
        Commands::Restore { project, files } => {
            println!("Restoring {:?} from project {}", files, project);
            // TODO: Implement restore
        }
        Commands::Tui => {
            println!("Launching TUI... (not yet implemented, run dotmatrix-tui)");
        }
        Commands::Gui => {
            println!("Launching GUI... (not yet implemented, run dotmatrix-gui)");
        }
    }

    Ok(())
}
