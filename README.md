# 🎬 dotmatrix

> *"I'm a Mog: half man, half dog. I'm my own best friend!"* - Barf, Spaceballs

A dotfile management and versioning tool that indexes your configuration files where they live, without the symlink madness.

Named after Dot Matrix from Spaceballs, because managing dotfiles should be as reliable as a loyal droid companion.

## Features

- **Index in place**: Track dotfiles where they live, no symlinks needed
- **XDG compliant**: Follows Linux standards for config and data directories
- **Git-based versioning**: Full history of your dotfiles with commit messages
- **Pattern matching**: Track entire directories or specific file patterns
- **Exclude lists**: Ignore temporary files, logs, and other cruft
- **Fast backup & restore**: Quick snapshots of your system configuration
- **CLI first**: Power user friendly with clean command structure
- **TUI planned**: Easy visual management coming soon

## Why dotmatrix?

Most dotfile managers use symlinks, which can be fragile and confusing. dotmatrix takes a different approach:

1. **Index** your dotfiles where they naturally live
2. **Backup** copies to versioned storage when you want
3. **Restore** from any point in history

No broken symlinks. No complicated stow configurations. Just simple, reliable dotfile management.

## Installation

### From source

```bash
git clone https://github.com/yourusername/dotmatrix.git
cd dotmatrix
cargo build --release
cargo install --path .
```

### Binary (coming soon)

Pre-built binaries will be available for Linux, macOS, and Windows.

## Quick Start

```bash
# Initialize dotmatrix
dotmatrix init

# Edit your config to add files to track
$EDITOR ~/.config/dotmatrix/config.toml

# Scan and index your dotfiles
dotmatrix scan

# Create a backup
dotmatrix backup -m "Initial backup"

# List tracked files
dotmatrix list

# Check status of changes
dotmatrix status

# Restore from backup
dotmatrix restore
```

## Configuration

dotmatrix uses `~/.config/dotmatrix/config.toml`:

```toml
git_enabled = true

tracked_files = [
    "~/.bashrc",
    "~/.zshrc",
    "~/.gitconfig",
    "~/.config/nvim/**",
    "~/.config/alacritty/**",
]

exclude = [
    "**/*.log",
    "**/.DS_Store",
    "**/node_modules/**",
    "**/__pycache__/**",
]
```

## Directory Structure

```
~/.config/dotmatrix/
└── config.toml              # Your configuration

~/.local/share/dotmatrix/
├── index.json               # File index database
├── storage/                 # Backup storage
│   ├── <hash1>/
│   ├── <hash2>/
│   └── ...
└── .git/                    # Version control
```

## Commands

### `dotmatrix init`
Initialize dotmatrix with default configuration and create necessary directories.

### `dotmatrix add <patterns>`
Add file patterns to tracking list.

```bash
dotmatrix add ~/.vimrc ~/.config/fish/**
```

### `dotmatrix scan`
Scan all tracked files and update the index with current state.

### `dotmatrix backup [-m <message>]`
Create a backup of all tracked files and commit to version control.

```bash
dotmatrix backup -m "Updated neovim config"
```

### `dotmatrix restore [--commit <hash>]`
Restore files from backup. Without a commit hash, restores from latest backup.

```bash
dotmatrix restore --commit abc123
```

### `dotmatrix status`
Show which tracked files have changed since last backup.

### `dotmatrix list`
List all currently tracked files and patterns.

### `dotmatrix remove <patterns>`
Remove file patterns from tracking.

## Development Status

dotmatrix is in early development. Current status:

- [x] Project structure and CLI skeleton
- [x] Config and index management
- [x] XDG directory support
- [ ] File scanning and hashing
- [ ] Backup implementation
- [ ] Git integration
- [ ] Restore functionality
- [ ] Pattern matching (glob support)
- [ ] Exclude list handling
- [ ] Status command
- [ ] TUI interface
- [ ] Tests
- [ ] Documentation

## Contributing

Contributions are welcome! This is an early-stage project, so there's plenty of room for:

- Feature implementations
- Bug fixes
- Documentation improvements
- Testing
- Ideas and suggestions

## Development

```bash
# Run in development
cargo run -- init

# Run tests (when we have them)
cargo test

# Build release
cargo build --release

# Format code
cargo fmt

# Lint
cargo clippy
```

## License

MIT License - See LICENSE file for details

## Inspiration

- The eternal wisdom of Spaceballs
- The pain of managing dotfiles across multiple machines
- The realization that symlinks aren't always the answer

## Author

Woofson

---

*"What's the matter, Colonel Sandurz? Chicken?"* 🐔
