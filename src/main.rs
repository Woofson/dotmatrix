use clap::{Parser, Subcommand};
use dotmatrix::config::Config;
use dotmatrix::index::Index;
use std::fs;

#[derive(Parser)]
#[command(name = "dotmatrix")]
#[command(author, version, about = "Dotfile management and versioning", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize dotmatrix configuration and storage
    Init,
    /// Add files or patterns to tracking
    Add {
        patterns: Vec<String>,
    },
    /// Scan tracked files and update index
    Scan,
    /// Backup tracked files to storage
    Backup {
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Restore files from storage
    Restore {
        #[arg(short, long)]
        commit: Option<String>,
    },
    /// Show status of tracked files
    Status,
    /// List all tracked files
    List,
    /// Remove files from tracking
    Remove {
        patterns: Vec<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => cmd_init()?,
        Commands::Add { patterns } => cmd_add(patterns)?,
        Commands::Scan => cmd_scan()?,
        Commands::Backup { message } => cmd_backup(message)?,
        Commands::Restore { commit } => cmd_restore(commit)?,
        Commands::Status => cmd_status()?,
        Commands::List => cmd_list()?,
        Commands::Remove { patterns } => cmd_remove(patterns)?,
    }

    Ok(())
}

fn cmd_init() -> anyhow::Result<()> {
    println!("Initializing dotmatrix...");

    // Get paths
    let config_dir = dotmatrix::get_config_dir()?;
    let config_path = dotmatrix::get_config_path()?;
    let data_dir = dotmatrix::get_data_dir()?;
    let storage_path = dotmatrix::get_storage_path()?;
    let index_path = dotmatrix::get_index_path()?;

    // Create directories
    fs::create_dir_all(&config_dir)?;
    fs::create_dir_all(&data_dir)?;
    fs::create_dir_all(&storage_path)?;

    // Create default config if it doesn't exist
    if !config_path.exists() {
        let config = Config::default();
        config.save(&config_path)?;
        println!("Created config at: {}", config_path.display());
    } else {
        println!("Config already exists at: {}", config_path.display());
    }

    // Create empty index if it doesn't exist
    if !index_path.exists() {
        let index = Index::new();
        index.save(&index_path)?;
        println!("Created index at: {}", index_path.display());
    } else {
        println!("Index already exists at: {}", index_path.display());
    }

    println!("\nDotmatrix initialized successfully!");
    println!("Config directory: {}", config_dir.display());
    println!("Data directory: {}", data_dir.display());
    println!("Storage directory: {}", storage_path.display());

    Ok(())
}

fn cmd_add(patterns: Vec<String>) -> anyhow::Result<()> {
    println!("Adding patterns: {:?}", patterns);
    println!("TODO: Implement add command");
    Ok(())
}

fn cmd_scan() -> anyhow::Result<()> {
    println!("Scanning tracked files...");
    println!("TODO: Implement scan command");
    Ok(())
}

fn cmd_backup(message: Option<String>) -> anyhow::Result<()> {
    println!("Creating backup...");
    if let Some(msg) = message {
        println!("Message: {}", msg);
    }
    println!("TODO: Implement backup command");
    Ok(())
}

fn cmd_restore(commit: Option<String>) -> anyhow::Result<()> {
    println!("Restoring files...");
    if let Some(c) = commit {
        println!("From commit: {}", c);
    }
    println!("TODO: Implement restore command");
    Ok(())
}

fn cmd_status() -> anyhow::Result<()> {
    println!("Checking status...");
    println!("TODO: Implement status command");
    Ok(())
}

fn cmd_list() -> anyhow::Result<()> {
    let config_path = dotmatrix::get_config_path()?;
    
    if !config_path.exists() {
        println!("No config file found. Run 'dotmatrix init' first.");
        return Ok(());
    }

    let config = Config::load(&config_path)?;
    
    println!("Tracked files:");
    for pattern in &config.tracked_files {
        println!("  {}", pattern);
    }

    Ok(())
}

fn cmd_remove(patterns: Vec<String>) -> anyhow::Result<()> {
    println!("Removing patterns: {:?}", patterns);
    println!("TODO: Implement remove command");
    Ok(())
}
