# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
make release          # Build optimized binary (default)
make build            # Build debug version
make test             # Run tests (cargo test)
make lint             # Run clippy with -D warnings
make fmt              # Format code with rustfmt
make install          # Install to ~/.cargo/bin/
make run ARGS='init'  # Run with arguments
```

## Architecture

Rust CLI tool for dotfile management without symlinks. Tracks files in-place and stores versioned backups.

### Module Structure

```
src/
├── lib.rs      # XDG path helpers, re-exports modules
├── config.rs   # TOML config (tracked_files, exclude, backup_mode, git_enabled)
├── index.rs    # JSON index with FileEntry (path, hash, size, last_modified)
├── scanner.rs  # File discovery, SHA256 hashing, glob matching, Verbosity enum
├── tui.rs      # Interactive TUI with ratatui (status, browse, add modes)
└── main.rs     # CLI via clap, all command implementations
```

### Key Types

- `Config` (config.rs) - TOML config with serde
- `BackupMode` (config.rs) - Incremental or Archive mode
- `TrackedPattern` (config.rs) - Simple string or object with path+mode override
- `Index` / `FileEntry` (index.rs) - JSON file metadata database
- `Verbosity` (scanner.rs) - Quiet/Normal/Verbose/Debug levels

### Data Storage

```
~/.config/dotmatrix/config.toml     # Configuration
~/.local/share/dotmatrix/
├── index.json                      # File metadata
├── storage/                        # Content-addressed backups (by SHA256)
│   └── ab/cd1234...
├── archives/                       # Tarball backups
│   ├── backup-YYYY-MM-DD-HHMMSS.tar.gz
│   └── latest.tar.gz -> ...
└── .git/                           # Version control (when enabled)
```

### Backup Modes

- **incremental**: Files stored by SHA256 hash in `storage/{hash[0..2]}/{hash}`. Automatic deduplication.
- **archive**: Compressed tarballs in `archives/` with timestamps.

Per-pattern mode override via config:
```toml
tracked_files = [
    "~/.bashrc",                               # Uses default mode
    { path = "~/.config/nvim/**", mode = "archive" },  # Override
]
```

## Command Implementations

All in `src/main.rs`:

| Command | Function | Key Features |
|---------|----------|--------------|
| init | `cmd_init()` | Creates dirs, git init, prompts for git identity |
| add | `cmd_add()` | Shell expansion warning for >10 files |
| remove | `cmd_remove()` | Updates config, prompts for scan |
| scan | `cmd_scan()` | Orphan detection, interactive cleanup |
| backup | `cmd_backup()` | Groups files by mode, dispatches to incremental/archive |
| restore | `cmd_restore()` | Comparison view, diff, safety backup, --extract-to, --remap |
| status | `cmd_status()` | Modified/new/deleted, quick mode, JSON output |
| list | `cmd_list()` | Shows tracked patterns with modes |
| tui | `cmd_tui()` | Interactive TUI via `tui::run()` |

### TUI (tui.rs)

Built with `ratatui` + `crossterm`. Three modes:
- **Status**: View tracked files with modification status
- **Browse**: Manage backup contents
- **Add**: Browse common config paths to add

Key types: `App` (state), `TuiMode`, `DisplayFile`, `FileStatus`

## Code Patterns

- Use `anyhow::Result` for error handling
- Tilde expansion via `scanner::expand_tilde()`
- Glob patterns via the `glob` crate
- 8KB buffer for file hashing (`scanner::hash_file()`)
- XDG compliance via `dirs` crate
- Git operations via `std::process::Command`
- TUI via `ratatui` with `crossterm` backend

## Design Principles

1. **Safety-first restore**: Must show comparison (dates, sizes), require confirmation, create pre-restore backup, support `--dry-run` and `--diff`

2. **Content-addressed storage**: Files stored by SHA256 hash for automatic deduplication

3. **System-wide tracking**: Can track `/etc/`, `/opt/`, any readable path

4. **Verbosity levels**: Use `-v` for verbose, `-vv` for debug output

## Adding New Features

1. Add CLI args to `Commands` enum in main.rs
2. Update match arm in `main()`
3. Implement `cmd_*()` function
4. Use `scanner::scan_patterns_with_verbosity()` for file operations
5. Run `make lint` before committing
