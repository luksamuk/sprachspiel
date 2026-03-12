# File Write Tools - Implementation Plan

**Priority:** 🔴 HIGH (First item after PR #2)

**Status:** 📋 PLANNED

**Estimated Effort:** 5-7 days (all phases)

---

## Background

### Current State

The project has 5 file tools, all read-only:

| Tool | Purpose |
|------|---------|
| `read_file` | Read text file contents |
| `read_file_segment` | Read specific lines |
| `count_lines` | Count lines in file |
| `list_directory` | List directory contents |
| `search_files` | Regex search in files |

**Gap:** No ability to create, modify, or append to files. The `run_command` tool explicitly blocks pipes and redirects, preventing file writing via shell.

### Problem Statement

1. **LLM cannot write files** - Limited to reading existing content
2. **run_command blocks redirects** - `echo "content" > file` is rejected
3. **No backup mechanism** - Cannot create backups before editing
4. **Security concerns** - Need safe write operations with sandbox

---

## Design Decisions

Based on discussion on 2026-03-12:

### 1. Backup on Edit

**Decision:** Option B - Only create backup if user requests

- Default: No automatic backup
- Optional: `create_backup=true` parameter saves `.bak` file
- LLM can explicitly request backup when needed

### 2. File Size Limit

**Decision:** 5MB for both read and write operations

- Current: 1MB limit for reads
- New: 5MB limit for all file operations
- Allows working with reasonable code files (most under 5MB)
- Configurable in constants

### 3. Blocked Patterns

**Decision:** Option B - Configurable with hardcoded defaults

- Default blocklist in code: `.env`, `secrets`, `credentials`, `id_rsa`, `.pem`, `.ssh`
- Configurable via `config.toml` under `[file-tools]`
- Merged at runtime: defaults + user additions

### 4. Sandbox Enforcement

**Decision:** Mandatory sandbox for all write operations

- `sandbox=false` accepted for backward compatibility but **ignored for writes**
- Write operations ALWAYS sandboxed to CWD
- Same path validation as read operations
- No writing outside allowed directories

---

## Security Model

### Threat Mitigation

| Threat | Mitigation |
|--------|------------|
| **Destructive overwrite** | `overwrite=false` by default |
| **Path traversal** | `canonicalize()` before write |
| **Symlink attack** | Verify path is not symlink |
| **Sensitive files** | Blocklist of patterns + configurable |
| **Resource exhaustion** | 5MB size limit per operation |
| **Binary injection** | UTF-8 validation, reject non-UTF-8 |
| **Directory writes** | Verify path is file, not directory |
| **Permission escalation** | Sandbox always enforced for writes |

### Blocked File Patterns

**Default blocklist (always blocked):**

```toml
# In code: BLOCKED_WRITE_PATTERNS constant
[".env", ".env.local", ".env.production", 
 "secrets", "credentials", "id_rsa", "id_dsa", "id_ed25519",
 ".pem", ".key", ".ssh/authorized_keys", ".ssh/known_hosts",
 ".gnupg", "credentials.json", "secrets.json", "secrets.yaml"]
```

**User-configured additional patterns (config.toml):**

```toml
[file-tools]
blocked_patterns = [
    ".env.*".           # All .env variants
    "*secret*",         # Any file with 'secret' in name
    "*.pem",            # Certificate files
    "config/database.yml",  # Specific sensitive files
]
```

**Runtime logic:**

```rust
fn is_blocked_path(path: &Path) -> bool {
    let filename = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    
    // Check hardcoded defaults
    for pattern in DEFAULT_BLOCKED_PATTERNS {
        if filename.to_lowercase().contains(pattern) {
            return true;
        }
    }
    
    // Check user-configured patterns
    let config = get_file_tools_config();
    for pattern in &config.blocked_patterns {
        if pattern_matches(pattern, path) {
            return true;
        }
    }
    
    false
}
```

---

## Tools Specification

### `write_file`

**Purpose:** Create a new file or completely overwrite an existing file.

```rust
/// Write content to a file, creating or overwriting it.
///
/// Creates a new file or completely replaces an existing file's content.
/// Use this when you need to create a new file or replace the entire content.
///
/// # Arguments
/// * `path` - Path to the file (relative to current directory or absolute).
///   - Example: "src/main.rs", "output/result.txt"
/// * `content` - The text content to write to the file.
///   - Must be valid UTF-8 text.
///   - Maximum size: 5MB.
/// * `overwrite` - Whether to overwrite existing file (default: "false").
///   - "false": Return error if file exists (safer)
///   - "true": Overwrite existing file
/// * `sandbox` - Ignored for write operations (always sandboxed).
///   - Included for backward compatibility.
///
/// # Returns
/// Success message with file path and size, or error message.
///
/// # Security
/// - Always sandboxed to current directory tree
/// - Blocked for sensitive file patterns (.env, secrets, .pem, etc.)
/// - Maximum file size: 5MB
///
/// # Errors
/// - File exists and overwrite=false
/// - Path is outside sandbox (even with sandbox=false)
/// - File matches blocked pattern
/// - Content is not valid UTF-8
/// - Content exceeds 5MB
/// - Parent directory doesn't exist
///
/// # Example
/// ```ignore
/// write_file("output.txt", "Hello, World!", "false", null)
/// write_file("src/module.rs", code_content, "true", null)
/// ```
#[ollama_rs::function]
pub async fn write_file(
    path: String,
    content: String,
    overwrite: Option<String>,
    sandbox: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>>
```

---

### `edit_file`

**Purpose:** Edit an existing file with find/replace, insert, or delete operations.

```rust
/// Edit an existing file with various operations.
///
/// Provides surgical edits to existing files without rewriting the entire content.
/// Safer and more context-efficient than read + write_file.
///
/// # Arguments
/// * `path` - Path to the file to edit.
/// * `operation` - The edit operation to perform:
///   - "replace": Find and replace text (requires `search` + optional `replace`)
///   - "insert": Insert lines after a specific line (requires `after_line` + `content`)
///   - "delete_lines": Delete a range of lines (requires `start_line` + `end_line`)
/// * `search` - Text or regex pattern to find (for "replace" operation).
/// * `replace` - Text to replace with (for "replace" operation).
///   - Use empty string "" to delete matches.
/// * `after_line` - Line number after which to insert (for "insert" operation).
///   - "0" = insert at beginning, "5" = insert after line 5.
/// * `content` - Content to insert (for "insert" operation).
/// * `start_line` - First line to delete (for "delete_lines" operation).
///   - Lines are 1-indexed.
/// * `end_line` - Last line to delete (for "delete_lines" operation).
///   - Use same as start_line to delete single line.
/// * `create_backup` - Create .bak file before editing (default: "false").
/// * `sandbox` - Ignored for write operations (always sandboxed).
///
/// # Returns
/// Success message showing what was changed, or error message.
///
/// # Security
/// - File must exist before editing
/// - Always sandboxed to current directory tree
/// - Blocked for sensitive file patterns
/// - Creates backup file in same directory with .bak extension if requested
///
/// # Example
/// ```ignore
/// // Replace text
/// edit_file("config.yml", "replace", "old_name", "new_name", null, null, null, null, null, null)
///
/// // Insert after line 10
/// edit_file("README.md", "insert", null, null, "10", "## New Section\n\nContent here.", null, null, null, null)
///
/// // Delete lines 5-10
/// edit_file("script.py", "delete_lines", null, null, null, null, "5", "10", null, null)
///
/// // Replace with backup
/// edit_file("important.py", "replace", "old_func", "new_func", null, null, null, null, null, "true")
/// ```
#[ollama_rs::function]
pub async fn edit_file(
    path: String,
    operation: String,
    search: Option<String>,
    replace: Option<String>,
    after_line: Option<String>,
    content: Option<String>,
    start_line: Option<String>,
    end_line: Option<String>,
    create_backup: Option<String>,
    sandbox: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>>
```

---

### `append_file`

**Purpose:** Add content to the end of an existing file.

```rust
/// Append content to the end of an existing file.
///
/// Useful for logging, accumulating output, or extending existing files.
///
/// # Arguments
/// * `path` - Path to the file.
/// * `content` - Content to append to the file.
/// * `create` - Create file if it doesn't exist (default: "false").
///   - "false": Return error if file doesn't exist
///   - "true": Create file if it doesn't exist (same as write_file for new files)
/// * `sandbox` - Ignored for write operations (always sandboxed).
///
/// # Returns
/// Success message with total file size, or error message.
///
/// # Security
/// - Always sandboxed to current directory tree
/// - Blocked for sensitive file patterns
/// - Maximum total file size: 5MB
///
/// # Example
/// ```ignore
/// // Append to existing file
/// append_file("log.txt", "New log entry\n", "false", null)
///
/// // Create if not exists, then append
/// append_file("output.txt", "First line\n", "true", null)
/// append_file("output.txt", "Second line\n", "false", null)
/// ```
#[ollama_rs::function]
pub async fn append_file(
    path: String,
    content: String,
    create: Option<String>,
    sandbox: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>>
```

---

## Implementation Phases

### Phase 1: `write_file` (2-3 days)

**Files to create:**
- `src/tools/files_write.rs` - New module for write operations

**Files to modify:**
- `src/tools/mod.rs` - Export new module
- `src/tools/registry.rs` - Register new tools
- `src/external/config.rs` - Add `blocked_patterns` config
- `src/main.rs` - Load new config section
- `doc/src/tools.md` - Document new tools
- `AGENTS.md` - Update tool documentation guidelines
- `doc/src/development/roadmap.md` - Update status
- `IMPLEMENTATION.md` - Move from Priority 2 to completed

**Implementation steps:**

1. Create `files_write.rs` module
2. Define `DEFAULT_BLOCKED_PATTERNS` constant
3. Implement `validate_write_path()`:
   - Sandbox check (reuse from `validate_path`)
   - Blocklist pattern check
   - Write permission check (test with `OpenOptions::new().write(true)`)
4. Implement `is_blocked_path()`:
   - Check hardcoded patterns
   - Check configured patterns
5. Implement `write_file()`:
   - UTF-8 validation
   - Size limit check (5MB)
   - Atomic write (temp file + rename)
   - Error messages following AGENTS.md guidelines
6. Add config loading for `blocked_patterns`
7. Write unit tests
8. Write integration tests
9. Update documentation

**Atomic write pattern:**

```rust
fn atomic_write(path: &Path, content: &str) -> Result<(), std::io::Error> {
    let temp_path = path.with_extension("tmp");
    
    // Write to temp file
    std::fs::write(&temp_path, content)?;
    
    // Atomic rename
    std::fs::rename(&temp_path, path)?;
    
    Ok(())
}
```

---

### Phase 2: `edit_file` (2-3 days)

**Operations:**

| Operation | Steps |
|-----------|-------|
| `replace` | 1. Read file<br>2. Find matches (literal or regex)<br>3. Replace all occurrences<br>4. Write result |
| `insert` | 1. Read file lines<br>2. Find insertion point<br>3. Insert new lines<br>4. Write result |
| `delete_lines` | 1. Read file lines<br>2. Remove lines in range<br>3. Write result |

**Implementation steps:**

1. Implement `edit_replace()`:
   - Literal search first, then regex
   - Include replacement count in output
   - Show diff preview (up to 5 lines)
2. Implement `edit_insert()`:
   - Line numbering starts at 1
   - `after_line=0` means insert at beginning
   - Preserve line endings (detect LF vs CRLF)
3. Implement `edit_delete_lines()`:
   - Validate line range exists
   - Show deleted lines preview
4. Implement `create_backup()`:
   - Create `{filename}.bak` in same directory
   - Only if requested
5. Write unit tests for each operation
6. Write integration tests

---

### Phase 3: `append_file` (1 day)

**Implementation steps:**

1. Simple implementation:
   - Open file in append mode
   - Write content
   - Return total size
2. Handle `create` parameter:
   - If false and file doesn't exist → error
   - If true and file doesn't exist → create with content
3. Write tests
4. Update documentation

---

## Configuration

### config.toml

Add new section for file write tools:

```toml
[file-tools]
# Maximum file size for write operations (in bytes)
# Default: 5242880 (5MB)
max_file_size = 5242880

# Additional blocked patterns for write operations
# These are in addition to the default blocklist
# Patterns support glob syntax: *, ?, character classes []
blocked_patterns = [
    ".env.*",           # All .env variants
    "*secret*",         # Any file with 'secret' in name
    "*.pem",            # Certificate files
    "config/database.yml",  # Specific sensitive files
]

# Whether to create backups by default for edit operations
# Default: false (must be explicitly requested)
create_backup_default = false
```

### Default Blocked Patterns (in code)

Always blocked, regardless of configuration:

```rust
const DEFAULT_BLOCKED_PATTERNS: &[&str] = &[
    // Environment files
    ".env",
    ".env.local",
    ".env.development",
    ".env.production",
    ".env.staging",
    
    // Secrets and credentials
    "secrets",
    "credentials",
    "secrets.json",
    "credentials.json",
    "secrets.yaml",
    "secrets.yml",
    
    // SSH keys
    "id_rsa",
    "id_dsa",
    "id_ed25519",
    "id_ecdsa",
    
    // Certificates and keys
    ".pem",
    ".key",
    
    // SSH directory
    ".ssh",
    "authorized_keys",
    "known_hosts",
    
    // GPG
    ".gnupg",
    
    // Cloud credentials
    "credentials.json",
    "service-account.json",
];
```

---

## Testing Strategy

### Unit Tests

| Function | Tests |
|----------|-------|
| `validate_write_path()` | - Sandbox enforcement<br>- Blocked patterns<br>- Symlink detection<br>- Directory rejection |
| `is_blocked_path()` | - Default patterns<br>- Configured patterns<br>- Case insensitive matching |
| `write_file()` | - Create new file<br>- Overwrite with permission<br>- Block overwrite<br>- UTF-8 validation<br>- Size limit |
| `edit_file` (replace) | - Literal replace<br>- Regex replace<br>- Delete with empty replace<br>- No match error |
| `edit_file` (insert) | - Insert at beginning<br>- Insert in middle<br>- Insert at end<br>- Invalid line number |
| `edit_file` (delete) | - Delete single line<br>- Delete range<br>- Invalid range |
| `append_file()` | - Append to existing<br>- Create and append<br>- Append to non-existent |

### Integration Tests

1. **Sandbox boundary tests:**
   - Try to write to `../outside.txt`
   - Try to write to `/etc/passwd`
   - Try to write to symlink outside sandbox

2. **Blocked pattern tests:**
   - Try to write to `.env`
   - Try to write to `secrets.json`
   - Try to edit `.ssh/authorized_keys`

3. **Concurrent access tests:**
   - Multiple writes to same file
   - Edit while another process has file open

4. **Large file tests:**
   - Write file at size limit (5MB)
   - Write file over size limit
   - Append to file at size limit

---

## Security Audit Checklist

Before merging, verify:

- [ ] All write operations call `validate_write_path()`
- [ ] `sandbox=false` is ignored for writes
- [ ] Blocked patterns checked BEFORE any file operations
- [ ] Symlinks are resolved and checked
- [ ] File size checked BEFORE writing
- [ ] UTF-8 validation on all content
- [ ] Atomic write pattern used (temp file + rename)
- [ ] Error messages don't leak sensitive paths
- [ ] Unit tests cover all security checks
- [ ] Integration tests test boundary conditions

---

## Documentation Updates

### Files to Update

1. **doc/src/tools.md** - Add documentation for new tools
2. **AGENTS.md** - Update Tool Development Guidelines
3. **doc/src/development/roadmap.md** - Mark as in progress
4. **IMPLEMENTATION.md** - Move Priority 2 to write tools
5. **doc/src/CHANGELOG.md** - Add entry for release

### AGENTS.md Addition

```markdown
### File Write Tools Security

File write operations have additional security requirements:

1. **Always sandboxed** - `sandbox=false` is ignored for write operations
2. **Blocked patterns** - Sensitive files are always blocked
3. **Size limits** - Maximum 5MB per write operation
4. **UTF-8 only** - Binary content is rejected
5. **Atomic writes** - Use temp file + rename pattern

**Blocked patterns by default:**
- Environment files (`.env`, `.env.local`, etc.)
- Secrets and credentials (`secrets.json`, `credentials.json`)
- SSH keys (`id_rsa`, `id_ed25519`, `.ssh/`)
- Certificates (`.pem`, `.key`)
- Cloud credentials (`service-account.json`)

**When implementing new write tools:**
- Always call `validate_write_path()` before operations
- Check `is_blocked_path()` after path resolution
- Implement atomic writes to prevent corruption
- Return meaningful error messages
- Log operations in debug mode
```

---

## Dependencies

- None (independent of Notes System, Skills System, or other planned features)
- Reuses existing sandbox infrastructure from `files.rs`
- Requires only standard library features

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| **Data loss from overwrites** | `overwrite=false` by default, clear error messages |
| **Corrupted files** | Atomic write pattern (temp file + rename) |
| **Security bypass** | Always enforce sandbox for writes, even with `sandbox=false` |
| **Performance on large files** | 5MB limit, stream operations for large content |
| **Regex injection in `edit_file`** | Timeout on regex operations, limit matches |
| **User frustration with blocked files** | Clear error messages explaining why file is blocked |

---

## Success Criteria

1. **Functional:**
   - Can create new files with `write_file`
   - Can edit existing files with `edit_file` (all operations)
   - Can append to files with `append_file`
   - Sandbox is always enforced
   - Blocked patterns prevent writing sensitive files

2. **Security:**
   - Cannot write outside CWD
   - Cannot write blocked patterns
   - Cannot bypass sandbox with `sandbox=false`
   - Size limits enforced

3. **Usability:**
   - Error messages guide LLM to correct usage
   - Operations are context-efficient (edit_file vs read + write)
   - Backup option available for safety

4. **Performance:**
   - Write operations complete in <1 second for typical files
   - Atomic writes prevent corruption
   - Memory usage bounded by size limit

---

## References

- Current file tools: `src/tools/files.rs`
- Sandbox implementation: `src/tools/files.rs` (validate_path)
- Landlock sandbox: `src/tools/run_cmd.rs`
- Configuration system: `src/external/config.rs`
- Anthropic tool design: [Writing Tools for Agents](https://www.anthropic.com/engineering/writing-tools-for-agents)