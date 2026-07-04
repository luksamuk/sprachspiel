//! OpenAI API request/response serde structs.
//!
//! Covers the subset of the OpenAI spec used by sprachspiel:
//! - `/v1/chat/completions` (chat + streaming + tool calling)
//! - `/v1/embeddings`
//! - `/v1/models` (capability detection)
//!
//! Compatible with:
//! - OpenAI (`https://api.openai.com/v1`)
//! - Ollama OpenAI-compat (`http://localhost:11434/v1`)
//! - llama.cpp server (via llama-swap)
//! - vLLM (via llama-swap)
//! - LM Studio
//! - Any other OpenAI-spec server
//!
//! Note: Only the standard OpenAI-spec fields are used. Non-standard
//! extensions (`top_k`, `repeat_penalty`, `num_ctx` etc.) are NOT sent
//! — they are not portable across providers (see ollama/ollama#11325).

use serde::{Deserialize, Serialize};

/// OpenAI chat message (request/response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIMessage {
    /// Sender role. Optional in streaming deltas (OpenAI sends the role
    /// only on the very first chunk; subsequent chunks omit it). Required
    /// for non-streaming responses and request bodies.
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    /// Reasoning / thinking content emitted in streaming deltas by some
    /// OpenAI-compat providers (llama-swap, DeepSeek, Qwen3-thinking,
    /// etc.). Not part of the strict OpenAI spec but widely supported.
    /// On the request side, leave `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Tool calls issued by the assistant. Present in assistant messages
    /// when the model decides to call a function.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
    /// For tool role messages: the id of the tool call this is responding to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// OpenAI tool call.
///
/// In streaming responses, only the first chunk for a given tool call
/// carries the `id` and `function.name`; subsequent chunks carry
/// incremental `function.arguments` fragments and the same `index` to
/// identify the tool call they extend. Some providers (e.g. llama-swap
/// proxying local llama.cpp) may omit `id`/`type` on continuation
/// chunks. We therefore deserialize all fields as optional/default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIToolCall {
    /// Index of the tool call in the response. OpenAI streams this
    /// on every chunk. Used to correlate continuation chunks to the
    /// first chunk of a tool call.
    #[serde(default)]
    pub index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<String>,
    #[serde(default)]
    pub function: OpenAIToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenAIToolCallFunction {
    /// Function name. Set on the first chunk for each tool call.
    /// Empty string on continuation chunks.
    #[serde(default)]
    pub name: String,
    /// Arguments as a JSON-encoded string (OpenAI-spec: arguments is a string,
    /// unlike Ollama native which uses a JSON object). Empty on the first
    /// chunk; populated incrementally across continuation chunks.
    #[serde(default)]
    pub arguments: String,
}

/// OpenAI tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAITool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAIToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Chat completion request body.
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OpenAITool>>,
    #[serde(default)]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamOptions {
    pub include_usage: bool,
}

/// Non-streaming chat completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: u32,
    pub message: OpenAIMessage,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// Number of tokens in the prompt. OpenAI sends this for
    /// both /v1/chat/completions and /v1/embeddings. Some
    /// non-canonical servers (e.g., llama.cpp older versions) may
    /// omit it; default to 0 to keep the parse robust.
    #[serde(default)]
    pub prompt_tokens: u32,
    /// Number of tokens in the completion. OpenAI sends this
    /// only for /v1/chat/completions — /v1/embeddings
    /// responses omit it. Default to 0.
    #[serde(default)]
    pub completion_tokens: u32,
    /// Total tokens consumed. Always present.
    #[serde(default)]
    pub total_tokens: u32,
}

/// Streaming chunk (Server-Sent Event).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[allow(dead_code)] // Used indirectly: deserialized via ChatChunk.choices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: OpenAIMessage,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// Embeddings request body.
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingsRequest {
    pub model: String,
    pub input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    #[serde(default)]
    pub encoding_format: String,
}

/// Embeddings response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsResponse {
    pub object: String,
    pub data: Vec<EmbeddingObject>,
    pub model: String,
    pub usage: Usage,
}

#[allow(dead_code)] // Used indirectly: deserialized via EmbeddingsResponse.data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingObject {
    /// OpenAI sends `"object": "embedding"` per-item. llama-swap
    /// and some other servers omit this field; default to empty
    /// so the parse doesn't fail when the server's serialization
    /// is slightly non-canonical.
    #[serde(default)]
    pub object: String,
    pub embedding: Vec<f32>,
    /// OpenAI sends `"index": 0, 1, ...` per-item. llama-swap and
    /// some other servers omit this field; default to 0.
    #[serde(default)]
    pub index: u32,
}

/// Models list response (for capability detection).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

#[allow(dead_code)] // Used indirectly: deserialized via ModelsResponse.data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    #[serde(default)]
    pub owned_by: Option<String>,
    /// Some servers include this field with model metadata including
    /// context length information.
    #[serde(default)]
    pub max_model_len: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_serde_missing_completion_tokens() {
        // W2 #121 extension: llama-swap (and other OpenAI-spec
        // embeddings servers) omit the `completion_tokens` field
        // from the usage block. The strict-verify probe must
        // parse these responses without failing. Also covers
        // `prompt_tokens` and `total_tokens` being omitted by
        // non-canonical servers.
        let body = r#"{"prompt_tokens":3,"total_tokens":3}"#;
        let usage: Usage = serde_json::from_str(body).unwrap();
        assert_eq!(usage.prompt_tokens, 3);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 3);
    }

    #[test]
    fn test_embedding_object_optional_fields() {
        // W2 #121 extension: llama-swap may omit `object` and
        // `index` per-item. The strict-verify probe must parse
        // these responses without failing.
        let body = r#"{"embedding":[0.1, 0.2, 0.3]}"#;
        let emb: EmbeddingObject = serde_json::from_str(body).unwrap();
        assert_eq!(emb.embedding, vec![0.1, 0.2, 0.3]);
        assert_eq!(emb.object, ""); // default
        assert_eq!(emb.index, 0); // default
    }

    #[test]
    fn test_embeddings_response_full_openai() {
        // W2 #121 extension: full OpenAI-spec embeddings response
        // (with `object`, `index`, `completion_tokens`).
        let body = r#"{
            "object": "list",
            "data": [
                {"object": "embedding", "index": 0, "embedding": [0.1, 0.2, 0.3]}
            ],
            "model": "nomic-embed-text-v2-moe",
            "usage": {"prompt_tokens": 3, "completion_tokens": 0, "total_tokens": 3}
        }"#;
        let resp: EmbeddingsResponse = serde_json::from_str(body).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].embedding.len(), 3);
        assert_eq!(resp.data[0].index, 0);
    }

    #[test]
    fn test_embeddings_response_minimal_llama_swap() {
        // W2 #121 extension: minimal llama-swap response (no
        // `completion_tokens`, no per-item `object`/`index`).
        // This is the format returned by llama.cpp's OpenAI
        // embeddings endpoint.
        let body = r#"{
            "model": "nomic-embed-text-v2-moe",
            "object": "list",
            "usage": {"prompt_tokens": 3, "total_tokens": 3},
            "data": [{"embedding": [0.1, 0.2, 0.3]}]
        }"#;
        let resp: EmbeddingsResponse = serde_json::from_str(body).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].embedding.len(), 3);
        assert_eq!(resp.data[0].index, 0); // default
        assert_eq!(resp.usage.completion_tokens, 0); // default
    }
}
