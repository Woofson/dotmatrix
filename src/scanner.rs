use anyhow::{Context, Result};
use glob::glob;
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::index::FileEntry;

/// Options for recursive directory scanning
#[derive(Debug, Clone, Default)]
pub struct RecursiveScanOptions {
    /// Maximum depth to recurse (None = unlimited)
    pub max_depth: Option<usize>,
    /// Additional glob patterns to exclude
    pub additional_excludes: Vec<String>,
    /// Whether to respect .gitignore files (default: true)
    pub respect_gitignore: bool,
}

impl RecursiveScanOptions {
    pub fn new() -> Self {
        Self {
            max_depth: None,
            additional_excludes: Vec::new(),
            respect_gitignore: true,
        }
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    pub fn with_excludes(mut self, excludes: Vec<String>) -> Self {
        self.additional_excludes = excludes;
        self
    }

    pub fn with_gitignore(mut self, respect: bool) -> Self {
        self.respect_gitignore = respect;
        self
    }
}

/// Result of a recursive directory scan
#[derive(Debug, Clone, Default)]
pub struct RecursiveScanResult {
    /// Files found that should be tracked
    pub files: Vec<PathBuf>,
    /// Number of directories scanned
    pub directories_scanned: usize,
    /// Number of files excluded by .gitignore
    pub gitignore_excluded: usize,
    /// Number of files excluded by config patterns
    pub config_excluded: usize,
    /// Errors encountered during scanning (path, error message)
    pub errors: Vec<(PathBuf, String)>,
}

/// Scan a directory recursively, respecting .gitignore and exclude patterns
pub fn scan_directory_recursive(
    dir: &Path,
    config_excludes: &[String],
    options: &RecursiveScanOptions,
) -> Result<RecursiveScanResult> {
    let mut result = RecursiveScanResult::default();

    // Build the walker
    let mut builder = WalkBuilder::new(dir);

    // Configure gitignore handling
    builder.git_ignore(options.respect_gitignore);
    builder.git_global(options.respect_gitignore);
    builder.git_exclude(options.respect_gitignore);

    // Set max depth if specified
    if let Some(depth) = options.max_depth {
        builder.max_depth(Some(depth));
    }

    // Note: Additional excludes are handled in the loop below since the ignore
    // crate's pattern handling is for files, not glob patterns directly

    // Track gitignore-excluded files by running a second pass without gitignore
    let mut all_files_count: usize = 0;
    if options.respect_gitignore {
        let mut no_ignore_builder = WalkBuilder::new(dir);
        no_ignore_builder.git_ignore(false);
        no_ignore_builder.git_global(false);
        no_ignore_builder.git_exclude(false);
        if let Some(depth) = options.max_depth {
            no_ignore_builder.max_depth(Some(depth));
        }
        for entry in no_ignore_builder.build() {
            if let Ok(e) = entry {
                if e.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    all_files_count += 1;
                }
            }
        }
    }

    // Walk the directory
    let mut files_before_config_exclude = 0;
    for entry in builder.build() {
        match entry {
            Ok(entry) => {
                let path = entry.path();

                // Track directories
                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    result.directories_scanned += 1;
                    continue;
                }

                // Skip non-files
                if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    continue;
                }

                files_before_config_exclude += 1;

                // Check against additional excludes
                let should_exclude = options.additional_excludes.iter().any(|pattern| {
                    if let Ok(glob_pattern) = glob::Pattern::new(pattern) {
                        glob_pattern.matches_path(path)
                    } else {
                        false
                    }
                });

                if should_exclude {
                    result.config_excluded += 1;
                    continue;
                }

                // Check against config excludes
                if is_excluded(path, config_excludes) {
                    result.config_excluded += 1;
                    continue;
                }

                result.files.push(path.to_path_buf());
            }
            Err(e) => {
                // ignore::Error doesn't have a direct path method, extract from error message
                result.errors.push((PathBuf::new(), e.to_string()));
            }
        }
    }

    // Calculate gitignore exclusions
    if options.respect_gitignore {
        result.gitignore_excluded = all_files_count.saturating_sub(files_before_config_exclude);
    }

    // Sort files for consistent output
    result.files.sort();

    Ok(result)
}

/// Find all .gitignore files that apply to a directory
pub fn find_gitignore_files(dir: &Path) -> Vec<PathBuf> {
    let mut gitignores = Vec::new();

    // Check for .gitignore in the directory itself
    let local_gitignore = dir.join(".gitignore");
    if local_gitignore.exists() {
        gitignores.push(local_gitignore);
    }

    // Walk up to find parent .gitignore files (up to home directory)
    let home = dirs::home_dir();
    let mut current = dir.parent();
    while let Some(parent) = current {
        let gitignore = parent.join(".gitignore");
        if gitignore.exists() {
            gitignores.push(gitignore);
        }
        // Stop at home directory
        if home.as_ref() == Some(&parent.to_path_buf()) {
            break;
        }
        current = parent.parent();
    }

    gitignores
}

/// Expand a path with ~ to the user's home directory
pub fn expand_tilde(path: &str) -> Result<PathBuf> {
    if let Some(rest) = path.strip_prefix("~/") {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        Ok(home.join(rest))
    } else {
        Ok(PathBuf::from(path))
    }
}

/// Check if a path matches any exclude pattern
pub fn is_excluded(path: &Path, exclude_patterns: &[String]) -> bool {
    let path_str = path.to_string_lossy();

    for pattern in exclude_patterns {
        // Expand pattern if it contains ~
        let expanded_pattern = if let Some(rest) = pattern.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(rest).to_string_lossy().to_string()
            } else {
                pattern.clone()
            }
        } else {
            pattern.clone()
        };

        // Use glob pattern matching
        if let Ok(pattern_obj) = glob::Pattern::new(&expanded_pattern) {
            if pattern_obj.matches(&path_str) {
                return true;
            }
        }
    }

    false
}

/// Calculate SHA256 hash of a file
pub fn hash_file(path: &Path) -> Result<String> {
    let mut file =
        File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192]; // 8KB buffer for efficient reading

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Get file metadata and create a FileEntry
pub fn scan_file(path: &Path) -> Result<FileEntry> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("Failed to read metadata: {}", path.display()))?;

    let size = metadata.len();

    let last_modified = metadata
        .modified()
        .with_context(|| format!("Failed to get modification time: {}", path.display()))?
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let hash = hash_file(path)?;

    Ok(FileEntry {
        path: path.to_path_buf(),
        hash,
        last_modified,
        size,
    })
}

/// Scan a pattern and return all matching files (excluding those in exclude list)
pub fn scan_pattern(pattern: &str, exclude_patterns: &[String]) -> Result<Vec<PathBuf>> {
    let expanded = expand_tilde(pattern)?;
    let mut pattern_str = expanded.to_string_lossy().to_string();

    // If pattern ends with /**, append /* to actually match files
    // glob's ** matches directories, but we need to match files inside them
    if pattern_str.ends_with("/**") {
        pattern_str.push_str("/*");
    }

    let mut files = Vec::new();

    // If the pattern has no glob characters, treat it as a literal path
    if !pattern_str.contains('*') && !pattern_str.contains('?') && !pattern_str.contains('[') {
        let path = PathBuf::from(&pattern_str);
        if path.exists() {
            if path.is_file() && !is_excluded(&path, exclude_patterns) {
                files.push(path);
            } else if path.is_dir() {
                // If it's a directory without glob, skip it
                // User should use pattern like "path/**" to include directory contents
                return Err(anyhow::anyhow!(
                    "Path is a directory: {}. Use '{}/**' to track directory contents.",
                    path.display(),
                    path.display()
                ));
            }
        } else {
            return Err(anyhow::anyhow!("File not found: {}", path.display()));
        }
    } else {
        // Use glob to find matching files
        for entry in
            glob(&pattern_str).with_context(|| format!("Invalid glob pattern: {}", pattern_str))?
        {
            match entry {
                Ok(path) => {
                    if path.is_file() && !is_excluded(&path, exclude_patterns) {
                        files.push(path);
                    }
                }
                Err(e) => {
                    eprintln!("⚠️  Warning: Failed to read path: {}", e);
                }
            }
        }
    }

    Ok(files)
}

/// Verbosity level for scanning operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Verbosity {
    Quiet,   // No output
    #[default]
    Normal,  // Only errors/warnings
    Verbose, // Show patterns being processed
    Debug,   // Show all files found
}

/// Scan multiple patterns and return all matching files
pub fn scan_patterns(patterns: &[String], exclude_patterns: &[String]) -> Result<Vec<PathBuf>> {
    scan_patterns_with_verbosity(patterns, exclude_patterns, Verbosity::Normal)
}

/// Scan multiple patterns with specified verbosity level
pub fn scan_patterns_with_verbosity(
    patterns: &[String],
    exclude_patterns: &[String],
    verbosity: Verbosity,
) -> Result<Vec<PathBuf>> {
    let mut all_files = Vec::new();
    let mut errors = Vec::new();

    for pattern in patterns {
        if verbosity >= Verbosity::Verbose {
            eprintln!("Scanning pattern: {}", pattern);
        }
        match scan_pattern(pattern, exclude_patterns) {
            Ok(mut files) => {
                if verbosity >= Verbosity::Verbose {
                    eprintln!("  Found {} files", files.len());
                }
                if verbosity >= Verbosity::Debug {
                    for f in &files {
                        eprintln!("    {}", f.display());
                    }
                }
                all_files.append(&mut files);
            }
            Err(e) => {
                if verbosity >= Verbosity::Verbose {
                    eprintln!("  Error: {}", e);
                }
                errors.push(format!("Pattern '{}': {}", pattern, e));
            }
        }
    }

    // Remove duplicates (in case patterns overlap)
    all_files.sort();
    all_files.dedup();

    // Report errors but don't fail completely (unless quiet)
    if !errors.is_empty() && verbosity >= Verbosity::Normal {
        eprintln!("⚠️  Some patterns had errors:");
        for error in &errors {
            eprintln!("   {}", error);
        }
    }

    Ok(all_files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        let result = expand_tilde("~/test.txt").unwrap();
        assert!(result.to_string_lossy().contains("test.txt"));
        assert!(!result.to_string_lossy().contains("~"));
    }

    #[test]
    fn test_expand_tilde_no_home() {
        let result = expand_tilde("/etc/test.txt").unwrap();
        assert_eq!(result, PathBuf::from("/etc/test.txt"));
    }

    #[test]
    fn test_is_excluded() {
        let exclude = vec!["**/*.log".to_string(), "**/.DS_Store".to_string()];

        assert!(is_excluded(Path::new("/home/user/test.log"), &exclude));
        assert!(is_excluded(Path::new("/home/user/.DS_Store"), &exclude));
        assert!(!is_excluded(Path::new("/home/user/test.txt"), &exclude));
    }
}
