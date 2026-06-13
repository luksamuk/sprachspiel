//! OpenAI-compatible provider.
//!
//! Implements [`LlmProvider`] for the OpenAI HTTP API. Works with:
//! - OpenAI
//! - Ollama's `/v1/chat/completions` endpoint
//! - llama.cpp (via llama-swap)
//! - vLLM (via llama-swap)
//! - LM Studio
//! - Any other OpenAI-spec server
//!
//! # Streaming
//!
//! OpenAI streaming uses Server-Sent Events (SSE) with the format:
//! ```text
//! data: {"choices": [{"delta": {"content": "token"}}]}
//!
//! data: [DONE]
//! ```
//!
//! # Tool calling
//!
//! Tools are defined in `tools: Vec<OpenAITool>` and the assistant
//! returns `tool_calls: Vec<OpenAIToolCall>` where each call has
//! `function.arguments` as a JSON-encoded string. The arguments are
//! accumulated across chunks (OpenAI delivers them incrementally —
//! unlike Ollama native which delivers complete arguments in a single chunk).
//!
//! # Retry-After
//!
//! HTTP 429 responses are parsed for the `Retry-After` header and the
//! delay is populated into `ProviderError::RateLimit::retry_after`.
//! W2 #121 item B4: this wires up the previously-unused
//! `retry_after: Option<Duration>` field on `RateLimit`.

#![allow(dead_code)] // Many methods used by shim that will be wired in P6.0e.4

use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;
use futures::Stream;
use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};

use super::openai_types::{
    ChatChunk, ChatChoice, ChatRequest, ChatResponse, EmbeddingsRequest, EmbeddingsResponse,
    ModelsResponse, OpenAIMessage, OpenAITool, OpenAIToolCall, OpenAIToolCallFunction,
    OpenAIToolFunction, StreamOptions, Usage as OpenAIUsage,
};

/// Tuple of OpenAI request fields derived from `ProviderOptions`.
type ConvertedOptions = (
    Option<f32>,         // temperature
    Option<f32>,         // top_p
    Option<u32>,         // max_tokens (from num_predict)
    Option<Vec<String>>, // stop_sequences
    Option<u32>,         // seed
);
use super::types::{
    LlmMessage, LlmResponse, LlmRole, LlmStreamChunk, LlmToolCall, ProviderCapabilities,
    ProviderError, ProviderOptions, ToolInfo, ToolType,
};
use crate::provider::LlmProvider;
use crate::user_models::ProviderConfig;

/// Configuration for the OpenAI-compatible provider.
#[derive(Debug, Clone)]
pub struct OpenAICompatibleConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub connect_timeout_secs: u64,
    pub read_timeout_secs: u64,
    pub stream_idle_timeout_secs: u64,
    pub max_retries: u32,
    pub retry_base_delay_ms: u64,
    pub retry_max_delay_ms: u64,
    pub retry_jitter_percent: u8,
}

impl From<&ProviderConfig> for OpenAICompatibleConfig {
    fn from(cfg: &ProviderConfig) -> Self {
        Self {
            base_url: cfg.base_url.clone(),
            api_key: cfg
                .api_key_env
                .as_ref()
                .and_then(|env_var| std::env::var(env_var).ok()),
            connect_timeout_secs: cfg.connect_timeout_secs,
            read_timeout_secs: cfg.read_timeout_secs,
            stream_idle_timeout_secs: cfg.stream_idle_timeout_secs,
            max_retries: cfg.max_retries,
            retry_base_delay_ms: cfg.retry_base_delay_ms,
            retry_max_delay_ms: cfg.retry_max_delay_ms,
            retry_jitter_percent: cfg.retry_jitter_percent,
        }
    }
}

/// OpenAI-compatible provider implementation.
pub struct OpenAICompatibleProvider {
    config: OpenAICompatibleConfig,
    client: reqwest::Client,
    api_key: Option<String>,
}

impl OpenAICompatibleProvider {
    /// Access the underlying reqwest client (used by the ollama_rs shim).
    pub fn as_client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Create a new `OpenAICompatibleProvider`.
    pub fn new(config: OpenAICompatibleConfig) -> Result<Self, ProviderError> {
        let api_key = config.api_key.clone().or_else(|| {
            std::env::var("OPENAI_API_KEY").ok()
        });

        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(config.connect_timeout_secs))
            .read_timeout(Duration::from_secs(config.read_timeout_secs))
            .build()
            .map_err(|e| ProviderError::Config(format!("Failed to build HTTP client: {e}")))?;

        Ok(Self {
            config,
            client,
            api_key,
        })
    }

    /// Build the request URL for a given endpoint.
    fn url(&self, endpoint: &str) -> String {
        let base = self.config.base_url.trim_end_matches('/');
        if endpoint.starts_with('/') {
            format!("{base}{endpoint}")
        } else {
            format!("{base}/{endpoint}")
        }
    }

    /// Build request headers (including auth if API key is set).
    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(ref key) = self.api_key
            && let Ok(value) = HeaderValue::from_str(&format!("Bearer {key}"))
        {
            headers.insert(reqwest::header::AUTHORIZATION, value);
        }
        headers
    }

    /// Convert internal `LlmMessage` list to OpenAI-spec message format.
    fn convert_messages(messages: Vec<LlmMessage>) -> Vec<serde_json::Value> {
        messages
            .into_iter()
            .map(|m| {
                let role = match m.role {
                    LlmRole::User => "user",
                    LlmRole::Assistant => "assistant",
                    LlmRole::System => "system",
                    LlmRole::Tool => "tool",
                };

                // Build the message object. Use Value to allow flexible
                // fields (tool_calls, tool_call_id, name) per role.
                let mut obj = serde_json::json!({
                    "role": role,
                });

                if role == "tool" {
                    // Tool messages in OpenAI use `content` as the tool result.
                    obj["content"] = serde_json::Value::String(m.content.clone());
                } else if let Some(images) = m.images.clone() {
                    if !images.is_empty() {
                        // Vision support: include images in user messages.
                        let mut parts = vec![serde_json::json!({
                            "type": "text",
                            "text": m.content.clone(),
                        })];
                        for img in images {
                            parts.push(serde_json::json!({
                                "type": "image_url",
                                "image_url": {
                                    "url": format!("data:image/png;base64,{img}"),
                                },
                            }));
                        }
                        obj["content"] = serde_json::Value::Array(parts);
                    } else {
                        obj["content"] = serde_json::Value::String(m.content.clone());
                    }
                } else {
                    obj["content"] = serde_json::Value::String(m.content.clone());
                }

                if let Some(name) = m.name {
                    obj["name"] = serde_json::Value::String(name);
                }

                if let Some(tool_calls) = m.tool_calls {
                    let openai_calls: Vec<serde_json::Value> = tool_calls
                        .into_iter()
                        .map(|c| {
                            serde_json::json!({
                                "id": c.id,
                                "type": "function",
                                "function": {
                                    "name": c.name,
                                    "arguments": c.arguments.to_string(),
                                }
                            })
                        })
                        .collect();
                    obj["tool_calls"] = serde_json::Value::Array(openai_calls);
                }

                if let Some(tool_call_id) = m.tool_call_id {
                    obj["tool_call_id"] = serde_json::Value::String(tool_call_id);
                }

                obj
            })
            .collect()
    }

    /// Convert `ProviderOptions` to OpenAI request fields.
    #[allow(clippy::type_complexity)]
    fn convert_options(options: &ProviderOptions) -> ConvertedOptions {
        (
            options.temperature,
            options.top_p,
            options.num_predict.map(|n| n.max(0) as u32),
            options.stop_sequences.clone(),
            options.seed,
        )
    }

    /// Convert `ToolInfo` (agnostic) to OpenAI `OpenAITool` (spec).
    fn convert_tools(tools: Vec<ToolInfo>) -> Vec<OpenAITool> {
        tools
            .into_iter()
            .map(|t| OpenAITool {
                tool_type: match t.tool_type {
                    ToolType::Function => "function".to_string(),
                },
                function: OpenAIToolFunction {
                    name: t.function.name,
                    description: t.function.description,
                    parameters: serde_json::to_value(&t.function.parameters)
                        .unwrap_or_else(|e| {
                            log::warn!("Failed to serialize tool parameters: {e}");
                            serde_json::json!({})
                        }),
                },
            })
            .collect()
    }

    /// Convert OpenAI response to internal `LlmResponse`.
    fn convert_response(response: ChatResponse) -> LlmResponse {
        let choice = response.choices.into_iter().next();
        let (content, tool_calls, done_reason) = match choice {
            Some(ChatChoice {
                message, finish_reason, ..
            }) => {
                let tool_calls = message.tool_calls.map(|calls| {
                    calls
                        .into_iter()
                        .map(|c| LlmToolCall {
                            id: c.id,
                            name: c.function.name,
                            arguments: serde_json::from_str(&c.function.arguments)
                                .unwrap_or_else(|e| {
                                    log::warn!(
                                        "Failed to parse tool arguments JSON: {e}. Using raw string."
                                    );
                                    serde_json::Value::String(c.function.arguments)
                                }),
                        })
                        .collect()
                });
                (message.content.unwrap_or_default(), tool_calls, finish_reason)
            }
            None => (String::new(), None, None),
        };

        LlmResponse {
            model: response.model,
            content,
            tool_calls,
            done_reason,
            eval_count: response.usage.as_ref().map(|u| u.completion_tokens),
            prompt_eval_count: response.usage.as_ref().map(|u| u.prompt_tokens),
        }
    }

    /// Classify an HTTP response into a `ProviderError`.
    #[allow(dead_code)]
    fn classify_response(&self, response: reqwest::Response) -> ProviderError {
        let status = response.status();
        let headers = response.headers().clone();
        // We can't easily get the body without consuming the response,
        // so this helper is for status-only classification.
        if status.as_u16() == 429 {
            // Parse Retry-After header
            let retry_after = headers
                .get(RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(parse_retry_after);
            ProviderError::RateLimit {
                message: "HTTP 429 Too Many Requests".to_string(),
                retry_after,
            }
        } else {
            ProviderError::Api {
                status: status.as_u16(),
                body: format!("HTTP {}", status.as_u16()),
            }
        }
    }

    /// Backoff delay for an attempt (with jitter).
    fn backoff_delay(&self, attempt: u32) -> Duration {
        if self.config.retry_base_delay_ms == 0 {
            return Duration::from_millis(0);
        }
        let shift = attempt.saturating_sub(1).min(8);
        let base_ms = self
            .config
            .retry_base_delay_ms
            .saturating_mul(1u64 << shift);
        let capped_ms = base_ms.min(self.config.retry_max_delay_ms);
        let jitter_range = if self.config.retry_jitter_percent == 0 {
            0
        } else {
            (capped_ms as f64 * (self.config.retry_jitter_percent as f64 / 100.0)) as u64
        };
        let jitter = if jitter_range > 0 {
            rand::random::<u64>() % (jitter_range + 1)
        } else {
            0
        };
        Duration::from_millis(capped_ms.saturating_add(jitter))
    }

    /// Send a chat completion (non-streaming) with retry.
    async fn chat_with_retry(
        &self,
        _model: &str,
        request: ChatRequest,
    ) -> Result<LlmResponse, ProviderError> {
        let url = self.url("/chat/completions");
        let mut last_error: Option<ProviderError> = None;

        for attempt in 1..=self.config.max_retries.max(1) {
            let response = self
                .client
                .post(&url)
                .headers(self.headers())
                .json(&request)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let chat_resp: ChatResponse = resp.json().await.map_err(|e| {
                            ProviderError::Other(format!("Failed to parse response: {e}"))
                        })?;
                        return Ok(Self::convert_response(chat_resp));
                    } else if status.as_u16() == 429 {
                        let retry_after = resp
                            .headers()
                            .get(RETRY_AFTER)
                            .and_then(|v| v.to_str().ok())
                            .and_then(parse_retry_after);
                        last_error = Some(ProviderError::RateLimit {
                            message: format!("HTTP 429 (attempt {attempt})"),
                            retry_after,
                        });
                        if attempt < self.config.max_retries.max(1) {
                            let delay = retry_after.unwrap_or(self.backoff_delay(attempt));
                            tokio::time::sleep(delay).await;
                        }
                    } else if status.is_server_error() {
                        last_error = Some(ProviderError::Api {
                            status: status.as_u16(),
                            body: format!("HTTP {} (attempt {})", status.as_u16(), attempt),
                        });
                        if attempt < self.config.max_retries.max(1) {
                            tokio::time::sleep(self.backoff_delay(attempt)).await;
                        }
                    } else {
                        let body = resp.text().await.unwrap_or_default();
                        return Err(ProviderError::Api {
                            status: status.as_u16(),
                            body,
                        });
                    }
                }
                Err(e) => {
                    last_error = Some(classify_reqwest_error(e));
                    if attempt < self.config.max_retries.max(1) {
                        tokio::time::sleep(self.backoff_delay(attempt)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| ProviderError::Other("Unknown error".to_string())))
    }
}

#[async_trait]
impl LlmProvider for OpenAICompatibleProvider {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<LlmMessage>,
        tools: Vec<ToolInfo>,
        options: ProviderOptions,
    ) -> Result<LlmResponse, ProviderError> {
        let (temperature, top_p, max_tokens, stop, seed) = Self::convert_options(&options);
        let request = ChatRequest {
            model: model.to_string(),
            messages: Self::convert_messages(messages),
            temperature,
            top_p,
            max_tokens,
            stop,
            seed,
            tools: if tools.is_empty() {
                None
            } else {
                Some(Self::convert_tools(tools))
            },
            stream: false,
            stream_options: None,
        };

        self.chat_with_retry(model, request).await
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<LlmMessage>,
        tools: Vec<ToolInfo>,
        options: ProviderOptions,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<LlmStreamChunk, ProviderError>> + Send>>,
        ProviderError,
    > {
        let (temperature, top_p, max_tokens, stop, seed) = Self::convert_options(&options);
        let request = ChatRequest {
            model: model.to_string(),
            messages: Self::convert_messages(messages),
            temperature,
            top_p,
            max_tokens,
            stop,
            seed,
            tools: if tools.is_empty() {
                None
            } else {
                Some(Self::convert_tools(tools))
            },
            stream: true,
            stream_options: Some(StreamOptions { include_usage: true }),
        };

        let url = self.url("/chat/completions");
        let response = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&request)
            .send()
            .await
            .map_err(classify_reqwest_error)?;

        let status = response.status();
        if !status.is_success() {
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(parse_retry_after);
            let body = response.text().await.unwrap_or_default();
            if status.as_u16() == 429 {
                return Err(ProviderError::RateLimit {
                    message: body,
                    retry_after,
                });
            }
            return Err(ProviderError::Api {
                status: status.as_u16(),
                body,
            });
        }

        let idle_timeout = Duration::from_secs(self.config.stream_idle_timeout_secs);
        let stream = parse_sse_stream(response, idle_timeout);

        Ok(Box::pin(stream))
    }

    async fn generate(
        &self,
        model: &str,
        prompt: &str,
        images: Vec<String>,
        _audio: Vec<String>,
        options: ProviderOptions,
    ) -> Result<String, ProviderError> {
        // For OpenAI, /v1/chat/completions supports images as content
        // parts in a user message. We emulate the /v1/generate (legacy)
        // path via chat.
        let (temperature, top_p, max_tokens, stop, seed) = Self::convert_options(&options);
        let user_msg = LlmMessage {
            role: LlmRole::User,
            content: prompt.to_string(),
            tool_calls: None,
            images: if images.is_empty() { None } else { Some(images) },
            audio: None,
            thinking: None,
            name: None,
            tool_call_id: None,
        };

        let request = ChatRequest {
            model: model.to_string(),
            messages: Self::convert_messages(vec![user_msg]),
            temperature,
            top_p,
            max_tokens,
            stop,
            seed,
            tools: None,
            stream: false,
            stream_options: None,
        };

        let response = self.chat_with_retry(model, request).await?;
        Ok(response.content)
    }

    async fn embed(
        &self,
        text: &str,
        model: &str,
        dimensions: Option<usize>,
    ) -> Result<Vec<f32>, ProviderError> {
        let url = self.url("/embeddings");
        let request = EmbeddingsRequest {
            model: model.to_string(),
            input: text.to_string(),
            dimensions: dimensions.map(|d| d as u32),
            encoding_format: "float".to_string(),
        };

        let response = self
            .client
            .post(&url)
            .headers(self.headers())
            .json(&request)
            .send()
            .await
            .map_err(classify_reqwest_error)?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api {
                status: status.as_u16(),
                body,
            });
        }

        let emb_resp: EmbeddingsResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Other(format!("Failed to parse embeddings response: {e}")))?;

        emb_resp
            .data
            .into_iter()
            .next()
            .map(|e| e.embedding)
            .ok_or_else(|| ProviderError::Other("Empty embeddings response".to_string()))
    }

    async fn detect_capabilities(
        &self,
        model: &str,
    ) -> Result<ProviderCapabilities, ProviderError> {
        let url = self.url("/models");
        let response = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await
            .map_err(classify_reqwest_error)?;

        let status = response.status();
        if !status.is_success() {
            // Fallback: try /api/show (Ollama native, served at same base
            // without /v1 prefix). This is best-effort.
            return Ok(ProviderCapabilities {
                completion: true,
                provider: "openai-compatible".to_string(),
                model: model.to_string(),
                ..Default::default()
            });
        }

        let models_resp: ModelsResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::Other(format!("Failed to parse models response: {e}")))?;

        // Find the requested model
        let model_info = models_resp
            .data
            .iter()
            .find(|m| m.id == model || m.id.starts_with(model))
            .or_else(|| models_resp.data.first());

        Ok(ProviderCapabilities {
            completion: true,
            tools: true, // OpenAI-spec always supports tools via the API
            thinking: false, // OpenAI doesn't expose "thinking" capability separately
            vision: true, // Most OpenAI-compat servers support vision via image_url
            embedding: true, // /v1/embeddings is standard
            insert: false,
            audio: false,
            image: true,
            provider: "openai-compatible".to_string(),
            model: model_info.map(|m| m.id.clone()).unwrap_or_else(|| model.to_string()),
        })
    }

    fn provider_name(&self) -> &str {
        "openai-compatible"
    }

    async fn is_available(&self) -> bool {
        let url = self.url("/models");
        match self
            .client
            .get(&url)
            .headers(self.headers())
            .timeout(Duration::from_secs(3))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }
}

/// Parse the `Retry-After` header value into a `Duration`.
///
/// Supports both formats:
/// - Seconds: `Retry-After: 120` → 120 seconds
/// - HTTP date: `Retry-After: Wed, 21 Oct 2015 07:28:00 GMT` (less common)
fn parse_retry_after(value: &str) -> Option<Duration> {
    // Try parsing as integer seconds first
    if let Ok(secs) = value.trim().parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    // Try parsing as HTTP date (RFC 7231)
    if let Ok(date) = chrono::DateTime::parse_from_rfc2822(value) {
        let now = chrono::Utc::now();
        if let Ok(delta) = (date.with_timezone(&chrono::Utc) - now).to_std() {
            return Some(delta);
        }
    }
    None
}

/// Classify a reqwest error into a `ProviderError`.
fn classify_reqwest_error(err: reqwest::Error) -> ProviderError {
    if err.is_timeout() {
        ProviderError::Timeout(err.to_string())
    } else if err.is_connect() || err.is_request() {
        ProviderError::Connection(err.to_string())
    } else {
        ProviderError::Other(err.to_string())
    }
}

/// Parse a Server-Sent Events stream from an OpenAI chat completion response.
fn parse_sse_stream(
    response: reqwest::Response,
    idle_timeout: Duration,
) -> impl Stream<Item = Result<LlmStreamChunk, ProviderError>> + Send {
    async_stream::stream! {
        use futures::StreamExt;

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut tool_call_accumulators: HashMap<u32, PartialToolCall> = HashMap::new();

        loop {
            let chunk_result = tokio::time::timeout(idle_timeout, stream.next()).await;

            match chunk_result {
                Ok(Some(Ok(bytes))) => {
                    let text = String::from_utf8_lossy(&bytes);
                    buffer.push_str(&text);

                    // Process complete SSE events (terminated by \n\n)
                    while let Some(idx) = buffer.find("\n\n") {
                        let event_str: String = buffer.drain(..idx + 2).collect();
                        for line in event_str.lines() {
                            if let Some(data) = line.strip_prefix("data: ") {
                                let data = data.trim();
                                if data == "[DONE]" {
                                    return;
                                }
                                if data.is_empty() {
                                    continue;
                                }
                                match serde_json::from_str::<ChatChunk>(data) {
                                    Ok(chunk) => {
                                        // Convert each choice's delta to an LlmStreamChunk
                                        for choice in chunk.choices {
                                            let mut llm_chunk = LlmStreamChunk {
                                                content: choice.delta.content.clone(),
                                                thinking: None,
                                                tool_calls: None,
                                                done: false,
                                                done_reason: choice.finish_reason.clone(),
                                                eval_count: None,
                                                prompt_eval_count: None,
                                            };

                                            // Accumulate tool calls (incremental arguments)
                                            if let Some(delta_calls) = choice.delta.tool_calls {
                                                for delta_call in delta_calls {
                                                    let index = delta_call.id.len() as u32; // use id hash as index
                                                    let accumulator = tool_call_accumulators
                                                        .entry(index)
                                                        .or_insert_with(|| PartialToolCall {
                                                            id: delta_call.id.clone(),
                                                            name: String::new(),
                                                            arguments: String::new(),
                                                        });
                                                    if !delta_call.function.name.is_empty() {
                                                        accumulator.name = delta_call.function.name;
                                                    }
                                                    accumulator.arguments.push_str(&delta_call.function.arguments);
                                                }
                                                // After processing all deltas, build the tool calls
                                                let complete_calls: Vec<LlmToolCall> = tool_call_accumulators
                                                    .values()
                                                    .filter(|p| !p.name.is_empty())
                                                    .map(|p| LlmToolCall {
                                                        id: p.id.clone(),
                                                        name: p.name.clone(),
                                                        arguments: serde_json::from_str(&p.arguments)
                                                            .unwrap_or_else(|_| serde_json::Value::String(p.arguments.clone())),
                                                    })
                                                    .collect();
                                                if !complete_calls.is_empty() {
                                                    llm_chunk.tool_calls = Some(complete_calls);
                                                }
                                            }

                                            yield Ok(llm_chunk);
                                        }

                                        // If usage is reported in the chunk (with stream_options.include_usage),
                                        // propagate it.
                                        if let Some(usage) = chunk.usage {
                                            // We don't have a current chunk to attach to; yield a "done" sentinel
                                            yield Ok(LlmStreamChunk {
                                                content: None,
                                                thinking: None,
                                                tool_calls: None,
                                                done: true,
                                                done_reason: Some("stop".to_string()),
                                                eval_count: Some(usage.completion_tokens),
                                                prompt_eval_count: Some(usage.prompt_tokens),
                                            });
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to parse SSE chunk: {e} (data: {data})");
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Some(Err(e))) => {
                    yield Err(classify_reqwest_error(e));
                    return;
                }
                Ok(None) => return, // Stream ended
                Err(_elapsed) => {
                    yield Err(ProviderError::Timeout(format!(
                        "SSE stream idle timeout after {}s",
                        idle_timeout.as_secs()
                    )));
                    return;
                }
            }
        }
    }
}

/// Partial tool call state for accumulating OpenAI incremental arguments.
#[derive(Debug, Clone)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_retry_after_seconds() {
        let result = parse_retry_after("120");
        assert_eq!(result, Some(Duration::from_secs(120)));
    }

    #[test]
    fn test_parse_retry_after_invalid() {
        let result = parse_retry_after("not-a-number");
        assert_eq!(result, None);
    }

    #[test]
    fn test_backoff_delay_zero_attempt() {
        let cfg = OpenAICompatibleConfig {
            base_url: "http://localhost:11434/v1".to_string(),
            api_key: None,
            connect_timeout_secs: 5,
            read_timeout_secs: 300,
            stream_idle_timeout_secs: 60,
            max_retries: 3,
            retry_base_delay_ms: 2000,
            retry_max_delay_ms: 16000,
            retry_jitter_percent: 0,
        };
        let provider = OpenAICompatibleProvider::new(cfg).unwrap();
        assert_eq!(provider.backoff_delay(1), Duration::from_secs(2));
        assert_eq!(provider.backoff_delay(2), Duration::from_secs(4));
        assert_eq!(provider.backoff_delay(3), Duration::from_secs(8));
    }

    #[test]
    fn test_backoff_delay_caps_at_max() {
        let cfg = OpenAICompatibleConfig {
            base_url: "http://localhost:11434/v1".to_string(),
            api_key: None,
            connect_timeout_secs: 5,
            read_timeout_secs: 300,
            stream_idle_timeout_secs: 60,
            max_retries: 3,
            retry_base_delay_ms: 2000,
            retry_max_delay_ms: 8000,
            retry_jitter_percent: 0,
        };
        let provider = OpenAICompatibleProvider::new(cfg).unwrap();
        // 2s, 4s, 8s (capped) — should not exceed 8s
        assert_eq!(provider.backoff_delay(3), Duration::from_secs(8));
    }

    #[test]
    fn test_convert_messages_basic() {
        let msgs = vec![
            LlmMessage {
                role: LlmRole::System,
                content: "You are a helpful assistant".to_string(),
                tool_calls: None,
                images: None,
                audio: None,
                thinking: None,
                name: None,
                tool_call_id: None,
            },
            LlmMessage {
                role: LlmRole::User,
                content: "Hello".to_string(),
                tool_calls: None,
                images: None,
                audio: None,
                thinking: None,
                name: None,
                tool_call_id: None,
            },
        ];
        let converted = OpenAICompatibleProvider::convert_messages(msgs);
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0]["role"], "system");
        assert_eq!(converted[0]["content"], "You are a helpful assistant");
        assert_eq!(converted[1]["role"], "user");
        assert_eq!(converted[1]["content"], "Hello");
    }

    #[test]
    fn test_convert_messages_tool_result() {
        let msgs = vec![LlmMessage {
            role: LlmRole::Tool,
            content: "Tool result".to_string(),
            tool_calls: None,
            images: None,
            audio: None,
            thinking: None,
            name: None,
            tool_call_id: Some("call_123".to_string()),
        }];
        let converted = OpenAICompatibleProvider::convert_messages(msgs);
        assert_eq!(converted[0]["role"], "tool");
        assert_eq!(converted[0]["tool_call_id"], "call_123");
    }

    #[test]
    fn test_convert_response_with_tool_calls() {
        let response = ChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "test-model".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: OpenAIMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_calls: Some(vec![OpenAIToolCall {
                        id: "call_1".to_string(),
                        tool_type: "function".to_string(),
                        function: OpenAIToolCallFunction {
                            name: "get_weather".to_string(),
                            arguments: r#"{"location":"London"}"#.to_string(),
                        },
                    }]),
                    tool_call_id: None,
                },
                finish_reason: Some("tool_calls".to_string()),
            }],
            usage: Some(OpenAIUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
        };

        let llm_resp = OpenAICompatibleProvider::convert_response(response);
        assert_eq!(llm_resp.content, "");
        assert!(llm_resp.tool_calls.is_some());
        let calls = llm_resp.tool_calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].id, "call_1");
    }

    #[test]
    fn test_url_construction() {
        let cfg = OpenAICompatibleConfig {
            base_url: "http://localhost:11434/v1".to_string(),
            api_key: None,
            connect_timeout_secs: 5,
            read_timeout_secs: 300,
            stream_idle_timeout_secs: 60,
            max_retries: 3,
            retry_base_delay_ms: 2000,
            retry_max_delay_ms: 16000,
            retry_jitter_percent: 20,
        };
        let provider = OpenAICompatibleProvider::new(cfg).unwrap();
        assert_eq!(
            provider.url("/chat/completions"),
            "http://localhost:11434/v1/chat/completions"
        );
    }

    #[test]
    fn test_url_construction_with_trailing_slash() {
        let cfg = OpenAICompatibleConfig {
            base_url: "http://localhost:11434/v1/".to_string(),
            api_key: None,
            connect_timeout_secs: 5,
            read_timeout_secs: 300,
            stream_idle_timeout_secs: 60,
            max_retries: 3,
            retry_base_delay_ms: 2000,
            retry_max_delay_ms: 16000,
            retry_jitter_percent: 20,
        };
        let provider = OpenAICompatibleProvider::new(cfg).unwrap();
        assert_eq!(
            provider.url("/chat/completions"),
            "http://localhost:11434/v1/chat/completions"
        );
    }
}
