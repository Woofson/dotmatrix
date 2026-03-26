# Dotmatrix 2.0

> *"We'll have none of that mister! How far did he get? What'd he touch?"* - Dot Matrix, Spaceballs

**Project compositor with git versioning.** Track files scattered across your system without moving them. Each project gets its own isolated git repository for independent version history.

Named after Dot Matrix from Spaceballs, because managing your files should be as reliable as a loyal droid companion.

## What's New in v2

- **Per-project git repositories** - Each project has its own `.git/`, store, and index
- **Independent remotes** - Push different projects to different repositories
- **Path migration** - Restore backups even after changing username or machine
- **File viewer** - Syntax-highlighted viewer with conf.d directory assembly
- **Custom commit messages** - Add context to your backup commits

## Features

- **Files stay where they are** - No symlinks, no moving, track files in place
- **Project-based organization** - Group related files across different directories
- **Per-project versioning** - Independent git history for each project
- **Drift detection** - SHA256-based change detection (synced, drifted, new, missing)
- **Three track modes** - Git (version control), Backup (incremental), or Both
- **Per-file encryption** - Encrypt sensitive files using [age](https://age-encryption.org)
- **Archive backups** - tar.gz, zip, or 7z snapshots
- **Cross-platform** - Linux, macOS, Windows
- **TUI interface** - Keyboard-driven terminal UI with ratatui
- **Path remapping** - Restore to different locations (distro-hopping friendly)

## Installation

```bash
git clone https://github.com/Woofson/dotmatrix.git
cd dotmatrix
cargo install --path crates/dmtui
```

Or build the TUI directly:

```bash
cargo build --release -p dmtui
cp target/release/dotmatrix-tui ~/.local/bin/
```

## Quick Start

```bash
# Launch the TUI
dotmatrix-tui
```

In the TUI:
1. Press `n` to create a new project
2. Press `Tab` to go to "Add Files"
3. Navigate to files and press `Enter` to add them
4. Press `Tab` to return to "Projects"
5. Press `b` to backup

## Configuration

### Manifest Location
`~/.config/dotmatrix/manifest.toml`

```toml
[project.nvim-config]
files = [
    { path = "~/.config/nvim/init.lua", track = "git" },
    { path = "~/.config/nvim/lua/**", track = "git" },
]
remote = "git@github.com:user/nvim-config.git"

[project.ssh-keys]
files = [
    { path = "~/.ssh/config", track = "both", encrypted = true },
    { path = "~/.ssh/id_*", track = "backup", encrypted = true },
]
```

### Data Structure
`~/.local/share/dotmatrix/`

```
~/.local/share/dotmatrix/
├── projects/
│   ├── nvim-config/
│   │   ├── .git/           # Project-specific git repo
│   │   ├── store/          # Content-addressed storage
│   │   └── index.json      # File tracking index
│   └── ssh-keys/
│       ├── .git/
│       ├── store/
│       └── index.json
└── backups/                 # Archive backups (shared)
```

## Track Modes

| Mode | Symbol | Description |
|------|--------|-------------|
| Git | `[G]` | Version controlled, diffable, shareable |
| Backup | `[B]` | Incremental content-addressed storage |
| Both | `[+]` | Both git tracking and backup |

## TUI Key Bindings

### Navigation (All Tabs)
| Key | Action |
|-----|--------|
| `↑/k` `↓/j` | Move up/down |
| `PgUp/PgDn` | Page up/down |
| `Home/End` | Jump to start/end |

### Global
| Key | Action |
|-----|--------|
| `Tab` / `1-3` | Switch tabs |
| `?` | Show/hide help |
| `v` | View file content |
| `q` | Quit |

### File Viewer
| Key | Action |
|-----|--------|
| `↑/k` `↓/j` | Scroll up/down |
| `PgUp/PgDn` | Page up/down |
| `g/Home` | Go to top |
| `G/End` | Go to bottom |
| `v/q/Esc` | Close viewer |

### Projects Tab
| Key | Action |
|-----|--------|
| `Enter/→/l` | Expand/collapse project |
| `←/h` | Collapse project |
| `m` | Toggle track mode (Git → Backup → Both) |
| `x` | Toggle encryption |
| `b` | Backup project |
| `B` | Backup with custom message |
| `s` | Sync project |
| `n` | New project |
| `d/Del` | Delete project |
| `r` | Refresh |
| `g` | Refresh git status |
| `G` | Set git remote URL |
| `p` | Push to remote |
| `P` | Pull from remote |

### Add Files Tab
| Key | Action |
|-----|--------|
| `Enter/→/l` | Open directory or add file |
| `←/h/Bksp` | Parent directory |
| `a` | Add selected file |
| `R` | Recursive add (folder) |
| `t` | Cycle track mode |
| `p` | Cycle target project |
| `n` | New project |
| `~` | Go to home |

### Restore Tab
| Key | Action |
|-----|--------|
| `Enter/→/l` | View files in backup / Restore |
| `Space` | Toggle multi-select |
| `v` | View file content |
| `←/h/Bksp` | Back to commits |
| `r` | Refresh |

## Status Indicators

### File Status
| Symbol | Meaning |
|--------|---------|
| `✓` | Synced (green) |
| `⚠` | Drifted (yellow) |
| `+` | New file (cyan) |
| `✗` | Missing (red) |

### Git Status
| Symbol | Meaning |
|--------|---------|
| `[synced]` | Up to date with remote |
| `[↑N]` | N commits ahead |
| `[↓N]` | N commits behind |
| `[no remote]` | No remote configured |

### Restore Status
| Symbol | Meaning |
|--------|---------|
| `NEW` | File missing locally |
| `CHG` | Local file differs |
| `OK` | Matches backup |

## File Viewer

Press `v` on any file to view its contents with syntax highlighting.

**conf.d Directory Support:** When viewing a directory, all files are assembled into a single view with headers. Files are sorted by numeric prefix first (`00-base.conf`, `10-network.conf`, `99-local.conf`), then alphabetically.

## Path Migration

Restore automatically remaps paths when your username or home directory changes:

- `/home/olduser/.config/file` → `/home/newuser/.config/file`
- `/Users/olduser/.config/file` → `/Users/newuser/.config/file`

This allows restoring backups made on different machines or by different users.

## Architecture

```
     TUI              ← keyboard-driven interface
      ↓
   dmcore             ← all logic, no presentation
      ↓
manifest + store + git + backup
```

### Crates
- `dmcore` - Core library (all logic)
- `dmtui` - TUI binary (ratatui)
- `dmcli` - CLI binary (planned)
- `dmgui` - GUI binary (planned, egui)

## Development

```bash
cargo build                    # Build all crates
cargo build -p dmtui           # Build just TUI
cargo test                     # Run tests
cargo run -p dmtui             # Run TUI
```

## License

MIT License - See LICENSE file for details

## Author

[Woofson](https://github.com/Woofson)
