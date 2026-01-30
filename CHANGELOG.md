# Changelog

All notable changes to dotmatrix will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Project Inception - 2026-01-30

#### Added
- Initial project structure and scaffolding
- Basic CLI interface using `clap` with command skeleton:
  - `init` - Initialize dotmatrix configuration and storage
  - `add` - Add file patterns to tracking
  - `scan` - Scan tracked files and update index
  - `backup` - Backup tracked files to storage
  - `restore` - Restore files from storage
  - `status` - Show status of tracked files
  - `list` - List all tracked files (implemented)
  - `remove` - Remove files from tracking
- XDG Base Directory compliance:
  - Config: `~/.config/dotmatrix/config.toml`
  - Data: `~/.local/share/dotmatrix/`
  - Index: `~/.local/share/dotmatrix/index.json`
  - Storage: `~/.local/share/dotmatrix/storage/`
- Core modules:
  - `src/lib.rs` - Path helpers for XDG directories
  - `src/config.rs` - Configuration management with TOML
  - `src/index.rs` - File index database with JSON serialization
  - `src/main.rs` - CLI entry point and command routing
- Dependencies added:
  - `serde` + `serde_json` for JSON serialization
  - `toml` for config file parsing
  - `clap` for CLI argument parsing
  - `dirs` for XDG directory paths
  - `sha2` for file hashing (prepared for future use)
  - `glob` for pattern matching (prepared for future use)
  - `chrono` for timestamps (prepared for future use)
  - `anyhow` for error handling
- Documentation:
  - `README.md` with project overview, installation, usage
  - `DESIGN_NOTES.md` with detailed implementation requirements
  - `.gitignore` for Rust project
  - `CHANGELOG.md` (this file)

#### Design Decisions

**Project Name**: dotmatrix
- Named after Dot Matrix from Spaceballs
- Reflects the tool's purpose: managing dot(files) in a matrix/index

**Core Philosophy**: Index in Place
- Track files where they live (no symlinks)
- Copy to versioned storage on backup
- Restore from any point in history
- Works with files anywhere on the system (not just `~/`)

**Safety-First Restore**
- Never silently overwrite files
- Show detailed comparison before restore:
  - File dates (current vs backup, with newer/older indicators)
  - File sizes
  - Warning when backup is older than current
- Optional diff viewing
- Automatic safety backup before restore
- Dry-run mode for preview
- Interactive confirmation required

**System-Wide Tracking**
- Not limited to home directory
- Can track `/etc/`, `/opt/`, any readable path
- Useful for system administrators and multi-location configs
- Permission checking on both read (backup) and write (restore)

**XDG Compliance**
- Follows Linux standards for config and data separation
- Clean home directory (no `~/.dotmatrix/` clutter)
- Config in `~/.config/dotmatrix/`
- Data in `~/.local/share/dotmatrix/`

#### Implementation Status

**Completed**:
- ✅ Project structure
- ✅ CLI command skeleton
- ✅ Config file management (load/save)
- ✅ Index file management (load/save)
- ✅ `init` command (creates directories and default config)
- ✅ `list` command (shows tracked files from config)
- ✅ XDG directory helpers

**TODO** (In Priority Order):
- [ ] File scanning implementation
  - [ ] Expand `~` and glob patterns
  - [ ] Read file metadata (size, mtime)
  - [ ] Calculate SHA256 hash
  - [ ] Handle read permission errors gracefully
  - [ ] Respect exclude patterns
- [ ] `scan` command
  - [ ] Scan all tracked files
  - [ ] Update index with current state
  - [ ] Show summary of changes
- [ ] `backup` command
  - [ ] Copy files to storage (content-addressed by hash)
  - [ ] Update index
  - [ ] Git commit (if enabled)
  - [ ] Show backup summary
- [ ] `restore` command (HIGH PRIORITY - safety critical)
  - [ ] Load index from commit/latest
  - [ ] Compare with current file state
  - [ ] Show detailed comparison table
  - [ ] Implement diff viewing
  - [ ] Create safety backup before restore
  - [ ] Dry-run mode
  - [ ] Interactive confirmation
  - [ ] Restore files
- [ ] `status` command
  - [ ] Compare current files with index
  - [ ] Show changed/new/deleted files
  - [ ] Show summary statistics
- [ ] `add` command
  - [ ] Add patterns to config
  - [ ] Save updated config
  - [ ] Optionally run scan
- [ ] `remove` command
  - [ ] Remove patterns from config
  - [ ] Optionally remove from index
  - [ ] Optionally clean storage
- [ ] Git integration
  - [ ] Initialize repo on `init`
  - [ ] Commit on backup
  - [ ] Meaningful commit messages
  - [ ] List commits/history
  - [ ] Restore from specific commit
- [ ] Pattern matching
  - [ ] Glob pattern support (`**/*.conf`)
  - [ ] Multiple pattern matching
  - [ ] Exclude pattern implementation
- [ ] Error handling improvements
  - [ ] Better error messages
  - [ ] Helpful suggestions
  - [ ] Graceful permission failures
- [ ] Testing
  - [ ] Unit tests for all modules
  - [ ] Integration tests for commands
  - [ ] Test fixtures
- [ ] TUI (future)
  - [ ] File browser
  - [ ] Interactive selection
  - [ ] Visual diff viewer

#### Known Issues
- None yet (project just started)

#### Notes for Future Development

**When implementing file scanning:**
- Use `glob` crate for pattern matching
- Use `sha2` for hashing with `Sha256::digest()`
- Handle large files efficiently (stream hashing, don't load entire file)
- Skip files user can't read (log warning, don't fail)

**When implementing restore:**
- This is the most critical safety feature
- Reference `DESIGN_NOTES.md` for detailed requirements
- Test thoroughly with various scenarios
- Consider edge cases (missing files, permission changes, etc.)

**When implementing git integration:**
- Use `git2` crate (add to dependencies)
- Store metadata in commit messages
- Consider git-lfs for large files (future enhancement)

**Code Style Preferences:**
- Use `anyhow::Result` for error handling
- Prefer explicit error messages over generic ones
- Add doc comments to public functions
- Use descriptive variable names
- Keep functions focused and small

---

## Version History

### [0.1.0] - TBD
- Initial release (not yet published)

---

**Changelog Maintenance**:
- Update this file with every significant change
- Include dates for major milestones
- Document design decisions and rationale
- Note breaking changes clearly
- Keep "Notes for Future Development" section updated

*Last updated: 2026-01-30*
