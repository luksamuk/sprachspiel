//! Ollama API request/response structs for serde serialization.

use serde::{Deserialize, Serialize};

/// Ollama chat message (request/response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OllamaToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

/// Ollama tool call format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaToolCall {
    pub function: OllamaToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaToolCallFunction {
    pub name: String,
    pub arguments: String, // JSON string
}

/// Chat request to /api/chat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<OllamaMessage>,
    #[serde(default)]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<crate::provider::types::ToolInfo>>,
}

/// Chat response from /api/chat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub model: String,
    pub created_at: String,
    pub message: OllamaMessage,
    #[serde(default)]
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_count: Option<u32>,
}

/// Generate request to /api/generate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
}

/// Generate response from /api/generate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub model: String,
    pub created_at: String,
    pub response: String,
    #[serde(default)]
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done_reason: Option<String>,
}

/// Embed request to /api/embed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedRequest {
    pub model: String,
    pub input: String,
    #[serde(default)]
    pub truncate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
}

/// Embed response from /api/embed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedResponse {
    pub embeddings: Vec<Vec<f32>>,
}

/// Model show response from /api/show
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelShowResponse {
    pub model: String,
    pub modified_at: String,
    pub size: u64,
    pub digest: String,
    pub details: Option<serde_json::Value>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}
