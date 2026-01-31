use chrono::{Local, TimeZone, Utc};
use clap::{Parser, Subcommand};
use dotmatrix::config::{BackupMode, Config, TrackedPattern};
use dotmatrix::index::{FileEntry, Index};
use dotmatrix::scanner::{self, Verbosity};
use dotmatrix::tui;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;
use tar::Builder;

#[derive(Parser)]
#[command(name = "dotmatrix")]
#[command(author, version, about = "Dotfile management and versioning", long_about = None)]
struct Cli {
    /// Increase verbosity (-v for verbose, -vv for debug)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

/// Convert CLI verbosity count to Verbosity enum
fn get_verbosity(count: u8) -> Verbosity {
    match count {
        0 => Verbosity::Normal,
        1 => Verbosity::Verbose,
        _ => Verbosity::Debug,
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize dotmatrix configuration and storage
    Init,
    /// Add files or patterns to tracking
    Add { patterns: Vec<String> },
    /// Scan tracked files and update index
    Scan {
        #[arg(short, long)]
        yes: bool, // Auto-confirm cleanup without prompting
    },
    /// Backup tracked files to storage
    Backup {
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Restore files from storage
    Restore {
        /// Restore from specific git commit
        #[arg(short, long)]
        commit: Option<String>,
        /// Show what would be restored without making changes
        #[arg(long)]
        dry_run: bool,
        /// Auto-confirm restore without prompting
        #[arg(short, long)]
        yes: bool,
        /// Show diff for each file
        #[arg(long)]
        diff: bool,
        /// Restore only specific file(s)
        #[arg(short, long)]
        file: Option<Vec<String>>,
        /// Extract all files to a directory (preserving structure)
        #[arg(long)]
        extract_to: Option<String>,
        /// Remap home directory (e.g., --remap /home/olduser=/home/newuser)
        #[arg(long)]
        remap: Option<String>,
    },
    /// Show status of tracked files
    Status {
        /// Show all files including unchanged
        #[arg(short, long)]
        all: bool,
        /// Quick mode: compare by size/mtime only (skip hash)
        #[arg(short, long)]
        quick: bool,
        /// Output as JSON for scripting
        #[arg(long)]
        json: bool,
    },
    /// List all tracked files
    List,
    /// Remove files from tracking
    Remove { patterns: Vec<String> },
    /// Launch interactive TUI
    Tui,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let verbosity = get_verbosity(cli.verbose);

    match cli.command {
        Commands::Init => cmd_init()?,
        Commands::Add { patterns } => cmd_add(patterns)?,
        Commands::Scan { yes } => cmd_scan(yes, verbosity)?,
        Commands::Backup { message } => cmd_backup(message, verbosity)?,
        Commands::Restore { commit, dry_run, yes, diff, file, extract_to, remap } => {
            cmd_restore(commit, dry_run, yes, diff, file, extract_to, remap, verbosity)?
        }
        Commands::Status { all, quick, json } => cmd_status(all, quick, json, verbosity)?,
        Commands::List => cmd_list()?,
        Commands::Remove { patterns } => cmd_remove(patterns)?,
        Commands::Tui => cmd_tui()?,
    }

    Ok(())
}

/// Get git config value (global or local)
fn get_git_config(key: &str) -> Option<String> {
    Command::new("git")
        .args(["config", "--global", key])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if value.is_empty() {
                    None
                } else {
                    Some(value)
                }
            } else {
                None
            }
        })
}

/// Set git config value in a specific directory
fn set_git_config(data_dir: &PathBuf, key: &str, value: &str) -> anyhow::Result<()> {
    Command::new("git")
        .args(["config", key, value])
        .current_dir(data_dir)
        .output()?;
    Ok(())
}

/// Prompt user for input with a default value
fn prompt_with_default(prompt: &str, default: Option<&str>) -> String {
    if let Some(def) = default {
        print!("{} [{}]: ", prompt, def);
    } else {
        print!("{}: ", prompt);
    }
    std::io::Write::flush(&mut std::io::stdout()).ok();

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    let input = input.trim().to_string();

    if input.is_empty() {
        default.unwrap_or("").to_string()
    } else {
        input
    }
}

fn cmd_init() -> anyhow::Result<()> {
    println!("Initializing dotmatrix...\n");

    // Get paths
    let config_dir = dotmatrix::get_config_dir()?;
    let config_path = dotmatrix::get_config_path()?;
    let data_dir = dotmatrix::get_data_dir()?;
    let storage_path = dotmatrix::get_storage_path()?;
    let archives_path = dotmatrix::get_archives_path()?;
    let index_path = dotmatrix::get_index_path()?;

    // Create directories
    fs::create_dir_all(&config_dir)?;
    fs::create_dir_all(&data_dir)?;
    fs::create_dir_all(&storage_path)?;
    fs::create_dir_all(&archives_path)?;

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

    // Initialize git repository
    let git_dir = data_dir.join(".git");
    if !git_dir.exists() {
        println!("\n📦 Setting up git repository...");

        let output = Command::new("git")
            .args(["init"])
            .current_dir(&data_dir)
            .output()?;

        if output.status.success() {
            println!("✓ Initialized git repository");

            // Check for global git config
            let global_name = get_git_config("user.name");
            let global_email = get_git_config("user.email");

            // Prompt for git identity if not configured globally
            let (name, email) = if let (Some(n), Some(e)) = (&global_name, &global_email) {
                println!("✓ Using git identity from global config");
                (n.clone(), e.clone())
            } else {
                println!("\n📝 Git identity not found in global config.");
                println!("   Please provide your details for version control:\n");

                let n = prompt_with_default("   Name", global_name.as_deref());
                let e = prompt_with_default("   Email", global_email.as_deref());

                if n.is_empty() || e.is_empty() {
                    println!("\n⚠️  Git identity not configured. Commits will fail.");
                    println!("   Run 'git config' in {} to fix.", data_dir.display());
                }
                (n, e)
            };

            // Set local git config
            if !name.is_empty() && !email.is_empty() {
                set_git_config(&data_dir, "user.name", &name)?;
                set_git_config(&data_dir, "user.email", &email)?;
                println!("✓ Git identity configured: {} <{}>", name, email);
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("⚠️  Git init failed: {}", stderr.trim());
        }
    } else {
        println!("✓ Git repository already exists");
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
        let already_tracked = config.tracked_files.iter().any(|p| p.path() == pattern);
        if !already_tracked {
            config.tracked_files.push(TrackedPattern::simple(pattern));
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

fn cmd_scan(auto_yes: bool, verbosity: Verbosity) -> anyhow::Result<()> {
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
    let pattern_strings = config.pattern_strings();
    if verbosity >= Verbosity::Verbose {
        println!("Finding files matching patterns...");
        for pattern in &config.tracked_files {
            println!("  Pattern: {}", pattern);
        }
        println!();
    }

    let files = scanner::scan_patterns_with_verbosity(
        &pattern_strings,
        &config.exclude,
        verbosity,
    )?;

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
            print!(
                "Remove these {} entries from the index? [y/N] ",
                orphaned.len()
            );
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
            println!(
                "\n✓ Cleaned up {} orphaned entries from index.",
                orphaned.len()
            );
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

/// Get storage path for a file based on its hash (content-addressed)
fn get_file_storage_path(hash: &str) -> anyhow::Result<PathBuf> {
    let storage = dotmatrix::get_storage_path()?;
    // Use first 2 chars of hash as subdirectory for organization
    Ok(storage.join(&hash[0..2]).join(hash))
}

/// Run git commit in the data directory
fn git_commit(data_dir: &PathBuf, message: String, file_count: usize) -> anyhow::Result<()> {
    println!("\n📦 Committing to git...");

    let git_dir = data_dir.join(".git");

    // Initialize git repo if needed
    if !git_dir.exists() {
        let output = Command::new("git")
            .args(["init"])
            .current_dir(data_dir)
            .output()?;

        if output.status.success() {
            println!("   ✓ Initialized git repository");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("   ⚠️  Git init failed: {}", stderr.trim());
        }
    }

    // Stage all changes
    let output = Command::new("git")
        .args(["add", "."])
        .current_dir(data_dir)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("   ⚠️  Git add failed: {}", stderr.trim());
    }

    // Create commit
    let commit_msg = if message.is_empty() {
        format!("Backup: {} files", file_count)
    } else {
        message
    };

    let output = Command::new("git")
        .args(["commit", "-m", &commit_msg])
        .current_dir(data_dir)
        .output()?;

    if output.status.success() {
        println!("   ✓ Committed: {}", commit_msg);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("nothing to commit") || stderr.contains("nothing to commit") {
            println!("   ✓ Nothing new to commit");
        } else {
            println!("   ⚠️  Git commit failed: {}", stderr.trim());
        }
    }

    Ok(())
}

/// Backup using content-addressed storage (incremental mode)
fn backup_incremental(
    files: &[PathBuf],
    index: &mut Index,
    index_path: &PathBuf,
    data_dir: &PathBuf,
    message: Option<String>,
    git_enabled: bool,
) -> anyhow::Result<()> {
    println!("Mode: incremental (content-addressed)\n");

    let mut backed_up = 0;
    let mut unchanged = 0;
    let mut errors = 0;

    for file in files {
        print!("Backing up: {} ... ", file.display());
        std::io::Write::flush(&mut std::io::stdout()).ok();

        match scanner::scan_file(file) {
            Ok(entry) => {
                let storage_path = get_file_storage_path(&entry.hash)?;

                // Check if file already exists in storage (deduplication)
                let needs_copy = !storage_path.exists();

                // Check if file changed since last index
                let is_changed = if let Some(old_entry) = index.get_file(file) {
                    old_entry.hash != entry.hash
                } else {
                    true // New file
                };

                if needs_copy {
                    if let Some(parent) = storage_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(file, &storage_path)?;
                }

                index.add_file(file.clone(), entry);

                if is_changed {
                    if needs_copy {
                        println!("✓ backed up");
                    } else {
                        println!("✓ backed up (deduplicated)");
                    }
                    backed_up += 1;
                } else {
                    println!("✓ unchanged");
                    unchanged += 1;
                }
            }
            Err(e) => {
                println!("❌ {}", e);
                errors += 1;
            }
        }
    }

    index.save(index_path)?;

    if git_enabled {
        let msg = message.unwrap_or_else(|| {
            format!(
                "Backup: {} files ({} new/changed, {} unchanged)",
                backed_up + unchanged,
                backed_up,
                unchanged
            )
        });
        git_commit(data_dir, msg, backed_up + unchanged)?;
    }

    println!("\n📊 Backup complete:");
    println!("   Backed up: {}", backed_up);
    println!("   Unchanged: {}", unchanged);
    if errors > 0 {
        println!("   Errors: {}", errors);
    }
    println!("\n✓ Index saved to: {}", index_path.display());

    Ok(())
}

/// Backup using compressed tarball (archive mode)
fn backup_archive(
    files: &[PathBuf],
    index: &mut Index,
    index_path: &PathBuf,
    data_dir: &PathBuf,
    message: Option<String>,
    git_enabled: bool,
) -> anyhow::Result<()> {
    println!("Mode: archive (compressed tarball)\n");

    let archives_dir = dotmatrix::get_archives_path()?;
    fs::create_dir_all(&archives_dir)?;

    // Generate timestamped filename
    let timestamp = Local::now().format("%Y-%m-%d-%H%M%S");
    let archive_name = format!("backup-{}.tar.gz", timestamp);
    let archive_path = archives_dir.join(&archive_name);

    println!("Creating archive: {}\n", archive_name);

    // Create gzipped tar archive
    let tar_gz = File::create(&archive_path)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(enc);

    let mut archived = 0;
    let mut errors = 0;

    for file in files {
        print!("Adding: {} ... ", file.display());
        std::io::Write::flush(&mut std::io::stdout()).ok();

        match scanner::scan_file(file) {
            Ok(entry) => {
                // Strip leading / to make path relative for tar
                let archive_path_name = file
                    .to_string_lossy()
                    .trim_start_matches('/')
                    .to_string();
                match tar.append_path_with_name(file, &archive_path_name) {
                    Ok(_) => {
                        println!("✓");
                        index.add_file(file.clone(), entry);
                        archived += 1;
                    }
                    Err(e) => {
                        println!("❌ {}", e);
                        errors += 1;
                    }
                }
            }
            Err(e) => {
                println!("❌ {}", e);
                errors += 1;
            }
        }
    }

    // Finish the archive
    let enc = tar.into_inner()?;
    enc.finish()?;

    // Update symlink to latest
    let latest_link = archives_dir.join("latest.tar.gz");
    if latest_link.exists() || latest_link.is_symlink() {
        fs::remove_file(&latest_link).ok();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&archive_name, &latest_link).ok();

    index.save(index_path)?;

    // Get archive size
    let archive_size = fs::metadata(&archive_path)
        .map(|m| m.len())
        .unwrap_or(0);
    let size_str = if archive_size > 1024 * 1024 {
        format!("{:.1} MB", archive_size as f64 / (1024.0 * 1024.0))
    } else if archive_size > 1024 {
        format!("{:.1} KB", archive_size as f64 / 1024.0)
    } else {
        format!("{} bytes", archive_size)
    };

    if git_enabled {
        let msg = message.unwrap_or_else(|| format!("Archive backup: {}", archive_name));
        git_commit(data_dir, msg, archived)?;
    }

    println!("\n📊 Archive complete:");
    println!("   Files archived: {}", archived);
    if errors > 0 {
        println!("   Errors: {}", errors);
    }
    println!("   Archive size: {}", size_str);
    println!("\n✓ Archive saved to: {}", archive_path.display());

    Ok(())
}

/// Check if a file path matches a glob pattern (with ~ expansion)
fn path_matches_pattern(file: &Path, pattern: &str) -> bool {
    let expanded_pattern = if let Some(stripped) = pattern.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(stripped).to_string_lossy().to_string()
        } else {
            pattern.to_string()
        }
    } else {
        pattern.to_string()
    };

    // Use glob pattern matching
    if let Ok(glob_pattern) = glob::Pattern::new(&expanded_pattern) {
        glob_pattern.matches_path(file)
    } else {
        // Fallback to exact match
        file.to_string_lossy() == expanded_pattern
    }
}

/// Determine the effective backup mode for a file based on matching patterns
fn get_file_mode(file: &Path, config: &Config) -> BackupMode {
    // Check patterns in reverse order (later patterns override earlier ones)
    for pattern in config.tracked_files.iter().rev() {
        if path_matches_pattern(file, pattern.path()) {
            return config.mode_for_pattern(pattern);
        }
    }
    // Default to global backup mode
    config.backup_mode
}

fn cmd_backup(message: Option<String>, verbosity: Verbosity) -> anyhow::Result<()> {
    println!("Creating backup...\n");

    let config_path = dotmatrix::get_config_path()?;
    let index_path = dotmatrix::get_index_path()?;
    let data_dir = dotmatrix::get_data_dir()?;

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

    let pattern_strings = config.pattern_strings();
    let files = scanner::scan_patterns_with_verbosity(
        &pattern_strings,
        &config.exclude,
        verbosity,
    )?;

    if files.is_empty() {
        println!("⚠️  No files found matching tracked patterns.");
        println!("   Run 'dotmatrix add <pattern>' to track files first.");
        return Ok(());
    }

    // Group files by their effective backup mode
    let mut incremental_files: Vec<PathBuf> = Vec::new();
    let mut archive_files: Vec<PathBuf> = Vec::new();

    for file in files {
        match get_file_mode(&file, &config) {
            BackupMode::Archive => archive_files.push(file),
            BackupMode::Incremental => incremental_files.push(file),
        }
    }

    let total_files = incremental_files.len() + archive_files.len();
    println!("Found {} files to backup.", total_files);

    if !incremental_files.is_empty() && !archive_files.is_empty() {
        println!(
            "   {} files (incremental), {} files (archive)\n",
            incremental_files.len(),
            archive_files.len()
        );
    } else {
        println!();
    }

    // Backup incremental files first
    if !incremental_files.is_empty() {
        backup_incremental(
            &incremental_files,
            &mut index,
            &index_path,
            &data_dir,
            None, // Don't commit yet
            false, // Don't commit yet
        )?;
    }

    // Then backup archive files
    if !archive_files.is_empty() {
        backup_archive(
            &archive_files,
            &mut index,
            &index_path,
            &data_dir,
            None, // Don't commit yet
            false, // Don't commit yet
        )?;
    }

    // Single git commit at the end
    if config.git_enabled {
        let msg = message.unwrap_or_else(|| format!("Backup: {} files", total_files));
        git_commit(&data_dir, msg, total_files)?;
    }

    Ok(())
}

/// Format file size for human-readable display
fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

/// Format unix timestamp for display
fn format_time(unix_ts: u64) -> String {
    Utc.timestamp_opt(unix_ts as i64, 0)
        .single()
        .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Shorten path for display (replace home with ~)
fn display_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rel) = path.strip_prefix(&home) {
            return format!("~/{}", rel.display());
        }
    }
    path.display().to_string()
}

/// Comparison info for a file
struct FileComparison {
    path: PathBuf,          // Original path in backup
    dest_path: PathBuf,     // Destination path (after remap/extract_to)
    current_exists: bool,
    current_size: Option<u64>,
    current_mtime: Option<u64>,
    current_hash: Option<String>,
    backup_size: u64,
    backup_mtime: u64,
    backup_hash: String,
}

impl FileComparison {
    fn is_identical(&self) -> bool {
        if let Some(ref current_hash) = self.current_hash {
            current_hash == &self.backup_hash
        } else {
            false
        }
    }

    fn current_is_newer(&self) -> bool {
        if let Some(current_mtime) = self.current_mtime {
            current_mtime > self.backup_mtime
        } else {
            false
        }
    }
}

/// Create safety backup of current files before restore
fn create_restore_backup(files: &[&FileComparison]) -> anyhow::Result<Option<PathBuf>> {
    // Only backup files that exist (at destination path)
    let existing: Vec<_> = files.iter().filter(|f| f.current_exists).collect();
    if existing.is_empty() {
        return Ok(None);
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let timestamp = Local::now().format("%Y%m%d-%H%M%S");
    let backup_dir = home.join(format!(".dotmatrix-restore-backup-{}", timestamp));

    fs::create_dir_all(&backup_dir)?;

    for comp in existing {
        // Preserve directory structure in backup (use dest_path since that's where the file is)
        let rel_path = comp.dest_path.to_string_lossy().trim_start_matches('/').to_string();
        let dest = backup_dir.join(&rel_path);

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(&comp.dest_path, &dest)?;
    }

    Ok(Some(backup_dir))
}

/// Show diff between current file and backup content
fn show_file_diff(current_path: &Path, backup_hash: &str) -> anyhow::Result<()> {
    let storage_path = get_file_storage_path(backup_hash)?;

    if !storage_path.exists() {
        println!("   (backup file not found in storage)");
        return Ok(());
    }

    if !current_path.exists() {
        println!("   (current file does not exist - will be created)");
        return Ok(());
    }

    // Use system diff command
    let output = Command::new("diff")
        .args([
            "-u",
            "--color=auto",
            &current_path.to_string_lossy(),
            &storage_path.to_string_lossy(),
        ])
        .output();

    match output {
        Ok(out) => {
            if out.stdout.is_empty() && out.stderr.is_empty() {
                println!("   (files are identical)");
            } else {
                println!("{}", String::from_utf8_lossy(&out.stdout));
                if !out.stderr.is_empty() {
                    eprintln!("{}", String::from_utf8_lossy(&out.stderr));
                }
            }
        }
        Err(_) => {
            println!("   (diff command not available)");
        }
    }

    Ok(())
}

/// Parse remap option (format: /old/path=/new/path)
fn parse_remap(remap: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = remap.splitn(2, '=').collect();
    if parts.len() == 2 {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
}

/// Apply path remapping for restore
fn remap_path(path: &Path, remap: Option<&(String, String)>, extract_to: Option<&Path>) -> PathBuf {
    let path_str = path.to_string_lossy();

    // First apply remap if specified
    let remapped = if let Some((from, to)) = remap {
        if path_str.starts_with(from) {
            PathBuf::from(path_str.replacen(from, to, 1))
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    };

    // Then apply extract_to if specified
    if let Some(base) = extract_to {
        // Strip leading / and join with extract_to base
        let rel_path = remapped.to_string_lossy().trim_start_matches('/').to_string();
        base.join(rel_path)
    } else {
        remapped
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_restore(
    _commit: Option<String>,
    dry_run: bool,
    auto_yes: bool,
    show_diff: bool,
    filter_files: Option<Vec<String>>,
    extract_to: Option<String>,
    remap: Option<String>,
    _verbosity: Verbosity,
) -> anyhow::Result<()> {
    println!("Preparing restore...\n");

    // Parse remap option
    let remap_pair = remap.as_ref().and_then(|r| {
        let parsed = parse_remap(r);
        if parsed.is_none() {
            eprintln!("⚠️  Invalid remap format. Use: --remap /old/path=/new/path");
        }
        parsed
    });

    // Parse extract_to path
    let extract_path = extract_to.map(|p| {
        let expanded = if let Some(stripped) = p.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(stripped)
            } else {
                PathBuf::from(p)
            }
        } else {
            PathBuf::from(p)
        };
        expanded
    });

    if let Some(ref path) = extract_path {
        println!("📂 Extract destination: {}", path.display());
    }
    if let Some((ref from, ref to)) = remap_pair {
        println!("🔄 Path remapping: {} → {}", from, to);
    }
    if extract_path.is_some() || remap_pair.is_some() {
        println!();
    }

    let config_path = dotmatrix::get_config_path()?;
    let index_path = dotmatrix::get_index_path()?;

    if !config_path.exists() {
        println!("❌ No config file found. Run 'dotmatrix init' first.");
        return Ok(());
    }

    if !index_path.exists() {
        println!("❌ No index found. Run 'dotmatrix backup' first.");
        return Ok(());
    }

    let config = Config::load(&config_path)?;
    let index = Index::load(&index_path)?;

    if index.files.is_empty() {
        println!("⚠️  No files in backup index.");
        println!("   Run 'dotmatrix backup' to create a backup first.");
        return Ok(());
    }

    // Filter files if --file specified
    let entries: Vec<&FileEntry> = if let Some(ref patterns) = filter_files {
        index
            .files
            .values()
            .filter(|e| {
                let path_str = e.path.to_string_lossy();
                patterns.iter().any(|p| path_str.contains(p))
            })
            .collect()
    } else {
        index.files.values().collect()
    };

    if entries.is_empty() {
        println!("⚠️  No matching files found in backup.");
        return Ok(());
    }

    // Build comparison list
    let mut comparisons: Vec<FileComparison> = Vec::new();

    for entry in &entries {
        // Calculate destination path (with remap/extract_to applied)
        let dest_path = remap_path(
            &entry.path,
            remap_pair.as_ref(),
            extract_path.as_deref(),
        );

        // Check if destination exists (not original path)
        let current_exists = dest_path.exists();
        let (current_size, current_mtime, current_hash) = if current_exists {
            let meta = fs::metadata(&dest_path)?;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            let hash = scanner::hash_file(&dest_path).ok();
            (Some(meta.len()), mtime, hash)
        } else {
            (None, None, None)
        };

        comparisons.push(FileComparison {
            path: entry.path.clone(),
            dest_path,
            current_exists,
            current_size,
            current_mtime,
            current_hash,
            backup_size: entry.size,
            backup_mtime: entry.last_modified,
            backup_hash: entry.hash.clone(),
        });
    }

    // Filter out identical files
    let to_restore: Vec<_> = comparisons.iter().filter(|c| !c.is_identical()).collect();

    if to_restore.is_empty() {
        println!("✓ All files already match backup (nothing to restore).");
        return Ok(());
    }

    // Display comparison
    println!("The following files will be restored:\n");

    let mut warnings = 0;
    for comp in &to_restore {
        // Show original path and destination if different
        if comp.path != comp.dest_path {
            println!("{}", display_path(&comp.path));
            println!("  → {}", display_path(&comp.dest_path));
        } else {
            println!("{}", display_path(&comp.path));
        }

        if comp.current_exists {
            let newer_marker = if comp.current_is_newer() {
                warnings += 1;
                " [NEWER]"
            } else {
                ""
            };
            println!(
                "  Current:  {}  ({}){}",
                format_time(comp.current_mtime.unwrap_or(0)),
                format_size(comp.current_size.unwrap_or(0)),
                newer_marker
            );
        } else {
            println!("  Current:  (does not exist)");
        }

        println!(
            "  Backup:   {}  ({})",
            format_time(comp.backup_mtime),
            format_size(comp.backup_size)
        );

        if comp.current_is_newer() {
            println!("  ⚠️  Current file is NEWER than backup!");
        } else if !comp.current_exists {
            println!("  ✓ Will create new file");
        }

        // Show diff if requested
        if show_diff {
            println!("\n  --- Diff ---");
            show_file_diff(&comp.dest_path, &comp.backup_hash)?;
        }

        println!();
    }

    // Summary
    println!("📊 Summary:");
    println!("   Files to restore: {}", to_restore.len());
    if warnings > 0 {
        println!(
            "   ⚠️  {} file(s) where current is NEWER than backup",
            warnings
        );
    }

    // Dry run stops here
    if dry_run {
        println!("\n🔍 Dry run complete. No files were modified.");
        return Ok(());
    }

    // Confirmation
    let proceed = if auto_yes {
        println!("\nAuto-confirming restore (--yes flag)...");
        true
    } else {
        print!("\nRestore {} files? [y/N/d(iff)] ", to_restore.len());
        std::io::Write::flush(&mut std::io::stdout()).ok();

        let mut response = String::new();
        std::io::stdin().read_line(&mut response).ok();
        let response = response.trim().to_lowercase();

        if response == "d" || response == "diff" {
            // Show diffs and ask again
            println!("\n--- Showing diffs ---\n");
            for comp in &to_restore {
                println!("{}:", display_path(&comp.dest_path));
                show_file_diff(&comp.dest_path, &comp.backup_hash)?;
                println!();
            }

            print!("\nRestore {} files? [y/N] ", to_restore.len());
            std::io::Write::flush(&mut std::io::stdout()).ok();

            let mut response2 = String::new();
            std::io::stdin().read_line(&mut response2).ok();
            response2.trim().to_lowercase() == "y" || response2.trim().to_lowercase() == "yes"
        } else {
            response == "y" || response == "yes"
        }
    };

    if !proceed {
        println!("\n❌ Restore cancelled.");
        return Ok(());
    }

    // Create safety backup
    println!("\n📦 Creating safety backup of current files...");
    match create_restore_backup(&to_restore) {
        Ok(Some(backup_path)) => {
            println!("   ✓ Current files backed up to: {}", backup_path.display());
        }
        Ok(None) => {
            println!("   ✓ No existing files to backup");
        }
        Err(e) => {
            println!("   ⚠️  Failed to create safety backup: {}", e);
            println!("   Continue anyway? [y/N] ");
            std::io::Write::flush(&mut std::io::stdout()).ok();

            let mut response = String::new();
            std::io::stdin().read_line(&mut response).ok();
            if response.trim().to_lowercase() != "y" {
                println!("❌ Restore cancelled.");
                return Ok(());
            }
        }
    }

    // Perform restore
    println!("\n📥 Restoring files...\n");

    let mut restored = 0;
    let mut errors = 0;

    for comp in &to_restore {
        // Show destination path if different from original
        if comp.path != comp.dest_path {
            print!("Restoring: {} → {} ... ",
                display_path(&comp.path),
                display_path(&comp.dest_path)
            );
        } else {
            print!("Restoring: {} ... ", display_path(&comp.dest_path));
        }
        std::io::Write::flush(&mut std::io::stdout()).ok();

        // Get backup file from storage
        let storage_path = get_file_storage_path(&comp.backup_hash)?;

        if !storage_path.exists() {
            // Try archive mode
            if config.backup_mode == BackupMode::Archive {
                println!("❌ Archive restore not yet implemented");
                errors += 1;
                continue;
            } else {
                println!("❌ Backup file not found in storage");
                errors += 1;
                continue;
            }
        }

        // Create parent directory if needed (use dest_path)
        if let Some(parent) = comp.dest_path.parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent) {
                    println!("❌ Failed to create directory: {}", e);
                    errors += 1;
                    continue;
                }
            }
        }

        // Copy from storage to destination
        match fs::copy(&storage_path, &comp.dest_path) {
            Ok(_) => {
                println!("✓");
                restored += 1;
            }
            Err(e) => {
                println!("❌ {}", e);
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    println!("   💡 Try running with sudo for system files");
                }
                errors += 1;
            }
        }
    }

    // Summary
    println!("\n📊 Restore complete:");
    println!("   Restored: {}", restored);
    if errors > 0 {
        println!("   Errors: {}", errors);
    }

    Ok(())
}

/// File status for comparison
#[derive(Debug, Clone)]
enum FileStatus {
    Unchanged,
    Modified,
    New,
    Deleted,
}

/// Status entry for a file
#[derive(Debug)]
struct StatusEntry {
    path: PathBuf,
    status: FileStatus,
    current_size: Option<u64>,
    backup_size: Option<u64>,
}

fn cmd_status(show_all: bool, quick_mode: bool, json_output: bool, verbosity: Verbosity) -> anyhow::Result<()> {
    let config_path = dotmatrix::get_config_path()?;
    let index_path = dotmatrix::get_index_path()?;

    if !config_path.exists() {
        if json_output {
            println!("{{\"error\": \"No config file found\"}}");
        } else {
            println!("❌ No config file found. Run 'dotmatrix init' first.");
        }
        return Ok(());
    }

    let config = Config::load(&config_path)?;
    let index = if index_path.exists() {
        Index::load(&index_path)?
    } else {
        Index::new()
    };

    if !json_output {
        if quick_mode {
            println!("📊 Dotmatrix Status (quick mode - size/mtime only)\n");
        } else {
            println!("📊 Dotmatrix Status\n");
        }
    }

    // Find all current tracked files
    // Use Quiet verbosity for JSON output to avoid mixing stderr with JSON
    let pattern_strings = config.pattern_strings();
    let scan_verbosity = if json_output { Verbosity::Quiet } else { verbosity };
    let current_files = scanner::scan_patterns_with_verbosity(
        &pattern_strings,
        &config.exclude,
        scan_verbosity,
    )?;
    let current_set: std::collections::HashSet<_> = current_files.iter().cloned().collect();

    let mut entries: Vec<StatusEntry> = Vec::new();

    // Check files in current patterns
    for file in &current_files {
        if let Some(backup_entry) = index.get_file(file) {
            // File exists in both current and backup
            if !file.exists() {
                // File was deleted
                entries.push(StatusEntry {
                    path: file.clone(),
                    status: FileStatus::Deleted,
                    current_size: None,
                    backup_size: Some(backup_entry.size),
                });
            } else {
                // Check if modified
                let is_modified = if quick_mode {
                    // Quick mode: compare size and mtime
                    let meta = fs::metadata(file)?;
                    let current_size = meta.len();
                    let current_mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);

                    current_size != backup_entry.size || current_mtime != backup_entry.last_modified
                } else {
                    // Full mode: compare hash
                    match scanner::hash_file(file) {
                        Ok(hash) => hash != backup_entry.hash,
                        Err(_) => true, // Assume modified if can't hash
                    }
                };

                let current_size = fs::metadata(file).map(|m| m.len()).ok();

                if is_modified {
                    entries.push(StatusEntry {
                        path: file.clone(),
                        status: FileStatus::Modified,
                        current_size,
                        backup_size: Some(backup_entry.size),
                    });
                } else {
                    entries.push(StatusEntry {
                        path: file.clone(),
                        status: FileStatus::Unchanged,
                        current_size,
                        backup_size: Some(backup_entry.size),
                    });
                }
            }
        } else {
            // File is new (not in backup)
            let current_size = fs::metadata(file).map(|m| m.len()).ok();
            entries.push(StatusEntry {
                path: file.clone(),
                status: FileStatus::New,
                current_size,
                backup_size: None,
            });
        }
    }

    // Check for deleted files (in backup but not in current patterns)
    for (path, entry) in &index.files {
        if !current_set.contains(path) && !entries.iter().any(|e| &e.path == path) {
            entries.push(StatusEntry {
                path: path.clone(),
                status: FileStatus::Deleted,
                current_size: None,
                backup_size: Some(entry.size),
            });
        }
    }

    // Sort entries by path
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    // Count by status
    let modified: Vec<_> = entries
        .iter()
        .filter(|e| matches!(e.status, FileStatus::Modified))
        .collect();
    let new_files: Vec<_> = entries
        .iter()
        .filter(|e| matches!(e.status, FileStatus::New))
        .collect();
    let deleted: Vec<_> = entries
        .iter()
        .filter(|e| matches!(e.status, FileStatus::Deleted))
        .collect();
    let unchanged: Vec<_> = entries
        .iter()
        .filter(|e| matches!(e.status, FileStatus::Unchanged))
        .collect();

    if json_output {
        // JSON output
        let json = serde_json::json!({
            "modified": modified.iter().map(|e| e.path.to_string_lossy()).collect::<Vec<_>>(),
            "new": new_files.iter().map(|e| e.path.to_string_lossy()).collect::<Vec<_>>(),
            "deleted": deleted.iter().map(|e| e.path.to_string_lossy()).collect::<Vec<_>>(),
            "unchanged_count": unchanged.len(),
            "summary": {
                "modified": modified.len(),
                "new": new_files.len(),
                "deleted": deleted.len(),
                "unchanged": unchanged.len(),
                "total": entries.len()
            }
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    // Human-readable output
    let has_changes = !modified.is_empty() || !new_files.is_empty() || !deleted.is_empty();

    if !modified.is_empty() {
        println!("Modified files:");
        for entry in &modified {
            let size_info = match (entry.current_size, entry.backup_size) {
                (Some(cur), Some(bak)) if cur != bak => {
                    format!(" ({} → {})", format_size(bak), format_size(cur))
                }
                _ => String::new(),
            };
            println!("  M  {}{}", display_path(&entry.path), size_info);
        }
        println!();
    }

    if !new_files.is_empty() {
        println!("New files (not yet backed up):");
        for entry in &new_files {
            let size_info = entry
                .current_size
                .map(|s| format!(" ({})", format_size(s)))
                .unwrap_or_default();
            println!("  +  {}{}", display_path(&entry.path), size_info);
        }
        println!();
    }

    if !deleted.is_empty() {
        println!("Deleted files (in backup but missing):");
        for entry in &deleted {
            println!("  -  {}", display_path(&entry.path));
        }
        println!();
    }

    if show_all && !unchanged.is_empty() {
        println!("Unchanged files:");
        for entry in &unchanged {
            println!("  ✓  {}", display_path(&entry.path));
        }
        println!();
    }

    // Summary
    if !has_changes {
        println!("✓ All {} files up to date with backup.", unchanged.len());
    } else {
        println!(
            "Summary: {} modified, {} new, {} deleted, {} unchanged",
            modified.len(),
            new_files.len(),
            deleted.len(),
            unchanged.len()
        );

        if !modified.is_empty() || !new_files.is_empty() {
            println!("\nRun 'dotmatrix backup' to save changes.");
        }
    }

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
        if let Some(pos) = config.tracked_files.iter().position(|x| x.path() == pattern) {
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

fn cmd_tui() -> anyhow::Result<()> {
    let config_path = dotmatrix::get_config_path()?;
    let index_path = dotmatrix::get_index_path()?;

    if !config_path.exists() {
        println!("❌ No config file found. Run 'dotmatrix init' first.");
        return Ok(());
    }

    let config = Config::load(&config_path)?;
    let index = if index_path.exists() {
        Index::load(&index_path)?
    } else {
        Index::new()
    };

    tui::run(config, index, config_path, index_path)
}
