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
use super::tui::components::chat_selection::ChatSelection;
use super::tui::components::completion_menu::CompletionMenuState;
use super::tui::components::status_bar::StatusBarState;
use super::tui::live_turn::LiveTurn;
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
    /// Chat messages displayed in the chat area (committed history).
    ///
    /// During an LLM turn, `messages` is immutable. All volatile content
    /// (streaming text, thinking, tool-call previews, tool results) lives in
    /// `live_turn` and is merged into `messages` when the turn completes.
    messages: Vec<ChatMessage>,
    /// Volatile state of the in-flight LLM turn.
    ///
    /// `None` when no LLM turn is active. Created on the first streaming token
    /// and committed/dropped when the turn ends.
    live_turn: Option<LiveTurn>,
    /// Current round index in a multi-round LLM tool-call cycle.
    ///
    /// Kept in sync with `LiveTurn::round_index` while a live turn is active.
    /// Used by legacy callers that query the round before a live turn exists.
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
            live_turn: None,
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

    // ── Live turn API ───────────────────────────────────────────────────

    /// Get a reference to the current live turn, if any.
    #[cfg(test)]
    pub fn live_turn(&self) -> Option<&LiveTurn> {
        self.live_turn.as_ref()
    }

    /// Get a mutable reference to the current live turn, if any.
    pub fn live_turn_mut(&mut self) -> Option<&mut LiveTurn> {
        self.live_turn.as_mut()
    }

    /// Finalize the current live turn and commit its blocks to the message history.
    ///
    /// Returns the committed messages. If no live turn exists, returns an empty vec.
    #[cfg(test)]
    pub fn commit_live_turn(&mut self) -> Vec<ChatMessage> {
        if let Some(turn) = self.live_turn.take() {
            let committed = turn.finalize();
            for msg in &committed {
                self.messages.push(msg.clone());
            }
            self.scroll.reset_to_bottom();
            return committed;
        }
        Vec::new()
    }

    /// Cancel the current live turn without committing anything.
    #[cfg(test)]
    pub fn cancel_live_turn(&mut self) {
        self.live_turn = None;
    }

    /// Render the committed history plus any live-turn blocks for display.
    ///
    /// This is what `chat_area::render()` consumes.
    pub fn render_messages(&self) -> Vec<ChatMessage> {
        let mut rendered = Vec::with_capacity(
            self.messages.len() + self.live_turn.as_ref().map_or(0, |t| t.blocks.len()),
        );
        rendered.extend(self.messages.iter().cloned());
        if let Some(turn) = self.live_turn.as_ref() {
            rendered.extend(turn.render_blocks());
        }
        rendered
    }

    /// Update or insert a tool-call preview.
    ///
    /// Creates the live turn if it does not exist. In the two-buffer model,
    /// previews live in `LiveTurn::tool_previews` keyed by `tool_call_id`.
    ///
    /// Test-only: production code uses [`upsert_tool_preview_direct`] which
    /// avoids the string format → parse roundtrip.
    #[cfg(test)]
    pub fn upsert_tool_preview(&mut self, tool_call_id: String, content: String) {
        let (name, args) = Self::parse_tool_preview_content(&content, &tool_call_id);
        let turn = self
            .live_turn
            .get_or_insert_with(|| LiveTurn::new(self.current_round));
        turn.upsert_tool_preview(tool_call_id, name, args);
    }

    /// Update or insert a tool-call preview with explicit name and args.
    ///
    /// More efficient than [`upsert_tool_preview`] because it avoids the
    /// string format → parse roundtrip. Used by the streaming
    /// `ToolCallPreview` handler which already has the name and args.
    pub fn upsert_tool_preview_direct(
        &mut self,
        tool_call_id: String,
        name: String,
        args: serde_json::Value,
    ) {
        let turn = self
            .live_turn
            .get_or_insert_with(|| LiveTurn::new(self.current_round));
        turn.upsert_tool_preview(tool_call_id, name, args);
    }

    /// Freeze every tool-call preview message into a finalized tool message.
    ///
    /// Called when tool calls are fully collected (on `ToolCallStarted` or
    /// when a round ends) so the transient preview becomes a stable entry.
    pub fn freeze_all_tool_previews(&mut self) {
        if let Some(turn) = self.live_turn.as_mut() {
            turn.freeze_all_tool_previews();
        }
    }

    /// Attach the result of a tool call to its matching live-turn block.
    ///
    /// Called when `ToolExecutionFinished` arrives from the coordinator.
    /// If the live turn does not exist or does not contain a matching block,
    /// a warning is logged and the result is dropped.
    ///
    /// Test-only: production code calls `LiveTurn::set_tool_result` directly
    /// via `view.app_mut().live_turn_mut()`.
    #[cfg(test)]
    pub fn set_tool_result(&mut self, tool_call_id: &str, content: String, is_error: bool) {
        if let Some(turn) = self.live_turn.as_mut() {
            turn.set_tool_result(tool_call_id, content, is_error);
        } else {
            log::warn!(
                "set_tool_result called with no active live turn for id {}",
                tool_call_id
            );
        }
    }

    /// Best-effort parse of the formatted preview content used by the old API.
    #[cfg(test)]
    fn parse_tool_preview_content(
        content: &str,
        tool_call_id: &str,
    ) -> (String, serde_json::Value) {
        // Try to extract the name from "🔧 name(`id`)" or "🔧 name(args)".
        let without_emoji = content.trim_start_matches("🔧 ").trim_start();
        let name = without_emoji
            .split('(')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();

        // If there's a JSON code block, parse it; otherwise try to parse the
        // argument string after the first '('.
        if let Some(start) = content.find("```json\n")
            && let Some(end) = content.rfind("\n```")
        {
            let json_str = &content[start + 8..end];
            if let Ok(value) = serde_json::from_str(json_str) {
                return (name, value);
            }
        }

        // Fallback: extract the argument portion between name( and )
        if let Some(start) = content.find('(') {
            let arg_start = start + 1;
            let arg_end = content.rfind(')').unwrap_or(content.len());
            let args_str = &content[arg_start..arg_end];
            let args_str = args_str
                .trim()
                .trim_start_matches('`')
                .trim_end_matches('`');
            if let Ok(value) = serde_json::from_str(args_str) {
                return (name, value);
            }
            if args_str == tool_call_id {
                return (name, serde_json::Value::Object(serde_json::Map::new()));
            }
        }

        (name, serde_json::Value::Object(serde_json::Map::new()))
    }

    /// Return the number of messages in the chat area.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Append a streaming token to the last `AssistantStreaming` message.
    ///
    /// If the last message is not `AssistantStreaming`, searches backward
    /// within the streaming zone (contiguous tail of Thinking/AssistantStreaming
    /// messages) for an existing `AssistantStreaming` block. If found, appends
    /// to it. Otherwise creates a new one.
    ///
    /// **Round-awareness:** Streaming tokens are always round 0 (final response).
    /// If the last `AssistantStreaming` block has `round_index > 0` (a stable
    /// inter-round block from a previous turn), streaming tokens must NOT append
    /// to it — they belong to a different round. Instead, a new `AssistantStreaming`
    /// block is created.
    /// Append a streaming token to the current live turn.
    pub fn append_stream_token(&mut self, token: &str) {
        if token.is_empty() {
            return;
        }
        let turn = self
            .live_turn
            .get_or_insert_with(|| LiveTurn::new(self.current_round));
        turn.start_streaming();
        turn.append_text(token);
        self.scroll.reset_to_bottom();
    }

    /// Append a streaming thinking token to the current live turn.
    pub fn append_stream_thinking(&mut self, token: &str) {
        if token.is_empty() {
            return;
        }
        let turn = self
            .live_turn
            .get_or_insert_with(|| LiveTurn::new(self.current_round));
        turn.append_thinking(token);
        self.scroll.reset_to_bottom();
    }

    /// Get the current round index.
    pub fn current_round(&self) -> usize {
        self.live_turn
            .as_ref()
            .map_or(self.current_round, |t| t.round_index)
    }

    /// Increment the current round index.
    pub fn increment_round(&mut self) {
        self.current_round += 1;
        if let Some(turn) = self.live_turn.as_mut() {
            turn.round_index = self.current_round;
        }
    }

    /// Reset the current round index to 0.
    pub fn reset_round(&mut self) {
        self.current_round = 0;
        if let Some(turn) = self.live_turn.as_mut() {
            turn.round_index = 0;
        }
    }

    /// Finalize the current streaming zone by converting all streaming
    /// content blocks in the live turn to stable, non-streaming blocks.
    ///
    /// Called when streaming is interrupted by tool calls (e.g., on
    /// `ToolCallStarted`). In the two-buffer model this simply finalizes the
    /// last streaming block in the live turn; the content is committed only when
    /// the full turn ends.
    pub fn finalize_streaming_zone_as_is(&mut self) {
        if let Some(turn) = self.live_turn.as_mut() {
            turn.finalize_last_block();
            log::debug!(
                "finalize_streaming_zone_as_is: blocks={}, previews={}",
                turn.blocks.len(),
                turn.tool_previews.len(),
            );
        } else {
            log::debug!("finalize_streaming_zone_as_is: no active live turn");
        }
    }

    /// Check whether the streaming zone is non-empty.
    ///
    /// In the two-buffer model this is true whenever a live turn exists and
    /// has content.
    pub fn has_streaming_zone(&self) -> bool {
        self.live_turn.as_ref().is_some_and(|t| !t.is_empty())
    }

    /// Finalize the current live turn and commit it to the message history.
    ///
    /// In the two-buffer model, all streaming content lives in `LiveTurn`.
    /// This method consumes the live turn, converts its blocks into stable
    /// `ChatMessage`s, and appends them to `messages`. The optional `thinking`
    /// and `content` parameters override the accumulated blocks: if provided,
    /// they replace the respective streaming blocks with authoritative final
    /// content from the provider.
    pub fn finalize_stream(&mut self, content: &str, thinking: Option<&str>) {
        let thinking_desc = thinking.as_ref().map_or_else(
            || "None".to_string(),
            |t| format!("Some({} chars)", t.len()),
        );
        log::debug!(
            "finalize_stream: messages_len={}, content_len={}, thinking={}",
            self.messages.len(),
            content.len(),
            thinking_desc,
        );

        if let Some(mut turn) = self.live_turn.take() {
            turn.start_finalizing();
            turn.finalize_last_block();

            // Override accumulated thinking with authoritative content.
            // BUG-2 fix: previously this used `retain` to remove ALL Thinking
            // blocks, destroying earlier-round thinking. The intermediate fix
            // removed only the last Thinking but re-inserted at index 0, which
            // REVERSED the order (Thinking2 appeared before Thinking1).
            //
            // Correct fix: replace the LAST Thinking block's content IN-PLACE,
            // preserving its position in the block order. This keeps the
            // chronological order: Thinking1 → ToolCall → Thinking2(now
            // authoritative) → Response.
            if let Some(thinking_content) = thinking {
                if let Some(last_thinking_idx) = turn
                    .blocks
                    .iter()
                    .rposition(|b| matches!(b, super::tui::live_turn::TurnBlock::Thinking { .. }))
                {
                    // Replace in-place — maintains scroll position and order
                    if let super::tui::live_turn::TurnBlock::Thinking {
                        content,
                        is_streaming,
                    } = &mut turn.blocks[last_thinking_idx]
                    {
                        *content = thinking_content.to_string();
                        *is_streaming = false;
                    }
                } else if !thinking_content.is_empty() {
                    // No existing Thinking block — insert at the beginning
                    turn.blocks.insert(
                        0,
                        super::tui::live_turn::TurnBlock::Thinking {
                            content: thinking_content.to_string(),
                            is_streaming: false,
                        },
                    );
                }
            }

            // Override accumulated text with authoritative content.
            // Bug C fix: previously this used `retain` to remove ALL Text
            // blocks and replaced them with a single Text block containing
            // only `post_tool_content`. This destroyed pre-tool text from
            // earlier ReAct rounds that was already frozen and displayed.
            // Now we only remove the LAST Text block (the one being finalized
            // with authoritative content) and preserve earlier Text blocks.
            if let Some(last_text_idx) = turn
                .blocks
                .iter()
                .rposition(|b| matches!(b, super::tui::live_turn::TurnBlock::Text { .. }))
            {
                turn.blocks.remove(last_text_idx);
            }
            if !content.is_empty() {
                turn.blocks.push(super::tui::live_turn::TurnBlock::Text {
                    content: content.to_string(),
                    is_streaming: false,
                });
            }

            let committed = turn.finalize();
            for msg in committed {
                self.messages.push(msg);
            }
        } else {
            // No live turn — legacy fallback: just add the final assistant message.
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
    pub fn set_llm_state(&mut self, state: LlmState) {
        self.llm_state = state;
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

    /// Set a transient status-bar message (right-aligned, red).
    ///
    /// Used for provider retry warnings. Pass `None` to clear.
    pub fn set_status_overlay(&mut self, overlay: Option<String>) {
        self.status_bar.overlay = overlay;
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
            let render_messages = self.render_messages();
            let meta = super::tui::components::chat_area::render(
                f,
                chunks[0],
                &render_messages,
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
    use crate::chat::tui::components::chat_area::MessageType;
    use crate::chat::tui::live_turn::{TurnBlock, TurnState};

    /// Create a minimal App for testing streaming message operations.
    fn test_app() -> App {
        let (app, _embedding_tx, _async_message_tx) =
            App::with_embedding_channel(MarkdownTheme::Dark, vec!["test-model".to_string()]);
        app
    }

    // ── Live turn integration tests (Two-Buffer model) ─────────────────

    #[test]
    fn test_live_turn_created_on_first_stream_token() {
        let mut app = test_app();
        assert!(app.live_turn().is_none());
        app.append_stream_token("Hello");
        assert!(app.live_turn().is_some());
        assert_eq!(app.live_turn().unwrap().state, TurnState::Streaming);
    }

    #[test]
    fn test_render_messages_combines_committed_and_live() {
        let mut app = test_app();
        app.add_message(ChatMessage::user("Q".to_string()));
        app.append_stream_thinking("Thinking");
        app.append_stream_token("Answer");

        let rendered = app.render_messages();
        assert_eq!(rendered.len(), 3);
        assert_eq!(rendered[0].msg_type, MessageType::User);
        assert_eq!(rendered[1].msg_type, MessageType::Thinking);
        assert_eq!(rendered[2].msg_type, MessageType::AssistantStreaming);

        // Committed history is unchanged
        assert_eq!(app.messages.len(), 1);
    }

    #[test]
    fn test_commit_live_turn_moves_blocks_to_history() {
        let mut app = test_app();
        app.add_message(ChatMessage::user("Q".to_string()));
        app.append_stream_thinking("Thinking");
        app.append_stream_token("Answer");

        let committed = app.commit_live_turn();
        assert_eq!(committed.len(), 2);
        assert_eq!(app.messages.len(), 3);
        assert_eq!(app.messages[1].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[2].msg_type, MessageType::Assistant);
        assert!(app.live_turn().is_none());
    }

    #[test]
    fn test_cancel_live_turn_drops_volatile_content() {
        let mut app = test_app();
        app.add_message(ChatMessage::user("Q".to_string()));
        app.append_stream_token("Partial");
        app.cancel_live_turn();

        assert!(app.live_turn().is_none());
        assert_eq!(app.messages.len(), 1);
        let rendered = app.render_messages();
        assert_eq!(rendered.len(), 1);
    }

    #[test]
    fn test_finalize_stream_authoritative_overrides() {
        let mut app = test_app();
        app.append_stream_thinking("Streamed thinking");
        app.append_stream_token("Streamed answer");

        app.finalize_stream("Final answer", Some("Final thinking"));

        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages[0].content, "Final thinking");
        assert_eq!(app.messages[1].content, "Final answer");
    }

    #[test]
    fn test_multi_round_tool_call_ordering_with_live_turn() {
        // Two-buffer model: the live turn accumulates all rounds and is
        // committed once at the end. Tool messages added via add_message
        // sit in committed history while the live turn is open.
        let mut app = test_app();

        // Round 0: pre-tool streaming
        app.add_message(ChatMessage::user("Weather?".to_string()));
        app.append_stream_thinking("Need weather");
        app.append_stream_token("Searching...");
        app.finalize_streaming_zone_as_is();
        app.increment_round(); // round 1

        // Round 1 tool call + result (simulated via add_message for now)
        app.add_message(ChatMessage::tool("🔧 search(weather)".to_string()).with_round_index(1));
        app.add_message(ChatMessage::tool("Sunny".to_string()).with_round_index(1));

        // Round 1 post-tool streaming
        app.append_stream_thinking("Got weather");
        app.append_stream_token("Based on...");
        app.finalize_streaming_zone_as_is();
        app.increment_round(); // round 2

        // Round 2 tool call + result
        app.add_message(ChatMessage::tool("🔧 calc(temp)".to_string()).with_round_index(2));
        app.add_message(ChatMessage::tool("= 28".to_string()).with_round_index(2));

        // Final response commits the whole live turn
        app.finalize_stream("Final answer", Some("Final thinking"));

        // Bug C fix + BUG-2 fix: Text AND Thinking blocks from earlier
        // rounds are now PRESERVED with correct ordering. The last Thinking
        // block ("Got weather") is replaced IN-PLACE by "Final thinking"
        // (not removed and re-inserted at index 0, which reversed the order).
        // The last Text block ("Based on...") is removed and replaced by
        // "Final answer", but "Searching..." (from round 0) is preserved.
        // Messages: User, Tool, Tool, Tool, Tool,
        //   Thinking("Need weather"), Text("Searching..."),
        //   Thinking("Final thinking"), Text("Final answer") = 9
        assert_eq!(app.messages.len(), 9);
        assert_eq!(app.messages[0].msg_type, MessageType::User);
        assert_eq!(app.messages[1].msg_type, MessageType::Tool);
        assert_eq!(app.messages[2].msg_type, MessageType::Tool);
        assert_eq!(app.messages[3].msg_type, MessageType::Tool);
        assert_eq!(app.messages[4].msg_type, MessageType::Tool);
        // "Need weather" from round 0 is preserved (BUG-2 fix) — comes first
        assert_eq!(app.messages[5].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[5].content, "Need weather");
        // "Searching..." text from round 0 is preserved (Bug C fix)
        assert_eq!(app.messages[6].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[6].content, "Searching...");
        // "Final thinking" replaces "Got weather" IN-PLACE (not at index 0)
        assert_eq!(app.messages[7].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[7].content, "Final thinking");
        assert_eq!(app.messages[8].msg_type, MessageType::Assistant);
        assert_eq!(app.messages[8].content, "Final answer");
    }

    #[test]
    fn test_round_index_synced_between_app_and_live_turn() {
        let mut app = test_app();
        assert_eq!(app.current_round(), 0);

        app.append_stream_token("start");
        app.increment_round();
        assert_eq!(app.current_round(), 1);
        assert_eq!(app.live_turn().unwrap().round_index, 1);

        app.increment_round();
        assert_eq!(app.current_round(), 2);
        assert_eq!(app.live_turn().unwrap().round_index, 2);

        app.reset_round();
        assert_eq!(app.current_round(), 0);
        assert_eq!(app.live_turn().unwrap().round_index, 0);
    }

    #[test]
    fn test_tool_preview_in_live_turn() {
        let mut app = test_app();
        app.upsert_tool_preview(
            "call_1".to_string(),
            "🔧 weather(`call_1`)\n```json\n{\"city\": \"São Paulo\"}\n```".to_string(),
        );

        let rendered = app.render_messages();
        assert_eq!(rendered.len(), 1);
        assert_eq!(rendered[0].msg_type, MessageType::Tool);
        assert!(rendered[0].is_streaming);
        assert_eq!(rendered[0].tool_call_id.as_deref(), Some("call_1"));
    }

    #[test]
    fn test_freeze_all_tool_previews_promotes_to_blocks() {
        let mut app = test_app();
        app.upsert_tool_preview("a".to_string(), "🔧 search(`a`)".to_string());
        app.upsert_tool_preview("b".to_string(), "🔧 calc(`b`)".to_string());
        app.freeze_all_tool_previews();

        let rendered = app.render_messages();
        assert_eq!(rendered.len(), 2);
        assert!(!rendered[0].is_streaming);
        assert!(!rendered[1].is_streaming);
    }

    #[test]
    fn test_tool_result_attached_to_live_turn_block() {
        let mut app = test_app();
        app.add_message(ChatMessage::user("Q".to_string()));
        app.upsert_tool_preview(
            "call_1".to_string(),
            "🔧 weather(`call_1`)\n```json\n{\"city\": \"São Paulo\"}\n```".to_string(),
        );
        app.freeze_all_tool_previews();
        app.set_tool_result("call_1", "sunny".to_string(), false);

        // The result is stored in the live-turn block for the LLM.
        let turn = app.live_turn.as_ref().expect("live turn should exist");
        let block = &turn.blocks[0];
        assert!(
            matches!(block, TurnBlock::ToolCall { tool_call_id, result: Some(r), .. } if tool_call_id == "call_1" && r.content == "sunny")
        );

        // The rendered chat message only shows the compact tool line; results
        // are suppressed from the TUI chat area.
        let rendered = app.render_messages();
        assert_eq!(rendered.len(), 2);
        assert_eq!(rendered[1].msg_type, MessageType::Tool);
        assert_eq!(rendered[1].tool_call_id.as_deref(), Some("call_1"));
        assert!(
            !rendered[1].content.contains("sunny"),
            "Tool result must not appear in rendered chat content"
        );
        assert!(rendered[1].content.contains("weather"));
        // tool_call_id is NOT shown in normal (non-trace) mode
        assert!(
            !rendered[1].content.contains("call_1"),
            "Tool call id should not appear in normal mode"
        );
    }

    #[test]
    fn test_multi_round_tool_results_stay_ordered() {
        let mut app = test_app();
        app.add_message(ChatMessage::user("Q".to_string()));

        app.upsert_tool_preview("a".to_string(), "🔧 search(`a`)".to_string());
        app.freeze_all_tool_previews();
        app.increment_round();
        app.set_tool_result("a", "result-a".to_string(), false);

        // Result-a is stored in the live-turn block for the LLM before it is
        // suppressed from the TUI display.
        {
            let turn = app.live_turn.as_ref().expect("live turn should exist");
            assert!(
                matches!(
                    &turn.blocks[0],
                    TurnBlock::ToolCall { tool_call_id, result: Some(r), .. }
                    if tool_call_id == "a" && r.content == "result-a"
                ),
                "result-a should be attached to tool-call block a"
            );
        }

        app.append_stream_thinking("Hmm");
        app.append_stream_token("Based on...");
        app.finalize_streaming_zone_as_is();

        app.upsert_tool_preview("b".to_string(), "🔧 calc(`b`)".to_string());
        app.freeze_all_tool_previews();
        app.increment_round();
        app.set_tool_result("b", "result-b".to_string(), false);

        {
            let turn = app
                .live_turn
                .as_ref()
                .expect("live turn should still exist");
            let b_block = turn.blocks.iter().rev().find_map(|b| match b {
                TurnBlock::ToolCall {
                    tool_call_id,
                    result,
                    ..
                } if tool_call_id == "b" => result.as_ref(),
                _ => None,
            });
            assert_eq!(
                b_block.map(|r| r.content.as_str()),
                Some("result-b"),
                "result-b should be attached to tool-call block b"
            );
        }

        app.finalize_stream("Final answer", None);

        // The committed order preserves the live turn's block order:
        // user, first tool call, inter-round thinking, second tool call,
        // final assistant text.
        assert_eq!(app.messages.len(), 5);
        assert_eq!(app.messages[0].msg_type, MessageType::User);
        assert_eq!(app.messages[1].msg_type, MessageType::Tool);
        assert_eq!(app.messages[2].msg_type, MessageType::Thinking);
        assert_eq!(app.messages[3].msg_type, MessageType::Tool);
        assert_eq!(app.messages[4].msg_type, MessageType::Assistant);

        // Tool results are suppressed from the rendered chat messages.
        assert!(
            !app.messages[1].content.contains("result-a"),
            "Tool result must not appear in rendered chat content: {}",
            app.messages[1].content
        );
        assert!(
            !app.messages[3].content.contains("result-b"),
            "Tool result must not appear in rendered chat content: {}",
            app.messages[3].content
        );
    }

    #[test]
    fn test_has_streaming_zone_with_live_turn() {
        let mut app = test_app();
        assert!(!app.has_streaming_zone());
        app.append_stream_token("x");
        assert!(app.has_streaming_zone());
        app.finalize_stream("x", None);
        assert!(!app.has_streaming_zone());
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
