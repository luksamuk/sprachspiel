//! Project identification utilities.
//!
//! Provides functions to identify the current project from git repository
//! or filesystem context.

use std::process::Command;

/// Get the current project identifier.
///
/// Returns:
/// - Normalized git remote origin URL if in a git repo with origin
/// - Current folder name as fallback
/// - None if no identifier can be determined
pub fn get_project_id() -> Option<String> {
    get_git_remote_url().or_else(get_folder_name)
}

/// Get the git remote origin URL.
fn get_git_remote_url() -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;

    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !url.is_empty() {
            return Some(normalize_git_url(&url));
        }
    }
    None
}

/// Get the current folder name.
fn get_folder_name() -> Option<String> {
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
}

/// Normalize a git URL to a consistent format.
///
/// Examples:
/// - git@github.com:user/repo.git -> github.com/user/repo
/// - https://github.com/user/repo.git -> github.com/user/repo
/// - git@gitlab.com:user/repo -> gitlab.com/user/repo
pub fn normalize_git_url(url: &str) -> String {
    let url = url.trim();

    if url.starts_with("git@") {
        let url = url.strip_prefix("git@").unwrap_or(url);
        let url = url.replace(':', "/");
        let url = url.strip_suffix(".git").unwrap_or(&url);
        url.to_string()
    } else if url.starts_with("https://") || url.starts_with("http://") {
        let url = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .unwrap_or(url);
        let url = url.strip_suffix(".git").unwrap_or(url);
        url.to_string()
    } else if url.ends_with(".git") {
        url.strip_suffix(".git").unwrap_or(url).to_string()
    } else {
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_git_url_ssh() {
        assert_eq!(
            normalize_git_url("git@github.com:user/repo.git"),
            "github.com/user/repo"
        );
    }

    #[test]
    fn test_normalize_git_url_https() {
        assert_eq!(
            normalize_git_url("https://github.com/user/repo.git"),
            "github.com/user/repo"
        );
    }

    #[test]
    fn test_normalize_git_url_https_no_suffix() {
        assert_eq!(
            normalize_git_url("https://github.com/user/repo"),
            "github.com/user/repo"
        );
    }

    #[test]
    fn test_normalize_git_url_gitlab() {
        assert_eq!(
            normalize_git_url("git@gitlab.com:user/repo.git"),
            "gitlab.com/user/repo"
        );
    }
}
