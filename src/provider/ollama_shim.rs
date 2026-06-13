//! Compatibility shim for `ollama_rs` types (W2 #121).
//!
//! The codebase used to depend on the third-party `ollama_rs` crate
//! directly. W2 #121 replaces the native `/api/chat` HTTP transport
//! with the OpenAI-compat `/v1/chat/completions` endpoint (via
//! `OpenAICompatibleProvider`).
//!
//! To minimize the migration surface in this PR, this module re-exports
//! the ollama_rs types and provides a `CompatOllama` struct that
//! delegates to `OpenAICompatibleProvider`. The `Ollama` type alias
//! (re-exported as `crate::provider::Ollama`) is what the rest of the
//! codebase uses.
//!
//! This shim is REMOVED in #123 (Remove ollama-rs).

#![allow(dead_code)]
#![allow(clippy::all)]

pub use ollama_rs::generation::chat::request::ChatMessageRequest;
pub use ollama_rs::generation::chat::{ChatMessage, ChatMessageResponse, MessageRole};
pub use ollama_rs::generation::completion::request::GenerationRequest as _GenerationRequest;
pub use ollama_rs::generation::images::Image as _Image;
pub use ollama_rs::models::ModelInfo;
pub use ollama_rs::models::ModelOptions as _ModelOptions;

use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;

use crate::provider::LlmProvider;
use crate::provider::openai_compat::{OpenAICompatibleConfig, OpenAICompatibleProvider};
use crate::provider::types::{LlmMessage, LlmRole, ProviderOptions};
use crate::user_models::ProviderConfig;

/// Re-export of `crate::provider::Ollama` — a shim that delegates to OpenAI-compatible transport.
pub type Ollama = CompatOllama;

/// A shim that mimics the `crate::provider::Ollama` API surface but talks to
/// Ollama via the OpenAI-compat endpoint (W2 #121).
#[derive(Clone)]
pub struct CompatOllama {
    base_url: String,
    inner: Arc<OpenAICompatibleProvider>,
}

impl CompatOllama {
    /// Create a new shim pointing to the default Ollama URL.
    /// W2 #121: base_url is normalized to include `/v1` suffix.
    pub fn new(host: impl Into<String>, _port: u16) -> Self {
        let base_url = format!("{}/v1", host.into().trim_end_matches('/'));
        let cfg = OpenAICompatibleConfig {
            base_url: base_url.clone(),
            api_key: None,
            connect_timeout_secs: 5,
            read_timeout_secs: 300,
            stream_idle_timeout_secs: 60,
            max_retries: 3,
            retry_base_delay_ms: 2000,
            retry_max_delay_ms: 16000,
            retry_jitter_percent: 20,
        };
        let inner = OpenAICompatibleProvider::new(cfg).unwrap_or_else(|e| {
            log::error!("Failed to create OpenAI provider (this is a config error; check base_url and timeouts): {e}");
            // Fall back to a provider pointing at localhost:11434/v1.
            // This will fail at request time if the server is unreachable,
            // but at least the struct is constructible.
            let fallback = OpenAICompatibleConfig {
                base_url: "http://localhost:11434/v1".to_string(),
                api_key: None,
                connect_timeout_secs: 5,
                read_timeout_secs: 300,
                stream_idle_timeout_secs: 60,
                max_retries: 0,
                retry_base_delay_ms: 1000,
                retry_max_delay_ms: 1000,
                retry_jitter_percent: 0,
            };
            match OpenAICompatibleProvider::new(fallback) {
                Ok(p) => p,
                Err(e2) => {
                    log::error!("Fallback config also failed: {e2}");
                    #[expect(clippy::panic, reason = "fallback URL is hardcoded and always valid")]
                    {
                        panic!("OpenAICompatibleProvider::new() failed on hardcoded fallback URL: {e2}");
                    }
                }
            }
        });
        Self {
            base_url,
            inner: Arc::new(inner),
        }
    }

    /// Create a shim from a `ProviderConfig` (preferred).
    pub fn from_provider_config(cfg: &ProviderConfig) -> Self {
        let mut base_url = cfg.base_url.clone();
        if !base_url.contains("/v1") && !base_url.ends_with('/') {
            base_url.push_str("/v1");
        }
        let openai_cfg = OpenAICompatibleConfig::from(cfg);
        let inner = OpenAICompatibleProvider::new(openai_cfg).unwrap_or_else(|e| {
            log::error!(
                "Failed to create OpenAI provider from config (check base_url syntax): {e}"
            );
            let fallback = OpenAICompatibleConfig {
                base_url: "http://localhost:11434/v1".to_string(),
                api_key: None,
                connect_timeout_secs: 5,
                read_timeout_secs: 300,
                stream_idle_timeout_secs: 60,
                max_retries: 0,
                retry_base_delay_ms: 1000,
                retry_max_delay_ms: 1000,
                retry_jitter_percent: 0,
            };
            match OpenAICompatibleProvider::new(fallback) {
                Ok(p) => p,
                Err(e2) => {
                    log::error!("Fallback config also failed: {e2}");
                    #[expect(clippy::panic, reason = "fallback URL is hardcoded and always valid")]
                    {
                        panic!(
                            "OpenAICompatibleProvider::new() failed on hardcoded fallback URL: {e2}"
                        );
                    }
                }
            }
        });
        Self {
            base_url,
            inner: Arc::new(inner),
        }
    }

    /// Get the inner `OpenAICompatibleProvider` for direct access.
    pub fn inner(&self) -> &OpenAICompatibleProvider {
        &self.inner
    }

    /// Probe the embedding endpoint (W2 #121).
    ///
    /// Delegates to [`OpenAICompatibleProvider::probe_embedding`].
    /// See that method for details.
    pub async fn probe_embedding(
        &self,
        model: &str,
    ) -> Result<(), crate::provider::types::ProviderError> {
        self.inner.probe_embedding(model).await
    }

    /// Get the base URL (normalized with /v1 suffix).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// List local models. W2 #121: hits `/v1/models` instead of `/api/tags`.
    pub async fn list_local_models(
        &self,
    ) -> Result<Vec<LocalModel>, ollama_rs::error::OllamaError> {
        use crate::provider::openai_types::ModelsResponse;
        let url = format!("{}/models", self.base_url);
        let response = self.inner.as_client().get(&url).send().await.map_err(|e| {
            ollama_rs::error::OllamaError::Other(format!("Failed to list models: {e}"))
        })?;
        if !response.status().is_success() {
            return Err(ollama_rs::error::OllamaError::Other(format!(
                "Failed to list models: HTTP {}",
                response.status().as_u16()
            )));
        }
        let models: ModelsResponse = response.json().await.map_err(|e| {
            ollama_rs::error::OllamaError::Other(format!("Failed to parse models response: {e}"))
        })?;
        Ok(models
            .data
            .into_iter()
            .map(|m| LocalModel {
                name: m.id,
                modified_at: String::new(),
                size: 0,
            })
            .collect())
    }

    /// Show model info.
    ///
    /// W2 #121: capabilities cannot be reliably inferred from OpenAI-compat
    /// `/v1/models` responses — the OpenAI spec does not expose
    /// `thinking` / `vision` / `tools` flags, and ad-hoc metadata fields
    /// (e.g. llama-swap's `meta.llamaswap.features`) are deployment-specific.
    ///
    /// Policy: assume a permissive default set so that capability-driven
    /// code paths (tool calling, embeddings, etc.) engage. The user is
    /// responsible for disabling capabilities they do not want in
    /// `models.toml` (e.g. `thinking = false`, `tools = false`). Errors
    /// from the model itself (refusal, tool-call failure, missing
    /// embedding endpoint) are surfaced to the caller as usual.
    pub async fn show_model_info(
        &self,
        _name: String,
    ) -> Result<ModelInfo, ollama_rs::error::OllamaError> {
        Ok(ModelInfo {
            license: String::new(),
            modelfile: String::new(),
            parameters: String::new(),
            template: String::new(),
            model_info: Default::default(),
            capabilities: vec![
                "completion".to_string(),
                "tools".to_string(),
                "thinking".to_string(),
            ],
        })
    }

    /// Send a chat completion request.
    pub async fn send_chat_messages(
        &self,
        request: ChatMessageRequest,
    ) -> ollama_rs::error::Result<ChatMessageResponse> {
        let model = request.model_name.clone();
        let messages = convert_ollama_messages_to_llm(request.messages.clone());
        // Extract tools from the request (W2 #121: same fix as
        // send_chat_messages_stream — previously hardcoded `vec![]`).
        let tools = convert_ollama_tools_to_tool_info(request.tools.clone());
        let options = ProviderOptions::default();

        let response = self
            .inner
            .chat(&model, messages, tools, options)
            .await
            .map_err(|e| ollama_rs::error::OllamaError::Other(e.to_string()))?;

        Ok(convert_llm_response_to_ollama(&model, response))
    }

    /// Send a streaming chat completion request.
    /// W2 #121: streams LlmStreamChunk from OpenAICompatibleProvider
    /// converted to ollama-rs ChatMessage per chunk.
    pub async fn send_chat_messages_stream(
        &self,
        request: ChatMessageRequest,
    ) -> ollama_rs::error::Result<
        Pin<Box<dyn Stream<Item = Result<ChatMessage, ollama_rs::error::OllamaError>> + Send>>,
    > {
        let model = request.model_name.clone();
        let messages = convert_ollama_messages_to_llm(request.messages.clone());
        // Extract tools from the request (W2 #121 bug fix: was hardcoded `vec![]`,
        // causing the OpenAI-compatible backend to never receive tool definitions
        // and thus never emit `delta.tool_calls`).
        let tools = convert_ollama_tools_to_tool_info(request.tools.clone());
        let options = ProviderOptions::default();

        let stream = self
            .inner
            .chat_stream(&model, messages, tools, options)
            .await
            .map_err(|e| ollama_rs::error::OllamaError::Other(e.to_string()))?;

        // Convert LlmStreamChunk → ChatMessage. The shim returns ChatMessage
        // (not ChatMessageResponseChunk) so the legacy coordinator's
        // `while let Some(chunk_result) = stream.next().await { match chunk_result { Ok(chunk) => ... chunk.content ... } }`
        // loop works unchanged. See custom_coordinator.rs:725-733.
        let mapped = async_stream::stream! {
            use futures::StreamExt;
            let mut stream = stream;
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        let content = chunk.content.clone().unwrap_or_default();
                        let tool_calls = chunk
                            .tool_calls
                            .clone()
                            .unwrap_or_default()
                            .into_iter()
                            .map(|c| ollama_rs::generation::tools::ToolCall {
                                function: ollama_rs::generation::tools::ToolCallFunction {
                                    name: c.name,
                                    arguments: c.arguments,
                                },
                            })
                            .collect();
                        yield Ok(ChatMessage {
                            role: MessageRole::Assistant,
                            content,
                            tool_calls,
                            images: None,
                            thinking: chunk.thinking,
                        });
                    }
                    Err(e) => yield Err(ollama_rs::error::OllamaError::Other(e.to_string())),
                }
            }
        };

        let pinned: Pin<
            Box<dyn Stream<Item = Result<ChatMessage, ollama_rs::error::OllamaError>> + Send>,
        > = Box::pin(mapped);
        Ok(pinned)
    }

    /// Generate a completion.
    pub async fn generate(
        &self,
        request: &ollama_rs::generation::completion::request::GenerationRequest<'_>,
    ) -> ollama_rs::error::Result<ollama_rs::generation::completion::GenerationResponse> {
        let model = request.model_name.clone();
        let prompt = request.prompt.to_string();
        let images: Vec<String> = request
            .images
            .iter()
            .map(|i| i.to_base64().to_string())
            .collect();
        let options = ProviderOptions::default();

        let content = self
            .inner
            .generate(&model, &prompt, images, vec![], options)
            .await
            .map_err(|e| ollama_rs::error::OllamaError::Other(e.to_string()))?;

        Ok(ollama_rs::generation::completion::GenerationResponse {
            model,
            created_at: chrono::Utc::now().to_rfc3339(),
            response: content,
            done: true,
            context: None,
            total_duration: None,
            load_duration: None,
            prompt_eval_count: None,
            prompt_eval_duration: None,
            eval_count: None,
            eval_duration: None,
            thinking: None,
            logprobs: None,
        })
    }

    /// Generate embeddings.
    pub async fn generate_embeddings(
        &self,
        request: ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest,
    ) -> ollama_rs::error::Result<ollama_rs::generation::embeddings::GenerateEmbeddingsResponse>
    {
        let model = request.model_name.clone();
        let text = match &request.input {
            ollama_rs::generation::embeddings::request::EmbeddingsInput::Single(s) => s.clone(),
            ollama_rs::generation::embeddings::request::EmbeddingsInput::Multiple(v) => v.join(" "),
        };

        let embedding = self
            .inner
            .embed(&text, &model, request.dimensions.map(|d| d as usize))
            .await
            .map_err(|e| ollama_rs::error::OllamaError::Other(e.to_string()))?;

        Ok(
            ollama_rs::generation::embeddings::GenerateEmbeddingsResponse {
                embeddings: vec![embedding],
            },
        )
    }
}

impl std::fmt::Debug for CompatOllama {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompatOllama")
            .field("base_url", &self.base_url)
            .finish()
    }
}

/// Stub type for `list_local_models()` return.
#[derive(Debug, Clone)]
pub struct LocalModel {
    pub name: String,
    pub modified_at: String,
    pub size: u64,
}

// === Helper conversion functions ===

/// Convert ollama-rs `ToolInfo` (from `ChatMessageRequest.tools`) into our
/// agnostic `LlmToolInfo` so the OpenAI-compatible backend receives the
/// tool definitions on every chat call.
///
/// W2 #121: previously this conversion was missing, so the shim dropped
/// all tools on the floor — the LLM never knew what tools were available
/// and never emitted `delta.tool_calls` in streaming responses.
fn convert_ollama_tools_to_tool_info(
    tools: Vec<ollama_rs::generation::tools::ToolInfo>,
) -> Vec<crate::provider::ToolInfo> {
    tools
        .into_iter()
        .filter_map(|t| {
            // Round-trip via JSON to map ollama-rs's ToolInfo into the
            // agnostic LlmToolInfo (structurally identical, different paths).
            let json = match serde_json::to_value(&t) {
                Ok(j) => j,
                Err(e) => {
                    log::warn!("convert_ollama_tools: failed to serialize tool: {e}");
                    return None;
                }
            };
            match serde_json::from_value::<crate::provider::ToolInfo>(json) {
                Ok(info) => Some(info),
                Err(e) => {
                    log::warn!("convert_ollama_tools: failed to deserialize tool: {e}");
                    None
                }
            }
        })
        .collect()
}

fn convert_ollama_messages_to_llm(messages: Vec<ChatMessage>) -> Vec<LlmMessage> {
    messages
        .into_iter()
        .map(|m| {
            let role = match m.role {
                MessageRole::User => LlmRole::User,
                MessageRole::Assistant => LlmRole::Assistant,
                MessageRole::System => LlmRole::System,
                MessageRole::Tool => LlmRole::Tool,
            };
            let tool_calls = if !m.tool_calls.is_empty() {
                Some(
                    m.tool_calls
                        .into_iter()
                        .map(|c| crate::provider::types::LlmToolCall {
                            id: String::new(),
                            name: c.function.name,
                            arguments: c.function.arguments,
                        })
                        .collect(),
                )
            } else {
                None
            };
            LlmMessage {
                role,
                content: m.content,
                tool_calls,
                images: m.images.map(|imgs| {
                    imgs.into_iter()
                        .map(|i| i.to_base64().to_string())
                        .collect()
                }),
                audio: None,
                thinking: m.thinking,
                name: None,
                tool_call_id: None,
            }
        })
        .collect()
}

fn convert_llm_response_to_ollama(
    model: &str,
    response: crate::provider::types::LlmResponse,
) -> ChatMessageResponse {
    let tool_calls = response
        .tool_calls
        .unwrap_or_default()
        .into_iter()
        .map(|c| ollama_rs::generation::tools::ToolCall {
            function: ollama_rs::generation::tools::ToolCallFunction {
                name: c.name,
                arguments: c.arguments,
            },
        })
        .collect();

    let message = ChatMessage {
        role: MessageRole::Assistant,
        content: response.content,
        tool_calls,
        images: None,
        thinking: None, // LlmResponse doesn't carry thinking (only LlmMessage does)
    };

    ChatMessageResponse {
        model: model.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        message,
        logprobs: None,
        done: true,
        final_data: response
            .prompt_eval_count
            .zip(response.eval_count)
            .map(
                |(p, c)| ollama_rs::generation::chat::ChatMessageFinalResponseData {
                    total_duration: 0,
                    load_duration: 0,
                    prompt_eval_count: p as u64,
                    prompt_eval_duration: 0,
                    eval_count: c as u64,
                    eval_duration: 0,
                },
            ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_new_normalizes_url() {
        let ollama = CompatOllama::new("http://localhost:11434", 11434);
        assert_eq!(ollama.base_url(), "http://localhost:11434/v1");
    }

    #[test]
    fn test_ollama_new_strips_trailing_slash() {
        let ollama = CompatOllama::new("http://localhost:11434/", 11434);
        assert_eq!(ollama.base_url(), "http://localhost:11434/v1");
    }

    #[test]
    fn test_convert_ollama_messages_basic() {
        let msgs = vec![ChatMessage {
            role: MessageRole::User,
            content: "Hello".to_string(),
            tool_calls: vec![],
            images: None,
            thinking: None,
        }];
        let llm = convert_ollama_messages_to_llm(msgs);
        assert_eq!(llm.len(), 1);
        assert_eq!(llm[0].role, LlmRole::User);
        assert_eq!(llm[0].content, "Hello");
    }
}
