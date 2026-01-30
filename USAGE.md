# dotmatrix Usage Guide

## Quick Start Workflow

### 1. Initialize dotmatrix

```bash
dotmatrix init
```

This creates:
- `~/.config/dotmatrix/config.toml` - Your configuration
- `~/.local/share/dotmatrix/` - Data directory
- `~/.local/share/dotmatrix/storage/` - Backup storage
- `~/.local/share/dotmatrix/index.json` - File index

### 2. Configure tracked files

Edit `~/.config/dotmatrix/config.toml`:

```toml
git_enabled = true

tracked_files = [
    # User dotfiles
    "~/.bashrc",
    "~/.zshrc",
    "~/.gitconfig",
    "~/.config/nvim/**",
    
    # System configs (if you have read access)
    "/etc/nginx/nginx.conf",
    "/etc/hosts",
]

exclude = [
    "**/*.log",
    "**/.DS_Store",
    "**/node_modules/**",
]
```

Or use the CLI:

```bash
# Add files to track
dotmatrix add ~/.vimrc
dotmatrix add ~/.config/fish/**
dotmatrix add /etc/ssh/sshd_config

# Remove files from tracking
dotmatrix remove ~/.vimrc

# List tracked patterns
dotmatrix list
```

### 3. Scan your files

```bash
dotmatrix scan
```

Output:
```
Scanning tracked files...

Finding files matching patterns...
Found 15 files to scan.

Scanning: /home/user/.bashrc ... ✓ NEW
Scanning: /home/user/.zshrc ... ✓ NEW
Scanning: /home/user/.gitconfig ... ✓ NEW
...

📊 Scan complete:
   Total files: 15
   New files: 15

✓ Index saved to: /home/user/.local/share/dotmatrix/index.json
```

### 4. Create a backup (TODO - not yet implemented)

```bash
dotmatrix backup -m "Initial backup of dotfiles"
```

### 5. Check status (TODO - not yet implemented)

```bash
dotmatrix status
```

### 6. Restore files (TODO - not yet implemented)

```bash
# Restore from latest backup
dotmatrix restore

# Restore from specific commit
dotmatrix restore --commit abc123

# Dry run (see what would happen)
dotmatrix restore --dry-run

# Show diffs
dotmatrix restore --diff
```

## Common Workflows

### Tracking Neovim Configuration

```bash
# Add entire nvim directory
dotmatrix add ~/.config/nvim/**

# Scan to index
dotmatrix scan

# Backup with message
dotmatrix backup -m "Updated nvim config with new plugins"
```

### Tracking System Configs (as root/sudo)

```bash
# Add system files
dotmatrix add /etc/nginx/nginx.conf
dotmatrix add /etc/ssh/sshd_config
dotmatrix add /etc/fstab

dotmatrix scan
dotmatrix backup -m "System config snapshot before updates"
```

### Restoring After Fresh Install

```bash
# On new machine, install dotmatrix
# Clone your dotmatrix storage (if using git remote)

# Initialize
dotmatrix init

# Restore everything
dotmatrix restore

# Or restore specific files
dotmatrix restore --file ~/.bashrc
```

## Pattern Matching

dotmatrix supports glob patterns:

```bash
# Single file
~/.bashrc

# All files in directory (non-recursive)
~/.config/*

# All files in directory tree (recursive)
~/.config/**

# Specific file types
~/.config/**/*.lua

# Multiple patterns
dotmatrix add ~/.config/nvim/** ~/.config/fish/**
```

## Exclude Patterns

In your config.toml:

```toml
exclude = [
    # Log files
    "**/*.log",
    
    # OS files
    "**/.DS_Store",
    "**/Thumbs.db",
    
    # Development
    "**/node_modules/**",
    "**/__pycache__/**",
    "**/.git/**",
    
    # Temporary files
    "**/*.tmp",
    "**/*.swp",
    "**/*~",
]
```

## Tips and Tricks

### Check what files match a pattern

```bash
# Add pattern, then list
dotmatrix add ~/.config/fish/**
dotmatrix scan
# Check index.json to see what was found
```

### Backup before system updates

```bash
dotmatrix scan
dotmatrix backup -m "Pre-update snapshot $(date +%Y-%m-%d)"
```

### Track application configs

```bash
# VS Code
dotmatrix add ~/.config/Code/User/settings.json
dotmatrix add ~/.config/Code/User/keybindings.json

# Git
dotmatrix add ~/.gitconfig
dotmatrix add ~/.gitignore_global

# SSH
dotmatrix add ~/.ssh/config
```

### Exclude large or sensitive files

Add to config.toml exclude list:
```toml
exclude = [
    "**/*.key",           # Private keys
    "**/*.pem",           # Certificates
    "**/secrets/**",      # Secret directories
    "**/.env",            # Environment files
    "**/*.sqlite",        # Databases
    "**/*.db",
]
```

## File Locations Reference

- **Config**: `~/.config/dotmatrix/config.toml`
- **Index**: `~/.local/share/dotmatrix/index.json`
- **Storage**: `~/.local/share/dotmatrix/storage/`
- **Git repo**: `~/.local/share/dotmatrix/.git/`

## Troubleshooting

### "Permission denied" when scanning

Some files require elevated privileges. Either:
1. Run with sudo: `sudo dotmatrix scan`
2. Remove those files from tracking
3. Fix file permissions

### Pattern not matching any files

1. Check the pattern syntax
2. Use absolute paths or proper `~` expansion
3. Verify files exist: `ls -la <pattern>`
4. Check exclude patterns aren't filtering them out

### Files showing as changed every scan

This happens if:
- File modification time changes (even without content changes)
- File is being actively written to
- Add to exclude patterns if expected

## Next Steps

1. Implement `backup` command to actually copy files to storage
2. Add git integration for version control
3. Implement `restore` with safety features
4. Build `status` command to show changes
5. Add TUI for visual management

---

*Note: This guide will be updated as new features are implemented.*
