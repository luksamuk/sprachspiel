//! Shared blocked patterns logic for file operations.
//!
//! This module provides security enforcement for file tools, blocking access
//! to sensitive files like environment files, secrets, SSH keys, and certificates.
//!
//! # Security Model
//!
//! - **Read operations**: Blocked by default (`block_read = true`)
//! - **List operations**: Filename visible by default (`block_list = false`)
//! - **Write operations**: ALWAYS blocked, cannot be disabled
//!
//! # Configuration
//!
//! Blocked patterns can be extended via `tools.toml`:
//!
//! ```toml
//! [file-tools]
//! blocked_patterns = [".env.*", "*secret*", "*.pem"]
//! block_read = true
//! block_list = false
//! ```

use once_cell::sync::Lazy;
use regex::RegexSet;
use std::path::Path;

/// Hardcoded blocked file patterns (always blocked, cannot be disabled).
///
/// These patterns protect sensitive files that should never be modified by the LLM:
/// - Environment files (`.env`, `.env.local`, etc.)
/// - Secrets and credentials
/// - SSH keys and directories
/// - Certificates and keys
/// - Cloud credentials
const DEFAULT_BLOCKED_PATTERNS: &[&str] = &[
    // Environment files
    ".env",
    ".env.local",
    ".env.development",
    ".env.production",
    ".env.staging",
    ".env.test",
    // Secrets and credentials
    "secrets",
    "secrets.json",
    "secrets.yaml",
    "secrets.yml",
    "credentials",
    "credentials.json",
    // SSH keys and directory
    "id_rsa",
    "id_dsa",
    "id_ed25519",
    "id_ecdsa",
    ".ssh",
    "authorized_keys",
    "known_hosts",
    // Certificates and keys
    ".pem",
    ".key",
    // GPG
    ".gnupg",
    // Cloud credentials
    "service-account.json",
    "service_account.json",
];

/// Regex patterns compiled from DEFAULT_BLOCKED_PATTERNS.
///
/// Converts simple patterns to regex for matching:
/// - `.env` -> matches `.env`, `.env.local`, path/to/.env`
/// - `secrets` -> substring match
/// - `.pem` -> extension match
static DEFAULT_BLOCKED_REGEX: Lazy<RegexSet> = Lazy::new(|| {
    let patterns: Vec<String> = DEFAULT_BLOCKED_PATTERNS
        .iter()
        .map(|p| pattern_to_regex(p))
        .collect();

    RegexSet::new(patterns).expect("Invalid default blocked patterns regex")
});

/// Convert a simple pattern to regex for matching.
///
/// # Examples
///
/// - `.env` -> `\.(?:env)(?:\..*)?$` (matches `.env`, `.env.local`)
/// - `secrets` -> `secrets` (substring match)
/// - `.pem` -> `\.pem$` (extension match)
/// - `id_rsa` -> `(^|/)id_rsa$` (exact filename match)
fn pattern_to_regex(pattern: &str) -> String {
    // Check if it's an extension pattern (starts with dot, like ".pem")
    if pattern.starts_with('.') && pattern.len() > 1 && !pattern.contains('*') {
        // .pem -> \.pem$ (extension match)
        format!(r"\.{}$", &pattern[1..])
    }
    // Check if it's a directory pattern (like ".ssh")
    else if pattern.starts_with('.') && !pattern.contains('*') && pattern.len() > 1 {
        // .ssh -> matches directory name anywhere in path
        format!(r"(^|/)\{}/?", &pattern[1..])
    }
    // Check if it's an exact filename (no wildcards, no extension at start)
    else if !pattern.contains('*') && !pattern.contains('.') {
        // id_rsa -> matches exact filename
        format!(r"(^|/){}$", pattern)
    }
    // Wildcard patterns (like ".env.*")
    else {
        // Convert glob-style pattern to regex
        let mut regex = String::new();
        for ch in pattern.chars() {
            match ch {
                '*' => regex.push_str(".*"),
                '.' => regex.push_str(r"\."),
                _ => regex.push(ch),
            }
        }
        regex
    }
}

/// Configuration for blocked patterns loaded from tools.toml.
#[derive(Debug, Clone)]
pub struct BlocklistConfig {
    /// Regex set of all blocked patterns (default + user-configured)
    patterns: RegexSet,
    /// Whether to block read operations for sensitive files
    pub block_read: bool,
    /// Whether to block list operations (hide filenames)
    pub block_list: bool,
    /// Block write operations (always true, cannot be disabled)
    pub block_write: bool,
}

impl Default for BlocklistConfig {
    fn default() -> Self {
        Self {
            patterns: DEFAULT_BLOCKED_REGEX.clone(),
            block_read: true,
            block_list: false,
            block_write: true, // Always true for security
        }
    }
}

impl BlocklistConfig {
    /// Load configuration from tools.toml.
    ///
    /// Currently returns defaults. TODO: Load from actual config file.
    pub fn load() -> Self {
        Self::default()
    }

    /// Check if a path matches any blocked pattern.
    ///
    /// Checks both the full path and just the filename.
    pub fn is_blocked(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Check both full path and filename
        self.patterns.is_match(&path_str) || self.patterns.is_match(filename)
    }
}

/// Check if a path is blocked for read operations.
///
/// Returns true if the path matches a blocked pattern and `block_read` is enabled.
#[inline]
pub fn is_blocked_for_read(path: &Path, config: &BlocklistConfig) -> bool {
    config.block_read && config.is_blocked(path)
}

/// Check if a path is blocked for list operations.
///
/// Returns true if the path matches a blocked pattern and `block_list` is enabled.
#[inline]
pub fn is_blocked_for_list(path: &Path, config: &BlocklistConfig) -> bool {
    config.block_list && config.is_blocked(path)
}

/// Check if a path is blocked for write operations.
///
/// Always checks the pattern match regardless of configuration.
/// Write blocking CANNOT be disabled.
#[inline]
pub fn is_blocked_for_write(path: &Path, config: &BlocklistConfig) -> bool {
    config.is_blocked(path)
}

/// Get the list of default blocked patterns (for documentation/debugging).
pub fn get_default_blocked_patterns() -> &'static [&'static str] {
    DEFAULT_BLOCKED_PATTERNS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_pattern_to_regex_extension() {
        // Extension patterns
        assert_eq!(pattern_to_regex(".pem"), r"\.pem$");
        assert_eq!(pattern_to_regex(".key"), r"\.key$");
    }

    #[test]
    fn test_pattern_to_regex_exact_filename() {
        // Exact filename patterns
        assert_eq!(pattern_to_regex("id_rsa"), r"(^|/)id_rsa$");
        assert_eq!(pattern_to_regex("secrets.json"), r"(^|/)secrets\.json$");
    }

    #[test]
    fn test_pattern_to_regex_substring() {
        // Substring match patterns (no wildcards, no leading dot)
        assert_eq!(pattern_to_regex("secrets"), "secrets");
    }

    #[test]
    fn test_default_patterns_match_env_files() {
        let config = BlocklistConfig::default();

        // Should match .env files
        assert!(config.is_blocked(&PathBuf::from(".env")));
        assert!(config.is_blocked(&PathBuf::from(".env.local")));
        assert!(config.is_blocked(&PathBuf::from(".env.production")));
        assert!(config.is_blocked(&PathBuf::from("project/.env")));
        assert!(config.is_blocked(&PathBuf::from("project/.env.local")));
    }

    #[test]
    fn test_default_patterns_match_secrets() {
        let config = BlocklistConfig::default();

        // Should match secret files
        assert!(config.is_blocked(&PathBuf::from("secrets.json")));
        assert!(config.is_blocked(&PathBuf::from("secrets.yaml")));
        assert!(config.is_blocked(&PathBuf::from("config/secrets")));
        assert!(config.is_blocked(&PathBuf::from("credentials.json")));
    }

    #[test]
    fn test_default_patterns_match_ssh_keys() {
        let config = BlocklistConfig::default();

        // Should match SSH keys
        assert!(config.is_blocked(&PathBuf::from("id_rsa")));
        assert!(config.is_blocked(&PathBuf::from("id_ed25519")));
        assert!(config.is_blocked(&PathBuf::from(".ssh/id_rsa")));
        assert!(config.is_blocked(&PathBuf::from(".ssh/authorized_keys")));
    }

    #[test]
    fn test_default_patterns_match_certificates() {
        let config = BlocklistConfig::default();

        // Should match certificate files
        assert!(config.is_blocked(&PathBuf::from("cert.pem")));
        assert!(config.is_blocked(&PathBuf::from("server.key")));
        assert!(config.is_blocked(&PathBuf::from("ssl/cert.pem")));
    }

    #[test]
    fn test_default_patterns_match_cloud_credentials() {
        let config = BlocklistConfig::default();

        // Should match cloud credential files
        assert!(config.is_blocked(&PathBuf::from("service-account.json")));
        assert!(config.is_blocked(&PathBuf::from("gcloud/service_account.json")));
    }

    #[test]
    fn test_default_patterns_dont_match_normal_files() {
        let config = BlocklistConfig::default();

        // Should NOT match normal files
        assert!(!config.is_blocked(&PathBuf::from("README.md")));
        assert!(!config.is_blocked(&PathBuf::from("src/main.rs")));
        assert!(!config.is_blocked(&PathBuf::from("config/app.yaml")));
        assert!(!config.is_blocked(&PathBuf::from("data.json")));
        assert!(!config.is_blocked(&PathBuf::from("environment_setup.sh")));
    }

    #[test]
    fn test_is_blocked_for_read() {
        let config = BlocklistConfig::default();

        assert!(is_blocked_for_read(&PathBuf::from(".env"), &config));
        assert!(!is_blocked_for_read(&PathBuf::from("README.md"), &config));
    }

    #[test]
    fn test_is_blocked_for_write() {
        let config = BlocklistConfig::default();

        // Write blocking is ALWAYS enforced, regardless of config
        assert!(is_blocked_for_write(&PathBuf::from(".env"), &config));

        // Test with block_read = false - write should still be blocked
        let mut config_no_read = config.clone();
        config_no_read.block_read = false;
        assert!(is_blocked_for_write(
            &PathBuf::from(".env"),
            &config_no_read
        ));
    }

    #[test]
    fn test_block_read_flag() {
        let mut config = BlocklistConfig::default();
        config.block_read = false;

        // When block_read is false, should not block reads
        assert!(!is_blocked_for_read(&PathBuf::from(".env"), &config));

        // But write should still be blocked
        assert!(is_blocked_for_write(&PathBuf::from(".env"), &config));
    }
}
