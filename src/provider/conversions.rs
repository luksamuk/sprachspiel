//! Bidirectional conversions between ollama-rs types and provider-agnostic types.
//!
//! These conversions enable transparent migration from ollama-rs to the
//! provider abstraction layer. All conversions are lossless where possible;
//! fallible conversions use `TryFrom`.
//!
//! Note: We avoid implementing `From` for external types (orphan rules).
//! Instead, we provide helper functions and implement conversions between
//! our types and ollama-rs types where we own at least one type.

use ollama_rs::{
    error::{OllamaError, ToolCallError},
    generation::{
        chat::{ChatMessage, ChatMessageResponse, MessageRole as OllamaMessageRole},
        images::Image as OllamaImage,
        tools::ToolCall as OllamaToolCall,
    },
};

use crate::provider::types::{
    LlmMessage, LlmResponse, LlmRole, LlmStreamChunk, LlmToolCall, ProviderError, ProviderOptions,
};

/// Convert Ollama message role to our LlmRole.
impl From<OllamaMessageRole> for LlmRole {
    fn from(role: OllamaMessageRole) -> Self {
        match role {
            OllamaMessageRole::User => LlmRole::User,
            OllamaMessageRole::Assistant => LlmRole::Assistant,
            OllamaMessageRole::System => LlmRole::System,
            OllamaMessageRole::Tool => LlmRole::Tool,
        }
    }
}

/// Convert our LlmRole to Ollama message role.
impl From<LlmRole> for OllamaMessageRole {
    fn from(role: LlmRole) -> Self {
        match role {
            LlmRole::User => OllamaMessageRole::User,
            LlmRole::Assistant => OllamaMessageRole::Assistant,
            LlmRole::System => OllamaMessageRole::System,
            LlmRole::Tool => OllamaMessageRole::Tool,
        }
    }
}

/// Convert Ollama Image to base64 String.
pub fn ollama_image_to_string(img: OllamaImage) -> String {
    img.to_base64().to_string()
}

/// Convert base64 String to Ollama Image.
pub fn string_to_ollama_image(s: String) -> OllamaImage {
    OllamaImage::from_base64(s)
}

/// Convert Ollama ChatMessage to our LlmMessage.
impl From<ChatMessage> for LlmMessage {
    fn from(msg: ChatMessage) -> Self {
        LlmMessage {
            role: msg.role.into(),
            content: msg.content,
            tool_calls: if msg.tool_calls.is_empty() {
                None
            } else {
                Some(msg.tool_calls.into_iter().map(Into::into).collect())
            },
            images: msg
                .images
                .map(|imgs| imgs.into_iter().map(ollama_image_to_string).collect()),
            audio: None, // Ollama doesn't have audio field yet
            thinking: msg.thinking,
        }
    }
}

/// Convert our LlmMessage to Ollama ChatMessage.
///
/// This is fallible because our LlmMessage has audio field which Ollama doesn't support.
impl TryFrom<LlmMessage> for ChatMessage {
    type Error = ProviderError;

    fn try_from(msg: LlmMessage) -> Result<Self, Self::Error> {
        if msg.audio.is_some() {
            return Err(ProviderError::Unsupported(
                "audio input not supported by Ollama backend".into(),
            ));
        }

        Ok(ChatMessage {
            role: msg.role.into(),
            content: msg.content,
            tool_calls: msg
                .tool_calls
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            images: {
                let mut imgs = Vec::new();
                if let Some(audio) = msg.images {
                    for s in audio {
                        imgs.push(string_to_ollama_image(s));
                    }
                }
                Some(imgs)
            },
            thinking: msg.thinking,
        })
    }
}

/// Convert Ollama ToolCall to our LlmToolCall.
impl From<OllamaToolCall> for LlmToolCall {
    fn from(call: OllamaToolCall) -> Self {
        LlmToolCall {
            // Ollama ToolCall doesn't have an ID field; generate one from name + hash of args
            id: format!(
                "call_{}_{}",
                call.function.name,
                call.function.arguments.to_string().len()
            ),
            name: call.function.name,
            arguments: call.function.arguments,
        }
    }
}

/// Convert our LlmToolCall to Ollama ToolCall.
impl From<LlmToolCall> for OllamaToolCall {
    fn from(call: LlmToolCall) -> Self {
        OllamaToolCall {
            function: ollama_rs::generation::tools::ToolCallFunction {
                name: call.name,
                arguments: call.arguments,
            },
        }
    }
}

/// Convert Ollama ChatMessageResponse to our LlmResponse.
impl From<ChatMessageResponse> for LlmResponse {
    fn from(resp: ChatMessageResponse) -> Self {
        LlmResponse {
            model: resp.model,
            content: resp.message.content,
            tool_calls: if resp.message.tool_calls.is_empty() {
                None
            } else {
                Some(
                    resp.message
                        .tool_calls
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                )
            },
            done_reason: None, // Ollama doesn't provide done_reason in ChatMessageFinalResponseData
            eval_count: resp.final_data.as_ref().map(|d| d.eval_count as u32),
            prompt_eval_count: resp.final_data.as_ref().map(|d| d.prompt_eval_count as u32),
        }
    }
}

/// Convert our LlmStreamChunk to a format suitable for streaming.
///
/// Note: This is used when we need to convert our streaming chunks to Ollama's format
/// for compatibility during transition. The reverse is handled by the streaming parser.
impl From<LlmStreamChunk> for ChatMessageResponse {
    fn from(chunk: LlmStreamChunk) -> Self {
        let mut msg = ChatMessage::assistant(chunk.content.unwrap_or_default());
        if let Some(thinking) = chunk.thinking {
            msg.thinking = Some(thinking);
        }
        if let Some(calls) = chunk.tool_calls {
            msg.tool_calls = calls.into_iter().map(Into::into).collect();
        }

        ChatMessageResponse {
            model: "stream".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            message: msg,
            logprobs: None,
            done: chunk.done,
            final_data: if chunk.done {
                Some(ollama_rs::generation::chat::ChatMessageFinalResponseData {
                    total_duration: 0,
                    load_duration: 0,
                    prompt_eval_count: chunk.prompt_eval_count.unwrap_or(0) as u64,
                    prompt_eval_duration: 0,
                    eval_count: chunk.eval_count.unwrap_or(0) as u64,
                    eval_duration: 0,
                })
            } else {
                None
            },
        }
    }
}

/// Convert OllamaError to ProviderError with retry semantics preserved.
impl From<OllamaError> for ProviderError {
    fn from(err: OllamaError) -> Self {
        match err {
            OllamaError::ReqwestError(e) => {
                if e.is_timeout() {
                    ProviderError::Timeout(e.to_string())
                } else if e.is_connect() {
                    ProviderError::Connection(e.to_string())
                } else if let Some(status) = e.status() {
                    let code = status.as_u16();
                    if code == 429 {
                        ProviderError::RateLimit {
                            message: e.to_string(),
                            retry_after: None, // Would need header parsing
                        }
                    } else {
                        ProviderError::Api {
                            status: code,
                            body: e.to_string(),
                        }
                    }
                } else {
                    ProviderError::Connection(e.to_string())
                }
            }
            OllamaError::InternalError(e) => ProviderError::Api {
                status: 500,
                body: e.message,
            },
            OllamaError::ToolCallError(e) => match e {
                ToolCallError::UnknownToolName => ProviderError::Config("Unknown tool name".into()),
                ToolCallError::InvalidToolArguments(e) => {
                    ProviderError::Config(format!("Invalid tool arguments: {e}"))
                }
                ToolCallError::InternalToolError(e) => {
                    ProviderError::Other(format!("Internal tool error: {e}"))
                }
            },
            OllamaError::JsonError(e) => ProviderError::Config(format!("JSON error: {e}")),
            OllamaError::Other(s) => ProviderError::Other(s),
        }
    }
}

/// Convert ProviderOptions to Ollama ModelOptions.
impl From<ProviderOptions> for ollama_rs::models::ModelOptions {
    fn from(opts: ProviderOptions) -> Self {
        let mut options = ollama_rs::models::ModelOptions::default();
        if let Some(temp) = opts.temperature {
            options = options.temperature(temp);
        }
        if let Some(top_p) = opts.top_p {
            options = options.top_p(top_p);
        }
        if let Some(top_k) = opts.top_k {
            options = options.top_k(top_k);
        }
        if let Some(repeat_penalty) = opts.repeat_penalty {
            options = options.repeat_penalty(repeat_penalty);
        }
        if let Some(num_predict) = opts.num_predict {
            options = options.num_predict(num_predict);
        }
        if let Some(stop) = opts.stop_sequences {
            options = options.stop(stop);
        }
        options
    }
}

/// Convert Ollama ModelOptions to ProviderOptions.
impl From<ollama_rs::models::ModelOptions> for ProviderOptions {
    fn from(_opts: ollama_rs::models::ModelOptions) -> Self {
        // Note: ModelOptions fields are private, so we can't extract them directly.
        // This conversion is mainly for completeness; in practice we build ProviderOptions
        // from config directly.
        ProviderOptions::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::types::RetryCategory;
    use ollama_rs::error::InternalOllamaError;
    use ollama_rs::generation::chat::{
        ChatMessage as OllamaChatMessage, MessageRole as OllamaMessageRole,
    };
    use ollama_rs::generation::images::Image as OllamaImage;
    use ollama_rs::generation::tools::ToolCall as OllamaToolCall;
    use ollama_rs::generation::tools::ToolCallFunction;
    use serde_json::json;

    // --- ChatMessage <-> LlmMessage roundtrip ---

    #[test]
    fn test_chatmessage_to_llmmessage_basic() {
        let ollama_msg = OllamaChatMessage::user("Hello, world!".to_string());
        let llm_msg: LlmMessage = ollama_msg.into();

        assert_eq!(llm_msg.role, LlmRole::User);
        assert_eq!(llm_msg.content, "Hello, world!");
        assert!(llm_msg.tool_calls.is_none());
        assert!(llm_msg.images.is_none());
        assert!(llm_msg.audio.is_none());
        assert!(llm_msg.thinking.is_none());
    }

    #[test]
    fn test_chatmessage_to_llmmessage_with_tool_calls() {
        let mut ollama_msg = OllamaChatMessage::assistant("I'll help".to_string());
        ollama_msg.tool_calls = vec![OllamaToolCall {
            function: ToolCallFunction {
                name: "get_weather".to_string(),
                arguments: json!({"city": "São Paulo"}),
            },
        }];

        let llm_msg: LlmMessage = ollama_msg.into();

        assert_eq!(llm_msg.role, LlmRole::Assistant);
        assert!(llm_msg.tool_calls.is_some());
        let calls = llm_msg.tool_calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments["city"], "São Paulo");
    }

    #[test]
    fn test_chatmessage_to_llmmessage_with_images() {
        let mut ollama_msg = OllamaChatMessage::user("What's this?".to_string());
        ollama_msg.images = Some(vec![OllamaImage::from_base64("base64data".to_string())]);

        let llm_msg: LlmMessage = ollama_msg.into();

        assert!(llm_msg.images.is_some());
        let images = llm_msg.images.unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0], "base64data");
    }

    #[test]
    fn test_chatmessage_to_llmmessage_with_thinking() {
        let mut ollama_msg = OllamaChatMessage::assistant("Answer".to_string());
        ollama_msg.thinking = Some("Let me think...".to_string());

        let llm_msg: LlmMessage = ollama_msg.into();

        assert_eq!(llm_msg.thinking, Some("Let me think...".to_string()));
    }

    #[test]
    fn test_llmmessage_to_chatmessage_basic() {
        let llm_msg = LlmMessage::user("Hello".to_string());
        let ollama_msg: ChatMessage = llm_msg.try_into().unwrap();

        assert_eq!(ollama_msg.role, OllamaMessageRole::User);
        assert_eq!(ollama_msg.content, "Hello");
        assert!(ollama_msg.tool_calls.is_empty());
        assert!(
            ollama_msg
                .images
                .as_ref()
                .map(|v| v.is_empty())
                .unwrap_or(true)
        );
        assert!(ollama_msg.thinking.is_none());
    }

    #[test]
    fn test_llmmessage_to_chatmessage_with_tool_calls() {
        let llm_msg = LlmMessage::assistant("OK".to_string()).with_tool_calls(vec![LlmToolCall {
            id: "call_1".to_string(),
            name: "calculate".to_string(),
            arguments: json!({"expression": "2+2"}),
        }]);

        let ollama_msg: ChatMessage = llm_msg.try_into().unwrap();

        assert_eq!(ollama_msg.tool_calls.len(), 1);
        assert_eq!(ollama_msg.tool_calls[0].function.name, "calculate");
    }

    #[test]
    fn test_llmmessage_to_chatmessage_with_audio_fails() {
        let llm_msg = LlmMessage::user("Hi".to_string()).with_audio(vec!["audio_data".to_string()]);
        let result: Result<ChatMessage, ProviderError> = llm_msg.try_into();

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::Unsupported(_)));
    }

    // --- ToolCall roundtrip ---

    #[test]
    fn test_toolcall_roundtrip() {
        let ollama_call = OllamaToolCall {
            function: ToolCallFunction {
                name: "get_weather".to_string(),
                arguments: json!({"city": "Tokyo"}),
            },
        };

        let llm_call: LlmToolCall = ollama_call.clone().into();
        let back: OllamaToolCall = llm_call.into();

        assert_eq!(back.function.name, ollama_call.function.name);
        assert_eq!(back.function.arguments, ollama_call.function.arguments);
    }

    // --- ChatMessageResponse -> LlmResponse ---

    #[test]
    fn test_chatmessageresponse_to_llmresponse() {
        let ollama_resp = ChatMessageResponse {
            model: "llama3.1".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            message: {
                let mut msg = OllamaChatMessage::assistant("Response".to_string());
                msg.tool_calls = vec![OllamaToolCall {
                    function: ToolCallFunction {
                        name: "tool".to_string(),
                        arguments: json!({}),
                    },
                }];
                msg
            },
            logprobs: None,
            done: true,
            final_data: Some(ollama_rs::generation::chat::ChatMessageFinalResponseData {
                total_duration: 100,
                load_duration: 10,
                prompt_eval_count: 50,
                prompt_eval_duration: 5,
                eval_count: 30,
                eval_duration: 20,
            }),
        };

        let llm_resp: LlmResponse = ollama_resp.into();

        assert_eq!(llm_resp.model, "llama3.1");
        assert_eq!(llm_resp.content, "Response");
        assert_eq!(llm_resp.tool_calls.as_ref().map(|c| c.len()), Some(1));
        assert_eq!(llm_resp.eval_count, Some(30));
        assert_eq!(llm_resp.prompt_eval_count, Some(50));
    }

    // --- OllamaError -> ProviderError mapping ---

    #[test]
    fn test_ollamaerror_to_providererror_server() {
        let err = OllamaError::InternalError(InternalOllamaError {
            message: "500 Internal Server Error".to_string(),
        });
        let prov_err: ProviderError = err.into();

        assert!(matches!(prov_err, ProviderError::Api { status: 500, .. }));
        assert!(matches!(
            prov_err.retry_category(),
            RetryCategory::ServerRetry { max_attempts: 3 }
        ));
    }

    #[test]
    fn test_ollamaerror_to_providererror_timeout() {
        // Note: Can't easily construct reqwest::Error with specific kind in stable Rust
        // This test documents the intent - any reqwest timeout error should classify as NetworkRetry
        // Integration test: use `sprach query` with network timeout to verify
    }

    #[test]
    fn test_ollamaerror_to_providererror_connection() {
        // Note: Can't easily construct reqwest::Error with specific kind in stable Rust
        // This test documents the intent - any reqwest connection error should classify as NetworkRetry
        // Integration test: use `sprach query` with ollama down to verify
    }

    #[test]
    fn test_ollamaerror_to_providererror_tool_unknown() {
        let err = OllamaError::ToolCallError(ToolCallError::UnknownToolName);
        let prov_err: ProviderError = err.into();

        assert!(matches!(prov_err, ProviderError::Config(_)));
        assert!(matches!(prov_err.retry_category(), RetryCategory::NoRetry));
    }

    #[test]
    fn test_ollamaerror_to_providererror_tool_invalid_args() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = OllamaError::ToolCallError(ToolCallError::InvalidToolArguments(json_err));
        let prov_err: ProviderError = err.into();

        assert!(matches!(prov_err, ProviderError::Config(_)));
        assert!(matches!(prov_err.retry_category(), RetryCategory::NoRetry));
    }

    #[test]
    fn test_ollamaerror_to_providererror_tool_internal() {
        let err =
            OllamaError::ToolCallError(ToolCallError::InternalToolError("tool crashed".into()));
        let prov_err: ProviderError = err.into();

        assert!(matches!(prov_err, ProviderError::Other(_)));
        assert!(matches!(prov_err.retry_category(), RetryCategory::NoRetry));
    }

    #[test]
    fn test_ollamaerror_to_providererror_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = OllamaError::JsonError(json_err);
        let prov_err: ProviderError = err.into();

        assert!(matches!(prov_err, ProviderError::Config(_)));
        assert!(matches!(prov_err.retry_category(), RetryCategory::NoRetry));
    }

    // --- ProviderOptions -> ModelOptions ---

    #[test]
    fn test_provider_options_to_model_options() {
        let opts = ProviderOptions {
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            repeat_penalty: Some(1.1),
            num_predict: Some(2048),
            stop_sequences: Some(vec!["STOP".to_string()]),
            think: Some(true),
            format: Some("json".to_string()),
            audio_format: None,
        };

        let model_opts: ollama_rs::models::ModelOptions = opts.into();

        // Can't easily assert private fields, but conversion should not panic
        let _ = model_opts;
    }
}
