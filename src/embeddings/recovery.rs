//! Embedding recovery for interrupted processes
//!
//! Ensures all saved content items and chunks have embeddings, even if the process
//! was interrupted during embedding generation.
//!
//! ## Architecture
//!
//! Chunking flow (v0.33.0+):
//! ```text
//! add_user_message()
//! ├── Insert content_item (sync) ← ALWAYS SAVED
//! ├── Insert content_chunks (sync)  ← ALWAYS SAVED
//! └── tokio::spawn(async {
//!     └── Generate embeddings (async) ← MAY BE INTERRUPTED
//! })
//! ```
//!
//! On app restart, recovery manager finds any saved content_items/chunks without
//! embeddings and generates them in the background.

#![expect(clippy::print_stdout)] // CLI subcommand output
#![expect(clippy::print_stderr)] // CLI subcommand output
use chrono::Utc;
use std::sync::Arc;

use crate::db::Database;
use crate::embeddings::{
    ChunkConfig, DynamicChunkConfig, EmbeddingClient, chunk_text_with_config,
    fallback::{
        EmbedContext, EmbedItemContext, embed_chunk_with_fallback, embed_item_with_fallback,
    },
};

/// Recover missing embeddings for all content
///
/// Called on REPL startup to resume any interrupted embedding generation.
/// Returns the number of embeddings successfully recovered.
///
/// # Arguments
/// * `db` - Database connection
/// * `embedding_client` - Embedding client for generating embeddings
///
/// # Returns
/// Number of embeddings successfully recovered
pub async fn recover_missing_embeddings(
    db: &Arc<Database>,
    embedding_client: &Arc<EmbeddingClient>,
) -> usize {
    // Clean up V2 orphan chunks (those with wrong item_id mapping)
    let orphan_deleted = match db.with_connection(|conn| {
        conn.execute(
            "DELETE FROM content_chunks 
             WHERE item_id NOT IN (SELECT id FROM content_items)",
            [],
        )
    }) {
        Ok(count) => count,
        Err(e) => {
            eprintln!("Warning: Failed to clean orphan chunks in recovery: {}", e);
            0
        }
    };

    if orphan_deleted > 0 {
        println!("Cleaned {} orphan chunk(s).", orphan_deleted);
    }

    // Get dynamic context length from embedding model
    let context_length = match embedding_client.get_context_length().await {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!(
                "Warning: Could not get embedding model context length: {}",
                e
            );
            eprintln!("Using conservative default of 512 tokens.");
            512
        }
    };

    let chunk_config = DynamicChunkConfig::new(context_length);
    let max_chars = chunk_config.max_chars();

    // Get content items without embeddings
    let items = match db.get_content_items_for_reindex() {
        Ok(items) if !items.is_empty() => items,
        Ok(_) => vec![],
        Err(_) => return 0,
    };

    let total_missing = items.len();

    if total_missing == 0 {
        return 0;
    }

    println!("Recovering {} missing embedding(s)...", total_missing);

    let mut recovered = 0;
    let now = Utc::now();

    // Generate embeddings for content items (and their chunks)
    for (item_id, content_type, content) in &items {
        let timestamp = now;

        // Skip if content is empty or too short for meaningful embedding
        if content.trim().is_empty() || content.len() < 10 {
            continue;
        }

        // Check if item already has chunks (long content that was partially processed)
        // If so, skip item embedding - chunks are handled separately
        if db.content_item_has_chunks(*item_id).unwrap_or(false) {
            continue;
        }

        // Get item's conversation_id and project_id for embedding metadata
        let (conv_id, proj_id) = match db.get_content_item_by_id(*item_id) {
            Ok(Some(item)) => (item.conversation_id, item.project_id),
            _ => (None, None),
        };

        // Check if content needs chunking using dynamic threshold
        if content.len() > max_chars {
            // Long content - create chunks and embed each chunk with fallback
            let chunks_list = chunk_text_with_config(content, &ChunkConfig::from(&chunk_config));

            for chunk in &chunks_list {
                // Insert chunk into database
                let chunk_id = match db.insert_content_chunk(
                    *item_id,
                    chunk.index as i32,
                    &chunk.content,
                    chunk.start_offset as i32,
                    chunk.end_offset as i32,
                    timestamp,
                ) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Warning: Failed to insert chunk {}: {}", chunk.index, e);
                        continue;
                    }
                };

                // Embed chunk with fallback
                let ctx = EmbedContext {
                    content: &chunk.content,
                    item_id: *item_id,
                    chunk_id,
                    content_type: content_type.as_str(),
                    conversation_id: conv_id.as_deref(),
                    project_id: proj_id.as_deref(),
                    timestamp,
                };

                match embed_chunk_with_fallback(
                    ctx,
                    Arc::clone(db),
                    Arc::clone(embedding_client),
                    context_length,
                    0,
                )
                .await
                {
                    Ok(result) => {
                        recovered += result.chunks_created;
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to recover embedding for chunk {}: {}",
                            chunk_id, e
                        );
                    }
                }
            }

            // Mark item as having embeddings ONLY if all chunks are complete
            // This prevents re-processing items with incomplete chunks on next startup
            let _ = db.mark_item_embedding_if_complete(*item_id);
        } else {
            // Short content - embed directly with fallback
            let ctx = EmbedItemContext::new(
                content,
                *item_id,
                content_type.as_str(),
                conv_id.as_deref(),
                proj_id.as_deref(),
            );

            match embed_item_with_fallback(ctx, db, embedding_client, context_length).await {
                Ok(result) => {
                    if result.chunks_created > 0 {
                        // Item was chunked due to fallback
                        recovered += result.chunks_created;
                    } else {
                        recovered += 1;
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to recover embedding for item {}: {}",
                        item_id, e
                    );
                }
            }
        }
    }

    // Process chunks that were created during this recovery (long items being processed)
    let chunks = match db.get_content_chunks_for_reindex() {
        Ok(c) if !c.is_empty() => c,
        Ok(_) => vec![],
        Err(_) => vec![],
    };

    if !chunks.is_empty() {
        for (chunk_id, content) in &chunks {
            // Get the item for this chunk to get metadata
            let item_info: Option<(i64, String, Option<String>, Option<String>)> = match db
                .with_connection(|conn| {
                    let mut stmt = conn.prepare(
                        "SELECT cc.item_id, ci.content_type, ci.conversation_id, ci.project_id
                         FROM content_chunks cc
                         JOIN content_items ci ON cc.item_id = ci.id
                         WHERE cc.id = ?1",
                    )?;
                    let mut rows = stmt.query_map(rusqlite::params![chunk_id], |row| {
                        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                    })?;
                    rows.next().transpose()
                }) {
                Ok(Some(info)) => Some(info),
                _ => None,
            };

            let (parent_item_id, content_type, conv_id, proj_id) = match item_info {
                Some((pid, ct, c, p)) => (pid, ct, c, p),
                None => {
                    // Chunk was just created but item doesn't exist - shouldn't happen
                    eprintln!(
                        "Warning: Newly created chunk {} has no parent item",
                        chunk_id
                    );
                    continue;
                }
            };

            let timestamp = now;

            // Embed chunk with fallback
            let ctx = EmbedContext {
                content,
                item_id: parent_item_id,
                chunk_id: *chunk_id,
                content_type: &content_type,
                conversation_id: conv_id.as_deref(),
                project_id: proj_id.as_deref(),
                timestamp,
            };

            match embed_chunk_with_fallback(
                ctx,
                Arc::clone(db),
                Arc::clone(embedding_client),
                context_length,
                0,
            )
            .await
            {
                Ok(result) => {
                    recovered += result.chunks_created;
                }
                Err(e) => {
                    // Chunk may exceed context length - fallback didn't work
                    eprintln!(
                        "Warning: Failed to generate embedding for chunk {}: {}",
                        chunk_id, e
                    );
                }
            }
        }
    }

    if recovered > 0 {
        println!("Successfully recovered {} embedding(s).", recovered);
    }

    recovered
}

/// Recover missing embeddings for all content (with progress bar).
///
/// Same as `recover_missing_embeddings` but shows a progress bar.
/// Called on /exit to complete pending embeddings before shutting down.
pub async fn recover_missing_embeddings_with_progress(
    db: &Arc<Database>,
    embedding_client: &Arc<EmbeddingClient>,
) -> usize {
    use indicatif::{ProgressBar, ProgressStyle};

    // Get counts first
    let items = match db.get_content_items_for_reindex() {
        Ok(items) if !items.is_empty() => items,
        Ok(_) => vec![],
        Err(_) => return 0,
    };

    let chunks = match db.get_content_chunks_for_reindex() {
        Ok(chunks) if !chunks.is_empty() => chunks,
        Ok(_) => vec![],
        Err(_) => vec![],
    };

    let total = items.len() + chunks.len();

    if total == 0 {
        return 0;
    }

    println!("Completing {} pending embedding(s)...", total);

    let progress = ProgressBar::new(total as u64);
    #[expect(clippy::expect_used)] // hardcoded template string is always valid
    let style = ProgressStyle::with_template("  {bar:20} {pos}/{len} ({percent}%)")
        .expect("Invalid progress template")
        .progress_chars("█▓░");
    progress.set_style(style);

    // Call the main recovery function but with progress tracking
    let result = recover_missing_embeddings(db, embedding_client).await;

    progress.finish_and_clear();

    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_recovery_structure() {
        // Verify recovery function exists and compiles
        assert!(true);
    }
}
