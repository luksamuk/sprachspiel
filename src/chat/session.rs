//! Chat session management
//!
//! Handles the state of a chat session including messages, model, and metadata.

use chrono::{DateTime, Utc};
use ollama_rs::generation::chat::ChatMessage;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

use super::history::{ConversationStorage, SessionInfo};
use super::todo_state::TodoState;
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

    /// Create a new named session
    #[allow(dead_code)]
    pub fn new_named(
        name: String,
        model: String,
        project_id: Option<String>,
        anonymous: bool,
    ) -> Self {
        let mut session = Self::new(model, project_id, anonymous);
        session.name = Some(name);
        session
    }

    /// Load a session from storage
    pub fn load(
        storage: &ConversationStorage,
        project_id: &Option<String>,
        session_id: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        storage.load_session(project_id, session_id)
    }

    /// Save the session to storage
    pub fn save(
        &self,
        storage: &ConversationStorage,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.anonymous {
            return Ok(());
        }
        storage.save_session(&self.project_id, &self.id, self)
    }

    /// Add a user message to the session
    /// 
    /// If database is attached, saves to SQLite immediately and generates
    /// embedding asynchronously (fire-and-forget).
    pub fn add_user_message(&mut self, content: String) {
        let now = Utc::now();
        
        // Add to memory (immediate)
        self.messages.push(SavedMessage {
            role: MessageRole::User,
            content: content.clone(),
            timestamp: now,
        });
        self.updated_at = now;
        
        // Save to SQLite if database is attached (immediate)
        if !self.anonymous
            && let Some(ref db) = self.db
        {
            // Ensure conversation exists before inserting message
            self.ensure_conversation_exists();

            match db.insert_message(&self.id, "user", &content, now) {
                Ok(message_id) => {
                    // Insert chunks synchronously (guaranteed persistence)
                    // Generate embeddings asynchronously (can be recovered on restart)
                    if let Some(ref client) = self.embedding_client {
                        let client = Arc::clone(client);
                        let db = Arc::clone(db);
                        let conv_id = self.id.clone();
                        let timestamp = now;
                        let content = content.clone();

                        // Check if chunking needed and insert chunks synchronously
                        let chunk_data = if crate::embeddings::needs_chunking(&content) {
                            let chunks = crate::embeddings::chunk_text(&content);
                            let mut data = Vec::new();
                            for chunk in &chunks {
                                match db.insert_chunk(
                                    message_id,
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
                                        let _ = db.update_chunk_embedding(
                                            chunk_id,
                                            &embedding,
                                            &conv_id,
                                            timestamp,
                                        );
                                    }
                                }
                            } else {
                                if let Ok(embedding) = client.embed(&content).await {
                                    let _ = db.update_message_embedding(
                                        message_id,
                                        &embedding,
                                        &conv_id,
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

    /// Add an assistant message to the session
    /// 
    /// If database is attached, saves to SQLite immediately.
    /// Applies chunking for long messages (>1024 chars).
    pub fn add_assistant_message(&mut self, content: String) {
        let now = Utc::now();
        
        // Add to memory (immediate)
        self.messages.push(SavedMessage {
            role: MessageRole::Assistant,
            content: content.clone(),
            timestamp: now,
        });
        self.updated_at = now;
        
        // Save to SQLite if database is attached (immediate)
        if !self.anonymous
            && let Some(ref db) = self.db
        {
            // Ensure conversation exists before inserting message
            self.ensure_conversation_exists();

            match db.insert_message(&self.id, "assistant", &content, now) {
                Ok(message_id) => {
                    // Insert chunks synchronously (guaranteed persistence)
                    // Generate embeddings asynchronously (can be recovered on restart)
                    if let Some(ref client) = self.embedding_client {
                        let client = Arc::clone(client);
                        let db = Arc::clone(db);
                        let conv_id = self.id.clone();
                        let timestamp = now;
                        let content = content.clone();

                        // Check if chunking needed and insert chunks synchronously
                        let chunk_data = if crate::embeddings::needs_chunking(&content) {
                            let chunks = crate::embeddings::chunk_text(&content);
                            let mut data = Vec::new();
                            for chunk in &chunks {
                                match db.insert_chunk(
                                    message_id,
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
                                        let _ = db.update_chunk_embedding(
                                            chunk_id,
                                            &embedding,
                                            &conv_id,
                                            timestamp,
                                        );
                                    }
                                }
                            } else {
                                if let Ok(embedding) = client.embed(&content).await {
                                    let _ = db.update_message_embedding(
                                        message_id,
                                        &embedding,
                                        &conv_id,
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
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        // Preserved: compacted_summary, compacted_range
        // These allow RAG to work after clear
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
        let mut removed = 0;
        while let Some(last) = self.messages.last() {
            if last.role == MessageRole::Assistant {
                self.messages.pop();
                removed += 1;
            } else {
                break;
            }
        }
        if removed > 0 {
            self.updated_at = Utc::now();
        }
        removed
    }

    /// Get the last user message (for retry functionality)
    pub fn get_last_user_message(&self) -> Option<&SavedMessage> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
    }

    /// Set the compacted summary and update the LLM message index (full compaction)
    ///
    /// This is the legacy API for full compaction. Prefer `set_compacted_summary_with_range()`
    /// for middle compaction support (preserves first N and last N messages).
    ///
    /// Use this only when you want to compact ALL messages (no preservation).
    #[allow(dead_code)]
    pub fn set_compacted_summary(&mut self, summary: String) {
        self.compacted_summary = Some(summary);
        self.messages_sent_to_llm = self.messages.len();
        self.compacted_range = Some((0, self.messages.len()));
        self.updated_at = Utc::now();
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
        self.updated_at = Utc::now();
    }

    /// Clear the compacted summary (send full history to LLM)
    #[allow(dead_code)]
    pub fn clear_compacted_summary(&mut self) {
        self.compacted_summary = None;
        self.messages_sent_to_llm = 0;
        self.compacted_range = None;
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

    /// Get messages as ChatMessage for the API (full history)
    #[allow(dead_code)]
    pub fn as_chat_messages(&self, system_prompt: &str) -> Vec<ChatMessage> {
        let mut messages = Vec::new();

        // Add system message
        let prompt = self.system_prompt.as_deref().unwrap_or(system_prompt);
        messages.push(ChatMessage::system(prompt.to_string()));

        // Add conversation history
        for msg in &self.messages {
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

    /// Convert to SessionInfo for listing
    #[allow(dead_code)]
    pub fn to_info(&self) -> SessionInfo {
        SessionInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            model: self.model.clone(),
            message_count: self.messages.len(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
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
        });
        session.set_compacted_summary_with_range("Summary of conversation".into(), Some((0, 1)));
        
        // Verify setup
        assert_eq!(session.messages.len(), 1);
        assert!(session.compacted_summary.is_some());
        
        // Clear
        session.clear_messages();
        
        // Verify
        assert!(session.messages.is_empty());  // Messages cleared
        assert!(session.compacted_summary.is_some());  // Summary PRESERVED!
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
        });
        session.set_compacted_summary_with_range("Summary".into(), Some((0, 1)));
        
        // Verify setup
        assert_eq!(session.messages.len(), 1);
        assert!(session.compacted_summary.is_some());
        
        // Forget
        session.forget_session();
        
        // Verify
        assert!(session.messages.is_empty());  // Messages cleared
        assert!(session.compacted_summary.is_none());  // Summary CLEARED!
        assert!(session.compacted_range.is_none());  // Range CLEARED!
        assert_eq!(session.messages_sent_to_llm, 0);
    }
    
    #[test]
    fn test_clear_vs_forget_difference() {
        let mut session = ChatSession::new("test-model".into(), None, false);
        
        // Add messages and summary
        session.messages.push(SavedMessage {
            role: MessageRole::User,
            content: "Message 1".into(),
            timestamp: Utc::now(),
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
        });
        
        // Test forget_session clears everything
        session.forget_session();
        assert!(session.messages.is_empty());
        assert!(session.compacted_summary.is_none());
        assert!(session.compacted_range.is_none());
    }
}
