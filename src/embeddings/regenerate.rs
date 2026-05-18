//! Embedding regeneration for migrated content
//!
//! Regenerates embeddings for content_items and content_chunks after
//! schema migration from v6 to v7. This is necessary because the old
//! message_embeddings table cannot be safely migrated due to UNIQUE
//! constraint conflicts when multiple messages have the same content.
//!
//! Embeddings are derived data and can be regenerated from source content.
//!
//! # Output modes
//!
//! - `quiet = false` (terminal mode): prints progress and errors to stdout/stderr,
//!   shows an indicatif progress bar with ETA
//! - `quiet = true` (TUI mode): suppresses all direct terminal output, logs
//!   warnings via `log::warn!`, uses a hidden progress bar. The caller (TUI)
//!   shows status messages through the `ChatView` instead.

#![expect(clippy::print_stdout)] // Terminal-mode output (guarded by `quiet` flag)
#![expect(clippy::print_stderr)] // Terminal-mode output (guarded by `quiet` flag)
use chrono::Utc;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;

use crate::db::Database;
use crate::embeddings::{
    ChunkConfig, DynamicChunkConfig, EmbeddingClient, chunk_text_with_config,
    fallback::{
        EmbedContext, EmbedItemContext, embed_chunk_with_fallback, embed_item_with_fallback,
    },
};

/// Result of embedding regeneration
#[derive(Debug, Clone, Copy)]
pub struct RegenerationStats {
    /// Number of content items processed
    pub items_processed: usize,
    /// Number of content chunks processed
    pub chunks_processed: usize,
    /// Number of items that failed
    pub items_failed: usize,
    /// Number of chunks that failed
    pub chunks_failed: usize,
}

impl RegenerationStats {
    /// Total items and chunks processed
    pub fn total_processed(&self) -> usize {
        self.items_processed + self.chunks_processed
    }

    /// Total failures
    pub fn total_failed(&self) -> usize {
        self.items_failed + self.chunks_failed
    }

    /// Returns true if there were any errors
    pub fn has_errors(&self) -> bool {
        self.total_failed() > 0
    }
}

/// Regenerate all embeddings for content_items and content_chunks.
///
/// This function is called after schema migration v6→v7 to regenerate
/// embeddings that were lost during migration. Embeddings are derived
/// data and can be safely regenerated from source content.
///
/// Uses dynamic chunk sizing based on the embedding model's context length.
/// For models with smaller contexts (e.g., nomic-embed-text-v2-moe with 512 tokens),
/// long content is automatically chunked before embedding.
///
/// # Arguments
/// * `db` - Database connection
/// * `embedding_client` - Embedding client for generating embeddings
/// * `quiet` - When `true`, suppresses terminal output and progress bars (TUI mode).
///   Warnings are logged via `log::warn!` instead of `eprintln!`.
///   When `false`, shows a progress bar with ETA and prints status to stdout/stderr.
///
/// # Returns
/// Statistics about regeneration (items/chunks processed and failed)
///
/// Does not panic - returns stats with failures counted on error.
/// If Ollama is unreachable, logs a warning and returns partial stats
/// instead of crashing the application. Recovery will be attempted
/// on next startup.
pub async fn regenerate_all_embeddings(
    db: &Arc<Database>,
    embedding_client: &Arc<EmbeddingClient>,
    quiet: bool,
    progress_tx: Option<crate::chat::app::EmbeddingProgressTx>,
) -> RegenerationStats {
    // Note: V2 orphan chunks are cleaned by recover_missing_embeddings(), not here.
    // We don't clean ALL chunks because items with successful embeddings have has_embedding=1.

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

    // Get all items and chunks without embeddings
    let items = match db.get_content_items_for_reindex() {
        Ok(i) => i,
        Err(e) => {
            if quiet {
                log::warn!("Error checking for items without embeddings: {}", e);
            } else {
                eprintln!("Error checking for items without embeddings: {}", e);
            }
            return RegenerationStats {
                items_processed: 0,
                chunks_processed: 0,
                items_failed: 0,
                chunks_failed: 0,
            };
        }
    };

    let chunks = match db.get_content_chunks_for_reindex() {
        Ok(c) => c,
        Err(e) => {
            if quiet {
                log::warn!("Error checking for chunks without embeddings: {}", e);
            } else {
                eprintln!("Error checking for chunks without embeddings: {}", e);
            }
            return RegenerationStats {
                items_processed: 0,
                chunks_processed: 0,
                items_failed: 0,
                chunks_failed: 0,
            };
        }
    };

    let total = items.len() + chunks.len();

    if total == 0 {
        // No embeddings to regenerate - this is normal for new installations
        return RegenerationStats {
            items_processed: 0,
            chunks_processed: 0,
            items_failed: 0,
            chunks_failed: 0,
        };
    }

    if !quiet {
        println!(
            "Regenerating embeddings for {} items (context: {} tokens)...",
            items.len(),
            context_length
        );
    }

    // Report initial progress (0 of total) so the status bar shows total count
    if let Some(ref tx) = progress_tx {
        let _ = tx.send((0, total));
    }

    // Setup progress bar (hidden in quiet mode to avoid corrupting TUI alternate screen)
    let progress = if quiet {
        ProgressBar::hidden()
    } else {
        let pb = ProgressBar::new(total as u64);
        #[expect(clippy::expect_used)] // hardcoded template string is always valid
        let style =
            ProgressStyle::with_template("  {bar:20} {pos}/{len} ({percent}%) ETA: {eta_precise}")
                .expect("Invalid progress template")
                .progress_chars("█▓░");
        pb.set_style(style);
        pb
    };

    let mut stats = RegenerationStats {
        items_processed: 0,
        chunks_processed: 0,
        items_failed: 0,
        chunks_failed: 0,
    };

    // Report embedding progress to the TUI status bar.
    // Sends (current_processed, total) where total is the combined count
    // of items and chunks to process.
    let report_progress = |processed: usize, total: usize| {
        if let Some(ref tx) = progress_tx {
            let _ = tx.send((processed, total));
        }
    };

    // Process content items
    for (item_id, content_type, content) in &items {
        // Skip if content is empty or too short
        if content.trim().is_empty() || content.len() < 10 {
            stats.items_failed += 1;
            progress.inc(1);
            report_progress(progress.position() as usize, total);
            continue;
        }

        // Get item metadata for embedding
        let (conv_id, proj_id) = match db.get_content_item_by_id(*item_id) {
            Ok(Some(item)) => (item.conversation_id, item.project_id),
            _ => (None, None),
        };
        let timestamp = Utc::now();

        // Check if content needs chunking based on dynamic config
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
                        if quiet {
                            log::warn!("Failed to insert chunk {}: {}", chunk.index, e);
                        } else {
                            eprintln!("Warning: Failed to insert chunk {}: {}", chunk.index, e);
                        }
                        stats.chunks_failed += 1;
                        continue;
                    }
                };

                // Embed chunk with fallback (handles oversized content)
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
                        stats.chunks_processed += result.chunks_created;
                    }
                    Err(e) => {
                        if quiet {
                            log::warn!("Failed to embed chunk {}: {}", chunk_id, e);
                        } else {
                            eprintln!("Warning: Failed to embed chunk {}: {}", chunk_id, e);
                        }
                        stats.chunks_failed += 1;
                    }
                }
            }

            // Mark item as having embeddings ONLY if all chunks are complete
            // This prevents re-processing items with incomplete chunks on next startup
            let _ = db.mark_item_embedding_if_complete(*item_id);
            stats.items_processed += 1;
        } else {
            // Short content - embed directly (with fallback for oversized content)
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
                        stats.chunks_processed += result.chunks_created;
                    }
                    stats.items_processed += 1;
                }
                Err(e) => {
                    if quiet {
                        log::warn!("Failed to generate embedding for item {}: {}", item_id, e);
                    } else {
                        eprintln!("Failed to generate embedding for item {}: {}", item_id, e);
                    }
                    stats.items_failed += 1;

                    // If Ollama is down or unreachable, stop processing gracefully
                    // instead of panicking. Recovery will be attempted on next startup.
                    let err_str = e.to_string();
                    if err_str.contains("connection refused")
                        || err_str.contains("network")
                        || err_str.contains("timeout")
                    {
                        progress.finish_and_clear();
                        if quiet {
                            log::warn!("Cannot connect to Ollama for embedding generation.");
                        } else {
                            eprintln!(
                                "\nError: Cannot connect to Ollama for embedding generation."
                            );
                            eprintln!("Please ensure Ollama is running and try again.");
                            eprintln!(
                                "Progress saved: {}/{} items processed.",
                                stats.items_processed,
                                items.len()
                            );
                        }
                        // Count remaining items as failed
                        let remaining = items.len() - stats.items_processed - stats.items_failed;
                        stats.items_failed += remaining;
                        break;
                    }
                }
            }
        }

        progress.inc(1);
        report_progress(progress.position() as usize, total);
    }

    // Process content chunks
    for (chunk_id, content) in &chunks {
        // Skip if content is empty
        if content.trim().is_empty() {
            stats.chunks_failed += 1;
            progress.inc(1);
            report_progress(progress.position() as usize, total);
            continue;
        }

        // Get conversation_id and project_id from the parent item
        let result: Option<(String, Option<String>, Option<String>)> = db
            .with_connection(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT ci.content_type, ci.conversation_id, ci.project_id
                     FROM content_items ci
                     JOIN content_chunks cc ON cc.item_id = ci.id
                     WHERE cc.id = ?1",
                )?;
                let mut rows = stmt.query_map(rusqlite::params![chunk_id], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?;
                rows.next().transpose()
            })
            .ok()
            .flatten();

        let (content_type, conv_id, proj_id) = match result {
            Some((ct, c, p)) => (ct, c, p),
            None => {
                // Chunk has no parent item - shouldn't happen
                stats.chunks_failed += 1;
                progress.inc(1);
                report_progress(progress.position() as usize, total);
                continue;
            }
        };

        // Get parent item_id for the chunk
        let parent_item_id: Option<i64> = db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT item_id FROM content_chunks WHERE id = ?1",
                    rusqlite::params![chunk_id],
                    |row| row.get(0),
                )
            })
            .ok();

        let parent_item_id = match parent_item_id {
            Some(id) => id,
            None => {
                stats.chunks_failed += 1;
                progress.inc(1);
                continue;
            }
        };

        let timestamp = Utc::now();
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
                stats.chunks_processed += result.chunks_created;
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
                stats.chunks_failed += 1;

                // If Ollama is down or unreachable, stop processing gracefully
                // instead of panicking. Recovery will be attempted on next startup.
                let err_str = e.to_string();
                if err_str.contains("connection refused")
                    || err_str.contains("network")
                    || err_str.contains("timeout")
                {
                    progress.finish_and_clear();
                    if quiet {
                        log::warn!("Cannot connect to Ollama for embedding generation.");
                    } else {
                        eprintln!("\nError: Cannot connect to Ollama for embedding generation.");
                        eprintln!("Please ensure Ollama is running and try again.");
                        eprintln!(
                            "Progress saved: {}/{} chunks processed.",
                            stats.chunks_processed,
                            chunks.len()
                        );
                    }
                    // Count remaining chunks as failed
                    let remaining = chunks.len() - stats.chunks_processed - stats.chunks_failed;
                    stats.chunks_failed += remaining;
                    break;
                }
            }
        }

        progress.inc(1);
        report_progress(progress.position() as usize, total);
    }

    progress.finish_and_clear();

    // Signal completion to the TUI status bar (progress = total means done)
    // poll_embedding_progress() clears the indicator when current >= total.
    if let Some(ref tx) = progress_tx {
        let _ = tx.send((total, total));
    }

    // Report any failures
    if stats.total_failed() > 0 {
        if quiet {
            log::warn!(
                "{} item(s) and {} chunk(s) failed to generate embeddings.",
                stats.items_failed,
                stats.chunks_failed
            );
        } else {
            println!(
                "Warning: {} item(s) and {} chunk(s) failed to generate embeddings.",
                stats.items_failed, stats.chunks_failed
            );
            println!("These will be regenerated on next startup via recovery.");
        }
    }

    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regeneration_stats() {
        let stats = RegenerationStats {
            items_processed: 100,
            chunks_processed: 50,
            items_failed: 5,
            chunks_failed: 2,
        };

        assert_eq!(stats.total_processed(), 150);
        assert_eq!(stats.total_failed(), 7);
        assert!(stats.has_errors());

        let no_errors = RegenerationStats {
            items_processed: 10,
            chunks_processed: 5,
            items_failed: 0,
            chunks_failed: 0,
        };

        assert!(!no_errors.has_errors());
    }
}
