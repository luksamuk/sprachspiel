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
use crate::embeddings::{
    DEFAULT_CONTEXT_LENGTH, EmbedContext, EmbedItemContext, EmbeddingClient,
    embed_chunk_with_fallback, embed_item_with_fallback,
};

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
    /// Currently active skill (activated via /skill \<name\> command)
    #[serde(default)]
    pub active_skill: Option<ActiveSkill>,
    /// Channel sender for embedding progress updates.
    /// Background embedding tasks send (current, total) tuples to update the status bar.
    #[serde(skip)]
    pub embedding_tx: Option<crate::chat::app::EmbeddingProgressTx>,
    /// Channel sender for async system messages from background tasks.
    /// Background tasks (e.g., /reindex) send completion messages
    /// that appear in the TUI chat area.
    #[serde(skip)]
    pub async_message_tx: Option<crate::chat::app::AsyncMessageTx>,
    /// Whether a `/reindex --yes` is currently running.
    /// Prevents concurrent reindex operations which would conflict on the database.
    #[serde(skip)]
    pub is_reindexing: Arc<std::sync::atomic::AtomicBool>,
}

/// An active skill loaded via /skill \<name\> command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSkill {
    /// Skill name
    pub name: String,
    /// Full skill content
    pub content: String,
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
    /// Database item ID (in-memory only, not persisted)
    #[serde(skip)]
    pub item_id: Option<i64>,
}

impl Default for SavedMessage {
    fn default() -> Self {
        Self {
            role: MessageRole::User,
            content: String::new(),
            timestamp: Utc::now(),
            prompt_tokens: None,
            message_type: None,
            item_id: None,
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
            active_skill: None,
            embedding_tx: None,
            async_message_tx: None,
            is_reindexing: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Load a session from SQLite database by ID or name.
    ///
    /// First tries exact ID match. If not found, tries name (title) match.
    pub fn load_sqlite(
        db: &Arc<Database>,
        conversation_id: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let meta = db.get_conversation_by_id_or_name(conversation_id)?;
        let items = db.get_conversation_items(&meta.id)?;
        let todo_rows = db.get_todos(&meta.id)?;

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
                    item_id: Some(item.id),
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
            active_skill: None,
            embedding_tx: None,
            async_message_tx: None,
            is_reindexing: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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

        // Ensure conversation row exists before UPDATE and FK-dependent INSERTs.
        // Without this, update_conversation_metadata() silently affects 0 rows,
        // and save_todos() fails with FOREIGN KEY constraint (conversation_id
        // references conversations(id)).
        self.ensure_conversation_exists();

        // Update conversation metadata
        db.update_conversation_metadata(&crate::db::ConversationMetadataParams {
            id: &self.id,
            name: self.name.as_deref(),
            model: &self.model,
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
            item_id: None, // TODO: populated from DB below
            ..Default::default()
        });
        self.updated_at = now;

        let mut result_id: Option<i64> = None;

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
                    result_id = Some(item_id);
                    // Update the in-memory message with the item_id
                    if let Some(last) = self.messages.last_mut() {
                        last.item_id = Some(item_id);
                    }
                    // Insert chunks synchronously (guaranteed persistence)
                    // Generate embeddings asynchronously (can be recovered on restart)
                    if let Some(ref client) = self.embedding_client {
                        let client = Arc::clone(client);
                        let db = Arc::clone(db);
                        let conv_id = self.id.clone();
                        let timestamp = now;
                        let content = content.clone();
                        let project_id = self.project_id.clone();
                        let progress_tx = self.embedding_tx.clone();

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
                                        log::warn!("Failed to insert chunk: {}", e);
                                    }
                                }
                            }
                            data
                        } else {
                            vec![]
                        };

                        // Generate embeddings asynchronously (can be interrupted, will be recovered)
                        // Report progress to TUI status bar: (0,1) = starting
                        if let Some(ref tx) = progress_tx {
                            let _ = tx.send((0, 1));
                        }
                        tokio::spawn(async move {
                            if !chunk_data.is_empty() {
                                for (chunk_id, content) in chunk_data {
                                    // Use fallback for oversized content
                                    let ctx = EmbedContext {
                                        content: &content,
                                        item_id,
                                        chunk_id,
                                        content_type: "message",
                                        conversation_id: Some(&conv_id),
                                        project_id: project_id.as_deref(),
                                        timestamp,
                                    };
                                    let _ = embed_chunk_with_fallback(
                                        ctx,
                                        Arc::clone(&db),
                                        Arc::clone(&client),
                                        DEFAULT_CONTEXT_LENGTH,
                                        0,
                                    )
                                    .await;
                                }
                            } else {
                                // Use fallback for oversized content
                                let ctx = EmbedItemContext::new(
                                    &content,
                                    item_id,
                                    "message",
                                    Some(&conv_id),
                                    project_id.as_deref(),
                                );
                                let _ = embed_item_with_fallback(
                                    ctx,
                                    &db,
                                    &client,
                                    DEFAULT_CONTEXT_LENGTH,
                                )
                                .await;
                            }
                            // Signal completion to the TUI status bar
                            if let Some(ref tx) = progress_tx {
                                let _ = tx.send((1, 1));
                            }
                        });
                    }
                }
                Err(e) => {
                    log::warn!("Could not save message to database: {}", e);
                }
            }
        }

        result_id
    }

    /// Add an assistant message to the session
    ///
    /// Returns the message ID if saved to database, None otherwise.
    pub fn add_assistant_message(
        &mut self,
        content: String,
        prompt_tokens: Option<u64>,
    ) -> Option<i64> {
        let now = Utc::now();

        // Add to memory (immediate)
        self.messages.push(SavedMessage {
            role: MessageRole::Assistant,
            content: content.clone(),
            timestamp: now,
            prompt_tokens,
            message_type: None,
            item_id: None, // TODO: populated from DB below
        });
        self.updated_at = now;

        let mut result_id: Option<i64> = None;

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
                    result_id = Some(item_id);
                    // Update the in-memory message with the item_id
                    if let Some(last) = self.messages.last_mut() {
                        last.item_id = Some(item_id);
                    }
                    // Insert chunks synchronously (guaranteed persistence)
                    // Generate embeddings asynchronously (can be recovered on restart)
                    if let Some(ref client) = self.embedding_client {
                        let client = Arc::clone(client);
                        let db = Arc::clone(db);
                        let conv_id = self.id.clone();
                        let timestamp = now;
                        let content = content.clone();
                        let project_id = self.project_id.clone();
                        let progress_tx = self.embedding_tx.clone();

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
                                        log::warn!("Failed to insert chunk: {}", e);
                                    }
                                }
                            }
                            data
                        } else {
                            vec![]
                        };

                        // Generate embeddings asynchronously (can be interrupted, will be recovered)
                        // Report progress to TUI status bar: (0,1) = starting
                        if let Some(ref tx) = progress_tx {
                            let _ = tx.send((0, 1));
                        }
                        tokio::spawn(async move {
                            if !chunk_data.is_empty() {
                                for (chunk_id, content) in chunk_data {
                                    // Use fallback for oversized content
                                    let ctx = EmbedContext {
                                        content: &content,
                                        item_id,
                                        chunk_id,
                                        content_type: "message",
                                        conversation_id: Some(&conv_id),
                                        project_id: project_id.as_deref(),
                                        timestamp,
                                    };
                                    let _ = embed_chunk_with_fallback(
                                        ctx,
                                        Arc::clone(&db),
                                        Arc::clone(&client),
                                        DEFAULT_CONTEXT_LENGTH,
                                        0,
                                    )
                                    .await;
                                }
                            } else {
                                // Use fallback for oversized content
                                let ctx = EmbedItemContext::new(
                                    &content,
                                    item_id,
                                    "message",
                                    Some(&conv_id),
                                    project_id.as_deref(),
                                );
                                let _ = embed_item_with_fallback(
                                    ctx,
                                    &db,
                                    &client,
                                    DEFAULT_CONTEXT_LENGTH,
                                )
                                .await;
                            }
                            // Signal completion to the TUI status bar
                            if let Some(ref tx) = progress_tx {
                                let _ = tx.send((1, 1));
                            }
                        });
                    }
                }
                Err(e) => {
                    log::warn!("Could not save message to database: {}", e);
                }
            }
        }

        result_id
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
                    let progress_tx = self.embedding_tx.clone();

                    // Report progress to TUI status bar: (0,1) = starting
                    if let Some(ref tx) = progress_tx {
                        let _ = tx.send((0, 1));
                    }
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
                                ) {
                                    // Use fallback for oversized content
                                    let ctx = EmbedContext {
                                        content: &chunk.content,
                                        item_id,
                                        chunk_id,
                                        content_type: "message",
                                        conversation_id: Some(&conv_id),
                                        project_id: project_id.as_deref(),
                                        timestamp,
                                    };
                                    let _ = embed_chunk_with_fallback(
                                        ctx,
                                        Arc::clone(&db),
                                        Arc::clone(&client),
                                        DEFAULT_CONTEXT_LENGTH,
                                        0,
                                    )
                                    .await;
                                }
                            }
                        } else {
                            // Use fallback for oversized content
                            let ctx = EmbedItemContext::new(
                                &content_clone,
                                item_id,
                                "message",
                                Some(&conv_id),
                                project_id.as_deref(),
                            );
                            let _ =
                                embed_item_with_fallback(ctx, &db, &client, DEFAULT_CONTEXT_LENGTH)
                                    .await;
                        }
                        // Signal completion to the TUI status bar
                        if let Some(ref tx) = progress_tx {
                            let _ = tx.send((1, 1));
                        }
                    });
                }

                Some(item_id)
            }
            Err(e) => {
                log::warn!("Could not save pre-tool message: {}", e);
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
                log::warn!("Could not ensure conversation exists: {}", e);
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
    #[allow(dead_code)] // Used in tests
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

    /// Get recent User/Assistant exchanges for context display on session resume.
    ///
    /// Returns up to `count` exchanges (1 exchange = 1 User + 1 Assistant message),
    /// filtering out System and Tool messages. Results are in chronological order
    /// (oldest first), suitable for display as a "recent context" summary.
    ///
    /// Each exchange is represented as a tuple of `(user_content, assistant_content)`,
    /// where `assistant_content` is `None` if no assistant reply followed the user message.
    pub fn get_recent_exchanges(&self, count: usize) -> Vec<(SavedMessage, Option<SavedMessage>)> {
        // Build exchanges: walk through messages in order, pairing each
        // User message with the next Assistant message (if any).
        let mut all_exchanges: Vec<(SavedMessage, Option<SavedMessage>)> = Vec::new();
        let mut pending_user: Option<SavedMessage> = None;

        for msg in &self.messages {
            match msg.role {
                MessageRole::User => {
                    // If there's a pending user without an assistant, save it as incomplete
                    if let Some(user_msg) = pending_user.take() {
                        all_exchanges.push((user_msg, None));
                    }
                    pending_user = Some(msg.clone());
                }
                MessageRole::Assistant => {
                    if let Some(user_msg) = pending_user.take() {
                        all_exchanges.push((user_msg, Some(msg.clone())));
                    } else {
                        // Orphan assistant message — skip
                    }
                }
                MessageRole::System | MessageRole::Tool => {
                    // Filter out system and tool messages
                }
            }
        }

        // If there's a pending user message without a reply, include it
        if let Some(user_msg) = pending_user {
            all_exchanges.push((user_msg, None));
        }

        // Take the last `count` exchanges
        let start = all_exchanges.len().saturating_sub(count);
        all_exchanges.into_iter().skip(start).collect()
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
            // Only use real tokens if non-zero (0 means invalid/not-set)
            Some(tokens) if tokens > 0 => tokens as usize,
            _ => {
                // Fallback: estimate from message content when no real tokens available
                // This happens when loading from DB before first interaction
                // or when prompt_tokens is 0 or None
                //
                // IMPORTANT: If there's a compacted summary, only count:
                // - Summary tokens
                // - Active messages (from messages_sent_to_llm onwards)
                // NOT the compacted messages (they're already in the summary)

                // If compacted, count only active messages + summary
                // If not compacted, count ALL messages
                let (active_start, has_summary) = if self.has_compacted_messages() {
                    (self.messages_sent_to_llm, true)
                } else {
                    (0, false)
                };

                let messages_tokens: usize = self
                    .messages
                    .iter()
                    .skip(active_start)
                    .map(|m| {
                        crate::tokens::estimate_tokens(&m.content) + crate::tokens::MESSAGE_OVERHEAD
                    })
                    .sum();

                // Add estimated tokens from compacted summary if present
                let summary_tokens = if has_summary {
                    self.compacted_summary
                        .as_ref()
                        .map(|s| {
                            let word_count = s.split_whitespace().count();
                            (word_count as f32 * 1.3).ceil() as usize
                                + crate::tokens::MESSAGE_OVERHEAD
                        })
                        .unwrap_or(0)
                } else {
                    0
                };

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
        // history_real_tokens estimates: active messages + summary (NOT compacted messages)
        // messages_sent_to_llm = 3, so messages[3..] + summary
        let tokens = session.history_real_tokens();

        // Fallback: estimate only active messages + summary (NOT compacted ones)
        // messages_sent_to_llm = 3, so messages 3 and 4 (~14 tokens) + summary (~7 tokens) = ~21 tokens
        assert!(
            tokens > 10 && tokens < 50,
            "Should estimate active messages + summary only, got {}",
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

#[test]
fn test_history_real_tokens_full_compaction() {
    // Edge case: after full compaction, messages_sent_to_llm == messages.len()
    // Should still return summary_tokens
    let mut session = ChatSession::new("test-model".into(), None, false);

    // Add messages
    for i in 0..5 {
        session.messages.push(SavedMessage {
            role: MessageRole::User,
            content: format!("Message {}", i),
            timestamp: Utc::now(),
            ..Default::default()
        });
    }

    // Full compaction - this sets messages_sent_to_llm = messages.len()
    session.set_compacted_summary_with_range("Summary of conversation".into(), None);

    // After full compaction, prompt_tokens are cleared, so fallback estimation is used
    // messages_sent_to_llm = 5 (equals messages.len()), so skip(5) returns empty iterator
    // Should return summary_tokens only
    let tokens = session.history_real_tokens();

    // Summary has ~3 words, so ~4 tokens + 4 overhead = ~8 tokens
    assert!(
        tokens > 0 && tokens < 20,
        "Should have summary tokens, got {}",
        tokens
    );
}

#[test]
fn test_history_real_tokens_partial_compaction_preserves_first() {
    // Test middle compaction that preserves first and last messages
    let mut session = ChatSession::new("test-model".into(), None, false);

    // Add 5 messages
    for i in 0..5 {
        session.messages.push(SavedMessage {
            role: MessageRole::User,
            content: format!("Message number {}", i), // More words for better estimation
            timestamp: Utc::now(),
            prompt_tokens: None,
            ..Default::default()
        });
    }

    // Compact middle messages (0,1,2), preserve first (0) and last (4,5)
    // This means messages_sent_to_llm should be 3
    session.set_compacted_summary_with_range("Summary text here".into(), Some((0, 3)));

    // history_real_tokens should estimate:
    // - Summary tokens
    // - Active messages from index 3 onwards (messages 3 and 4)
    let tokens = session.history_real_tokens();

    // Summary ~3 words = ~4 tokens + 4 overhead = ~8 tokens
    // Message 3 and 4: each ~3 words = ~4 tokens + 4 overhead = ~8 tokens each = 16 tokens total
    // Total should be ~24 tokens
    assert!(
        tokens > 15 && tokens < 40,
        "Should have summary + active message tokens, got {}",
        tokens
    );
}

#[test]
fn test_history_real_tokens_empty_session() {
    // Edge case: empty session
    let session = ChatSession::new("test-model".into(), None, false);
    let tokens = session.history_real_tokens();
    assert_eq!(tokens, 0, "Empty session should have 0 tokens");
}

#[test]
fn test_history_real_tokens_all_compacted_no_summary() {
    // Edge case: all messages compacted but summary is somehow empty
    let mut session = ChatSession::new("test-model".into(), None, false);

    // Add messages
    for i in 0..5 {
        session.messages.push(SavedMessage {
            role: MessageRole::User,
            content: format!("Message {}", i),
            timestamp: Utc::now(),
            ..Default::default()
        });
    }

    // Set an empty summary (shouldn't happen in practice, but test the edge case)
    session.compacted_summary = Some("".to_string());
    session.messages_sent_to_llm = 5; // All compacted
    session.compacted_range = Some((0, 5));

    let tokens = session.history_real_tokens();
    // Empty summary has 0 words but still adds MESSAGE_OVERHEAD (4 tokens)
    // This is expected behavior - even empty messages have overhead
    assert_eq!(
        tokens, 4,
        "Empty summary with all compacted should have MESSAGE_OVERHEAD tokens"
    );
}

#[test]
fn test_history_real_tokens_prompts_tokens_zero_vs_none() {
    // Test edge case: prompt_tokens can be Some(0) vs None
    // Both should use fallback estimation

    let mut session1 = ChatSession::new("test-model".into(), None, false);
    let mut session2 = ChatSession::new("test-model".into(), None, false);

    // Session 1: prompt_tokens = Some(0) (invalid)
    session1.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "Message content here".into(),
        timestamp: Utc::now(),
        prompt_tokens: Some(0), // Zero is invalid
        ..Default::default()
    });

    // Session 2: prompt_tokens = None (not set)
    session2.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "Message content here".into(),
        timestamp: Utc::now(),
        prompt_tokens: None,
        ..Default::default()
    });

    // Both should fallback to estimation
    let tokens1 = session1.history_real_tokens();
    let tokens2 = session2.history_real_tokens();

    // Both estimates should be similar (same content)
    assert!(tokens1 > 0, "Some(0) should use fallback estimation");
    assert!(tokens2 > 0, "None should use fallback estimation");

    // Should be equal since same content
    assert_eq!(tokens1, tokens2);
}

#[test]
fn test_history_real_tokens_cumulative_vs_zero() {
    // Test: prompt_tokens is cumulative from Ollama
    // Last non-zero value is the total

    let mut session = ChatSession::new("test-model".into(), None, false);

    // First message: prompt_tokens = 100 (includes system + msg1)
    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "First".into(),
        timestamp: Utc::now(),
        prompt_tokens: Some(100),
        ..Default::default()
    });

    // Second message: prompt_tokens = 105 (includes system + msg1 + msg2)
    session.messages.push(SavedMessage {
        role: MessageRole::Assistant,
        content: "Second".into(),
        timestamp: Utc::now(),
        prompt_tokens: Some(105),
        ..Default::default()
    });

    // Third message: prompt_tokens = 110 (includes all 3 messages)
    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "Third".into(),
        timestamp: Utc::now(),
        prompt_tokens: Some(110),
        ..Default::default()
    });

    // history_real_tokens should return the LAST cumulative value
    let tokens = session.history_real_tokens();
    assert_eq!(tokens, 110, "Should return last cumulative prompt_tokens");
}

#[test]
fn test_history_real_tokens_first_nonzero() {
    // Test reverse iteration finds FIRST non-zero prompt_tokens (which is the highest cumulative)

    let mut session = ChatSession::new("test-model".into(), None, false);

    // Messages without prompt_tokens
    for i in 0..3 {
        session.messages.push(SavedMessage {
            role: MessageRole::User,
            content: format!("Msg {}", i),
            timestamp: Utc::now(),
            prompt_tokens: None,
            ..Default::default()
        });
    }

    // Last two messages with prompt_tokens
    session.messages.push(SavedMessage {
        role: MessageRole::Assistant,
        content: "Response 1".into(),
        timestamp: Utc::now(),
        prompt_tokens: Some(200),
        ..Default::default()
    });
    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "Query 2".into(),
        timestamp: Utc::now(),
        prompt_tokens: Some(250),
        ..Default::default()
    });

    // Should find the last (most recent) prompt_tokens = 250
    let tokens = session.history_real_tokens();
    assert_eq!(tokens, 250);
}

#[test]
fn test_history_real_tokens_after_multiple_responses() {
    // Simulate a session with multiple responses
    // Each response adds prompt_tokens (cumulative from Ollama)
    let mut session = ChatSession::new("test-model".into(), None, false);

    // First exchange
    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "Hello".into(),
        timestamp: Utc::now(),
        prompt_tokens: None, // User messages don't have prompt_tokens
        ..Default::default()
    });
    session.messages.push(SavedMessage {
        role: MessageRole::Assistant,
        content: "Hi there!".into(),
        timestamp: Utc::now(),
        prompt_tokens: Some(100), // First response: 100 tokens cumulative
        ..Default::default()
    });

    let tokens1 = session.history_real_tokens();
    assert_eq!(
        tokens1, 100,
        "Should return cumulative tokens from last message"
    );

    // Second exchange
    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "How are you?".into(),
        timestamp: Utc::now(),
        prompt_tokens: None,
        ..Default::default()
    });
    session.messages.push(SavedMessage {
        role: MessageRole::Assistant,
        content: "I'm doing great!".into(),
        timestamp: Utc::now(),
        prompt_tokens: Some(150), // Second response: 150 tokens cumulative
        ..Default::default()
    });

    let tokens2 = session.history_real_tokens();
    assert_eq!(tokens2, 150, "Should return updated cumulative tokens");

    // Third exchange
    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "Good to hear".into(),
        timestamp: Utc::now(),
        prompt_tokens: None,
        ..Default::default()
    });
    session.messages.push(SavedMessage {
        role: MessageRole::Assistant,
        content: "Thanks!".into(),
        timestamp: Utc::now(),
        prompt_tokens: Some(200), // Third response: 200 tokens cumulative
        ..Default::default()
    });

    let tokens3 = session.history_real_tokens();
    assert_eq!(tokens3, 200, "Should return latest cumulative tokens");
}

#[test]
fn test_history_real_tokens_with_tool_call_in_between() {
    // Simulate session with tool calls (which don't have prompt_tokens)
    let mut session = ChatSession::new("test-model".into(), None, false);

    // User message
    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "What's the weather?".into(),
        timestamp: Utc::now(),
        prompt_tokens: None,
        ..Default::default()
    });

    // Tool call (no prompt_tokens - only assistant messages get them)
    session.messages.push(SavedMessage {
        role: MessageRole::Tool,
        content: "{\"result\": \"sunny\"}".into(),
        timestamp: Utc::now(),
        prompt_tokens: None,
        ..Default::default()
    });

    // Final assistant response
    session.messages.push(SavedMessage {
        role: MessageRole::Assistant,
        content: "It's sunny!".into(),
        timestamp: Utc::now(),
        prompt_tokens: Some(500), // Includes system + tools + history
        ..Default::default()
    });

    let tokens = session.history_real_tokens();
    assert_eq!(
        tokens, 500,
        "Should return cumulative tokens from assistant message"
    );
}

#[test]
fn test_get_recent_exchanges_empty_session() {
    let session = ChatSession::new("test-model".into(), None, false);
    let exchanges = session.get_recent_exchanges(3);
    assert!(
        exchanges.is_empty(),
        "Empty session should have no exchanges"
    );
}

#[test]
fn test_get_recent_exchanges_single_exchange() {
    let mut session = ChatSession::new("test-model".into(), None, false);
    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "Hello".into(),
        timestamp: Utc::now(),
        ..Default::default()
    });
    session.messages.push(SavedMessage {
        role: MessageRole::Assistant,
        content: "Hi there!".into(),
        timestamp: Utc::now(),
        ..Default::default()
    });

    let exchanges = session.get_recent_exchanges(3);
    assert_eq!(exchanges.len(), 1);
    assert_eq!(exchanges[0].0.content, "Hello");
    assert_eq!(exchanges[0].1.as_ref().unwrap().content, "Hi there!");
}

#[test]
fn test_get_recent_exchanges_filters_system_and_tool() {
    let mut session = ChatSession::new("test-model".into(), None, false);
    session.messages.push(SavedMessage {
        role: MessageRole::System,
        content: "System prompt".into(),
        timestamp: Utc::now(),
        ..Default::default()
    });
    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "Hello".into(),
        timestamp: Utc::now(),
        ..Default::default()
    });
    session.messages.push(SavedMessage {
        role: MessageRole::Tool,
        content: "Tool result".into(),
        timestamp: Utc::now(),
        ..Default::default()
    });
    session.messages.push(SavedMessage {
        role: MessageRole::Assistant,
        content: "Hi!".into(),
        timestamp: Utc::now(),
        ..Default::default()
    });

    let exchanges = session.get_recent_exchanges(3);
    assert_eq!(exchanges.len(), 1);
    // User should be matched with the assistant response
    assert_eq!(exchanges[0].0.content, "Hello");
    assert_eq!(exchanges[0].1.as_ref().unwrap().content, "Hi!");
}

#[test]
fn test_get_recent_exchanges_multiple_exchanges() {
    let mut session = ChatSession::new("test-model".into(), None, false);
    for i in 0..5 {
        session.messages.push(SavedMessage {
            role: MessageRole::User,
            content: format!("User {}", i),
            timestamp: Utc::now(),
            ..Default::default()
        });
        session.messages.push(SavedMessage {
            role: MessageRole::Assistant,
            content: format!("Assistant {}", i),
            timestamp: Utc::now(),
            ..Default::default()
        });
    }

    // Request 3 exchanges (should get last 3 of 5)
    let exchanges = session.get_recent_exchanges(3);
    assert_eq!(exchanges.len(), 3);

    // Should be in chronological order (oldest first)
    assert_eq!(exchanges[0].0.content, "User 2");
    assert_eq!(exchanges[0].1.as_ref().unwrap().content, "Assistant 2");
    assert_eq!(exchanges[1].0.content, "User 3");
    assert_eq!(exchanges[1].1.as_ref().unwrap().content, "Assistant 3");
    assert_eq!(exchanges[2].0.content, "User 4");
    assert_eq!(exchanges[2].1.as_ref().unwrap().content, "Assistant 4");
}

#[test]
fn test_get_recent_exchanges_incomplete_exchange() {
    // Test: last user message without assistant reply
    let mut session = ChatSession::new("test-model".into(), None, false);
    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "First question".into(),
        timestamp: Utc::now(),
        ..Default::default()
    });
    session.messages.push(SavedMessage {
        role: MessageRole::Assistant,
        content: "First answer".into(),
        timestamp: Utc::now(),
        ..Default::default()
    });
    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "Second question".into(),
        timestamp: Utc::now(),
        ..Default::default()
    });

    let exchanges = session.get_recent_exchanges(3);
    assert_eq!(exchanges.len(), 2);

    // First exchange: complete
    assert_eq!(exchanges[0].0.content, "First question");
    assert_eq!(exchanges[0].1.as_ref().unwrap().content, "First answer");

    // Second exchange: user only (no assistant reply yet)
    assert_eq!(exchanges[1].0.content, "Second question");
    assert!(exchanges[1].1.is_none());
}

#[test]
fn test_get_recent_exchanges_tool_messages_between() {
    // Test: Tool messages between user and assistant are skipped
    let mut session = ChatSession::new("test-model".into(), None, false);
    session.messages.push(SavedMessage {
        role: MessageRole::User,
        content: "What's the weather?".into(),
        timestamp: Utc::now(),
        ..Default::default()
    });
    session.messages.push(SavedMessage {
        role: MessageRole::Tool,
        content: "Weather data".into(),
        timestamp: Utc::now(),
        ..Default::default()
    });
    session.messages.push(SavedMessage {
        role: MessageRole::Assistant,
        content: "It's sunny!".into(),
        timestamp: Utc::now(),
        ..Default::default()
    });

    let exchanges = session.get_recent_exchanges(3);
    assert_eq!(exchanges.len(), 1);
    assert_eq!(exchanges[0].0.content, "What's the weather?");
    assert_eq!(exchanges[0].1.as_ref().unwrap().content, "It's sunny!");
}
