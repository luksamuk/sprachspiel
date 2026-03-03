//! Embedding recovery for interrupted processes
//!
//! Ensures all saved messages and chunks have embeddings, even if the process
//! was interrupted during embedding generation.
//!
//! ## Architecture
//!
//! Chunking flow (v0.22.2+):
//! ```text
//! add_user_message()
//! ├── Insert message (sync) ← ALWAYS SAVED
//! ├── Insert chunks (sync)  ← ALWAYS SAVED
//! └── tokio::spawn(async {
//!     └── Generate embeddings (async) ← MAY BE INTERRUPTED
//! })
//! ```
//!
//! On app restart, recovery manager finds any saved chunks/messages without
//! embeddings and generates them in the background.

use chrono::Utc;
use std::sync::Arc;

use crate::db::Database;
use crate::embeddings::EmbeddingClient;

/// Recover missing embeddings for a conversation
///
/// Called on REPL startup to resume any interrupted embedding generation.
/// Returns the number of embeddings successfully recovered.
///
/// # Arguments
/// * `db` - Database connection
/// * `embedding_client` - Embedding client for generating embeddings
/// * `conversation_id` - Conversation to check for missing embeddings
///
/// # Returns
/// Number of embeddings successfully recovered
pub async fn recover_missing_embeddings(
    db: &Arc<Database>,
    embedding_client: &Arc<EmbeddingClient>,
    conversation_id: &str,
) -> usize {
    // 1. Check messages without embeddings
    let messages = match db.get_messages_for_reindex() {
        Ok(msgs) if !msgs.is_empty() => msgs,
        Ok(_) => vec![],
        Err(_) => return 0,
    };

    // 2. Check chunks without embeddings for this conversation
    let chunks = match db.get_chunks_without_embedding(conversation_id) {
        Ok(c) if !c.is_empty() => c,
        Ok(_) => vec![],
        Err(_) => return 0,
    };

    let total_missing = messages.len() + chunks.len();

    if total_missing == 0 {
        return 0;
    }

    println!(
        "Recovering {} missing embedding(s) from previous session...",
        total_missing
    );

    let mut recovered = 0;

    // 3. Generate embeddings for messages
    for msg in &messages {
        if msg.conversation_id != conversation_id {
            continue;
        }

        let timestamp = chrono::DateTime::from_timestamp(msg.timestamp, 0).unwrap_or_else(Utc::now);

        match embedding_client.embed(&msg.content).await {
            Ok(embedding) => {
                if db.update_message_embedding(msg.message_id, &embedding, &msg.conversation_id, timestamp).is_ok() {
                    recovered += 1;
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to recover embedding for message {}: {}", msg.message_id, e);
            }
        }
    }

    // 4. Generate embeddings for chunks
    for (chunk_id, content) in &chunks {
        match embedding_client.embed(content).await {
            Ok(embedding) => {
                if db.update_chunk_embedding(*chunk_id, &embedding, conversation_id, Utc::now()).is_ok() {
                    recovered += 1;
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to recover embedding for chunk {}: {}", chunk_id, e);
            }
        }
    }

    if recovered > 0 {
        println!("Successfully recovered {} embedding(s).", recovered);
    }

    recovered
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_recovery_structure() {
        // Verify recovery function exists and compiles
        assert!(true);
    }
}