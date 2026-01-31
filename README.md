# dotmatrix

> *"I'm a Mog: half man, half dog. I'm my own best friend!"* - Barf, Spaceballs

A dotfile management and versioning tool that indexes your configuration files where they live, without the symlink madness.

Named after Dot Matrix from Spaceballs, because managing dotfiles should be as reliable as a loyal droid companion.

## Features

- **Index in place**: Track dotfiles where they live, no symlinks needed
- **XDG compliant**: Follows Linux standards for config and data directories
- **Git-based versioning**: Full history of your dotfiles with commit messages
- **Pattern matching**: Track entire directories or specific file patterns
- **Exclude lists**: Ignore temporary files, logs, and other cruft
- **Two backup modes**: Incremental (content-addressed) or archive (tarballs)
- **Safety-first restore**: Comparison view, diffs, and automatic safety backups
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
git_enabled = true
backup_mode = "incremental"  # or "archive"

tracked_files = [
    "~/.bashrc",
    "~/.zshrc",
    "~/.gitconfig",
    "~/.config/nvim/**",
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
```

### Status Options

```bash
dotmatrix status           # Show only changes
dotmatrix status --all     # Include unchanged files
dotmatrix status --quick   # Fast mode (size/mtime only)
dotmatrix status --json    # JSON output for scripting
```

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

*"What's the matter, Colonel Sandurz? Chicken?"*
