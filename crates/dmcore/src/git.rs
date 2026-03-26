//! Git operations for the store
//!
//! Handles git initialization, commits, and status for the data directory.

use std::path::Path;
use std::process::Command;

use crate::config::Config;

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

    // Initialize with 'main' as the default branch
    let output = Command::new("git")
        .args(["init", "-b", "main"])
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

/// Get the configured remote URL
pub fn get_remote_url(dir: &Path) -> anyhow::Result<Option<String>> {
    if !is_git_repo(dir) {
        return Ok(None);
    }

    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(dir)
        .output()?;

    if output.status.success() {
        Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ))
    } else {
        Ok(None)
    }
}

/// Set the remote URL (creates or updates origin)
pub fn set_remote_url(dir: &Path, url: &str) -> anyhow::Result<()> {
    if !is_git_repo(dir) {
        anyhow::bail!("Not a git repository");
    }

    // Check if origin exists
    let check = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(dir)
        .output()?;

    if check.status.success() {
        // Update existing remote
        let output = Command::new("git")
            .args(["remote", "set-url", "origin", url])
            .current_dir(dir)
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to set remote URL: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    } else {
        // Add new remote
        let output = Command::new("git")
            .args(["remote", "add", "origin", url])
            .current_dir(dir)
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to add remote: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    Ok(())
}

/// Pull from remote
pub fn pull(dir: &Path) -> anyhow::Result<String> {
    if !is_git_repo(dir) {
        anyhow::bail!("Not a git repository");
    }

    let output = Command::new("git")
        .args(["pull", "--rebase"])
        .current_dir(dir)
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.contains("Already up to date") || stdout.contains("Already up-to-date") {
            Ok("Already up to date".to_string())
        } else {
            Ok("Pull successful".to_string())
        }
    } else {
        anyhow::bail!(
            "Pull failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Remote repository status
#[derive(Debug, Clone, Default)]
pub struct RemoteStatus {
    /// Number of commits ahead of remote
    pub ahead: usize,
    /// Number of commits behind remote
    pub behind: usize,
    /// Whether a remote is configured
    pub has_remote: bool,
    /// Whether the remote is reachable
    pub remote_reachable: bool,
}

impl RemoteStatus {
    /// Check if local and remote are in sync
    pub fn is_synced(&self) -> bool {
        self.has_remote && self.remote_reachable && self.ahead == 0 && self.behind == 0
    }
}

/// Fetch from remote (updates tracking refs)
pub fn fetch(dir: &Path) -> anyhow::Result<()> {
    if !is_git_repo(dir) {
        anyhow::bail!("Not a git repository");
    }

    let output = Command::new("git")
        .args(["fetch", "--quiet"])
        .current_dir(dir)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to fetch: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Get number of commits ahead of remote
pub fn commits_ahead(dir: &Path) -> anyhow::Result<usize> {
    if !is_git_repo(dir) {
        return Ok(0);
    }

    // Get the upstream branch
    let output = Command::new("git")
        .args(["rev-list", "--count", "@{upstream}..HEAD"])
        .current_dir(dir)
        .output()?;

    if output.status.success() {
        let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(count_str.parse().unwrap_or(0))
    } else {
        Ok(0)
    }
}

/// Get number of commits behind remote
pub fn commits_behind(dir: &Path) -> anyhow::Result<usize> {
    if !is_git_repo(dir) {
        return Ok(0);
    }

    let output = Command::new("git")
        .args(["rev-list", "--count", "HEAD..@{upstream}"])
        .current_dir(dir)
        .output()?;

    if output.status.success() {
        let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(count_str.parse().unwrap_or(0))
    } else {
        Ok(0)
    }
}

/// Get the remote status for a repository
pub fn get_remote_status(dir: &Path) -> anyhow::Result<RemoteStatus> {
    let mut status = RemoteStatus::default();

    if !is_git_repo(dir) {
        return Ok(status);
    }

    // Check if remote exists
    if get_remote_url(dir)?.is_none() {
        return Ok(status);
    }
    status.has_remote = true;

    // Try to fetch (will fail if remote unreachable)
    if fetch(dir).is_ok() {
        status.remote_reachable = true;

        // Get ahead/behind counts
        status.ahead = commits_ahead(dir).unwrap_or(0);
        status.behind = commits_behind(dir).unwrap_or(0);
    }

    Ok(status)
}

/// Push to remote
pub fn push(dir: &Path) -> anyhow::Result<String> {
    if !is_git_repo(dir) {
        anyhow::bail!("Not a git repository");
    }

    // First try regular push
    let output = Command::new("git")
        .args(["push"])
        .current_dir(dir)
        .output()?;

    if output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.contains("Everything up-to-date") {
            Ok("Everything up-to-date".to_string())
        } else {
            Ok("Push successful".to_string())
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Check if we need to set upstream
        if stderr.contains("no upstream branch") || stderr.contains("has no upstream") {
            let output = Command::new("git")
                .args(["push", "-u", "origin", "HEAD"])
                .current_dir(dir)
                .output()?;

            if output.status.success() {
                Ok("Push successful (set upstream)".to_string())
            } else {
                anyhow::bail!(
                    "Push failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        } else {
            anyhow::bail!("Push failed: {}", stderr);
        }
    }
}

/// Initialize a git repository for a specific project
///
/// Creates the project directory structure and initializes git if needed.
/// Returns the project directory path.
pub fn init_project_repo(config: &Config, project_name: &str) -> anyhow::Result<std::path::PathBuf> {
    let project_dir = config.project_dir(project_name)?;
    std::fs::create_dir_all(&project_dir)?;

    if !is_git_repo(&project_dir) {
        init_repo(&project_dir)?;
    }

    Ok(project_dir)
}
