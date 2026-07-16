//! LlmMessage::tool() wrapper for LLM error recovery.
//!
//! The recovery pattern (push a tool message after an error so the LLM can
//! self-correct) is centralized here so future changes to the message format
//! only need to update ONE function body.

use crate::provider::types::LlmMessage;

/// Push a tool result message into the conversation history.
pub fn push_tool_result(messages: &mut Vec<LlmMessage>, content: String) {
    messages.push(LlmMessage::tool(content));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::types::LlmRole;

    #[test]
    fn test_push_tool_result_appends_message() {
        let mut messages: Vec<LlmMessage> = Vec::new();
        push_tool_result(&mut messages, "Error: tool failed".to_string());
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_push_tool_result_preserves_existing_messages() {
        let mut messages: Vec<LlmMessage> = vec![LlmMessage {
            role: LlmRole::User,
            content: "Hello".to_string(),
            tool_calls: None,
            images: None,
            audio: None,
            thinking: None,
            name: None,
            tool_call_id: None,
        }];
        push_tool_result(&mut messages, "Error: tool failed".to_string());
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].content, "Error: tool failed");
    }
}
