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
//! use ask_ai::security::validate_subagent_path;
//!
//! let path = Path::new("image.jpg");
//! match validate_subagent_path(path) {
//!     Ok(canonical) => println!("Valid: {}", canonical.display()),
//!     Err(e) => eprintln!("Error: {}", e),
//! }
//! ```
#![allow(dead_code)]

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

    // Check if path exists before canonicalizing
    if !expanded_path.exists() {
        return Err(format!(
            "FILE NOT FOUND: '{}'. The file or directory does not exist.",
            path.display()
        ));
    }

    // Canonicalize to resolve symlinks and normalize
    let canonical_path = match expanded_path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return Err(format!("Cannot access path '{}': {}", path.display(), e));
        }
    };

    // Sandbox is ALWAYS enforced — check that the path is within CWD
    // or within allowed temporary directories
    let cwd = match std::env::current_dir() {
        Ok(c) => c,
        Err(_) => {
            return Err("Could not determine current directory".to_string());
        }
    };
    let canonical_cwd = match cwd.canonicalize() {
        Ok(c) => c,
        Err(_) => {
            return Err("Could not determine current directory".to_string());
        }
    };

    // Check if path is within CWD
    if canonical_path.starts_with(&canonical_cwd) {
        // Check blocklist for sensitive files
        if is_blocked_for_read(&canonical_path, &BLOCKLIST_CONFIG) {
            let err_msg = format!(
                "Error: BLOCKED - '{}' matches a protected file pattern. \
                 This file may contain sensitive information (credentials, secrets, keys). \
                 Reading such files is restricted for security.",
                path.display()
            );
            return Err(err_msg);
        }
        return Ok(canonical_path);
    }

    // Allow /tmp and /var/tmp (needed for tool interop, e.g., pdftotext output)
    if is_temp_directory(&canonical_path) {
        // Check blocklist for sensitive files in temp directories
        if is_blocked_for_read(&canonical_path, &BLOCKLIST_CONFIG) {
            let err_msg = format!(
                "Error: BLOCKED - '{}' matches a protected file pattern. \
                 This file may contain sensitive information (credentials, secrets, keys). \
                 Reading such files is restricted for security.",
                path.display()
            );
            return Err(err_msg);
        }
        return Ok(canonical_path);
    }

    Err(format!(
        "Path '{}' is outside the allowed directory. \
         File operations are restricted to the current working directory \
         and temporary directories (/tmp, /var/tmp).",
        path.display()
    ))
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
        let result = validate_subagent_path(Path::new("/nonexistent/file.txt"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("FILE NOT FOUND"));
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
    fn test_validate_subagent_paths_multi() {
        let paths = vec![PathBuf::from("."), PathBuf::from("/tmp")];
        let result = validate_subagent_paths(&paths);
        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.len(), 2);
    }
}
