//! Session migration from JSON to SQLite
//!
//! Migrates JSON session files to SQLite database with embeddings.
//! Applies chunking to long messages (>1024 chars) for better semantic search.

use std::sync::Arc;

use chrono::Utc;

use super::Database;
use crate::chat::session::{ChatSession, MessageRole};
use crate::consts::roles::{ROLE_ASSISTANT, ROLE_SYSTEM, ROLE_TOOL, ROLE_USER};
use crate::embeddings::{EmbeddingClient, chunk_text, needs_chunking};
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
    /// Total chunks created
    pub chunks_created: usize,
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
                MessageRole::User => ROLE_USER,
                MessageRole::Assistant => ROLE_ASSISTANT,
                MessageRole::System => ROLE_SYSTEM,
                MessageRole::Tool => ROLE_TOOL,
            },
            &msg.content,
            msg.timestamp,
        ).map_err(|e| format!("Failed to insert message: {}", e))?;
        
        stats.messages_migrated += 1;
        
        // Generate embeddings for ALL roles (not just user)
        // Apply chunking for long messages
        if needs_chunking(&msg.content) {
            // Long message: split into chunks
            let chunks = chunk_text(&msg.content);
            
            for chunk in &chunks {
                let chunk_id = db.insert_chunk(
                    message_id,
                    chunk.index as i32,
                    &chunk.content,
                    chunk.start_offset as i32,
                    chunk.end_offset as i32,
                    msg.timestamp,
                ).map_err(|e| format!("Failed to insert chunk: {}", e))?;
                
                // Generate embedding for chunk
                match embedding_client.embed(&chunk.content).await {
                    Ok(embedding) => {
                        if let Err(e) = db.update_chunk_embedding(
                            chunk_id,
                            &embedding,
                            &session.id,
                            msg.timestamp,
                        ) {
                            stats.errors.push(format!(
                                "Failed to save chunk embedding for message {}: {}",
                                idx, e
                            ));
                        } else {
                            stats.embeddings_generated += 1;
                        }
                    }
                    Err(e) => {
                        stats.errors.push(format!(
                            "Failed to generate chunk embedding for message {}: {}",
                            idx, e
                        ));
                    }
                }
            }
            
            stats.chunks_created += chunks.len();
        } else {
            // Short message: single embedding
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
                "  Progress: {}/{} messages ({} embeddings, {} chunks)",
                idx + 1,
                total_messages,
                stats.embeddings_generated,
                stats.chunks_created
            );
        }
    }
    
    // Rebuild FTS5 index after migration
    if let Err(e) = db.rebuild_fts5() {
        stats.errors.push(format!("Failed to rebuild FTS5 index: {}", e));
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
        // Reindex ALL roles (not just user)
        
        // Convert timestamp
        let timestamp = chrono::DateTime::from_timestamp(msg.timestamp, 0)
            .unwrap_or_else(Utc::now);
        
        // Apply chunking for long messages
        if needs_chunking(&msg.content) {
            // Long message: split into chunks
            let chunks = chunk_text(&msg.content);
            
            for chunk in &chunks {
                let chunk_id = db.insert_chunk(
                    msg.message_id,
                    chunk.index as i32,
                    &chunk.content,
                    chunk.start_offset as i32,
                    chunk.end_offset as i32,
                    timestamp,
                ).map_err(|e| format!("Failed to insert chunk: {}", e))?;
                
                match embedding_client.embed(&chunk.content).await {
                    Ok(embedding) => {
                        if let Err(e) = db.update_chunk_embedding(
                            chunk_id,
                            &embedding,
                            conversation_id,
                            timestamp,
                        ) {
                            stats.errors.push(format!(
                                "Failed to update chunk embedding for message {}: {}",
                                idx, e
                            ));
                        } else {
                            stats.embeddings_generated += 1;
                        }
                    }
                    Err(e) => {
                        stats.errors.push(format!(
                            "Failed to generate chunk embedding for message {}: {}",
                            idx, e
                        ));
                    }
                }
            }
            
            stats.chunks_created += chunks.len();
        } else {
            // Short message: single embedding
            match embedding_client.embed(&msg.content).await {
                Ok(embedding) => {
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
        }
        
        // Print progress every 10 messages
        if messages.len() > 10 && (idx + 1) % 10 == 0 {
            println!(
                "  Progress: {}/{} embeddings ({} chunks)",
                idx + 1,
                messages.len(),
                stats.chunks_created
            );
        }
    }
    
    stats.messages_migrated = messages.len();
    
    Ok(stats)
}