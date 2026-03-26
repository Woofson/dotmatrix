//! dmcore - Core library for dotmatrix
//!
//! Project compositor with git versioning. Tracks files scattered across disk
//! without moving them, letting them stay native to their respective tools.
//!
//! # Architecture
//!
//! - **Manifest**: Maps logical projects to scattered real disk paths
//! - **Drift detection**: SHA256-based change detection
//! - **Track modes**: `git`, `backup`, or `both` per file
//! - **Backup**: Incremental (content-addressed) or archive
//!
//! # Design Rules
//!
//! - No presentation code (println!, colors, prompts)
//! - Return types that frontends interpret and render
//! - All logic lives here, frontends are thin wrappers

pub mod backup;
pub mod config;
pub mod crypto;
pub mod git;
pub mod index;
pub mod manifest;
pub mod project;
pub mod scanner;
pub mod store;

pub use backup::{
    backup_archive, backup_incremental, backup_incremental_encrypted,
    backup_project_incremental, backup_project_incremental_encrypted,
    backup_project_incremental_encrypted_with_message, list_archives, project_needs_password,
    ArchiveInfo, BackupResult,
};
pub use config::{contract_path, expand_path, ArchiveFormat, BackupMode, Config};
pub use crypto::{decrypt_bytes, decrypt_file, encrypt_bytes, encrypt_file};
pub use git::{
    commit, commits_ahead, commits_behind, fetch, get_remote_status, get_remote_url,
    init_project_repo, init_repo, is_git_repo, pull, push, recent_commits, set_remote_url,
    stage_all, CommitInfo, RemoteStatus,
};
pub use index::{FileEntry, Index};
pub use manifest::Manifest;
pub use project::{Project, TrackMode, TrackedFile};
pub use scanner::{
    file_metadata, hash_file, scan_file, scan_project, FileStatus, ProjectSummary, ScanResult,
};
pub use store::{
    exists_in_store, get_stored_path, retrieve_file, retrieve_file_encrypted,
    retrieve_file_from, retrieve_file_from_encrypted, store_file, store_file_encrypted,
    store_file_to, store_file_to_encrypted, StoreResult,
};
