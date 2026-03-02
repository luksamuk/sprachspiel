//! Token estimation utilities
//!
//! Word-based estimation with message overhead for LLM context management.

use ollama_rs::generation::chat::ChatMessage;

/// Tokens per message overhead (role, formatting)
pub const MESSAGE_OVERHEAD: usize = 4;

/// Estimate tokens in text using word-based estimation
/// ~0.75 words per token for English text (GPT-style)
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let word_count = text.split_whitespace().count();
    ((word_count as f32) / 0.75).ceil() as usize
}

/// Estimate tokens for code content
/// Code has higher entropy: ~0.5 tokens per character
///
/// Note: This function is kept for future use in code-specific token estimation.
/// It is tested and available for when code token counting is needed.
#[allow(dead_code)]
pub fn estimate_tokens_code(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    ((text.len() as f32) * 0.5).ceil() as usize
}

/// Count tokens in a list of chat messages
/// Includes 4 tokens overhead per message for role and formatting
pub fn count_messages_tokens(messages: &[ChatMessage]) -> usize {
    if messages.is_empty() {
        return 0;
    }
    messages
        .iter()
        .map(|msg| MESSAGE_OVERHEAD + estimate_tokens(&msg.content))
        .sum()
}

/// Context window usage metrics
#[derive(Debug, Clone, Copy)]
pub struct ContextMetrics {
    /// Tokens used by system prompt
    pub system_tokens: usize,
    /// Tokens used by tool definitions
    pub tools_tokens: usize,
    /// Tokens used by conversation history
    pub history_tokens: usize,
    /// Total tokens used (system + tools + history)
    pub total_tokens: usize,
    /// Maximum context window size
    pub context_window: usize,
    /// Utilization percentage (0.0 to 1.0)
    pub utilization: f32,
}

impl ContextMetrics {
    /// Returns available tokens remaining in context window
    pub fn available(&self) -> usize {
        self.context_window.saturating_sub(self.total_tokens)
    }
}

/// Calculate context metrics from session state
///
/// # Arguments
/// * `history_messages` - Chat messages from conversation history
/// * `context_window` - Maximum context window size in tokens
/// * `system_prompt` - System prompt text
/// * `tools_tokens` - Estimated tokens for tool definitions
pub fn calculate_context_metrics(
    history_messages: &[ChatMessage],
    context_window: usize,
    system_prompt: &str,
    tools_tokens: usize,
) -> ContextMetrics {
    let system_tokens = estimate_tokens(system_prompt) + MESSAGE_OVERHEAD;
    let history_tokens = count_messages_tokens(history_messages);
    let total_tokens = system_tokens
        .saturating_add(tools_tokens)
        .saturating_add(history_tokens);
    let utilization = if context_window > 0 {
        (total_tokens as f32 / context_window as f32).min(1.0)
    } else {
        0.0
    };
    ContextMetrics {
        system_tokens,
        tools_tokens,
        history_tokens,
        total_tokens,
        context_window,
        utilization,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_simple() {
        let text = "hello world";
        assert_eq!(estimate_tokens(text), 3);
    }

    #[test]
    fn test_estimate_tokens_longer() {
        let text = "The quick brown fox jumps over the lazy dog";
        assert_eq!(estimate_tokens(text), 12);
    }

    #[test]
    fn test_estimate_tokens_multiple_spaces() {
        let text = "hello    world   test";
        assert_eq!(estimate_tokens(text), 4);
    }

    #[test]
    fn test_estimate_tokens_code_empty() {
        assert_eq!(estimate_tokens_code(""), 0);
    }

    #[test]
    fn test_estimate_tokens_code_simple() {
        let code = "fn main() {}";
        assert_eq!(estimate_tokens_code(code), 6);
    }

    #[test]
    fn test_count_messages_tokens_empty() {
        let messages: Vec<ChatMessage> = Vec::new();
        assert_eq!(count_messages_tokens(&messages), 0);
    }

    #[test]
    fn test_count_messages_tokens_single() {
        let messages = vec![ChatMessage::user("hello world".to_string())];
        assert_eq!(count_messages_tokens(&messages), 7);
    }

    #[test]
    fn test_count_messages_tokens_multiple() {
        let messages = vec![
            ChatMessage::user("hello world".to_string()),
            ChatMessage::assistant("hi there".to_string()),
        ];
        assert_eq!(count_messages_tokens(&messages), 14);
    }

    #[test]
    fn test_context_metrics_available() {
        let metrics = ContextMetrics {
            system_tokens: 100,
            tools_tokens: 50,
            history_tokens: 200,
            total_tokens: 350,
            context_window: 4096,
            utilization: 0.085,
        };
        assert_eq!(metrics.available(), 3746);
    }

    #[test]
    fn test_context_metrics_available_overflow() {
        let metrics = ContextMetrics {
            system_tokens: 1000,
            tools_tokens: 500,
            history_tokens: 4000,
            total_tokens: 5500,
            context_window: 4096,
            utilization: 1.0,
        };
        assert_eq!(metrics.available(), 0);
    }

    #[test]
    fn test_calculate_context_metrics() {
        let messages = vec![
            ChatMessage::user("hello world".to_string()),
            ChatMessage::assistant("hi there".to_string()),
        ];
        let metrics = calculate_context_metrics(&messages, 4096, "You are helpful.", 100);
        assert_eq!(metrics.system_tokens, 8);
        assert_eq!(metrics.tools_tokens, 100);
        assert_eq!(metrics.history_tokens, 14);
        assert_eq!(metrics.total_tokens, 122);
        assert!((metrics.utilization - 0.029).abs() < 0.001);
    }

    #[test]
    fn test_calculate_context_metrics_empty() {
        let messages: Vec<ChatMessage> = Vec::new();
        let metrics = calculate_context_metrics(&messages, 4096, "", 0);
        assert_eq!(metrics.system_tokens, 4);
        assert_eq!(metrics.history_tokens, 0);
        assert_eq!(metrics.total_tokens, 4);
    }
}
