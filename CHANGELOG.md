# Changelog

All notable changes to dotmatrix will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.2] - 2026-02-12

### TUI Help Improvements
- Help window now scrollable with Up/Down/j/k/PageUp/PageDown
- Added [I] and [A] mode indicator explanations
- Expanded restore symbols section (NEW/CHG/OK)
- Any non-scroll key closes help

## [0.3.1] - 2026-02-12

### TUI Usability Improvements
- Renamed "Backup Status" tab to "Tracked Files" for clarity
- Renamed "Add Files" tab to just "Add Files" with clearer descriptions
- Reordered tabs: Tracked Files → Add Files → Restore (more intuitive workflow)
- Updated all help text to use beginner-friendly language
- Panel titles now explain what to do, not just what you're seeing
- Status bar hints simplified and made more actionable

## [0.3.0] - 2026-02-12

### Changed
- `init` command now displays available backup modes (incremental, archive) with descriptions
- Default config now includes `~/.config/dotmatrix/*` in tracked files (self-tracking)

### TUI Improvements
- Renamed tabs for clarity: "Status" → "Backup Status", "Browse" → "Restore", "Add" → "Add Files"
- Added descriptive panel titles explaining what each view shows
- Added context-aware hints in status bar for each mode
- Reorganized help screen with clear sections explaining tabs and symbols
- **Fixed:** Enter key no longer removes files from tracking in Backup Status mode
- **Added:** Press `b` in Backup Status to run backup with auto-generated timestamp commit message (format: `Backup YYYY-MM-DD HH:MM:SS`)
- **Added:** Config changes now saved only on exit (not immediately), allowing dotmatrix config to be backed up properly
- **Added:** `config_dirty` and `index_dirty` flags to track unsaved changes

### Restore Functionality (TUI)
- **Added:** Full restore functionality in the Restore tab
- Restore tab now shows git commit history with hash, date, and commit message
- Select a backup to view all files that were included in that backup
- Files show status: `NEW` (doesn't exist locally), `CHG` (local file differs), `OK` (matches backup)
- Select individual files or use Space to select multiple files for restore
- Press Enter to restore selected files from backup storage
- Press Backspace to go back from file list to commit list
- Files are restored from content-addressed storage using their hash

### Cross-Platform & Configuration
- **Added:** Configurable `data_dir` in config.toml for custom backup location
- **Added:** Windows support - paths now work correctly on all platforms
- **Added:** `expand_path()` function for cross-platform ~ expansion
- **Added:** Platform-specific default tracked files (Windows vs Unix)
- **Added:** `get_data_dir_with_config()` and related helpers for explicit config passing
- **Fixed:** TUI backup now uses current executable path instead of assuming PATH

### Project Inception - 2026-01-30

#### Session 1: Project Setup (Morning)
Initial project structure and scaffolding - see details below in "Added" section.

#### Session 2: File Scanning Implementation (Afternoon)

**What we built:**
Implemented complete file scanning functionality in `src/scanner.rs`:

1. **Path Expansion** (`expand_tilde` function):
   - Converts `~/` paths to absolute paths using user's home directory
   - Handles both tilde and absolute paths correctly

2. **Exclude Pattern Matching** (`is_excluded` function):
   - Uses glob patterns to filter out unwanted files
   - Supports patterns like `**/*.log`, `**/.DS_Store`, etc.
   - Expands tildes in exclude patterns too

3. **File Hashing** (`hash_file` function):
   - Calculates SHA256 hash of file contents
   - Uses 8KB buffer for efficient reading of large files
   - Returns hex-encoded hash string

4. **File Metadata** (`scan_file` function):
   - Creates `FileEntry` with path, hash, size, and mtime
   - Handles metadata extraction from filesystem
   - Proper error context for debugging

5. **Pattern Scanning** (`scan_pattern` and `scan_patterns` functions):
   - Supports glob patterns (`~/.config/**`, `/etc/*.conf`)
   - Handles literal paths (no glob characters)
   - Distinguishes between files and directories
   - Deduplicates results from overlapping patterns
   - Collects errors without failing entire scan

**Commands implemented:**
- ✅ `add` - Add file patterns to config
- ✅ `remove` - Remove patterns from config  
- ✅ `scan` - Full implementation with:
  - Pattern matching and file discovery
  - Hash calculation for all files
  - Index updates (new/updated/unchanged detection)
  - Beautiful progress output with status indicators
  - Summary statistics
  - Graceful error handling

**User Experience improvements:**
- Added emoji indicators (✓, ❌, ⚠️, 🎬, 📋, 🚫, 📊)
- Clear status messages (NEW, UPDATED, unchanged)
- Helpful next-step suggestions
- Graceful error messages with context

**Testing considerations:**
- Added basic unit tests in `scanner.rs`
- Need to add integration tests when you have Rust installed
- Test with various file types, permissions, patterns

**What's next:**
The `backup` command is the logical next step. It will:
1. Run a scan (or use existing index)
2. Copy files to storage (content-addressed by hash)
3. Optionally commit to git
4. Show backup summary

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
- ✅ `add` command (adds patterns to config)
- ✅ `remove` command (removes patterns from config)
- ✅ XDG directory helpers
- ✅ **File scanning module (`src/scanner.rs`)**:
  - ✅ Path expansion (`~` → `/home/user`)
  - ✅ Glob pattern matching support
  - ✅ SHA256 file hashing
  - ✅ File metadata reading (size, mtime)
  - ✅ Exclude pattern filtering
  - ✅ Permission error handling
  - ✅ Multi-pattern scanning
- ✅ **`scan` command fully implemented**:
  - ✅ Finds files matching patterns
  - ✅ Calculates hashes for all files
  - ✅ Updates index with file metadata
  - ✅ Shows NEW/UPDATED/unchanged status
  - ✅ Displays scan summary
  - ✅ Handles errors gracefully

**TODO** (In Priority Order):
- [x] File scanning implementation
  - [x] Expand `~` and glob patterns
  - [x] Read file metadata (size, mtime)
  - [x] Calculate SHA256 hash
  - [x] Handle read permission errors gracefully
  - [x] Respect exclude patterns
- [x] `scan` command
  - [x] Scan all tracked files
  - [x] Update index with current state
  - [x] Show summary of changes
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
