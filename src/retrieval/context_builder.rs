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
use crate::db::Database;
use crate::debug_tools::log_debug;
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
    use_debug: bool,
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
    
    if use_debug {
        log_debug(&format!(
            "Retrieval: enabled={}, should_retrieve={}, force_retrieve={}",
            config.enabled, should_retrieve, force_retrieve
        ));
        log_debug(&format!(
            "Session: id={}, anonymous={}, messages={}, has_summary={}",
            session.id,
            session.anonymous,
            session.messages.len(),
            session.compacted_summary.is_some()
        ));
    }
    
    if should_retrieve || force_retrieve {
        if use_debug {
            log_debug(&format!(
                "Attempting retrieval: db={}, embedding_client={}",
                db.is_some(),
                embedding_client.is_some()
            ));
        }
        
        if let (Some(db), Some(client)) = (db, embedding_client) {
            if use_debug {
                log_debug("Generating embedding for query...");
            }
            
            if let Ok(embedding) = client.embed(user_query).await {
                if use_debug {
                    log_debug(&format!(
                        "Searching for relevant messages in conversation: {}",
                        session.id
                    ));
                }
                
                if let Ok(results) = db.search_hybrid(
                    user_query,
                    &embedding,
                    Some(&session.id),
                    config.relevant_count,
                    config.keyword_weight,
                    config.semantic_weight,
                ) {
                    // Enrich results with conversation context (attach assistant responses to user questions)
                    let enriched_results = match db.enrich_with_context(results) {
                        Ok(r) => r,
                        Err(e) => {
                            if use_debug {
                                log_debug(&format!("Warning: Failed to enrich results: {}", e));
                            }
                            // Return empty on error - the original `results` is consumed by enrich_with_context
                            Vec::new()
                        }
                    };
                    
                    if use_debug {
                        log_debug(&format!(
                            "Search returned {} results",
                            enriched_results.len()
                        ));
                    }
                    
                    if !enriched_results.is_empty() {
                        retrieved_count = enriched_results.len();
                        retrieval_performed = true;
                        
                        let mut retrieved_text = String::from("<retrieved_context>\n");
                        retrieved_text.push_str("MESSAGES FROM YOUR PAST CONVERSATION with this user.\n");
                        retrieved_text.push_str("Each message has an ID. Use remember(id=\"N\") for full content.\n");
                        retrieved_text.push_str("Use remember(query=\"topic\") to search for past discussions.\n\n");
                        for msg in enriched_results.iter() {
                            retrieved_text.push_str(&format!(
                                "<message id=\"{}\">\n<role>{}</role>\n<content>{}</content>\n</message>\n",
                                msg.message_id,
                                msg.role,
                                msg.content
                            ));
                            
                            // If user message has an assistant response, include it
                            if let Some(ref answer) = msg.next_message {
                                retrieved_text.push_str(&format!(
                                    "<message id=\"{}\">\n<role>{}</role>\n<content>{}</content>\n</message>\n",
                                    answer.message_id,
                                    answer.role,
                                    answer.content
                                ));
                            }
                        }
                        retrieved_text.push_str("</retrieved_context>");
                        messages.push(ChatMessage::system(retrieved_text));
                        
                        if use_debug {
                            let enriched_count = enriched_results.iter().filter(|r| r.next_message.is_some()).count();
                            log_debug(&format!(
                                "Added {} retrieved messages to context ({} enriched with responses)",
                                retrieved_count,
                                enriched_count
                            ));
                        }
                    }
                } else if use_debug {
                    log_debug("Search returned no results");
                }
            } else if use_debug {
                log_debug("Failed to generate embedding for query");
            }
        } else if use_debug {
            log_debug("Skipping retrieval: db or embedding_client not available");
        }
    } else if use_debug {
        log_debug("Skipping retrieval: conditions not met");
    }

    // 3. First preserved messages (if middle compaction)
    // According to "lost in the middle" research, important content should be
    // at BEGINNING or END, not middle.
    if let Some((first_preserved, _)) = session.compacted_range {
        if first_preserved > 0 {
            for msg in &session.messages[..first_preserved] {
                match msg.role {
                    MessageRole::User => {
                        messages.push(ChatMessage::user(msg.content.clone()));
                    }
                    MessageRole::Assistant => {
                        messages.push(ChatMessage::assistant(msg.content.clone()));
                    }
                    MessageRole::System => {
                        // System messages are handled separately
                    }
                    MessageRole::Tool => {
                        messages.push(ChatMessage::tool(msg.content.clone()));
                    }
                }
            }
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
        Some((_, last_preserved_start)) => last_preserved_start,
        None => session.messages_sent_to_llm.min(session.messages.len()),
    };

    let recent_messages: Vec<_> = session.messages[start_idx..]
        .iter()
        .rev()
        .take(config.recent_count)
        .rev()
        .collect();

    for msg in recent_messages {
        match msg.role {
            MessageRole::User => {
                messages.push(ChatMessage::user(msg.content.clone()));
            }
            MessageRole::Assistant => {
                messages.push(ChatMessage::assistant(msg.content.clone()));
            }
            MessageRole::System => {
                // System messages are handled separately
            }
            MessageRole::Tool => {
                messages.push(ChatMessage::tool(msg.content.clone()));
            }
        }
    }

    // 6. Current query (always at the very end - critical for model performance)
    // This is added by the caller, not here

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
pub fn get_effective_message_count(
    session: &ChatSession,
    db: Option<&Arc<Database>>,
) -> usize {
    // If session has messages, use session count
    if !session.messages.is_empty() {
        return session.messages.len();
    }
    
    // If session is empty but has summary, check DB
    // (messages were cleared but context persists)
    if session.compacted_summary.is_some() {
        if let Some(db) = db {
            if let Ok(count) = db.count_conversation_messages(&session.id) {
                return count;
            }
        }
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
pub fn should_force_retrieve(
    session: &ChatSession,
    db: Option<&Arc<Database>>,
) -> bool {
    // Check DB for message count
    if let Some(db) = db {
        if !session.anonymous && !session.id.is_empty() {
            if let Ok(db_count) = db.count_conversation_messages(&session.id) {
                // If DB has more messages than session, retrieval should happen
                // This covers:
                // - After /clear: DB has old messages, session has 0-1 new messages
                // - During conversation: DB and session are in sync
                let session_count = session.messages.len();
                if db_count > session_count {
                    return true;
                }
            }
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
                role: if i % 2 == 0 { MessageRole::User } else { MessageRole::Assistant },
                content: format!("Test message {} content", i),
                timestamp: Utc::now(),
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
        let mut session = create_test_session(0);  // Empty after clear
        session.set_compacted_summary_with_range("Summary".into(), Some((5, 20)));
        
        // After /clear, session is empty but has summary
        // Without DB, can't check DB count
        let count = get_effective_message_count(&session, None);
        assert_eq!(count, 0);  // Can't reach DB, returns 0
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
        let mut session = create_test_session(0);  // Empty after clear
        session.set_compacted_summary_with_range("Summary".into(), Some((0, 10)));
        
        // Empty session with summary - should force retrieve
        assert!(should_force_retrieve(&session, None));
    }

    #[test]
    fn test_should_force_retrieve_empty_no_summary() {
        let session = create_test_session(0);  // Empty, no summary
        
        // Empty session, no summary, no DB - should NOT force
        assert!(!should_force_retrieve(&session, None));
    }
}