//! Git operations for the store
//!
//! Handles git initialization, commits, and status for the data directory.

use std::path::Path;
use std::process::Command;

/// Check if a directory is a git repository
pub fn is_git_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Initialize a git repository
pub fn init_repo(dir: &Path) -> anyhow::Result<()> {
    if is_git_repo(dir) {
        return Ok(());
    }

    std::fs::create_dir_all(dir)?;

    let output = Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to init git repo: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Create .gitignore for restore-backups
    let gitignore = dir.join(".gitignore");
    std::fs::write(&gitignore, "restore-backups/\n")?;

    // Initial commit
    stage_all(dir)?;
    commit(dir, "Initial dotmatrix repository")?;

    Ok(())
}

/// Stage all changes
pub fn stage_all(dir: &Path) -> anyhow::Result<()> {
    let output = Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to stage changes: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Check if there are staged changes
pub fn has_staged_changes(dir: &Path) -> anyhow::Result<bool> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(dir)
        .output()?;

    // Exit code 0 = no changes, 1 = changes
    Ok(!output.status.success())
}

/// Commit staged changes
pub fn commit(dir: &Path, message: &str) -> anyhow::Result<bool> {
    // First check if there are changes to commit
    if !has_staged_changes(dir)? {
        return Ok(false);
    }

    let output = Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(dir)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // "nothing to commit" is not an error
        if stderr.contains("nothing to commit") {
            return Ok(false);
        }
        anyhow::bail!("Failed to commit: {}", stderr);
    }

    Ok(true)
}

/// Get the current commit hash (short form)
pub fn current_commit(dir: &Path) -> anyhow::Result<Option<String>> {
    if !is_git_repo(dir) {
        return Ok(None);
    }

    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(dir)
        .output()?;

    if output.status.success() {
        Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_string()))
    } else {
        Ok(None)
    }
}

/// Get commit count
pub fn commit_count(dir: &Path) -> anyhow::Result<usize> {
    if !is_git_repo(dir) {
        return Ok(0);
    }

    let output = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(dir)
        .output()?;

    if output.status.success() {
        let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(count_str.parse().unwrap_or(0))
    } else {
        Ok(0)
    }
}

/// Get list of recent commits
pub fn recent_commits(dir: &Path, limit: usize) -> anyhow::Result<Vec<CommitInfo>> {
    if !is_git_repo(dir) {
        return Ok(Vec::new());
    }

    let output = Command::new("git")
        .args([
            "log",
            &format!("-{}", limit),
            "--format=%H|%h|%s|%ai",
        ])
        .current_dir(dir)
        .output()?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let commits = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            if parts.len() >= 4 {
                Some(CommitInfo {
                    hash: parts[0].to_string(),
                    short_hash: parts[1].to_string(),
                    message: parts[2].to_string(),
                    date: parts[3].to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(commits)
}

/// Information about a git commit
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub date: String,
}
