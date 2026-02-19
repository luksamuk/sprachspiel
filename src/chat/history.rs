//! Project identification and conversation persistence
//!
//! Handles:
//! - Identifying the current project (git remote URL or folder name fallback)
//! - Storing and retrieving conversation sessions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// Information about a saved session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub name: Option<String>,
    pub model: String,
    pub message_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Manages conversation storage on disk
pub struct ConversationStorage {
    base_path: PathBuf,
}

impl ConversationStorage {
    /// Create a new storage instance
    pub fn new() -> Self {
        let base_path = Self::get_storage_path();
        Self { base_path }
    }

    /// Get the base storage path (~/.local/share/ask-ai/conversations/)
    fn get_storage_path() -> PathBuf {
        if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(data_home)
                .join("ask-ai")
                .join("conversations")
        } else if let Some(home_dir) = dirs::home_dir() {
            home_dir
                .join(".local")
                .join("share")
                .join("ask-ai")
                .join("conversations")
        } else {
            PathBuf::from(".ask-ai").join("conversations")
        }
    }

    /// Get the path for a project's sessions
    fn project_path(&self, project_id: &Option<String>) -> PathBuf {
        match project_id {
            Some(id) => {
                let safe_id = sanitize_path(id);
                self.base_path.join(safe_id)
            }
            None => self.base_path.join("anonymous"),
        }
    }

    /// Get the path for a specific session file
    pub fn session_path(&self, project_id: &Option<String>, session_id: &str) -> PathBuf {
        self.project_path(project_id)
            .join(format!("{}.json", session_id))
    }

    /// Save a session to disk
    pub fn save_session<T: Serialize>(
        &self,
        project_id: &Option<String>,
        session_id: &str,
        session: &T,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path = self.session_path(project_id, session_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(session)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Load a session from disk
    pub fn load_session<T: for<'de> Deserialize<'de>>(
        &self,
        project_id: &Option<String>,
        session_id: &str,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let path = self.session_path(project_id, session_id);
        let json = std::fs::read_to_string(&path)?;
        let session: T = serde_json::from_str(&json)?;
        Ok(session)
    }

    /// List sessions for a project
    pub fn list_sessions(&self, project_id: &Option<String>) -> Vec<SessionInfo> {
        let project_path = self.project_path(project_id);
        let mut sessions = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&project_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json")
                    && let Ok(json) = std::fs::read_to_string(&path)
                    && let Ok(info) = serde_json::from_str::<SessionInfo>(&json)
                {
                    sessions.push(info);
                }
            }
        }

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions
    }

    /// Delete a session
    #[allow(dead_code)]
    pub fn delete_session(
        &self,
        project_id: &Option<String>,
        session_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path = self.session_path(project_id, session_id);
        std::fs::remove_file(&path)?;
        Ok(())
    }

    /// Check if a session exists
    pub fn session_exists(&self, project_id: &Option<String>, session_id: &str) -> bool {
        self.session_path(project_id, session_id).exists()
    }

    /// Get the default session ID for a project
    pub fn default_session_id() -> String {
        "default".to_string()
    }
}

impl Default for ConversationStorage {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the current project identifier
///
/// Returns:
/// - Normalized git remote origin URL if in a git repo with origin
/// - Current folder name as fallback
/// - None if no identifier can be determined
pub fn get_project_id() -> Option<String> {
    get_git_remote_url().or_else(get_folder_name)
}

/// Get the git remote origin URL
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

/// Get the current folder name
fn get_folder_name() -> Option<String> {
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
}

/// Normalize a git URL to a consistent format
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

/// Sanitize a string for use as a path component
fn sanitize_path(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '/' {
                c
            } else {
                '_'
            }
        })
        .collect()
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

    #[test]
    fn test_sanitize_path() {
        assert_eq!(
            sanitize_path("github.com/user/repo"),
            "github_com/user/repo"
        );
        assert_eq!(sanitize_path("simple"), "simple");
        assert_eq!(sanitize_path("with spaces"), "with_spaces");
    }
}
