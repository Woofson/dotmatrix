# Design Notes for dotmatrix

## Core Principles

### 1. Safety First - Never Silently Overwrite

**The Problem**: Humans have poor memory. Days, weeks, or months after a backup, you won't remember:
- When files were last modified
- What changed in them
- Whether the backup is newer or older than current state

**The Solution**: Make restore operations **explicit, informative, and safe**.

#### Restore Requirements

Before any file is touched, `dotmatrix restore` MUST show:

1. **File comparison overview**
   - Which files will be affected
   - Current file date vs backup date (newer/older indicator)
   - Current size vs backup size
   - Visual indicator if backup is older than current (⚠️ WARNING)

2. **Interactive confirmation**
   ```
   The following files will be restored:
   
   ~/.bashrc
     Current:  2026-01-28 14:32  (2.1 KB)  [NEWER]
     Backup:   2026-01-15 09:14  (1.8 KB)  [older]
     ⚠️  Current file is NEWER than backup!
   
   /etc/nginx/nginx.conf
     Current:  2026-01-20 11:05  (4.3 KB)  [older]
     Backup:   2026-01-25 16:22  (4.5 KB)  [NEWER]
     ✓ Backup is newer
   
   Restore 2 files? [y/N/d(iff)]
   ```

3. **Optional diff viewing**
   - `d` or `diff` option shows actual file differences
   - Per-file diff before restoration
   - Uses system `diff` or similar tool
   - Colorized output if terminal supports it

4. **Backup before restore**
   - Before overwriting ANY file, create a `.dotmatrix-restore-backup-<timestamp>/` directory
   - Copy current files there first
   - Print location: "Current files backed up to: ~/.dotmatrix-restore-backup-20260130-143022/"
   - This is a safety net for "oh crap" moments

5. **Dry-run mode**
   ```bash
   dotmatrix restore --dry-run
   dotmatrix restore --dry-run --diff
   ```
   - Shows what WOULD happen
   - No files touched
   - Perfect for checking before committing

#### Restore Flags

```bash
dotmatrix restore                    # Interactive with prompts
dotmatrix restore --yes              # Auto-confirm (still shows overview)
dotmatrix restore --dry-run          # Show what would happen, touch nothing
dotmatrix restore --diff             # Show diffs for all files
dotmatrix restore --commit abc123    # Restore from specific commit
dotmatrix restore --file ~/.bashrc   # Restore only specific file(s)
```

#### Permission Handling

- If trying to restore to `/etc/` or other system paths without permission:
  - Show clear error: "Cannot write to /etc/nginx/nginx.conf (permission denied)"
  - Suggest: "Run with sudo or restore to temporary location first"
  - Option: `--extract-to <dir>` to pull files to a different location for review

#### Edge Cases

1. **File doesn't exist in backup**: Skip with warning
2. **File doesn't exist currently**: Show as "new file" in overview
3. **File deleted since backup**: Ask whether to restore it
4. **No changes detected**: "All files already match backup (nothing to do)"
5. **Backup is identical**: Show "✓ No changes" for that file

### 2. System-Wide File Tracking

dotmatrix can track files from anywhere on the filesystem:
- `~/.bashrc` (user home)
- `/etc/nginx/nginx.conf` (system configs)
- `/opt/myapp/config.yml` (application configs)
- Any path the user has read access to

**Implementation considerations:**
- Expand `~` to actual home directory path
- Handle absolute paths correctly
- Check read permissions before attempting to index
- Store original absolute paths in index
- On restore, check write permissions before attempting

### 3. Path Expansion and Normalization

All paths should be:
- Expanded (`~` → `/home/username`)
- Canonicalized (symlinks resolved)
- Stored as absolute paths in index
- Displayed to user in readable format (show `~` where appropriate)

### 4. Index Integrity

The index (`~/.local/share/dotmatrix/index.json`) is the source of truth:
- Maps file paths to their metadata (hash, size, mtime)
- Must be updated atomically (write to temp file, then rename)
- Include format version for future compatibility
- Validate on load (detect corruption)

### 5. Storage Strategy

Files stored in `~/.local/share/dotmatrix/storage/`:
- Content-addressed by SHA256 hash
- Deduplication automatic (same content = same hash)
- Original paths stored in index, not in storage filenames
- Consider compression for large files (future enhancement)

### 6. Git Integration

When `git_enabled = true`:
- Commit after each backup
- Commit message from user or auto-generated
- Include metadata in commit (which files changed, sizes, etc.)
- Tag important backups (future enhancement)
- Allow restore from any commit in history

### 7. Error Handling Philosophy

- **Be explicit**: Never fail silently
- **Be helpful**: Suggest fixes when possible
- **Be safe**: Confirm destructive operations
- **Be informative**: Show what's happening and why

Example:
```
❌ Cannot read /etc/shadow (permission denied)
💡 This file requires root access. Run with sudo or remove from tracked files.
```

## Future Enhancements

### TUI (Text User Interface)
- Visual file browser
- Select files for tracking
- Review changes before backup
- Interactive restore with diff viewing
- Manage exclude patterns

### Advanced Features
- Encryption support for sensitive files
- Remote sync (sync storage to another machine)
- Hooks (pre-backup, post-restore scripts)
- File templates (generate config from template + variables)
- Multiple profiles (work, personal, server, etc.)

### Performance
- Parallel file hashing for large directories
- Incremental backups (only changed files)
- Smart scanning (skip files with same mtime + size)

## Development Priorities

1. ✅ Project structure
2. ✅ Config and index management  
3. ⏳ File scanning with hash calculation
4. ⏳ Backup implementation (copy to storage)
5. ⏳ Safe restore with confirmations and diff
6. ⏳ Git integration
7. ⏳ Pattern matching (glob support)
8. ⏳ Comprehensive error handling
9. ⏳ Tests for all critical paths
10. ⏳ TUI interface

## Questions to Resolve

- Compression in storage? (gzip files over X size?)
- Max file size limit? (skip huge files?)
- Symlink handling? (follow or store link itself?)
- Binary file detection? (skip or include?)
- Concurrent operation safety? (file locks?)

---

*Last updated: 2026-01-30*
