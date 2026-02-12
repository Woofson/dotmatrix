# Dot Matrix

> *"We'll have none of that mister! How far did he get? What'd he touch?"* - Dot Matrix, Spaceballs

A dotfile management and versioning tool that indexes your configuration files where they live, without the symlink madness.

Named after Dot Matrix from Spaceballs, because managing dotfiles should be as reliable as a loyal droid companion.

## Features

- **Index in place**: Track dotfiles where they live, no symlinks needed
- **Cross-platform**: Works on Linux, macOS, and Windows
- **Configurable storage**: Custom backup location via `data_dir` in config
- **Git-based versioning**: Full history of your dotfiles with commit messages
- **Pattern matching**: Track entire directories or specific file patterns
- **Exclude lists**: Ignore temporary files, logs, and other cruft
- **Two backup modes**: Incremental (content-addressed) or archive (tarballs)
- **Per-pattern modes**: Override backup mode for specific files/directories
- **Safety-first restore**: Comparison view, diffs, and automatic safety backups
- **Path remapping**: Restore to different locations for distro-hopping
- **Interactive TUI**: Browse, backup, and restore with keyboard navigation
- **CLI first**: Power user friendly with clean command structure

## Installation

```bash
git clone https://github.com/Woofson/dotmatrix.git
cd dotmatrix
make install
```

Or with cargo directly:

```bash
cargo install --path .
```

## Quick Start

```bash
# Initialize dotmatrix (sets up directories and git)
dotmatrix init

# Add files to track
dotmatrix add ~/.bashrc ~/.config/nvim/**

# Scan and index your dotfiles
dotmatrix scan

# Create a backup
dotmatrix backup -m "Initial backup"

# Check what changed
dotmatrix status

# Restore from backup (with safety prompts)
dotmatrix restore
```

## Configuration

Edit `~/.config/dotmatrix/config.toml`:

```toml
# Optional: Custom backup location (defaults to system data directory)
# data_dir = "~/Dropbox/dotmatrix"  # Sync via cloud
# data_dir = "D:/Backups/dotmatrix"  # Windows example

git_enabled = true
backup_mode = "incremental"  # default mode: "incremental" or "archive"

tracked_files = [
    "~/.bashrc",
    "~/.zshrc",
    "~/.gitconfig",
    "~/.config/dotmatrix/*",  # Track dotmatrix's own config
    # Override mode per pattern
    { path = "~/.config/nvim/**", mode = "archive" },
]

exclude = [
    "**/*.log",
    "**/.DS_Store",
    "**/node_modules/**",
]
```

### Backup Modes

- **incremental**: Content-addressed storage with automatic deduplication. Best for frequent backups.
- **archive**: Compressed tarballs with timestamps. Best for occasional snapshots.

Patterns can override the default mode by using the object syntax with `path` and `mode` fields.

### Default Paths

| Platform | Config | Data |
|----------|--------|------|
| Linux | `~/.config/dotmatrix/` | `~/.local/share/dotmatrix/` |
| macOS | `~/Library/Application Support/dotmatrix/` | Same |
| Windows | `%APPDATA%\dotmatrix\` | `%LOCALAPPDATA%\dotmatrix\` |

## Commands

| Command | Description |
|---------|-------------|
| `init` | Initialize dotmatrix and git repository |
| `add <patterns>` | Add file patterns to tracking |
| `remove <patterns>` | Remove patterns from tracking |
| `scan` | Scan tracked files and update index |
| `backup [-m msg]` | Backup tracked files to storage |
| `restore [--dry-run]` | Restore files from backup |
| `status` | Show changes since last backup |
| `list` | List tracked patterns |
| `tui` | Launch interactive TUI |

### Global Flags

- `-v, --verbose`: Increase verbosity (`-v` for verbose, `-vv` for debug)
- `-h, --help`: Show help
- `-V, --version`: Show version

### Restore Options

```bash
dotmatrix restore              # Interactive with comparison view
dotmatrix restore --dry-run    # Preview only, no changes
dotmatrix restore --diff       # Show file diffs
dotmatrix restore --yes        # Auto-confirm (still creates safety backup)
dotmatrix restore --file .bashrc  # Restore specific file

# For distro-hopping or different environments:
dotmatrix restore --extract-to ~/restored  # Extract to a directory
dotmatrix restore --remap /home/old=/home/new  # Remap paths
```

### Status Options

```bash
dotmatrix status           # Show only changes
dotmatrix status --all     # Include unchanged files
dotmatrix status --quick   # Fast mode (size/mtime only)
dotmatrix status --json    # JSON output for scripting
```

### TUI Mode

Launch the interactive terminal UI:

```bash
dotmatrix tui
```

**Three tabs** (switch with `Tab`):
- **Tracked Files**: View your backed-up files and their status
- **Add Files**: Browse your computer to add files to tracking
- **Restore**: Browse backup history and restore files

**Key bindings:**
| Key | Action |
|-----|--------|
| `j/k`, arrows | Navigate |
| `Tab` | Next tab |
| `b` | Run backup (Tracked Files tab) |
| `Enter` | Add file / Select backup / Restore |
| `Backspace` | Go back / Parent directory |
| `~` | Go to home (Add Files tab) |
| `Space` | Select multiple items |
| `a` | Type a path manually |
| `d` | Stop tracking file |
| `?` | Help |
| `q` | Quit (saves changes) |

**Status symbols** (Tracked Files tab):
- ` ` (space) = Backed up and unchanged
- `M` = Modified since last backup
- `+` = New, not yet backed up
- `-` = Deleted from your system

**Restore symbols**:
- `NEW` = File missing locally
- `CHG` = Local file differs from backup
- `OK` = Matches backup

## Directory Structure

```
~/.config/dotmatrix/
└── config.toml

~/.local/share/dotmatrix/
├── index.json
├── storage/          # Incremental backups (by hash)
├── archives/         # Tarball backups
└── .git/
```

## Safety Features

- **Comparison view**: See dates, sizes, and newer/older indicators before restore
- **Diff viewing**: Preview exact changes with `--diff`
- **Safety backup**: Automatic backup of current files before any restore
- **Dry-run mode**: Preview what would happen without making changes

## Development

```bash
make build      # Debug build
make release    # Release build
make test       # Run tests
make lint       # Run clippy
make fmt        # Format code
```

## License

MIT License - See LICENSE file for details

## Author

[Woofson](https://github.com/Woofson)

---

