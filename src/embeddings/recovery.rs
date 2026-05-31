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
//!
//! ## Background recovery and concurrency safety
//!
//! Since v0.44.0, the recovery pipeline runs as a background `tokio::spawn` task
//! in the TUI, allowing the event loop to start immediately. The `Database` struct
//! uses `Arc<Mutex<Connection>>`, which serializes all SQLite accesses. This means:
//!
//! - **No "database is locked" errors** — the Mutex ensures recovery and RAG queries
//!   are serialized, not concurrent.
//! - **No incorrect results** — items with `has_embedding = 0` are excluded from
//!   vector search results, so partial recovery only yields temporarily incomplete
//!   (never incorrect) search results.
//! - **No exit flush needed** — missing embeddings are recovered on next startup.
//!   The previous synchronous flush on `/quit` could block exit for minutes.
//!
//! # Output modes
//!
//! - `quiet = false` (terminal mode): prints progress and errors to stdout/stderr,
//!   shows an indicatif progress bar (in `_with_progress` variant)
//! - `quiet = true` (TUI mode): suppresses all direct terminal output, logs
//!   warnings via `log::warn!`, uses a hidden progress bar. The caller (TUI)
//!   shows status messages through the `ChatView` instead.

#![expect(clippy::print_stdout)] // Terminal-mode output (guarded by `quiet` flag)
#![expect(clippy::print_stderr)] // Terminal-mode output (guarded by `quiet` flag)
use chrono::Utc;
use std::sync::Arc;

use crate::chat::app::{EmbeddingPhase, EmbeddingProgress, EmbeddingProgressTx};
use crate::db::Database;
use crate::embeddings::{
    ChunkConfig, DynamicChunkConfig, EmbeddingClient, MIN_EMBED_CONTENT_LEN,
    chunk_text_with_config,
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
/// * `quiet` - When `true`, suppresses terminal output and progress bars (TUI mode).
///   Warnings are logged via `log::warn!` instead of `eprintln!`.
///   When `false`, prints status to stdout/stderr.
///
/// # Returns
/// Number of embeddings successfully recovered
pub async fn recover_missing_embeddings(
    db: &Arc<Database>,
    embedding_client: &Arc<EmbeddingClient>,
    quiet: bool,
    progress_tx: Option<EmbeddingProgressTx>,
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
            if quiet {
                log::warn!("Failed to clean orphan chunks in recovery: {}", e);
            } else {
                eprintln!("Warning: Failed to clean orphan chunks in recovery: {}", e);
            }
            0
        }
    };

    if orphan_deleted > 0 && !quiet {
        println!("Cleaned {} orphan chunk(s).", orphan_deleted);
    }

    // Get dynamic context length from embedding model
    let context_length = match embedding_client.get_context_length().await {
        Ok(ctx) => ctx,
        Err(e) => {
            if quiet {
                log::warn!("Could not get embedding model context length: {}", e);
            } else {
                eprintln!(
                    "Warning: Could not get embedding model context length: {}",
                    e
                );
                eprintln!("Using conservative default of 512 tokens.");
            }
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

    // Also count pre-existing chunks without embeddings
    let preexisting_chunks = match db.get_content_chunks_for_reindex() {
        Ok(c) => c.len(),
        _ => 0,
    };

    // Dynamic total: items + pre-existing chunks, grows when chunking splits items.
    let mut total_missing = items.len() + preexisting_chunks;

    if total_missing == 0 {
        return 0;
    }

    if !quiet {
        println!("Recovering {} missing embedding(s)...", total_missing);
    }

    // Track processed count manually for accurate progress.
    let mut processed: usize = 0;

    // Entity counts track items (documents/messages); embedding counts track
    // individual vector index operations (may exceed entities when chunking).
    let entities_total = items.len();
    let mut entities_current: usize = 0;

    // Report initial progress so the status bar shows total count
    if let Some(ref tx) = progress_tx {
        let _ = tx.send(EmbeddingProgress::new(
            EmbeddingPhase::Content,
            0,
            entities_total,
            0,
            total_missing,
        ));
    }

    let mut recovered = 0;
    let now = Utc::now();

    // Generate embeddings for content items (and their chunks)
    for (item_id, content_type, content) in &items {
        let timestamp = now;

        // Skip if content is empty or too short for meaningful embedding.
        // The SQL query already filters by MIN_EMBED_CONTENT_LEN, but this
        // check is a defense-in-depth in case the query changes.
        if content.trim().is_empty() || content.len() < MIN_EMBED_CONTENT_LEN {
            log::debug!(
                "Skipping item {}: content too short for embedding ({} bytes)",
                item_id,
                content.len()
            );
            processed += 1;
            entities_current += 1;
            if let Some(ref tx) = progress_tx {
                let _ = tx.send(EmbeddingProgress::new(
                    EmbeddingPhase::Content,
                    entities_current,
                    entities_total,
                    processed,
                    total_missing,
                ));
            }
            continue;
        }

        // Check if item already has chunks (long content that was partially processed)
        // If so, skip item embedding - chunks are handled separately
        if db.content_item_has_chunks(*item_id).unwrap_or(false) {
            processed += 1;
            entities_current += 1;
            if let Some(ref tx) = progress_tx {
                let _ = tx.send(EmbeddingProgress::new(
                    EmbeddingPhase::Content,
                    entities_current,
                    entities_total,
                    processed,
                    total_missing,
                ));
            }
            continue;
        }

        // Get item's conversation_id and project_id for embedding metadata
        let (conv_id, proj_id) = match db.get_content_item_by_id(*item_id) {
            Ok(Some(item)) => (item.conversation_id, item.project_id),
            _ => (None, None),
        };

        // Check if content needs chunking using dynamic threshold
        if content.len() > max_chars {
            // Long content - create chunks and embed each chunk with fallback.
            // Each chunk is a separate unit of work, so we expand the total.
            let chunks_list = chunk_text_with_config(content, &ChunkConfig::from(&chunk_config));
            let num_chunks = chunks_list.len();
            let extra_work = num_chunks.saturating_sub(1); // item already counted as 1
            if extra_work > 0 {
                total_missing += extra_work;
            }

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
                        if quiet {
                            log::warn!("Failed to insert chunk {}: {}", chunk.index, e);
                        } else {
                            eprintln!("Warning: Failed to insert chunk {}: {}", chunk.index, e);
                        }
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
                        if quiet {
                            log::warn!("Failed to recover embedding for chunk {}: {}", chunk_id, e);
                        } else {
                            eprintln!(
                                "Warning: Failed to recover embedding for chunk {}: {}",
                                chunk_id, e
                            );
                        }
                    }
                }
                processed += 1;
                if let Some(ref tx) = progress_tx {
                    let _ = tx.send(EmbeddingProgress::new(
                        EmbeddingPhase::Content,
                        entities_current,
                        entities_total,
                        processed,
                        total_missing,
                    ));
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
                        // Item was chunked due to fallback — each extra chunk is a unit of work
                        recovered += result.chunks_created;
                        let extra_work = result.chunks_created.saturating_sub(1);
                        if extra_work > 0 {
                            total_missing += extra_work;
                        }
                    } else {
                        recovered += 1;
                    }
                }
                Err(e) => {
                    if quiet {
                        log::warn!("Failed to recover embedding for item {}: {}", item_id, e);
                    } else {
                        eprintln!(
                            "Warning: Failed to recover embedding for item {}: {}",
                            item_id, e
                        );
                    }
                }
            }
            processed += 1;
            entities_current += 1;
            if let Some(ref tx) = progress_tx {
                let _ = tx.send(EmbeddingProgress::new(
                    EmbeddingPhase::Content,
                    entities_current,
                    entities_total,
                    processed,
                    total_missing,
                ));
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
                    if quiet {
                        log::warn!("Newly created chunk {} has no parent item", chunk_id);
                    } else {
                        eprintln!(
                            "Warning: Newly created chunk {} has no parent item",
                            chunk_id
                        );
                    }
                    processed += 1;
                    entities_current += 1;
                    if let Some(ref tx) = progress_tx {
                        let _ = tx.send(EmbeddingProgress::new(
                            EmbeddingPhase::Content,
                            entities_current,
                            entities_total,
                            processed,
                            total_missing,
                        ));
                    }
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
                    if quiet {
                        log::warn!("Failed to generate embedding for chunk {}: {}", chunk_id, e);
                    } else {
                        eprintln!(
                            "Warning: Failed to generate embedding for chunk {}: {}",
                            chunk_id, e
                        );
                    }
                }
            }
            processed += 1;
            entities_current += 1;
            if let Some(ref tx) = progress_tx {
                let _ = tx.send(EmbeddingProgress::new(
                    EmbeddingPhase::Content,
                    entities_current,
                    entities_total,
                    processed,
                    total_missing,
                ));
            }
        }
    }

    // Signal completion to the TUI status bar
    if let Some(ref tx) = progress_tx {
        let _ = tx.send(EmbeddingProgress::completed());
    }

    if recovered > 0 && !quiet {
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
