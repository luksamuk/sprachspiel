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

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui_textarea::TextArea;

use super::completer::ChatCompleter;
use super::input::{CrosstermInput, InputBackend, InputResult};
use super::tui::TuiTerminal;
use super::tui::components::chat_area::ChatMessage;
use super::tui::components::chat_area::MessageType;
use super::tui::components::chat_selection::ChatSelection;
use super::tui::components::completion_menu::CompletionMenuState;
use super::tui::components::status_bar::StatusBarState;
use super::tui::markdown::MarkdownTheme;

/// Processing state of the LLM
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmState {
    /// Idle — waiting for user input
    Idle,
    /// Thinking — spinner active, input disabled
    Thinking,
    /// Streaming — response coming in, input disabled
    Streaming,
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
    /// Scroll state for the chat area
    scroll: ScrollState,
    /// Spinner animation frames (random rattles preset)
    spinner_frames: Vec<&'static str>,
    /// Current spinner frame index
    spinner_frame: usize,
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
    /// Create a new App with default state
    pub fn new(theme: MarkdownTheme, model_names: Vec<String>) -> Self {
        let completer = ChatCompleter::new(model_names.clone());

        // Create textarea with custom styling: no line numbers, no cursor line highlight
        let mut textarea = TextArea::default();
        textarea.set_line_number_style(ratatui::style::Style::default());
        textarea.set_cursor_line_style(ratatui::style::Style::default());
        textarea.set_tab_length(4);

        Self {
            messages: Vec::new(),
            textarea,
            input_disabled: false,
            disabled_reason: None,
            history_input: CrosstermInput::new(model_names),
            completer,
            completion_menu: CompletionMenuState::new(),
            chat_selection: ChatSelection::new(),
            visual_lines_cache: Vec::new(),
            scroll_from_top_cache: 0,
            chat_area_rect_cache: ratatui::layout::Rect::default(),
            status_bar: StatusBarState::new(String::new(), 0, 0, 0, false, false),
            llm_state: LlmState::Idle,
            theme,
            scroll: ScrollState::new(),
            spinner_frames: random_tui_spinner_frames(),
            spinner_frame: 0,
        }
    }

    /// Add a message to the chat area and auto-scroll to bottom
    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.scroll.reset_to_bottom();
    }

    /// Append a streaming token to the last `AssistantStreaming` message.
    ///
    /// If the last message is not `AssistantStreaming`, creates a new one.
    /// This enables incremental display of LLM responses token by token.
    pub fn append_stream_token(&mut self, token: &str) {
        if let Some(last) = self.messages.last_mut()
            && last.msg_type == MessageType::AssistantStreaming
        {
            last.content.push_str(token);
            return;
        }
        // No streaming message yet — create one
        self.messages
            .push(ChatMessage::assistant_streaming(token.to_string()));
        self.scroll.reset_to_bottom();
    }

    /// Append a streaming thinking token to the last `Thinking` message.
    ///
    /// If the last message is not `Thinking`, creates a new one.
    /// This enables incremental display of thinking content during streaming.
    pub fn append_stream_thinking(&mut self, token: &str) {
        if let Some(last) = self.messages.last_mut()
            && last.msg_type == MessageType::Thinking
        {
            last.content.push_str(token);
            return;
        }
        // No thinking message yet — create one
        self.messages.push(ChatMessage::thinking(token.to_string()));
        self.scroll.reset_to_bottom();
    }

    /// Replace the last `AssistantStreaming` message with the final
    /// markdown-rendered `Assistant` message.
    ///
    /// Also replaces the last `Thinking` message if the thinking content
    /// differs from what was streamed (e.g., additional formatting).
    /// Sets LLM state back to Idle.
    pub fn finalize_stream(&mut self, content: &str, _thinking: Option<&str>) {
        // Find and replace the last AssistantStreaming message
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

    /// Get the messages in the chat area
    #[allow(dead_code)] // PR3: Will be used for scroll/pagination features
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Get a reference to the textarea
    #[allow(dead_code)] // Public API for external state queries
    pub fn textarea(&self) -> &TextArea<'static> {
        &self.textarea
    }

    /// Get a mutable reference to the textarea
    #[allow(dead_code)] // Public API for external state mutation
    pub fn textarea_mut(&mut self) -> &mut TextArea<'static> {
        &mut self.textarea
    }

    /// Whether input is disabled (during LLM processing)
    #[allow(dead_code)] // Public API for external state queries
    pub fn input_disabled(&self) -> bool {
        self.input_disabled
    }

    /// Get the disabled reason text
    #[allow(dead_code)] // Public API for external state queries
    pub fn disabled_reason(&self) -> Option<&str> {
        self.disabled_reason.as_deref()
    }

    /// Get a reference to the completion menu state
    #[allow(dead_code)] // Public API for external state queries
    pub fn completion_menu(&self) -> &CompletionMenuState {
        &self.completion_menu
    }

    /// Get a mutable reference to the completion menu state
    #[allow(dead_code)] // Public API for external state mutation
    pub fn completion_menu_mut(&mut self) -> &mut CompletionMenuState {
        &mut self.completion_menu
    }

    /// Get the current LLM state
    #[allow(dead_code)] // PR3: Will be used for state-dependent UI rendering
    pub fn llm_state(&self) -> LlmState {
        self.llm_state
    }

    /// Get the current scroll state
    #[allow(dead_code)] // Public API for external scroll queries
    pub fn scroll_state(&self) -> &ScrollState {
        &self.scroll
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

    /// Get the cached visual lines (for selection text extraction)
    #[allow(dead_code)] // Used internally by Ctrl+Shift+C copy handler
    pub fn visual_lines_cache(&self) -> &[String] {
        &self.visual_lines_cache
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

    /// Scroll to the bottom of the chat (newest messages).
    ///
    /// Called on terminal resize to ensure newest content stays visible.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll.scroll_to_bottom();
    }

    /// Set the LLM state and update input/status accordingly
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
    /// Most keys are passed to `textarea.input()` which handles them natively:
    /// - Shift+arrows/Home/End for text selection
    /// - Ctrl+arrows for word movement
    /// - Insert char replaces selected text
    /// - Backspace/Delete with selection deletes it
    /// - Ctrl+A/E, Home/End for line navigation
    /// - Ctrl+W for cut, Ctrl+Y for paste, Ctrl+U/R for undo/redo
    ///
    /// We only intercept keys that need custom behavior:
    /// - Enter: submit (textarea default is newline)
    /// - Shift+Enter: newline (our convention; textarea default is also newline)
    /// - Ctrl+C: clear input or cancel LLM
    /// - Ctrl+D: EOF on empty, forward delete otherwise
    /// - Ctrl+Shift+C/V: clipboard copy/paste
    /// - Up/Down without shift: history navigation (single-line) or cursor (multi-line)
    /// - PageUp/PageDown/Home/End without Ctrl/Alt: chat scroll
    /// - Tab: completion
    ///
    /// When input is disabled (during LLM processing), only Ctrl+C
    /// and scroll keys are processed — all other keys are ignored.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Option<InputResult> {
        // ── Clipboard shortcuts (always work) ────────────────────────

        // Ctrl+Shift+C — copy chat selection or textarea selection to clipboard
        if matches!(
            key,
            crossterm::event::KeyEvent {
                code: KeyCode::Char('C'),
                modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT,
                ..
            }
        ) {
            if self.chat_selection.is_active() {
                let text = self.chat_selection.extract_text(&self.visual_lines_cache);
                if !text.is_empty() {
                    let _ = cli_clipboard::set_contents(text);
                }
                self.chat_selection.clear();
            } else if self.textarea.is_selecting() {
                self.textarea.copy();
                if let Some(text) = self.yank_text()
                    && !text.is_empty()
                {
                    let _ = cli_clipboard::set_contents(text);
                }
            }
            return None;
        }

        // Ctrl+Shift+V — paste from system clipboard
        if matches!(
            key,
            crossterm::event::KeyEvent {
                code: KeyCode::Char('V'),
                modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT,
                ..
            }
        ) {
            if let Ok(text) = cli_clipboard::get_contents()
                && !text.is_empty()
            {
                self.textarea.insert_str(&text);
                self.chat_selection.clear();
            }
            return None;
        }

        // ── Ctrl+C handling (always works) ───────────────────────────

        if matches!(
            key,
            crossterm::event::KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
        ) {
            let has_text = !self.textarea_is_empty();
            if has_text {
                // Clear input: select all, cut (copies to kill-ring), clear
                self.textarea.select_all();
                self.textarea.cut();
                // Try system clipboard (best-effort)
                if let Some(text) = self.yank_text()
                    && !text.is_empty()
                {
                    let _ = cli_clipboard::set_contents(text);
                }
                self.textarea.delete_char(); // ensure textarea is empty
                return None;
            }
            // No text: cancel LLM or no-op
            return Some(InputResult::Interrupted);
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
                // Tab — confirm selection
                crossterm::event::KeyEvent {
                    code: KeyCode::Tab,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    if let Some(item) = self.completion_menu.confirm() {
                        // For slash commands, the replacement is the full trigger + space
                        let replacement = format!("{} ", item);
                        self.set_textarea_content(&replacement);
                        // After completing a command with args, try sub-completion
                        self.try_completion_after_confirm();
                    }
                    return None;
                }

                // Enter — confirm selection and submit (if no sub-completions)
                crossterm::event::KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                    ..
                } => {
                    if let Some(item) = self.completion_menu.confirm() {
                        let replacement = format!("{} ", item);
                        self.set_textarea_content(&replacement);
                        // Try sub-completion; if none available, no-op
                        self.try_completion_after_confirm();
                    }
                    return None;
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
            // Enter — submit the line (textarea default is newline, we override)
            crossterm::event::KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                let line = self.textarea_lines();
                self.textarea_clear();
                self.chat_selection.clear();
                if !line.is_empty() {
                    self.history_input.add_history(&line);
                }
                self.scroll.reset_to_bottom();
                Some(InputResult::Line(line))
            }

            // Shift+Enter — newline (explicit, same as textarea default)
            crossterm::event::KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                self.textarea.insert_newline();
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
                    self.textarea.input(key);
                    None
                }
            }

            // Up (no shift) — history nav or textarea cursor up
            crossterm::event::KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                if self.textarea_is_multiline() {
                    self.textarea.input(key);
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
                    self.textarea.input(key);
                } else {
                    self.history_next();
                }
                None
            }

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

            // ── All other keys: pass to textarea.input() ─────────────
            // This handles: Shift+arrows (selection), Ctrl+arrows (word movement),
            // Home/End (with shift for selection), Backspace/Delete (with selection),
            // Ctrl+A/E, Ctrl+W (cut), Ctrl+Y (paste), Ctrl+U/R (undo/redo),
            // Alt+Backspace (delete word), regular chars (replace selection),
            // and any other key the textarea knows about.
            _ => {
                self.textarea.input(key);
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
                ..
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
    /// command takes arguments and if so, automatically triggers completion
    /// for those arguments. This enables recursive completion:
    /// `/mo` → Tab → `/model ` → model name list appears.
    fn try_completion_after_confirm(&mut self) {
        let buffer = self.textarea_lines();
        let cursor_pos = self.cursor_byte_offset();

        if cursor_pos == buffer.len() && buffer.starts_with('/') {
            let result = self.completer.complete(&buffer, cursor_pos);
            match result {
                super::completer::CompletionResult::None => {
                    self.completion_menu.hide();
                }
                super::completer::CompletionResult::Single {
                    replacement,
                    cursor_pos,
                } => {
                    self.completion_menu.hide();
                    self.set_textarea_content(&replacement);
                    self.set_cursor_to_byte_offset(cursor_pos);
                }
                super::completer::CompletionResult::Multiple {
                    matches,
                    descriptions,
                    ..
                } => {
                    let common = common_prefix_str(&matches);
                    self.completion_menu.show(matches, descriptions, common);
                }
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

    /// Update model names in the completer (e.g., after a model switch).
    ///
    /// Called when the model list changes to keep tab completion current.
    #[allow(dead_code)] // Will be used after model switch in event loop
    pub fn update_model_names(&mut self, model_names: Vec<String>) {
        self.completer.set_model_names(model_names);
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

            // Input line height adapts to multi-line content.
            // Cap at 10 lines to prevent the input from consuming the entire screen.
            let input_height = self.textarea.lines().len().min(10) as u16;

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
                &self.scroll,
                self.theme,
                &self.chat_selection,
            );

            // Cache visual lines, scroll offset, and chat area rect for
            // mouse/selection integration (updated every render cycle)
            self.visual_lines_cache = meta.visual_lines;
            self.scroll_from_top_cache = meta.scroll_from_top;
            self.chat_area_rect_cache = chunks[0];

            // Render status bar
            super::tui::components::status_bar::render(f, chunks[1], &self.status_bar);

            // Render input line
            super::tui::components::input_line::render(
                f,
                chunks[2],
                &self.textarea,
                self.input_disabled,
                self.disabled_reason.as_deref(),
            );

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
