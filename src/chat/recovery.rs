//! ChatMessage::tool() wrapper for LLM error recovery.
//!
//! Delegates to `ChatMessage::tool()` from the shim. In #123 (Remove
//! ollama-rs), this will be replaced with `LlmMessage::tool()` from
//! #119 (Agnostic Provider Types).
//!
//! The recovery pattern (push a tool message after an error so the LLM can
//! self-correct) is the central pattern of #116. By centralizing the
//! `ChatMessage::tool()` call here, future changes to the message format
//! only need to update ONE function body.
//!
//! See: `IMPLEMENTATION.md` — W2 Provider Chain

use crate::provider::ollama_shim::ChatMessage;

/// Push a tool result message into the conversation history.
///
/// Wraps the ollama-rs shim's `ChatMessage::tool()` for the legacy
/// coordinator path. The shim's `ChatMessage` is re-exported from
/// `ollama_rs::generation::chat::ChatMessage`.
pub fn push_tool_result(messages: &mut Vec<ChatMessage>, content: String) {
    messages.push(ChatMessage::tool(content));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ollama_shim::MessageRole;

    #[test]
    fn test_push_tool_result_appends_message() {
        let mut messages: Vec<ChatMessage> = Vec::new();
        push_tool_result(&mut messages, "Error: tool failed".to_string());
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_push_tool_result_preserves_existing_messages() {
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
