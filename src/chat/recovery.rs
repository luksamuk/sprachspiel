//! ChatMessage::tool() wrapper for LLM error recovery.
//!
//! **W2 Wave Context:** This wrapper exists to localize the migration point
//! in #121 (Consumer Migration). Currently delegates to
//! `ollama_rs::ChatMessage::tool()`. In #121, this will be replaced with
//! `LlmMessage::tool()` from #119 (Agnostic Provider Types).
//!
//! The recovery pattern (push a tool message after an error so the LLM can
//! self-correct) is the central pattern of #116. By centralizing the
//! `ChatMessage::tool()` call here, #121 only needs to change ONE function
//! body instead of all call sites.
//!
//! **Exception:** `src/chat/custom_coordinator.rs` uses
//! `self.history.push(ChatMessage::tool(...))` directly because
//! `self.history` is generic over `C: ChatHistory` (an ollama-rs trait),
//! not `Vec<ChatMessage>`. This exception is expected to be resolved by
//! #121 (Consumer Migration), which will replace the `ChatHistory` type
//! with agnostic types.
//!
//! See: `IMPLEMENTATION.md` — W2 Provider Chain

use ollama_rs::generation::chat::ChatMessage;

/// Push a tool result message into the conversation history.
///
/// This wraps `ChatMessage::tool()` so that #121 (Consumer Migration)
/// can swap the implementation to `LlmMessage::tool()` in a single
/// place. Callers should NEVER call `ChatMessage::tool()` directly for
/// error recovery — use this wrapper instead.
pub fn push_tool_result(messages: &mut Vec<ChatMessage>, content: String) {
    messages.push(ChatMessage::tool(content));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_tool_result_appends_message() {
        let mut messages: Vec<ChatMessage> = Vec::new();
        push_tool_result(&mut messages, "Error: tool failed".to_string());
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_push_tool_result_preserves_existing_messages() {
        use ollama_rs::generation::chat::MessageRole;
        let mut messages: Vec<ChatMessage> = vec![ChatMessage {
            role: MessageRole::User,
            content: "Hello".to_string(),
            tool_calls: vec![],
            images: None,
            thinking: None,
        }];
        push_tool_result(&mut messages, "Error: tool failed".to_string());
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].content, "Error: tool failed");
    }
}
