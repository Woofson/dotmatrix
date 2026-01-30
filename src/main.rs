use clap::{Parser, Subcommand};
use dotmatrix::config::Config;
use dotmatrix::index::Index;
use dotmatrix::scanner;
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
    Scan {
        #[arg(short, long)]
        yes: bool,  // Auto-confirm cleanup without prompting
    },
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
        Commands::Scan { yes } => cmd_scan(yes)?,
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
        println!("✓ Created config at: {}", config_path.display());
    } else {
        println!("✓ Config already exists at: {}", config_path.display());
    }

    // Create empty index if it doesn't exist
    if !index_path.exists() {
        let index = Index::new();
        index.save(&index_path)?;
        println!("✓ Created index at: {}", index_path.display());
    } else {
        println!("✓ Index already exists at: {}", index_path.display());
    }

    println!("\n🎬 Dotmatrix initialized successfully!");
    println!("   Config directory: {}", config_dir.display());
    println!("   Data directory: {}", data_dir.display());
    println!("   Storage directory: {}", storage_path.display());
    println!("\nNext steps:");
    println!("   1. Edit your config: {}", config_path.display());
    println!("   2. Run 'dotmatrix scan' to index your files");

    Ok(())
}

fn cmd_add(patterns: Vec<String>) -> anyhow::Result<()> {
    let config_path = dotmatrix::get_config_path()?;
    
    if !config_path.exists() {
        println!("❌ No config file found. Run 'dotmatrix init' first.");
        return Ok(());
    }
    
    // Warn if too many patterns (likely shell expansion)
    if patterns.len() > 10 {
        println!("⚠️  Warning: You passed {} file paths!", patterns.len());
        println!("   Did your shell expand a glob pattern like ~/.config/**?");
        println!("   If so, use quotes to prevent expansion:");
        println!("   dotmatrix add '~/.config/nvim/**'\n");
        println!("   Press Ctrl+C to cancel, or wait 3 seconds to continue...");
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    
    let mut config = Config::load(&config_path)?;
    
    // Add new patterns (avoid duplicates)
    let mut added = 0;
    for pattern in &patterns {
        if !config.tracked_files.contains(pattern) {
            config.tracked_files.push(pattern.clone());
            println!("✓ Added: {}", pattern);
            added += 1;
        } else {
            println!("⚠️  Already tracked: {}", pattern);
        }
    }
    
    if added > 0 {
        config.save(&config_path)?;
        println!("\n✓ Config updated! Added {} pattern(s).", added);
        println!("Run 'dotmatrix scan' to index the files.");
    } else {
        println!("\n⚠️  No new patterns added (all already tracked).");
    }
    
    Ok(())
}

fn cmd_scan(auto_yes: bool) -> anyhow::Result<()> {
    println!("Scanning tracked files...\n");
    
    let config_path = dotmatrix::get_config_path()?;
    let index_path = dotmatrix::get_index_path()?;
    
    if !config_path.exists() {
        println!("❌ No config file found. Run 'dotmatrix init' first.");
        return Ok(());
    }
    
    let config = Config::load(&config_path)?;
    let mut index = if index_path.exists() {
        Index::load(&index_path)?
    } else {
        Index::new()
    };
    
    // Find all files matching patterns
    println!("Finding files matching patterns...");
    for pattern in &config.tracked_files {
        println!("  Pattern: {}", pattern);
    }
    println!();
    
    let files = scanner::scan_patterns(&config.tracked_files, &config.exclude)?;
    
    if files.is_empty() {
        println!("\n⚠️  No files found matching tracked patterns.");
        println!("   Check your config at: {}", config_path.display());
        return Ok(());
    }
    
    println!("Found {} files to scan.\n", files.len());
    
    // Scan each file
    let mut scanned = 0;
    let mut updated = 0;
    let mut new_files = 0;
    let mut errors = 0;
    
    for file in &files {
        print!("Scanning: {} ... ", file.display());
        std::io::Write::flush(&mut std::io::stdout()).ok();
        
        match scanner::scan_file(file) {
            Ok(entry) => {
                // Check if file is new or changed
                let is_new = !index.files.contains_key(file);
                let is_changed = if let Some(old_entry) = index.get_file(file) {
                    old_entry.hash != entry.hash
                } else {
                    false
                };
                
                index.add_file(file.clone(), entry);
                
                if is_new {
                    println!("✓ NEW");
                    new_files += 1;
                } else if is_changed {
                    println!("✓ UPDATED");
                    updated += 1;
                } else {
                    println!("✓ unchanged");
                }
                
                scanned += 1;
            }
            Err(e) => {
                println!("❌ {}", e);
                errors += 1;
            }
        }
    }
    
    // Save updated index
    index.save(&index_path)?;
    
    // Check for orphaned files (in index but don't match current patterns)
    let current_paths: std::collections::HashSet<_> = files.iter().cloned().collect();
    let mut orphaned = Vec::new();
    
    for path in index.files.keys() {
        if !current_paths.contains(path) {
            orphaned.push(path.clone());
        }
    }
    
    if !orphaned.is_empty() {
        println!("\n⚠️  Found {} orphaned entries in index:", orphaned.len());
        println!("   These files are tracked in the index but no longer match any pattern in your config.\n");
        
        // Show a sample of orphaned files (first 10)
        let show_count = orphaned.len().min(10);
        for path in orphaned.iter().take(show_count) {
            println!("   • {}", path.display());
        }
        if orphaned.len() > show_count {
            println!("   ... and {} more", orphaned.len() - show_count);
        }
        
        println!("\n📝 What this means:");
        println!("   - These files were previously tracked");
        println!("   - They no longer match patterns in ~/.config/dotmatrix/config.toml");
        println!("   - They will be removed from the index (not from your disk!)");
        println!("   - Your actual files are safe and unchanged\n");
        
        let should_remove = if auto_yes {
            println!("Auto-confirming cleanup (--yes flag)...");
            true
        } else {
            print!("Remove these {} entries from the index? [y/N] ", orphaned.len());
            std::io::Write::flush(&mut std::io::stdout()).ok();
            
            let mut response = String::new();
            std::io::stdin().read_line(&mut response).ok();
            
            response.trim().to_lowercase() == "y" || response.trim().to_lowercase() == "yes"
        };
        
        if should_remove {
            for path in &orphaned {
                index.remove_file(path);
            }
            index.save(&index_path)?;
            println!("\n✓ Cleaned up {} orphaned entries from index.", orphaned.len());
        } else {
            println!("\n⚠️  Skipped cleanup. Orphaned entries remain in the index.");
            println!("   (They won't be scanned or backed up, but take up space in index.json)");
        }
    }
    
    // Print summary
    println!("\n📊 Scan complete:");
    println!("   Total files: {}", scanned);
    if new_files > 0 {
        println!("   New files: {}", new_files);
    }
    if updated > 0 {
        println!("   Updated files: {}", updated);
    }
    if errors > 0 {
        println!("   Errors: {}", errors);
    }
    println!("\n✓ Index saved to: {}", index_path.display());

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
        println!("❌ No config file found. Run 'dotmatrix init' first.");
        return Ok(());
    }

    let config = Config::load(&config_path)?;
    
    println!("📋 Tracked file patterns:");
    for pattern in &config.tracked_files {
        println!("   {}", pattern);
    }
    
    println!("\n🚫 Exclude patterns:");
    for pattern in &config.exclude {
        println!("   {}", pattern);
    }

    Ok(())
}

fn cmd_remove(patterns: Vec<String>) -> anyhow::Result<()> {
    let config_path = dotmatrix::get_config_path()?;
    
    if !config_path.exists() {
        println!("❌ No config file found. Run 'dotmatrix init' first.");
        return Ok(());
    }
    
    let mut config = Config::load(&config_path)?;
    
    // Remove patterns
    let mut removed = 0;
    for pattern in &patterns {
        if let Some(pos) = config.tracked_files.iter().position(|x| x == pattern) {
            config.tracked_files.remove(pos);
            println!("✓ Removed from tracking: {}", pattern);
            removed += 1;
        } else {
            println!("⚠️  Not tracked: {}", pattern);
        }
    }
    
    if removed > 0 {
        config.save(&config_path)?;
        println!("\n✓ Config updated! Removed {} pattern(s).", removed);
        println!("\nNote: Files are still in the index.");
        println!("Run 'dotmatrix scan' to update the index and remove untracked files.");
    } else {
        println!("\n⚠️  No patterns were removed.");
    }
    
    Ok(())
}
