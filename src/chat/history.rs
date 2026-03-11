//! Legacy conversation storage for migration and restore.
//!
//! This module is DEPRECATED and should NOT be used in new code.
//! Use `Database` from `src/db/operations.rs` instead.
//!
//! ## Purpose
//! This module exists solely to support:
//! - `/restore` command (import JSON backups)
//! - Automatic migration detection on startup
//!
//! ## Migration Path
//! After all JSON sessions are migrated, this module will be removed.
//! See `src/db/legacy_check.rs` for migration logic.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::session::ChatSession;

/// Information about a saved session
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[deprecated(note = "Legacy struct, not used in SQLite storage")]
pub struct SessionInfo {
    pub id: String,
    pub name: Option<String>,
    pub model: String,
    pub message_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Manages conversation storage on disk (JSON-based)
///
/// **DEPRECATED**: Only used for legacy migration and `/restore` command.
#[deprecated(note = "Use SQLite storage via Database struct instead")]
#[allow(deprecated)]
pub struct ConversationStorage {
    base_path: PathBuf,
}

#[allow(deprecated)]
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

    /// Get the base storage path
    pub fn base_path(&self) -> &PathBuf {
        &self.base_path
    }

    /// Get the path for a project's sessions
    pub fn project_path(&self, project_id: &Option<String>) -> PathBuf {
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn list_sessions(&self, project_id: &Option<String>) -> Vec<SessionInfo> {
        let project_path = self.project_path(project_id);
        let mut sessions = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&project_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json")
                    && let Ok(json) = std::fs::read_to_string(&path)
                    && let Ok(session) = serde_json::from_str::<ChatSession>(&json)
                {
                    sessions.push(session.to_info());
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
    #[allow(dead_code)]
    pub fn session_exists(&self, project_id: &Option<String>, session_id: &str) -> bool {
        self.session_path(project_id, session_id).exists()
    }

    /// Get the default session ID for a project
    #[allow(dead_code)]
    pub fn default_session_id() -> String {
        "default".to_string()
    }
}

#[allow(deprecated)]
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
///
/// **DEPRECATED**: Use `crate::project::get_project_id()` instead.
#[allow(dead_code)]
#[deprecated(note = "Use crate::project::get_project_id() instead")]
pub fn get_project_id() -> Option<String> {
    crate::project::get_project_id()
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
