//! Context builder for optimal LLM context composition
//!
//! Implements research-based context ordering to avoid "lost in the middle" issues:
//! - System prompt at the BEGINNING
//! - Retrieved messages AFTER system (before summary)
//! - Summary before recent messages
//! - Recent messages before current query
//! - Current query at the END

use ollama_rs::generation::chat::ChatMessage;
use std::sync::Arc;
use std::time::Instant;

use crate::chat::session::{ChatSession, MessageRole};
use crate::content::ContentSearchResult;
use crate::db::Database;
use crate::embeddings::EmbeddingClient;

/// Minimum messages before auto-retrieval activates
pub const MIN_MESSAGES_FOR_RETRIEVAL: usize = 5;

/// Number of semantically relevant messages to retrieve
pub const RELEVANT_MESSAGES_COUNT: usize = 5;

/// Number of recent messages to include in context
pub const RECENT_MESSAGES_COUNT: usize = 10;

/// Minimum interval between retrievals in seconds
pub const MIN_RETRIEVAL_INTERVAL_SECS: u64 = 5;

/// Keyword weight for RRF (BM25)
pub const KEYWORD_WEIGHT: f32 = 0.4;

/// Semantic weight for RRF (vector similarity)
pub const SEMANTIC_WEIGHT: f32 = 0.6;

/// Format timestamp for human-readable display
fn format_timestamp(timestamp: i64) -> String {
    use chrono::{Datelike, TimeZone, Utc};

    let dt = Utc
        .timestamp_opt(timestamp, 0)
        .single()
        .unwrap_or_else(Utc::now);
    let now = Utc::now();
    let diff = now.signed_duration_since(dt);

    if diff.num_hours() < 24 {
        // Today - show time only
        dt.format("%H:%M").to_string()
    } else if diff.num_days() < 7 {
        // This week - show day and time
        dt.format("%A %H:%M").to_string()
    } else if dt.year() == now.year() {
        // Same year - show month and day
        dt.format("%b %d %H:%M").to_string()
    } else {
        // Different year - show full date
        dt.format("%Y-%m-%d %H:%M").to_string()
    }
}

/// Format retrieved messages into context string
fn format_retrieved_context(results: &[ContentSearchResult]) -> String {
    use crate::db::SourceType;

    let mut text = String::from("<retrieved_context>\n");
    text.push_str("MESSAGES FROM YOUR PAST CONVERSATION with this user.\n\n");
    text.push_str(&format!(
        "Each message has an ID. Use remember(id=\"{}:N\") for full content or remember(query=\"topic\") to search.\n\n",
        SourceType::Conversation.prefix()
    ));
    text.push_str("CITATIONS: When referencing retrieved content, include the source ID after the statement.\n");
    text.push_str(&format!(
        "- Conversations: [{}:N]\n",
        SourceType::Conversation.prefix()
    ));
    text.push_str(&format!(
        "- Documents: [{}:N]\n",
        SourceType::Document.prefix()
    ));
    text.push_str(&format!("- Notes: [{}:N]\n\n", SourceType::Note.prefix()));
    text.push_str(&format!(
        "Example: \"As we discussed [{}:42], the project uses Rust.\"\n\n",
        SourceType::Conversation.prefix()
    ));

    for result in results {
        let item = &result.item;
        let timestamp = format_timestamp(item.created_at.timestamp());
        let prefix = SourceType::Conversation.prefix();
        text.push_str(&format!(
            "<message id=\"{}:{}\">\n<role>{}</role>\n<content>{}</content>\n<timestamp>{}</timestamp>\n</message>\n",
            prefix,
            item.id,
            item.role.as_deref().unwrap_or("unknown"),
            item.content,
            timestamp
        ));

        // If user message has assistant responses, include them
        for sub_item in &result.subsequent_items {
            let type_prefix = match sub_item.item.message_type.as_deref() {
                Some("pre_tool_content") => " [Intermediate]",
                _ => "",
            };
            let sub_timestamp = format_timestamp(sub_item.item.created_at.timestamp());
            let sub_prefix = sub_item.source_type.prefix();
            text.push_str(&format!(
                "<message id=\"{}:{}\"{}>\n<role>{}</role>\n<content>{}</content>\n<timestamp>{}</timestamp>\n</message>\n",
                sub_prefix,
                sub_item.item.id,
                type_prefix,
                sub_item.item.role.as_deref().unwrap_or("unknown"),
                sub_item.item.content,
                sub_timestamp
            ));
        }
    }

    text.push_str("</retrieved_context>");
    text
}

/// Configuration for context retrieval
#[derive(Debug, Clone)]
pub struct RetrievalConfig {
    /// Enable semantic retrieval
    pub enabled: bool,
    /// Minimum messages before activation
    pub min_messages: usize,
    /// Number of semantically relevant messages to retrieve
    pub relevant_count: usize,
    /// Number of recent messages to include
    pub recent_count: usize,
    /// Minimum interval between retrievals (seconds)
    pub min_query_interval_secs: u64,
    /// Keyword weight for RRF (0.0 - 1.0)
    pub keyword_weight: f32,
    /// Semantic weight for RRF (0.0 - 1.0)
    pub semantic_weight: f32,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_messages: MIN_MESSAGES_FOR_RETRIEVAL,
            relevant_count: RELEVANT_MESSAGES_COUNT,
            recent_count: RECENT_MESSAGES_COUNT,
            min_query_interval_secs: MIN_RETRIEVAL_INTERVAL_SECS,
            keyword_weight: KEYWORD_WEIGHT,
            semantic_weight: SEMANTIC_WEIGHT,
        }
    }
}

/// Result of building context
#[derive(Debug, Clone)]
pub struct ContextResult {
    /// Messages to send to LLM
    pub messages: Vec<ChatMessage>,
    /// Whether retrieval was performed
    pub retrieval_performed: bool,
    /// Number of retrieved messages
    pub retrieved_count: usize,
}

/// Result of a retrieval operation
#[derive(Debug, Clone)]
pub struct RetrievalResult {
    /// Chat message containing retrieved context
    pub message: ChatMessage,
    /// Number of items retrieved
    pub count: usize,
}

/// Perform semantic retrieval for context.
///
/// Searches for relevant messages using hybrid search (BM25 + vector similarity).
/// Returns None if retrieval fails or returns no results.
async fn perform_retrieval(
    db: &Arc<Database>,
    client: &Arc<EmbeddingClient>,
    query: &str,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
    config: &RetrievalConfig,
) -> Option<RetrievalResult> {
    log::debug!("Generating embedding for query...");

    let embedding = client.embed(query).await.ok()?;

    log::debug!(
        "Searching for relevant messages (conversation: {:?}, project: {:?})",
        conversation_id,
        project_id
    );

    let results = db
        .search_messages_hybrid(
            query,
            &embedding,
            conversation_id,
            project_id,
            config.relevant_count,
            config.keyword_weight,
            config.semantic_weight,
        )
        .ok()?;

    let enriched_results = match db.enrich_content_results_with_context(results) {
        Ok(r) => r,
        Err(e) => {
            log::debug!("Warning: Failed to enrich results: {}", e);
            return None;
        }
    };

    log::debug!("Search returned {} results", enriched_results.len());

    if enriched_results.is_empty() {
        return None;
    }

    let count = enriched_results.len();
    let retrieved_text = format_retrieved_context(&enriched_results);

    log::debug!(
        "Added {} retrieved messages to context ({} enriched with responses)",
        count,
        enriched_results
            .iter()
            .filter(|r| !r.subsequent_items.is_empty())
            .count()
    );

    Some(RetrievalResult {
        message: ChatMessage::system(retrieved_text),
        count,
    })
}

/// Push messages as ChatMessages, filtering out system messages.
///
/// System messages are handled separately in the context building flow,
/// so they are skipped when converting session messages to ChatMessages.
fn push_messages_as_chat_messages<'a, I>(messages: &mut Vec<ChatMessage>, source: I)
where
    I: IntoIterator<Item = &'a crate::chat::session::SavedMessage>,
{
    for msg in source {
        match msg.role {
            MessageRole::User => messages.push(ChatMessage::user(msg.content.clone())),
            MessageRole::Assistant => messages.push(ChatMessage::assistant(msg.content.clone())),
            MessageRole::System => { /* skip - handled separately */ }
            MessageRole::Tool => messages.push(ChatMessage::tool(msg.content.clone())),
        }
    }
}

/// Build context for LLM with optimal ordering
///
/// Context order (to avoid "lost in the middle"):
/// 1. System prompt (always first)
/// 2. Retrieved messages (after system, not in middle)
/// 3. Compacted summary (if present)
/// 4. Recent messages (chronological)
/// 5. Current query (always last)
///
/// Returns ContextResult with messages and retrieval status.
pub async fn build_context(
    session: &ChatSession,
    db: Option<&Arc<Database>>,
    embedding_client: Option<&Arc<EmbeddingClient>>,
    user_query: &str,
    system_prompt: &str,
    config: &RetrievalConfig,
) -> ContextResult {
    let mut messages = Vec::new();
    let mut retrieval_performed = false;
    let mut retrieved_count = 0;

    // 1. System prompt (always first - research shows up to 30% better performance)
    messages.push(ChatMessage::system(system_prompt.to_string()));

    // 2. Retrieved messages (placed after system to avoid being lost)
    // Normal retrieval: enabled and meets threshold
    let should_retrieve = config.enabled && config.should_retrieve(session, db);
    // Forced retrieval: after /clear, session empty but DB has messages
    let force_retrieve = should_force_retrieve(session, db);

    log::debug!(
        "Retrieval: enabled={}, should_retrieve={}, force_retrieve={}",
        config.enabled,
        should_retrieve,
        force_retrieve
    );
    log::debug!(
        "Session: id={}, anonymous={}, messages={}, has_summary={}",
        session.id,
        session.anonymous,
        session.messages.len(),
        session.compacted_summary.is_some()
    );

    if should_retrieve || force_retrieve {
        log::debug!(
            "Attempting retrieval: db={}, embedding_client={}",
            db.is_some(),
            embedding_client.is_some()
        );

        if let (Some(db), Some(client)) = (db, embedding_client) {
            if let Some(result) = perform_retrieval(
                db,
                client,
                user_query,
                Some(&session.id),
                session.project_id.as_deref(),
                config,
            )
            .await
            {
                messages.push(result.message);
                retrieved_count = result.count;
                retrieval_performed = true;
            }
        } else {
            log::debug!("Skipping retrieval: db or embedding_client not available");
        }
    } else {
        log::debug!("Skipping retrieval: conditions not met");
    }

    // 3. First preserved messages (if middle compaction)
    // According to "lost in the middle" research, important content should be
    // at BEGINNING or END, not middle.
    if let Some((first_preserved, _)) = session.compacted_range {
        // Clamp to actual message count to avoid panic after /clear
        let first_preserved = first_preserved.min(session.messages.len());
        if first_preserved > 0 {
            push_messages_as_chat_messages(&mut messages, &session.messages[..first_preserved]);
        }
    }

    // 4. Compacted summary (if present)
    if let Some(ref summary) = session.compacted_summary {
        messages.push(ChatMessage::system(format!(
            "<summary_context>\n{}\n</summary_context>",
            summary
        )));
    }

    // 5. Recent messages (before query - avoid "lost in middle")
    // Use compacted_range for middle compaction, or messages_sent_to_llm for legacy
    let start_idx = match session.compacted_range {
        Some((_, last_preserved_start)) => {
            // Clamp to actual message count to avoid panic after /clear
            last_preserved_start.min(session.messages.len())
        }
        None => session.messages_sent_to_llm.min(session.messages.len()),
    };

    let recent_messages: Vec<_> = session.messages[start_idx..]
        .iter()
        .rev()
        .take(config.recent_count)
        .rev()
        .collect();

    push_messages_as_chat_messages(&mut messages, recent_messages.into_iter());

    // 6. Current query (always at the very end - critical for model performance)
    // This is added by the caller, not here

    ContextResult {
        messages,
        retrieval_performed,
        retrieved_count,
    }
}

/// Build context for query mode (no session persistence)
///
/// Similar to build_context() but for ephemeral queries:
/// 1. System prompt (always first)
/// 2. Retrieved messages from project history (if available)
/// 3. Current query (always last)
///
/// Unlike build_context():
/// - No recent messages (no session state)
/// - No compacted summary (no session state)
/// - Only searches by project_id (not conversation_id)
/// - Does not persist any messages
pub async fn build_query_context(
    project_id: Option<&str>,
    db: Option<&Arc<Database>>,
    embedding_client: Option<&Arc<EmbeddingClient>>,
    user_query: &str,
    system_prompt: &str,
    config: &RetrievalConfig,
) -> ContextResult {
    let mut messages = Vec::new();
    let mut retrieval_performed = false;
    let mut retrieved_count = 0;

    // 1. System prompt (always first)
    messages.push(ChatMessage::system(system_prompt.to_string()));

    // 2. Retrieved messages (search across all project sessions)
    if config.enabled {
        log::debug!(
            "Query mode retrieval: project_id={:?}, enabled={}",
            project_id,
            config.enabled
        );

        if let (Some(db), Some(client)) = (db, embedding_client) {
            if let Some(result) = perform_retrieval(
                db, client, user_query, None, // No conversation_id - search all in project
                project_id, config,
            )
            .await
            {
                messages.push(result.message);
                retrieved_count = result.count;
                retrieval_performed = true;
            }
        } else {
            log::debug!("Skipping retrieval: db or embedding_client not available");
        }
    } else {
        log::debug!("Skipping retrieval: disabled");
    }

    // 3. Current query (always last)
    messages.push(ChatMessage::user(user_query.to_string()));

    log::debug!(
        "Query context built: {} messages, retrieval={}, retrieved={}",
        messages.len(),
        retrieval_performed,
        retrieved_count
    );

    ContextResult {
        messages,
        retrieval_performed,
        retrieved_count,
    }
}

/// Get effective message count for retrieval decisions
///
/// After `/clear`, session.messages may be empty but the database
/// still contains the conversation history. This function returns
/// the effective count considering both sources.
pub fn get_effective_message_count(session: &ChatSession, db: Option<&Arc<Database>>) -> usize {
    // If session has messages, use session count
    if !session.messages.is_empty() {
        return session.messages.len();
    }

    // If session is empty but has summary, check DB
    // (messages were cleared but context persists)
    if session.compacted_summary.is_some()
        && let Some(db) = db
        && let Ok(count) = db.count_conversation_items(&session.id)
    {
        return count as usize;
    }

    0
}

/// Check if retrieval should be forced after /clear
///
/// When session has fewer messages than the database, it means:
/// 1. User ran /clear (messages removed from session but not from DB)
/// 2. Or there's historical context in DB that should be retrieved
///
/// This check works regardless of how many new messages user added after /clear.
pub fn should_force_retrieve(session: &ChatSession, db: Option<&Arc<Database>>) -> bool {
    // Check DB for message count
    if let Some(db) = db
        && !session.anonymous
        && !session.id.is_empty()
        && let Ok(db_count) = db.count_conversation_items(&session.id)
    {
        // If DB has more messages than session, retrieval should happen
        // This covers:
        // - After /clear: DB has old messages, session has 0-1 new messages
        // - During conversation: DB and session are in sync
        let session_count = session.messages.len();
        if db_count as usize > session_count {
            return true;
        }
    }

    // Also force retrieve if session has summary (from compaction)
    // even if DB check didn't trigger
    if session.messages.is_empty() && session.compacted_summary.is_some() {
        return true;
    }

    false
}

impl RetrievalConfig {
    /// Check if retrieval should be performed
    ///
    /// After `/clear`, session.messages may be empty but DB still has history.
    /// This checks both the session and the database.
    pub fn should_retrieve(&self, session: &ChatSession, db: Option<&Arc<Database>>) -> bool {
        // Use session + DB count to decide
        let effective_count = get_effective_message_count(session, db);

        if effective_count < self.min_messages {
            return false;
        }

        // Check throttling
        if let Some(last_time) = session.last_retrieval_time {
            let elapsed = last_time.elapsed().as_secs();
            if elapsed < self.min_query_interval_secs {
                return false;
            }
        }

        true
    }
}

/// Update last retrieval time after successful retrieval
pub fn update_retrieval_time(session: &mut ChatSession) {
    session.last_retrieval_time = Some(Instant::now());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::session::SavedMessage;
    use chrono::Utc;

    fn create_test_session(message_count: usize) -> ChatSession {
        let mut session = ChatSession::new("test-model".to_string(), None, false);
        session.id = "test-conv".to_string();

        for i in 0..message_count {
            session.messages.push(SavedMessage {
                role: if i % 2 == 0 {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                content: format!("Test message {} content", i),
                timestamp: Utc::now(),
                ..Default::default()
            });
        }

        session
    }

    #[test]
    fn test_retrieval_config_default() {
        let config = RetrievalConfig::default();
        assert!(config.enabled);
        assert_eq!(config.min_messages, MIN_MESSAGES_FOR_RETRIEVAL);
        assert_eq!(config.relevant_count, RELEVANT_MESSAGES_COUNT);
        assert_eq!(config.recent_count, RECENT_MESSAGES_COUNT);
    }

    #[test]
    fn test_should_retrieve_below_min_messages() {
        let config = RetrievalConfig::default();
        let session = create_test_session(MIN_MESSAGES_FOR_RETRIEVAL - 2);

        assert!(!config.should_retrieve(&session, None));
    }

    #[test]
    fn test_should_retrieve_above_min_messages() {
        let config = RetrievalConfig::default();
        let session = create_test_session(MIN_MESSAGES_FOR_RETRIEVAL + 2);

        assert!(config.should_retrieve(&session, None));
    }

    #[test]
    fn test_should_retrieve_throttled() {
        let config = RetrievalConfig::default();
        let mut session = create_test_session(25);
        session.last_retrieval_time = Some(Instant::now()); // Just now

        // Should be throttled
        assert!(!config.should_retrieve(&session, None));
    }

    #[test]
    fn test_should_retrieve_after_throttle() {
        let config = RetrievalConfig {
            min_query_interval_secs: 0, // No throttle
            ..RetrievalConfig::default()
        };
        let mut session = create_test_session(25);
        session.last_retrieval_time = Some(Instant::now());

        // Should not be throttled with 0 interval
        assert!(config.should_retrieve(&session, None));
    }

    #[test]
    fn test_update_retrieval_time() {
        let mut session = create_test_session(10);

        // Initially None
        assert!(session.last_retrieval_time.is_none());

        // Update
        update_retrieval_time(&mut session);
        assert!(session.last_retrieval_time.is_some());
    }

    #[test]
    fn test_retrieval_toggle() {
        let mut session = create_test_session(10);

        // Initially true (default changed in v0.23.0)
        assert!(session.retrieval_enabled);

        // Toggle off
        session.retrieval_enabled = false;
        assert!(!session.retrieval_enabled);

        // Toggle back on
        session.retrieval_enabled = true;
        assert!(session.retrieval_enabled);
    }

    #[test]
    fn test_get_effective_message_count_with_messages() {
        let session = create_test_session(25);

        // Session has messages, so count should be session len
        let count = get_effective_message_count(&session, None);
        assert_eq!(count, 25);
    }

    #[test]
    fn test_get_effective_message_count_empty_no_summary() {
        let session = create_test_session(0);

        // Empty session, no summary, no DB
        let count = get_effective_message_count(&session, None);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_get_effective_message_count_after_clear_with_summary() {
        let mut session = create_test_session(0); // Empty after clear
        session.set_compacted_summary_with_range("Summary".into(), Some((5, 20)));

        // After /clear, session is empty but has summary
        // Without DB, can't check DB count
        let count = get_effective_message_count(&session, None);
        assert_eq!(count, 0); // Can't reach DB, returns 0
    }

    #[test]
    fn test_should_force_retrieve_with_messages() {
        let session = create_test_session(10);

        // Session has 10 messages, no DB - can't compare, should NOT force
        assert!(!should_force_retrieve(&session, None));
    }

    #[test]
    fn test_should_force_retrieve_after_clear_with_new_messages() {
        // Simulates: user ran /clear, then asked 1-2 questions
        // Session has 2 messages (after clear), DB has 6 (before clear)
        let session = create_test_session(2);
        // Without DB, can't detect the difference
        assert!(!should_force_retrieve(&session, None));
    }

    #[test]
    fn test_should_force_retrieve_empty_with_summary() {
        let mut session = create_test_session(0); // Empty after clear
        session.set_compacted_summary_with_range("Summary".into(), Some((0, 10)));

        // Empty session with summary - should force retrieve
        assert!(should_force_retrieve(&session, None));
    }

    #[test]
    fn test_should_force_retrieve_empty_no_summary() {
        let session = create_test_session(0); // Empty, no summary

        // Empty session, no summary, no DB - should NOT force
        assert!(!should_force_retrieve(&session, None));
    }

    #[test]
    fn test_push_messages_filters_system() {
        use crate::chat::session::SavedMessage;

        let mut messages = Vec::new();
        let source = vec![
            SavedMessage {
                role: MessageRole::User,
                content: "user msg".to_string(),
                ..Default::default()
            },
            SavedMessage {
                role: MessageRole::System,
                content: "system msg".to_string(),
                ..Default::default()
            },
            SavedMessage {
                role: MessageRole::Assistant,
                content: "assistant msg".to_string(),
                ..Default::default()
            },
        ];

        push_messages_as_chat_messages(&mut messages, source.iter());

        // System message should be filtered out
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "user msg");
        assert_eq!(messages[1].content, "assistant msg");
    }

    #[test]
    fn test_push_messages_converts_all_roles() {
        use crate::chat::session::SavedMessage;

        let mut messages = Vec::new();
        let source = vec![
            SavedMessage {
                role: MessageRole::User,
                content: "user".to_string(),
                ..Default::default()
            },
            SavedMessage {
                role: MessageRole::Assistant,
                content: "assistant".to_string(),
                ..Default::default()
            },
            SavedMessage {
                role: MessageRole::Tool,
                content: "tool".to_string(),
                ..Default::default()
            },
        ];

        push_messages_as_chat_messages(&mut messages, source.iter());

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].content, "user");
        assert_eq!(messages[1].content, "assistant");
        assert_eq!(messages[2].content, "tool");
    }

    #[test]
    fn test_push_messages_empty_source() {
        let mut messages = Vec::new();
        let source: Vec<SavedMessage> = Vec::new();

        push_messages_as_chat_messages(&mut messages, source.iter());

        assert!(messages.is_empty());
    }
}
