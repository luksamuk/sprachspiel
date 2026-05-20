//! Command output types for decoupling command logic from presentation
//!
//! This module defines `CommandOutput`, a typed enum that represents the
//! output produced by chat commands. Instead of commands directly calling
//! `println!`/`eprintln!` with embedded ANSI codes, they return
//! `Vec<CommandOutput>` and the `ChatView` trait handles rendering.
//!
//! # Architecture
//!
//! ```text
//! command_handlers.rs
//!     ↓ returns
//! Vec<CommandOutput>
//!     ↓ consumed by
//! ChatView::show_command_outputs()
//!     ↓ implemented by
//! RatatuiView (TUI chat) ─── standalone renderer (query/translate/summarize/OCR)
//! ```
//!
//! # Design Principles
//!
//! - **Data carries semantics, not styling.** No ANSI codes in CommandOutput data.
//!   The view layer applies styling (RatatuiView uses ratatui Styles, standalone renderer uses ANSI for pipe-safe output).
//! - **Compound results use Vec, not nesting.** `handle_command()` returns
//!   `Vec<CommandOutput>` — no `Compound` variant that creates arbitrary nesting.
//! - **Structured data for complex displays.** List commands return typed structs
//!   (FactListData, TodoListData, etc.) so views can format them responsively.
//! - **Simple messages use string variants.** Info/Success/Warning/Error carry
//!   plain text messages. The view adds icons (✓, ✗, ⚠️, ⏳) and colors.

use crate::facts::types::{Category, Fact};

/// Output produced by a chat command.
///
/// Each variant carries semantic data (no ANSI codes). The `ChatView`
/// implementation determines how to render each variant.
///
/// Commands return `Vec<CommandOutput>` to support multi-part results
/// (e.g., a warning followed by a success message).
#[derive(Debug, Clone)]
pub enum CommandOutput {
    // ── Simple message variants ──────────────────────────────────────
    /// Informational message (dim/cyan styling).
    ///
    /// Used for confirmations, status updates, and general info.
    /// Examples: "Session saved", "New session started", "Tools enabled"
    Info(String),

    /// Success message (green styling with ✓ icon).
    ///
    /// Used for operations that completed successfully.
    /// Examples: "✓ Compacted 10 messages", "✓ Fact stored"
    Success(String),

    /// Warning message (yellow styling with ⚠ icon).
    ///
    /// Used for cautions and non-fatal issues.
    /// Examples: "Model does not support think mode", "/forget requires --yes"
    Warning(String),

    /// Error message (red styling with ✗ icon).
    ///
    /// Used for failures and error conditions.
    /// Examples: "Failed to remove fact", "Database not initialized"
    Error(String),

    /// Progress indicator (yellow styling with ⏳ icon).
    ///
    /// Used for in-progress operations.
    /// Examples: "Compacting messages...", "Running decay cycle..."
    Progress(String),

    // ── Structured data variants ──────────────────────────────────────
    /// Fact list display.
    ///
    /// Contains facts grouped by scope (global/project) for the `/fact list` command.
    FactList(FactListData),

    /// Fact remove result.
    ///
    /// Contains the removed fact info or error.
    FactRemoved(FactRemoveResult),

    /// Fact search results.
    ///
    /// Contains search results from `/fact search`.
    FactSearchResults(FactSearchData),

    /// Note list display.
    ///
    /// Contains notes for the `/note list` command.
    NoteList(NoteListData),

    /// Note add result.
    ///
    /// Contains the outcome of adding a note.
    NoteAdded(NoteAddResult),

    /// Todo list display.
    ///
    /// Contains tasks for the `/todo list` command.
    TodoList(TodoListData),

    /// Context information display.
    ///
    /// Contains token usage, model info, etc. for the `/context` command.
    ContextInfo(ContextData),

    /// Session list display.
    ///
    /// Contains session entries for the `/list` command.
    SessionList(SessionListData),

    /// Compact result display.
    ///
    /// Contains compaction statistics for the `/compact` command.
    CompactResult(CompactData),

    /// Export result display.
    ///
    /// Contains the exported content (markdown or JSON) for the `/export` command.
    ExportResult(ExportData),

    /// Skill list display.
    ///
    /// Contains available skills for the `/skill` command.
    SkillList(SkillListData),

    /// Document list display.
    ///
    /// Contains imported documents for the `/doc list` command.
    DocumentList(DocumentListData),

    /// Content prune result.
    ///
    /// Contains statistics from `/content prune`.
    ContentPruneResult(ContentPruneData),

    /// Search results display.
    ///
    /// Contains semantic search results for the `/search` command.
    SearchResults(SearchData),

    /// Reindex result.
    ///
    /// Contains statistics from `/reindex`.
    /// Note: In TUI mode, reindex runs in the background and results are sent
    /// as async system messages via `AsyncMessageTx`, not as `CommandOutput`.
    #[allow(dead_code)] // Used in terminal mode; TUI uses async_message_tx
    ReindexResult(ReindexData),

    /// Help text display.
    ///
    /// Contains the formatted help text for the `/help` command.
    HelpText(String),

    /// Markdown content display.
    ///
    /// Contains markdown text to be rendered by the view layer.
    /// Used for compact summaries, note/document content, and other
    /// markdown-formatted output.
    MarkdownContent(String),

    /// Token usage metrics display.
    ///
    /// Shows prompt tokens, response tokens, and total after a response.
    /// Rendered dimmed (gray) by the view layer.
    TokenDisplay {
        /// Prompt tokens
        prompt_tokens: u64,
        /// Response tokens
        response_tokens: u64,
        /// Total tokens
        total_tokens: u64,
    },

    // ── Flow control ──────────────────────────────────────────────────
    /// Exit the REPL.
    ///
    /// Returned by `/quit` and `/exit` commands.
    Quit,
}

// ── Data structs for structured CommandOutput variants ────────────────

/// Data for fact list display (`/fact list`).
#[derive(Debug, Clone)]
pub struct FactListData {
    /// Facts grouped by scope
    pub global_facts: Vec<Fact>,
    /// Project-scoped facts
    pub project_facts: Vec<Fact>,
    /// The scope being displayed
    pub scope: FactListScopeData,
}

/// Scope for fact list display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactListScopeData {
    /// Show all facts (global + project)
    All,
    /// Show only global facts
    Global,
    /// Show only project facts
    Project,
}

/// Result of removing a fact (`/fact remove`).
#[derive(Debug, Clone)]
pub struct FactRemoveResult {
    /// The ID that was requested for removal
    pub id: i64,
    /// The content of the removed fact (if found)
    pub content: Option<String>,
    /// Whether removal succeeded
    pub success: bool,
    /// Error message (if removal failed)
    pub error: Option<String>,
}

/// Fact search results (`/fact search`).
#[derive(Debug, Clone)]
pub struct FactSearchData {
    /// The search query
    pub query: String,
    /// Search results
    pub results: Vec<FactSearchResult>,
    /// Total number of results
    pub total: usize,
}

/// A single fact search result.
#[derive(Debug, Clone)]
pub struct FactSearchResult {
    /// Fact ID
    pub id: i64,
    /// Fact content
    pub content: String,
    /// Fact category (preference, identity, fact)
    #[allow(dead_code)]
    // Category enum for structured fact grouping — TUI will use for icons/color
    pub category: Category,
    /// Relevance score (0.0 - 1.0)
    pub score: f64,
}

/// Data for note list display (`/note list`).
#[derive(Debug, Clone)]
pub struct NoteListData {
    /// Notes in this scope
    pub notes: Vec<crate::content::Note>,
    /// Current page (1-indexed)
    pub page: usize,
    /// Total number of pages
    pub total_pages: usize,
    /// Total number of notes in scope
    pub total_notes: usize,
}

/// Result of adding a note (`/note add`).
#[derive(Debug, Clone)]
pub struct NoteAddResult {
    /// Whether the add succeeded
    pub success: bool,
    /// Success message
    pub message: String,
}

/// Data for todo list display (`/todo list`).
#[derive(Debug, Clone)]
pub struct TodoListData {
    /// Formatted todo list string
    pub formatted_list: String,
    /// Number of tasks
    pub count: usize,
}

/// Data for context info display (`/context`).
#[derive(Debug, Clone)]
pub struct ContextData {
    /// Formatted context information string
    pub formatted: String,
}

/// Data for session list display (`/list`).
#[derive(Debug, Clone)]
pub struct SessionListData {
    /// Session entries
    pub sessions: Vec<SessionEntry>,
    /// Whether the list is empty
    pub is_empty: bool,
}

/// A single session entry in the list.
#[derive(Debug, Clone)]
pub struct SessionEntry {
    /// Session name
    pub name: String,
    /// Message count
    pub message_count: usize,
    /// Whether this is the current session
    pub is_current: bool,
    /// Last updated time (age display)
    pub updated_at: Option<String>,
}

/// Data for compact result display (`/compact`).
#[derive(Debug, Clone)]
pub struct CompactData {
    /// Number of messages compacted
    pub count: usize,
    /// Number of first messages preserved (middle compaction)
    pub preserved_first: usize,
    /// Number of last messages preserved (middle compaction)
    pub preserved_last: usize,
}

/// Data for export result display (`/export`).
#[derive(Debug, Clone)]
pub struct ExportData {
    /// The exported content
    pub content: String,
    /// Export format (Markdown or JSON) — used to determine output style
    #[allow(dead_code)]
    // Format discriminates display mode — TUI will show format label differently
    pub format: ExportFormat,
    /// File path (if saved to file)
    pub file_path: Option<String>,
}

/// Export format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportFormat {
    /// Markdown format
    Markdown,
    /// JSON format
    Json,
}

/// Data for skill list display (`/skill`).
#[derive(Debug, Clone)]
pub struct SkillListData {
    /// Available skills
    pub skills: Vec<SkillEntry>,
}

/// A single skill entry.
#[derive(Debug, Clone)]
pub struct SkillEntry {
    /// Skill name
    pub name: String,
    /// Skill description
    pub description: String,
}

/// Data for document list display (`/doc list`).
#[derive(Debug, Clone)]
pub struct DocumentListData {
    /// Document entries
    pub documents: Vec<DocumentEntry>,
    /// Whether the list is empty
    pub is_empty: bool,
}

/// A single document entry.
#[derive(Debug, Clone)]
pub struct DocumentEntry {
    /// Document title
    pub title: String,
    /// Document ID
    pub id: i64,
    /// Source type (file extension)
    pub source_type: String,
    /// Word count
    pub word_count: usize,
    /// Creation timestamp (for age calculation)
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Data for content prune result (`/content prune`).
#[derive(Debug, Clone)]
pub struct ContentPruneData {
    /// Number of items pruned
    pub pruned_count: usize,
    /// Number of items checked
    pub total_count: usize,
    /// Whether pruning succeeded
    pub success: bool,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Data for search results display (`/search`).
#[derive(Debug, Clone)]
pub struct SearchData {
    /// Search results formatted string
    pub formatted: String,
}

/// Data for reindex result (`/reindex --yes`).
#[derive(Debug, Clone)]
pub struct ReindexData {
    /// Number of embeddings successfully regenerated
    pub regenerated: usize,
    /// Total items + chunks to re-index (includes both content_items and content_chunks)
    pub total: usize,
    /// Whether reindexing succeeded
    pub success: bool,
    /// Error message (if failed)
    pub error: Option<String>,
}

// ── Helper constructors ───────────────────────────────────────────────

impl CommandOutput {
    /// Create an Info output.
    pub fn info(msg: impl Into<String>) -> Self {
        CommandOutput::Info(msg.into())
    }

    /// Create a Success output.
    pub fn success(msg: impl Into<String>) -> Self {
        CommandOutput::Success(msg.into())
    }

    /// Create a Warning output.
    pub fn warning(msg: impl Into<String>) -> Self {
        CommandOutput::Warning(msg.into())
    }

    /// Create an Error output.
    pub fn error(msg: impl Into<String>) -> Self {
        CommandOutput::Error(msg.into())
    }

    /// Create a Progress output.
    pub fn progress(msg: impl Into<String>) -> Self {
        CommandOutput::Progress(msg.into())
    }

    /// Create a Quit output (convenience).
    pub fn quit() -> Self {
        CommandOutput::Quit
    }
}
