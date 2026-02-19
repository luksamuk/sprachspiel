//! Chat session management
//!
//! Handles the state of a chat session including messages, model, and metadata.

use chrono::{DateTime, Utc};
use ollama_rs::generation::chat::ChatMessage;
use serde::{Deserialize, Serialize};

use super::history::{ConversationStorage, SessionInfo};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Index of first message to send to LLM (after compacted portion)
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
            messages_sent_to_llm: 0,
            created_at: now,
            updated_at: now,
            anonymous,
            think: false,
            tools: true,
            tool_output_level: ToolOutputLevel::default(),
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
    pub fn add_user_message(&mut self, content: String) {
        let now = Utc::now();
        self.messages.push(SavedMessage {
            role: MessageRole::User,
            content,
            timestamp: now,
        });
        self.updated_at = now;
    }

    /// Add an assistant message to the session
    pub fn add_assistant_message(&mut self, content: String) {
        let now = Utc::now();
        self.messages.push(SavedMessage {
            role: MessageRole::Assistant,
            content,
            timestamp: now,
        });
        self.updated_at = now;
    }

    /// Set the system prompt
    pub fn set_system_prompt(&mut self, prompt: String) {
        self.system_prompt = Some(prompt);
        self.updated_at = Utc::now();
    }

    /// Clear all messages (keep system prompt and summary)
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.compacted_summary = None;
        self.messages_sent_to_llm = 0;
        self.updated_at = Utc::now();
    }

    /// Set the compacted summary and update the LLM message index
    pub fn set_compacted_summary(&mut self, summary: String) {
        self.compacted_summary = Some(summary);
        self.messages_sent_to_llm = self.messages.len();
        self.updated_at = Utc::now();
    }

    /// Clear the compacted summary (send full history to LLM)
    #[allow(dead_code)]
    pub fn clear_compacted_summary(&mut self) {
        self.compacted_summary = None;
        self.messages_sent_to_llm = 0;
        self.updated_at = Utc::now();
    }

    /// Check if there are compacted messages
    pub fn has_compacted_messages(&self) -> bool {
        self.compacted_summary.is_some() && self.messages_sent_to_llm > 0
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
