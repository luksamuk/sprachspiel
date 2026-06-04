//! File write operations with security enforcement.
//!
//! This module provides tools for creating, editing, and appending to files.
//! All operations are sandboxed and blocklist-checked for security.
//!
//! # Security Model
//!
//! - **Sandbox always enforced** — file operations are restricted to the current
//!   working directory. The LLM cannot bypass this restriction.
//! - **Blocked patterns** for sensitive files (`.env`, `secrets`, SSH keys, etc.)
//! - **Atomic writes** using temp file + rename pattern
//! - **UTF-8 validation** and **5MB size limit**
//!
//! # Tools
//!
//! - `write_file` - Create or completely overwrite a file
//! - `edit_file` - Surgical edits (replace/insert/delete lines)
//! - `append_file` - Append content to end of file

use super::files_blocklist::{BlocklistConfig, is_blocked_for_write};
use crate::debug_tools::{log_tool_call, log_tool_result};
use crate::utils::{expand_tilde_path, format_size, parse_bool};
use sprachspiel_tool_derive::tool;
use std::path::{Path, PathBuf};

/// Maximum file size for write operations (5MB)
const MAX_WRITE_SIZE: usize = 5_242_880;

// =============================================================================
// write_file
// =============================================================================

/// Write content to a file, creating or overwriting it.
///
/// Creates a new file or completely replaces an existing file's content.
/// Use this when you need to create a new file or replace the entire content.
///
/// # Arguments
/// * `path` - Path to the file (relative to current directory or absolute).
///   - Examples: "output.txt", "src/module.rs", "results/data.json"
/// * `content` - The text content to write to the file.
///   - Must be valid UTF-8 text.
///   - Maximum size: 5MB.
/// * `overwrite` - Whether to overwrite existing file (default: "false"). Optional.
///   - "false" (default): Return error if file exists (safer)
///   - "true": Overwrite existing file
///
/// # Security
/// - Sandbox always enforced — file operations restricted to current working directory
/// - Blocked patterns ALWAYS enforced (cannot write to .env, secrets, etc.)
/// - Maximum file size: 5MB
/// - Atomic write (temp file + rename) to prevent corruption
///
/// # Errors
/// - File exists and overwrite=false
/// - Path matches blocked pattern (ALWAYS blocked)
/// - Path outside sandbox (always enforced)
/// - Content is not valid UTF-8
/// - Content exceeds 5MB
/// - Parent directory doesn't exist
///
/// # Example
/// ```ignore
/// write_file("output.txt", "Hello, World!", "false", null)
/// write_file("src/main.rs", code_content, "true")
/// ```
#[tool]
pub async fn write_file(
    path: String,
    content: String,
    overwrite: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let overwrite = parse_bool(overwrite, false);

    log_tool_call(
        "write_file",
        &[
            ("path".to_string(), path.clone()),
            (
                "content_length".to_string(),
                format!("{} bytes", content.len()),
            ),
            ("overwrite".to_string(), overwrite.to_string()),
        ],
    );

    // Load blocklist configuration
    let config = BlocklistConfig::load();

    // Validate path (blocked patterns ALWAYS enforced, sandbox always enforced)
    let canonical_path = match validate_write_path(&expand_tilde_path(&path), &config) {
        Ok(p) => p,
        Err(e) => {
            log_tool_result("write_file", &e);
            return Ok(e);
        }
    };

    // Check if file exists
    if canonical_path.exists() && !overwrite {
        let err_msg = format!(
            "Error: File '{}' already exists. Use overwrite=true to replace it.",
            path
        );
        log_tool_result("write_file", &err_msg);
        return Ok(err_msg);
    }

    // Validate content size
    if content.len() > MAX_WRITE_SIZE {
        let size_mb = content.len() as f64 / 1_048_576.0;
        let err_msg = format!(
            "Error: Content too large ({:.2} MB). Maximum file size is 5 MB.",
            size_mb
        );
        log_tool_result("write_file", &err_msg);
        return Ok(err_msg);
    }

    // Validate UTF-8 (content is already String, so it's valid UTF-8)
    // This check is implicit since we receive String, not Vec<u8>

    // Atomic write
    if let Err(e) = atomic_write(&canonical_path, &content) {
        let err_msg = format!("Error: Failed to write file '{}': {}", path, e);
        log_tool_result("write_file", &err_msg);
        return Ok(err_msg);
    }

    let size = format_size(content.len() as u64);
    let action = if canonical_path.exists() {
        "overwritten"
    } else {
        "created"
    };

    let result = format!("Successfully wrote {} to '{}' ({}).", size, path, action);
    log_tool_result("write_file", &result);
    Ok(result)
}

// =============================================================================
// edit_file
// =============================================================================

/// Edit an existing file with various operations.
///
/// Provides surgical edits to existing files without rewriting the entire content.
/// Safer and more context-efficient than read_file + write_file.
///
/// # Arguments
/// * `path` - Path to the file to edit.
/// * `operation` - The edit operation to perform:
///   - "replace": Find and replace text (requires `search` + `replace`)
///   - "insert": Insert lines after a specific line (requires `after_line` + `content`)
///   - "delete_lines": Delete a range of lines (requires `start_line` + `end_line`)
/// * `search` - Text or regex pattern to find (for "replace" operation). Optional.
/// * `replace` - Text to replace with (for "replace" operation). Use empty string to delete matches. Optional.
/// * `after_line` - Line number after which to insert (for "insert" operation). Optional.
///   - "0" = insert at beginning
///   - "5" = insert after line 5
/// * `content` - Content to insert (for "insert" operation). Optional.
/// * `start_line` - First line to delete (for "delete_lines" operation). 1-indexed. Optional.
/// * `end_line` - Last line to delete (for "delete_lines" operation). Optional.
///   - Use same as start_line to delete single line.
/// * `create_backup` - Create .bak file before editing (default: "false"). Optional.
///
/// # Returns
/// Success message showing what was changed, or error message.
///
/// # Security
/// - Sandbox always enforced — file operations restricted to current working directory
/// - Blocked patterns ALWAYS enforced (cannot edit .env, secrets, etc.)
/// - Creates backup file with .bak extension if requested
///
/// # Example
/// ```ignore
/// // Replace text
/// edit_file("config.yml", "replace", "old_value", "new_value", null, null, null, null, null)
///
/// // Insert after line 10
/// edit_file("README.md", "insert", null, null, "10", "## New Section\n\nContent.", null, null, null)
///
/// // Delete lines 5-10
/// edit_file("script.py", "delete_lines", null, null, null, null, "5", "10", null)
/// ```
#[tool]
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
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let create_backup = parse_bool(create_backup, false);

    log_tool_call(
        "edit_file",
        &[
            ("path".to_string(), path.clone()),
            ("operation".to_string(), operation.clone()),
            ("create_backup".to_string(), create_backup.to_string()),
        ],
    );

    // Load blocklist configuration
    let config = BlocklistConfig::load();

    // Validate path (blocked patterns ALWAYS enforced, sandbox always enforced)
    let canonical_path = match validate_write_path(&expand_tilde_path(&path), &config) {
        Ok(p) => p,
        Err(e) => {
            log_tool_result("edit_file", &e);
            return Ok(e);
        }
    };

    // File must exist for edit operations
    if !canonical_path.exists() {
        let err_msg = format!(
            "Error: File '{}' does not exist. Use write_file to create new files.",
            path
        );
        log_tool_result("edit_file", &err_msg);
        return Ok(err_msg);
    }

    // Read current content
    let original_content = match std::fs::read_to_string(&canonical_path) {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Error: Cannot read file '{}': {}", path, e);
            log_tool_result("edit_file", &err_msg);
            return Ok(err_msg);
        }
    };

    // Create backup if requested
    if create_backup {
        let backup_path = canonical_path.with_extension("bak");
        if let Err(e) = std::fs::copy(&canonical_path, &backup_path) {
            let err_msg = format!("Error: Failed to create backup: {}", e);
            log_tool_result("edit_file", &err_msg);
            return Ok(err_msg);
        }
    }

    // Perform edit operation
    let new_content = match operation.as_str() {
        "replace" => edit_replace(&original_content, search.as_deref(), replace.as_deref()),
        "insert" => edit_insert(&original_content, after_line.as_deref(), content.as_deref()),
        "delete_lines" => edit_delete_lines(
            &original_content,
            start_line.as_deref(),
            end_line.as_deref(),
        ),
        _ => {
            let err_msg = format!(
                "Error: Invalid operation '{}'. Must be 'replace', 'insert', or 'delete_lines'.",
                operation
            );
            log_tool_result("edit_file", &err_msg);
            return Ok(err_msg);
        }
    };

    // Handle edit result
    let new_content = match new_content {
        Ok(c) => c,
        Err(e) => {
            log_tool_result("edit_file", &e);
            return Ok(e);
        }
    };

    // Check if content changed
    if new_content == original_content {
        let result = format!("No changes made to '{}' (content unchanged).", path);
        log_tool_result("edit_file", &result);
        return Ok(result);
    }

    // Check size limit
    if new_content.len() > MAX_WRITE_SIZE {
        let size_mb = new_content.len() as f64 / 1_048_576.0;
        let err_msg = format!(
            "Error: Resulting file too large ({:.2} MB). Maximum is 5 MB.",
            size_mb
        );
        log_tool_result("edit_file", &err_msg);
        return Ok(err_msg);
    }

    // Atomic write
    if let Err(e) = atomic_write(&canonical_path, &new_content) {
        let err_msg = format!("Error: Failed to write file '{}': {}", path, e);
        log_tool_result("edit_file", &err_msg);
        return Ok(err_msg);
    }

    // Calculate diff statistics
    let original_lines = original_content.lines().count();
    let new_lines = new_content.lines().count();

    let result = format!(
        "Successfully edited '{}': {} lines -> {} lines ({:+}). Operation: {}",
        path,
        original_lines,
        new_lines,
        new_lines as isize - original_lines as isize,
        operation
    );
    log_tool_result("edit_file", &result);
    Ok(result)
}

// =============================================================================
// append_file
// =============================================================================

/// Append content to the end of an existing file.
///
/// Useful for logging, accumulating output, or extending existing files.
///
/// # Arguments
/// * `path` - Path to the file.
/// * `content` - Content to append to the file.
/// * `create` - Create file if it doesn't exist (default: "false"). Optional.
///   - "false": Return error if file doesn't exist
///   - "true": Create file if it doesn't exist
///
/// # Returns
/// Success message with total file size, or error message.
///
/// # Security
/// - Sandbox always enforced — file operations restricted to current working directory
/// - Blocked patterns ALWAYS enforced (cannot append to .env, secrets, etc.)
/// - Maximum total file size: 5MB
///
/// # Example
/// ```ignore
/// // Append to existing file
/// append_file("log.txt", "New log entry\n", "false")
///
/// // Create if not exists, then append
/// append_file("output.txt", "First line\n", "true")
/// ```
#[tool]
pub async fn append_file(
    path: String,
    content: String,
    create: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let create = parse_bool(create, false);
    log_tool_call(
        "append_file",
        &[
            ("path".to_string(), path.clone()),
            (
                "content_length".to_string(),
                format!("{} bytes", content.len()),
            ),
            ("create".to_string(), create.to_string()),
        ],
    );

    // Load blocklist configuration
    let config = BlocklistConfig::load();

    // Validate path (blocked patterns ALWAYS enforced, sandbox always enforced)
    let canonical_path = match validate_write_path(&expand_tilde_path(&path), &config) {
        Ok(p) => p,
        Err(e) => {
            log_tool_result("append_file", &e);
            return Ok(e);
        }
    };

    // Check if file exists
    let file_exists = canonical_path.exists();

    if !file_exists && !create {
        let err_msg = format!(
            "Error: File '{}' does not exist. Use create=true to create it.",
            path
        );
        log_tool_result("append_file", &err_msg);
        return Ok(err_msg);
    }

    // Check total size for existing files
    let existing_size = if file_exists {
        match std::fs::metadata(&canonical_path) {
            Ok(m) => m.len() as usize,
            Err(e) => {
                let err_msg = format!("Error: Cannot read file metadata: {}", e);
                log_tool_result("append_file", &err_msg);
                return Ok(err_msg);
            }
        }
    } else {
        0
    };

    let total_size = existing_size + content.len();
    if total_size > MAX_WRITE_SIZE {
        let existing_mb = existing_size as f64 / 1_048_576.0;
        let new_mb = content.len() as f64 / 1_048_576.0;
        let err_msg = format!(
            "Error: Resulting file too large ({:.2} MB + {:.2} MB = {:.2} MB). Maximum is 5 MB.",
            existing_mb,
            new_mb,
            total_size as f64 / 1_048_576.0
        );
        log_tool_result("append_file", &err_msg);
        return Ok(err_msg);
    }

    // Append content
    let result = if file_exists {
        // Append to existing file
        use std::io::Write;
        let file = match std::fs::OpenOptions::new()
            .append(true)
            .open(&canonical_path)
        {
            Ok(f) => f,
            Err(e) => {
                let err_msg = format!("Error: Cannot open file '{}': {}", path, e);
                log_tool_result("append_file", &err_msg);
                return Ok(err_msg);
            }
        };

        match std::io::BufWriter::new(file).write_all(content.as_bytes()) {
            Ok(()) => format!(
                "Successfully appended {} to '{}' (total: {}).",
                format_size(content.len() as u64),
                path,
                format_size(total_size as u64)
            ),
            Err(e) => {
                let err_msg = format!("Error: Failed to append to '{}': {}", path, e);
                log_tool_result("append_file", &err_msg);
                return Ok(err_msg);
            }
        }
    } else {
        // Create new file
        if let Err(e) = atomic_write(&canonical_path, &content) {
            let err_msg = format!("Error: Failed to create file '{}': {}", path, e);
            log_tool_result("append_file", &err_msg);
            return Ok(err_msg);
        }
        format!(
            "Successfully created '{}' with {}.",
            path,
            format_size(content.len() as u64)
        )
    };

    log_tool_result("append_file", &result);
    Ok(result)
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Validate path for write operations.
///
/// This function enforces security requirements for all write operations:
/// 1. Blocked patterns ALWAYS enforced (cannot write sensitive files)
/// 2. Sandbox ALWAYS enforced (cannot write outside current working directory)
/// 3. Parent directory must exist
pub fn validate_write_path(path: &Path, config: &BlocklistConfig) -> Result<PathBuf, String> {
    // 1. Get absolute path
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| "Could not determine current directory".to_string())?
            .join(path)
    };

    // 2. Check blocked patterns BEFORE any filesystem access
    // This prevents timing attacks on sensitive file names
    // Blocked patterns are ALWAYS enforced, regardless of sandbox setting
    if is_blocked_for_write(&abs_path, config) {
        return Err(format!(
            "Error: Cannot write to '{}'. This path matches a protected pattern \
             (environment files, secrets, SSH keys, certificates, or credentials).",
            path.display()
        ));
    }

    // 3. Validate parent exists (file doesn't need to exist for writes)
    let parent = abs_path
        .parent()
        .ok_or_else(|| "Invalid path: no parent directory".to_string())?;

    if !parent.exists() {
        return Err(format!(
            "Error: Parent directory does not exist: {}",
            parent.display()
        ));
    }

    // 4. Canonicalize parent for sandbox check
    let canonical_parent = parent
        .canonicalize()
        .map_err(|e| format!("Cannot access parent directory: {}", e))?;

    // 5. Sandbox is ALWAYS enforced — check that the path is within CWD
    // or within allowed temporary directories
    let cwd =
        std::env::current_dir().map_err(|_| "Could not determine current directory".to_string())?;
    let canonical_cwd = cwd
        .canonicalize()
        .map_err(|_| "Could not determine current directory".to_string())?;

    if !canonical_parent.starts_with(&canonical_cwd) {
        // Allow /tmp and /var/tmp (needed for tool interop, e.g., pdftotext output)
        if !is_temp_directory(&canonical_parent) {
            return Err(format!(
                "Error: Path '{}' is outside the allowed directory. \
                 File operations are restricted to the current working directory.",
                path.display()
            ));
        }
    }

    // 6. Check if path is a symlink and resolve it
    if path.exists() {
        let canonical_path = abs_path
            .canonicalize()
            .map_err(|e| format!("Cannot resolve path: {}", e))?;

        // Re-check blocked patterns after resolving symlinks
        if is_blocked_for_write(&canonical_path, config) {
            return Err(format!(
                "Error: Cannot write to '{}'. This path (resolved from symlink) matches a protected pattern.",
                canonical_path.display()
            ));
        }

        // Re-check sandbox after resolving symlinks (always enforced)
        let cwd = std::env::current_dir()
            .map_err(|_| "Could not determine current directory".to_string())?;
        let canonical_cwd = cwd
            .canonicalize()
            .map_err(|_| "Could not determine current directory".to_string())?;

        if !canonical_path.starts_with(&canonical_cwd) && !is_temp_directory(&canonical_path) {
            return Err(format!(
                "Error: Path '{}' (resolved from symlink) is outside the allowed directory.",
                path.display()
            ));
        }

        return Ok(canonical_path);
    }

    Ok(abs_path)
}

/// Check if a canonical path is within an allowed temporary directory.
fn is_temp_directory(canonical_path: &Path) -> bool {
    // Check /tmp (standard temporary directory)
    if let Ok(canonical_tmp) = Path::new("/tmp").canonicalize()
        && canonical_path.starts_with(&canonical_tmp)
    {
        return true;
    }
    // Check /var/tmp (persistent temporary directory)
    if let Ok(canonical_var_tmp) = Path::new("/var/tmp").canonicalize()
        && canonical_path.starts_with(&canonical_var_tmp)
    {
        return true;
    }
    false
}

/// Atomic write using temp file + rename pattern.
///
/// This prevents corruption if the write is interrupted.
fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let temp_path = path.with_extension("tmp");

    // Write to temp file first
    std::fs::write(&temp_path, content)
        .map_err(|e| format!("Failed to write temporary file: {}", e))?;

    // Atomic rename to final location
    std::fs::rename(&temp_path, path).map_err(|e| {
        // Clean up temp file on failure
        let _ = std::fs::remove_file(&temp_path);
        format!("Failed to rename file: {}", e)
    })?;

    Ok(())
}

/// Edit operation: find and replace text.
fn edit_replace(
    content: &str,
    search: Option<&str>,
    replace: Option<&str>,
) -> Result<String, String> {
    let search = search
        .ok_or_else(|| "Error: 'search' parameter required for 'replace' operation.".to_string())?;

    let replace_text = replace.unwrap_or("");

    if search.is_empty() {
        return Err("Error: 'search' pattern cannot be empty.".to_string());
    }

    // Count matches
    let count = content.matches(search).count();
    if count == 0 {
        return Err(format!(
            "Error: Pattern '{}' not found in file. No changes made.",
            search
        ));
    }

    // Replace all occurrences
    let new_content = content.replace(search, replace_text);

    Ok(new_content)
}

/// Edit operation: insert content after a specific line.
fn edit_insert(
    content: &str,
    after_line: Option<&str>,
    insert_content: Option<&str>,
) -> Result<String, String> {
    let after_line_num = after_line
        .ok_or_else(|| {
            "Error: 'after_line' parameter required for 'insert' operation.".to_string()
        })?
        .parse::<usize>()
        .map_err(|_| "Error: 'after_line' must be a valid number.".to_string())?;

    let insert_text = insert_content
        .ok_or_else(|| "Error: 'content' parameter required for 'insert' operation.".to_string())?;

    let lines: Vec<&str> = content.lines().collect();

    if after_line_num > lines.len() {
        return Err(format!(
            "Error: 'after_line' {} exceeds file length ({} lines).",
            after_line_num,
            lines.len()
        ));
    }

    // Insert at position (after_line_num = 0 means insert at beginning)
    let mut new_lines = Vec::with_capacity(lines.len() + insert_text.lines().count());
    new_lines.extend(lines.iter().take(after_line_num));
    new_lines.extend(insert_text.lines());
    new_lines.extend(lines.iter().skip(after_line_num));

    // Preserve trailing newline if original had one
    let new_content = if content.ends_with('\n') {
        new_lines.join("\n") + "\n"
    } else {
        new_lines.join("\n")
    };

    Ok(new_content)
}

/// Edit operation: delete a range of lines.
fn edit_delete_lines(
    content: &str,
    start_line: Option<&str>,
    end_line: Option<&str>,
) -> Result<String, String> {
    let start = start_line
        .ok_or_else(|| {
            "Error: 'start_line' parameter required for 'delete_lines' operation.".to_string()
        })?
        .parse::<usize>()
        .map_err(|_| "Error: 'start_line' must be a valid number.".to_string())?;

    let end = end_line
        .ok_or_else(|| {
            "Error: 'end_line' parameter required for 'delete_lines' operation.".to_string()
        })?
        .parse::<usize>()
        .map_err(|_| "Error: 'end_line' must be a valid number.".to_string())?;

    if start == 0 {
        return Err("Error: Line numbers start at 1, not 0.".to_string());
    }

    if end < start {
        return Err(format!(
            "Error: 'end_line' ({}) must be >= 'start_line' ({}).",
            end, start
        ));
    }

    let lines: Vec<&str> = content.lines().collect();

    if start > lines.len() {
        return Err(format!(
            "Error: 'start_line' {} exceeds file length ({} lines).",
            start,
            lines.len()
        ));
    }

    if end > lines.len() {
        return Err(format!(
            "Error: 'end_line' {} exceeds file length ({} lines).",
            end,
            lines.len()
        ));
    }

    // Convert to 0-indexed and delete range
    let start_idx = start - 1;
    let end_idx = end;

    let new_lines: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| *i < start_idx || *i >= end_idx)
        .map(|(_, line)| *line)
        .collect();

    // Preserve trailing newline if original had one
    let new_content = if content.ends_with('\n') {
        new_lines.join("\n") + "\n"
    } else {
        new_lines.join("\n")
    };

    Ok(new_content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_atomic_write_creates_file() {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = dir.path().join("test.txt");

        atomic_write(&file_path, "Hello, World!").expect("Failed to write");

        assert!(file_path.exists());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "Hello, World!");
    }

    #[test]
    fn test_atomic_write_overwrites() {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = dir.path().join("test.txt");

        atomic_write(&file_path, "Original").expect("Failed to write");
        atomic_write(&file_path, "Replaced").expect("Failed to write");

        assert_eq!(fs::read_to_string(&file_path).unwrap(), "Replaced");
    }

    #[test]
    fn test_validate_write_path_blocks_sensitive_files() {
        let config = BlocklistConfig::default();

        // Should block .env files (blocked patterns always enforced)
        let result = validate_write_path(&PathBuf::from(".env"), &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("protected pattern"));

        // Should block secrets files
        let result = validate_write_path(&PathBuf::from("secrets.json"), &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_edit_replace_finds_and_replaces() {
        let content = "Hello, World!\nGoodbye, World!";
        let result = edit_replace(content, Some("World"), Some("Universe"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello, Universe!\nGoodbye, Universe!");
    }

    #[test]
    fn test_edit_replace_deletes_with_empty_replace() {
        let content = "Hello, World!\nGoodbye!";
        let result = edit_replace(content, Some("World!\n"), Some(""));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello, Goodbye!");
    }

    #[test]
    fn test_edit_replace_errors_on_no_match() {
        let content = "Hello, World!";
        let result = edit_replace(content, Some("NotFound"), Some("Replaced"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_edit_insert_at_beginning() {
        let content = "Line 2\nLine 3";
        let result = edit_insert(content, Some("0"), Some("Line 1\n"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_edit_insert_in_middle() {
        let content = "Line 1\nLine 3";
        let result = edit_insert(content, Some("1"), Some("Line 2\n"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_edit_insert_at_end() {
        // Content "Line 1\nLine 2" has 2 lines
        // Insert after line 2 (at end) with "Line 3" (no leading newline)
        let content = "Line 1\nLine 2";
        let result = edit_insert(content, Some("2"), Some("Line 3"));
        assert!(result.is_ok());
        // Result: "Line 1\nLine 2\nLine 3"
        assert_eq!(result.unwrap(), "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_edit_delete_lines_single_line() {
        let content = "Line 1\nLine 2\nLine 3";
        let result = edit_delete_lines(content, Some("2"), Some("2"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Line 1\nLine 3");
    }

    #[test]
    fn test_edit_delete_lines_range() {
        let content = "Line 1\nLine 2\nLine 3\nLine 4";
        let result = edit_delete_lines(content, Some("2"), Some("3"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Line 1\nLine 4");
    }

    #[test]
    fn test_edit_delete_lines_errors_on_invalid_range() {
        let content = "Line 1\nLine 2";
        let result = edit_delete_lines(content, Some("3"), Some("4"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds file length"));
    }
}
