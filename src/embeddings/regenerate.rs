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
use crate::embeddings::EmbeddingClient;

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

    println!("Regenerating embeddings for {} items...", total);

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

        match embedding_client.embed(content).await {
            Ok(embedding) => {
                let timestamp = Utc::now();

                // Extract conversation_id from content_items based on content_type
                // For 'message' type, we need to get the conversation_id from the item
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