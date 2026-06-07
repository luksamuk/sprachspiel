//! Application state and event loop for the TUI chat REPL
//!
//! This module provides the `App` struct that holds all state for the
//! chat REPL and the event loop that processes crossterm key events,
//! LLM responses, and terminal resize events.
//!
//! # Architecture
//!
//! ```text
//! App (event loop via tokio + crossterm)
//!     ├─ CrosstermInput (history, InputBackend trait)
//!     ├─ TextArea (input buffer, cursor, selection, kill-ring)
//!     ├─ CompletionMenuState (floating completion overlay)
//!     ├─ ChatMessage[] (chat area content)
//!     ├─ StatusBarState (model, tokens, spinner)
//!     ├─ ScrollState (auto-scroll and manual offset)
//!     └─ LlmState (idle, thinking, streaming, tool_call)
//! ```

use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui_textarea::{CursorMove, TextArea};
use tokio::sync::mpsc;

use super::completer::ChatCompleter;
use super::input::{CrosstermInput, InputBackend, InputResult};
use super::tui::TuiTerminal;
use super::tui::components::chat_area::ChatMessage;
use super::tui::components::chat_area::MessageType;
use super::tui::components::chat_selection::ChatSelection;
use super::tui::components::completion_menu::CompletionMenuState;
use super::tui::components::status_bar::StatusBarState;
use super::tui::markdown::MarkdownTheme;

/// Each phase shows a different emoji in the status bar to indicate
/// what type of entity is being indexed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingPhase {
    /// Indexing content items (messages, notes, documents). Shows 📄.
    Content,
    /// Indexing fact embeddings. Shows 💡.
    Facts,
    /// Verifying/deduplicating facts. Shows 🔍.
    FactDedup,
}

/// Progress of the embedding indexing pipeline.
///
/// Tracks entity-level and embedding-level progress separately.
/// Display format: `⚙ 60/100 📄 · 65/105↗`
///
/// - `60/100` — entities processed/total (yellow, bold)
/// - `📄` — phase emoji (plain)
/// - `65/105↗` — embeddings processed/total (cyan, bold); `↗` when embeddings > entities
///
/// Completion is signaled explicitly via `completed: true` rather than by
/// comparing counter values to avoid sentinel values like `usize::MAX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddingProgress {
    pub phase: EmbeddingPhase,
    pub entities_current: usize,
    pub entities_total: usize,
    pub embeddings_current: usize,
    pub embeddings_total: usize,
    /// Explicit completion flag. When true, the indexing pipeline has finished
    /// and the progress indicator should be cleared regardless of counter values.
    pub completed: bool,
}

impl EmbeddingProgress {
    pub fn completed() -> Self {
        Self {
            phase: EmbeddingPhase::Content,
            entities_current: 0,
            entities_total: 0,
            embeddings_current: 0,
            embeddings_total: 0,
            completed: true,
        }
    }

    pub fn new(
        phase: EmbeddingPhase,
        entities_current: usize,
        entities_total: usize,
        embeddings_current: usize,
        embeddings_total: usize,
    ) -> Self {
        Self {
            phase,
            entities_current,
            entities_total,
            embeddings_current,
            embeddings_total,
            completed: false,
        }
    }

    pub fn is_completed(&self) -> bool {
        self.completed
    }
}

pub type EmbeddingProgressTx = mpsc::UnboundedSender<EmbeddingProgress>;

/// Type alias for async system message channel sender.
///
/// Background tasks (e.g., /reindex) send system message strings to be
/// displayed in the TUI chat area when the operation completes.
pub type AsyncMessageTx = mpsc::UnboundedSender<String>;

/// Processing state of the LLM
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmState {
    /// Idle — waiting for user input
    Idle,
    /// Thinking — spinner active, input disabled
    Thinking,
    /// Streaming — response coming in, input disabled
    Streaming,
    /// Compacting — conversation compaction in progress
    Compacting,
    /// Running a tool call
    ToolCall,
}

/// How often the spinner frame advances (milliseconds).
///
/// 120ms gives a brisk, lively pace for braille dot animations —
/// fast enough to feel responsive, not so fast it becomes a blur.
/// This is independent of streaming token arrival rate.
pub const SPINNER_TICK_MS: u64 = 120;

/// Scroll state for the chat area.
///
/// Tracks whether the chat should auto-scroll to the bottom (default)
/// or whether the user has manually scrolled up to review older messages.
///
/// - `auto_scroll = true`: chat scrolls to show the newest message (bottom)
/// - `auto_scroll = false`: user has scrolled up; `manual_offset` tracks
///   how many lines above the bottom the viewport is positioned
#[derive(Debug, Clone, Copy)]
pub struct ScrollState {
    /// Whether to auto-scroll to the bottom on new content
    pub auto_scroll: bool,
    /// Manual scroll offset in lines from the bottom (0 = at bottom)
    pub manual_offset: u16,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self {
            auto_scroll: true,
            manual_offset: 0,
        }
    }
}

impl ScrollState {
    /// Create a new scroll state that auto-scrolls to bottom
    pub fn new() -> Self {
        Self::default()
    }

    /// Scroll up by `lines` lines (towards older messages).
    /// Disables auto-scroll and increments the manual offset.
    pub fn scroll_up(&mut self, lines: u16) {
        self.auto_scroll = false;
        self.manual_offset = self.manual_offset.saturating_add(lines);
    }

    /// Scroll down by `lines` lines (towards newer messages).
    /// If offset reaches 0, re-enables auto-scroll.
    pub fn scroll_down(&mut self, lines: u16) {
        self.manual_offset = self.manual_offset.saturating_sub(lines);
        if self.manual_offset == 0 {
            self.auto_scroll = true;
        }
    }

    /// Clamp `manual_offset` to the valid range `[0, max_scroll]`.
    ///
    /// Called during rendering when `total_lines` and `visible_height` are
    /// known. Prevents "overscroll" accumulation from rapid mouse wheel
    /// scrolling — without this, `scroll_up()` can grow `manual_offset`
    /// well beyond `max_scroll`, causing sluggish response when scrolling
    /// back down because every `scroll_down(MOUSE_SCROLL_LINES)` only
    /// subtracts 3 from a huge offset.
    pub fn clamp_offset(&mut self, total_lines: usize, visible_height: usize) {
        if total_lines <= visible_height {
            // Content fits in the viewport — no scroll needed
            self.manual_offset = 0;
            self.auto_scroll = true;
            return;
        }
        let max_scroll = total_lines.saturating_sub(visible_height) as u16;
        if self.manual_offset > max_scroll {
            self.manual_offset = max_scroll;
        }
    }

    /// Scroll to the top (oldest messages)
    pub fn scroll_to_top(&mut self) {
        self.auto_scroll = false;
        // u16::MAX is safe here: effective_scroll_from_top() uses saturating_sub,
        // so manual_offset > max_scroll always results in from_top = 0 (showing top).
        // This is NOT the same as the previous u16::MAX scroll bug, which passed
        // the value directly to Paragraph::scroll() without clamping.
        self.manual_offset = u16::MAX;
    }

    /// Scroll to the bottom (newest messages)
    pub fn scroll_to_bottom(&mut self) {
        self.auto_scroll = true;
        self.manual_offset = 0;
    }

    /// Reset to auto-scroll (used when submitting a message)
    pub fn reset_to_bottom(&mut self) {
        self.auto_scroll = true;
        self.manual_offset = 0;
    }

    /// Calculate the effective scroll offset from the top of the content.
    ///
    /// Given the total number of content lines and the visible area height,
    /// returns the number of lines to skip from the top.
    ///
    /// - If auto-scrolling: show the bottom of content (newest messages)
    /// - If manual scrolling: show content `manual_offset` lines above the bottom
    pub fn effective_scroll_from_top(&self, total_lines: usize, visible_height: usize) -> u16 {
        if total_lines <= visible_height {
            // Content fits in the viewport — no scroll needed
            return 0;
        }

        let max_scroll = total_lines.saturating_sub(visible_height);

        if self.auto_scroll {
            // Auto-scroll: show the bottom of content
            max_scroll as u16
        } else {
            // Manual scroll: offset from the bottom
            let from_top = max_scroll.saturating_sub(self.manual_offset as usize);
            from_top as u16
        }
    }
}

/// The main application state for the TUI chat REPL
pub struct App {
    /// Chat messages displayed in the chat area
    messages: Vec<ChatMessage>,
    /// Current round index in a multi-round LLM tool-call cycle.
    ///
    /// Incremented on each `ToolCallStarted` event, reset to 0 on each new
    /// user prompt (`handle_key_line`) and on `Complete`/`Cancelled`/`Error`.
    /// Used by `insert_at_round_boundary()` to position inter-round content
    /// (thinking, text from `InterToolText`) after all messages of the
    /// previous round. Ephemeral — not persisted to SQLite.
    current_round: usize,
    /// Text editor widget for input (replaces InputState)
    textarea: TextArea<'static>,
    /// Whether input is disabled (e.g., during LLM processing)
    input_disabled: bool,
    /// Disabled reason (shown when input is disabled)
    disabled_reason: Option<String>,
    /// CrosstermInput for history management
    history_input: CrosstermInput,
    /// Tab completion engine (slash commands + model names)
    completer: ChatCompleter,
    /// Floating completion menu state
    completion_menu: CompletionMenuState,
    /// Chat text selection state (mouse-based)
    chat_selection: ChatSelection,
    /// Cache of visual lines (after word-wrap) for chat text selection.
    /// Updated during each render cycle by `chat_area::render()`.
    /// Used for selection text extraction and mouse position mapping.
    visual_lines_cache: Vec<String>,
    /// Maps each display row in visual_lines_cache to its source line index.
    /// Used by selection highlight to map display-row coordinates to source lines.
    source_line_map_cache: Vec<usize>,
    /// Cache of scroll offset from top (updated during each render cycle).
    /// Used for mapping mouse positions to visual line coordinates.
    scroll_from_top_cache: u16,
    /// Cache of the chat area Rect (updated during each render cycle).
    /// Used for mouse position mapping in click/drag selection.
    chat_area_rect_cache: ratatui::layout::Rect,
    /// Status bar state
    status_bar: StatusBarState,
    /// LLM processing state
    llm_state: LlmState,
    /// Markdown rendering theme
    theme: MarkdownTheme,
    /// Whether style rendering is enabled (mermaid diagrams, LaTeX formulas, syntax
    /// highlighting, box-drawing tables). When false, Mermaid blocks
    /// show as source code blocks, code blocks have fg colors stripped,
    /// and tables are rendered as pipe-delimited text.
    style_enabled: bool,
    /// Scroll state for the chat area
    scroll: ScrollState,
    /// Spinner animation frames (random rattles preset)
    spinner_frames: Vec<&'static str>,
    /// Current spinner frame index
    spinner_frame: usize,
    /// Whether the first streamed block of a multi-block turn was finalized.
    ///
    /// Set to `true` when `StreamBlockDone` is received (tool call interrupt).
    /// Cleared back to `false` on `Complete`. Used by the event loop to
    /// decide whether content is "already shown" or needs adding.
    pub block_finalized: bool,
    /// Channel receiver for embedding progress updates from background tasks.
    embedding_progress_rx: mpsc::UnboundedReceiver<EmbeddingProgress>,
    /// Channel receiver for asynchronous system messages from background tasks
    /// (e.g., reindex completion notification).
    async_message_rx: mpsc::UnboundedReceiver<String>,
    /// Cached count of visual (wrapped) lines in the textarea input.
    /// Updated after each render cycle; used to calculate input height before
    /// the textarea has been rendered with the correct viewport width.
    /// Value 0 means "not yet calculated" — fall back to logical line count.
    cached_input_screen_lines: usize,
}

/// Pick a random spinner preset from rattles (same logic as `spinner.rs`).
fn random_tui_spinner_frames() -> Vec<&'static str> {
    use rattles::Rattle;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn extract_frames<T: Rattle>(rattler: rattles::Rattler<T>) -> Vec<&'static str> {
        let len = rattler.len();
        let mut ticked = rattler.into_ticked();
        let mut frames = Vec::with_capacity(len);
        for _ in 0..len {
            frames.push(ticked.tick()[0]);
        }
        frames
    }

    let presets: Vec<fn() -> Vec<&'static str>> = vec![
        || extract_frames(rattles::presets::braille::dots()),
        || extract_frames(rattles::presets::braille::dots2()),
        || extract_frames(rattles::presets::braille::dots3()),
        || extract_frames(rattles::presets::braille::dots4()),
        || extract_frames(rattles::presets::braille::dots5()),
        || extract_frames(rattles::presets::braille::dots6()),
        || extract_frames(rattles::presets::braille::dots7()),
        || extract_frames(rattles::presets::braille::dots8()),
        || extract_frames(rattles::presets::braille::dots9()),
        || extract_frames(rattles::presets::braille::dots10()),
        || extract_frames(rattles::presets::braille::dots11()),
        || extract_frames(rattles::presets::braille::dots12()),
        || extract_frames(rattles::presets::braille::bounce()),
        || extract_frames(rattles::presets::braille::breathe()),
        || extract_frames(rattles::presets::braille::snake()),
        || extract_frames(rattles::presets::braille::wave()),
        || extract_frames(rattles::presets::braille::orbit()),
        || extract_frames(rattles::presets::braille::pulse()),
        || extract_frames(rattles::presets::braille::sparkle()),
        || extract_frames(rattles::presets::braille::scan()),
        || extract_frames(rattles::presets::braille::helix()),
        || extract_frames(rattles::presets::ascii::arc()),
        || extract_frames(rattles::presets::ascii::balloon()),
        || extract_frames(rattles::presets::ascii::circle_halves()),
        || extract_frames(rattles::presets::ascii::circle_quarters()),
        || extract_frames(rattles::presets::ascii::triangle()),
        || extract_frames(rattles::presets::ascii::grow_horizontal()),
        || extract_frames(rattles::presets::arrows::arrow()),
    ];

    let idx = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        % (presets.len() as u128)) as usize;

    presets[idx]()
}

impl App {
    /// Create a new App with an embedding progress sender for background tasks.
    ///
    /// Returns the App, an `EmbeddingProgressTx` for progress updates, and an
    /// `AsyncMessageTx` for async system messages from background tasks.
    pub fn with_embedding_channel(
        theme: MarkdownTheme,
        model_names: Vec<String>,
    ) -> (Self, EmbeddingProgressTx, AsyncMessageTx) {
        let completer = ChatCompleter::new(model_names.clone());

        // Create textarea with custom styling: no line numbers, no cursor line highlight
        let mut textarea = TextArea::default();
        textarea.set_line_number_style(ratatui::style::Style::default());
        textarea.set_cursor_line_style(ratatui::style::Style::default());
        textarea.set_tab_length(4);
        // Enable word-wrap so long lines fold at word boundaries (with glyph
        // fallback for words wider than the viewport). This makes the input
        // area purely vertical-scroll, eliminating horizontal scroll.
        textarea.set_wrap_mode(ratatui_textarea::WrapMode::WordOrGlyph);

        // Channel for embedding progress updates
        let (embedding_tx, embedding_progress_rx) = mpsc::unbounded_channel();

        // Channel for async system messages (e.g., reindex completion)
        let (async_message_tx, async_message_rx) = mpsc::unbounded_channel();

        let app = Self {
            messages: Vec::new(),
            current_round: 0,
            textarea,
            input_disabled: false,
            disabled_reason: None,
            history_input: CrosstermInput::new(model_names),
            completer,
            completion_menu: CompletionMenuState::new(),
            chat_selection: ChatSelection::new(),
            visual_lines_cache: Vec::new(),
            source_line_map_cache: Vec::new(),
            scroll_from_top_cache: 0,
            chat_area_rect_cache: ratatui::layout::Rect::default(),
            status_bar: StatusBarState::new(String::new(), 0, 0, 0, false, false),
            llm_state: LlmState::Idle,
            theme,
            style_enabled: true,
            scroll: ScrollState::new(),
            spinner_frames: random_tui_spinner_frames(),
            spinner_frame: 0,
            block_finalized: false,
            embedding_progress_rx,
            async_message_rx,
            cached_input_screen_lines: 0,
        };

        (app, embedding_tx, async_message_tx)
    }

    /// Add a message to the chat area and auto-scroll to bottom
    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.scroll.reset_to_bottom();
    }

    /// Append a streaming token to the last `AssistantStreaming` message.
    ///
    /// If the last message is not `AssistantStreaming`, searches backward
    /// within the streaming zone (contiguous tail of Thinking/AssistantStreaming
    /// messages) for an existing `AssistantStreaming` block. If found, appends
    /// to it. Otherwise creates a new one.
    ///
    /// **Round-awareness:** Streaming tokens are always round 0 (final response).
    /// If the last `AssistantStreaming` block has `round_index > 0` (inserted via
    /// InterToolText), streaming tokens must NOT append to it — they belong to a
    /// different round. Instead, a new `AssistantStreaming` block is created.
    pub fn append_stream_token(&mut self, token: &str) {
        // Happy path: last message is AssistantStreaming — append directly,
        // but only if it's a streaming block (round 0). Stable blocks
        // (round > 0, from InterToolText) must not receive streaming tokens.
        if let Some(last) = self.messages.last_mut()
            && last.msg_type == MessageType::AssistantStreaming
            && last.round_index == 0
        {
            last.content.push_str(token);
            return;
        }

        // Find the streaming zone boundary and search within it
        let streaming_start = self.streaming_zone_start();
        let zone_len = self.messages.len().saturating_sub(streaming_start);

        // Interleaved tokens: search backward within the streaming zone
        // for an existing AssistantStreaming block (round 0) to append to.
        // Skip blocks with round_index > 0 — those are stable InterToolText blocks.
        if let Some(prev_streaming) = self
            .messages
            .iter_mut()
            .rev()
            .take(zone_len)
            .find(|m| m.msg_type == MessageType::AssistantStreaming && m.round_index == 0)
        {
            prev_streaming.content.push_str(token);
            return;
        }

        // No streaming message yet — create one
        self.messages
            .push(ChatMessage::assistant_streaming(token.to_string()));
        self.scroll.reset_to_bottom();
    }

    /// Append a streaming thinking token to the last `Thinking` message.
    ///
    /// If the last message is not `Thinking`, searches backward within the
    /// streaming zone (contiguous tail of Thinking/AssistantStreaming messages)
    /// for an existing `Thinking` block. If found, appends to it. Otherwise
    /// creates a new one.
    ///
    /// **Round-awareness:** Streaming thinking tokens are always round 0 (final
    /// response). If the last `Thinking` block has `round_index > 0` (inserted
    /// via InterToolText), streaming thinking tokens must NOT append to it —
    /// they belong to a different round. Instead, a new `Thinking` block is created.
    pub fn append_stream_thinking(&mut self, token: &str) {
        // Happy path: last message is Thinking — append directly,
        // but only if it's a streaming block (round 0). Stable blocks
        // (round > 0, from InterToolText) must not receive streaming tokens.
        if let Some(last) = self.messages.last_mut()
            && last.msg_type == MessageType::Thinking
            && last.round_index == 0
        {
            last.content.push_str(token);
            return;
        }

        // Find the streaming zone boundary and search within it
        let streaming_start = self.streaming_zone_start();
        let zone_len = self.messages.len().saturating_sub(streaming_start);

        // Interleaved tokens: search backward within the streaming zone
        // for an existing Thinking block (round 0) to append to.
        // Skip blocks with round_index > 0 — those are stable InterToolText blocks.
        if let Some(prev_thinking) = self
            .messages
            .iter_mut()
            .rev()
            .take(zone_len)
            .find(|m| m.msg_type == MessageType::Thinking && m.round_index == 0)
        {
            prev_thinking.content.push_str(token);
            return;
        }

        // No thinking message yet — create one
        self.messages.push(ChatMessage::thinking(token.to_string()));
        self.scroll.reset_to_bottom();
    }

    /// Returns the start index of the streaming zone.
    ///
    /// The streaming zone is the contiguous tail of streaming-eligible
    /// messages: `Thinking` with `round_index == 0` and `AssistantStreaming`.
    /// Everything before this index is stable and must not be modified by
    /// streaming operations.
    ///
    /// **Round-awareness:** `Thinking` blocks with `round_index > 0` are
    /// stable inter-round blocks (inserted via InterToolText) and must NOT
    /// be included in the streaming zone. Including them would cause
    /// `finalize_stream()` to consolidate inter-round thinking into a
    /// single block, merging thinking from different rounds.
    fn streaming_zone_start(&self) -> usize {
        self.messages
            .iter()
            .enumerate()
            .rev()
            .find(|(_, m)| {
                // Streaming-eligible: round-0 Thinking and any AssistantStreaming
                let is_streaming_eligible = matches!(m.msg_type, MessageType::AssistantStreaming)
                    || (m.msg_type == MessageType::Thinking && m.round_index == 0);
                !is_streaming_eligible
            })
            .map(|(i, _)| i + 1)
            .unwrap_or(0)
    }

    /// Insert a message before the streaming zone.
    ///
    /// When the LLM is in `ToolCall` or `Streaming` state, tool messages
    /// and view actions should appear BEFORE any streaming content
    /// (Thinking/AssistantStreaming messages). This method finds the
    /// streaming zone start and inserts the message there, pushing
    /// streaming content down.
    ///
    /// If there is no streaming zone (all messages are stable), this
    /// finds the boundary before any trailing Tool messages and inserts
    /// before them. This ensures inter-tool text (from `InterToolText`
    /// events) appears in the correct position — after pre-tool content
    /// and before subsequent tool calls — rather than appended after
    /// all existing tool messages.
    ///
    /// Fallback: if there are no trailing tool messages either, appends
    /// at the end via `add_message()`.
    pub fn insert_before_streaming_zone(&mut self, message: ChatMessage) {
        let zone_start = self.streaming_zone_start();
        if zone_start < self.messages.len() {
            // There's a streaming zone — insert before it
            self.messages.insert(zone_start, message);
        } else {
            // No streaming zone — find the boundary before trailing
            // Tool messages. Inter-tool text (from InterToolText events)
            // must appear BEFORE tool messages, not after them.
            let tool_boundary = self
                .messages
                .iter()
                .enumerate()
                .rev()
                .find(|(_, m)| m.msg_type != MessageType::Tool)
                .map(|(i, _)| i + 1)
                .unwrap_or(0);
            if tool_boundary < self.messages.len() {
                // There are trailing tool messages — insert before them
                self.messages.insert(tool_boundary, message);
            } else {
                // No trailing tool messages — just append
                self.messages.push(message);
            }
        }
        self.scroll.reset_to_bottom();
    }

    /// Insert a message after all messages with `round_index <= message.round_index`,
    /// respecting the streaming zone boundary (only `AssistantStreaming` messages).
    ///
    /// This is the round-aware replacement for `insert_before_streaming_zone()`
    /// when dealing with inter-round content from multi-round LLM tool call cycles.
    /// When a multi-round tool cycle occurs (e.g., model searches → observes results →
    /// searches again), inter-round content (thinking, text from `InterToolText`)
    /// must appear AFTER all messages of the previous round and BEFORE messages
    /// of subsequent rounds.
    ///
    /// # Algorithm
    ///
    /// 1. Find the boundary before any `AssistantStreaming` messages at the tail.
    ///    (Unlike `streaming_zone_start()`, this does NOT include `Thinking` blocks,
    ///    because finalized Thinking content from `InterToolText` should be treated
    ///    as stable for round boundary purposes.)
    /// 2. Within the stable zone (before `AssistantStreaming`), find the last message
    ///    with `round_index <= target_round`. Insert after that message.
    /// 3. If no `AssistantStreaming` zone exists, search all messages.
    /// 4. Fallback: if no message has `round_index <= target_round`, insert at
    ///    position 0 (all messages have higher round_index).
    pub fn insert_at_round_boundary(&mut self, message: ChatMessage) {
        let target_round = message.round_index;

        // Find the boundary before AssistantStreaming messages at the tail.
        // We only exclude AssistantStreaming (actively streaming, incomplete content)
        // from the round-boundary search. Finalized Thinking blocks from
        // InterToolText are stable content and should participate in the search.
        let stream_boundary = self
            .messages
            .iter()
            .enumerate()
            .rev()
            .find(|(_, m)| m.msg_type != MessageType::AssistantStreaming)
            .map(|(i, _)| i + 1)
            .unwrap_or(0);

        let search_end = if stream_boundary < self.messages.len() {
            stream_boundary
        } else {
            self.messages.len()
        };

        if search_end == 0 {
            // All messages are AssistantStreaming — insert at position 0
            self.messages.insert(0, message);
        } else {
            // Find the last message in the stable zone with round_index <= target_round
            let insert_pos = self.messages[..search_end]
                .iter()
                .enumerate()
                .rev()
                .find(|(_, m)| m.round_index <= target_round)
                .map(|(i, _)| i + 1)
                .unwrap_or(0);

            self.messages.insert(insert_pos, message);
        }
        self.scroll.reset_to_bottom();
    }

    /// Get the current round index.
    ///
    /// Used by the event loop to assign round_index to tool messages
    /// before they are drained into the chat area.
    pub fn current_round(&self) -> usize {
        self.current_round
    }

    /// Increment the current round index.
    ///
    /// Called on `ToolCallStarted` — each tool call round increments
    /// the counter so that inter-round content from subsequent rounds
    /// can be positioned correctly.
    pub fn increment_round(&mut self) {
        self.current_round += 1;
    }

    /// Reset the current round index to 0.
    ///
    /// Called on `Complete`, `Cancelled`, `Error`, and at the start of
    /// each new user prompt (`handle_key_line`). Round tracking is
    /// ephemeral — it does not persist across LLM interactions.
    pub fn reset_round(&mut self) {
        self.current_round = 0;
    }

    /// Finalize the current streaming zone by converting all
    /// `AssistantStreaming` messages to stable `Assistant`.
    ///
    /// Called when streaming is interrupted by tool calls before
    /// `StreamBlockDone`/`StreamDone` arrive (e.g., on `ToolCallStarted`).
    /// Preserves the streamed content as-is without authoritative final data.
    /// Thinking blocks remain unchanged (already stream-accumulated).
    ///
    /// Preserves `round_index` on converted messages so that round-aware
    /// ordering is maintained after the zone is finalized.
    pub fn finalize_streaming_zone_as_is(&mut self) {
        let streaming_start = self.streaming_zone_start();
        for i in streaming_start..self.messages.len() {
            if self.messages[i].msg_type == MessageType::AssistantStreaming {
                let content = self.messages[i].content.clone();
                let round = self.messages[i].round_index;
                self.messages[i] = ChatMessage::assistant_markdown(content).with_round_index(round);
            }
        }
        self.scroll.reset_to_bottom();
    }

    /// Check whether the streaming zone is non-empty.
    ///
    /// Returns `true` if there are streaming-eligible messages
    /// (round-0 `Thinking` or `AssistantStreaming`) at the tail
    /// of the message list, indicating that content is currently
    /// being displayed via streaming events.
    /// Used to avoid duplicating content that's already being shown.
    pub fn has_streaming_zone(&self) -> bool {
        self.streaming_zone_start() < self.messages.len()
    }

    /// Replace the last `AssistantStreaming` message with the final
    /// markdown-rendered `Assistant` message, and consolidate any
    /// fragmented `Thinking` blocks from the streaming session.
    ///
    /// During streaming with interleaved thinking/content tokens, multiple
    /// `Thinking` blocks may have been created (see `append_stream_thinking`).
    /// This method consolidates them into a single `Thinking` block using the
    /// final thinking content from the complete LLM response, then replaces
    /// `AssistantStreaming` with the final markdown-rendered `Assistant`.
    ///
    /// Only `Thinking` blocks in the **streaming zone** (the contiguous tail
    /// of round-0 `Thinking` and `AssistantStreaming` messages) are consolidated
    /// or removed. `Thinking` blocks from earlier tool-call rounds (preceded
    /// by `Tool`, `Assistant`, `User`, etc., or with `round_index > 0`) are
    /// preserved intact.
    pub fn finalize_stream(&mut self, content: &str, thinking: Option<&str>) {
        // Determine the streaming zone boundary using shared helper
        let streaming_start = self.streaming_zone_start();

        // Collect Thinking positions within the streaming zone only
        let thinking_positions: Vec<usize> = (streaming_start..self.messages.len())
            .filter(|&i| self.messages[i].msg_type == MessageType::Thinking)
            .collect();

        if !thinking_positions.is_empty() {
            if let Some(thinking_content) = thinking {
                if thinking_positions.len() > 1 {
                    // Consolidate fragmented Thinking blocks: replace first
                    // with authoritative content, remove the rest.
                    self.messages[thinking_positions[0]] =
                        ChatMessage::thinking(thinking_content.to_string());
                    // Remove in reverse order to preserve indices
                    for &pos in thinking_positions.iter().rev().skip(1) {
                        self.messages.remove(pos);
                    }
                } else {
                    // Single Thinking block — update with authoritative content
                    self.messages[thinking_positions[0]] =
                        ChatMessage::thinking(thinking_content.to_string());
                }
            } else {
                // No thinking in final response — remove Thinking blocks
                // in the streaming zone only. Tool-call Thinking blocks
                // before the streaming zone (or with round_index > 0) are preserved.
                for &pos in thinking_positions.iter().rev() {
                    self.messages.remove(pos);
                }
            }
        }

        // Find and replace the last AssistantStreaming message.
        // Note: positions may have shifted after removing Thinking blocks above.
        if let Some(pos) = self
            .messages
            .iter()
            .rposition(|m| m.msg_type == MessageType::AssistantStreaming)
        {
            self.messages[pos] = ChatMessage::assistant_markdown(content.to_string());
        } else {
            // No streaming message found — just add the final one
            self.messages
                .push(ChatMessage::assistant_markdown(content.to_string()));
        }
        self.scroll.reset_to_bottom();
    }

    /// Update the completer's session name entries.
    ///
    /// Called after session-changing commands (/save, /load, /session forget)
    /// to keep the tab completion list current.
    pub fn refresh_session_entries(&mut self, entries: Vec<(String, String)>) {
        self.completer.set_session_entries(entries);
    }

    /// Get a mutable reference to the textarea
    pub fn textarea_mut(&mut self) -> &mut TextArea<'static> {
        &mut self.textarea
    }

    /// Get the current LLM state
    pub fn llm_state(&self) -> LlmState {
        self.llm_state
    }

    /// Get a mutable reference to the scroll state
    pub fn scroll_state_mut(&mut self) -> &mut ScrollState {
        &mut self.scroll
    }

    /// Get a reference to the chat selection state
    pub fn chat_selection(&self) -> &ChatSelection {
        &self.chat_selection
    }

    /// Get a mutable reference to the chat selection state
    pub fn chat_selection_mut(&mut self) -> &mut ChatSelection {
        &mut self.chat_selection
    }

    /// Get the cached scroll offset from top (for mouse mapping)
    pub fn scroll_from_top_cache(&self) -> u16 {
        self.scroll_from_top_cache
    }

    /// Get the cached chat area rect (for mouse mapping)
    pub fn chat_area_rect_cache(&self) -> ratatui::layout::Rect {
        self.chat_area_rect_cache
    }

    /// Get a reference to the status bar state.
    ///
    /// Used by `RatatuiView` to read current token counts for
    /// progress bar updates in `show_token_metrics()` and
    /// `show_context_warning()`.
    pub fn status_bar(&self) -> &StatusBarState {
        &self.status_bar
    }

    /// Whether style rendering is enabled (mermaid diagrams, LaTeX formulas, syntax
    /// highlighting, box-drawing tables).
    pub fn style_enabled(&self) -> bool {
        self.style_enabled
    }

    /// Toggle style rendering on/off and update the status bar indicator.
    pub fn toggle_style(&mut self) {
        self.style_enabled = !self.style_enabled;
        self.status_bar.style_enabled = self.style_enabled;
    }

    /// Scroll to the bottom of the chat (newest messages).
    ///
    /// Called on terminal resize to ensure newest content stays visible.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll.scroll_to_bottom();
    }

    /// Set the LLM state and update input/status accordingly.
    /// Clears `block_finalized` when transitioning to Idle.
    pub fn set_llm_state(&mut self, state: LlmState) {
        self.llm_state = state;
        if state == LlmState::Idle {
            self.block_finalized = false;
        }
        match state {
            LlmState::Idle => {
                self.input_disabled = false;
                self.disabled_reason = None;
                self.status_bar.spinner = None;
                self.status_bar.status_label = None;
                self.spinner_frame = 0;
            }
            LlmState::Thinking => {
                self.input_disabled = true;
                self.disabled_reason = Some("Thinking...".to_string());
                // Pick a new random spinner preset for this LLM cycle
                self.spinner_frames = random_tui_spinner_frames();
                self.spinner_frame = 0;
                let frame = self.spinner_frames.first().unwrap_or(&"⠋");
                self.status_bar.spinner = Some(frame.to_string());
                self.status_bar.status_label = Some("Thinking...".to_string());
            }
            LlmState::Streaming => {
                self.input_disabled = true;
                self.disabled_reason = Some("Streaming...".to_string());
                // Pick a new random spinner preset for the streaming phase
                // (distinct from the one used during thinking)
                self.spinner_frames = random_tui_spinner_frames();
                self.spinner_frame = 0;
                let frame = self.spinner_frames.first().unwrap_or(&"⠋");
                self.status_bar.spinner = Some(frame.to_string());
                self.status_bar.status_label = Some("Streaming...".to_string());
            }
            LlmState::Compacting => {
                self.input_disabled = true;
                self.disabled_reason = Some("Compacting...".to_string());
                self.spinner_frames = random_tui_spinner_frames();
                self.spinner_frame = 0;
                let frame = self.spinner_frames.first().unwrap_or(&"⠋");
                self.status_bar.spinner = Some(frame.to_string());
                self.status_bar.status_label = Some("Compacting...".to_string());
            }
            LlmState::ToolCall => {
                self.input_disabled = true;
                self.disabled_reason = Some("Running tool...".to_string());
                // Pick a new random spinner preset for this tool call cycle
                self.spinner_frames = random_tui_spinner_frames();
                self.spinner_frame = 0;
                let frame = self.spinner_frames.first().unwrap_or(&"⠋");
                self.status_bar.spinner = Some(frame.to_string());
                self.status_bar.status_label = Some("Running tool...".to_string());
            }
        }
    }

    /// Update the status bar model name and token info
    pub fn update_status_model(
        &mut self,
        model_name: &str,
        think_enabled: bool,
        tools_enabled: bool,
    ) {
        self.status_bar.model_name = model_name.to_string();
        self.status_bar.think_enabled = think_enabled;
        self.status_bar.tools_enabled = tools_enabled;
    }

    /// Update the status bar token counts
    pub fn update_status_tokens(&mut self, used_tokens: usize, max_tokens: usize, percent: u8) {
        self.status_bar.used_tokens = used_tokens;
        self.status_bar.max_tokens = max_tokens;
        self.status_bar.percent = percent;
    }

    /// Poll the embedding progress channel and update the status bar.
    ///
    /// Drains all messages from the channel, keeping only the latest
    /// progress update. Clears `embedding_progress` when the progress
    /// indicates completion.
    pub fn poll_embedding_progress(&mut self) {
        while let Ok(progress) = self.embedding_progress_rx.try_recv() {
            self.status_bar.embedding_progress = if progress.is_completed() {
                None
            } else {
                Some(progress)
            };
        }
    }

    /// Set the embedding progress indicator directly (for synchronous embedding operations).
    pub fn set_embedding_progress(&mut self, progress: EmbeddingProgress) {
        self.status_bar.embedding_progress = if progress.is_completed() {
            None
        } else {
            Some(progress)
        };
    }

    /// Poll for async system messages from background tasks.
    ///
    /// Background tasks (e.g., /reindex) send system message strings through
    /// the `async_message_rx` channel. This method drains the channel and
    /// adds each message to the chat area as a system message.
    pub fn poll_async_messages(&mut self) {
        while let Ok(msg) = self.async_message_rx.try_recv() {
            self.add_message(ChatMessage::system(msg));
        }
    }

    /// Advance the spinner frame.
    ///
    /// The spinner animates independently of streaming token arrival —
    /// it ticks via the spinner interval in the event loop,
    /// regardless of whether tokens are arriving or not.
    /// A fresh random rattles preset is picked each time the LLM enters
    /// a new phase (Thinking, Streaming, ToolCall), so the animation
    /// varies not just between sessions but between cycles.
    pub fn tick_spinner(&mut self) {
        if self.llm_state == LlmState::Idle || self.spinner_frames.is_empty() {
            return;
        }
        self.spinner_frame = (self.spinner_frame + 1) % self.spinner_frames.len();
        self.status_bar.spinner = Some(self.spinner_frames[self.spinner_frame].to_string());
    }

    /// Process a crossterm key event
    ///
    /// Returns `Some(InputResult::Line(line))` when Enter is pressed,
    /// `Some(InputResult::Interrupted)` for Ctrl+C,
    /// `Some(InputResult::Eof)` for Ctrl+D on empty line,
    /// and `None` for other key events.
    ///
    /// # Design
    ///
    /// We use `textarea.input_without_shortcuts()` for the default handler,
    /// which only handles char input, Tab, Backspace, Delete, and Enter (newline).
    /// All other shortcuts are bound explicitly here:
    ///
    /// **Submission & control:**
    /// - Enter: submit line (textarea default is newline)
    /// - Shift+Enter: newline
    /// - Ctrl+C: context-dependent copy/cancel:
    ///   - Chat selection active → copy selected chat text to clipboard
    ///   - Textarea selection active → copy selection, deselect (text preserved)
    ///   - Textarea has text (no selection) → select all, copy, clear input (cancel)
    ///   - Empty textarea → cancel LLM or exit
    /// - Ctrl+V: paste from system clipboard
    /// - Ctrl+D: EOF on empty, forward delete otherwise
    /// - Ctrl+Y: yank (paste from textarea kill-ring)
    /// - Tab: completion
    ///
    /// **Cursor movement (no selection):**
    /// - Ctrl+A/Home: move to start of line
    /// - Ctrl+E/End: move to end of line
    /// - Left/Right: character movement
    /// - Ctrl+Left/Ctrl+Right: word movement
    /// - Up/Down (single-line): history navigation
    /// - Up/Down (multi-line): textarea cursor movement
    ///
    /// **Selection (Shift modifier starts selection):**
    /// - Shift+Left/Right: select characters
    /// - Shift+Home/End: select to line start/end
    /// - Ctrl+Shift+Left/Right: select word
    ///
    /// **Editing:**
    /// - Ctrl+W: delete word backward (cut to system clipboard)
    /// - Ctrl+K: delete to end of line
    /// - Ctrl+U: undo
    /// - Ctrl+R: redo
    /// - Ctrl+X: cut selection to system clipboard
    ///
    /// When input is disabled (during LLM processing), only Ctrl+C
    /// and scroll keys are processed — all other keys are ignored.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Option<InputResult> {
        // Filter Release events — crossterm 0.29 sends both Press and Release
        // for each key event. We only handle Press to avoid double-processing.
        if key.kind != KeyEventKind::Press {
            return None;
        }

        // Diagnose key events in the log file (safe because TUI mode routes
        // all log output to the file, not stderr, so this won't corrupt the display).
        log::debug!(
            "handle_key: code={:?} modifiers={:?} kind={:?}",
            key.code,
            key.modifiers,
            key.kind
        );

        // ── Ctrl+C handling (always works) ───────────────────────────
        // Context-dependent: copy selection or cancel, with 4 priority levels:
        // 1. Chat selection active → copy chat text to clipboard
        // 2. Textarea selection active → copy selection, deselect (text preserved)
        // 3. Textarea has text (no selection) → select all, copy, clear (cancel input)
        // 4. Empty textarea → cancel LLM or exit

        if matches!(
            key,
            crossterm::event::KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
        ) {
            self.completion_menu.hide();

            // Priority 1: Chat selection active → copy selected chat text to clipboard
            if self.chat_selection.is_active() {
                let text = self.chat_selection.extract_text(&self.visual_lines_cache);
                if !text.is_empty() {
                    let _ = crate::clipboard::set_contents(text);
                }
                self.chat_selection.clear();
                return None;
            }

            // Priority 2 & 3: Textarea has content
            if !self.textarea_is_empty() {
                if self.textarea.is_selecting() {
                    // Priority 2: Selection in textarea → copy selection, deselect
                    // Text is preserved — only the selection is canceled
                    self.textarea.copy();
                    if let Some(text) = self.yank_text()
                        && !text.is_empty()
                    {
                        let _ = crate::clipboard::set_contents(text);
                    }
                    self.textarea.cancel_selection();
                } else {
                    // Priority 3: No selection → select all, copy, then clear (cancel input)
                    self.textarea.select_all();
                    self.textarea.copy();
                    if let Some(text) = self.yank_text()
                        && !text.is_empty()
                    {
                        let _ = crate::clipboard::set_contents(text);
                    }
                    self.textarea_clear();
                }
                return None;
            }

            // Priority 4: Empty textarea → cancel LLM or no-op
            return Some(InputResult::Interrupted);
        }

        // ── Ctrl+V — paste from system clipboard (always works) ──────
        // Terminal emulators like kitty intercept Ctrl+Shift+C/V, so we
        // use Ctrl+V for paste instead. This works reliably across all
        // terminals because it's a standard key event that crossterm receives.

        if matches!(
            key,
            crossterm::event::KeyEvent {
                code: KeyCode::Char('v'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
        ) {
            self.completion_menu.hide();
            if let Ok(text) = crate::clipboard::get_contents()
                && !text.is_empty()
            {
                self.textarea.insert_str(&text);
                self.chat_selection.clear();
            }
            return None;
        }

        // ── Input disabled (LLM processing) ─────────────────────────

        if self.input_disabled {
            match key.code {
                KeyCode::PageUp => {
                    self.scroll.scroll_up(10);
                }
                KeyCode::PageDown => {
                    self.scroll.scroll_down(10);
                }
                KeyCode::Home => {
                    self.scroll.scroll_to_top();
                }
                KeyCode::End => {
                    self.scroll.scroll_to_bottom();
                }
                _ => {}
            }
            return None;
        }

        // ── Mutual exclusion: typing clears chat selection ───────────

        if self.chat_selection.is_active() {
            // Scroll and navigation keys don't clear chat selection
            match key.code {
                KeyCode::PageUp | KeyCode::PageDown | KeyCode::Tab => {}
                _ => {
                    self.chat_selection.clear();
                }
            }
        }

        // ── Completion menu (when visible) ───────────────────────────

        if self.completion_menu.is_visible() {
            match key {
                // Tab — confirm selection, hide menu, try sub-completion
                crossterm::event::KeyEvent {
                    code: KeyCode::Tab,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    if let Some(item) = self.completion_menu.confirm() {
                        let replacement = format!("{} ", item);
                        self.set_textarea_content(&replacement);
                        // Try sub-completion (e.g., /model → model names)
                        self.try_completion_after_confirm();
                    }
                    return None;
                }

                // Enter — confirm selection, hide menu, submit input
                crossterm::event::KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    // Confirm the menu selection (if any) into the textarea
                    if let Some(item) = self.completion_menu.confirm() {
                        let replacement = format!("{} ", item);
                        self.set_textarea_content(&replacement);
                    }
                    // Menu is now hidden (confirm() hides it).
                    // Fall through to the Enter handler below to submit the line.
                    // We do NOT return None here — we want Enter to submit.
                }

                // Escape — dismiss menu
                crossterm::event::KeyEvent {
                    code: KeyCode::Esc, ..
                } => {
                    self.completion_menu.hide();
                    return None;
                }

                // Up — navigate menu
                crossterm::event::KeyEvent {
                    code: KeyCode::Up,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    self.completion_menu.select_up();
                    return None;
                }

                // Down — navigate menu
                crossterm::event::KeyEvent {
                    code: KeyCode::Down,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    self.completion_menu.select_down();
                    return None;
                }

                // Any other key — dismiss menu and fall through
                _ => {
                    self.completion_menu.hide();
                    // Don't return — fall through to normal handling below
                }
            }
        }

        // ── Key-specific handling ────────────────────────────────────

        match key {
            // ============================================================
            // Submission & control
            // ============================================================

            // Enter — submit the line
            crossterm::event::KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let line = self.textarea_lines();
                self.textarea_clear();
                self.history_input.history_pos = None; // Reset history navigation on submit
                self.chat_selection.clear();
                if !line.is_empty() {
                    self.history_input.add_history(&line);
                }
                self.scroll.reset_to_bottom();
                Some(InputResult::Line(line))
            }

            // Shift+Enter — newline
            crossterm::event::KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                self.textarea.insert_newline();
                None
            }

            // Alt+Enter — newline (fallback for terminals that don't
            // distinguish Shift+Enter from Enter, e.g. most Linux terminals)
            crossterm::event::KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::ALT,
                ..
            } => {
                self.textarea.insert_newline();
                None
            }

            // Ctrl+Y — yank (paste from textarea kill-ring)
            // Kill-ring is populated by Ctrl+W (delete word), Ctrl+K (delete
            // to EOL), and Ctrl+X (cut selection). This is standard Emacs
            // behavior and the default in ratatui-textarea.
            crossterm::event::KeyEvent {
                code: KeyCode::Char('y'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.textarea.paste();
                None
            }

            // Ctrl+W — delete word backward (cut to system clipboard)
            crossterm::event::KeyEvent {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.textarea.delete_word();
                if let Some(text) = self.yank_text()
                    && !text.is_empty()
                {
                    let _ = crate::clipboard::set_contents(text);
                }
                None
            }

            // Ctrl+K — delete from cursor to end of line
            crossterm::event::KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.textarea.delete_line_by_end();
                None
            }

            // Ctrl+X — cut selection to system clipboard
            crossterm::event::KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.textarea.is_selecting() {
                    self.textarea.cut();
                    if let Some(text) = self.yank_text()
                        && !text.is_empty()
                    {
                        let _ = crate::clipboard::set_contents(text);
                    }
                }
                None
            }

            // Ctrl+U — undo
            crossterm::event::KeyEvent {
                code: KeyCode::Char('u'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.textarea.undo();
                None
            }

            // Ctrl+R — redo
            crossterm::event::KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.textarea.redo();
                None
            }

            // Ctrl+D — EOF on empty line, forward delete otherwise
            #[allow(clippy::collapsible_match)]
            crossterm::event::KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.textarea_is_empty() {
                    Some(InputResult::Eof)
                } else {
                    self.textarea.delete_next_char();
                    None
                }
            }

            // Ctrl+A — move to start of line (no selection)
            // (with Shift: select to start of line — handled in shift+home below)
            crossterm::event::KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::Head);
                None
            }

            // Ctrl+E — move to end of line (no selection)
            crossterm::event::KeyEvent {
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::End);
                None
            }

            // ============================================================
            // Cursor movement (no Shift = move, Shift = select)
            // ============================================================

            // Left — character back (or select if Shift)
            crossterm::event::KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::Back);
                None
            }
            crossterm::event::KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                if !self.textarea.is_selecting() {
                    self.textarea.start_selection();
                }
                self.textarea.move_cursor(CursorMove::Back);
                None
            }

            // Right — character forward (or select if Shift)
            crossterm::event::KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::Forward);
                None
            }
            crossterm::event::KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                if !self.textarea.is_selecting() {
                    self.textarea.start_selection();
                }
                self.textarea.move_cursor(CursorMove::Forward);
                None
            }

            // Ctrl+Left / Ctrl+Shift+Left — word back (select if Shift held)
            crossterm::event::KeyEvent {
                code: KeyCode::Left,
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL)
                && !modifiers.contains(KeyModifiers::ALT) =>
            {
                if modifiers.contains(KeyModifiers::SHIFT) {
                    if !self.textarea.is_selecting() {
                        self.textarea.start_selection();
                    }
                } else {
                    self.textarea.cancel_selection();
                }
                self.textarea.move_cursor(CursorMove::WordBack);
                None
            }

            // Ctrl+Right / Ctrl+Shift+Right — word forward (select if Shift held)
            crossterm::event::KeyEvent {
                code: KeyCode::Right,
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL)
                && !modifiers.contains(KeyModifiers::ALT) =>
            {
                if modifiers.contains(KeyModifiers::SHIFT) {
                    if !self.textarea.is_selecting() {
                        self.textarea.start_selection();
                    }
                } else {
                    self.textarea.cancel_selection();
                }
                self.textarea.move_cursor(CursorMove::WordForward);
                None
            }

            // Up (no shift) — history nav or textarea cursor up
            crossterm::event::KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if self.textarea_is_multiline() {
                    self.textarea.cancel_selection();
                    self.textarea.move_cursor(CursorMove::Up);
                } else {
                    self.history_prev();
                }
                None
            }
            // Shift+Up — select up (multiline only) or history prev (single-line)
            crossterm::event::KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                if self.textarea_is_multiline() {
                    if !self.textarea.is_selecting() {
                        self.textarea.start_selection();
                    }
                    self.textarea.move_cursor(CursorMove::Up);
                } else {
                    self.history_prev();
                }
                None
            }

            // Down (no shift) — history nav or textarea cursor down
            crossterm::event::KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if self.textarea_is_multiline() {
                    self.textarea.cancel_selection();
                    self.textarea.move_cursor(CursorMove::Down);
                } else {
                    self.history_next();
                }
                None
            }
            // Shift+Down — select down (multiline only) or history next (single-line)
            crossterm::event::KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                if self.textarea_is_multiline() {
                    if !self.textarea.is_selecting() {
                        self.textarea.start_selection();
                    }
                    self.textarea.move_cursor(CursorMove::Down);
                } else {
                    self.history_next();
                }
                None
            }

            // Home — move to start of line (or select if Shift)
            crossterm::event::KeyEvent {
                code: KeyCode::Home,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::Head);
                None
            }
            crossterm::event::KeyEvent {
                code: KeyCode::Home,
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                if !self.textarea.is_selecting() {
                    self.textarea.start_selection();
                }
                self.textarea.move_cursor(CursorMove::Head);
                None
            }

            // End — move to end of line (or select if Shift)
            crossterm::event::KeyEvent {
                code: KeyCode::End,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.textarea.cancel_selection();
                self.textarea.move_cursor(CursorMove::End);
                None
            }
            crossterm::event::KeyEvent {
                code: KeyCode::End,
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                if !self.textarea.is_selecting() {
                    self.textarea.start_selection();
                }
                self.textarea.move_cursor(CursorMove::End);
                None
            }

            // ============================================================
            // Scroll & navigation
            // ============================================================

            // PageUp — scroll chat up
            crossterm::event::KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.scroll.scroll_up(10);
                None
            }

            // PageDown — scroll chat down
            crossterm::event::KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.scroll.scroll_down(10);
                None
            }

            // Tab — attempt completion
            crossterm::event::KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.try_tab_complete();
                None
            }

            // ============================================================
            // Shift+Char — insert uppercase character
            // ============================================================
            // Crossterm terminals may send Char('v') with SHIFT or
            // Char('V') with SHIFT depending on the platform. The
            // textarea's input_without_shortcuts() matches Char(c) with
            // ctrl:false + alt:false (ignoring shift), which inserts the
            // character as-is. When the terminal sends lowercase 'v' with
            // SHIFT, the textarea inserts 'v' — losing the Shift. This
            // handler normalizes Shift+letter to always produce the
            // uppercase character, regardless of what the terminal sends.
            crossterm::event::KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::SHIFT,
                ..
            } if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.textarea.insert_char(c.to_ascii_uppercase());
                self.auto_complete_on_type();
                None
            }

            // ============================================================
            // All other keys: pass to textarea (basic editing only)
            // ============================================================
            // input_without_shortcuts() handles: char input, Backspace,
            // Delete, and Enter (which inserts newline in textarea).
            // We intercept Enter above, so only basic editing falls through.
            _ => {
                self.textarea.input_without_shortcuts(key);
                // Auto-trigger completion when typing slash commands
                self.auto_complete_on_type();
                None
            }
        }
    }

    /// Check if textarea is empty (no user content)
    fn textarea_is_empty(&self) -> bool {
        self.textarea.lines().iter().all(|line| line.is_empty())
    }

    /// Check if textarea has multiple lines of content
    fn textarea_is_multiline(&self) -> bool {
        self.textarea.lines().len() > 1
    }

    /// Get the textarea content as a single String (lines joined by \n)
    fn textarea_lines(&self) -> String {
        self.textarea.lines().join("\n")
    }

    /// Clear the textarea content
    fn textarea_clear(&mut self) {
        // TextArea::clear() returns bool but we don't need it here
        let _ = self.textarea.clear();
    }

    /// Get the last yanked/cut text (for clipboard copy)
    fn yank_text(&self) -> Option<String> {
        let text = self.textarea.yank_text();
        if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        }
    }

    /// Navigate to previous history entry
    ///
    /// Loads the previous command from history into the textarea.
    /// On transition into history mode, saves the current textarea content.
    fn history_prev(&mut self) {
        if self.history_input.history.is_empty() {
            return;
        }

        // If not navigating, save current buffer
        if self.history_input.history_pos.is_none() {
            self.history_input.saved_buffer = self.textarea_lines();
            self.history_input.history_pos =
                Some(self.history_input.history.len().saturating_sub(1));
        } else if let Some(pos) = self.history_input.history_pos {
            if pos > 0 {
                self.history_input.history_pos = Some(pos - 1);
            } else {
                return; // Already at oldest
            }
        }

        if let Some(pos) = self.history_input.history_pos {
            self.set_textarea_content(&self.history_input.history[pos].clone());
        }
    }

    /// Navigate to next history entry
    ///
    /// When navigating past the newest entry, restores the saved buffer.
    fn history_next(&mut self) {
        match self.history_input.history_pos {
            None => {}
            Some(pos) => {
                if pos + 1 >= self.history_input.history.len() {
                    // Past the newest entry: restore saved buffer
                    self.history_input.history_pos = None;
                    self.set_textarea_content(&self.history_input.saved_buffer.clone());
                } else {
                    self.history_input.history_pos = Some(pos + 1);
                    self.set_textarea_content(&self.history_input.history[pos + 1].clone());
                }
            }
        }
    }

    /// Set textarea content from a string (replaces all lines).
    ///
    /// Splits on newlines, clears the textarea, and inserts the new content
    /// with cursor at end.
    fn set_textarea_content(&mut self, text: &str) {
        let _ = self.textarea.clear();
        if !text.is_empty() {
            // Insert string handles both \n and \r\n
            self.textarea.insert_str(text);
        }
    }

    /// Attempt tab completion based on current textarea content.
    ///
    /// Uses `ChatCompleter` to find slash command or model name completions.
    /// On single match: replaces the content with the completed text.
    /// On multiple matches: shows the completion menu overlay.
    /// When no matches: does nothing (bell could be added later).
    fn try_tab_complete(&mut self) {
        use super::completer::CompletionResult;

        let buffer = self.textarea_lines();
        let cursor_pos = self.cursor_byte_offset();

        let result = self.completer.complete(&buffer, cursor_pos);

        match result {
            CompletionResult::None => {
                // No completion — hide menu if visible
                self.completion_menu.hide();
            }
            CompletionResult::Single {
                replacement,
                cursor_pos,
            } => {
                self.completion_menu.hide();
                self.set_textarea_content(&replacement);
                // Move cursor to the specified position
                self.set_cursor_to_byte_offset(cursor_pos);
            }
            CompletionResult::Multiple {
                matches,
                descriptions,
            } => {
                // Compute common prefix for highlighting
                let common = common_prefix_str(&matches);
                self.completion_menu.show(matches, descriptions, common);
            }
        }
    }

    /// Try sub-completion after confirming a completion menu selection.
    ///
    /// After confirming a slash command (e.g., `/model`), checks if the
    /// command takes arguments and if so, shows the completion menu for
    /// those arguments. This enables recursive completion:
    /// `/mo` → Tab → `/model ` → model name list appears.
    ///
    /// Note: We always SHOW the menu, never auto-replace text. The user
    /// explicitly selects with Tab/Enter or dismisses with Esc.
    fn try_completion_after_confirm(&mut self) {
        let buffer = self.textarea_lines();
        let cursor_pos = self.cursor_byte_offset();

        if cursor_pos == buffer.len() && buffer.starts_with('/') {
            let result = self.completer.complete(&buffer, cursor_pos);
            match result {
                super::completer::CompletionResult::None => {
                    self.completion_menu.hide();
                }
                super::completer::CompletionResult::Single { replacement, .. } => {
                    // Single sub-completion match: show as a one-item menu
                    // so the user can Tab/Enter to confirm, or Esc to dismiss.
                    // Don't auto-replace — the user decides.
                    self.completion_menu.show(
                        vec![replacement.clone()],
                        vec![String::new()],
                        replacement.clone(),
                    );
                }
                super::completer::CompletionResult::Multiple {
                    matches,
                    descriptions,
                } => {
                    let common = common_prefix_str(&matches);
                    self.completion_menu.show(matches, descriptions, common);
                }
            }
        }
    }

    /// Auto-trigger completion menu when typing slash commands.
    ///
    /// After each character is typed, checks if the current input starts
    /// with `/` and has completions available. Shows the menu to display
    /// options, but NEVER replaces the text — the user must explicitly
    /// press Tab or Enter to confirm a selection.
    ///
    /// This avoids the "stuck input" problem where auto-completion would
    /// replace the text while the user is still typing.
    fn auto_complete_on_type(&mut self) {
        let buffer = self.textarea_lines();
        let cursor_pos = self.cursor_byte_offset();

        // Only auto-complete at end of buffer for slash commands
        if cursor_pos != buffer.len() || !buffer.starts_with('/') {
            // Hide menu if we're no longer in slash command context
            if self.completion_menu.is_visible() && !buffer.starts_with('/') {
                self.completion_menu.hide();
            }
            return;
        }

        let result = self.completer.complete(&buffer, cursor_pos);
        match result {
            super::completer::CompletionResult::None => {
                self.completion_menu.hide();
            }
            super::completer::CompletionResult::Single { replacement, .. } => {
                // Single match: don't auto-replace, just hide the menu.
                // The user typed enough to be unique — they can press Tab
                // if they want to complete, or keep typing.
                // Show as one-item menu so they know what's available.
                self.completion_menu.show(
                    vec![replacement.clone()],
                    vec![String::new()],
                    replacement.clone(),
                );
            }
            super::completer::CompletionResult::Multiple {
                matches,
                descriptions,
            } => {
                let common = common_prefix_str(&matches);
                self.completion_menu.show(matches, descriptions, common);
            }
        }
    }

    /// Get the byte offset of the cursor in the textarea.
    ///
    /// Calculates the byte position by summing line lengths + newlines
    /// for all lines before the cursor's line, plus the cursor column.
    fn cursor_byte_offset(&self) -> usize {
        let cursor = self.textarea.cursor();
        let row = cursor.0;
        let col = cursor.1;
        let lines = self.textarea.lines();
        let mut offset = 0;
        for (i, line) in lines.iter().enumerate() {
            if i == row {
                // Count characters (not bytes) in the line up to col,
                // then get byte offset of that character boundary
                let char_offset = col.min(line.chars().count());
                return offset + line.chars().take(char_offset).collect::<String>().len();
            }
            offset += line.len() + 1; // +1 for newline
        }
        offset
    }

    /// Set the cursor to a specific byte offset in the textarea.
    ///
    /// Navigates through lines to find the row and column that
    /// corresponds to the given byte offset.
    fn set_cursor_to_byte_offset(&mut self, byte_offset: usize) {
        let lines = self.textarea.lines();
        let mut remaining = byte_offset;
        for (row, line) in lines.iter().enumerate() {
            if remaining <= line.len() {
                // Find the character column that corresponds to this byte offset
                let mut byte_pos = 0;
                let mut col: u16 = 0;
                for ch in line.chars() {
                    byte_pos += ch.len_utf8();
                    if byte_pos > remaining {
                        break;
                    }
                    col += 1;
                }
                // If remaining is past all chars, cursor goes to end of line
                if remaining > line.len() {
                    col = line.chars().count() as u16;
                }
                self.textarea
                    .move_cursor(ratatui_textarea::CursorMove::Jump(row as u16, col));
                return;
            }
            remaining = remaining.saturating_sub(line.len() + 1);
        }
        // If byte_offset is past the end, move to bottom
        self.textarea
            .move_cursor(ratatui_textarea::CursorMove::Bottom);
    }

    /// Save history to file
    pub fn save_history(&mut self) -> Result<(), String> {
        self.history_input.save_history()
    }

    /// Render the TUI
    pub fn render(
        &mut self,
        terminal: &mut TuiTerminal,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        terminal.draw(|f| {
            let size = f.area();

            // Input line height adapts to multi-line content with word-wrap.
            // Use cached screen lines (populated after the first render) or
            // fall back to logical line count for the initial frame.
            let screen_lines = if self.cached_input_screen_lines > 0 {
                self.cached_input_screen_lines
            } else {
                self.textarea.lines().len().max(1)
            };
            // Cap input at 1/3 of total height, minimum 3 lines.
            let max_input_lines = (size.height as usize / 3).max(3);
            let input_height = (screen_lines.min(max_input_lines) as u16).max(1);

            // Layout: chat area (flexible) | status bar (2 lines) | input line (dynamic)
            let chunks = Layout::vertical([
                Constraint::Min(3),               // Chat area gets all remaining space
                Constraint::Length(2),            // Status bar (separator + content)
                Constraint::Length(input_height), // Input line (grows with multi-line)
            ])
            .split(size);

            // Render chat area
            let meta = super::tui::components::chat_area::render(
                f,
                chunks[0],
                &self.messages,
                &mut self.scroll,
                self.theme,
                self.style_enabled,
                &self.chat_selection,
            );

            // Cache visual lines, scroll offset, source line map, and chat area rect for
            // mouse/selection integration (updated every render cycle)
            self.visual_lines_cache = meta.visual_lines;
            self.source_line_map_cache = meta.source_line_map;
            self.scroll_from_top_cache = meta.scroll_from_top;
            self.chat_area_rect_cache = chunks[0];

            // Render status bar
            super::tui::components::status_bar::render(f, chunks[1], &self.status_bar);

            // Render input line and get the number of wrapped visual lines
            let rendered_lines = super::tui::components::input_line::render(
                f,
                chunks[2],
                &self.textarea,
                self.input_disabled,
                self.disabled_reason.as_deref(),
            );
            self.cached_input_screen_lines = rendered_lines.max(1);

            // Render completion menu overlay (above the status bar)
            // This is drawn LAST so it floats on top of other widgets
            if self.completion_menu.is_visible() {
                super::tui::components::completion_menu::render_overlay(
                    f,
                    chunks[1],
                    &self.completion_menu,
                );
            }
        })?;

        Ok(())
    }
}

/// Find the common prefix among a list of strings.
///
/// Used to highlight the shared portion of completion items in the menu.
fn common_prefix_str(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }

    let first = strings[0].as_bytes();
    let mut prefix_len = first.len();

    for s in &strings[1..] {
        let bytes = s.as_bytes();
        let mut j = 0;
        while j < prefix_len && j < bytes.len() && first[j] == bytes[j] {
            j += 1;
        }
        prefix_len = j;
        if prefix_len == 0 {
            return String::new();
        }
    }

    String::from_utf8_lossy(&first[..prefix_len]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal App for testing streaming message operations.
    fn test_app() -> App {
        let (app, _embedding_tx, _async_message_tx) =
            App::with_embedding_channel(MarkdownTheme::Dark, vec!["test-model".to_string()]);
        app
    }

    // ── append_stream_thinking tests ─────────────────────────────

    #[test]
    fn test_append_stream_thinking_happy_path() {
        // Consecutive thinking tokens append to the same block
        let mut app = test_app();
        app.append_stream_thinking("Hello");
        app.append_stream_thinking(" world");

        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].content, "Hello world");
    }

    #[test]
    fn test_append_stream_thinking_interleaved_after_streaming() {
        // Thinking token arrives when last message is AssistantStreaming
        // → should find and append to the existing Thinking block
        let mut app = test_app();
        app.append_stream_thinking("Let me think"); // Thinking created
        app.append_stream_token("Here is"); // AssistantStreaming created
        app.append_stream_thinking(" more"); // Should append to Thinking, not create new

        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].content, "Let me think more");
        assert_eq!(app.messages[1].msg_type, MessageType::AssistantStreaming);
        assert_eq!(app.messages[1].content, "Here is");
    }

    // ── append_stream_token tests ────────────────────────────────

    #[test]
    fn test_append_stream_token_happy_path() {
        // Consecutive content tokens append to the same block
        let mut app = test_app();
        app.append_stream_token("Hello");
        app.append_stream_token(" world");

        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].msg_type, MessageType::AssistantStreaming);
        assert_eq!(app.messages[0].content, "Hello world");
    }

    #[test]
    fn test_append_stream_token_interleaved_after_thinking() {
        // Content token arrives when last message is Thinking
        // → should find and append to the existing AssistantStreaming block
        let mut app = test_app();
        app.append_stream_thinking("Hmm"); // Thinking created
        app.append_stream_token("Answer:"); // AssistantStreaming created
        app.append_stream_thinking(" wait"); // Thinking updated
        app.append_stream_token(" 42"); // Should append to AssistantStreaming, not create new

        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].content, "Hmm wait");
        assert_eq!(app.messages[1].msg_type, MessageType::AssistantStreaming);
        assert_eq!(app.messages[1].content, "Answer: 42");
    }

    // ── full interleaving simulation ─────────────────────────────

    #[test]
    fn test_thinking_content_interleaving_no_fragmentation() {
        // Simulates the bug scenario: thinking and content tokens arriving
        // interleaved should NOT create fragmented message blocks.
        // Before the fix, this would create:
        //   [Thinking("The user wants..."), AssistantStreaming("B"), Thinking(".")]
        // After the fix, this should create:
        //   [Thinking("The user wants another test table..."), AssistantStreaming("Bora!")]
        let mut app = test_app();

        // Phase 1: thinking tokens start arriving
        app.append_stream_thinking("The user wants another test table.");
        // Phase 2: content token arrives before thinking finishes
        app.append_stream_token("B");
        // Phase 3: more thinking tokens arrive (interleaved)
        app.append_stream_thinking(" Let me render it.");
        // Phase 4: more content tokens
        app.append_stream_token("ora! Mais uma:");

        assert_eq!(
            app.messages.len(),
            2,
            "Should have exactly 2 messages (Thinking + AssistantStreaming)"
        );
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(
            app.messages[0].content,
            "The user wants another test table. Let me render it."
        );
        assert_eq!(app.messages[1].msg_type, MessageType::AssistantStreaming);
        assert_eq!(app.messages[1].content, "Bora! Mais uma:");
    }

    // ── finalize_stream tests ─────────────────────────────────────

    #[test]
    fn test_finalize_stream_consolidates_thinking_blocks() {
        // Multiple fragmented Thinking blocks should be consolidated into one
        let mut app = test_app();
        app.append_stream_thinking("First part");
        app.append_stream_token("Response");
        app.append_stream_thinking(" second part");

        // Before finalize, we should have 2 messages (no fragmentation)
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].content, "First part second part");
        assert_eq!(app.messages[1].msg_type, MessageType::AssistantStreaming);

        // Finalize with consolidated thinking content
        app.finalize_stream("Response", Some("First part second part"));

        // After finalize: Thinking consolidated, AssistantStreaming → Assistant
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].content, "First part second part");
        assert_eq!(app.messages[1].msg_type, MessageType::Assistant);
    }

    #[test]
    fn test_finalize_stream_removes_thinking_when_none() {
        // When finalize is called with no thinking content, all Thinking
        // blocks should be removed
        let mut app = test_app();
        app.append_stream_thinking("hmm");
        app.append_stream_token("Answer");

        app.finalize_stream("Answer", None);

        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[0].content, "Answer");
    }

    #[test]
    fn test_finalize_stream_no_thinking_at_all() {
        // Response with no thinking at all
        let mut app = test_app();
        app.append_stream_token("Hello");
        app.append_stream_token(" world");

        app.finalize_stream("Hello world", None);

        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[0].content, "Hello world");
    }

    #[test]
    fn test_finalize_stream_preserves_tool_thinking_blocks() {
        // Tool-call Thinking blocks created BEFORE the streaming zone
        // (preceded by Tool, Assistant, User messages) must NOT be
        // removed or consolidated by finalize_stream.
        let mut app = test_app();

        // Simulate a prior tool call round:
        // Tool thinking → Tool result → (tool output already displayed)
        app.add_message(ChatMessage::thinking("Tool thinking".to_string()));
        app.add_message(ChatMessage::tool("🔧 weather: Sunny, 22°C".to_string()));

        // Now the streaming session begins:
        app.append_stream_thinking("Response thinking");
        app.append_stream_token("Response content");

        // Finalize — only the streaming-zone Thinking block should be touched
        app.finalize_stream("Response content", Some("Response thinking"));

        // Tool thinking block is preserved (before the streaming zone)
        // Streaming thinking is consolidated
        // AssistantStreaming → Assistant
        assert_eq!(
            app.messages.len(),
            4,
            "Should have 4 messages: ToolThinking, Tool, Thinking, Assistant"
        );
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].content, "Tool thinking");
        assert_eq!(app.messages[1].msg_type, MessageType::Tool);
        assert_eq!(app.messages[2].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[2].content, "Response thinking");
        assert_eq!(app.messages[3].msg_type, MessageType::Assistant);
    }

    #[test]
    fn test_finalize_stream_removes_streaming_thinking_when_none() {
        // When thinking is None, only Thinking blocks in the streaming zone
        // should be removed. Tool-call Thinking blocks before the zone
        // must be preserved.
        let mut app = test_app();

        // Prior tool-call thinking (before streaming zone)
        app.add_message(ChatMessage::thinking("Tool thinking".to_string()));
        app.add_message(ChatMessage::tool("Tool result".to_string()));

        // Streaming zone: Thinking + AssistantStreaming
        app.append_stream_thinking("Stream thinking");
        app.append_stream_token("Stream content");

        // Finalize with thinking: None — streaming Thinking should be removed
        app.finalize_stream("Stream content", None);

        // Tool thinking preserved, streaming thinking removed
        assert_eq!(app.messages.len(), 3);
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].content, "Tool thinking");
        assert_eq!(app.messages[1].msg_type, MessageType::Tool);
        assert_eq!(app.messages[2].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[2].content, "Stream content");
    }

    #[test]
    fn test_finalize_stream_multiple_tool_thinking_preserved() {
        // Multiple rounds of tool calls with thinking, followed by streaming.
        // Tool-call Thinking blocks are outside the streaming zone (separated
        // by Tool messages) and must be preserved intact.
        let mut app = test_app();

        // First tool call round
        app.add_message(ChatMessage::thinking("Tool call 1 thinking".to_string()));
        app.add_message(ChatMessage::tool("🔧 weather".to_string()));

        // Second tool call round
        app.add_message(ChatMessage::thinking("Tool call 2 thinking".to_string()));
        app.add_message(ChatMessage::tool("🔧 calc: 42".to_string()));

        // Final streaming response
        app.append_stream_thinking("Final thinking");
        app.append_stream_token("Final answer");

        app.finalize_stream("Final answer", Some("Final thinking"));

        // All tool thinking blocks preserved, streaming thinking consolidated,
        // AssistantStreaming replaced by Assistant
        // Messages: [ToolThink, Tool, ToolThink, Tool, Think, Assistant]
        assert_eq!(app.messages.len(), 6);
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].content, "Tool call 1 thinking");
        assert_eq!(app.messages[1].msg_type, MessageType::Tool);
        assert_eq!(app.messages[2].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[2].content, "Tool call 2 thinking");
        assert_eq!(app.messages[3].msg_type, MessageType::Tool);
        assert_eq!(app.messages[4].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[4].content, "Final thinking");
        assert_eq!(app.messages[5].msg_type, MessageType::Assistant);
    }

    #[test]
    fn test_finalize_stream_thinking_only_response() {
        // Response that only has thinking, no content tokens
        let mut app = test_app();
        app.append_stream_thinking("Deep thoughts");

        // No content was ever streamed — finalize should add Assistant message
        app.finalize_stream("", Some("Deep thoughts"));

        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].content, "Deep thoughts");
        assert_eq!(app.messages[1].msg_type, MessageType::Assistant);
    }

    #[test]
    fn test_full_interleaving_scenario_then_finalize() {
        // Full simulation: interleaved thinking/content tokens → finalize
        // This is the exact bug scenario from the user report
        let mut app = test_app();

        // Simulate the exact problematic sequence
        app.append_stream_thinking(
            "The user wants another test table. Let me render another one with different content",
        );
        app.append_stream_token("B");
        app.append_stream_thinking(".");
        app.append_stream_token("ora! Mais uma:");

        // Verify no fragmentation during streaming
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(
            app.messages[0].content,
            "The user wants another test table. Let me render another one with different content."
        );
        assert_eq!(app.messages[1].msg_type, MessageType::AssistantStreaming);
        assert_eq!(app.messages[1].content, "Bora! Mais uma:");

        // Finalize with complete content
        app.finalize_stream(
            "Bora! Mais uma:\n\n| A | B |\n|---|---|\n| 1 | 2 |",
            Some("The user wants another test table. Let me render another one with different content."),
        );

        // Verify final state: one Thinking block + one Assistant block
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(
            app.messages[0].content,
            "The user wants another test table. Let me render another one with different content."
        );
        assert_eq!(app.messages[1].msg_type, MessageType::Assistant);
        assert_eq!(
            app.messages[1].content,
            "Bora! Mais uma:\n\n| A | B |\n|---|---|\n| 1 | 2 |"
        );
    }

    // ── insert_before_streaming_zone tests ──────────────────────────

    #[test]
    fn test_insert_before_streaming_zone_with_zone() {
        // Tool message should be inserted before the streaming zone
        let mut app = test_app();

        // Simulate: User, then streaming zone (Thinking + AssistantStreaming)
        app.add_message(ChatMessage::user("Search for weather".to_string()));
        app.append_stream_thinking("Let me search");
        app.append_stream_token("The weather is");

        // Insert a Tool message before the streaming zone
        app.insert_before_streaming_zone(ChatMessage::tool("🔧 weather: Sunny".to_string()));

        // Tool message should appear between User and Thinking
        assert_eq!(app.messages.len(), 4);
        assert_eq!(app.messages[0].msg_type, MessageType::User);
        assert_eq!(app.messages[1].msg_type, MessageType::Tool);
        assert_eq!(app.messages[1].content, "🔧 weather: Sunny");
        assert_eq!(app.messages[2].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[3].msg_type, MessageType::AssistantStreaming);
    }

    #[test]
    fn test_insert_before_streaming_zone_no_zone() {
        // When there's no streaming zone and no trailing tools, append
        let mut app = test_app();

        app.add_message(ChatMessage::user("Hello".to_string()));

        // No streaming zone, no trailing tools — append
        app.insert_before_streaming_zone(ChatMessage::tool("Tool msg".to_string()));

        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].msg_type, MessageType::User);
        assert_eq!(app.messages[1].msg_type, MessageType::Tool);
        assert_eq!(app.messages[1].content, "Tool msg");
    }

    #[test]
    fn test_insert_before_streaming_zone_no_zone_trailing_tools() {
        // When there's no streaming zone but there ARE trailing tool messages,
        // insert BEFORE them. This is the InterToolText ordering fix:
        // inter-tool text must appear BETWEEN tool rounds, not after them.
        let mut app = test_app();

        app.add_message(ChatMessage::user("What's the weather?".to_string()));
        app.add_message(ChatMessage::assistant_markdown("Let me check".to_string()));
        // Trailing tool messages from round 1 (already drained)
        app.add_message(ChatMessage::tool("🔧 weather()".to_string()));
        app.add_message(ChatMessage::tool("Sunny, 22°C".to_string()));

        // InterToolText arrives — should insert BEFORE tool messages
        app.insert_before_streaming_zone(ChatMessage::assistant_markdown(
            "Now let me calculate:".to_string(),
        ));

        assert_eq!(app.messages.len(), 5);
        assert_eq!(app.messages[0].msg_type, MessageType::User);
        assert_eq!(app.messages[1].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[1].content, "Let me check");
        // Inter-tool text inserted BEFORE tool messages
        assert_eq!(app.messages[2].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[2].content, "Now let me calculate:");
        assert_eq!(app.messages[3].msg_type, MessageType::Tool);
        assert_eq!(app.messages[4].msg_type, MessageType::Tool);
    }

    #[test]
    fn test_insert_before_streaming_zone_mixed_trailing() {
        // Trailing messages: Assistant, Tool, Tool, Tool
        // Insert should go BEFORE the Tool messages, after the Assistant
        let mut app = test_app();

        app.add_message(ChatMessage::user("Question".to_string()));
        app.add_message(ChatMessage::assistant_markdown("Pre-tool text".to_string()));
        app.add_message(ChatMessage::tool("🔧 tool1()".to_string()));
        app.add_message(ChatMessage::tool("result1".to_string()));
        app.add_message(ChatMessage::tool("🔧 tool2()".to_string()));
        app.add_message(ChatMessage::tool("result2".to_string()));

        // InterToolText arrives — insert before first trailing tool
        app.insert_before_streaming_zone(ChatMessage::assistant_markdown(
            "Between tools:".to_string(),
        ));

        assert_eq!(app.messages.len(), 7);
        // Order: User, Assistant "Pre-tool", Assistant "Between tools", Tool, Tool, Tool, Tool
        assert_eq!(app.messages[0].msg_type, MessageType::User);
        assert_eq!(app.messages[1].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[1].content, "Pre-tool text");
        assert_eq!(app.messages[2].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[2].content, "Between tools:");
        assert_eq!(app.messages[3].msg_type, MessageType::Tool);
    }

    #[test]
    fn test_insert_before_streaming_zone_mid_conversation() {
        // Multiple tool call rounds, then streaming response
        let mut app = test_app();

        // First tool round
        app.add_message(ChatMessage::user("Question".to_string()));
        app.add_message(ChatMessage::thinking("Thinking 1".to_string()));
        app.add_message(ChatMessage::tool("Tool result 1".to_string()));

        // Second streaming zone starts
        app.append_stream_thinking("More thinking");
        app.append_stream_token("Response");

        // Insert tool message before the second streaming zone
        app.insert_before_streaming_zone(ChatMessage::tool("Tool result 2".to_string()));

        // Tool result 2 should be between Tool result 1 and Thinking
        assert_eq!(app.messages.len(), 6);
        assert_eq!(app.messages[0].msg_type, MessageType::User);
        assert_eq!(app.messages[1].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[2].msg_type, MessageType::Tool);
        assert_eq!(app.messages[2].content, "Tool result 1");
        assert_eq!(app.messages[3].msg_type, MessageType::Tool);
        assert_eq!(app.messages[3].content, "Tool result 2");
        assert_eq!(app.messages[4].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[5].msg_type, MessageType::AssistantStreaming);
    }

    #[test]
    fn test_insert_before_streaming_zone_only_streaming() {
        // Only streaming messages, no stable messages before
        let mut app = test_app();

        app.append_stream_token("Streaming response");

        // Insert before the single streaming message
        app.insert_before_streaming_zone(ChatMessage::tool("Tool msg".to_string()));

        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].msg_type, MessageType::Tool);
        assert_eq!(app.messages[1].msg_type, MessageType::AssistantStreaming);
    }

    // ── insert_at_round_boundary tests ────────────────────────────

    #[test]
    fn test_insert_at_round_boundary_round0_no_rounds_yet() {
        // Inserting a round-0 message into empty messages — append
        let mut app = test_app();
        app.insert_at_round_boundary(
            ChatMessage::thinking("Think".to_string()).with_round_index(0),
        );
        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].round_index, 0);
    }

    #[test]
    fn test_insert_at_round_boundary_round1_after_round0() {
        // Round 0 messages exist. Insert round-1 content after them.
        let mut app = test_app();
        app.add_message(ChatMessage::user("Search for X".to_string()));
        app.add_message(
            ChatMessage::assistant_markdown("Let me search".to_string()).with_round_index(0),
        );
        app.add_message(ChatMessage::tool("🔧 search(X)".to_string()).with_round_index(0));
        app.add_message(ChatMessage::tool("Result: ...".to_string()).with_round_index(0));

        // InterToolText for round 1 inserts after round 0
        app.insert_at_round_boundary(
            ChatMessage::thinking("Now let me refine".to_string()).with_round_index(1),
        );
        assert_eq!(app.messages.len(), 5);
        // Round 1 thinking should be after all round 0 messages
        assert_eq!(app.messages[4].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[4].round_index, 1);
    }

    #[test]
    fn test_insert_at_round_boundary_round1_between_round0_and_streaming() {
        // Round 0 content finalized, then round 1 content inserted.
        // In production, streaming zone is finalized by ToolCallStarted before
        // InterToolText arrives, so there's no streaming zone when round boundary
        // insertion happens. We simulate this realistic scenario.
        let mut app = test_app();
        app.add_message(ChatMessage::user("Question".to_string()));
        app.add_message(
            ChatMessage::assistant_markdown("Pre-tool".to_string()).with_round_index(0),
        );
        app.add_message(ChatMessage::tool("🔧 tool1()".to_string()).with_round_index(0));

        // Round 0 streaming was finalized by ToolCallStarted, then thinking
        // arrived via InterToolText for round 1
        app.add_message(ChatMessage::thinking("More thinking".to_string()).with_round_index(0));
        app.add_message(
            ChatMessage::assistant_markdown("Streaming".to_string()).with_round_index(0),
        );
        app.finalize_streaming_zone_as_is(); // ToolCallStarted finalizes the zone

        // Insert round-1 inter-tool content after round-0 messages
        app.insert_at_round_boundary(
            ChatMessage::assistant_markdown("Between rounds".to_string()).with_round_index(1),
        );

        // Should be: User, Assistant(0), Tool(0), Thinking(0), Assistant(0), Assistant(1)
        assert_eq!(app.messages.len(), 6);
        assert_eq!(app.messages[5].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[5].content, "Between rounds");
        assert_eq!(app.messages[5].round_index, 1);
    }

    #[test]
    fn test_insert_at_round_boundary_round2_after_round1() {
        // Multi-round: round 0 and round 1 content exist, insert round 2.
        let mut app = test_app();
        app.add_message(ChatMessage::user("Question".to_string()));
        app.add_message(
            ChatMessage::assistant_markdown("Round 0 text".to_string()).with_round_index(0),
        );
        app.add_message(ChatMessage::tool("🔧 search()".to_string()).with_round_index(0));
        app.add_message(
            ChatMessage::assistant_markdown("Round 1 text".to_string()).with_round_index(1),
        );
        app.add_message(ChatMessage::tool("🔧 calc()".to_string()).with_round_index(1));

        // Insert round-2 inter-tool content after round 1
        app.insert_at_round_boundary(
            ChatMessage::thinking("Round 2 thinking".to_string()).with_round_index(2),
        );

        // Should be: User, Asst(0), Tool(0), Asst(1), Tool(1), Thinking(2)
        assert_eq!(app.messages.len(), 6);
        assert_eq!(app.messages[5].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[5].round_index, 2);
    }

    #[test]
    fn test_insert_at_round_boundary_all_same_round() {
        // All messages have round_index 0. Insert a round-0 message
        // should append after them (since round_index <= 0 matches all).
        let mut app = test_app();
        app.add_message(ChatMessage::user("Q".to_string()));
        app.add_message(ChatMessage::assistant_markdown("A".to_string()).with_round_index(0));

        app.insert_at_round_boundary(ChatMessage::tool("Result".to_string()).with_round_index(0));

        assert_eq!(app.messages.len(), 3);
        assert_eq!(app.messages[2].msg_type, MessageType::Tool);
        assert_eq!(app.messages[2].round_index, 0);
    }

    #[test]
    fn test_round_lifecycle_increment_and_reset() {
        // Test current_round, increment_round, reset_round lifecycle
        let mut app = test_app();
        assert_eq!(app.current_round(), 0);

        app.increment_round(); // ToolCallStarted: round 0 → round 1
        assert_eq!(app.current_round(), 1);

        app.increment_round(); // Second tool round: round 1 → round 2
        assert_eq!(app.current_round(), 2);

        app.reset_round(); // Complete: back to 0
        assert_eq!(app.current_round(), 0);
    }

    #[test]
    fn test_round_index_default_zero() {
        // ChatMessage constructors default round_index to 0
        assert_eq!(ChatMessage::user("hi".into()).round_index, 0);
        assert_eq!(
            ChatMessage::assistant_markdown("resp".into()).round_index,
            0
        );
        assert_eq!(ChatMessage::thinking("think".into()).round_index, 0);
        assert_eq!(ChatMessage::tool("tool".into()).round_index, 0);
        assert_eq!(ChatMessage::system("info".into()).round_index, 0);
        assert_eq!(ChatMessage::error("err".into()).round_index, 0);
        assert_eq!(ChatMessage::separator().round_index, 0);
    }

    #[test]
    fn test_with_round_index_builder() {
        // with_round_index sets the round_index
        let msg = ChatMessage::tool("🔧 search()".to_string()).with_round_index(3);
        assert_eq!(msg.round_index, 3);
        assert_eq!(msg.msg_type, MessageType::Tool);
        assert_eq!(msg.content, "🔧 search()");
    }

    #[test]
    fn test_insert_at_round_boundary_multiround_realistic() {
        // Simulate a realistic multi-round tool call cycle:
        // User → (think + stream) → [ToolCallStarted round 1]
        // → tool messages round 1 → [InterToolText round 2]
        // → tool messages round 2 → [StreamDone]
        let mut app = test_app();

        // Round 0: user prompt + streaming content
        app.add_message(ChatMessage::user(
            "Search for weather in São Paulo".to_string(),
        ));
        app.append_stream_thinking("I need to search");
        app.append_stream_token("Let me check the weather");

        // ToolCallStarted: finalize streaming, enter round 1
        app.finalize_streaming_zone_as_is();
        app.increment_round();
        assert_eq!(app.current_round(), 1);

        // Drain tool messages for round 1
        app.add_message(ChatMessage::tool("🔧 weather(São Paulo)".to_string()).with_round_index(1));
        app.add_message(ChatMessage::tool("Sunny, 28°C".to_string()).with_round_index(1));

        // InterToolText for round 2: model processes results and calls another tool
        app.increment_round(); // round 2
        assert_eq!(app.current_round(), 2);
        app.insert_at_round_boundary(
            ChatMessage::thinking("The weather is nice, let me suggest activities".to_string())
                .with_round_index(2),
        );
        app.insert_at_round_boundary(
            ChatMessage::assistant_markdown("Based on the weather".to_string()).with_round_index(2),
        );

        // Verify ordering: User(0), Thinking(streaming→stable), Asst(streaming→stable),
        // Tool(1), Tool(1), Thinking(2), Asst(2)
        assert_eq!(app.messages.len(), 7);
        assert_eq!(app.messages[0].msg_type, MessageType::User);
        // Messages 1-2 are the finalized streaming zone (thinking + asst from round 0)
        assert!(matches!(app.messages[1].msg_type, MessageType::Thinking));
        assert!(matches!(app.messages[2].msg_type, MessageType::Assistant));
        // Round 1 tool messages
        assert_eq!(app.messages[3].round_index, 1);
        assert_eq!(app.messages[4].round_index, 1);
        // Round 2 inter-tool text
        assert_eq!(app.messages[5].round_index, 2);
        assert_eq!(app.messages[5].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[6].round_index, 2);
        assert_eq!(app.messages[6].msg_type, MessageType::Assistant);

        // Reset at end of cycle
        app.reset_round();
        assert_eq!(app.current_round(), 0);
    }

    // ── Regression tests for round-aware streaming zone ──────────

    #[test]
    fn test_streaming_zone_excludes_round_gt0_thinking() {
        // Thinking blocks with round_index > 0 (from InterToolText) must
        // NOT be included in the streaming zone. Otherwise finalize_stream()
        // would consolidate inter-round thinking into a single block.
        let mut app = test_app();

        // Stable inter-round Thinking (round 2) — NOT streaming
        app.add_message(ChatMessage::thinking("Round 2 thinking".to_string()).with_round_index(2));
        // Streaming zone starts AFTER the stable Thinking block
        app.append_stream_thinking("Stream thinking");
        app.append_stream_token("Stream content");

        // Streaming zone should only include round-0 Thinking + AssistantStreaming
        let zone_start = app.streaming_zone_start();
        // The stable Thinking(round=2) should NOT be in the streaming zone
        assert_eq!(
            app.messages[zone_start].msg_type,
            MessageType::Thinking,
            "Streaming zone should start at the streaming Thinking block"
        );
        assert_eq!(
            app.messages[zone_start].round_index, 0,
            "Streaming zone Thinking should be round 0"
        );
    }

    #[test]
    fn test_append_stream_thinking_creates_new_block_after_stable_thinking() {
        // When a Thinking block with round_index > 0 is the last message,
        // append_stream_thinking should create a NEW block instead of appending
        // to the stable block.
        let mut app = test_app();

        // Add a stable inter-round Thinking block
        app.add_message(ChatMessage::thinking("Round 2 thinking".to_string()).with_round_index(2));

        // Now stream thinking tokens (round 0, final response)
        app.append_stream_thinking("Final ");

        // Should create a NEW Thinking block, not append to round 2 block
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].content, "Round 2 thinking");
        assert_eq!(app.messages[0].round_index, 2);
        assert_eq!(app.messages[1].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[1].content, "Final ");
        assert_eq!(app.messages[1].round_index, 0);
    }

    #[test]
    fn test_append_stream_token_creates_new_block_after_stable_thinking() {
        // When a Thinking block with round_index > 0 is the last message,
        // append_stream_token should create a NEW AssistantStreaming block
        // instead of trying to append to the Thinking block.
        let mut app = test_app();

        // Add a stable inter-round Thinking block
        app.add_message(ChatMessage::thinking("Round 2 thinking".to_string()).with_round_index(2));

        // Now stream content tokens (round 0, final response)
        app.append_stream_token("Final content");

        // Should create a NEW AssistantStreaming block
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].round_index, 2);
        assert_eq!(app.messages[1].msg_type, MessageType::AssistantStreaming);
        assert_eq!(app.messages[1].content, "Final content");
        assert_eq!(app.messages[1].round_index, 0);
    }

    #[test]
    fn test_finalize_stream_preserves_inter_round_thinking() {
        // Multi-round tool call cycle with inter-round Thinking blocks.
        // finalize_stream should NOT consolidate inter-round Thinking into
        // a single block — each round's Thinking should be preserved separately.
        let mut app = test_app();

        // Round 0 streaming (then finalized by ToolCallStarted)
        app.add_message(ChatMessage::user("Search for X".to_string()));
        app.append_stream_thinking("I need to search");
        app.append_stream_token("Let me check");
        app.finalize_streaming_zone_as_is();

        // Round 1 tool messages
        app.increment_round(); // round 1
        app.add_message(ChatMessage::tool("🔧 search(X)".to_string()).with_round_index(1));
        app.add_message(ChatMessage::tool("Result: ...".to_string()).with_round_index(1));

        // Round 2 inter-round content (InterToolText)
        app.increment_round(); // round 2
        app.insert_at_round_boundary(
            ChatMessage::thinking("The search result shows...".to_string()).with_round_index(2),
        );
        app.insert_at_round_boundary(
            ChatMessage::assistant_markdown("Based on the results".to_string()).with_round_index(2),
        );

        // Round 2 tool messages
        app.add_message(ChatMessage::tool("🔧 calc(42)".to_string()).with_round_index(2));
        app.add_message(ChatMessage::tool("= 42".to_string()).with_round_index(2));

        // Round 3: Final response streaming starts
        app.append_stream_thinking("Final thinking");
        app.append_stream_token("Final answer");

        // Verify inter-round Thinking is preserved BEFORE finalize
        let inter_round_thinking = app
            .messages
            .iter()
            .filter(|m| m.msg_type == MessageType::Thinking && m.round_index > 0)
            .count();
        assert!(
            inter_round_thinking > 0,
            "Should have inter-round Thinking blocks before finalize"
        );

        // Finalize the stream
        app.finalize_stream("Final answer", Some("Final thinking"));

        // Inter-round Thinking should STILL be preserved after finalize
        let inter_round_thinking_after = app
            .messages
            .iter()
            .filter(|m| m.msg_type == MessageType::Thinking && m.round_index > 0)
            .count();
        assert_eq!(
            inter_round_thinking, inter_round_thinking_after,
            "Inter-round Thinking blocks must be preserved by finalize_stream"
        );

        // Verify the round-2 Thinking content is intact and not merged
        let round2_thinking: Vec<&str> = app
            .messages
            .iter()
            .filter(|m| m.msg_type == MessageType::Thinking && m.round_index > 0)
            .map(|m| m.content.as_str())
            .collect();
        assert!(
            round2_thinking.iter().any(|c| c.contains("search result")),
            "Round 2 thinking content must be preserved: {:?}",
            round2_thinking
        );
    }

    #[test]
    fn test_finalize_stream_removes_round0_thinking_preserves_round_gt0() {
        // When finalize_stream is called with thinking: None, it removes
        // round-0 Thinking blocks but preserves round > 0 blocks.
        let mut app = test_app();

        // Inter-round Thinking (round 2) — must be preserved
        app.add_message(
            ChatMessage::thinking("Inter-round thinking".to_string()).with_round_index(2),
        );
        app.add_message(
            ChatMessage::assistant_markdown("Inter-round content".to_string()).with_round_index(2),
        );

        // Streaming Thinking (round 0) — must be REMOVED
        app.append_stream_thinking("Stream thinking");
        app.append_stream_token("Stream content");

        // Finalize with thinking: None — streaming Thinking should be removed
        app.finalize_stream("Final content", None);

        // Round-0 Thinking gone, round-2 Thinking preserved
        let thinking_count = app
            .messages
            .iter()
            .filter(|m| m.msg_type == MessageType::Thinking)
            .count();
        assert_eq!(thinking_count, 1, "Only inter-round Thinking should remain");

        let remaining_thinking = app
            .messages
            .iter()
            .find(|m| m.msg_type == MessageType::Thinking)
            .unwrap();
        assert_eq!(remaining_thinking.round_index, 2);
        assert_eq!(remaining_thinking.content, "Inter-round thinking");
    }

    #[test]
    fn test_streaming_zone_with_interleaved_stable_and_streaming_thinking() {
        // After InterToolText inserts Thinking(round=2), then streaming
        // starts, the streaming zone should start AFTER the stable block.
        let mut app = test_app();

        // Stable inter-round Thinking and content
        app.add_message(ChatMessage::user("Question".to_string()));
        app.add_message(
            ChatMessage::assistant_markdown("Let me check".to_string()).with_round_index(0),
        );
        app.add_message(ChatMessage::tool("🔧 search()".to_string()).with_round_index(0));
        app.add_message(
            ChatMessage::thinking("Inter-round thinking".to_string()).with_round_index(2),
        );
        app.add_message(
            ChatMessage::assistant_markdown("More content".to_string()).with_round_index(2),
        );

        // Streaming starts — should go AFTER stable content
        app.append_stream_thinking("Final thinking");
        app.append_stream_token("Final answer");

        // Verify message order: User, Assistant(0), Tool(0), Thinking(2), Assistant(2), Thinking(0), AssistantStreaming(0)
        assert_eq!(app.messages.len(), 7);
        assert_eq!(app.messages[0].msg_type, MessageType::User);
        assert_eq!(app.messages[1].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[1].round_index, 0);
        assert_eq!(app.messages[2].msg_type, MessageType::Tool);
        assert_eq!(app.messages[3].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[3].round_index, 2);
        assert_eq!(app.messages[4].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[4].round_index, 2);
        //Streaming zone starts here
        assert_eq!(app.messages[5].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[5].round_index, 0);
        assert_eq!(app.messages[6].msg_type, MessageType::AssistantStreaming);
        assert_eq!(app.messages[6].round_index, 0);
    }

    // ── has_streaming_zone tests ────────────────────────────────────

    #[test]
    fn test_has_streaming_zone_true_thinking() {
        let mut app = test_app();
        app.append_stream_thinking("Thinking");
        assert!(app.has_streaming_zone());
    }

    #[test]
    fn test_has_streaming_zone_false_stable_thinking_only() {
        // Inter-round Thinking (round > 0) should NOT count as streaming zone
        let mut app = test_app();
        app.add_message(
            ChatMessage::thinking("Inter-round thinking".to_string()).with_round_index(2),
        );
        assert!(
            !app.has_streaming_zone(),
            "Stable Thinking (round > 0) should not be part of streaming zone"
        );
    }

    #[test]
    fn test_has_streaming_zone_true_streaming() {
        let mut app = test_app();
        app.append_stream_token("Content");
        assert!(app.has_streaming_zone());
    }

    #[test]
    fn test_has_streaming_zone_true_interleaved() {
        let mut app = test_app();
        app.add_message(ChatMessage::user("Q".to_string()));
        app.append_stream_thinking("Thinking");
        app.append_stream_token("Content");
        assert!(app.has_streaming_zone());
    }

    #[test]
    fn test_has_streaming_zone_false() {
        let mut app = test_app();
        app.add_message(ChatMessage::user("Q".to_string()));
        app.add_message(ChatMessage::assistant_markdown("A".to_string()));
        assert!(!app.has_streaming_zone());
    }

    #[test]
    fn test_content_block_lifecycle_pre_and_post_tool() {
        // Realistic simulation of the ToolCallStarted -> StreamDone
        // flow that preserves pre-tool content across tool execution.
        // ToolCallStarted calls finalize_streaming_zone_as_is() which
        // converts AssistantStreaming to stable Assistant_markdown.
        // StreamDone then finalizes the post-tool block only.
        let mut app = test_app();

        // Pre-tool streaming
        app.append_stream_thinking("I should calculate");
        app.append_stream_token("Let me compute ");

        // ToolCallStarted arrives: finalize_streaming_zone_as_is
        // converts the zone into stable messages.
        app.finalize_streaming_zone_as_is();
        app.block_finalized = true;
        app.set_llm_state(LlmState::ToolCall);

        // With no streaming zone, tool messages append normally
        app.add_message(ChatMessage::tool("🔧 calc".to_string()));
        app.add_message(ChatMessage::tool("42".to_string()));

        // Post-tool streaming: new tokens create a fresh AssistantStreaming
        // at the tail (a new content block).
        app.append_stream_token("22 + 20 = 42");

        // StreamDone arrives: finalize only the new zone (post-tool block).
        app.finalize_stream("22 + 20 = 42", None);

        // Result: pre-tool preserved, tools inserted between, post-tool finalized
        assert_eq!(app.messages.len(), 5);
        // Thinking was appended first, then AssistantStreaming:
        // after finalize_streaming_zone_as_is, Thinking remains first
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].content, "I should calculate");
        assert_eq!(app.messages[1].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[1].content, "Let me compute ");
        assert_eq!(app.messages[2].msg_type, MessageType::Tool);
        assert_eq!(app.messages[3].msg_type, MessageType::Tool);
        assert_eq!(app.messages[4].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[4].content, "22 + 20 = 42");
    }

    #[test]
    fn test_content_block_lifecycle_multiple_tool_calls() {
        // Full flow with two tool calls.
        // ToolCallStarted finalizes pre-tool block (finalize_streaming_zone_as_is).
        // Inter-tool text arrives via InterToolText (no streaming zone).
        // StreamDone finalizes the final (post-tool) block.
        let mut app = test_app();

        // -- Block 0: pre-tool streaming --
        app.append_stream_thinking("Vou buscar info");
        app.append_stream_token("Vou buscar ");

        // ToolCallStarted: finalize pre-tool block (converts AssistantStreaming)
        app.finalize_streaming_zone_as_is();
        app.block_finalized = true;
        app.set_llm_state(LlmState::ToolCall);

        // Tool 1 messages appended normally (no streaming zone)
        app.add_message(ChatMessage::tool("🔧 weather()".to_string()));
        app.add_message(ChatMessage::tool("Sunny, 22°C".to_string()));

        // -- Block 1: between-tools text arrives via InterToolText --
        // InterToolText adds assistant_markdown directly (no streaming)
        app.add_message(ChatMessage::assistant_markdown(
            "Agora calcular: ".to_string(),
        ));
        app.set_llm_state(LlmState::ToolCall);

        // Tool 2 messages
        app.add_message(ChatMessage::tool("🔧 calc".to_string()));
        app.add_message(ChatMessage::tool("42".to_string()));

        // -- Block 2: post-tool final content (StreamDone) --
        // StreamDone calls finalize_stream which, finding no AssistantStreaming,
        // adds a new Assistant message.
        app.finalize_stream("Pronto!", None);

        // Result: three preserved blocks separated by tools
        assert_eq!(app.messages.len(), 8);
        // Thinking first (was appended first during streaming), then Assistant
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].content, "Vou buscar info");
        assert_eq!(app.messages[1].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[1].content, "Vou buscar ");
        assert_eq!(app.messages[2].msg_type, MessageType::Tool);
        assert_eq!(app.messages[3].msg_type, MessageType::Tool);
        assert_eq!(app.messages[4].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[4].content, "Agora calcular: ");
        assert_eq!(app.messages[5].msg_type, MessageType::Tool);
        assert_eq!(app.messages[6].msg_type, MessageType::Tool);
        assert_eq!(app.messages[7].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[7].content, "Pronto!");
    }

    #[test]
    fn test_has_streaming_zone_false_tool_calls() {
        // Tool messages are not part of the streaming zone
        let mut app = test_app();
        app.add_message(ChatMessage::tool("Tool result".to_string()));
        assert!(!app.has_streaming_zone());
    }

    // ── Content Block Stateful Streaming lifecycle tests ────────────

    #[test]
    fn test_content_block_lifecycle_single_block() {
        // A simple turn with no tool calls — standard streaming path
        let mut app = test_app();
        assert!(!app.block_finalized);

        app.append_stream_token("Hello");
        app.append_stream_token(" world");

        app.finalize_stream("Hello world", None);

        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[0].content, "Hello world");
        // block_finalized not touched by finalize_stream
        assert!(!app.block_finalized);
    }

    #[test]
    fn test_content_block_set_llm_state_clears_block_finalized() {
        let mut app = test_app();

        app.set_llm_state(LlmState::ToolCall);
        assert!(!app.block_finalized);

        app.block_finalized = true;

        app.set_llm_state(LlmState::Idle);
        assert!(!app.block_finalized);
    }

    #[test]
    fn test_finalize_zone_as_is_preserves_streamed_content() {
        // When ToolCallStarted arrives before StreamBlockDone/StreamDone,
        // finalize_streaming_zone_as_is() should convert AssistantStreaming
        // to stable Assistant WITHOUT replacing content.
        let mut app = test_app();

        app.append_stream_thinking("Analyzing");
        app.append_stream_token("Result: 42");

        // Tool calls interrupt: finalize as-is before tool messages
        app.finalize_streaming_zone_as_is();

        // Content is preserved as-is, no authoritative replacement
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[0].content, "Analyzing");
        assert_eq!(app.messages[1].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[1].content, "Result: 42");
        // No streaming zone after finalization
        assert!(!app.has_streaming_zone());
    }

    #[test]
    fn test_tool_messages_after_finalize_zone_inserted_correctly() {
        // The real fix: tool messages must appear AFTER pre-tool content,
        // never before. finalize_streaming_zone_as_is() ensures this.
        let mut app = test_app();

        // User message + pre-tool streaming
        app.add_message(ChatMessage::user("Ola".to_string()));
        app.append_stream_thinking("Hmm");
        app.append_stream_token("Boa noite");

        // Tool calls interrupt — MUST finalize before inserting tools
        app.finalize_streaming_zone_as_is();
        app.set_llm_state(LlmState::ToolCall);

        // Tool messages now append normally (no zone = append at end)
        app.add_message(ChatMessage::tool("file_read".to_string()));
        app.add_message(ChatMessage::tool("content".to_string()));

        // Visual order: User → Thinking → Assistant → Tool
        assert_eq!(app.messages.len(), 5);
        assert_eq!(app.messages[0].msg_type, MessageType::User);
        assert_eq!(app.messages[1].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[1].content, "Hmm");
        assert_eq!(app.messages[2].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[2].content, "Boa noite");
        assert_eq!(app.messages[3].msg_type, MessageType::Tool);
        assert_eq!(app.messages[4].msg_type, MessageType::Tool);
    }

    // ── ScrollState tests ──────────────────────────────────────────────

    #[test]
    fn test_scroll_state_default() {
        let state = ScrollState::default();
        assert!(state.auto_scroll, "Default should be auto-scroll");
        assert_eq!(state.manual_offset, 0, "Default offset should be 0");
    }

    #[test]
    fn test_scroll_up_increments_offset() {
        let mut state = ScrollState::new();
        state.scroll_up(5);
        assert!(!state.auto_scroll, "Scrolling up disables auto-scroll");
        assert_eq!(state.manual_offset, 5);
        state.scroll_up(3);
        assert_eq!(state.manual_offset, 8, "Scroll up accumulates offset");
    }

    #[test]
    fn test_scroll_down_decrements_offset() {
        let mut state = ScrollState::new();
        state.scroll_up(10);
        assert_eq!(state.manual_offset, 10);
        state.scroll_down(3);
        assert_eq!(state.manual_offset, 7);
        state.scroll_down(7);
        assert_eq!(state.manual_offset, 0, "Scroll down clamps at 0");
        assert!(
            state.auto_scroll,
            "Reaching offset 0 re-enables auto-scroll"
        );
    }

    #[test]
    fn test_scroll_down_does_not_re_enable_auto_scroll_before_bottom() {
        let mut state = ScrollState::new();
        state.scroll_up(10);
        state.scroll_down(3);
        assert_eq!(
            state.manual_offset, 7,
            "Offset decreases but doesn't reach 0"
        );
        assert!(
            !state.auto_scroll,
            "Auto-scroll should NOT re-enable until offset reaches 0"
        );
    }

    #[test]
    fn test_clamp_offset_reduces_overscroll() {
        let mut state = ScrollState::new();
        // Simulate rapid mouse wheel scrolling: 100 lines of offset
        // but content only allows 50 lines of scrolling
        state.scroll_up(100);
        assert_eq!(state.manual_offset, 100);

        // Content: 200 lines total, viewport: 150 lines → max_scroll = 50
        state.clamp_offset(200, 150);
        assert_eq!(
            state.manual_offset, 50,
            "Clamp should reduce overscroll to max_scroll"
        );
        assert!(
            !state.auto_scroll,
            "Auto-scroll should remain disabled after clamping"
        );
    }

    #[test]
    fn test_clamp_offset_no_change_when_in_range() {
        let mut state = ScrollState::new();
        state.scroll_up(30);
        // Content: 200 lines, viewport: 150 → max_scroll = 50
        state.clamp_offset(200, 150);
        assert_eq!(
            state.manual_offset, 30,
            "Clamp should not change offset when within range"
        );
    }

    #[test]
    fn test_clamp_offset_resets_when_content_fits_viewport() {
        let mut state = ScrollState::new();
        state.scroll_up(50);
        assert!(!state.auto_scroll);

        // Content: 100 lines, viewport: 120 → content fits, no scroll needed
        state.clamp_offset(100, 120);
        assert_eq!(state.manual_offset, 0, "Offset should reset to 0");
        assert!(
            state.auto_scroll,
            "Auto-scroll should re-enable when content fits viewport"
        );
    }

    #[test]
    fn test_effective_scroll_from_top_auto_scroll() {
        let state = ScrollState::new();
        // 200 lines, 150 visible → max_scroll = 50
        let result = state.effective_scroll_from_top(200, 150);
        assert_eq!(result, 50, "Auto-scroll should show bottom of content");
    }

    #[test]
    fn test_effective_scroll_from_top_manual_scroll() {
        let mut state = ScrollState::new();
        state.scroll_up(30);
        // 200 lines, 150 visible → max_scroll = 50
        // from_top = max_scroll - manual_offset = 50 - 30 = 20
        let result = state.effective_scroll_from_top(200, 150);
        assert_eq!(result, 20, "Manual scroll should offset from bottom");
    }

    #[test]
    fn test_scroll_overscroll_bug_scenario() {
        // Reproduces the bug: rapid scroll up causes manual_offset to
        // accumulate beyond max_scroll, making scroll_down feel sluggish.
        // With clamp_offset called during render, the offset is clamped.
        let mut state = ScrollState::new();

        // User scrolls up rapidly (e.g., 30 mouse wheel events × 3 = 90)
        state.scroll_up(90);
        assert_eq!(state.manual_offset, 90);

        // Content: 200 lines, viewport: 150 → max_scroll = 50
        // clamp_offset reduces 90 → 50
        state.clamp_offset(200, 150);
        assert_eq!(state.manual_offset, 50, "Overscroll clamped to max_scroll");

        // Now scroll_down works immediately (3 lines at a time)
        state.scroll_down(3);
        assert_eq!(state.manual_offset, 47, "Scroll down responds immediately");
    }

    // ── poll_embedding_progress tests ──────────────────────────────────

    #[test]
    fn test_poll_embedding_progress_receives_progress() {
        let (mut app, tx, _async_tx) = App::with_embedding_channel(MarkdownTheme::Dark, vec![]);

        let _ = tx.send(EmbeddingProgress::new(
            EmbeddingPhase::Content,
            3,
            10,
            5,
            10,
        ));
        app.poll_embedding_progress();

        let state = app.status_bar();
        assert_eq!(
            state.embedding_progress,
            Some(EmbeddingProgress::new(
                EmbeddingPhase::Content,
                3,
                10,
                5,
                10
            )),
            "Should show progress 3/10 entities, 5/10 embeddings"
        );
    }

    #[test]
    fn test_poll_embedding_progress_completion_clears_indicator() {
        let (mut app, tx, _async_tx) = App::with_embedding_channel(MarkdownTheme::Dark, vec![]);

        let _ = tx.send(EmbeddingProgress::completed());
        app.poll_embedding_progress();

        let state = app.status_bar();
        assert_eq!(
            state.embedding_progress, None,
            "Completion should clear indicator"
        );
    }

    #[test]
    fn test_poll_embedding_progress_drains_multiple() {
        let (mut app, tx, _async_tx) = App::with_embedding_channel(MarkdownTheme::Dark, vec![]);

        let _ = tx.send(EmbeddingProgress::new(
            EmbeddingPhase::Content,
            1,
            10,
            1,
            10,
        ));
        let _ = tx.send(EmbeddingProgress::new(
            EmbeddingPhase::Content,
            5,
            10,
            5,
            10,
        ));
        let _ = tx.send(EmbeddingProgress::new(
            EmbeddingPhase::Content,
            8,
            10,
            8,
            10,
        ));
        app.poll_embedding_progress();

        let state = app.status_bar();
        assert_eq!(
            state.embedding_progress,
            Some(EmbeddingProgress::new(
                EmbeddingPhase::Content,
                8,
                10,
                8,
                10
            )),
            "Should keep latest progress update"
        );
    }

    #[test]
    fn test_poll_embedding_progress_no_messages_keeps_state() {
        let (mut app, _tx, _async_tx) = App::with_embedding_channel(MarkdownTheme::Dark, vec![]);

        // No messages sent — state should remain default (None)
        app.poll_embedding_progress();

        let state = app.status_bar();
        assert_eq!(
            state.embedding_progress, None,
            "No messages should keep embedding_progress as None"
        );
    }

    #[test]
    fn test_toggle_style_flips_state() {
        let (mut app, _tx, _async_tx) = App::with_embedding_channel(MarkdownTheme::Dark, vec![]);

        // Default: style enabled
        assert!(app.style_enabled(), "Style should be enabled by default");
        assert!(
            app.status_bar().style_enabled,
            "Status bar should reflect style enabled"
        );

        // Toggle off
        app.toggle_style();
        assert!(
            !app.style_enabled(),
            "Style should be disabled after toggle"
        );
        assert!(
            !app.status_bar().style_enabled,
            "Status bar should reflect style disabled"
        );

        // Toggle back on
        app.toggle_style();
        assert!(
            app.style_enabled(),
            "Style should be enabled after second toggle"
        );
        assert!(
            app.status_bar().style_enabled,
            "Status bar should reflect style enabled again"
        );
    }

    // ── LlmState::Compacting tests ─────────────────────────────

    #[test]
    fn test_llm_state_compacting_disables_input() {
        let mut app = test_app();
        app.set_llm_state(LlmState::Compacting);
        assert_eq!(app.llm_state(), LlmState::Compacting);
        // Input should be disabled during compaction
        assert!(
            app.disabled_reason.is_some(),
            "Input should be disabled during compaction"
        );
        assert!(
            app.disabled_reason.as_ref().unwrap().contains("Compacting"),
            "Disabled reason should mention 'Compacting', got: {:?}",
            app.disabled_reason
        );
    }

    #[test]
    fn test_llm_state_compacting_spinner_label() {
        let mut app = test_app();
        app.set_llm_state(LlmState::Compacting);
        // Status bar should show "Compacting..."
        assert_eq!(
            app.status_bar.status_label,
            Some("Compacting...".to_string()),
            "Status label should be 'Compacting...'"
        );
    }

    #[test]
    fn test_random_tui_spinner_frames_returns_non_empty() {
        // The spinner seed is time-based, so we can't assert determinism
        // without refactoring. This test verifies the function returns
        // a valid (non-empty) spinner frame list and doesn't panic.
        let frames = super::random_tui_spinner_frames();
        assert!(!frames.is_empty(), "Spinner frames must not be empty");
        assert!(
            frames.iter().all(|f| !f.is_empty()),
            "Each frame must be a non-empty string"
        );
    }
}
