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
//!     ├─ InputState (buffer, cursor, disabled state)
//!     ├─ ChatMessage[] (chat area content)
//!     ├─ StatusBarState (model, tokens, spinner)
//!     ├─ ScrollState (auto-scroll and manual offset)
//!     └─ LlmState (idle, thinking, streaming, tool_call)
//! ```

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Layout};

use super::completer::ChatCompleter;
use super::input::{CrosstermInput, InputBackend, InputResult};
use super::tui::TuiTerminal;
use super::tui::components::chat_area::ChatMessage;
use super::tui::components::input_line::InputState;
use super::tui::components::status_bar::StatusBarState;
use super::tui::markdown::MarkdownTheme;

/// Processing state of the LLM
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmState {
    /// Idle — waiting for user input
    Idle,
    /// Thinking — spinner active, input disabled
    #[allow(dead_code)] // PR3: Will be used for non-blocking spinner animation
    Thinking,
    /// Streaming — response coming in, input disabled
    Streaming,
    /// Running a tool call
    #[allow(dead_code)] // PR3: Will be used when tool call UI shows spinner
    ToolCall,
}

/// Spinner frames for animation (braille dots pattern)
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

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
    /// Current input buffer and cursor state
    input_state: InputState,
    /// CrosstermInput for history management
    history_input: CrosstermInput,
    /// Tab completion engine (slash commands + model names)
    completer: ChatCompleter,
    /// Status bar state
    status_bar: StatusBarState,
    /// LLM processing state
    llm_state: LlmState,
    /// Markdown rendering theme
    theme: MarkdownTheme,
    /// Scroll state for the chat area
    scroll: ScrollState,
    /// Current spinner frame index
    spinner_frame: usize,
}

impl App {
    /// Create a new App with default state
    pub fn new(theme: MarkdownTheme, model_names: Vec<String>) -> Self {
        let completer = ChatCompleter::new(model_names.clone());
        Self {
            messages: Vec::new(),
            input_state: InputState::new(),
            history_input: CrosstermInput::new(model_names),
            completer,
            status_bar: StatusBarState::new(String::new(), 0, 0, 0, false, false),
            llm_state: LlmState::Idle,
            theme,
            scroll: ScrollState::new(),
            spinner_frame: 0,
        }
    }

    /// Add a message to the chat area and auto-scroll to bottom
    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.scroll.reset_to_bottom();
    }

    /// Get the messages in the chat area
    #[allow(dead_code)] // PR3: Will be used for scroll/pagination features
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Get the current input state
    #[allow(dead_code)] // Public API — will be used by streaming display in Phase 3.2
    pub fn input_state(&self) -> &InputState {
        &self.input_state
    }

    /// Get a mutable reference to the input state
    #[allow(dead_code)] // Public API — will be used by streaming display in Phase 3.2
    pub fn input_state_mut(&mut self) -> &mut InputState {
        &mut self.input_state
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
                self.input_state.set_disabled(false, None);
                self.status_bar.spinner = None;
                self.status_bar.status_label = None;
            }
            LlmState::Thinking => {
                self.input_state
                    .set_disabled(true, Some("Thinking...".to_string()));
                self.status_bar.spinner = Some("⠋".to_string());
                self.status_bar.status_label = Some("Thinking...".to_string());
            }
            LlmState::Streaming => {
                self.input_state
                    .set_disabled(true, Some("Streaming...".to_string()));
                // During streaming, show model name (no spinner)
                self.status_bar.spinner = None;
                self.status_bar.status_label = None;
            }
            LlmState::ToolCall => {
                self.input_state
                    .set_disabled(true, Some("Running tool...".to_string()));
                self.status_bar.spinner = Some("⠋".to_string());
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

    /// Advance the spinner frame
    pub fn tick_spinner(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
        if self.llm_state != LlmState::Idle && self.llm_state != LlmState::Streaming {
            self.status_bar.spinner = Some(SPINNER_FRAMES[self.spinner_frame].to_string());
        }
    }

    /// Process a crossterm key event
    ///
    /// Returns `Some(InputResult::Line(line))` when Enter is pressed,
    /// `Some(InputResult::Interrupted)` for Ctrl+C,
    /// `Some(InputResult::Eof)` for Ctrl+D on empty line,
    /// and `None` for other key events (buffer updated internally).
    ///
    /// When input is disabled (during LLM processing), only Ctrl+C
    /// is processed — all other keys are ignored.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Option<InputResult> {
        // Ctrl+C always works, even when input is disabled
        if matches!(
            key,
            crossterm::event::KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
        ) {
            return Some(InputResult::Interrupted);
        }

        // When input is disabled (LLM processing), still allow scroll keys
        if self.input_state.disabled {
            // Allow scroll keys even when input is disabled
            match key.code {
                KeyCode::PageUp => {
                    self.scroll.scroll_up(10);
                    return None;
                }
                KeyCode::PageDown => {
                    self.scroll.scroll_down(10);
                    return None;
                }
                KeyCode::Home => {
                    self.scroll.scroll_to_top();
                    return None;
                }
                KeyCode::End => {
                    self.scroll.scroll_to_bottom();
                    return None;
                }
                _ => {
                    // Ignore all other keys when input is disabled
                    return None;
                }
            }
        }

        match key {
            // Enter — submit the line
            crossterm::event::KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                let line = self.input_state.buffer.clone();
                self.input_state.clear();
                if !line.is_empty() {
                    self.history_input.add_history(&line);
                }
                // Auto-scroll to bottom when submitting
                self.scroll.reset_to_bottom();
                Some(InputResult::Line(line))
            }

            // Ctrl+D — EOF on empty line, forward delete on non-empty
            crossterm::event::KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.input_state.buffer.is_empty() {
                    Some(InputResult::Eof)
                } else {
                    // Forward delete: remove character at cursor
                    self.input_state.delete_char_right();
                    None
                }
            }

            // Backspace — delete char before cursor
            crossterm::event::KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => {
                self.input_state.backspace();
                None
            }

            // Left arrow — move cursor left
            crossterm::event::KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.input_state.cursor_left();
                None
            }

            // Right arrow — move cursor right
            crossterm::event::KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.input_state.cursor_right();
                None
            }

            // Up arrow — history previous
            crossterm::event::KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.history_prev();
                None
            }

            // Down arrow — history next
            crossterm::event::KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.history_next();
                None
            }

            // PageUp — scroll chat up (older messages)
            crossterm::event::KeyEvent {
                code: KeyCode::PageUp,
                ..
            } => {
                self.scroll.scroll_up(10);
                None
            }

            // PageDown — scroll chat down (newer messages)
            crossterm::event::KeyEvent {
                code: KeyCode::PageDown,
                ..
            } => {
                self.scroll.scroll_down(10);
                None
            }

            // Home — scroll to top (oldest messages)
            crossterm::event::KeyEvent {
                code: KeyCode::Home,
                ..
            } => {
                self.scroll.scroll_to_top();
                None
            }

            // End — scroll to bottom (newest messages)
            crossterm::event::KeyEvent {
                code: KeyCode::End, ..
            } => {
                self.scroll.scroll_to_bottom();
                None
            }

            // Tab — attempt tab completion (slash commands, model names)
            crossterm::event::KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.try_tab_complete();
                None
            }

            // Regular character — insert at cursor
            crossterm::event::KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                ..
            } => {
                self.input_state.insert_char(c);
                None
            }

            // Ignore other key events
            _ => None,
        }
    }

    /// Navigate to previous history entry
    ///
    /// This duplicates the logic in `CrosstermInput::history_prev()` because
    /// `CrosstermInput` maintains its own buffer/cursor state (used by the
    /// `InputBackend` trait) while `App` uses `InputState` for TUI rendering.
    /// PR3 should unify these into a single input state owner.
    fn history_prev(&mut self) {
        // Save current buffer before starting navigation
        if self.history_input.history.is_empty() {
            return;
        }

        // If not navigating, save current buffer
        if self.history_input.history_pos.is_none() {
            self.history_input.saved_buffer = self.input_state.buffer.clone();
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
            self.input_state.buffer = self.history_input.history[pos].clone();
            self.input_state.cursor_pos = self.input_state.buffer.len();
        }
    }

    /// Navigate to next history entry
    ///
    /// See `history_prev()` for the dual-state documentation.
    fn history_next(&mut self) {
        match self.history_input.history_pos {
            None => {}
            Some(pos) => {
                if pos + 1 >= self.history_input.history.len() {
                    // Past the newest entry: restore saved buffer
                    self.history_input.history_pos = None;
                    self.input_state.buffer = self.history_input.saved_buffer.clone();
                    self.input_state.cursor_pos = self.input_state.buffer.len();
                } else {
                    self.history_input.history_pos = Some(pos + 1);
                    self.input_state.buffer = self.history_input.history[pos + 1].clone();
                    self.input_state.cursor_pos = self.input_state.buffer.len();
                }
            }
        }
    }

    /// Attempt tab completion based on current input buffer.
    ///
    /// Uses `ChatCompleter` to find slash command or model name completions.
    /// On single match: replaces the buffer with the completed text.
    /// On multiple matches: cycles through them on repeated Tab presses.
    /// When no matches: does nothing (bell could be added later).
    fn try_tab_complete(&mut self) {
        use super::completer::CompletionResult;

        let buffer = self.input_state.buffer.clone();
        let cursor_pos = self.input_state.cursor_pos;

        let result = self.completer.complete(&buffer, cursor_pos);

        match result {
            CompletionResult::None => {
                // No completion available — could ring terminal bell here
            }
            CompletionResult::Single {
                replacement,
                cursor_pos,
            } => {
                self.input_state.buffer = replacement;
                self.input_state.cursor_pos = cursor_pos;
            }
            CompletionResult::Multiple {
                matches,
                cycle_index,
            } => {
                // Use the current cycle match as the completion
                if let Some(selected) = matches.get(cycle_index) {
                    let replacement = format!("{} ", selected);
                    self.input_state.buffer = replacement.clone();
                    self.input_state.cursor_pos = replacement.len();
                }
            }
        }
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
        &self,
        terminal: &mut TuiTerminal,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        terminal.draw(|f| {
            let size = f.area();

            // Layout: chat area (flexible) | status bar (2 lines) | input line (1 line)
            let chunks = Layout::vertical([
                Constraint::Min(3),    // Chat area gets all remaining space
                Constraint::Length(2), // Status bar (separator + content)
                Constraint::Length(1), // Input line
            ])
            .split(size);

            // Render chat area
            super::tui::components::chat_area::render(
                f,
                chunks[0],
                &self.messages,
                &self.scroll,
                self.theme,
            );

            // Render status bar
            super::tui::components::status_bar::render(f, chunks[1], &self.status_bar);

            // Render input line
            super::tui::components::input_line::render(f, chunks[2], &self.input_state);
        })?;

        Ok(())
    }
}
