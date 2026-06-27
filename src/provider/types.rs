//! Provider-agnostic types for the W2 Provider Chain.
//!
//! These types decouple the codebase from specific LLM provider APIs
//! (Ollama, OpenAI-compatible, etc.). They mirror the JSON shapes used
//! by LLM provider APIs while providing a unified surface for business logic.

#![allow(dead_code)] // W2 #123: retry_category, RetryCategory, LlmMessage methods will be consumed when the retry loop migrates from OllamaError to ProviderError

use schemars::Schema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Tool type discriminator (re-exported for convenience).
#[allow(dead_code)] // Consumed by #120
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub enum ToolType {
    Function,
}

/// Tool function info (name, description, JSON schema parameters).
#[allow(dead_code)] // Consumed by #120
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunctionInfo {
    pub name: String,
    pub description: String,
    pub parameters: Schema,
}

/// A tool's JSON schema info, generated from a `Tool` impl.
///
/// Mirrors the JSON shape used by LLM tool APIs:
/// ```json
/// {
///   "type": "function",
///   "function": {
///     "name": "...",
///     "description": "...",
///     "parameters": { ... JSON schema ... }
///   }
/// }
/// ```
#[allow(dead_code)] // Consumed by #120
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    #[serde(rename = "type")]
    pub tool_type: ToolType,
    pub function: ToolFunctionInfo,
}

impl ToolInfo {
    /// Create a new `ToolInfo` for the given `Tool` type.
    #[allow(dead_code)] // Consumed by #120
    pub fn new<P, T>() -> Self
    where
        P: serde::de::DeserializeOwned + schemars::JsonSchema,
        T: crate::tools::Tool<Params = P>,
    {
        use schemars::SchemaGenerator;
        use schemars::generate::SchemaSettings;

        let mut settings = SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator: SchemaGenerator = settings.into_generator();
        let parameters = generator.into_root_schema_for::<P>();

        Self {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: T::name().to_string(),
                description: T::description().to_string(),
                parameters,
            },
        }
    }
}

/// LLM message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmRole {
    User,
    Assistant,
    System,
    Tool,
}

/// A message in an LLM conversation.
///
/// Provider-agnostic equivalent of `ollama_rs::generation::chat::ChatMessage`.
/// W2 #121: extended with `name` and `tool_call_id` for OpenAI tool support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: LlmRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<LlmToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>, // base64-encoded
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<Vec<String>>, // base64-encoded (mp3, wav, ogg)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    /// Name of the speaker (used for multi-user/tool-name annotations).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// For tool messages: the id of the tool call this is responding to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[cfg(test)]
impl LlmMessage {
    pub fn user(content: String) -> Self {
        Self {
            role: LlmRole::User,
            content,
            tool_calls: None,
            images: None,
            audio: None,
            thinking: None,
            name: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: String) -> Self {
        Self {
            role: LlmRole::Assistant,
            content,
            tool_calls: None,
            images: None,
            audio: None,
            thinking: None,
            name: None,
            tool_call_id: None,
        }
    }

    pub fn system(content: String) -> Self {
        Self {
            role: LlmRole::System,
            content,
            tool_calls: None,
            images: None,
            audio: None,
            thinking: None,
            name: None,
            tool_call_id: None,
        }
    }

    pub fn tool(content: String) -> Self {
        Self {
            role: LlmRole::Tool,
            content,
            tool_calls: None,
            images: None,
            audio: None,
            thinking: None,
            name: None,
            tool_call_id: None,
        }
    }

    /// Tool result message with the id of the tool call being responded to.
    pub fn tool_result(content: String, tool_call_id: String) -> Self {
        Self {
            role: LlmRole::Tool,
            content,
            tool_calls: None,
            images: None,
            audio: None,
            thinking: None,
            name: None,
            tool_call_id: Some(tool_call_id),
        }
    }

    pub fn with_tool_calls(mut self, calls: Vec<LlmToolCall>) -> Self {
        self.tool_calls = Some(calls);
        self
    }

    pub fn with_images(mut self, images: Vec<String>) -> Self {
        self.images = Some(images);
        self
    }

    pub fn with_audio(mut self, audio: Vec<String>) -> Self {
        self.audio = Some(audio);
        self
    }

    pub fn with_thinking(mut self, thinking: String) -> Self {
        self.thinking = Some(thinking);
        self
    }
}

/// A tool call from the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Response from an LLM chat completion.
///
/// Provider-agnostic equivalent of `ollama_rs::generation::chat::ChatMessageResponse`.
#[allow(dead_code)] // Consumed by #120/#121
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub model: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<LlmToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eval_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<u32>,
}

/// Token usage reported in a streaming or non-streaming response.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Provider-agnostic event emitted by a streaming LLM chat completion.
///
/// W2 #122: replaces the pull-based `LlmStreamChunk` model. The provider
/// pushes semantic lifecycle events (text/thinking/tool-call deltas, retry
/// status, completion) instead of forcing the consumer to diff successive
/// chunks. This matches the event-stream design used by the Pi Coding Agent.
#[derive(Debug, Clone)]
pub enum LlmStreamEvent {
    /// A new text content block has started.
    TextStart,
    /// Incremental text token.
    TextDelta { delta: String },

    /// A new thinking/reasoning block has started.
    ThinkingStart {
        #[allow(dead_code)] // Reasoning signature — used when provider implements signed thinking
        signature: Option<String>,
    },
    /// Incremental thinking token.
    ThinkingDelta { delta: String },

    /// A new tool call has started (name may still be `None`).
    ToolCallStart {
        index: u32,
        id: Option<String>,
        name: Option<String>,
    },
    /// Partial update to a tool call (name and/or arguments delta).
    ToolCallDelta {
        index: u32,
        id: Option<String>,
        name_delta: Option<String>,
        argument_delta: String,
    },
    /// Tool call finalized with parsed arguments.
    ToolCallEnd {
        #[allow(dead_code)] // Debug correlation — matches ToolCallStart.index
        index: u32,
        call: LlmToolCall,
    },

    /// Provider is about to retry a failed HTTP request.
    ProviderRetryStarted {
        attempt: u32,
        max_attempts: u32,
        delay_ms: u64,
        reason: String,
    },
    /// Provider retry finished (success or exhausted).
    ProviderRetryFinished { success: bool, attempt: u32 },

    /// Stream completed normally.
    Done {
        #[allow(dead_code)] // Finish reason — used when provider emits it
        reason: Option<String>,
        usage: Option<LlmUsage>,
    },
}

/// Capabilities reported by a model/provider.
///
/// Based on llama-swap feature flags for OpenAI-compatible backends.
#[allow(dead_code)] // Consumed by #120
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub completion: bool,
    pub tools: bool,
    pub thinking: bool,
    pub vision: bool,
    pub embedding: bool,
    pub insert: bool,
    pub audio: bool,
    pub image: bool,
    pub provider: String,
    pub model: String,
}

/// Options for provider requests.
///
/// Mirrors `ollama_rs::models::ModelOptions` with additions.
/// W2 #121: removed `top_k`, `repeat_penalty`, `think` (not OpenAI-portable).
/// Added `seed` (cross-provider, optional).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderOptions {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub repeat_penalty: Option<f32>,
    pub num_predict: Option<i32>,
    pub stop_sequences: Option<Vec<String>>,
    pub think: Option<bool>,
    pub format: Option<String>,
    pub audio_format: Option<String>,
    /// Optional seed for reproducible outputs (cross-provider).
    pub seed: Option<u32>,
}

/// Provider error with retry classification semantics.
///
/// Each variant maps to a `RetryCategory` for the W2 retry infrastructure (#116).
#[derive(Debug, thiserror::Error, Clone)]
pub enum ProviderError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Rate limited (HTTP 429): {message}")]
    RateLimit {
        message: String,
        // retry_after is not serialized; used internally for retry logic
        #[allow(dead_code)] // Set in #122 via Retry-After header parsing
        retry_after: Option<Duration>,
    },

    #[error("API error (HTTP {status}): {body}")]
    Api { status: u16, body: String },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Unsupported feature: {0}")]
    Unsupported(String),

    #[error("{0}")]
    Other(String),
}

impl ProviderError {
    /// Classify this error into a `RetryCategory` for the retry infrastructure.
    pub fn retry_category(&self) -> RetryCategory {
        match self {
            ProviderError::Api { status, .. } if *status >= 500 => {
                RetryCategory::ServerRetry { max_attempts: 3 }
            }
            ProviderError::Timeout(_) => RetryCategory::NetworkRetry { max_attempts: 5 },
            ProviderError::Connection(_) => RetryCategory::NetworkRetry { max_attempts: 5 },
            ProviderError::RateLimit { retry_after, .. } => RetryCategory::RateLimitRetry {
                max_attempts: 3,
                retry_after: *retry_after,
            },
            ProviderError::Config(_) => RetryCategory::NoRetry,
            ProviderError::Unsupported(_) => RetryCategory::NoRetry,
            ProviderError::Other(_) => RetryCategory::NoRetry,
            // Api with status < 500 falls through to NoRetry
            ProviderError::Api { .. } => RetryCategory::NoRetry,
        }
    }
}

/// Retry strategy for a classified error.
///
/// Relocated from `src/retry.rs` per W2 Provider Chain plan.
/// Each variant carries its own `max_attempts` so per-category limits
/// are visible in the type signature.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum RetryCategory {
    /// Tool execution failures — retry immediately, no delay
    ImmediateRetry { max_attempts: usize },
    /// Network/timeout errors — exponential backoff
    NetworkRetry { max_attempts: usize },
    /// HTTP 500, OOM, cold start — long linear backoff
    ServerRetry { max_attempts: usize },
    /// Rate limiting (HTTP 429) — respects `Retry-After` header.
    /// `retry_after` is `None` until #122 wires up header parsing.
    RateLimitRetry {
        max_attempts: usize,
        #[allow(dead_code)] // Used in #122 Retry-After header parsing
        retry_after: Option<Duration>,
    },
    /// Non-recoverable errors (cancel, context overflow, malformed JSON)
    NoRetry,
}

impl RetryCategory {
    pub fn max_attempts(&self) -> usize {
        match self {
            RetryCategory::ImmediateRetry { max_attempts } => *max_attempts,
            RetryCategory::NetworkRetry { max_attempts } => *max_attempts,
            RetryCategory::ServerRetry { max_attempts } => *max_attempts,
            RetryCategory::RateLimitRetry { max_attempts, .. } => *max_attempts,
            RetryCategory::NoRetry => 0,
        }
    }

    pub fn is_retryable(&self) -> bool {
        !matches!(self, RetryCategory::NoRetry)
    }
}

/// Calculate the backoff delay for a given retry category and attempt number.
///
/// `attempt` is 1-indexed: the first retry is attempt 1.
#[allow(dead_code)] // Consumed by #120/#121
pub fn retry_delay(category: &RetryCategory, attempt: usize) -> Duration {
    match category {
        RetryCategory::ImmediateRetry { .. } => Duration::ZERO,
        RetryCategory::NetworkRetry { .. } => {
            // Exponential: 100ms, 200ms, 400ms, 800ms, 1.6s, capped at 1.6s
            const BASE_MS: u64 = 100;
            const MAX_MS: u64 = 1600;
            let shift = attempt.saturating_sub(1).min(4) as u32;
            let ms = BASE_MS.saturating_mul(1u64 << shift);
            Duration::from_millis(ms.min(MAX_MS))
        }
        RetryCategory::ServerRetry { .. } => {
            // Linear: 5s × attempt → 5s, 10s, 15s
            const BASE_SECS: u64 = 5;
            Duration::from_secs(BASE_SECS * attempt as u64)
        }
        RetryCategory::RateLimitRetry { retry_after, .. } => {
            // Use Retry-After header value if present, else 2s default
            const DEFAULT_SECS: u64 = 2;
            retry_after.unwrap_or(Duration::from_secs(DEFAULT_SECS))
        }
        RetryCategory::NoRetry => Duration::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_error_retry_category_server() {
        let err = ProviderError::Api {
            status: 500,
            body: "Internal Server Error".to_string(),
        };
        assert!(matches!(
            err.retry_category(),
            RetryCategory::ServerRetry { max_attempts: 3 }
        ));

        let err = ProviderError::Api {
            status: 503,
            body: "Service Unavailable".to_string(),
        };
        assert!(matches!(
            err.retry_category(),
            RetryCategory::ServerRetry { max_attempts: 3 }
        ));
    }

    #[test]
    fn test_provider_error_retry_category_client() {
        let err = ProviderError::Api {
            status: 400,
            body: "Bad Request".to_string(),
        };
        assert!(matches!(err.retry_category(), RetryCategory::NoRetry));

        let err = ProviderError::Api {
            status: 401,
            body: "Unauthorized".to_string(),
        };
        assert!(matches!(err.retry_category(), RetryCategory::NoRetry));
    }

    #[test]
    fn test_provider_error_retry_category_timeout() {
        let err = ProviderError::Timeout("request timeout".to_string());
        assert!(matches!(
            err.retry_category(),
            RetryCategory::NetworkRetry { max_attempts: 5 }
        ));
    }

    #[test]
    fn test_provider_error_retry_category_connection() {
        let err = ProviderError::Connection("connection refused".to_string());
        assert!(matches!(
            err.retry_category(),
            RetryCategory::NetworkRetry { max_attempts: 5 }
        ));
    }

    #[test]
    fn test_provider_error_retry_category_ratelimit() {
        let err = ProviderError::RateLimit {
            message: "rate limited".to_string(),
            retry_after: Some(Duration::from_secs(5)),
        };
        let cat = err.retry_category();
        assert!(matches!(
            cat,
            RetryCategory::RateLimitRetry {
                max_attempts: 3,
                ..
            }
        ));
        if let RetryCategory::RateLimitRetry { retry_after, .. } = cat {
            assert_eq!(retry_after, Some(Duration::from_secs(5)));
        }

        let err = ProviderError::RateLimit {
            message: "rate limited".to_string(),
            retry_after: None,
        };
        let cat = err.retry_category();
        assert!(matches!(
            cat,
            RetryCategory::RateLimitRetry {
                max_attempts: 3,
                ..
            }
        ));
        if let RetryCategory::RateLimitRetry { retry_after, .. } = cat {
            assert_eq!(retry_after, None);
        }
    }

    #[test]
    fn test_provider_error_retry_category_config() {
        let err = ProviderError::Config("invalid api key".to_string());
        assert!(matches!(err.retry_category(), RetryCategory::NoRetry));
    }

    #[test]
    fn test_provider_error_retry_category_unsupported() {
        let err = ProviderError::Unsupported("streaming not implemented".to_string());
        assert!(matches!(err.retry_category(), RetryCategory::NoRetry));
    }

    // --- retry_delay ---

    #[test]
    fn test_retry_delay_server_linear() {
        let cat = RetryCategory::ServerRetry { max_attempts: 3 };
        assert_eq!(retry_delay(&cat, 1), Duration::from_secs(5));
        assert_eq!(retry_delay(&cat, 2), Duration::from_secs(10));
        assert_eq!(retry_delay(&cat, 3), Duration::from_secs(15));
    }

    #[test]
    fn test_retry_delay_network_exponential() {
        let cat = RetryCategory::NetworkRetry { max_attempts: 5 };
        assert_eq!(retry_delay(&cat, 1), Duration::from_millis(100));
        assert_eq!(retry_delay(&cat, 2), Duration::from_millis(200));
        assert_eq!(retry_delay(&cat, 3), Duration::from_millis(400));
        assert_eq!(retry_delay(&cat, 4), Duration::from_millis(800));
        assert_eq!(retry_delay(&cat, 5), Duration::from_millis(1600));
    }

    #[test]
    fn test_retry_delay_immediate_zero() {
        let cat = RetryCategory::ImmediateRetry { max_attempts: 3 };
        assert_eq!(retry_delay(&cat, 1), Duration::ZERO);
        assert_eq!(retry_delay(&cat, 2), Duration::ZERO);
    }

    #[test]
    fn test_retry_delay_ratelimit_with_retry_after() {
        let cat = RetryCategory::RateLimitRetry {
            max_attempts: 3,
            retry_after: Some(Duration::from_secs(3)),
        };
        assert_eq!(retry_delay(&cat, 1), Duration::from_secs(3));
    }

    #[test]
    fn test_retry_delay_ratelimit_without_retry_after() {
        let cat = RetryCategory::RateLimitRetry {
            max_attempts: 3,
            retry_after: None,
        };
        assert_eq!(retry_delay(&cat, 1), Duration::from_secs(2));
    }
}
