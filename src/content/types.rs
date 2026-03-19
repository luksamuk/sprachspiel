//! Types for the Content System
//!
//! Defines content types for unified storage: messages, notes, and documents.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Maximum content length per note (characters)
pub const MAX_NOTE_CONTENT_SIZE: usize = 10000;

/// Content type discriminator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    /// Chat message
    Message,
    /// User note
    Note,
    /// Imported document
    Document,
}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentType::Message => write!(f, "message"),
            ContentType::Note => write!(f, "note"),
            ContentType::Document => write!(f, "document"),
        }
    }
}

impl std::str::FromStr for ContentType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "message" => Ok(ContentType::Message),
            "note" => Ok(ContentType::Note),
            "document" => Ok(ContentType::Document),
            _ => Err(format!("Invalid content type: {}", s)),
        }
    }
}

/// Scope for content visibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ContentScope {
    /// Project-specific content
    #[default]
    Project,
    /// Global content (visible across all projects)
    Global,
}

impl std::fmt::Display for ContentScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentScope::Project => write!(f, "project"),
            ContentScope::Global => write!(f, "global"),
        }
    }
}

impl std::str::FromStr for ContentScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "project" => Ok(ContentScope::Project),
            "global" => Ok(ContentScope::Global),
            _ => Err(format!("Invalid scope: {}", s)),
        }
    }
}

/// Source of content creation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentSource {
    /// Created by user
    User,
    /// Created by LLM
    Llm,
}

impl std::fmt::Display for ContentSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentSource::User => write!(f, "user"),
            ContentSource::Llm => write!(f, "llm"),
        }
    }
}

impl std::str::FromStr for ContentSource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(ContentSource::User),
            "llm" => Ok(ContentSource::Llm),
            _ => Err(format!("Invalid source: {}", s)),
        }
    }
}

/// A note stored in the content system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// Unique identifier
    pub id: i64,
    /// Scope (project or global)
    pub scope: ContentScope,
    /// Source (user or LLM)
    pub source: ContentSource,
    /// Optional title
    pub title: Option<String>,
    /// Note content
    pub content: String,
    /// Importance (0.0 to 1.0, affects decay)
    pub importance: f32,
    /// Access count (incremented on retrieval)
    pub access_count: u32,
    /// Current decay score (0.0 to 1.0)
    pub decay_score: f32,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Last access timestamp
    pub last_accessed: DateTime<Utc>,
    /// Project ID (for project-scoped notes)
    pub project_id: Option<String>,
}

impl Note {
    /// Create a new note with default values
    pub fn new(
        content: String,
        scope: ContentScope,
        project_id: Option<String>,
        source: ContentSource,
        title: Option<String>,
    ) -> Result<Self, String> {
        if content.len() > MAX_NOTE_CONTENT_SIZE {
            return Err(format!(
                "Note content exceeds {} characters (got {})",
                MAX_NOTE_CONTENT_SIZE,
                content.len()
            ));
        }

        if !content.is_char_boundary(content.len()) {
            return Err("Note content has invalid unicode".to_string());
        }

        let now = Utc::now();

        Ok(Note {
            id: 0,
            scope,
            source,
            title,
            content,
            importance: 0.5,
            access_count: 0,
            decay_score: 1.0,
            created_at: now,
            updated_at: now,
            last_accessed: now,
            project_id,
        })
    }
}

/// Content item with full metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentItem {
    /// Unique identifier
    pub id: i64,
    /// Content type
    pub content_type: ContentType,
    /// Conversation ID (for messages)
    pub conversation_id: Option<String>,
    /// Role (for messages)
    pub role: Option<String>,
    /// Message type (for messages)
    pub message_type: Option<String>,
    /// Previous item ID (for messages)
    pub previous_item_id: Option<i64>,
    /// Prompt tokens (for messages)
    pub prompt_tokens: Option<i64>,
    /// Scope (for notes/documents)
    pub scope: Option<ContentScope>,
    /// Source (for notes/documents)
    pub source: Option<ContentSource>,
    /// Title (for notes/documents)
    pub title: Option<String>,
    /// Content text
    pub content: String,
    /// Importance (for retrieval ranking)
    pub importance: f32,
    /// Access count (for decay)
    pub access_count: u32,
    /// Decay score (for relevance)
    pub decay_score: f32,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Update timestamp
    pub updated_at: DateTime<Utc>,
    /// Last access timestamp
    pub last_accessed: DateTime<Utc>,
    /// Has embedding
    pub has_embedding: bool,
    /// Project ID (for project-scoped content)
    pub project_id: Option<String>,
}

/// Subsequent message for context enrichment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsequentItem {
    /// Content item
    pub item: ContentItem,
    /// Source type (always Conversation for messages)
    pub source_type: crate::db::SourceType,
}

/// Search result from content search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSearchResult {
    /// Content item
    pub item: ContentItem,
    /// Search score (BM25 or vector distance)
    pub score: f32,
    /// Search type (keyword, semantic, hybrid)
    pub search_type: ContentSearchType,
    /// Chunk content (if matched a chunk)
    pub chunk_content: Option<String>,
    /// Chunk offsets (if matched a chunk)
    pub chunk_offsets: Option<(i32, i32)>,
    /// Subsequent assistant messages (for user messages)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subsequent_items: Vec<SubsequentItem>,
}

/// Search type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentSearchType {
    /// Full-text search (BM25)
    Keyword,
    /// Vector similarity search
    Semantic,
    /// Hybrid (BM25 + vector)
    Hybrid,
}

impl std::fmt::Display for ContentSearchType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentSearchType::Keyword => write!(f, "keyword"),
            ContentSearchType::Semantic => write!(f, "semantic"),
            ContentSearchType::Hybrid => write!(f, "hybrid"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_display() {
        assert_eq!(ContentType::Message.to_string(), "message");
        assert_eq!(ContentType::Note.to_string(), "note");
        assert_eq!(ContentType::Document.to_string(), "document");
    }

    #[test]
    fn test_content_type_from_str() {
        use std::str::FromStr;
        assert!(matches!(
            ContentType::from_str("message"),
            Ok(ContentType::Message)
        ));
        assert!(matches!(
            ContentType::from_str("note"),
            Ok(ContentType::Note)
        ));
        assert!(matches!(
            ContentType::from_str("document"),
            Ok(ContentType::Document)
        ));
        assert!(ContentType::from_str("invalid").is_err());
    }

    #[test]
    fn test_scope_display() {
        assert_eq!(ContentScope::Project.to_string(), "project");
        assert_eq!(ContentScope::Global.to_string(), "global");
    }

    #[test]
    fn test_source_display() {
        assert_eq!(ContentSource::User.to_string(), "user");
        assert_eq!(ContentSource::Llm.to_string(), "llm");
    }

    #[test]
    fn test_note_new_validates_length() {
        let long_content = "x".repeat(MAX_NOTE_CONTENT_SIZE + 1);
        let result = Note::new(
            long_content,
            ContentScope::Project,
            Some("test".to_string()),
            ContentSource::User,
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds"));
    }

    #[test]
    fn test_note_new_valid_content() {
        let content = "This is a valid note".to_string();
        let note = Note::new(
            content.clone(),
            ContentScope::Project,
            Some("test-project".to_string()),
            ContentSource::User,
            Some("Test Title".to_string()),
        )
        .expect("Failed to create note");

        assert_eq!(note.content, content);
        assert_eq!(note.scope, ContentScope::Project);
        assert_eq!(note.source, ContentSource::User);
        assert_eq!(note.title, Some("Test Title".to_string()));
        assert_eq!(note.project_id, Some("test-project".to_string()));
        assert_eq!(note.importance, 0.5);
        assert_eq!(note.decay_score, 1.0);
        assert_eq!(note.access_count, 0);
    }
}
