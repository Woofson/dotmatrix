# Dot Matrix

> *"We'll have none of that mister! How far did he get? What'd he touch?"* - Dot Matrix, Spaceballs

A dotfile management and versioning tool that indexes your configuration files where they live, without the symlink madness.

Named after Dot Matrix from Spaceballs, because managing dotfiles should be as reliable as a loyal droid companion.

## Features

- **Index in place**: Track dotfiles where they live, no symlinks needed
- **Cross-platform**: Works on Linux, macOS, and Windows
- **Per-file encryption**: Encrypt sensitive files (SSH, credentials) using [age](https://age-encryption.org)
- **Git sync**: Push/pull backups to remote repositories
- **Configurable storage**: Custom backup location via `data_dir` in config
- **Git-based versioning**: Full history of your dotfiles with commit messages
- **Pattern matching**: Track entire directories or specific file patterns
- **Exclude lists**: Ignore temporary files, logs, and other cruft
- **Two backup modes**: Incremental (content-addressed) or archive (tar.gz/zip/7z)
- **Per-pattern modes**: Override backup mode for specific files/directories
- **Safety-first restore**: Comparison view, diffs, and automatic safety backups
- **Path remapping**: Restore to different locations for distro-hopping
- **Interactive TUI**: Browse, backup, and restore with keyboard navigation
- **Native GUI**: Modern graphical interface with mouse support
- **CLI first**: Power user friendly with clean command structure
- **Automation-ready**: Password file and stdin support for cron/scripts

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

### Linux Desktop Integration

To add Dot Matrix to your application menu:

```bash
cp dotmatrix.desktop ~/.local/share/applications/
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

# Launch interactive interfaces
dotmatrix tui    # Terminal UI
dotmatrix gui    # Graphical UI
dotmatrix        # Auto-select based on platform/config
```

### Windows GUI-Only Mode

On Windows, use `dmgui.exe` for a console-free GUI experience (no terminal window).

## Configuration

Edit `~/.config/dotmatrix/config.toml` (see `example-config.toml` for a complete reference):

```toml
# Optional: Custom backup location (defaults to system data directory)
# data_dir = "~/Dropbox/dotmatrix"  # Sync via cloud
# data_dir = "D:/Backups/dotmatrix"  # Windows example

git_enabled = true
backup_mode = "incremental"  # "incremental" or "archive"
archive_format = "targz"     # "targz", "zip", or "sevenzip"

# Interface preference when running without arguments
# "auto" = GUI on Windows, TUI on Linux/macOS
# "gui" = Always use GUI
# "tui" = Always use TUI
preferred_interface = "auto"

tracked_files = [
    "~/.bashrc",
    "~/.zshrc",
    "~/.gitconfig",
    "~/.config/dotmatrix/*",  # Track dotmatrix's own config
    # Override mode per pattern
    { path = "~/.config/nvim/**", mode = "archive" },
    # Encrypt sensitive files
    { path = "~/.ssh/config", encrypted = true },
    { path = "~/.aws/credentials", encrypted = true },
]

exclude = [
    "**/*.log",
    "**/.DS_Store",
    "**/node_modules/**",
]

# Git remote for sync (optional)
git_remote_url = "https://github.com/username/dotfiles.git"
```

### Encryption

Files marked with `encrypted = true` are encrypted using the [age](https://age-encryption.org) standard before being stored. This is ideal for sensitive files like SSH configs, API keys, and credentials.

```toml
tracked_files = [
    { path = "~/.ssh/config", encrypted = true },
    { path = "~/.gnupg/**", encrypted = true },
    { path = "~/.aws/credentials", encrypted = true },
]
```

**Interactive mode (TUI/GUI):** You'll be prompted for a password.

**CLI/Scripts/Cron:** Provide password via one of:
```bash
# Option 1: Password file (recommended for cron)
dotmatrix backup --password-file ~/.dotmatrix-pass
# Remember: chmod 600 ~/.dotmatrix-pass

# Option 2: Stdin (good for scripts)
echo "mypassword" | dotmatrix backup --password-stdin
pass show dotmatrix | dotmatrix backup --password-stdin

# Option 3: Environment variable (fallback)
export DOTMATRIX_PASSWORD="mypassword"
dotmatrix backup
```

**Example cron job:**
```bash
0 2 * * * /usr/bin/dotmatrix backup --password-file ~/.dotmatrix-pass -m "Daily backup"
```

### Git Sync

Sync backups to a remote repository:

```toml
git_remote_url = "git@github.com:username/dotfiles.git"
```

- **TUI:** Press `p` to pull, `P` to push, `U` to set remote URL
- **GUI:** Use Pull/Push buttons in status bar

### Backup Modes

- **incremental**: Content-addressed storage with automatic deduplication. Best for frequent backups.
- **archive**: Compressed archives with timestamps. Best for occasional snapshots.

### Archive Formats

When using archive mode:
- **targz**: tar.gz (default on Linux/macOS)
- **zip**: ZIP archive (default on Windows, good compatibility)
- **sevenzip**: 7z archive (best compression)

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
| `gui` | Launch graphical interface |

### Backup Options

```bash
dotmatrix backup -m "Updated configs"     # With commit message
dotmatrix backup --password-file ~/.pass  # For encrypted files
dotmatrix backup --password-stdin         # Read password from stdin
```

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

# For encrypted files:
dotmatrix restore --password-file ~/.dotmatrix-pass
dotmatrix restore --password-stdin

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
| `v` | View file or folder contents (syntax highlighted) |
| `b` | Run backup (Tracked Files tab) |
| `B` | Backup with custom message |
| `p` | Pull from git remote |
| `P` | Push to git remote |
| `U` | Set git remote URL |
| `X` | Toggle encryption `[E]` |
| `M` | Toggle backup mode `[I]`/`[A]` |
| `S` | Save and reload (shows `*` when unsaved) |
| `Right/l` | Expand folder / Enter directory |
| `Left/h` | Collapse folder / Parent directory |
| `Enter` | Add file / Select backup / Restore |
| `~` | Go to home (Add Files tab) |
| `Space` | Select multiple items |
| `a` | Type a path manually |
| `d` | Stop tracking file |
| `?` | Help |
| `q` | Quit (saves changes) |

**Folder viewing (conf.d support):**

Press `v` on a folder to view all files inside as a single concatenated view. Perfect for `conf.d` style directories where configuration is split across multiple files:

- Files are sorted by numeric prefix first (`00-base.conf`, `10-network.conf`, `99-local.conf`)
- Each file gets a header separator with its filename
- Syntax highlighting applied to each file
- Scroll through all configs in one view

**Status symbols** (Tracked Files tab):
- ` ` (space) = Backed up and unchanged
- `M` = Modified since last backup
- `+` = New, not yet backed up
- `-` = Deleted from your system

**Restore symbols**:
- `NEW` = File missing locally
- `CHG` = Local file differs from backup
- `OK` = Matches backup

**File indicators** (Tracked Files tab):
- `[I]` = Incremental backup mode (content-addressed, deduped)
- `[A]` = Archive backup mode (compressed tarball)
- `[E]` = Encrypted (requires password for backup/restore)

### GUI Mode

Launch the graphical interface:

```bash
dotmatrix gui
```

Or use the GUI-only binary on Windows (`dmgui.exe`) for a console-free experience.

**Features:**
- Same three-tab layout as TUI (Tracked Files, Add Files, Restore)
- Full mouse support with click, double-click, and right-click context menus
- Keyboard shortcuts similar to TUI
- Burger menu (☰) for quick access to config file, backup folder, and quit
- Syntax-highlighted file viewer

**Key bindings:**
| Key | Action |
|-----|--------|
| `Ctrl+Q` | Quit |
| `Escape` | Close dialog / Go back |
| `Tab` | Next tab |
| `1/2/3` | Switch to tab |
| `v` | View file |
| `b` | Backup |
| `X` | Toggle encryption `[E]` |
| `M` | Toggle backup mode `[I]`/`[A]` |
| `Shift+S` | Save and reload |
| `?` | Help |

## Directory Structure

```
~/.config/dotmatrix/
└── config.toml

~/.local/share/dotmatrix/
├── index.json
├── storage/          # Incremental backups (by hash)
├── archives/         # Tarball backups
├── restore-backups/  # Safety backups before restore
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

