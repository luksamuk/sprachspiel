//! Chat session management
//!
//! Handles the state of a chat session including messages, model, and metadata.

use chrono::{DateTime, Utc};
use ollama_rs::generation::chat::ChatMessage;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

use super::todo_state::TodoState;
use crate::consts::roles::{ROLE_ASSISTANT, ROLE_USER};
use crate::db::Database;
use crate::embeddings::EmbeddingClient;

/// Tool output verbosity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ToolOutputLevel {
    /// Show compact tool call info (one line per call)
    #[default]
    Compact,
    /// Show full tool call details
    Full,
    /// Hide tool call output
    Hidden,
}

impl std::fmt::Display for ToolOutputLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolOutputLevel::Compact => write!(f, "compact"),
            ToolOutputLevel::Full => write!(f, "full"),
            ToolOutputLevel::Hidden => write!(f, "hidden"),
        }
    }
}

impl std::str::FromStr for ToolOutputLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "compact" | "c" => Ok(ToolOutputLevel::Compact),
            "full" | "f" => Ok(ToolOutputLevel::Full),
            "hidden" | "h" | "none" | "off" => Ok(ToolOutputLevel::Hidden),
            _ => Err(format!(
                "Invalid tool output level: '{}'. Use: compact, full, or hidden",
                s
            )),
        }
    }
}

/// Represents a single chat session
#[derive(Clone, Serialize, Deserialize)]
pub struct ChatSession {
    /// Unique session identifier
    pub id: String,
    /// Optional session name for easy reference
    pub name: Option<String>,
    /// Project identifier (git remote or folder name)
    pub project_id: Option<String>,
    /// Model preset name
    pub model: String,
    /// Custom system prompt (if any)
    pub system_prompt: Option<String>,
    /// Conversation history (full, never removed)
    pub messages: Vec<SavedMessage>,
    /// Compacted summary of old messages (for LLM context)
    #[serde(default)]
    pub compacted_summary: Option<String>,
    /// Range of compacted messages (middle compaction)
    ///
    /// If Some((first_preserved, last_preserved_start)):
    ///   - [..first_preserved] = preserved at start
    ///   - [first_preserved..last_preserved_start] = in summary
    ///   - [last_preserved_start..] = preserved at end
    ///
    /// If None: no compaction (use messages_sent_to_llm for legacy compatibility)
    #[serde(default)]
    pub compacted_range: Option<(usize, usize)>,
    /// Index of first message to send to LLM (after compacted portion)
    /// Deprecated: Use compacted_range for middle compaction
    #[serde(default)]
    pub messages_sent_to_llm: usize,
    /// Session creation time
    pub created_at: DateTime<Utc>,
    /// Last update time
    pub updated_at: DateTime<Utc>,
    /// Whether this is an anonymous session (not persisted)
    #[serde(default)]
    pub anonymous: bool,
    /// Whether thinking mode is enabled
    #[serde(default)]
    pub think: bool,
    /// Whether tools are enabled
    #[serde(default)]
    pub tools: bool,
    /// Tool output verbosity level
    #[serde(default)]
    pub tool_output_level: ToolOutputLevel,
    /// Todo list state for task tracking
    #[serde(default)]
    pub todos: TodoState,
    /// Database for message persistence (not serializable)
    #[serde(skip)]
    pub db: Option<Arc<Database>>,
    /// Embedding client for semantic search (not serializable)
    #[serde(skip)]
    pub embedding_client: Option<Arc<EmbeddingClient>>,
    /// Whether auto-retrieval is enabled
    #[serde(skip)]
    pub retrieval_enabled: bool,
    /// Last time retrieval was performed (for throttling)
    #[serde(skip)]
    pub last_retrieval_time: Option<Instant>,
}

/// A saved message for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    /// Prompt tokens used in this interaction (real count from Ollama)
    #[serde(default)]
    pub prompt_tokens: Option<u64>,
    /// Message type: "normal" or "pre_tool_content"
    #[serde(default)]
    pub message_type: Option<String>,
}

impl Default for SavedMessage {
    fn default() -> Self {
        Self {
            role: MessageRole::User,
            content: String::new(),
            timestamp: Utc::now(),
            prompt_tokens: None,
            message_type: None,
        }
    }
}

/// Message role
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl ChatSession {
    /// Create a new session with default values
    pub fn new(model: String, project_id: Option<String>, anonymous: bool) -> Self {
        let now = Utc::now();
        Self {
            id: if anonymous {
                String::new() // Anonymous sessions don't need an ID
            } else {
                // Use "default" as the default session ID for persistence
                "default".to_string()
            },
            name: None,
            project_id,
            model,
            system_prompt: None,
            messages: Vec::new(),
            compacted_summary: None,
            compacted_range: None,
            messages_sent_to_llm: 0,
            created_at: now,
            updated_at: now,
            anonymous,
            think: false,
            tools: true,
            tool_output_level: ToolOutputLevel::default(),
            todos: TodoState::new(),
            db: None,
            embedding_client: None,
            retrieval_enabled: true,
            last_retrieval_time: None,
        }
    }

    /// Load a session from SQLite database
    pub fn load_sqlite(
        db: &Arc<Database>,
        conversation_id: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let meta = db.get_conversation_metadata(conversation_id)?;
        let items = db.get_conversation_items(conversation_id)?;
        let todo_rows = db.get_todos(conversation_id)?;

        let saved_messages: Vec<SavedMessage> = items
            .into_iter()
            .filter_map(|item| {
                if item.content_type != crate::content::types::ContentType::Message {
                    return None;
                }
                Some(SavedMessage {
                    role: match item.role.as_deref()? {
                        "user" => MessageRole::User,
                        "assistant" => MessageRole::Assistant,
                        "system" => MessageRole::System,
                        _ => MessageRole::Tool,
                    },
                    content: item.content,
                    timestamp: item.created_at,
                    prompt_tokens: item.prompt_tokens.map(|t| t as u64),
                    message_type: item.message_type,
                })
            })
            .collect();

        let todos = TodoState::from_rows(&todo_rows);

        Ok(Self {
            id: meta.id,
            name: meta.name,
            project_id: meta.project_id,
            model: meta.model,
            system_prompt: meta.system_prompt,
            messages: saved_messages,
            compacted_summary: meta.compacted_summary,
            compacted_range: meta.compacted_range,
            messages_sent_to_llm: meta.compacted_range.map(|(_, end)| end).unwrap_or(0),
            created_at: meta.created_at,
            updated_at: meta.updated_at,
            anonymous: false,
            think: meta.think,
            tools: meta.tools,
            tool_output_level: meta.tool_output_level.parse().unwrap_or_default(),
            todos,
            db: Some(Arc::clone(db)),
            embedding_client: None,
            retrieval_enabled: true,
            last_retrieval_time: None,
        })
    }

    /// Save session metadata to SQLite
    ///
    /// Note: Messages are already saved to SQLite by add_user_message() and
    /// add_assistant_message(). This only saves session metadata and todos.
    pub fn save_sqlite(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.anonymous {
            return Ok(());
        }

        let db = self.db.as_ref().ok_or("No database attached to session")?;

        // Update conversation metadata
        db.update_conversation_metadata(&crate::db::ConversationMetadataParams {
            id: &self.id,
            name: self.name.as_deref(),
            system_prompt: self.system_prompt.as_deref(),
            compacted_summary: self.compacted_summary.as_deref(),
            compacted_range: self.compacted_range,
            think: self.think,
            tools: self.tools,
            tool_output_level: &self.tool_output_level.to_string(),
            updated_at: self.updated_at,
        })?;

        // Save todos
        let todo_rows = self.todos.to_rows();
        db.save_todos(&self.id, &todo_rows)?;

        Ok(())
    }

    /// Add a user message to the session
    ///
    /// If database is attached, saves to SQLite immediately.
    /// Applies chunking for long messages (>1024 chars).
    ///
    /// Returns the message ID if saved to database, None otherwise.
    pub fn add_user_message(&mut self, content: String) -> Option<i64> {
        let now = Utc::now();

        // Add to memory (immediate)
        self.messages.push(SavedMessage {
            role: MessageRole::User,
            content: content.clone(),
            timestamp: now,
            ..Default::default()
        });
        self.updated_at = now;

        // Save to SQLite if database is attached (immediate)
        if !self.anonymous
            && let Some(ref db) = self.db
        {
            // Ensure conversation exists before inserting message
            self.ensure_conversation_exists();

            match db.insert_content_item(
                "message",
                Some(&self.id),
                Some(ROLE_USER),
                None,
                None,
                None,
                None,
                None,
                None,
                &content,
                0.5,
                self.project_id.as_deref(),
                now,
            ) {
                Ok(item_id) => {
                    // Insert chunks synchronously (guaranteed persistence)
                    // Generate embeddings asynchronously (can be recovered on restart)
                    if let Some(ref client) = self.embedding_client {
                        let client = Arc::clone(client);
                        let db = Arc::clone(db);
                        let conv_id = self.id.clone();
                        let timestamp = now;
                        let content = content.clone();
                        let project_id = self.project_id.clone();

                        // Check if chunking needed and insert chunks synchronously
                        let chunk_data = if crate::embeddings::needs_chunking(&content) {
                            let chunks = crate::embeddings::chunk_text(&content);
                            let mut data = Vec::new();
                            for chunk in &chunks {
                                match db.insert_content_chunk(
                                    item_id,
                                    chunk.index as i32,
                                    &chunk.content,
                                    chunk.start_offset as i32,
                                    chunk.end_offset as i32,
                                    timestamp,
                                ) {
                                    Ok(chunk_id) => {
                                        data.push((chunk_id, chunk.content.clone()));
                                    }
                                    Err(e) => {
                                        eprintln!("Warning: Failed to insert chunk: {}", e);
                                    }
                                }
                            }
                            data
                        } else {
                            vec![]
                        };

                        // Generate embeddings asynchronously (can be interrupted, will be recovered)
                        tokio::spawn(async move {
                            if !chunk_data.is_empty() {
                                for (chunk_id, content) in chunk_data {
                                    if let Ok(embedding) = client.embed(&content).await {
                                        let _ = db.update_content_chunk_embedding(
                                            chunk_id,
                                            &embedding,
                                            "message",
                                            Some(&conv_id),
                                            project_id.as_deref(),
                                            timestamp,
                                        );
                                    }
                                }
                            } else {
                                if let Ok(embedding) = client.embed(&content).await {
                                    let _ = db.update_content_item_embedding(
                                        item_id,
                                        &embedding,
                                        "message",
                                        Some(&conv_id),
                                        project_id.as_deref(),
                                        timestamp,
                                    );
                                }
                            }
                        });
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Could not save message to database: {}", e);
                }
            }
        }

        None
    }

    /// Add an assistant message to the session
    ///
    /// If database is attached, saves to SQLite immediately.
    /// Applies chunking for long messages (>1024 chars).
    pub fn add_assistant_message(&mut self, content: String, prompt_tokens: Option<u64>) {
        let now = Utc::now();

        // Add to memory (immediate)
        self.messages.push(SavedMessage {
            role: MessageRole::Assistant,
            content: content.clone(),
            timestamp: now,
            prompt_tokens,
            message_type: None,
        });
        self.updated_at = now;

        // Save to SQLite if database is attached (immediate)
        if !self.anonymous
            && let Some(ref db) = self.db
        {
            // Ensure conversation exists before inserting message
            self.ensure_conversation_exists();

            match db.insert_content_item(
                "message",
                Some(&self.id),
                Some(ROLE_ASSISTANT),
                None,
                None,
                prompt_tokens.map(|t| t as i64),
                None,
                None,
                None,
                &content,
                0.5,
                self.project_id.as_deref(),
                now,
            ) {
                Ok(item_id) => {
                    // Insert chunks synchronously (guaranteed persistence)
                    // Generate embeddings asynchronously (can be recovered on restart)
                    if let Some(ref client) = self.embedding_client {
                        let client = Arc::clone(client);
                        let db = Arc::clone(db);
                        let conv_id = self.id.clone();
                        let timestamp = now;
                        let content = content.clone();
                        let project_id = self.project_id.clone();

                        // Check if chunking needed and insert chunks synchronously
                        let chunk_data = if crate::embeddings::needs_chunking(&content) {
                            let chunks = crate::embeddings::chunk_text(&content);
                            let mut data = Vec::new();
                            for chunk in &chunks {
                                match db.insert_content_chunk(
                                    item_id,
                                    chunk.index as i32,
                                    &chunk.content,
                                    chunk.start_offset as i32,
                                    chunk.end_offset as i32,
                                    timestamp,
                                ) {
                                    Ok(chunk_id) => {
                                        data.push((chunk_id, chunk.content.clone()));
                                    }
                                    Err(e) => {
                                        eprintln!("Warning: Failed to insert chunk: {}", e);
                                    }
                                }
                            }
                            data
                        } else {
                            vec![]
                        };

                        // Generate embeddings asynchronously (can be interrupted, will be recovered)
                        tokio::spawn(async move {
                            if !chunk_data.is_empty() {
                                for (chunk_id, content) in chunk_data {
                                    if let Ok(embedding) = client.embed(&content).await {
                                        let _ = db.update_content_chunk_embedding(
                                            chunk_id,
                                            &embedding,
                                            "message",
                                            Some(&conv_id),
                                            project_id.as_deref(),
                                            timestamp,
                                        );
                                    }
                                }
                            } else {
                                if let Ok(embedding) = client.embed(&content).await {
                                    let _ = db.update_content_item_embedding(
                                        item_id,
                                        &embedding,
                                        "message",
                                        Some(&conv_id),
                                        project_id.as_deref(),
                                        timestamp,
                                    );
                                }
                            }
                        });
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Could not save message to database: {}", e);
                }
            }
        }
    }

    /// Add a pre-tool content message to the database
    ///
    /// This stores intermediate assistant content generated before tool calls.
    /// Unlike regular messages, these are stored only in the database for
    /// semantic search, NOT in the in-memory session history.
    ///
    /// # Arguments
    /// * `content` - The pre-tool content (thinking/intro text)
    /// * `thinking_content` - Optional thinking content
    /// * `previous_message_id` - ID of the user message this responds to
    ///
    /// # Returns
    /// The message ID if saved successfully
    pub fn add_pre_tool_message(
        &mut self,
        content: String,
        thinking_content: Option<String>,
        previous_message_id: Option<i64>,
    ) -> Option<i64> {
        if self.anonymous {
            return None;
        }

        let now = Utc::now();
        let db = self.db.as_ref()?;

        // Ensure conversation exists
        self.ensure_conversation_exists();

        // Combine thinking and content for storage
        let full_content = if let Some(thinking) = thinking_content {
            format!("<thinking>\n{}\n</thinking>\n\n{}", thinking, content)
        } else {
            content
        };

        // Insert with message_type = "pre_tool_content"
        match db.insert_content_item(
            "message",
            Some(&self.id),
            Some(ROLE_ASSISTANT),
            Some("pre_tool_content"),
            previous_message_id,
            None,
            None,
            None,
            None,
            &full_content,
            0.5,
            self.project_id.as_deref(),
            now,
        ) {
            Ok(item_id) => {
                // Generate embedding asynchronously
                if let Some(ref client) = self.embedding_client {
                    let client = Arc::clone(client);
                    let db = Arc::clone(db);
                    let conv_id = self.id.clone();
                    let timestamp = now;
                    let content_clone = full_content.clone();
                    let project_id = self.project_id.clone();

                    tokio::spawn(async move {
                        if crate::embeddings::needs_chunking(&content_clone) {
                            let chunks = crate::embeddings::chunk_text(&content_clone);
                            for chunk in &chunks {
                                if let Ok(chunk_id) = db.insert_content_chunk(
                                    item_id,
                                    chunk.index as i32,
                                    &chunk.content,
                                    chunk.start_offset as i32,
                                    chunk.end_offset as i32,
                                    timestamp,
                                ) && let Ok(embedding) = client.embed(&chunk.content).await
                                {
                                    let _ = db.update_content_chunk_embedding(
                                        chunk_id,
                                        &embedding,
                                        "message",
                                        Some(&conv_id),
                                        project_id.as_deref(),
                                        timestamp,
                                    );
                                }
                            }
                        } else if let Ok(embedding) = client.embed(&content_clone).await {
                            let _ = db.update_content_item_embedding(
                                item_id,
                                &embedding,
                                "message",
                                Some(&conv_id),
                                project_id.as_deref(),
                                timestamp,
                            );
                        }
                    });
                }

                Some(item_id)
            }
            Err(e) => {
                eprintln!("Warning: Could not save pre-tool message: {}", e);
                None
            }
        }
    }

    /// Attach database and embedding client for persistence
    pub fn attach_db(&mut self, db: Arc<Database>, embedding_client: Arc<EmbeddingClient>) {
        self.db = Some(db);
        self.embedding_client = Some(embedding_client);
    }

    /// Ensure conversation exists in database (call before first message insert)
    pub fn ensure_conversation_exists(&self) {
        if self.anonymous {
            return;
        }
        if let Some(ref db) = self.db {
            let title = self.name.as_deref().unwrap_or(&self.id);
            if let Err(e) = db.insert_conversation(
                &self.id,
                self.project_id.as_deref(),
                Some(title),
                &self.model,
                self.created_at,
                self.updated_at,
            ) {
                eprintln!("Warning: Could not ensure conversation exists: {}", e);
            }
        }
    }

    /// Set the system prompt
    pub fn set_system_prompt(&mut self, prompt: String) {
        self.system_prompt = Some(prompt);
        self.updated_at = Utc::now();
    }

    /// Clear messages for a new topic, preserving conversation context
    ///
    /// This clears the message history but preserves the compacted summary,
    /// allowing the conversation context to persist for retrieval.
    /// Use `/forget` for a complete session reset.
    ///
    /// # Preserved
    /// - compacted_summary
    /// - compacted_range
    /// - SQLite conversation history
    ///
    /// # Cleared
    /// - messages (in-memory)
    /// - messages_sent_to_llm
    /// - compacted_range (reset since messages are gone)
    ///
    /// # Preserved
    /// - compacted_summary (for RAG to still work)
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        // Reset compacted_range since we no longer have those messages
        // This prevents range start > messages.len() panics
        self.compacted_range = None;
        self.messages_sent_to_llm = 0;
        self.updated_at = Utc::now();
    }

    /// Forget all context completely (new conversation, no history)
    ///
    /// Clears everything:
    /// - All messages
    /// - Compacted summary
    /// - Compacted range
    ///
    /// This is a destructive operation and cannot be undone.
    /// The caller is responsible for deleting from SQLite.
    pub fn forget_session(&mut self) {
        self.messages.clear();
        self.compacted_summary = None;
        self.compacted_range = None;
        self.messages_sent_to_llm = 0;
        self.updated_at = Utc::now();
    }

    /// Remove the last assistant message(s) for retry functionality
    /// Returns the number of messages removed
    pub fn remove_last_assistant_messages(&mut self) -> usize {
        self.remove_last_assistant_messages_with_content().0
    }

    /// Remove the last assistant message(s) and return content for cleanup
    /// Returns (count, Vec of assistant message contents)
    pub fn remove_last_assistant_messages_with_content(&mut self) -> (usize, Vec<String>) {
        let mut removed = 0;
        let mut contents = Vec::new();

        while let Some(last) = self.messages.last() {
            if last.role == MessageRole::Assistant {
                contents.push(last.content.clone());
                self.messages.pop();
                removed += 1;
            } else {
                break;
            }
        }

        // Also remove the preceding user message
        if let Some(last) = self.messages.last()
            && last.role == MessageRole::User
        {
            contents.push(last.content.clone());
            self.messages.pop();
            removed += 1;
        }

        if removed > 0 {
            self.updated_at = Utc::now();
        }
        (removed, contents)
    }

    /// Get the last user message (for retry functionality)
    pub fn get_last_user_message(&self) -> Option<&SavedMessage> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
    }

    /// Set the compacted summary with middle compaction (preserves first and last messages)
    pub fn set_compacted_summary_with_range(
        &mut self,
        summary: String,
        range: Option<(usize, usize)>,
    ) {
        self.compacted_summary = Some(summary);
        if let Some((first_preserved, last_preserved_start)) = range {
            self.compacted_range = Some((first_preserved, last_preserved_start));
            self.messages_sent_to_llm = last_preserved_start;
        } else {
            // Full compaction (backward compatible)
            self.compacted_range = Some((0, self.messages.len()));
            self.messages_sent_to_llm = self.messages.len();
        }

        // Clear prompt_tokens from all messages since they no longer reflect
        // the actual context size after compaction. The next message sent to
        // the LLM will have fresh prompt_tokens reflecting the reduced context.
        for msg in &mut self.messages {
            msg.prompt_tokens = None;
        }

        self.updated_at = Utc::now();
    }

    /// Check if there are compacted messages
    pub fn has_compacted_messages(&self) -> bool {
        self.compacted_summary.is_some()
            && (self.messages_sent_to_llm > 0 || self.compacted_range.is_some())
    }

    /// Get the number of compacted messages
    pub fn compacted_message_count(&self) -> usize {
        self.messages_sent_to_llm
    }

    /// Get real token count from the most recent prompt evaluation
    ///
    /// IMPORTANT: Ollama's prompt_eval_count is CUMULATIVE - it includes:
    /// - System prompt
    /// - Tool definitions (if any)
    /// - ALL conversation history
    /// - Current user message
    ///
    /// We return the most recent value as the total prompt size.
    /// For context display purposes, callers should NOT add system + tools again.
    pub fn history_real_tokens(&self) -> usize {
        // Find the most recent message with prompt_tokens
        // This value is ALREADY cumulative from Ollama's prompt_eval_count
        let last_prompt_tokens = self
            .messages
            .iter()
            .rev()
            .filter_map(|m| m.prompt_tokens)
            .next();

        match last_prompt_tokens {
            Some(tokens) => tokens as usize,
            None => {
                // Fallback: estimate from message content when no real tokens available
                // This happens when loading from DB before first interaction
                let messages_tokens: usize = self
                    .messages
                    .iter()
                    .skip(self.messages_sent_to_llm)
                    .map(|m| {
                        crate::tokens::estimate_tokens(&m.content) + crate::tokens::MESSAGE_OVERHEAD
                    })
                    .sum();

                // Add estimated tokens from compacted summary if present
                let summary_tokens = self
                    .compacted_summary
                    .as_ref()
                    .map(|s| {
                        let word_count = s.split_whitespace().count();
                        (word_count as f32 * 1.3).ceil() as usize + crate::tokens::MESSAGE_OVERHEAD
                    })
                    .unwrap_or(0);

                messages_tokens + summary_tokens
            }
        }
    }

    /// Get messages to send to LLM (summary + recent messages)
    pub fn get_messages_for_llm(&self, system_prompt: &str) -> Vec<ChatMessage> {
        let mut messages = Vec::new();

        // Add system message
        let prompt = self.system_prompt.as_deref().unwrap_or(system_prompt);
        messages.push(ChatMessage::system(prompt.to_string()));

        // Add compacted summary as a system message if present
        if let Some(ref summary) = self.compacted_summary {
            messages.push(ChatMessage::system(format!(
                "Previous conversation summary:\n{}",
                summary
            )));
        }

        // Add messages since last compact (or all if no compact)
        let start_idx = self.messages_sent_to_llm;
        for msg in self.messages.iter().skip(start_idx) {
            match msg.role {
                MessageRole::User => {
                    messages.push(ChatMessage::user(msg.content.clone()));
                }
                MessageRole::Assistant => {
                    messages.push(ChatMessage::assistant(msg.content.clone()));
                }
                MessageRole::System => {
                    // System messages are handled separately
                }
                MessageRole::Tool => {
                    messages.push(ChatMessage::tool(msg.content.clone()));
                }
            }
        }

        messages
    }

    /// Update the model
    pub fn set_model(&mut self, model: String) {
        self.model = model;
        self.updated_at = Utc::now();
    }

    /// Rename the session
    pub fn rename(&mut self, name: String) {
        self.name = Some(name);
        self.updated_at = Utc::now();
    }
}

impl Default for ChatSession {
    fn default() -> Self {
        Self::new("llama3.1".to_string(), None, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clear_messages_preserves_summary() {
        let mut session = ChatSession::new("test-model".into(), None, false);

        // Setup
        session.messages.push(SavedMessage {
            role: MessageRole::User,
            content: "Test message".into(),
            timestamp: Utc::now(),
            ..Default::default()
        });
        session.set_compacted_summary_with_range("Summary of conversation".into(), Some((0, 1)));

        // Verify setup
        assert_eq!(session.messages.len(), 1);
        assert!(session.compacted_summary.is_some());

        // Clear
        session.clear_messages();

        // Verify
        assert!(session.messages.is_empty()); // Messages cleared
        assert!(session.compacted_summary.is_some()); // Summary PRESERVED!
        assert_eq!(session.messages_sent_to_llm, 0);
    }

    #[test]
    fn test_forget_session_clears_everything() {
        let mut session = ChatSession::new("test-model".into(), None, false);

        // Setup
        session.messages.push(SavedMessage {
            role: MessageRole::User,
            content: "Test message".into(),
            timestamp: Utc::now(),
            ..Default::default()
        });
        session.set_compacted_summary_with_range("Summary".into(), Some((0, 1)));

        // Verify setup
        assert_eq!(session.messages.len(), 1);
        assert!(session.compacted_summary.is_some());

        // Forget
        session.forget_session();

        // Verify
        assert!(session.messages.is_empty()); // Messages cleared
        assert!(session.compacted_summary.is_none()); // Summary CLEARED!
        assert!(session.compacted_range.is_none()); // Range CLEARED!
        assert_eq!(session.messages_sent_to_llm, 0);
    }

    #[test]
    fn test_clear_vs_forget_difference() {
        let mut session = ChatSession::new("test-model".into(), None, false);

        // Add messages and summary
        session.messages.push(SavedMessage {
            role: MessageRole::User,
            content: "Test message".into(),
            timestamp: Utc::now(),
            ..Default::default()
        });
        session.set_compacted_summary_with_range("Summary".into(), Some((0, 1)));

        // Test clear_messages preserves summary
        session.clear_messages();
        assert!(session.messages.is_empty());
        assert!(session.compacted_summary.is_some());

        // Add messages again
        session.messages.push(SavedMessage {
            role: MessageRole::User,
            content: "Message 2".into(),
            timestamp: Utc::now(),
            ..Default::default()
        });

        // Test forget_session clears everything
        session.forget_session();
        assert!(session.messages.is_empty());
        assert!(session.compacted_summary.is_none());
        assert!(session.compacted_range.is_none());
    }

    #[test]
    fn test_history_real_tokens_returns_cumulative() {
        let mut session = ChatSession::new("test-model".into(), None, false);

        // Add messages with cumulative prompt_tokens
        // These represent Ollama's prompt_eval_count which IS cumulative
        for i in 0..5 {
            session.messages.push(SavedMessage {
                role: MessageRole::User,
                content: format!("Message {}", i),
                timestamp: Utc::now(),
                // Cumulative: each value includes all previous messages + this one
                // 500 is the final total (system + tools + all history)
                prompt_tokens: Some(100 * (i + 1)), // 100, 200, 300, 400, 500
                ..Default::default()
            });
        }

        // history_real_tokens returns the LAST (most recent) cumulative value
        let tokens = session.history_real_tokens();
        assert_eq!(tokens, 500); // Last message's prompt_tokens
    }

    #[test]
    fn test_history_real_tokens_fallback_estimation() {
        let mut session = ChatSession::new("test-model".into(), None, false);

        // Add messages WITHOUT prompt_tokens (fallback to estimation)
        for i in 0..5 {
            session.messages.push(SavedMessage {
                role: MessageRole::User,
                content: format!("Message {} ", i), // Short message
                timestamp: Utc::now(),
                prompt_tokens: None, // No real tokens available
                ..Default::default()
            });
        }

        // Should estimate from message content
        let tokens = session.history_real_tokens();
        // Each "Message N " is ~2 words, so ~3 tokens each + 4 overhead = ~7 tokens each
        // 5 messages * 7 tokens = 35 tokens (rough estimate)
        assert!(tokens > 0, "Should have some estimated tokens");
        assert!(
            tokens < 100,
            "Should be reasonable estimate for short messages"
        );
    }

    #[test]
    fn test_history_real_tokens_with_compaction() {
        let mut session = ChatSession::new("test-model".into(), None, false);

        // Add messages with cumulative prompt_tokens
        for i in 0..5 {
            session.messages.push(SavedMessage {
                role: MessageRole::User,
                content: format!("Message {}", i),
                timestamp: Utc::now(),
                prompt_tokens: Some(100 * (i + 1)), // Cumulative: 100, 200, 300, 400, 500
                ..Default::default()
            });
        }

        // Set compaction - this clears prompt_tokens
        session.set_compacted_summary_with_range("Summary".into(), Some((2, 3)));

        // After compaction, prompt_tokens are cleared, so fallback estimation is used
        // messages_sent_to_llm = 3, so messages[3..] are counted (messages 3 and 4)
        let tokens = session.history_real_tokens();

        // Fallback: estimate from messages_sent_to_llm onwards + summary
        // Message 3 and 4 have ~7-8 tokens each + 2*MESSAGE_OVERHEAD + summary tokens
        assert!(
            tokens < 100,
            "Should use fallback estimation after compaction, got {}",
            tokens
        );
    }

    #[test]
    fn test_get_messages_for_llm_respects_compaction() {
        let mut session = ChatSession::new("test-model".into(), None, false);

        // Add 5 messages
        for i in 0..5 {
            session.messages.push(SavedMessage {
                role: MessageRole::User,
                content: format!("Message {}", i),
                timestamp: Utc::now(),
                ..Default::default()
            });
        }

        // Compact first 3 messages
        session.set_compacted_summary_with_range("Summary".into(), Some((0, 3)));

        // get_messages_for_llm should return system + summary + messages 3,4
        let messages = session.get_messages_for_llm("You are helpful.");

        // 1 system + 1 summary + 2 messages = 4
        assert_eq!(messages.len(), 4);

        // First is system
        assert!(messages[0].content.starts_with("You are helpful."));

        // Second is summary
        assert!(
            messages[1]
                .content
                .contains("Previous conversation summary")
        );

        // Remaining are messages
        assert_eq!(messages[2].content, "Message 3");
        assert_eq!(messages[3].content, "Message 4");
    }

    #[test]
    fn test_to_rows_and_from_rows() {
        use crate::chat::todo_state::TaskStatus;

        let mut todos = super::TodoState::new();
        todos.add("Task 1".to_string());
        todos.add("Task 2".to_string());
        todos.update_status(1, TaskStatus::Done).unwrap();

        let rows = todos.to_rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].description, "Task 1");
        assert_eq!(rows[0].status, "done");
        assert_eq!(rows[1].description, "Task 2");
        assert_eq!(rows[1].status, "pending");

        // Convert back
        let restored = super::TodoState::from_rows(&rows);
        assert_eq!(restored.tasks.len(), 2);
    }
}
