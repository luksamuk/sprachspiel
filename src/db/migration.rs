//! Session migration from JSON to SQLite
//!
//! Migrates JSON session files to SQLite database with embeddings.

use std::sync::Arc;

use chrono::Utc;

use super::Database;
use crate::chat::session::{ChatSession, MessageRole};
use crate::embeddings::EmbeddingClient;
use crate::chat::history::ConversationStorage;

/// Migration statistics
#[derive(Debug, Clone, Default)]
pub struct MigrationStats {
    /// Number of sessions migrated
    pub sessions_migrated: usize,
    /// Total messages migrated
    pub messages_migrated: usize,
    /// Total embeddings generated
    pub embeddings_generated: usize,
    /// Number of errors
    pub errors: Vec<String>,
}

/// Migrate a single session from JSON to SQLite
pub async fn migrate_session(
    session: &ChatSession,
    db: &Arc<Database>,
    embedding_client: &Arc<EmbeddingClient>,
) -> Result<MigrationStats, String> {
    let mut stats = MigrationStats::default();
    
    if session.anonymous {
        return Err("Cannot migrate anonymous session".to_string());
    }
    
    // Ensure conversation exists
    let title = session.name.as_deref().unwrap_or(&session.id);
    db.insert_conversation(
        &session.id,
        session.project_id.as_deref(),
        Some(title),
        &session.model,
        session.created_at,
        session.updated_at,
    ).map_err(|e| format!("Failed to insert conversation: {}", e))?;
    
    stats.sessions_migrated = 1;
    
    // Migrate all messages with embeddings
    let messages = session.messages.clone();
    let total_messages = messages.len();
    
    for (idx, msg) in messages.iter().enumerate() {
        // Insert message
        let message_id = db.insert_message(
            &session.id,
            match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System => "system",
                MessageRole::Tool => "tool",
            },
            &msg.content,
            msg.timestamp,
        ).map_err(|e| format!("Failed to insert message: {}", e))?;
        
        stats.messages_migrated += 1;
        
        // Generate embedding for user messages only
        if msg.role == MessageRole::User {
            match embedding_client.embed(&msg.content).await {
                Ok(embedding) => {
                    if let Err(e) = db.update_message_embedding(
                        message_id,
                        &embedding,
                        &session.id,
                        msg.timestamp,
                    ) {
                        stats.errors.push(format!(
                            "Failed to save embedding for message {}: {}",
                            idx, e
                        ));
                    } else {
                        stats.embeddings_generated += 1;
                    }
                }
                Err(e) => {
                    stats.errors.push(format!(
                        "Failed to generate embedding for message {}: {}",
                        idx, e
                    ));
                }
            }
        }
        
        // Print progress every 10 messages
        if total_messages > 10 && (idx + 1) % 10 == 0 {
            println!(
                "  Progress: {}/{} messages ({} embeddings)",
                idx + 1,
                total_messages,
                stats.embeddings_generated
            );
        }
    }
    
    // Rebuild FTS5 index after migration
    if let Err(e) = db.rebuild_fts5() {
        stats.errors.push(format!("Failed to rebuild FTS5 index: {}", e));
    }
    
    Ok(stats)
}

/// Migrate all sessions for a project from JSON to SQLite
pub async fn migrate_project(
    storage: &ConversationStorage,
    project_id: &Option<String>,
    db: &Arc<Database>,
    embedding_client: &Arc<EmbeddingClient>,
) -> Result<MigrationStats, String> {
    let mut stats = MigrationStats::default();
    
    let sessions = storage.list_sessions(project_id);
    let total_sessions = sessions.len();
    
    if total_sessions == 0 {
        println!("No sessions found for project.");
        return Ok(stats);
    }
    
    println!("Found {} session(s) to migrate.", total_sessions);
    
    for (idx, info) in sessions.iter().enumerate() {
        let session_name = info.name.as_deref().unwrap_or(&info.id);
        println!(
            "[{}/{}] Migrating session: {} ({} messages)",
            idx + 1,
            total_sessions,
            session_name,
            info.message_count
        );
        
        match ChatSession::load(storage, project_id, &info.id) {
            Ok(session) => {
                match migrate_session(&session, db, embedding_client).await {
                    Ok(session_stats) => {
                        stats.sessions_migrated += session_stats.sessions_migrated;
                        stats.messages_migrated += session_stats.messages_migrated;
                        stats.embeddings_generated += session_stats.embeddings_generated;
                        stats.errors.extend(session_stats.errors);
                    }
                    Err(e) => {
                        stats.errors.push(format!("Failed to migrate session {}: {}", session_name, e));
                    }
                }
            }
            Err(e) => {
                stats.errors.push(format!("Failed to load session {}: {}", session_name, e));
            }
        }
    }
    
    // Rebuild FTS5 index after all migrations complete
    println!("Rebuilding search index...");
    match db.rebuild_fts5() {
        Ok(count) => println!("Indexed {} messages for keyword search.", count),
        Err(e) => stats.errors.push(format!("Failed to rebuild FTS5 index: {}", e)),
    }
    
    Ok(stats)
}

/// Reindex embeddings for all messages in a conversation
pub async fn reindex_conversation(
    db: &Arc<Database>,
    embedding_client: &Arc<EmbeddingClient>,
    conversation_id: &str,
) -> Result<MigrationStats, String> {
    let mut stats = MigrationStats::default();
    
    // Get all messages for the conversation
    let messages = db.get_conversation_messages(conversation_id, None)
        .map_err(|e| format!("Failed to get messages: {}", e))?;
    
    if messages.is_empty() {
        println!("No messages found in conversation.");
        return Ok(stats);
    }
    
    println!("Reindexing {} message(s)...", messages.len());
    
    for (idx, msg) in messages.iter().enumerate() {
        // Only reindex user messages
        if msg.role != "user" {
            continue;
        }
        
        match embedding_client.embed(&msg.content).await {
            Ok(embedding) => {
                // Convert timestamp
                let timestamp = chrono::DateTime::from_timestamp(msg.timestamp, 0)
                    .unwrap_or_else(Utc::now);
                    
                if let Err(e) = db.update_message_embedding(
                    msg.message_id,
                    &embedding,
                    conversation_id,
                    timestamp,
                ) {
                    stats.errors.push(format!(
                        "Failed to update embedding for message {}: {}",
                        idx, e
                    ));
                } else {
                    stats.embeddings_generated += 1;
                }
            }
            Err(e) => {
                stats.errors.push(format!(
                    "Failed to generate embedding for message {}: {}",
                    idx, e
                ));
            }
        }
        
        // Print progress every 10 messages
        if messages.len() > 10 && (idx + 1) % 10 == 0 {
            println!(
                "  Progress: {}/{} embeddings",
                idx + 1,
                messages.len()
            );
        }
    }
    
    stats.messages_migrated = messages.len();
    
    Ok(stats)
}