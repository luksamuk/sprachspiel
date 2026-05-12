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
use crate::chat::view::colors;
use crate::consts::roles::format_role_label;

use super::{ChatView, RecentContextInfo, RecentMessage, TokenMetrics, WelcomeInfo};
use crate::chat::app::{App, LlmState};
use crate::chat::tui::components::chat_area::ChatMessage;
use crate::chat::tui::markdown::MarkdownTheme;
use crate::chat::tui::{TuiTerminal, enter_tui, exit_tui, restore_terminal_on_panic};

/// Ratatui-based view for the TUI chat REPL.
///
/// Owns the `App` state and the terminal. All `ChatView` method calls
/// add messages to the App's message list and trigger a re-render.
/// The `App` handles all state (messages, input, status bar, theme).
pub struct RatatuiView {
    /// Application state (messages, input, status bar, theme)
    app: App,
    /// Ratatui terminal for rendering
    terminal: TuiTerminal,
    /// Whether we've shown the welcome banner yet
    welcome_shown: bool,
}

impl RatatuiView {
    /// Create a new RatatuiView with the given theme and model names.
    ///
    /// Initializes the terminal for TUI mode (raw mode, alternate screen).
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

        let app = App::new(theme, model_names);

        Self {
            app,
            terminal,
            welcome_shown: false,
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

    /// Get a mutable reference to the terminal.
    ///
    /// Used by the REPL loop for crossterm event polling.
    pub fn terminal_mut(&mut self) -> &mut TuiTerminal {
        &mut self.terminal
    }

    /// Restore the terminal to its original state.
    ///
    /// Must be called before exiting to prevent broken terminal state.
    /// Called automatically on panic via the installed hook.
    pub fn restore(mut self) {
        let _ = exit_tui(&mut self.terminal);
        let _ = self.app.save_history();
    }

    /// Render the current state to the terminal.
    pub fn render(&mut self) {
        let _ = self.app.render(&mut self.terminal);
    }
}

impl ChatView for RatatuiView {
    fn show_system(&mut self, message: &str) {
        self.app
            .add_message(ChatMessage::system(message.to_string()));
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
            self.app.add_message(ChatMessage::system(msg));
            self.render();
        }
    }

    fn show_context_warning(&mut self, percent: u8, message: &str) {
        let msg = format!("⚠ Context {}% full. {}", percent, message);
        self.app.add_message(ChatMessage::system(msg));
        self.render();
    }

    fn show_compact_progress(&mut self, message: &str) {
        let msg = format!("⏳ {}", message);
        self.app.add_message(ChatMessage::system(msg));
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
        self.app.add_message(ChatMessage::system(msg));
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
        self.app.add_message(ChatMessage::system(
            "Type /help for commands, /quit to exit".to_string(),
        ));
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
                self.app.add_message(ChatMessage::system(msg.clone()));
            }
            CommandOutput::Success(msg) => {
                self.app
                    .add_message(ChatMessage::system(format!("✓ {}", msg)));
            }
            CommandOutput::Warning(msg) => {
                self.app
                    .add_message(ChatMessage::system(format!("⚠️ {}", msg)));
            }
            CommandOutput::Error(msg) => {
                self.app.add_message(ChatMessage::error(msg.clone()));
            }
            CommandOutput::Progress(msg) => {
                self.app
                    .add_message(ChatMessage::system(format!("⏳ {}", msg)));
            }

            // ── Structured displays ──────────────────────────────────
            CommandOutput::FactList(data) => self.render_fact_list(data),
            CommandOutput::FactRemoved(data) => self.render_fact_removed(data),
            CommandOutput::FactSearchResults(data) => self.render_fact_search(data),
            CommandOutput::NoteList(data) => self.render_note_list(data),
            CommandOutput::NoteAdded(data) => self.render_note_added(data),
            CommandOutput::TodoList(data) => self.render_todo_list(data),
            CommandOutput::ContextInfo(data) => {
                self.app
                    .add_message(ChatMessage::system(data.formatted.clone()));
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
                self.app.add_message(ChatMessage::system(text.clone()));
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
                self.app.add_message(ChatMessage::system(msg));
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
    /// Display the welcome banner in the TUI.
    ///
    /// Renders the welcome info as a system message in the chat area.
    /// The ASCII art banner is rendered as plain text since tui-markdown
    /// doesn't handle it well. The session info lines are rendered as
    /// individual system messages.
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

        // Render the banner as a system message (plain text, ASCII art preserved)
        let banner = info.to_boxed_string();
        self.app.add_message(ChatMessage::system(banner));
        self.welcome_shown = true;
        self.render();
    }

    /// Display recent context summary for a resumed session.
    ///
    /// Shows the last few exchanges (user+assistant pairs) from the session.
    pub fn show_recent_context(&mut self, session: &ChatSession) {
        const MAX_EXCHANGES: usize = 3;
        let exchanges = session.get_recent_exchanges(MAX_EXCHANGES);

        if exchanges.is_empty() {
            return;
        }

        let total_messages = session.messages.len();

        let recent_exchanges: Vec<(RecentMessage, Option<RecentMessage>)> = exchanges
            .into_iter()
            .map(|(user_msg, asst_msg)| {
                let user = RecentMessage {
                    role_label: format_role_label("user"),
                    content: strip_thinking_tags(&user_msg.content).replace('\n', " "),
                };
                let assistant = asst_msg.map(|a| RecentMessage {
                    role_label: format_role_label("assistant"),
                    content: strip_thinking_tags(&a.content).replace('\n', " "),
                });
                (user, assistant)
            })
            .collect();

        let info = RecentContextInfo {
            total_messages,
            exchanges: recent_exchanges,
        };

        let summary = info.format_context_summary();
        if !summary.is_empty() {
            self.app.add_message(ChatMessage::system(summary));
            self.render();
        }
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
}

// ── Render methods for structured CommandOutput variants ─────────────

impl RatatuiView {
    fn render_fact_list(&mut self, data: &FactListData) {
        use colors::*;
        let mut lines = String::new();

        match data.scope {
            FactListScopeData::All => {
                if !data.global_facts.is_empty() {
                    lines.push_str(&format!("{}Global facts:{}\n", BOLD, RESET));
                    for fact in &data.global_facts {
                        lines.push_str(&format!(
                            "  {}#{} {}[{}]{} {}\n",
                            CYAN, fact.id, DIM, fact.category, RESET, fact.content
                        ));
                    }
                }
                if !data.project_facts.is_empty() {
                    lines.push_str(&format!("{}Project facts:{}\n", BOLD, RESET));
                    for fact in &data.project_facts {
                        lines.push_str(&format!(
                            "  {}#{} {}[{}]{} {}\n",
                            CYAN, fact.id, DIM, fact.category, RESET, fact.content
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
                    lines.push_str(&format!("{}Global facts:{}\n", BOLD, RESET));
                    for fact in &data.global_facts {
                        lines.push_str(&format!(
                            "  {}#{} {}[{}]{} {}\n",
                            CYAN, fact.id, DIM, fact.category, RESET, fact.content
                        ));
                    }
                }
            }
            FactListScopeData::Project => {
                if data.project_facts.is_empty() {
                    lines.push_str("No project facts stored.\n");
                } else {
                    lines.push_str(&format!("{}Project facts:{}\n", BOLD, RESET));
                    for fact in &data.project_facts {
                        lines.push_str(&format!(
                            "  {}#{} {}[{}]{} {}\n",
                            CYAN, fact.id, DIM, fact.category, RESET, fact.content
                        ));
                    }
                }
            }
        }

        self.app.add_message(ChatMessage::system(lines));
    }

    fn render_fact_removed(&mut self, data: &FactRemoveResult) {
        if data.success {
            if let Some(content) = &data.content {
                let msg = format!("✓ Removed fact #{}: {}", data.id, content);
                self.app.add_message(ChatMessage::system(msg));
            }
        } else if let Some(error) = &data.error {
            self.app.add_message(ChatMessage::error(error.clone()));
        }
    }

    fn render_fact_search(&mut self, data: &FactSearchData) {
        if data.results.is_empty() {
            self.app.add_message(ChatMessage::system(format!(
                "No facts found matching '{}'.",
                data.query
            )));
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
        self.app.add_message(ChatMessage::system(lines));
    }

    fn render_note_list(&mut self, data: &NoteListData) {
        if data.notes.is_empty() {
            self.app
                .add_message(ChatMessage::system("No notes stored.".to_string()));
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
        self.app.add_message(ChatMessage::system(lines));
    }

    fn render_note_added(&mut self, data: &NoteAddResult) {
        if data.success {
            self.app
                .add_message(ChatMessage::system(format!("✓ {}", data.message)));
        } else {
            self.app
                .add_message(ChatMessage::error(data.message.clone()));
        }
    }

    fn render_todo_list(&mut self, data: &TodoListData) {
        if data.count == 0 {
            self.app
                .add_message(ChatMessage::system("No tasks.".to_string()));
        } else {
            self.app
                .add_message(ChatMessage::system(data.formatted_list.clone()));
        }
    }

    fn render_session_list(&mut self, data: &SessionListData) {
        if data.is_empty {
            self.app.add_message(ChatMessage::system(
                "No saved sessions for this project.".to_string(),
            ));
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
        self.app.add_message(ChatMessage::system(lines));
    }

    fn render_compact_result(&mut self, data: &CompactData) {
        if data.preserved_first > 0 || data.preserved_last > 0 {
            let msg = format!(
                "✓ Compacted {} messages (preserved {} first, {} last).",
                data.count, data.preserved_first, data.preserved_last
            );
            self.app.add_message(ChatMessage::system(msg));
        } else {
            let msg = format!("✓ Compacted all {} messages.", data.count);
            self.app.add_message(ChatMessage::system(msg));
        }
    }

    fn render_export_result(&mut self, data: &ExportData) {
        let mut lines = String::new();
        if let Some(path) = &data.file_path {
            lines.push_str(&format!("Conversation exported to: {}\n", path));
        }
        lines.push_str(&data.content);
        self.app.add_message(ChatMessage::system(lines));
    }

    fn render_skill_list(&mut self, data: &SkillListData) {
        if data.skills.is_empty() {
            self.app
                .add_message(ChatMessage::system("No skills available.".to_string()));
            return;
        }
        let mut lines = String::from("Available skills:\n");
        for skill in &data.skills {
            lines.push_str(&format!("  {} - {}\n", skill.name, skill.description));
        }
        lines.push_str("Use /skill <name> to activate a skill.");
        self.app.add_message(ChatMessage::system(lines));
    }

    fn render_document_list(&mut self, data: &DocumentListData) {
        if data.is_empty {
            self.app
                .add_message(ChatMessage::system("No documents imported.".to_string()));
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
        self.app.add_message(ChatMessage::system(lines));
    }

    fn render_content_prune(&mut self, data: &ContentPruneData) {
        if data.success {
            let msg = format!(
                "✓ Pruned {}/{} content items.",
                data.pruned_count, data.total_count
            );
            self.app.add_message(ChatMessage::system(msg));
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
            self.app.add_message(ChatMessage::system(msg));
        } else if let Some(error) = &data.error {
            self.app
                .add_message(ChatMessage::error(format!("Reindex failed: {}", error)));
        }
    }
}

impl Drop for RatatuiView {
    fn drop(&mut self) {
        // Ensure terminal is restored even if restore() wasn't called
        // This is a safety net — restore() should be called explicitly
        let _ = exit_tui(&mut self.terminal);
        let _ = self.app.save_history();
    }
}
