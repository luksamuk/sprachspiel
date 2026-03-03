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

use crate::chat::session::{ChatSession, MessageRole};
use crate::db::Database;
use crate::embeddings::EmbeddingClient;

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
            min_messages: 20,
            relevant_count: 5,
            recent_count: 10,
            min_query_interval_secs: 5,
            keyword_weight: 0.4,
            semantic_weight: 0.6,
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
pub async fn build_context(
    session: &ChatSession,
    db: Option<&Arc<Database>>,
    embedding_client: Option<&Arc<EmbeddingClient>>,
    user_query: &str,
    system_prompt: &str,
    config: &RetrievalConfig,
) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    // 1. System prompt (always first - research shows up to 30% better performance)
    messages.push(ChatMessage::system(system_prompt.to_string()));

    // 2. Retrieved messages (if enabled - placed after system to avoid being lost)
    if config.enabled && config.should_retrieve(session) {
        if let (Some(db), Some(client)) = (db, embedding_client) {
            if let Ok(embedding) = client.embed(user_query).await {
                if let Ok(results) = db.search_hybrid(
                    user_query,
                    &embedding,
                    Some(&session.id),
                    config.relevant_count,
                    config.keyword_weight,
                    config.semantic_weight,
                ) {
                    if !results.is_empty() {
                        let mut retrieved_text = String::from("<retrieved_context>\n");
                        for (i, msg) in results.iter().enumerate() {
                            retrieved_text.push_str(&format!(
                                "<message index=\"{}\" timestamp=\"{}\">\n<role>{}</role>\n<content>{}</content>\n</message>\n",
                                i + 1,
                                msg.timestamp,
                                msg.role,
                                msg.content
                            ));
                        }
                        retrieved_text.push_str("</retrieved_context>");
                        messages.push(ChatMessage::system(retrieved_text));
                    }
                }
            }
        }
    }

    // 3. Compacted summary (if present)
    if let Some(ref summary) = session.compacted_summary {
        messages.push(ChatMessage::system(format!(
            "<summary_context>\n{}\n</summary_context>",
            summary
        )));
    }

    // 4. Recent messages (before query - avoid "lost in middle")
    let start_idx = session.messages_sent_to_llm.min(session.messages.len());
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

    // 5. Current query (always at the very end - critical for model performance)
    // This is added by the caller, not here

    messages
}

impl RetrievalConfig {
    /// Check if retrieval should be performed
    pub fn should_retrieve(&self, session: &ChatSession) -> bool {
        // Check minimum messages threshold
        if session.messages.len() < self.min_messages {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::session::SavedMessage;
    use chrono::Utc;
    use std::time::Instant;

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
        assert_eq!(config.min_messages, 20);
        assert_eq!(config.relevant_count, 5);
        assert_eq!(config.recent_count, 10);
    }

    #[test]
    fn test_should_retrieve_below_min_messages() {
        let config = RetrievalConfig::default();
        let session = create_test_session(10); // Below min_messages of 20

        assert!(!config.should_retrieve(&session));
    }

    #[test]
    fn test_should_retrieve_above_min_messages() {
        let config = RetrievalConfig::default();
        let session = create_test_session(25);

        assert!(config.should_retrieve(&session));
    }

    #[test]
    fn test_should_retrieve_throttled() {
        let config = RetrievalConfig::default();
        let mut session = create_test_session(25);
        session.last_retrieval_time = Some(Instant::now()); // Just now

        // Should be throttled
        assert!(!config.should_retrieve(&session));
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
        assert!(config.should_retrieve(&session));
    }
}