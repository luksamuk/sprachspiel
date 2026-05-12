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
//!     └─ LlmState (idle, thinking, streaming, tool_call)
//! ```

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::Terminal;
use ratatui::layout::{Constraint, Layout};

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
    Thinking,
    /// Streaming — response coming in, input disabled
    Streaming,
    /// Running a tool call
    ToolCall,
}

/// Spinner frames for animation (braille dots pattern)
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// The main application state for the TUI chat REPL
pub struct App {
    /// Chat messages displayed in the chat area
    messages: Vec<ChatMessage>,
    /// Current input buffer and cursor state
    input_state: InputState,
    /// CrosstermInput for history management
    history_input: CrosstermInput,
    /// Status bar state
    status_bar: StatusBarState,
    /// LLM processing state
    llm_state: LlmState,
    /// Markdown rendering theme
    theme: MarkdownTheme,
    /// Scroll offset for the chat area (0 = scrolled to bottom)
    scroll_offset: u16,
    /// Whether the app should quit
    should_quit: bool,
    /// Current spinner frame index
    spinner_frame: usize,
}

impl App {
    /// Create a new App with default state
    pub fn new(theme: MarkdownTheme, model_names: Vec<String>) -> Self {
        Self {
            messages: Vec::new(),
            input_state: InputState::new(),
            history_input: CrosstermInput::new(model_names),
            status_bar: StatusBarState::new(String::new(), 0, 0, 0, false, false),
            llm_state: LlmState::Idle,
            theme,
            scroll_offset: 0,
            should_quit: false,
            spinner_frame: 0,
        }
    }

    /// Add a message to the chat area and auto-scroll to bottom
    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.scroll_offset = 0;
    }

    /// Get the messages in the chat area
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Get the current input state
    pub fn input_state(&self) -> &InputState {
        &self.input_state
    }

    /// Get a mutable reference to the input state
    pub fn input_state_mut(&mut self) -> &mut InputState {
        &mut self.input_state
    }

    /// Get the current LLM state
    pub fn llm_state(&self) -> LlmState {
        self.llm_state
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

    /// Check if the app should quit
    pub fn should_quit(&self) -> bool {
        self.should_quit
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

        // When input is disabled, ignore all other keys
        if self.input_state.disabled {
            return None;
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
                Some(InputResult::Line(line))
            }

            // Ctrl+D — EOF on empty line, delete on non-empty
            crossterm::event::KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.input_state.buffer.is_empty() {
                    Some(InputResult::Eof)
                } else {
                    // Delete character at cursor (like forward delete)
                    // InputState doesn't have delete_char_right yet,
                    // but backspace works as a fallback
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
    fn history_next(&mut self) {
        match self.history_input.history_pos {
            None => return,
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
                self.scroll_offset,
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
