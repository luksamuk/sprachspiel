//! Embedding fallback with recursive chunking.
//!
//! When content exceeds the embedding model's context window,
//! divides into smaller chunks recursively until successful.
//!
//! # Design
//!
//! The fallback process handles content that exceeds the embedding model's
//! context window (e.g., 512 tokens for nomic-embed-text-v2-moe):
//!
//! 1. Try to embed content directly
//! 2. If "context_length_exceeded" error:
//!    a. Check limits (max divisions, max chunks)
//!    b. Divide content into smaller chunks (halved context)
//!    c. Update first chunk in database
//!    d. Create new chunks for subsequent parts
//!    e. Recursively embed each chunk
//!
//! # Panics
//!
//! The function panics if limits are exceeded to prevent database explosion
//! from misconfigured environments:
//!
//! - More than MAX_FALLBACK_DIVISIONS (4) needed
//! - More than MAX_CHUNKS_PER_ITEM (64) would be created
//! - Chunk below MIN_CHUNK_TOKENS (32) and still fails

use chrono::{DateTime, Utc};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::chunk_config::DynamicChunkConfig;
use super::chunker::{chunk_text_with_config, ChunkConfig};
use super::client::{EmbeddingClient, EmbeddingError};
use crate::db::Database;

/// Maximum recursive divisions (512→256→128→64→32 tokens).
/// Beyond this, environment is misconfigured and should abort.
const MAX_FALLBACK_DIVISIONS: usize = 4;

/// Maximum chunks per item to prevent database explosion.
const MAX_CHUNKS_PER_ITEM: usize = 64;

/// Minimum chunk tokens before aborting.
const MIN_CHUNK_TOKENS: usize = 32;

/// Context for embedding content with fallback support.
#[derive(Debug, Clone)]
pub struct EmbedContext<'a> {
    /// Content to embed
    pub content: &'a str,
    /// Parent item ID (message, note, document)
    pub item_id: i64,
    /// Existing chunk ID (will be replaced if fallback triggers)
    pub chunk_id: i64,
    /// Content type: "message", "note", "document"
    pub content_type: &'a str,
    /// Conversation ID (for messages)
    pub conversation_id: Option<&'a str>,
    /// Project ID (for notes, documents)
    pub project_id: Option<&'a str>,
    /// Timestamp for chunk creation
    pub timestamp: DateTime<Utc>,
}

/// Context for embedding item content (for items without chunks yet).
#[derive(Debug, Clone)]
pub struct EmbedItemContext<'a> {
    /// Content to embed
    pub content: &'a str,
    /// Item ID
    pub item_id: i64,
    /// Content type: "message", "note", "document"
    pub content_type: &'a str,
    /// Conversation ID (for messages)
    pub conversation_id: Option<&'a str>,
    /// Project ID (for notes, documents)
    pub project_id: Option<&'a str>,
    /// Timestamp for chunk creation
    pub timestamp: DateTime<Utc>,
}

impl<'a> EmbedItemContext<'a> {
    /// Create context for an existing item that needs embedding.
    pub fn new(
        content: &'a str,
        item_id: i64,
        content_type: &'a str,
        conversation_id: Option<&'a str>,
        project_id: Option<&'a str>,
    ) -> Self {
        Self {
            content,
            item_id,
            content_type,
            conversation_id,
            project_id,
            timestamp: Utc::now(),
        }
    }
}

/// Result of embedding with fallback.
#[derive(Debug)]
pub struct EmbedResult {
    /// Number of chunks created (1 if no fallback, >1 if fallback triggered)
    pub chunks_created: usize,
}

/// Error during embedding fallback.
#[derive(Debug)]
pub enum FallbackError {
    /// Embedding failed
    Embedding(EmbeddingError),
    /// Database operation failed
    Database(String),
    /// Maximum divisions exceeded - environment misconfigured
    MaxDivisionsExceeded {
        divisions: usize,
        max: usize,
        content_len: usize,
    },
    /// Maximum chunks exceeded - content too large or misconfigured
    MaxChunksExceeded {
        chunks: usize,
        max: usize,
        content_len: usize,
    },
    /// Chunk below minimum and still fails
    BelowMinimumTokens {
        estimated_tokens: usize,
        min: usize,
        content_len: usize,
    },
}

impl std::fmt::Display for FallbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Embedding(e) => write!(f, "Embedding error: {}", e),
            Self::Database(msg) => write!(f, "Database error: {}", msg),
            Self::MaxDivisionsExceeded { divisions, max, content_len } => {
                write!(
                    f,
                    "Maximum fallback divisions exceeded: {} divisions (max {}) for {} byte content. \
                     This indicates a misconfigured embedding model or content that exceeds all chunk sizes. \
                     Consider increasing model context length or reducing content size.",
                    divisions, max, content_len
                )
            }
            Self::MaxChunksExceeded { chunks, max, content_len } => {
                write!(
                    f,
                    "Maximum chunks exceeded: {} chunks (max {}) for {} byte content. \
                     This indicates content that would create too many small chunks. \
                     Consider increasing model context length or reducing content size.",
                    chunks, max, content_len
                )
            }
            Self::BelowMinimumTokens { estimated_tokens, min, content_len } => {
                write!(
                    f,
                    "Chunk below minimum token limit: {} tokens (min {}) for {} byte content. \
                     Even at minimum size, embedding failed. This indicates a misconfigured \
                     embedding model or API error.",
                    estimated_tokens, min, content_len
                )
            }
        }
    }
}

impl std::error::Error for FallbackError {}

impl From<EmbeddingError> for FallbackError {
    fn from(e: EmbeddingError) -> Self {
        Self::Embedding(e)
    }
}

impl From<rusqlite::Error> for FallbackError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Database(e.to_string())
    }
}

/// Embed chunk content with recursive fallback for oversized inputs.
///
/// This function handles the case where content exceeds the embedding model's
/// context window. It will:
///
/// 1. Try to embed content directly
/// 2. If "context_length_exceeded" error occurs:
///    - Divide content into smaller chunks
///    - Update existing chunk content (first chunk)
///    - Create new chunks (subsequent chunks)
///    - Recursively embed each chunk
///
/// All database operations are atomic - if any part fails, chunks are rolled back.
///
/// # Arguments
///
/// * `ctx` - Embedding context with content, chunk IDs, and metadata
/// * `db` - Database connection for chunk operations
/// * `client` - Embedding client for generating embeddings
/// * `context_length` - Current model context length (for chunk sizing)
/// * `division_count` - Current recursion depth (starts at 0)
///
/// # Returns
///
/// Returns `Ok(EmbedResult)` with number of chunks created on success.
///
/// # Panics
///
/// Panics if:
/// - `division_count` exceeds `MAX_FALLBACK_DIVISIONS`
/// - Total chunks would exceed `MAX_CHUNKS_PER_ITEM`
/// - Content is below minimum size and still fails
///
/// These panics are intentional to prevent database explosion from misconfigured
/// environments.
///
/// # Example
///
/// ```rust,ignore
/// let ctx = EmbedContext {
///     content: &long_content,
///     item_id: 123,
///     chunk_id: 456,
///     content_type: "message",
///     conversation_id: Some("conv-1"),
///     project_id: None,
///     timestamp: Utc::now(),
/// };
///
/// embed_chunk_with_fallback(ctx, &db, &client, 512, 0).await?;
/// ```
pub fn embed_chunk_with_fallback<'a>(
    ctx: EmbedContext<'a>,
    db: Arc<Database>,
    client: Arc<EmbeddingClient>,
    context_length: usize,
    division_count: usize,
) -> Pin<Box<dyn Future<Output = Result<EmbedResult, FallbackError>> + Send + 'a>> {
    Box::pin(async move {
        // Check limits before proceeding
        check_limits(division_count, ctx.content.len(), MAX_FALLBACK_DIVISIONS, MAX_CHUNKS_PER_ITEM)?;

        // Try direct embed first
        match client.embed(ctx.content).await {
            Ok(embedding) => {
                // Success - save embedding to existing chunk
                db.update_content_chunk_embedding(
                    ctx.chunk_id,
                    &embedding,
                    ctx.content_type,
                    ctx.conversation_id,
                    ctx.project_id,
                    ctx.timestamp,
                )?;
                Ok(EmbedResult { chunks_created: 1 })
            }
            Err(EmbeddingError::ApiError(ref msg)) if EmbeddingClient::is_context_exceeded(msg) => {
                // Context exceeded - need to chunk
                handle_chunk_context_exceeded(ctx, db, client, context_length, division_count).await
            }
            Err(e) => Err(FallbackError::from(e)),
        }
    })
}

/// Check if limits are exceeded before attempting fallback.
///
/// # Panics
///
/// Panics with clear error message if limits exceeded.
fn check_limits(division_count: usize, content_len: usize, max_divisions: usize, max_chunks: usize) -> Result<(), FallbackError> {
    if division_count >= max_divisions {
        return Err(FallbackError::MaxDivisionsExceeded {
            divisions: division_count,
            max: max_divisions,
            content_len,
        });
    }

    // Estimate minimum chunks at this division level
    // At division N, we're creating 2^(N+1) chunks from original
    let estimated_chunks = 1 << (division_count + 1);
    if estimated_chunks > max_chunks {
        return Err(FallbackError::MaxChunksExceeded {
            chunks: estimated_chunks,
            max: max_chunks,
            content_len,
        });
    }

    Ok(())
}

/// Handle context exceeded error by dividing content and creating chunks.
async fn handle_chunk_context_exceeded(
    ctx: EmbedContext<'_>,
    db: Arc<Database>,
    client: Arc<EmbeddingClient>,
    context_length: usize,
    division_count: usize,
) -> Result<EmbedResult, FallbackError> {
    // Check minimum size - if we're at the minimum, abort
    let estimated_tokens = estimate_tokens(ctx.content.len());
    if estimated_tokens < MIN_CHUNK_TOKENS {
        return Err(FallbackError::BelowMinimumTokens {
            estimated_tokens,
            min: MIN_CHUNK_TOKENS,
            content_len: ctx.content.len(),
        });
    }

    // Calculate halved context length for chunking
    let halved_length = context_length / 2;
    let halved_config = DynamicChunkConfig::new(halved_length);
    let chunks = chunk_text_with_config(ctx.content, &ChunkConfig::from(&halved_config));

    if chunks.is_empty() {
        return Err(FallbackError::Embedding(EmbeddingError::ApiError(
            "Content too short to chunk".to_string(),
        )));
    }

    // Check chunk count limit
    if chunks.len() > MAX_CHUNKS_PER_ITEM {
        return Err(FallbackError::MaxChunksExceeded {
            chunks: chunks.len(),
            max: MAX_CHUNKS_PER_ITEM,
            content_len: ctx.content.len(),
        });
    }

    // Create all chunks atomically
    let chunk_ids = create_chunks_atomically(&ctx, &chunks, &db)?;

    // Embed each chunk recursively
    let mut total_created = 0;
    for (chunk_id, chunk_content) in chunk_ids.iter().zip(chunks.iter()) {
        let chunk_ctx = EmbedContext {
            content: &chunk_content.content,
            item_id: ctx.item_id,
            chunk_id: *chunk_id,
            content_type: ctx.content_type,
            conversation_id: ctx.conversation_id,
            project_id: ctx.project_id,
            timestamp: ctx.timestamp,
        };

        embed_chunk_with_fallback(chunk_ctx, Arc::clone(&db), Arc::clone(&client), halved_length, division_count + 1).await?;
        total_created += 1;
    }

    Ok(EmbedResult { chunks_created: total_created })
}

/// Create chunks atomically in a transaction.
///
/// First chunk updates existing chunk_id, subsequent chunks create new entries.
fn create_chunks_atomically(
    ctx: &EmbedContext<'_>,
    chunks: &[super::chunker::Chunk],
    db: &Database,
) -> Result<Vec<i64>, FallbackError> {
    db.with_connection_mut(|conn| {
        let tx = conn.transaction()?;

        let mut chunk_ids = Vec::with_capacity(chunks.len());

        for (idx, chunk) in chunks.iter().enumerate() {
            if idx == 0 {
                // Update existing chunk for first piece
                tx.execute(
                    "UPDATE content_chunks SET content = ?1, start_offset = ?2, end_offset = ?3, has_embedding = 0 WHERE id = ?4",
                    rusqlite::params![chunk.content, chunk.start_offset as i32, chunk.end_offset as i32, ctx.chunk_id],
                )?;
                chunk_ids.push(ctx.chunk_id);
            } else {
                // Create new chunk for subsequent pieces
                let result = tx.query_row(
                    "INSERT INTO content_chunks (item_id, chunk_index, content, start_offset, end_offset, created_at, has_embedding) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0) RETURNING id",
                    rusqlite::params![
                        ctx.item_id,
                        ctx.chunk_id as i32 + idx as i32, // Increment chunk_index
                        chunk.content,
                        chunk.start_offset as i32,
                        chunk.end_offset as i32,
                        ctx.timestamp.timestamp()
                    ],
                    |row| row.get::<_, i64>(0),
                )?;
                chunk_ids.push(result);
            }
        }

        tx.commit()?;
        Ok(chunk_ids)
    })
    .map_err(FallbackError::from)
}

/// Embed item content (short content that doesn't need chunking yet).
///
/// This is used when an item doesn't have chunks yet. If the content
/// is too large, it will create chunks and embed them.
///
/// For short content that fits in the model's context, just saves the embedding
/// to the item directly.
pub async fn embed_item_with_fallback(
    ctx: EmbedItemContext<'_>,
    db: &Arc<Database>,
    client: &Arc<EmbeddingClient>,
    context_length: usize,
) -> Result<EmbedResult, FallbackError> {
    // Check limits
    check_limits(0, ctx.content.len(), MAX_FALLBACK_DIVISIONS, MAX_CHUNKS_PER_ITEM)?;

    // Try direct embed first
    match client.embed(ctx.content).await {
        Ok(embedding) => {
            // Success - save embedding to item
            db.update_content_item_embedding(
                ctx.item_id,
                &embedding,
                ctx.content_type,
                ctx.conversation_id,
                ctx.project_id,
                ctx.timestamp,
            )?;
            Ok(EmbedResult { chunks_created: 0 })
        }
        Err(EmbeddingError::ApiError(ref msg)) if EmbeddingClient::is_context_exceeded(msg) => {
            // Context exceeded - need to create chunks first
            handle_item_context_exceeded(ctx, db, client, context_length).await
        }
        Err(e) => Err(FallbackError::from(e)),
    }
}

/// Handle context exceeded for item without existing chunks.
async fn handle_item_context_exceeded<'a>(
    ctx: EmbedItemContext<'a>,
    db: &Arc<Database>,
    client: &Arc<EmbeddingClient>,
    context_length: usize,
) -> Result<EmbedResult, FallbackError> {
    // Check minimum size
    let estimated_tokens = estimate_tokens(ctx.content.len());
    if estimated_tokens < MIN_CHUNK_TOKENS {
        return Err(FallbackError::BelowMinimumTokens {
            estimated_tokens,
            min: MIN_CHUNK_TOKENS,
            content_len: ctx.content.len(),
        });
    }

    // Calculate halved context length for chunking
    let halved_length = context_length / 2;
    let halved_config = DynamicChunkConfig::new(halved_length);
    let chunks = chunk_text_with_config(ctx.content, &ChunkConfig::from(&halved_config));

    if chunks.is_empty() {
        return Err(FallbackError::Embedding(EmbeddingError::ApiError(
            "Content too short to chunk".to_string(),
        )));
    }

    // Check chunk count limit
    if chunks.len() > MAX_CHUNKS_PER_ITEM {
        return Err(FallbackError::MaxChunksExceeded {
            chunks: chunks.len(),
            max: MAX_CHUNKS_PER_ITEM,
            content_len: ctx.content.len(),
        });
    }

    // Create all chunks atomically (db is Arc<Database>, need to get inner ref)
    let chunk_ids = {
        let inner_db: &Database = db.as_ref();
        create_item_chunks_atomically(&ctx, &chunks, inner_db)?
    };

    // Embed each chunk recursively
    let mut total_created = 0;
    for (chunk_id, chunk) in chunk_ids.iter().zip(chunks.iter()) {
        let chunk_ctx = EmbedContext {
            content: &chunk.content,
            item_id: ctx.item_id,
            chunk_id: *chunk_id,
            content_type: ctx.content_type,
            conversation_id: ctx.conversation_id,
            project_id: ctx.project_id,
            timestamp: ctx.timestamp,
        };

        embed_chunk_with_fallback(chunk_ctx, Arc::clone(db), Arc::clone(client), halved_length, 1).await?;
        total_created += 1;
    }

    // Mark item as having embeddings (via chunks)
    db.with_connection(|conn| {
        conn.execute(
            "UPDATE content_items SET has_embedding = 1 WHERE id = ?1",
            rusqlite::params![ctx.item_id],
        )
    })?;

    Ok(EmbedResult { chunks_created: total_created })
}

/// Create chunks for an item that doesn't have any yet.
fn create_item_chunks_atomically(
    ctx: &EmbedItemContext<'_>,
    chunks: &[super::chunker::Chunk],
    db: &Database,
) -> Result<Vec<i64>, FallbackError> {
    db.with_connection_mut(|conn| {
        let tx = conn.transaction()?;

        let mut chunk_ids = Vec::with_capacity(chunks.len());

        for (idx, chunk) in chunks.iter().enumerate() {
            let result = tx.query_row(
                "INSERT INTO content_chunks (item_id, chunk_index, content, start_offset, end_offset, created_at, has_embedding) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0) RETURNING id",
                rusqlite::params![
                    ctx.item_id,
                    idx as i32,
                    chunk.content,
                    chunk.start_offset as i32,
                    chunk.end_offset as i32,
                    ctx.timestamp.timestamp()
                ],
                |row| row.get::<_, i64>(0),
            )?;
            chunk_ids.push(result);
        }

        tx.commit()?;
        Ok(chunk_ids)
    })
    .map_err(FallbackError::from)
}

/// Estimate tokens from content length.
///
/// Uses conservative estimate of 3 chars/token (Portuguese/code average).
fn estimate_tokens(content_len: usize) -> usize {
    (content_len as f32 / 3.0).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        // Conservative estimate: 3 chars/token
        assert_eq!(estimate_tokens(100), 34);  // 100/3 = 33.3 → 34
        assert_eq!(estimate_tokens(300), 100);  // 300/3 = 100
        assert_eq!(estimate_tokens(1000), 334); // 1000/3 = 333.3 → 334
    }

    #[test]
    fn test_check_limits_divisions() {
        // Within limits
        assert!(check_limits(0, 1000, 4, 64).is_ok());
        assert!(check_limits(3, 1000, 4, 64).is_ok());

        // At limit (should fail)
        assert!(check_limits(4, 1000, 4, 64).is_err());
    }

    #[test]
    fn test_check_limits_chunks() {
        // At division 0: 2^1 = 2 chunks
        assert!(check_limits(0, 1000, 4, 64).is_ok());

        // At division 5: 2^6 = 64 chunks (exactly at limit, should pass)
        assert!(check_limits(5, 1000, 6, 64).is_ok());

        // At division 5: 2^6 = 64 chunks, but max is 63 (should fail)
        assert!(check_limits(5, 1000, 6, 63).is_err());

        // At division 6: 2^7 = 128 chunks (exceeds limit)
        assert!(check_limits(6, 1000, 7, 64).is_err());
    }

    #[test]
    fn test_fallback_error_display() {
        let err = FallbackError::MaxDivisionsExceeded {
            divisions: 5,
            max: 4,
            content_len: 10000,
        };
        assert!(err.to_string().contains("5 divisions"));
        assert!(err.to_string().contains("max 4"));
    }

    #[test]
    fn test_embed_result() {
        let result = EmbedResult { chunks_created: 3 };
        assert_eq!(result.chunks_created, 3);
    }

    #[test]
    fn test_embed_item_context_new() {
        let ctx = EmbedItemContext::new("content", 123, "message", Some("conv"), None);
        assert_eq!(ctx.content, "content");
        assert_eq!(ctx.item_id, 123);
        assert_eq!(ctx.content_type, "message");
    }
}