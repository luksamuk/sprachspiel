//! Ollama provider implementation using native reqwest.
//!
//! This replaces the ollama-rs crate with a direct HTTP implementation,
//! enabling proper timeouts, retry logic, and streaming with idle timeout.

use crate::provider::types::{
    LlmMessage, LlmResponse, LlmRole, LlmStreamChunk, LlmToolCall,
    ProviderCapabilities, ProviderError, ProviderOptions, ToolInfo,
};
use crate::provider::ollama_api::{
    ChatRequest, ChatResponse, GenerateRequest, GenerateResponse, EmbedRequest, EmbedResponse,
    ModelShowResponse, OllamaMessage, OllamaToolCall, OllamaToolCallFunction,
};
use async_trait::async_trait;
use futures::Stream;
use reqwest::Client;
use std::pin::Pin;
use std::time::Duration;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

/// Configuration for OllamaProvider.
#[derive(Debug, Clone)]
pub struct OllamaProviderConfig {
    pub base_url: String,
    pub connect_timeout_secs: u64,
    pub read_timeout_secs: u64,
    pub stream_idle_timeout_secs: u64,
    pub max_retries: u32,
    pub retry_base_delay_ms: u64,
    pub retry_max_delay_ms: u64,
    pub retry_jitter_percent: u8,
}

/// Native reqwest-based Ollama provider.
pub struct OllamaProvider {
    client: Client,
    config: OllamaProviderConfig,
}

impl OllamaProvider {
    /// Create a new OllamaProvider with the given configuration.
    pub fn new(config: OllamaProviderConfig) -> Result<Self, ProviderError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.read_timeout_secs))
            .connect_timeout(Duration::from_secs(config.connect_timeout_secs))
            .build()
            .map_err(|e| ProviderError::Config(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { client, config })
    }

    /// Get the provider name.
    pub fn provider_name(&self) -> &'static str {
        "ollama"
    }

    /// Build the full API URL for an endpoint.
    fn url(&self, endpoint: &str) -> String {
        format!("{}{}", self.config.base_url.trim_end_matches('/'), endpoint)
    }

    /// Convert agnostic LlmMessage to Ollama API message.
    fn to_ollama_messages(&self, messages: &[LlmMessage]) -> Vec<OllamaMessage> {
        messages.iter().map(|m| OllamaMessage {
            role: match m.role {
                LlmRole::User => "user",
                LlmRole::Assistant => "assistant",
                LlmRole::System => "system",
                LlmRole::Tool => "tool",
            }.to_string(),
            content: m.content.clone(),
            images: m.images.clone(),
            tool_calls: m.tool_calls.as_ref().map(|tc| tc.iter().map(|t| OllamaToolCall {
                function: OllamaToolCallFunction {
                    name: t.name.clone(),
                    arguments: t.arguments.to_string(),
                }
            }).collect()),
            thinking: m.thinking.clone(),
        }).collect()
    }

    /// Convert ProviderOptions to Ollama request options.
    fn to_ollama_options(&self, options: &ProviderOptions) -> serde_json::Value {
        let mut obj = serde_json::json!({});
        
        if let Some(temp) = options.temperature {
            obj["temperature"] = serde_json::json!(temp);
        }
        if let Some(top_p) = options.top_p {
            obj["top_p"] = serde_json::json!(top_p);
        }
        if let Some(top_k) = options.top_k {
            obj["top_k"] = serde_json::json!(top_k);
        }
        if let Some(num_predict) = options.num_predict {
            obj["num_predict"] = serde_json::json!(num_predict);
        }
        if let Some(stop) = &options.stop_sequences {
            obj["stop"] = serde_json::json!(stop);
        }
        if let Some(think) = options.think {
            obj["think"] = serde_json::json!(think);
        }
        if let Some(format) = &options.format {
            obj["format"] = serde_json::json!(format);
        }
        if let Some(audio_format) = &options.audio_format {
            obj["audio_format"] = serde_json::json!(audio_format);
        }
        
        obj
    }

    /// Classify reqwest error into ProviderError with retry semantics.
    fn classify_error(&self, err: reqwest::Error) -> ProviderError {
        if err.is_timeout() {
            ProviderError::Timeout(err.to_string())
        } else if err.is_connect() {
            ProviderError::Connection(err.to_string())
        } else if let Some(status) = err.status() {
            let code = status.as_u16();
            if code == 429 {
                ProviderError::RateLimit {
                    message: err.to_string(),
                    retry_after: None, // Will be populated from header in chat()
                }
            } else if code >= 500 {
                ProviderError::Api {
                    status: code,
                    body: err.to_string(),
                }
            } else {
                ProviderError::Api {
                    status: code,
                    body: err.to_string(),
                }
            }
        } else {
            ProviderError::Connection(err.to_string())
        }
    }

    /// Create a ProviderError from HTTP status code and response body.
    fn status_error(&self, status: u16, body: String) -> ProviderError {
        if status == 429 {
            ProviderError::RateLimit {
                message: body,
                retry_after: None,
            }
        } else {
            ProviderError::Api {
                status,
                body,
            }
        }
    }

    /// Calculate exponential backoff with jitter.
    fn backoff_delay(&self, attempt: u32, retry_after: Option<Duration>) -> Duration {
        if let Some(ra) = retry_after {
            return ra;
        }
        
        let base = Duration::from_millis(self.config.retry_base_delay_ms);
        let max_delay = Duration::from_millis(self.config.retry_max_delay_ms);
        
        let exp_delay = base * 2_u32.pow((attempt - 1).min(self.config.retry_max_delay_ms as u32 / self.config.retry_base_delay_ms as u32));
        let delay = exp_delay.min(max_delay);
        
        // Add jitter: ±retry_jitter_percent%
        let jitter_range = delay * self.config.retry_jitter_percent as u32 / 100;
        let jitter = rand::random::<u64>() % (jitter_range.as_millis() as u64 + 1);
        
        delay + Duration::from_millis(jitter)
    }

    /// Execute a request with retry logic.
    async fn execute_with_retry<T, F, Fut>(
        &self,
        operation: F,
        cancel_token: CancellationToken,
    ) -> Result<T, ProviderError>
    where
        F: Fn(u32) -> Fut,
        Fut: std::future::Future<Output = Result<T, ProviderError>>,
    {
        let mut last_err = None;
        
        for attempt in 1..=self.config.max_retries {
            // Check cancellation
            if cancel_token.is_cancelled() {
                return Err(ProviderError::Other("Operation cancelled".to_string()));
            }

            match operation(attempt).await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    let category = err.retry_category();
                    
                    if !category.is_retryable() || attempt == self.config.max_retries {
                        return Err(err);
                    }
                    
                    // Extract retry_after for RateLimit
                    let retry_after = match &err {
                        ProviderError::RateLimit { retry_after, .. } => *retry_after,
                        _ => None,
                    };
                    
                    // Log retry attempt
                    log::warn!(
                        "Retrying request (attempt {}/{}): {}",
                        attempt,
                        self.config.max_retries,
                        err
                    );
                    
                    let delay = self.backoff_delay(attempt, retry_after);
                    
                    // Sleep with cancellation awareness
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {}
                        _ = cancel_token.cancelled() => {
                            return Err(ProviderError::Other("Operation cancelled during retry".to_string()));
                        }
                    }
                    
                    last_err = Some(err);
                }
            }
        }
        
        Err(last_err.unwrap_or_else(|| ProviderError::Other("Max retries exceeded".to_string())))
    }
}

#[async_trait]
impl crate::provider::LlmProvider for OllamaProvider {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<LlmMessage>,
        tools: Vec<ToolInfo>,
        options: ProviderOptions,
    ) -> Result<LlmResponse, ProviderError> {
        let url = self.url("/api/chat");
        
        let ollama_messages = self.to_ollama_messages(&messages);
        
        let request = ChatRequest {
            model: model.to_string(),
            messages: ollama_messages,
            stream: false,
            options: Some(self.to_ollama_options(&options)),
            tools: if tools.is_empty() { None } else { Some(tools) },
        };
        
        // Apply retry for the request
        let cancel_token = CancellationToken::new();
        let cancel_for_closure = cancel_token.clone();
        self.execute_with_retry(
            |_attempt| {
                let req = request.clone();
                let client = self.client.clone();
                let url = url.clone();
                let cancel = cancel_for_closure.clone();
                
                async move {
                    let resp = timeout(
                        Duration::from_secs(self.config.read_timeout_secs),
                        client.post(&url).json(&req).send()
                    ).await;
                    
                    match resp {
                        Ok(Ok(response)) => {
                            if !response.status().is_success() {
                                let status = response.status();
                                let body = response.text().await.unwrap_or_default();
                                return Err(self.status_error(status.as_u16(), body));
                            }
                            
                            let chat_resp: ChatResponse = response.json().await
                                .map_err(|e| ProviderError::Other(format!("JSON parse error: {}", e)))?;
                            
                            Ok(LlmResponse {
                                model: chat_resp.model,
                                content: chat_resp.message.content,
                                tool_calls: chat_resp.message.tool_calls.map(|tc| tc.into_iter().map(|t| LlmToolCall {
                                    id: t.function.name.clone(),
                                    name: t.function.name,
                                    arguments: serde_json::from_str(&t.function.arguments).unwrap_or_default(),
                                }).collect()),
                                done_reason: chat_resp.done_reason,
                                eval_count: chat_resp.eval_count,
                                prompt_eval_count: chat_resp.prompt_eval_count,
                            })
                        }
                        Ok(Err(e)) => Err(self.classify_error(e)),
                        Err(_) => Err(ProviderError::Timeout("Request timeout".to_string())),
                    }
                }
            },
            cancel_token,
        ).await
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
        let url = self.url("/api/chat");
        
        let ollama_messages = self.to_ollama_messages(&messages);
        
        let request = ChatRequest {
            model: model.to_string(),
            messages: ollama_messages,
            stream: true,
            options: Some(self.to_ollama_options(&options)),
            tools: if tools.is_empty() { None } else { Some(tools) },
        };
        
        let client = self.client.clone();
        let idle_secs = self.config.stream_idle_timeout_secs;
        
        // Send the request first to get the response
        let response = timeout(
            Duration::from_secs(self.config.connect_timeout_secs),
            client.post(&url).json(&request).send()
        ).await
            .map_err(|_| ProviderError::Timeout("Connection timeout".to_string()))?
            .map_err(|e| self.classify_error(e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(self.status_error(status.as_u16(), body));
        }
        
        let byte_stream = response.bytes_stream();
        
        let stream = async_stream::stream! {
            use futures::StreamExt;
            let mut buffer = String::new();
            let mut idle_timer = tokio::time::interval(Duration::from_secs(idle_secs));
            idle_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            idle_timer.tick().await; // First tick is immediate
            
            let mut byte_stream = byte_stream;
            loop {
                tokio::select! {
                    _ = idle_timer.tick() => {
                        yield Err(ProviderError::Timeout("Stream idle timeout".to_string()));
                        return;
                    }
                    chunk_result = byte_stream.next() => {
                        match chunk_result {
                            Some(Ok(bytes)) => {
                                idle_timer.reset();
                                buffer.push_str(&String::from_utf8_lossy(&bytes));
                                
                                // Process complete lines
                                while let Some(pos) = buffer.find('\n') {
                                    let line: String = buffer.drain(..=pos).collect();
                                    let line = line.trim_end();
                                    
                                    if let Ok(chat_resp) = serde_json::from_str::<ChatResponse>(line) {
                                        if chat_resp.done {
                                            yield Ok(LlmStreamChunk {
                                                content: None,
                                                thinking: None,
                                                tool_calls: None,
                                                done: true,
                                                done_reason: chat_resp.done_reason,
                                                eval_count: chat_resp.eval_count,
                                                prompt_eval_count: chat_resp.prompt_eval_count,
                                            });
                                            return;
                                        }
                                        
                                        let tool_calls = chat_resp.message.tool_calls.map(|tc| {
                                            tc.into_iter().map(|t| LlmToolCall {
                                                id: t.function.name.clone(),
                                                name: t.function.name,
                                                arguments: serde_json::from_str(&t.function.arguments).unwrap_or_default(),
                                            }).collect()
                                        });
                                        
                                        yield Ok(LlmStreamChunk {
                                            content: if chat_resp.message.content.is_empty() { 
                                                None 
                                            } else { 
                                                Some(chat_resp.message.content) 
                                            },
                                            thinking: chat_resp.message.thinking,
                                            tool_calls,
                                            done: false,
                                            done_reason: None,
                                            eval_count: None,
                                            prompt_eval_count: None,
                                        });
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                yield Err(ProviderError::Connection(e.to_string()));
                                return;
                            }
                            None => return,
                        }
                    }
                }
            }
        };
        
        Ok(Box::pin(stream))
    }

    async fn generate(
        &self,
        model: &str,
        prompt: &str,
        images: Vec<String>,
        audio: Vec<String>,
        options: ProviderOptions,
    ) -> Result<String, ProviderError> {
        let url = self.url("/api/generate");
        
        let request = GenerateRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            stream: false,
            images: if images.is_empty() { None } else { Some(images) },
            options: Some(self.to_ollama_options(&options)),
        };
        
        let cancel_token = CancellationToken::new();
        let cancel_for_closure = cancel_token.clone();
        
        self.execute_with_retry(
            |_| {
                let req = request.clone();
                let client = self.client.clone();
                let url = url.clone();
                let cancel = cancel_for_closure.clone();
                
                async move {
                    let resp = timeout(
                        Duration::from_secs(self.config.read_timeout_secs),
                        client.post(&url).json(&req).send()
                    ).await;
                    
                    match resp {
                        Ok(Ok(response)) => {
                            if !response.status().is_success() {
                                let status = response.status();
                                let body = response.text().await.unwrap_or_default();
                                return Err(self.status_error(status.as_u16(), body));
                            }
                            
                            let gen_resp: GenerateResponse = response.json().await
                                .map_err(|e| ProviderError::Other(format!("JSON parse error: {}", e)))?;
                            
                            Ok(gen_resp.response)
                        }
                        Ok(Err(e)) => Err(self.classify_error(e)),
                        Err(_) => Err(ProviderError::Timeout("Request timeout".to_string())),
                    }
                }
            },
            cancel_token,
        ).await
    }

    async fn embed(
        &self,
        text: &str,
        model: &str,
        dimensions: Option<usize>,
    ) -> Result<Vec<f32>, ProviderError> {
        let url = self.url("/api/embed");
        
        let mut request = EmbedRequest {
            model: model.to_string(),
            input: text.to_string(),
            truncate: true,
            options: None,
        };
        
        if let Some(dims) = dimensions {
            request.options = Some(serde_json::json!({ "dimensions": dims }));
        }
        
        let cancel_token = CancellationToken::new();
        
        self.execute_with_retry(
            |_| {
                let req = request.clone();
                let client = self.client.clone();
                let url = url.clone();
                
                async move {
                    let resp = timeout(
                        Duration::from_secs(self.config.read_timeout_secs),
                        client.post(&url).json(&req).send()
                    ).await;
                    
                    match resp {
                        Ok(Ok(response)) => {
                            if !response.status().is_success() {
                                let status = response.status();
                                let body = response.text().await.unwrap_or_default();
                                return Err(self.status_error(status.as_u16(), body));
                            }
                            
                            let embed_resp: EmbedResponse = response.json().await
                                .map_err(|e| ProviderError::Other(format!("JSON parse error: {}", e)))?;
                            
                            Ok(embed_resp.embeddings.first().cloned().unwrap_or_default())
                        }
                        Ok(Err(e)) => Err(self.classify_error(e)),
                        Err(_) => Err(ProviderError::Timeout("Request timeout".to_string())),
                    }
                }
            },
            cancel_token,
        ).await
    }

    async fn detect_capabilities(&self, model: &str) -> Result<ProviderCapabilities, ProviderError> {
        let url = self.url("/api/show");
        
        let cancel_token = CancellationToken::new();
        
        self.execute_with_retry(
            |_| {
                let client = self.client.clone();
                let url = url.clone();
                let model_name = model.to_string();
                
                async move {
                    let resp = timeout(
                        Duration::from_secs(self.config.read_timeout_secs),
                        client.post(&url).json(&serde_json::json!({ "name": model_name })).send()
                    ).await;
                    
                    match resp {
                        Ok(Ok(response)) => {
                            if !response.status().is_success() {
                                // If show fails, try tags endpoint
                                let tags_url = format!("{}{}", self.config.base_url.trim_end_matches('/'), "/api/tags");
                                let tags_resp = client.get(&tags_url).send().await
                                    .map_err(|e| ProviderError::Connection(e.to_string()))?;
                                
                                if !tags_resp.status().is_success() {
                                    return Err(ProviderError::Other("Failed to detect capabilities".to_string()));
                                }
                                
                                let tags: serde_json::Value = tags_resp.json().await
                                    .map_err(|e| ProviderError::Other(e.to_string()))?;
                                
                                // Check if model exists in tags
                                if let Some(models) = tags["models"].as_array() {
                                    for m in models {
                                        if m["name"].as_str() == Some(model) {
                                            return Ok(ProviderCapabilities::default());
                                        }
                                    }
                                }
                                
                                return Err(ProviderError::Other("Model not found".to_string()));
                            }
                            
                            let show: ModelShowResponse = response.json().await
                                .map_err(|e| ProviderError::Other(format!("JSON parse error: {}", e)))?;
                            
                            Ok(ProviderCapabilities {
                                completion: true,
                                tools: show.capabilities.iter().any(|c| c.contains("tools")),
                                thinking: show.capabilities.iter().any(|c| c.contains("thinking")),
                                vision: show.capabilities.iter().any(|c| c.contains("vision")),
                                embedding: show.capabilities.iter().any(|c| c.contains("embedding")),
                                insert: false,
                                audio: false,
                                image: show.capabilities.iter().any(|c| c.contains("image")),
                                provider: "ollama".to_string(),
                                model: model.to_string(),
                            })
                        }
                        Ok(Err(e)) => Err(self.classify_error(e)),
                        Err(_) => Err(ProviderError::Timeout("Request timeout".to_string())),
                    }
                }
            },
            cancel_token,
        ).await
    }

    fn provider_name(&self) -> &str {
        "ollama"
    }

    async fn is_available(&self) -> bool {
        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), "/api/tags");
        
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }
}