//! Embedding regeneration for migrated content
//!
//! Regenerates embeddings for content_items and content_chunks after
//! schema migration from v6 to v7. This is necessary because the old
//! message_embeddings table cannot be safely migrated due to UNIQUE
//! constraint conflicts when multiple messages have the same content.
//!
//! Embeddings are derived data and can be regenerated from source content.

use chrono::Utc;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;

use crate::db::Database;
use crate::embeddings::{chunk_text_with_config, ChunkConfig, DynamicChunkConfig, EmbeddingClient};

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
/// Shows a progress bar with ETA during regeneration.
///
/// # Arguments
/// * `db` - Database connection
/// * `embedding_client` - Embedding client for generating embeddings
///
/// # Returns
/// Statistics about regeneration (items/chunks processed and failed)
///
/// # Panics
/// Does not panic - returns stats with failures counted on error.
pub async fn regenerate_all_embeddings(
    db: &Arc<Database>,
    embedding_client: &Arc<EmbeddingClient>,
) -> RegenerationStats {
    // Note: V2 orphan chunks are cleaned by recover_missing_embeddings(), not here.
    // We don't clean ALL chunks because items with successful embeddings have has_embedding=1.

    // Get dynamic context length from embedding model
    let context_length = match embedding_client.get_context_length().await {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Warning: Could not get embedding model context length: {}", e);
            eprintln!("Using conservative default of 512 tokens.");
            512
        }
    };

    let chunk_config = DynamicChunkConfig::new(context_length);
    let max_chars = chunk_config.max_chars();

    // Get all items and chunks without embeddings
    let items = match db.get_content_items_for_reindex() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Error checking for items without embeddings: {}", e);
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
            eprintln!("Error checking for chunks without embeddings: {}", e);
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

    println!(
        "Regenerating embeddings for {} items (context: {} tokens)...",
        items.len(), context_length
    );

    // Setup progress bar with ETA
    let progress = ProgressBar::new(total as u64);
    progress.set_style(
        ProgressStyle::with_template(
            "  {bar:20} {pos}/{len} ({percent}%) ETA: {eta_precise}",
        )
        .expect("Invalid progress template")
        .progress_chars("█▓░"),
    );

    let mut stats = RegenerationStats {
        items_processed: 0,
        chunks_processed: 0,
        items_failed: 0,
        chunks_failed: 0,
    };

    // Process content items
    for (item_id, content_type, content) in &items {
        // Skip if content is empty or too short
        if content.trim().is_empty() || content.len() < 10 {
            stats.items_failed += 1;
            progress.inc(1);
            continue;
        }

        // Check if content needs chunking based on dynamic config
        if content.len() > max_chars {
            // Long content - create chunks and embed each chunk
            let chunks_list = chunk_text_with_config(content, &ChunkConfig::from(&chunk_config));
            let chunks_before = stats.chunks_processed;

            for chunk in &chunks_list {
                // Insert chunk into database
                let timestamp = Utc::now();
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
                        stats.chunks_failed += 1;
                        continue;
                    }
                };

                // Get item metadata for embedding
                let (conv_id, proj_id) = match db.get_content_item_by_id(*item_id) {
                    Ok(Some(item)) => (item.conversation_id, item.project_id),
                    _ => (None, None),
                };

                // Generate embedding for chunk
                match embedding_client.embed(&chunk.content).await {
                    Ok(embedding) => {
                        match db
                            .update_content_chunk_embedding(
                                chunk_id,
                                &embedding,
                                content_type,
                                conv_id.as_deref(),
                                proj_id.as_deref(),
                                timestamp,
                            )
                        {
                            Ok(_) => {
                                stats.chunks_processed += 1;
                            }
                            Err(e) => {
                                stats.chunks_failed += 1;
                                eprintln!("Warning: Failed to update chunk {} embedding in DB: {}", chunk_id, e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to generate embedding for chunk {}: {}",
                            chunk_id, e
                        );
                        stats.chunks_failed += 1;
                    }
                }
            }

            // Mark item as having embeddings if at least one chunk succeeded
            // This prevents re-processing on next startup
            let chunks_succeeded = stats.chunks_processed - chunks_before;
            if chunks_succeeded > 0 {
                let _ = db.with_connection(|conn| {
                    conn.execute(
                        "UPDATE content_items SET has_embedding = 1 WHERE id = ?1",
                        rusqlite::params![item_id],
                    )
                });
            }
            stats.items_processed += 1;
        } else {
            // Short content - embed directly
            match embedding_client.embed(content).await {
                Ok(embedding) => {
                    let timestamp = Utc::now();

                    // Extract conversation_id from content_items based on content_type
                    let conversation_id: Option<String> = db
                        .with_connection(|conn| {
                            conn.query_row(
                                "SELECT conversation_id FROM content_items WHERE id = ?1",
                                rusqlite::params![item_id],
                                |row| row.get(0),
                            )
                        })
                        .ok()
                        .flatten();

                    let project_id: Option<String> = db
                        .with_connection(|conn| {
                            conn.query_row(
                                "SELECT project_id FROM content_items WHERE id = ?1",
                                rusqlite::params![item_id],
                                |row| row.get(0),
                            )
                        })
                        .ok()
                        .flatten();

                    if db
                        .update_content_item_embedding(
                            *item_id,
                            &embedding,
                            content_type,
                            conversation_id.as_deref(),
                            project_id.as_deref(),
                            timestamp,
                        )
                        .is_ok()
                    {
                        stats.items_processed += 1;
                    } else {
                        stats.items_failed += 1;
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Failed to generate embedding for item {}: {}",
                        item_id, e
                    );
                    stats.items_failed += 1;

                    // If Ollama is down or unreachable, abort completely
                    if e.to_string().contains("connection refused")
                        || e.to_string().contains("network")
                        || e.to_string().contains("timeout")
                    {
                        progress.finish_and_clear();
                        println!("\nError: Cannot connect to Ollama for embedding generation.");
                        println!("Please ensure Ollama is running and try again.");
                        println!("Progress saved: {}/{} items processed.", stats.items_processed, items.len());
                        panic!("Embedding generation failed - Ollama unreachable");
                    }
                }
            }
        }

        progress.inc(1);
    }

    // Process content chunks
    for (chunk_id, content) in &chunks {
        // Skip if content is empty
        if content.trim().is_empty() {
            stats.chunks_failed += 1;
            progress.inc(1);
            continue;
        }

        match embedding_client.embed(content).await {
            Ok(embedding) => {
                let timestamp = Utc::now();

                // Get conversation_id and project_id from the parent item
                let result: Option<(Option<String>, Option<String>)> = db
                    .with_connection(|conn| {
                        conn.query_row(
                            "SELECT conversation_id, project_id FROM content_items ci \
                             JOIN content_chunks cc ON cc.item_id = ci.id \
                             WHERE cc.id = ?1",
                            rusqlite::params![chunk_id],
                            |row| Ok((row.get(0)?, row.get(1)?)),
                        )
                    })
                    .ok();

                let (conversation_id, project_id) = result.unwrap_or((None, None));

                if db
                    .update_content_chunk_embedding(
                        *chunk_id,
                        &embedding,
                        "message", // All existing chunks are messages
                        conversation_id.as_deref(),
                        project_id.as_deref(),
                        timestamp,
                    )
                    .is_ok()
                {
                    stats.chunks_processed += 1;
                } else {
                    stats.chunks_failed += 1;
                }
            }
            Err(e) => {
                eprintln!(
                    "Failed to generate embedding for chunk {}: {}",
                    chunk_id, e
                );
                stats.chunks_failed += 1;

                // If Ollama is down or unreachable, abort completely
                if e.to_string().contains("connection refused")
                    || e.to_string().contains("network")
                    || e.to_string().contains("timeout")
                {
                    progress.finish_and_clear();
                    println!("\nError: Cannot connect to Ollama for embedding generation.");
                    println!("Please ensure Ollama is running and try again.");
                    println!(
                        "Progress saved: {}/{} chunks processed.",
                        stats.chunks_processed,
                        chunks.len()
                    );
                    panic!("Embedding generation failed - Ollama unreachable");
                }
            }
        }

        progress.inc(1);
    }

    progress.finish_and_clear();

    // Report any failures
    if stats.total_failed() > 0 {
        println!(
            "Warning: {} item(s) and {} chunk(s) failed to generate embeddings.",
            stats.items_failed, stats.chunks_failed
        );
        println!("These will be regenerated on next startup via recovery.");
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