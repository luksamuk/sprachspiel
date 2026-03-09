//! Legacy session detection and restoration
//!
//! Provides functions to detect JSON sessions not yet migrated to SQLite
//! and restore them on demand.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use super::Database;
use crate::chat::history::ConversationStorage;
use crate::chat::session::{ChatSession, MessageRole};
use crate::consts::roles::{ROLE_ASSISTANT, ROLE_SYSTEM, ROLE_TOOL, ROLE_USER};
use crate::embeddings::EmbeddingClient;

/// Migration statistics for reporting
#[derive(Debug, Clone, Default)]
pub struct MigrationStats {
    pub sessions_migrated: usize,
    pub messages_migrated: usize,
    pub embeddings_generated: usize,
    pub errors: Vec<String>,
}

/// Information about a legacy JSON session
#[derive(Debug, Clone)]
pub struct LegacySession {
    /// Session ID (filename without .json)
    pub id: String,
    /// Session name (if set)
    pub name: Option<String>,
    /// Project ID (directory name)
    pub project_id: Option<String>,
    /// Path to the JSON file
    pub path: PathBuf,
    /// Whether this session exists in SQLite
    pub in_sqlite: bool,
}

/// Convert MessageRole to role string
fn role_to_string(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::User => ROLE_USER,
        MessageRole::Assistant => ROLE_ASSISTANT,
        MessageRole::System => ROLE_SYSTEM,
        MessageRole::Tool => ROLE_TOOL,
    }
}

/// Discover all project directories in the storage path
fn discover_project_dirs(storage: &ConversationStorage) -> Vec<Option<String>> {
    let mut projects = Vec::new();

    // Get the base storage path
    let base_path = storage.base_path();

    if let Ok(entries) = fs::read_dir(base_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Directory name is the project ID (sanitized)
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy().to_string();
                    // "anonymous" is the default for sessions without project
                    projects.push(Some(name_str));
                }
            }
        }
    }

    projects
}

/// Check for legacy JSON sessions not yet in SQLite
///
/// Returns a list of JSON sessions with their migration status.
/// Scans all project directories for JSON files.
pub fn check_legacy_sessions(
    storage: &ConversationStorage,
    db: &Arc<Database>,
) -> Vec<LegacySession> {
    let mut legacy_sessions = Vec::new();

    // Discover all project directories
    let project_dirs = discover_project_dirs(storage);

    // For each project directory, list sessions
    for project_id in &project_dirs {
        let project_path = storage.project_path(project_id);

        if let Ok(entries) = fs::read_dir(&project_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    // Extract session ID from filename
                    if let Some(stem) = path.file_stem() {
                        let session_id = stem.to_string_lossy().to_string();

                        // Try to load session to get name
                        #[allow(deprecated)]
                        let session_name = ChatSession::load(storage, project_id, &session_id)
                            .ok()
                            .and_then(|s| s.name);

                        // Check if conversation exists in SQLite
                        let in_sqlite = db.conversation_exists(&session_id).unwrap_or(false);

                        legacy_sessions.push(LegacySession {
                            id: session_id,
                            name: session_name,
                            project_id: project_id.clone(),
                            path,
                            in_sqlite,
                        });
                    }
                }
            }
        }
    }

    legacy_sessions
}

/// Restore a session from a JSON file to SQLite
///
/// This imports all messages, metadata, and todos from the JSON backup.
/// Useful for recovering sessions that were accidentally deleted from SQLite.
#[allow(deprecated)]
pub fn restore_session(
    storage: &ConversationStorage,
    db: &Arc<Database>,
    project_id: &Option<String>,
    session_id: &str,
) -> Result<ChatSession, String> {
    // Load the JSON session
    let session = ChatSession::load(storage, project_id, session_id)
        .map_err(|e| format!("Failed to load JSON session '{}': {}", session_id, e))?;

    // Check if already in SQLite
    if db.conversation_exists(session_id).unwrap_or(false) {
        return Err(format!(
            "Session '{}' already exists in SQLite. Use `/forget` first if you want to replace it.",
            session_id
        ));
    }

    // Create conversation entry
    let title = session.name.as_deref().unwrap_or(&session.id);
    db.insert_conversation(
        &session.id,
        session.project_id.as_deref(),
        Some(title),
        &session.model,
        session.created_at,
        session.updated_at,
    )
    .map_err(|e| format!("Failed to create conversation: {}", e))?;

    // Insert all messages
    for msg in &session.messages {
        db.insert_message(
            &session.id,
            role_to_string(&msg.role),
            &msg.content,
            msg.timestamp,
        )
        .map_err(|e| format!("Failed to insert message: {}", e))?;
    }

    // Save metadata (summary, think, tools, etc.)
    db.update_conversation_metadata(
        &session.id,
        session.name.as_deref(),
        session.system_prompt.as_deref(),
        session.compacted_summary.as_deref(),
        session.compacted_range,
        session.think,
        session.tools,
        &session.tool_output_level.to_string(),
        session.updated_at,
    )
    .map_err(|e| format!("Failed to save metadata: {}", e))?;

    // Save todos
    let todo_rows = session.todos.to_rows();
    db.save_todos(&session.id, &todo_rows)
        .map_err(|e| format!("Failed to save todos: {}", e))?;

    // Rebuild FTS5 index
    db.rebuild_fts5()
        .map_err(|e| format!("Failed to rebuild search index: {}", e))?;

    // Return the restored session with database attached
    let mut restored = session;
    restored.db = Some(Arc::clone(db));

    Ok(restored)
}

/// Check for uncommitted sessions at startup and print a warning
#[allow(dead_code)]
pub fn print_legacy_warning(legacy_sessions: &[LegacySession]) {
    let uncommitted: Vec<_> = legacy_sessions.iter().filter(|s| !s.in_sqlite).collect();

    if uncommitted.is_empty() {
        return;
    }

    println!();
    println!(
        "⚠️  Warning: {} session(s) not migrated to SQLite:",
        uncommitted.len()
    );
    println!("   These sessions exist as JSON files but are not in the database.");
    println!();

    for session in uncommitted.iter().take(5) {
        let name = session.name.as_deref().unwrap_or(&session.id);
        let project = session.project_id.as_deref().unwrap_or("none");
        println!("   - {} (project: {})", name, project);
    }

    if uncommitted.len() > 5 {
        println!("   ... and {} more", uncommitted.len() - 5);
    }

    println!();
    println!("To restore a session to SQLite, use: /restore <session-id>");
    println!();
}

/// Migrate ALL legacy JSON sessions to SQLite (one-time automatic migration)
///
/// This is executed once on first run after update:
/// - Migrates sessions not yet in SQLite (with embeddings)
/// - Archives ALL JSON files to `archived/` subdirectory
/// - Removes empty project directories
/// - Does NOT touch the OLD/ directory
///
/// Returns migration statistics.
pub async fn migrate_all_legacy_sessions(
    storage: &ConversationStorage,
    db: &Arc<Database>,
    embedding_client: &Arc<EmbeddingClient>,
) -> MigrationStats {
    let mut stats = MigrationStats::default();
    let base_path = storage.base_path();
    let archived_path = base_path.join("archived");

    // Discover all JSON sessions
    let legacy_sessions = check_legacy_sessions(storage, db);

    if legacy_sessions.is_empty() {
        return stats;
    }

    // Separate sessions into two groups:
    // - to_migrate: not in SQLite, need full migration
    // - to_archive: already in SQLite, just need archiving
    let to_migrate: Vec<_> = legacy_sessions.iter().filter(|s| !s.in_sqlite).collect();
    let to_archive: Vec<_> = legacy_sessions.iter().filter(|s| s.in_sqlite).collect();

    let total = legacy_sessions.len();
    println!();
    println!("🔄 Processing {} JSON session(s)...", total);

    // Create archived directory structure
    if let Err(e) = fs::create_dir_all(&archived_path) {
        stats.errors.push(format!("Failed to create archive directory: {}", e));
        return stats;
    }

    // Migrate sessions not yet in SQLite
    for session_info in &to_migrate {
        // Load from JSON
        #[allow(deprecated)]
        match ChatSession::load(storage, &session_info.project_id, &session_info.id) {
            Ok(session) => {
                // Migrate to SQLite with embeddings
                match super::migration::migrate_session(&session, db, embedding_client).await {
                    Ok(session_stats) => {
                        stats.sessions_migrated += 1;
                        stats.messages_migrated += session_stats.messages_migrated;
                        stats.embeddings_generated += session_stats.embeddings_generated;

                        let name = session_info.name.as_deref().unwrap_or(&session_info.id);
                        println!("   ✓ Migrated: {} ({} messages)", name, session_stats.messages_migrated);
                    }
                    Err(e) => {
                        stats.errors.push(format!(
                            "Failed to migrate {}: {}",
                            session_info.id, e
                        ));
                        continue;
                    }
                }
            }
            Err(e) => {
                stats.errors.push(format!(
                    "Failed to load {}: {}",
                    session_info.id, e
                ));
                continue;
            }
        }

        // Archive the JSON file
        if let Err(e) = archive_session(&session_info, &archived_path) {
            stats.errors.push(format!(
                "Failed to archive {}: {}",
                session_info.id, e
            ));
        }
    }

    // Archive sessions already in SQLite (JSON is obsolete)
    for session_info in &to_archive {
        if let Err(e) = archive_session(session_info, &archived_path) {
            stats.errors.push(format!(
                "Failed to archive {}: {}",
                session_info.id, e
            ));
        } else {
            let name = session_info.name.as_deref().unwrap_or(&session_info.id);
            println!("   📦 Archived: {} (already in SQLite)", name);
        }
    }

    // Remove empty project directories (but preserve OLD/ and archived/)
    let project_dirs = discover_project_dirs(storage);
    for project_id in project_dirs {
        // Skip archived and OLD directories
        if project_id.as_deref() == Some("archived") || project_id.as_deref() == Some("OLD") {
            continue;
        }

        let project_path = storage.project_path(&project_id);

        if project_path.exists() {
            // Check if directory contains only JSON files (will be empty after archiving)
            let should_remove = fs::read_dir(&project_path)
                .map(|mut entries| entries.all(|e| e.ok().and_then(|e| e.path().extension().map(|ext| ext == "json")).unwrap_or(false)))
                .unwrap_or(false);

            if should_remove {
                if let Err(e) = fs::remove_dir_all(&project_path) {
                    stats.errors.push(format!(
                        "Failed to remove empty directory {:?}: {}",
                        project_path, e
                    ));
                }
            }
        }
    }

    // Print summary
    println!();
    if stats.sessions_migrated > 0 {
        println!(
            "✅ Migration complete: {} session(s) migrated, {} message(s), {} embedding(s)",
            stats.sessions_migrated,
            stats.messages_migrated,
            stats.embeddings_generated
        );
    }
    
    let archived_count = to_archive.len();
    if archived_count > 0 {
        println!("📦 Archived: {} session(s) (already in SQLite)", archived_count);
    }
    
    println!("   Location: {}", archived_path.display());

    if !stats.errors.is_empty() {
        println!();
        println!("⚠️  Warnings:");
        for e in &stats.errors {
            println!("   - {}", e);
        }
    }
    println!();

    stats
}

/// Archive a session's JSON file to the archived directory
fn archive_session(session_info: &LegacySession, archived_path: &std::path::Path) -> std::io::Result<()> {
    let project_name = session_info.project_id.as_deref().unwrap_or("anonymous");
    let archived_project = archived_path.join(project_name);
    
    fs::create_dir_all(&archived_project)?;
    
    let archived_file = archived_project.join(format!("{}.json", session_info.id));
    fs::rename(&session_info.path, &archived_file)?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_session_detection() {
        // This would require mocking the storage and database
        // For now, we just verify the struct creation
        let session = LegacySession {
            id: "test".to_string(),
            name: Some("Test Session".to_string()),
            project_id: Some("test-project".to_string()),
            path: PathBuf::from("/tmp/test.json"),
            in_sqlite: false,
        };

        assert_eq!(session.id, "test");
        assert!(!session.in_sqlite);
    }
}
