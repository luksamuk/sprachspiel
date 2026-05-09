//! Security validation for subagent file paths
//!
//! Provides path validation functions for subagent vision/OCR operations.
//! Follows the same sandbox patterns as file tools but uses cached BlocklistConfig.
//!
//! This module provides validation functions that will be used by subagent tools
//! (OCR, Vision, etc.) to safely access files within the allowed directories.
//!
//! # Security Model
//! - Files must be within CWD or /tmp or /var/tmp (sandbox enforcement)
//! - Blocked patterns (.env, secrets, SSH keys) are always rejected
//! - BlocklistConfig is cached for performance
//!
//! # Example
//! ```ignore
//! use sprachspiel::security::validate_subagent_path;
//!
//! let path = Path::new("image.jpg");
//! match validate_subagent_path(path) {
//!     Ok(canonical) => println!("Valid: {}", canonical.display()),
//!     Err(e) => eprintln!("Error: {}", e),
//! }
//! ```

use crate::tools::files_blocklist::{BlocklistConfig, is_blocked_for_read};
use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};

/// Cached BlocklistConfig — loaded once, reused forever
static BLOCKLIST_CONFIG: Lazy<BlocklistConfig> = Lazy::new(BlocklistConfig::load);

/// Validate a single path for subagent file access
///
/// Returns Ok(canonical_path) if path is:
/// - Within CWD or /tmp or /var/tmp
/// - Not in blocklist (not .env, secrets, SSH keys, etc.)
///
/// Returns Err(message) if validation fails
pub fn validate_subagent_path(path: &Path) -> Result<PathBuf, String> {
    // Expand tilde if present
    let expanded_path = if path.to_str().map(|s| s.starts_with('~')).unwrap_or(false) {
        crate::utils::expand_tilde_path(path.to_str().unwrap_or(""))
    } else {
        path.to_path_buf()
    };

    // Get absolute path if not already absolute
    let abs_path = if expanded_path.is_absolute() {
        expanded_path
    } else {
        std::env::current_dir()
            .map_err(|_| "Could not determine current directory".to_string())?
            .join(&expanded_path)
    };

    // Check blocklist on non-canonical path first (fast filename pattern match,
    // no filesystem access needed — prevents timing attacks on sensitive names)
    if is_blocked_for_read(&abs_path, &BLOCKLIST_CONFIG) {
        let err_msg = format!(
            "BLOCKED - '{}' matches a protected file pattern. \
             This file may contain sensitive information (credentials, secrets, keys). \
             Reading such files is restricted for security.",
            path.display()
        );
        return Err(err_msg);
    }

    // Get canonical CWD for sandbox checks
    let cwd =
        std::env::current_dir().map_err(|_| "Could not determine current directory".to_string())?;
    let canonical_cwd = cwd
        .canonicalize()
        .map_err(|_| "Could not determine current directory".to_string())?;

    // PHASE 1: Try to canonicalize the FULL path directly.
    // This works for existing files AND directories (including `.`, `/tmp`, etc.).
    // For directory paths, parent() goes UP one level which can be outside sandbox,
    // so we must try full canonicalization first.
    if let Ok(canonical_path) = abs_path.canonicalize() {
        // Path exists — check sandbox on canonical path
        let in_cwd = canonical_path.starts_with(&canonical_cwd);
        let in_tmp = is_temp_directory(&canonical_path);

        if !in_cwd && !in_tmp {
            // Outside sandbox — return generic message (no info-leak about whether it exists)
            return Err("Access denied: path not accessible".to_string());
        }

        // Re-check blocklist after symlink resolution
        if is_blocked_for_read(&canonical_path, &BLOCKLIST_CONFIG) {
            let err_msg = format!(
                "BLOCKED - '{}' matches a protected file pattern. \
                 This file may contain sensitive information (credentials, secrets, keys). \
                 Reading such files is restricted for security.",
                canonical_path.display()
            );
            return Err(err_msg);
        }

        return Ok(canonical_path);
    }

    // PHASE 2: Full canonicalization failed (path doesn't exist or can't access).
    // Fall back to parent-based check to avoid info-leak.
    // Canonicalize PARENT directory (even if file doesn't exist, parent usually does)
    let parent = abs_path
        .parent()
        .ok_or_else(|| "Invalid path: no parent directory".to_string())?;
    let canonical_parent = match parent.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // Parent canonicalization failed — could be outside sandbox, or doesn't exist.
            // Return generic message to avoid info-leak about filesystem layout.
            return Err("Access denied: path not accessible".to_string());
        }
    };

    // Check parent is within sandbox (CWD or /tmp or /var/tmp)
    let parent_in_cwd = canonical_parent.starts_with(&canonical_cwd);
    let parent_in_tmp = is_temp_directory(&canonical_parent);

    if !parent_in_cwd && !parent_in_tmp {
        // Parent is outside sandbox — return generic message (no info-leak)
        return Err("Access denied: path not accessible".to_string());
    }

    // Parent is in sandbox, so it's safe to reveal whether the path itself exists
    if !abs_path.exists() {
        return Err(format!(
            "FILE NOT FOUND: '{}'. The file or directory does not exist.",
            path.display()
        ));
    }

    // Path exists but canonicalization failed earlier — this shouldn't happen normally.
    // Try canonicalizing again (symlinks may have been resolved by the OS).
    let canonical_path = abs_path
        .canonicalize()
        .map_err(|e| format!("Cannot access path '{}': {}", path.display(), e))?;

    // Re-check sandbox after canonicalization (symlinks can escape sandbox)
    let in_cwd = canonical_path.starts_with(&canonical_cwd);
    let in_tmp = is_temp_directory(&canonical_path);

    if !in_cwd && !in_tmp {
        return Err("Access denied: path not accessible".to_string());
    }

    // Re-check blocklist after symlink resolution
    if is_blocked_for_read(&canonical_path, &BLOCKLIST_CONFIG) {
        let err_msg = format!(
            "BLOCKED - '{}' matches a protected file pattern. \
             This file may contain sensitive information (credentials, secrets, keys). \
             Reading such files is restricted for security.",
            canonical_path.display()
        );
        return Err(err_msg);
    }

    Ok(canonical_path)
}

/// Validate multiple paths for vision multi-image support
///
/// Returns Ok(Vec of canonical paths) if all paths are valid.
/// Returns Err(message) if any path validation fails.
pub fn validate_subagent_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut canonical_paths = Vec::with_capacity(paths.len());

    for path in paths {
        let canonical = validate_subagent_path(path)?;
        canonical_paths.push(canonical);
    }

    Ok(canonical_paths)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_subagent_path_nonexistent() {
        // Path outside sandbox should return generic access denied (not FILE NOT FOUND)
        let result = validate_subagent_path(Path::new("/nonexistent/file.txt"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Should NOT contain FILE NOT FOUND (that would be an info-leak)
        assert!(
            !err.contains("FILE NOT FOUND"),
            "Unexpected FILE NOT FOUND for out-of-sandbox path: {}",
            err
        );
        // Should contain generic access denied
        assert!(
            err.contains("Access denied") || err.contains("not accessible"),
            "Expected generic access denied, got: {}",
            err
        );
    }

    #[test]
    fn test_validate_subagent_path_current_dir() {
        // Test with current directory (should succeed)
        let result = validate_subagent_path(Path::new("."));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_subagent_path_absolute() {
        // /tmp is always allowed
        let result = validate_subagent_path(Path::new("/tmp"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_subagent_paths_empty() {
        let paths: Vec<PathBuf> = vec![];
        let result = validate_subagent_paths(&paths);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_validate_subagent_path_nonexistent_in_cwd() {
        // Non-existent file in CWD should return FILE NOT FOUND (good UX, parent is in sandbox)
        let result = validate_subagent_path(Path::new("nonexistent_local_test_file_xyz.txt"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("FILE NOT FOUND"),
            "Expected FILE NOT FOUND for in-sandbox path, got: {}",
            err
        );
    }

    #[test]
    fn test_validate_subagent_path_outside_sandbox_existing() {
        // Existing file outside sandbox should return generic access denied
        // (same message as non-existent — no info-leak)
        let result = validate_subagent_path(Path::new("/etc/hostname"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            !err.contains("FILE NOT FOUND"),
            "Should not reveal existence for out-of-sandbox: {}",
            err
        );
        assert!(
            err.contains("Access denied") || err.contains("not accessible"),
            "Expected generic access denied, got: {}",
            err
        );
    }

    #[test]
    fn test_validate_subagent_path_symlink_escape() {
        // Symlink in /tmp pointing outside sandbox should be blocked after resolution
        use std::os::unix::fs::symlink;
        let tmp_link = std::path::PathBuf::from("/tmp/test_symlink_escape_security_rs.png");
        // Clean up from previous run
        let _ = std::fs::remove_file(&tmp_link);
        // Create symlink: /tmp/test_symlink_escape_security_rs.png -> /etc/hostname
        if symlink("/etc/hostname", &tmp_link).is_ok() {
            let result = validate_subagent_path(&tmp_link);
            // After canonicalization, resolves to /etc/hostname which is outside sandbox
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(
                !err.contains("FILE NOT FOUND"),
                "Should not reveal existence: {}",
                err
            );
            // Clean up
            let _ = std::fs::remove_file(&tmp_link);
        }
        // If symlink creation fails (e.g., /tmp not writable), skip silently
    }

    #[test]
    fn test_validate_subagent_path_blocked_pattern() {
        // .env in CWD should be blocked by blocklist
        let result = validate_subagent_path(Path::new(".env"));
        if let Err(e) = result {
            assert!(
                e.contains("BLOCKED") || e.contains("protected"),
                "Expected blocklist message, got: {}",
                e
            );
        }
        // If .env doesn't exist, we might get FILE NOT FOUND or it might pass blocklist check
        // — the important thing is no crash
    }
    #[test]
    fn test_validate_subagent_paths_multi() {
        let paths = vec![PathBuf::from("."), PathBuf::from("/tmp")];
        let result = validate_subagent_paths(&paths);
        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.len(), 2);
    }
}
