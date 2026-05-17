//! Ratatui view implementation for the TUI chat REPL
//!
//! This module provides `RatatuiView`, which implements the `ChatView` trait
//! using ratatui widgets instead of terminal println/eprintln. It owns an
//! `App` instance and delegates all state mutations to it, then triggers
//! a re-render.
//!
//! # Architecture
//!
//! ```text
//! run_chat_repl()
//!     ↓ creates
//! RatatuiView (owns App, owns Terminal)
//!     ↓ implements
//! ChatView trait (18 methods)
//!     ↓ delegates to
//! App::add_message(), App::set_llm_state(), etc.
//!     ↓ renders via
//! App::render(&mut terminal)
//! ```
//!
//! Non-chat subcommands (query, translate, OCR, summarize) continue using
//! `TerminalView` with termimad + println. Only the interactive REPL uses
//! `RatatuiView`.

use crate::chat::command_output::{
    CommandOutput, CompactData, ContentPruneData, DocumentListData, ExportData, FactListData,
    FactListScopeData, FactRemoveResult, FactSearchData, NoteAddResult, NoteListData, ReindexData,
    SessionListData, SkillListData, TodoListData,
};
use crate::chat::session::ChatSession;
use crate::chat::strip_thinking_tags;
use crate::consts::roles::format_role_label;
use crate::debug_tools;
use crate::utils::strip_ansi_codes;

use super::super::tui::markdown::collapse_tables;
use super::{ChatView, TokenMetrics, WelcomeInfo};
use crate::chat::app::{App, LlmState};
use crate::chat::tui::components::chat_area::ChatMessage;
use crate::chat::tui::markdown::MarkdownTheme;
use crate::chat::tui::{TuiTerminal, enter_tui, exit_tui, restore_terminal_on_panic};

/// Ratatui-based view for the TUI chat REPL.
///
/// Owns the `App` state and the terminal. All `ChatView` method calls
/// add messages to the App's message list and trigger a re-render.
/// The `App` handles all state (messages, input, status bar, theme).
///
/// Tool call display is routed through a global callback so that
/// `debug_tools::log_tool_call` output appears in the chat area
/// instead of corrupting the alternate screen with raw stderr.
pub struct RatatuiView {
    app: App,
    terminal: TuiTerminal,
    welcome_shown: bool,
    tool_call_rx: std::sync::mpsc::Receiver<String>,
    restored: bool,
    /// Sender for embedding progress updates (cloned by background tasks)
    embedding_tx: tokio::sync::mpsc::UnboundedSender<(usize, usize)>,
}

impl RatatuiView {
    /// Create a new RatatuiView with the given theme and model names.
    ///
    /// Initializes the terminal for TUI mode (raw mode, alternate screen),
    /// then creates the App state. Also sets up a global callback so that
    /// tool call output (`debug_tools::log_tool_call`) is routed through the
    /// chat area instead of corrupting the alternate screen with raw stderr.
    /// Call `restore()` when done to clean up the terminal.
    pub fn new(theme: MarkdownTheme, model_names: Vec<String>) -> Self {
        // Cannot proceed without terminal — fatal error is appropriate here
        #[allow(clippy::expect_used)] // TUI init failure is unrecoverable
        let terminal = enter_tui().expect("Failed to initialize TUI terminal");

        // Install a panic hook that restores the terminal
        let default_panic_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal_on_panic();
            default_panic_hook(info);
        }));

        let (app, embedding_tx) = App::with_embedding_channel(theme, model_names);

        // Set up tool call callback: route debug_tools output to the chat area
        // instead of raw stderr (which would corrupt the TUI alternate screen).
        let (tool_call_tx, tool_call_rx) = std::sync::mpsc::channel::<String>();
        let callback = std::sync::Arc::new(move |line: &str| {
            let _ = tool_call_tx.send(line.to_string());
        }) as std::sync::Arc<dyn Fn(&str) + Sync + Send>;
        debug_tools::set_tui_callback(Some(callback));

        Self {
            app,
            terminal,
            welcome_shown: false,
            tool_call_rx,
            restored: false,
            embedding_tx,
        }
    }

    /// Get a mutable reference to the App state.
    ///
    /// Used by the REPL loop for key event handling and state queries.
    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    /// Get a reference to the App state.
    pub fn app(&self) -> &App {
        &self.app
    }

    /// Get a clone of the embedding progress sender.
    ///
    /// Background embedding tasks can use this to report progress
    /// as `(current, total)` tuples.
    pub fn embedding_tx(&self) -> tokio::sync::mpsc::UnboundedSender<(usize, usize)> {
        self.embedding_tx.clone()
    }

    /// Get a mutable reference to the terminal.
    ///
    /// Used by the REPL loop for crossterm event polling.
    pub fn terminal_mut(&mut self) -> &mut TuiTerminal {
        &mut self.terminal
    }

    /// Restore the terminal to its original state.
    ///
    /// Must be called before exiting to prevent broken terminal state.
    /// Sets the `restored` flag so that `Drop` does not double-restore.
    /// Also clears the global TUI callback so tool calls go back to stderr.
    pub fn restore(mut self) {
        self.restored = true;
        // Clear the TUI callback so tool calls go back to stderr
        debug_tools::set_tui_callback(None);
        let _ = exit_tui(&mut self.terminal);
        let _ = self.app.save_history();
    }

    /// Render the current state to the terminal.
    ///
    /// Also ticks the spinner animation so it advances one frame per render.
    /// This is crucial for spinner visibility during LLM processing: even
    /// though the main event loop is blocked on `handle_user_message_tui`,
    /// each `show_*` method calls `render()`, which ticks the spinner.
    ///
    /// Note: tool call messages from `tool_call_rx` are NOT drained here.
    /// They are drained in the event loop via `drain_tool_messages()` so
    /// that ordering relative to LLM events can be controlled.
    pub fn render(&mut self) {
        let _ = self.app.render(&mut self.terminal);
    }

    /// Drain pending tool call messages from the global callback channel.
    ///
    /// Returns a Vec of tool call log lines that should be inserted into
    /// the chat area. Called from the event loop (not from render) so that
    /// tool messages can be inserted in the correct position relative to
    /// LLM events (before the streaming zone when LLM is active).
    pub fn drain_tool_messages(&mut self) -> Vec<String> {
        let mut messages = Vec::new();
        while let Ok(line) = self.tool_call_rx.try_recv() {
            messages.push(line);
        }
        messages
    }
}

impl ChatView for RatatuiView {
    /// TUI uses a built-in spinner in the status bar — indicatif spinners
    /// would corrupt the alternate screen buffer with ANSI escape codes.
    fn suppress_progress_spinner(&self) -> bool {
        true
    }

    fn show_system(&mut self, message: &str) {
        self.add_system_message(message);
        self.render();
    }

    fn show_error(&mut self, error: &str) {
        self.app.add_message(ChatMessage::error(error.to_string()));
        self.render();
    }

    fn show_assistant_response(&mut self, content: &str, thinking: Option<&str>) {
        // Show thinking block first if present
        if let Some(thinking_content) = thinking {
            self.app
                .add_message(ChatMessage::thinking(thinking_content.to_string()));
        }

        // Replace any streaming message with the final markdown version
        self.app
            .add_message(ChatMessage::assistant_markdown(content.to_string()));
        self.app.set_llm_state(LlmState::Idle);
        self.render();
    }

    fn show_token_metrics(&mut self, metrics: &TokenMetrics) {
        if metrics.total_tokens > 0 {
            let msg = format!(
                "[Tokens: {} prompt + {} response = {} total]",
                metrics.prompt_tokens, metrics.response_tokens, metrics.total_tokens
            );
            self.add_system_message(&msg);

            // Update the status bar progress with the latest tokens.
            // This ensures the bar stays current after each LLM response,
            // continuation, and tool call — not just after the final response.
            let max_tokens = self.app.status_bar().max_tokens;
            let percent = if max_tokens > 0 {
                ((metrics.total_tokens as f64 / max_tokens as f64) * 100.0).min(100.0) as u8
            } else {
                0
            };
            self.app
                .update_status_tokens(metrics.total_tokens as usize, max_tokens, percent);

            self.render();
        }
    }

    fn show_context_warning(&mut self, percent: u8, message: &str) {
        let msg = format!("⚠ Context {}% full. {}", percent, message);
        self.add_system_message(&msg);

        // Update the status bar progress to reflect the current context fill.
        self.app.update_status_tokens(
            self.app.status_bar().used_tokens,
            self.app.status_bar().max_tokens,
            percent,
        );

        self.render();
    }

    fn show_compact_progress(&mut self, message: &str) {
        let msg = format!("⏳ {}", message);
        self.add_system_message(&msg);
        self.render();
    }

    fn show_compact_complete(
        &mut self,
        count: usize,
        preserved_first: usize,
        preserved_last: usize,
    ) {
        let msg = if preserved_first > 0 || preserved_last > 0 {
            format!(
                "✓ Compacted {} messages (preserved {} first, {} last).",
                count, preserved_first, preserved_last
            )
        } else {
            format!("✓ Compacted all {} messages.", count)
        };
        self.add_system_message(&msg);
        self.render();
    }

    fn show_markdown(&mut self, content: &str) {
        self.app
            .add_message(ChatMessage::assistant_markdown(content.to_string()));
        self.render();
    }

    fn show_thinking(&mut self, thinking: &str) {
        self.app
            .add_message(ChatMessage::thinking(thinking.to_string()));
        self.render();
    }

    fn show_help_line(&mut self) {
        self.add_system_message("Type /help for commands, /quit to exit");
        self.render();
    }

    fn clear_continuation_line(&mut self) {
        // In TUI mode, we don't need to clear lines — we just re-render.
        // The continuation detection replaces the previous assistant message.
        // No-op for TUI.
    }

    fn show_command_output(&mut self, output: &CommandOutput) {
        match output {
            CommandOutput::Info(msg) => {
                self.add_system_message(msg);
            }
            CommandOutput::Success(msg) => {
                self.add_system_message(&format!("✓ {}", msg));
            }
            CommandOutput::Warning(msg) => {
                self.add_system_message(&format!("⚠️ {}", msg));
            }
            CommandOutput::Error(msg) => {
                self.app.add_message(ChatMessage::error(msg.clone()));
            }
            CommandOutput::Progress(msg) => {
                self.add_system_message(&format!("⏳ {}", msg));
            }

            // ── Structured displays ──────────────────────────────────
            CommandOutput::FactList(data) => self.render_fact_list(data),
            CommandOutput::FactRemoved(data) => self.render_fact_removed(data),
            CommandOutput::FactSearchResults(data) => self.render_fact_search(data),
            CommandOutput::NoteList(data) => self.render_note_list(data),
            CommandOutput::NoteAdded(data) => self.render_note_added(data),
            CommandOutput::TodoList(data) => self.render_todo_list(data),
            CommandOutput::ContextInfo(data) => {
                self.add_system_message(&data.formatted);
            }
            CommandOutput::SessionList(data) => self.render_session_list(data),
            CommandOutput::CompactResult(data) => self.render_compact_result(data),
            CommandOutput::ExportResult(data) => self.render_export_result(data),
            CommandOutput::SkillList(data) => self.render_skill_list(data),
            CommandOutput::DocumentList(data) => self.render_document_list(data),
            CommandOutput::ContentPruneResult(data) => self.render_content_prune(data),
            CommandOutput::SearchResults(data) => {
                self.app
                    .add_message(ChatMessage::assistant_markdown(data.formatted.clone()));
            }
            CommandOutput::ReindexResult(data) => self.render_reindex_result(data),
            CommandOutput::HelpText(text) => {
                self.add_system_message(text);
            }
            CommandOutput::MarkdownContent(content) => {
                self.app
                    .add_message(ChatMessage::assistant_markdown(content.clone()));
            }
            CommandOutput::TokenDisplay {
                prompt_tokens,
                response_tokens,
                total_tokens,
            } => {
                let msg = format!(
                    "[Tokens: {} prompt + {} response = {} total]",
                    prompt_tokens, response_tokens, total_tokens
                );
                self.add_system_message(&msg);
            }

            // ── Flow control ──────────────────────────────────────────
            CommandOutput::Quit => {
                // No output — REPL loop handles the exit
            }
        }
        self.render();
    }
}

// ── Convenience methods for RatatuiView ─────────────────────────────────

impl RatatuiView {
    /// Add a system message with ANSI codes stripped.
    ///
    /// In the TUI, system messages are rendered as plain text via `Line::raw()`.
    /// Any ANSI escape codes would appear as garbled text. This method strips
    /// ANSI codes before creating the `ChatMessage::system()`, ensuring clean
    /// rendering in the TUI while allowing the same code paths that produce
    /// ANSI-colored output for `TerminalView`.
    fn add_system_message(&mut self, text: &str) {
        let clean = strip_ansi_codes(text);
        self.app.add_message(ChatMessage::system(clean));
    }

    /// Display the welcome banner in the TUI.
    ///
    /// Uses the native ratatui banner with responsive layout:
    /// - Wide terminals: image + session info side-by-side
    /// - Medium terminals: styled "SPRACH SPIEL" text + session info
    /// - Narrow terminals: session info only
    #[expect(clippy::too_many_arguments)]
    pub fn show_welcome(
        &mut self,
        model_id: &str,
        tools_enabled: bool,
        think_enabled: bool,
        vision_enabled: bool,
        sandbox_status: &str,
        project: &str,
        session_name: &str,
        is_anonymous: bool,
        version: &str,
        server_url: &str,
        fact_count: i64,
        note_count: i64,
        doc_count: i64,
        skill_count: usize,
    ) {
        if self.welcome_shown {
            return;
        }

        let info = WelcomeInfo {
            model_id: model_id.to_string(),
            tools_enabled,
            think_enabled,
            vision_enabled,
            sandbox_status: sandbox_status.to_string(),
            project: project.to_string(),
            session_name: session_name.to_string(),
            is_anonymous,
            version: version.to_string(),
            server_url: server_url.to_string(),
            fact_count,
            note_count,
            doc_count,
            skill_count,
        };

        // Use plain session lines (no ANSI) for TUI rendering
        let session_lines = info.format_session_lines_plain();
        let banner_content = session_lines.join("\n");

        self.app.add_message(ChatMessage::banner(banner_content));
        self.welcome_shown = true;
        self.render();
    }

    /// Display recent context summary for a resumed session.
    ///
    /// Shows the last few exchanges (user+assistant pairs) from the session.
    /// Each exchange occupies exactly one visual line, truncated to the
    /// current terminal width using Unicode-aware visual column measurement.
    pub fn show_recent_context(&mut self, session: &ChatSession) {
        const MAX_EXCHANGES: usize = 3;
        let exchanges = session.get_recent_exchanges(MAX_EXCHANGES);

        if exchanges.is_empty() {
            return;
        }

        let total_messages = session.messages.len();

        // Get terminal width from crossterm for responsive truncation
        let terminal_width = crossterm::terminal::size()
            .map(|(cols, _)| cols as usize)
            .unwrap_or(80);

        // Header line: "  Recent context (N messages):"
        let header = format!("  Recent context ({} messages):", total_messages);

        let user_label = format_role_label("user");
        let assistant_label = format_role_label("assistant");

        // Measure visual widths of role labels
        let user_label_width = unicode_width::UnicodeWidthStr::width(user_label.as_str());
        let assistant_label_width = unicode_width::UnicodeWidthStr::width(assistant_label.as_str());

        // Each line: "  {label}: {content}"
        // Available content width = terminal_width - 2 (indent) - label_width - 2 (": ")
        let user_content_width = terminal_width.saturating_sub(2 + user_label_width + 2);
        let assistant_content_width = terminal_width.saturating_sub(2 + assistant_label_width + 2);

        let mut lines = vec![header];

        for (user_msg, asst_msg) in exchanges {
            let user_content = strip_thinking_tags(&user_msg.content).replace('\n', " ");
            let truncated_user =
                crate::utils::truncate_visual_width(&user_content, user_content_width);
            lines.push(format!("  {}: {}", user_label, truncated_user));

            if let Some(asst) = asst_msg {
                let asst_content =
                    collapse_tables(&strip_thinking_tags(&asst.content)).replace('\n', " ");
                let truncated_asst =
                    crate::utils::truncate_visual_width(&asst_content, assistant_content_width);
                lines.push(format!("  {}: {}", assistant_label, truncated_asst));
            }
        }

        let summary = lines.join("\n");
        self.add_system_message(&summary);
        self.render();
    }

    /// Update the status bar with model information.
    ///
    /// Called when model switches or capabilities change.
    pub fn update_status_model(
        &mut self,
        model_name: &str,
        think_enabled: bool,
        tools_enabled: bool,
    ) {
        self.app
            .update_status_model(model_name, think_enabled, tools_enabled);
        self.render();
    }

    /// Update the status bar with token usage information.
    pub fn update_status_tokens(&mut self, used_tokens: usize, max_tokens: usize, percent: u8) {
        self.app
            .update_status_tokens(used_tokens, max_tokens, percent);
        self.render();
    }

    /// Set the LLM processing state (affects spinner and input enabled).
    pub fn set_llm_state(&mut self, state: LlmState) {
        self.app.set_llm_state(state);
        self.render();
    }

    /// Append a streaming token to the chat area.
    ///
    /// Creates or appends to an `AssistantStreaming` message.
    /// Called for each token chunk from the LLM during streaming.
    pub fn stream_token(&mut self, token: &str) {
        // If we're in Thinking or ToolCall state, transition to Streaming
        if self.app.llm_state() == LlmState::Thinking || self.app.llm_state() == LlmState::ToolCall
        {
            self.app.set_llm_state(LlmState::Streaming);
        }
        self.app.append_stream_token(token);
        self.render();
    }

    /// Append a streaming thinking token to the chat area.
    ///
    /// Creates or appends to a `Thinking` message.
    /// Called for each thinking chunk from the LLM during streaming.
    pub fn stream_thinking(&mut self, token: &str) {
        // If we're in Idle or ToolCall state, transition to Thinking
        if self.app.llm_state() == LlmState::Idle || self.app.llm_state() == LlmState::ToolCall {
            self.app.set_llm_state(LlmState::Thinking);
        }
        self.app.append_stream_thinking(token);
        self.render();
    }

    /// Finalize the streaming response.
    ///
    /// Replaces the `AssistantStreaming` message with the final
    /// markdown-rendered `Assistant` message. Shows token metrics
    /// if available. Transitions LLM state to Idle.
    pub fn stream_done(
        &mut self,
        content: &str,
        thinking: Option<&str>,
        metrics: Option<&TokenMetrics>,
    ) {
        self.app.finalize_stream(content, thinking);

        // Show token metrics if available
        if let Some(m) = metrics
            && m.total_tokens > 0
        {
            let msg = format!(
                "[Tokens: {} prompt + {} response = {} total]",
                m.prompt_tokens, m.response_tokens, m.total_tokens
            );
            self.add_system_message(&msg);

            // Update status bar progress
            let max_tokens = self.app.status_bar().max_tokens;
            let percent = if max_tokens > 0 {
                ((m.total_tokens as f64 / max_tokens as f64) * 100.0).min(100.0) as u8
            } else {
                0
            };
            self.app
                .update_status_tokens(m.total_tokens as usize, max_tokens, percent);
        }

        self.app.set_llm_state(LlmState::Idle);
        self.render();
    }
}

// ── Render methods for structured CommandOutput variants ─────────────

impl RatatuiView {
    fn render_fact_list(&mut self, data: &FactListData) {
        let mut lines = String::new();

        match data.scope {
            FactListScopeData::All => {
                if !data.global_facts.is_empty() {
                    lines.push_str("Global facts:\n");
                    for fact in &data.global_facts {
                        lines.push_str(&format!(
                            "  #{} [{}] {}\n",
                            fact.id, fact.category, fact.content
                        ));
                    }
                }
                if !data.project_facts.is_empty() {
                    lines.push_str("Project facts:\n");
                    for fact in &data.project_facts {
                        lines.push_str(&format!(
                            "  #{} [{}] {}\n",
                            fact.id, fact.category, fact.content
                        ));
                    }
                }
                if data.global_facts.is_empty() && data.project_facts.is_empty() {
                    lines.push_str("No facts stored.\n");
                }
            }
            FactListScopeData::Global => {
                if data.global_facts.is_empty() {
                    lines.push_str("No global facts stored.\n");
                } else {
                    lines.push_str("Global facts:\n");
                    for fact in &data.global_facts {
                        lines.push_str(&format!(
                            "  #{} [{}] {}\n",
                            fact.id, fact.category, fact.content
                        ));
                    }
                }
            }
            FactListScopeData::Project => {
                if data.project_facts.is_empty() {
                    lines.push_str("No project facts stored.\n");
                } else {
                    lines.push_str("Project facts:\n");
                    for fact in &data.project_facts {
                        lines.push_str(&format!(
                            "  #{} [{}] {}\n",
                            fact.id, fact.category, fact.content
                        ));
                    }
                }
            }
        }

        self.add_system_message(&lines);
    }

    fn render_fact_removed(&mut self, data: &FactRemoveResult) {
        if data.success {
            if let Some(content) = &data.content {
                let msg = format!("✓ Removed fact #{}: {}", data.id, content);
                self.add_system_message(&msg);
            }
        } else if let Some(error) = &data.error {
            self.app.add_message(ChatMessage::error(error.clone()));
        }
    }

    fn render_fact_search(&mut self, data: &FactSearchData) {
        if data.results.is_empty() {
            self.add_system_message(&format!("No facts found matching '{}'.", data.query));
            return;
        }
        let mut lines = format!(
            "Facts matching '{}' ({} results):\n",
            data.query, data.total
        );
        for result in &data.results {
            lines.push_str(&format!(
                "  #{} [{:.2}] {}\n",
                result.id, result.score, result.content
            ));
        }
        self.add_system_message(&lines);
    }

    fn render_note_list(&mut self, data: &NoteListData) {
        if data.notes.is_empty() {
            self.add_system_message("No notes stored.");
            return;
        }
        let mut lines = format!(
            "Notes (page {}/{}, {} total):\n",
            data.page, data.total_pages, data.total_notes
        );
        for note in &data.notes {
            let title = note.title.as_deref().unwrap_or("(untitled)");
            lines.push_str(&format!("  #{} {}\n", note.id, title));
        }
        if data.total_pages > 1 {
            lines.push_str("  Use /note list --page N to see more");
        }
        self.add_system_message(&lines);
    }

    fn render_note_added(&mut self, data: &NoteAddResult) {
        if data.success {
            self.add_system_message(&format!("✓ {}", data.message));
        } else {
            self.app
                .add_message(ChatMessage::error(data.message.clone()));
        }
    }

    fn render_todo_list(&mut self, data: &TodoListData) {
        if data.count == 0 {
            self.add_system_message("No tasks.");
        } else {
            self.add_system_message(&data.formatted_list);
        }
    }

    fn render_session_list(&mut self, data: &SessionListData) {
        if data.is_empty {
            self.add_system_message("No saved sessions for this project.");
            return;
        }
        let mut lines = String::from("Sessions for this project:\n");
        for entry in &data.sessions {
            let marker = if entry.is_current { " (current)" } else { "" };
            let age = entry.updated_at.as_deref().unwrap_or("");
            let age_display = if age.is_empty() {
                String::new()
            } else {
                format!(", {}", age)
            };
            lines.push_str(&format!(
                "  • {}{} [{} messages{}]\n",
                entry.name, marker, entry.message_count, age_display
            ));
        }
        self.add_system_message(&lines);
    }

    fn render_compact_result(&mut self, data: &CompactData) {
        if data.preserved_first > 0 || data.preserved_last > 0 {
            let msg = format!(
                "✓ Compacted {} messages (preserved {} first, {} last).",
                data.count, data.preserved_first, data.preserved_last
            );
            self.add_system_message(&msg);
        } else {
            let msg = format!("✓ Compacted all {} messages.", data.count);
            self.add_system_message(&msg);
        }
    }

    fn render_export_result(&mut self, data: &ExportData) {
        let mut lines = String::new();
        if let Some(path) = &data.file_path {
            lines.push_str(&format!("Conversation exported to: {}\n", path));
        }
        lines.push_str(&data.content);
        self.add_system_message(&lines);
    }

    fn render_skill_list(&mut self, data: &SkillListData) {
        if data.skills.is_empty() {
            self.add_system_message("No skills available.");
            return;
        }
        let mut lines = String::from("Available skills:\n");
        for skill in &data.skills {
            lines.push_str(&format!("  {} - {}\n", skill.name, skill.description));
        }
        lines.push_str("Use /skill <name> to activate a skill.");
        self.add_system_message(&lines);
    }

    fn render_document_list(&mut self, data: &DocumentListData) {
        if data.is_empty {
            self.add_system_message("No documents imported.");
            return;
        }
        let mut lines = String::from("Imported documents:\n");
        for doc in &data.documents {
            let age_days = (chrono::Utc::now() - doc.created_at).num_days();
            lines.push_str(&format!(
                "  #{} {} ({}, {} words, {}d)\n",
                doc.id, doc.title, doc.source_type, doc.word_count, age_days
            ));
        }
        self.add_system_message(&lines);
    }

    fn render_content_prune(&mut self, data: &ContentPruneData) {
        if data.success {
            let msg = format!(
                "✓ Pruned {}/{} content items.",
                data.pruned_count, data.total_count
            );
            self.add_system_message(&msg);
        } else if let Some(error) = &data.error {
            self.app.add_message(ChatMessage::error(format!(
                "Failed to prune content: {}",
                error
            )));
        }
    }

    fn render_reindex_result(&mut self, data: &ReindexData) {
        if data.success {
            let msg = format!(
                "✓ Regenerated {} of {} embeddings.",
                data.regenerated, data.total
            );
            self.add_system_message(&msg);
        } else if let Some(error) = &data.error {
            self.app
                .add_message(ChatMessage::error(format!("Reindex failed: {}", error)));
        }
    }
}

impl Drop for RatatuiView {
    fn drop(&mut self) {
        // Safety net: restore terminal if restore() wasn't called explicitly.
        // The restored flag prevents double-restore when both restore() and
        // Drop run (restore() is consuming, so Drop only runs if restore()
        // was never called, e.g., on early return or panic).
        if !self.restored {
            let _ = exit_tui(&mut self.terminal);
            let _ = self.app.save_history();
        }
    }
}
