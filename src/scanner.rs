use anyhow::{Context, Result};
use glob::glob;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::index::FileEntry;

/// Expand a path with ~ to the user's home directory
pub fn expand_tilde(path: &str) -> Result<PathBuf> {
    if path.starts_with("~/") {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        Ok(home.join(&path[2..]))
    } else {
        Ok(PathBuf::from(path))
    }
}

/// Check if a path matches any exclude pattern
pub fn is_excluded(path: &Path, exclude_patterns: &[String]) -> bool {
    let path_str = path.to_string_lossy();
    
    for pattern in exclude_patterns {
        // Expand pattern if it contains ~
        let expanded_pattern = if pattern.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(&pattern[2..]).to_string_lossy().to_string()
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
    let mut file = File::open(path)
        .with_context(|| format!("Failed to open file: {}", path.display()))?;
    
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192]; // 8KB buffer for efficient reading
    
    loop {
        let bytes_read = file.read(&mut buffer)
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
    
    let last_modified = metadata.modified()
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
        for entry in glob(&pattern_str)
            .with_context(|| format!("Invalid glob pattern: {}", pattern_str))?
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

/// Scan multiple patterns and return all matching files
pub fn scan_patterns(patterns: &[String], exclude_patterns: &[String]) -> Result<Vec<PathBuf>> {
    let mut all_files = Vec::new();
    let mut errors = Vec::new();
    
    for pattern in patterns {
        eprintln!("DEBUG: Processing pattern: {}", pattern);
        match scan_pattern(pattern, exclude_patterns) {
            Ok(mut files) => {
                eprintln!("DEBUG:   Found {} files", files.len());
                for f in &files {
                    eprintln!("DEBUG:     - {}", f.display());
                }
                all_files.append(&mut files);
            }
            Err(e) => {
                eprintln!("DEBUG:   Error: {}", e);
                errors.push(format!("Pattern '{}': {}", pattern, e));
            }
        }
    }
    
    // Remove duplicates (in case patterns overlap)
    all_files.sort();
    all_files.dedup();
    
    // Report errors but don't fail completely
    if !errors.is_empty() {
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
        let exclude = vec![
            "**/*.log".to_string(),
            "**/.DS_Store".to_string(),
        ];
        
        assert!(is_excluded(Path::new("/home/user/test.log"), &exclude));
        assert!(is_excluded(Path::new("/home/user/.DS_Store"), &exclude));
        assert!(!is_excluded(Path::new("/home/user/test.txt"), &exclude));
    }
}
