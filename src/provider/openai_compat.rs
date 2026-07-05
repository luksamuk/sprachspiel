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
//! HTTP 429 responses carry a `Retry-After` header that is parsed
//! into the `retry_after: Option<Duration>` field on `RateLimit`.

use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;
use futures::Stream;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};

use super::openai_types::{
    ChatChoice, ChatChunk, ChatRequest, ChatResponse, EmbeddingsRequest, EmbeddingsResponse,
    ModelsResponse, OpenAITool, OpenAIToolFunction, StreamOptions,
};
use super::tool_accumulator::ToolCallAccumulator;

/// Tuple of OpenAI request fields derived from `ProviderOptions`.
type ConvertedOptions = (
    Option<f32>,         // temperature
    Option<f32>,         // top_p
    Option<u32>,         // max_tokens (from num_predict)
    Option<Vec<String>>, // stop_sequences
    Option<u32>,         // seed
);
use super::types::{
    LlmMessage, LlmResponse, LlmRole, LlmStreamEvent, LlmToolCall, LlmUsage, ProviderCapabilities,
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

/// What to do with an HTTP response that is not a success (2xx).
///
/// Extracted from `chat_with_retry` and `send_stream_request` to
/// eliminate the duplicated retry-classification logic (429/5xx/
/// transient-4xx/permanent-4xx) between the two functions.
enum RetryAction {
    /// Retry the request after `delay`. `event` is the
    /// `ProviderRetryStarted` event to emit; `error` is the
    /// error to record as `last_error`.
    Retry {
        delay: Duration,
        event: LlmStreamEvent,
        error: ProviderError,
    },
    /// Don't retry — surface the error. `event` is an optional
    /// `ProviderRetryFinished { success: false }` event to emit
    /// (only if retries were attempted).
    Fail {
        error: ProviderError,
        event: Option<LlmStreamEvent>,
    },
}

impl OpenAICompatibleProvider {
    /// Access the underlying reqwest client (used by the ollama_rs shim).
    pub fn as_client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Create a new `OpenAICompatibleProvider`.
    pub fn new(config: OpenAICompatibleConfig) -> Result<Self, ProviderError> {
        let api_key = config
            .api_key
            .clone()
            .or_else(|| std::env::var("OPENAI_API_KEY").ok());

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

    /// Probe the embedding endpoint.
    ///
    /// Sends a minimal POST to `/v1/embeddings` with a short test
    /// text (`"test"`) and returns the actual response dim count.
    /// The probe does NOT pass `dimensions` in the request body
    /// (adaptive — some providers reject it). The caller compares
    /// the response dim count against the alias's declared
    /// `dimensions` to verify a strict match.
    ///
    /// Returns:
    /// - `Ok(dim_count)` if the call succeeded (any 2xx status)
    /// - `Err(ProviderError::Api { status, body })` on 4xx/5xx
    /// - `Err(ProviderError::Http(_))` on network errors
    ///
    /// This is called once at startup by the indexing
    /// initialization path when `[indexing].probe = true` in
    /// `config.toml`. Set the flag to `false` to skip the probe
    /// (useful for cold-start scenarios).
    pub async fn probe_embedding(&self, model: &str) -> Result<usize, ProviderError> {
        let url = self.url("/embeddings");
        let request = EmbeddingsRequest {
            model: model.to_string(),
            input: "test".to_string(),
            // ADAPTIVE: do not pass `dimensions` in the request
            // body. Some providers (older llama.cpp, certain vLLM
            // builds) reject it with 400. The response dim count
            // is the ground truth; the caller compares it against
            // the alias's declared `dimensions` for strict verify.
            dimensions: None,
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

        let body = response.text().await.unwrap_or_default();
        let emb_resp: EmbeddingsResponse = serde_json::from_str(&body).map_err(|e| {
            ProviderError::Other(format!("Failed to parse embeddings response: {e}"))
        })?;

        let dim = emb_resp
            .data
            .into_iter()
            .next()
            .map(|e| e.embedding.len())
            .ok_or_else(|| ProviderError::Other("Empty embeddings response".to_string()))?;

        Ok(dim)
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
                    parameters: serde_json::to_value(&t.function.parameters).unwrap_or_else(|e| {
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
                message,
                finish_reason,
                ..
            }) => {
                let tool_calls = message.tool_calls.map(|calls| {
                    calls
                        .into_iter()
                        .map(|c| LlmToolCall {
                            id: c.id.unwrap_or_default(),
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
                (
                    message.content.unwrap_or_default(),
                    tool_calls,
                    finish_reason,
                )
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

    /// Decide whether a 4xx response is **transient** (worth retrying)
    /// or **permanent** (give up and surface to the user).
    ///
    /// llama-swap and similar OpenAI-compatible proxies can return
    /// 4xx responses with **empty bodies** while the upstream model
    /// is mid-swap (e.g. another model was just loaded and the
    /// proxy is still warming up the new one). The same request
    /// succeeds a few seconds later. Treating these as terminal
    /// causes a 1.5ms "HTTP 400" hang with no useful diagnostic.
    ///
    /// Conversely, a 4xx with a JSON body that names the failure
    /// (e.g. "exceed_context_size_error", "invalid_request_error",
    /// "missing required field") is almost always terminal —
    /// retrying it just hits the same path again.
    ///
    /// Heuristic:
    ///   1. 408 Request Timeout and 425 Too Early are always
    ///      transient (RFC 7231 / 8470).
    ///   2. 4xx with an empty body is treated as transient (model
    ///      swap, proxy hiccup).
    ///   3. 4xx with a body matching the OpenAI error envelope
    ///      shape (`{"error": {"code": ..., "message": ...}}`) is
    ///      classified by `message`:
    ///         - "exceed_context_size", "context_length_exceeded",
    ///           "too long" → permanent
    ///         - "invalid", "malformed", "missing", "unknown",
    ///           "validation" → permanent
    ///         - "model_not_found", "not loaded", "unavailable",
    ///           "warming up", "loading" → transient
    ///         - anything else → transient (give the proxy the
    ///           benefit of the doubt)
    ///   4. 4xx with a non-JSON body is treated as transient
    ///      (proxy returned HTML or plain text — usually a
    ///      transient 502/503 misclassified as 4xx by the proxy).
    fn is_transient_4xx_error(status: u16, body: &str) -> bool {
        Self::transient_4xx_reason(status, body).is_some()
    }

    /// Return a human-readable reason if a 4xx response is transient.
    ///
    /// This reason is surfaced to the UI via `ProviderRetryStarted`
    /// so the user sees "model warming up" instead of a raw HTTP status.
    fn transient_4xx_reason(status: u16, body: &str) -> Option<String> {
        // (1) Always-transient codes (within 4xx)
        if matches!(status, 408 | 425) {
            return Some(format!("transient client error (HTTP {status})"));
        }
        if !(400..500).contains(&status) {
            return None;
        }

        // (2) Empty body — llama-swap model swap signature
        let trimmed = body.trim();
        if trimmed.is_empty() {
            return Some("empty response from proxy (model loading?)".to_string());
        }

        // (3) JSON envelope — classify by message
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let msg = value
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .or_else(|| value.get("message").and_then(|m| m.as_str()))
                .or_else(|| value.get("error").and_then(|e| e.as_str()))
                .unwrap_or("");

            let msg_lower = msg.to_lowercase();

            // Permanent patterns (the user has to fix something
            // — retrying won't help)
            const PERMANENT: &[&str] = &[
                "exceed_context",
                "exceeds the available context",
                "exceeds the model",
                "context_length_exceeded",
                "context length exceeded",
                "too long",
                "too many tokens",
                "prompt is too long",
                "invalid",
                "malformed",
                "missing",
                "unknown field",
                "validation",
                "unsupported",
                "decode",
                "parse",
            ];
            if PERMANENT.iter().any(|p| msg_lower.contains(p)) {
                return None;
            }

            // Transient patterns (the proxy/upstream is not ready)
            const TRANSIENT: &[&str] = &[
                "model_not_found",
                "model not found",
                "not loaded",
                "unavailable",
                "warming up",
                "warming",
                "loading",
                "not ready",
                "no such model",
                "try again",
                "overloaded",
            ];
            if let Some(matched) = TRANSIENT.iter().find(|p| msg_lower.contains(*p)) {
                return Some(format!("{matched} (HTTP {status})"));
            }

            // Default for unknown JSON shape: transient
            return Some(format!("transient client error (HTTP {status})"));
        }

        // (4) Non-JSON body — likely a proxy misclassification
        Some(format!("non-JSON error body (HTTP {status})"))
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

    /// Classify a non-success HTTP response and decide whether to retry.
    ///
    /// Shared by `chat_with_retry` (non-streaming) and
    /// `send_stream_request` (streaming) to eliminate the duplicated
    /// retry-classification logic for 429/5xx/transient-4xx/permanent-4xx.
    async fn classify_retry_response(
        &self,
        status: reqwest::StatusCode,
        resp: reqwest::Response,
        attempt: u32,
        max_attempts: u32,
        tag: &str,
    ) -> RetryAction {
        let retry_after = resp
            .headers()
            .get(RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_retry_after);

        if status.as_u16() == 429 {
            if attempt < max_attempts {
                let delay = retry_after.unwrap_or(self.backoff_delay(attempt));
                log::info!(
                    "[{tag}] HTTP 429, retrying in {}ms (attempt {}/{})",
                    delay.as_millis(),
                    attempt,
                    max_attempts
                );
                return RetryAction::Retry {
                    delay,
                    event: LlmStreamEvent::ProviderRetryStarted {
                        attempt,
                        max_attempts,
                        delay_ms: delay.as_millis() as u64,
                        reason: "rate limited (HTTP 429)".to_string(),
                    },
                    error: ProviderError::RateLimit {
                        message: format!("HTTP 429 (attempt {attempt})"),
                        retry_after,
                    },
                };
            }
            let body = resp.text().await.unwrap_or_default();
            return RetryAction::Fail {
                error: ProviderError::RateLimit {
                    message: body,
                    retry_after,
                },
                event: (attempt > 1).then(|| LlmStreamEvent::ProviderRetryFinished {
                    success: false,
                    attempt,
                }),
            };
        }

        if status.is_server_error() {
            if attempt < max_attempts {
                let delay = self.backoff_delay(attempt);
                log::info!(
                    "[{tag}] {} (server error), retrying in {}ms (attempt {}/{})",
                    status.as_u16(),
                    delay.as_millis(),
                    attempt,
                    max_attempts
                );
                return RetryAction::Retry {
                    delay,
                    event: LlmStreamEvent::ProviderRetryStarted {
                        attempt,
                        max_attempts,
                        delay_ms: delay.as_millis() as u64,
                        reason: format!("server error (HTTP {})", status.as_u16()),
                    },
                    error: ProviderError::Api {
                        status: status.as_u16(),
                        body: format!("HTTP {} (attempt {})", status.as_u16(), attempt),
                    },
                };
            }
            return RetryAction::Fail {
                error: ProviderError::Api {
                    status: status.as_u16(),
                    body: format!("HTTP {} (server error)", status.as_u16()),
                },
                event: (attempt > 1).then(|| LlmStreamEvent::ProviderRetryFinished {
                    success: false,
                    attempt,
                }),
            };
        }

        // 4xx — distinguish transient vs permanent
        let body = resp.text().await.unwrap_or_default();
        let is_transient_4xx = Self::is_transient_4xx_error(status.as_u16(), &body);

        if is_transient_4xx && attempt < max_attempts {
            let delay = self.backoff_delay(attempt);
            let reason = Self::transient_4xx_reason(status.as_u16(), &body);
            log::info!(
                "[{tag}] transient 4xx (HTTP {}), retrying in {}ms (attempt {}/{})",
                status.as_u16(),
                delay.as_millis(),
                attempt,
                max_attempts
            );
            let preview: String = body.chars().take(500).collect();
            log::debug!(
                "[{tag}] transient 4xx body (HTTP {}, attempt {}/{}): {}",
                status.as_u16(),
                attempt,
                max_attempts,
                preview
            );
            return RetryAction::Retry {
                delay,
                event: LlmStreamEvent::ProviderRetryStarted {
                    attempt,
                    max_attempts,
                    delay_ms: delay.as_millis() as u64,
                    reason: reason.unwrap_or_else(|| format!("transient HTTP {status}")),
                },
                error: ProviderError::Api {
                    status: status.as_u16(),
                    body: format!("HTTP {} (transient, retrying): {}", status.as_u16(), body),
                },
            };
        }

        // Permanent 4xx or exhausted retries on transient 4xx
        let preview: String = body.chars().take(500).collect();
        log::debug!("[{tag}] 4xx body (surfacing): {}", preview);
        RetryAction::Fail {
            error: ProviderError::Api {
                status: status.as_u16(),
                body,
            },
            event: (attempt > 1).then(|| LlmStreamEvent::ProviderRetryFinished {
                success: false,
                attempt,
            }),
        }
    }

    /// Send a chat completion (non-streaming) with retry.
    ///
    /// `on_event` is called with `ProviderRetryStarted/Finished`
    /// events so the caller can surface retry status to the UI. For
    /// callers that do not need visibility, pass `|_| {}`.
    async fn chat_with_retry(
        &self,
        _model: &str,
        request: ChatRequest,
        on_event: impl Fn(LlmStreamEvent),
    ) -> Result<LlmResponse, ProviderError> {
        let url = self.url("/chat/completions");
        let mut last_error: Option<ProviderError> = None;
        let max_attempts = self.config.max_retries.max(1);

        for attempt in 1..=max_attempts {
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
                        on_event(LlmStreamEvent::ProviderRetryFinished {
                            success: true,
                            attempt,
                        });
                        let chat_resp: ChatResponse = resp.json().await.map_err(|e| {
                            ProviderError::Other(format!("Failed to parse response: {e}"))
                        })?;
                        return Ok(Self::convert_response(chat_resp));
                    }

                    let action = self
                        .classify_retry_response(
                            status,
                            resp,
                            attempt,
                            max_attempts,
                            "chat_with_retry",
                        )
                        .await;
                    match action {
                        RetryAction::Retry {
                            delay,
                            event,
                            error,
                        } => {
                            on_event(event);
                            last_error = Some(error);
                            tokio::time::sleep(delay).await;
                        }
                        RetryAction::Fail { error, event } => {
                            if let Some(ev) = event {
                                on_event(ev);
                            }
                            return Err(error);
                        }
                    }
                }
                Err(e) => {
                    let err = classify_reqwest_error(e);
                    last_error = Some(err.clone());
                    if attempt < max_attempts {
                        let delay = self.backoff_delay(attempt);
                        on_event(LlmStreamEvent::ProviderRetryStarted {
                            attempt,
                            max_attempts,
                            delay_ms: delay.as_millis() as u64,
                            reason: format!("network error: {err}"),
                        });
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        on_event(LlmStreamEvent::ProviderRetryFinished {
            success: false,
            attempt: max_attempts,
        });
        Err(last_error.unwrap_or_else(|| ProviderError::Other("Unknown error".to_string())))
    }

    /// Send the streaming request, applying retry logic, and collect retry
    /// events so they can be yielded at the head of the event stream.
    ///
    /// Retry visibility for the streaming path. Uses the shared
    /// `classify_retry_response` to decide retry vs fail (same logic
    /// as `chat_with_retry`), but instead of calling a callback it
    /// appends `ProviderRetryStarted/Finished` events to a vector that
    /// is prepended to the SSE event stream.
    async fn send_stream_request(
        &self,
        request: &ChatRequest,
    ) -> Result<(reqwest::Response, Vec<LlmStreamEvent>), ProviderError> {
        let url = self.url("/chat/completions");
        let mut attempt: u32 = 1;
        let max_attempts = self.config.max_retries.max(1);
        let mut retry_events: Vec<LlmStreamEvent> = Vec::new();

        let response = loop {
            let resp = self
                .client
                .post(&url)
                .headers(self.headers())
                .json(request)
                .send()
                .await;

            match resp {
                Ok(r) => {
                    let status = r.status();
                    if status.is_success() {
                        if attempt > 1 {
                            retry_events.push(LlmStreamEvent::ProviderRetryFinished {
                                success: true,
                                attempt,
                            });
                        }
                        break r;
                    }

                    let action = self
                        .classify_retry_response(status, r, attempt, max_attempts, "chat_stream")
                        .await;
                    match action {
                        RetryAction::Retry {
                            delay,
                            event,
                            error: _,
                        } => {
                            retry_events.push(event);
                            tokio::time::sleep(delay).await;
                            attempt += 1;
                            continue;
                        }
                        RetryAction::Fail { error, event } => {
                            if let Some(ev) = event {
                                retry_events.push(ev);
                            }
                            return Err(error);
                        }
                    }
                }
                Err(e) => {
                    let err = classify_reqwest_error(e);
                    if attempt < max_attempts {
                        let delay = self.backoff_delay(attempt);
                        retry_events.push(LlmStreamEvent::ProviderRetryStarted {
                            attempt,
                            max_attempts,
                            delay_ms: delay.as_millis() as u64,
                            reason: format!("network error: {err}"),
                        });
                        log::info!(
                            "[chat_stream] network error: {}, retrying in {}ms (attempt {}/{})",
                            err,
                            delay.as_millis(),
                            attempt,
                            max_attempts
                        );
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }
                    if attempt > 1 {
                        retry_events.push(LlmStreamEvent::ProviderRetryFinished {
                            success: false,
                            attempt,
                        });
                    }
                    return Err(err);
                }
            }
        };

        Ok((response, retry_events))
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

        // Non-streaming chat retries are surfaced through the
        // `on_event` callback.
        self.chat_with_retry(model, request, |_| {}).await
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: Vec<LlmMessage>,
        tools: Vec<ToolInfo>,
        options: ProviderOptions,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<LlmStreamEvent, ProviderError>> + Send>>,
        ProviderError,
    > {
        let (temperature, top_p, max_tokens, stop, seed) = Self::convert_options(&options);
        let messages_json = Self::convert_messages(messages);

        let request = ChatRequest {
            model: model.to_string(),
            messages: messages_json,
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
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
        };

        let (response, retry_events) = self.send_stream_request(&request).await?;
        let idle_timeout = Duration::from_secs(self.config.stream_idle_timeout_secs);
        let sse_stream = parse_sse_stream(response, idle_timeout);

        // Prepend retry lifecycle events so the consumer sees them before
        // any content events.
        let prelude = futures::stream::iter(retry_events.into_iter().map(Ok));
        let stream = prelude.chain(sse_stream);

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
            images: if images.is_empty() {
                None
            } else {
                Some(images)
            },
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

        let response = self.chat_with_retry(model, request, |_| {}).await?;
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

        let emb_resp: EmbeddingsResponse = response.json().await.map_err(|e| {
            ProviderError::Other(format!("Failed to parse embeddings response: {e}"))
        })?;

        emb_resp
            .data
            .into_iter()
            .next()
            .map(|e| e.embedding)
            .ok_or_else(|| ProviderError::Other("Empty embeddings response".to_string()))
    }

    #[allow(dead_code)] // W2 #123: consumed when ollama-rs is removed
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
            log::warn!(
                "detect_capabilities: /v1/models returned {status} for model '{model}', \
                 returning default capabilities"
            );
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
            tools: false,    // Default to false; merged with user models.toml flags
            thinking: false, // OpenAI doesn't expose "thinking" capability separately
            vision: true,    // Most OpenAI-compat servers support vision via image_url
            embedding: true, // /v1/embeddings is standard
            insert: false,
            audio: false,
            image: true,
            provider: "openai-compatible".to_string(),
            model: model_info
                .map(|m| m.id.clone())
                .unwrap_or_else(|| model.to_string()),
        })
    }

    #[allow(dead_code)] // W2 #123: consumed when ollama-rs is removed
    fn provider_name(&self) -> &str {
        "openai-compatible"
    }

    #[allow(dead_code)] // W2 #123: consumed when ollama-rs is removed
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
///
/// TODO #123: add a TTFB (time-to-first-byte) watchdog here. If no SSE chunk
/// arrives within ~120s of stream start, reconnect instead of waiting the
/// full idle_timeout (300s). Inspired by Hermes Agent's
/// HERMES_CODEX_TTFB_TIMEOUT_SECONDS. See IMPLEMENTATION.md #123 open topics.
fn parse_sse_stream(
    response: reqwest::Response,
    idle_timeout: Duration,
) -> impl Stream<Item = Result<LlmStreamEvent, ProviderError>> + Send {
    async_stream::stream! {
        use futures::StreamExt;

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut tool_call_accumulators = ToolCallAccumulator::new();
        let mut text_started = false;
        let mut thinking_started = false;

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
                                    // Stream ended normally — finalize tool calls.
                                    for event in tool_call_accumulators.finalize_all() {
                                        yield Ok(event);
                                    }
                                    return;
                                }
                                if data.is_empty() {
                                    continue;
                                }
                                    match serde_json::from_str::<ChatChunk>(data) {
                                    Ok(chunk) => {
                                        let mut finish_reason: Option<String> = None;
                                        for choice in chunk.choices {
                                            // Content delta
                                            if let Some(delta) = choice.delta.content
                                                && !delta.is_empty()
                                            {
                                                if !text_started {
                                                    text_started = true;
                                                    yield Ok(LlmStreamEvent::TextStart);
                                                }
                                                yield Ok(LlmStreamEvent::TextDelta { delta });
                                            }

                                            // Thinking delta
                                            if let Some(delta) = choice.delta.reasoning_content.clone()
                                                && !delta.is_empty()
                                            {
                                                if !thinking_started {
                                                    thinking_started = true;
                                                    yield Ok(LlmStreamEvent::ThinkingStart { signature: None });
                                                }
                                                yield Ok(LlmStreamEvent::ThinkingDelta { delta });
                                            }

                                            // Tool-call lifecycle events
                                            if let Some(delta_calls) = choice.delta.tool_calls {
                                                for delta_call in delta_calls {
                                                    for event in tool_call_accumulators.ingest(
                                                        delta_call.index,
                                                        delta_call.id,
                                                        Some(delta_call.function.name),
                                                        delta_call.function.arguments,
                                                    ) {
                                                        yield Ok(event);
                                                    }
                                                }
                                            }

                                            // Finish reason on this choice means the turn
                                            // is ending; finalize any in-flight tool calls.
                                            if choice.finish_reason.is_some() {
                                                finish_reason = choice.finish_reason.clone();
                                                for event in tool_call_accumulators.finalize_all() {
                                                    yield Ok(event);
                                                }
                                                text_started = false;
                                                thinking_started = false;
                                            }
                                        }

                                        // Usage reported in the final chunk.
                                        let usage_data = chunk.usage.map(|u| LlmUsage {
                                            prompt_tokens: u.prompt_tokens,
                                            completion_tokens: u.completion_tokens,
                                            total_tokens: u.total_tokens,
                                        });
                                        let reason = finish_reason.unwrap_or_else(|| "stop".to_string());
                                        yield Ok(LlmStreamEvent::Done {
                                            reason: Some(reason),
                                            usage: usage_data,
                                        });
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
                Ok(None) => {
                    // Stream ended — finalize any tool calls that were in flight.
                    for event in tool_call_accumulators.finalize_all() {
                        yield Ok(event);
                    }
                    return;
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::openai_types::{
        OpenAIMessage, OpenAIToolCall, OpenAIToolCallFunction, Usage as OpenAIUsage,
    };

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
                    role: Some("assistant".to_string()),
                    content: None,
                    reasoning_content: None,
                    name: None,
                    tool_calls: Some(vec![OpenAIToolCall {
                        index: 0,
                        id: Some("call_1".to_string()),
                        tool_type: Some("function".to_string()),
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
            base_url: "http://localhost:12434/v1/".to_string(),
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
            "http://localhost:12434/v1/chat/completions"
        );
    }

    // Regression tests for is_transient_4xx_error.
    //
    // llama-swap returns 400 with an empty body during model swap
    // (the proxy forwards the request before the upstream is
    // loaded). Permanent 4xx (context overflow, malformed body)
    // come with descriptive bodies. We must not waste retries on
    // the latter.

    #[test]
    fn test_is_transient_4xx_empty_body() {
        // The signature of llama-swap model swap — body is empty.
        assert!(OpenAICompatibleProvider::is_transient_4xx_error(400, ""));
        assert!(OpenAICompatibleProvider::is_transient_4xx_error(
            400,
            "   \n\t  "
        ));
    }

    #[test]
    fn test_is_transient_4xx_408_425() {
        // RFC-defined always-transient codes.
        assert!(OpenAICompatibleProvider::is_transient_4xx_error(408, ""));
        assert!(OpenAICompatibleProvider::is_transient_4xx_error(425, ""));
    }

    #[test]
    fn test_is_transient_4xx_permanent_context_overflow() {
        // The user reported case from #207: pdftotext output made
        // the request 40K tokens > 32K ctx. llama-swap returns a
        // structured exceed_context_size_error body. Retrying this
        // would just hit the same 400 — must NOT be transient.
        let body = r#"{"error":{"code":400,"message":"request (38449 tokens) exceeds the available context size (32768 tokens)","type":"exceed_context_size_error"}}"#;
        assert!(!OpenAICompatibleProvider::is_transient_4xx_error(400, body));
    }

    #[test]
    fn test_is_transient_4xx_permanent_invalid() {
        // Generic "invalid" / "malformed" / "missing" / "validation"
        // responses are terminal — the client has to fix something.
        assert!(!OpenAICompatibleProvider::is_transient_4xx_error(
            400,
            r#"{"error":"invalid request: model field missing"}"#
        ));
        assert!(!OpenAICompatibleProvider::is_transient_4xx_error(
            400,
            r#"{"error":"malformed JSON in request"}"#
        ));
        assert!(!OpenAICompatibleProvider::is_transient_4xx_error(
            400,
            r#"{"error":"validation failed: empty messages array"}"#
        ));
    }

    #[test]
    fn test_is_transient_4xx_transient_model_loading() {
        // llama-swap with a model that's not yet loaded: typically
        // returns 400 with "model not found" / "not loaded" /
        // "warming up" / "loading" in the message. Retrying is
        // the right call — the model will be ready in a few
        // seconds.
        assert!(OpenAICompatibleProvider::is_transient_4xx_error(
            400,
            r#"{"error":"model not found: qwen3.5-4b"}"#
        ));
        assert!(OpenAICompatibleProvider::is_transient_4xx_error(
            400,
            r#"{"error":"model not loaded, please try again"}"#
        ));
        assert!(OpenAICompatibleProvider::is_transient_4xx_error(
            400,
            r#"{"error":"warming up model, retry shortly"}"#
        ));
    }

    #[test]
    fn test_is_transient_4xx_unknown_json_default_transient() {
        // Unknown JSON shape: benefit of the doubt — retry.
        assert!(OpenAICompatibleProvider::is_transient_4xx_error(
            400,
            r#"{"weird":{"shape":42}}"#
        ));
    }

    #[test]
    fn test_is_transient_4xx_non_json_body() {
        // HTML page, plain text — proxy misclassification. Retry.
        assert!(OpenAICompatibleProvider::is_transient_4xx_error(
            400,
            "<html><body>502 Bad Gateway</body></html>"
        ));
    }

    #[test]
    fn test_is_transient_4xx_5xx_falls_through() {
        // The 5xx branch handles its own retry, so 5xx should
        // not be classified as transient by this helper (which
        // exists to refine the 4xx branch only).
        assert!(!OpenAICompatibleProvider::is_transient_4xx_error(500, ""));
        assert!(!OpenAICompatibleProvider::is_transient_4xx_error(
            503,
            r#"{"error":"unavailable"}"#
        ));
    }

    // Regression tests for the streaming-retry bug. Previously,
    // `chat_stream` returned Err immediately on any non-2xx response
    // — so a llama-swap cold-start 400 (empty body, model still loading)
    // would fail the user's request instead of being retried with
    // backoff like the non-streaming path did.

    #[test]
    fn test_streaming_retry_classifies_llama_swap_cold_start() {
        // llama-swap returns 400 with an empty body during model
        // swap. chat_stream now uses is_transient_4xx_error to
        // classify this and should retry.
        assert!(OpenAICompatibleProvider::is_transient_4xx_error(400, ""));
    }

    #[test]
    fn test_streaming_retry_classifies_5xx_as_transient() {
        // 5xx is handled by a different branch in chat_stream (the
        // `is_server_error` check, which always retries). Verify
        // the helper is consistent: 5xx should NOT be classified
        // by the 4xx helper (it has its own branch).
        assert!(!OpenAICompatibleProvider::is_transient_4xx_error(502, ""));
        assert!(!OpenAICompatibleProvider::is_transient_4xx_error(503, ""));
    }

    #[test]
    fn test_streaming_retry_classifies_context_overflow_as_permanent() {
        // Context overflow (large input) should fail fast, not
        // waste retries. The body tells us the cause.
        assert!(!OpenAICompatibleProvider::is_transient_4xx_error(
            400,
            r#"{"error":{"code":400,"message":"context length exceeded"}}"#
        ));
    }
}
