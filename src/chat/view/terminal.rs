//! Terminal view implementation
//!
//! This module provides the `TerminalView` struct, which implements
//! the `ChatView` trait using standard terminal output (println!/eprintln!).
//!
//! All output styling (colors, icons, formatting) is concentrated here.
//! `CommandOutput` data carries semantics only — no ANSI codes.
//! This enables future migration to `RatatuiView` (W6-PR2, #146) which
//! renders the same data with ratatui widgets.

use crate::chat::command_output::{
    CommandOutput, CompactData, ContentPruneData, DocumentListData, ExportData, FactAddOutcome,
    FactAddResult, FactListData, FactListScopeData, FactRemoveResult, FactSearchData,
    NoteAddResult, NoteListData, NoteRemoveResult, ReindexData, SearchData, SessionListData,
    SkillListData, TodoListData,
};
use crate::chat::strip_thinking_tags;
use crate::consts::roles::format_role_label;
use crate::markdown;

use super::super::session::ChatSession;
use super::{ChatView, RecentContextInfo, RecentMessage, TokenMetrics, WelcomeInfo};

// ── ANSI color constants for TerminalView rendering ──────────────────

/// Terminal ANSI color codes for `TerminalView` output styling.
///
/// These are used ONLY in the terminal view implementation.
/// `CommandOutput` data carries no ANSI codes — the view applies them.
mod term_colors {
    pub const RED: &str = "\x1B[31m";
    pub const GREEN: &str = "\x1B[32m";
    pub const YELLOW: &str = "\x1B[33m";
    pub const CYAN: &str = "\x1B[36m";
    pub const BOLD_CYAN: &str = "\x1B[1;36m";
    pub const DIM: &str = "\x1B[2m";
    pub const BOLD: &str = "\x1B[1m";
    pub const RESET: &str = "\x1B[0m";
}

/// Terminal output backend using println!/eprintln!
///
/// This implementation provides:
/// - System message display (plain text)
/// - Error display (red colored)
/// - Assistant response display with markdown rendering
/// - Token metrics display
/// - Context warnings (yellow colored)
/// - Compaction progress/complete messages
/// - Command output rendering via `CommandOutput` enum
pub struct TerminalView;

impl TerminalView {
    /// Create a new TerminalView instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for TerminalView {
    fn default() -> Self {
        Self::new()
    }
}

// ── ChatView trait implementation ────────────────────────────────────

impl ChatView for TerminalView {
    fn show_system(&mut self, message: &str) {
        println!("{}", message);
    }

    fn show_error(&mut self, error: &str) {
        eprintln!("{}✗ {}{}", term_colors::RED, error, term_colors::RESET);
    }

    fn show_assistant_response(&mut self, content: &str, thinking: Option<&str>) {
        // Display thinking content first if present (dimmed)
        if let Some(thinking_content) = thinking {
            // Thinking is already formatted by the thinking module
            // We just display it before the main content
            let _ = thinking_content; // Thinking is handled separately by display_thinking
        }

        // Display the main response content as markdown
        markdown::print_markdown_chat(content);
    }

    fn show_token_metrics(&mut self, metrics: &TokenMetrics) {
        if metrics.total_tokens > 0 {
            eprintln!(
                "\n{}[Tokens: {} prompt + {} response = {} total]{}",
                term_colors::DIM,
                metrics.prompt_tokens,
                metrics.response_tokens,
                metrics.total_tokens,
                term_colors::RESET
            );
        }
    }

    fn show_context_warning(&mut self, percent: u8, message: &str) {
        eprintln!(
            "{}⚠ Context {}% full. {}{}",
            term_colors::YELLOW,
            percent,
            message,
            term_colors::RESET
        );
    }

    fn show_compact_progress(&mut self, message: &str) {
        eprintln!(
            "{}⏳ {}{}",
            term_colors::YELLOW,
            message,
            term_colors::RESET
        );
    }

    fn show_compact_complete(
        &mut self,
        count: usize,
        preserved_first: usize,
        preserved_last: usize,
    ) {
        if preserved_first > 0 || preserved_last > 0 {
            // Middle compaction
            eprintln!(
                "{}✓ Compacted {} messages{} (preserved {} first, {} last).{}",
                term_colors::GREEN,
                count,
                term_colors::RESET,
                preserved_first,
                preserved_last,
                term_colors::RESET
            );
        } else {
            // Full compaction (backward compatible)
            eprintln!(
                "{}✓ Compacted all {} messages.{}",
                term_colors::GREEN,
                count,
                term_colors::RESET
            );
        }
    }

    fn show_command_output(&mut self, output: &crate::chat::CommandOutput) {
        match output {
            CommandOutput::Info(msg) => {
                println!("{}", msg);
            }
            CommandOutput::Success(msg) => {
                eprintln!("{}✓ {}{}", term_colors::GREEN, msg, term_colors::RESET);
            }
            CommandOutput::Warning(msg) => {
                eprintln!("{}⚠️ {}{}", term_colors::YELLOW, msg, term_colors::RESET);
            }
            CommandOutput::Error(msg) => {
                eprintln!("{}✗ {}{}", term_colors::RED, msg, term_colors::RESET);
            }
            CommandOutput::Progress(msg) => {
                eprintln!("{}⏳ {}{}", term_colors::YELLOW, msg, term_colors::RESET);
            }

            // ── Structured displays ──────────────────────────────────
            CommandOutput::FactList(data) => self.render_fact_list(data),
            CommandOutput::FactAdded(data) => self.render_fact_added(data),
            CommandOutput::FactRemoved(data) => self.render_fact_removed(data),
            CommandOutput::FactSearchResults(data) => self.render_fact_search(data),
            CommandOutput::NoteList(data) => self.render_note_list(data),
            CommandOutput::NoteAdded(data) => self.render_note_added(data),
            CommandOutput::NoteRemoved(data) => self.render_note_removed(data),
            CommandOutput::TodoList(data) => self.render_todo_list(data),
            CommandOutput::ContextInfo(data) => {
                println!("{}", data.formatted);
            }
            CommandOutput::SessionList(data) => self.render_session_list(data),
            CommandOutput::CompactResult(data) => self.render_compact_result(data),
            CommandOutput::ExportResult(data) => self.render_export_result(data),
            CommandOutput::SkillList(data) => self.render_skill_list(data),
            CommandOutput::DocumentList(data) => self.render_document_list(data),
            CommandOutput::ContentPruneResult(data) => self.render_content_prune(data),
            CommandOutput::SearchResults(data) => {
                println!("{}", data.formatted);
            }
            CommandOutput::ReindexResult(data) => self.render_reindex_result(data),
            CommandOutput::HelpText(text) => {
                print!("{}", text);
            }

            // ── Flow control ──────────────────────────────────────────
            CommandOutput::Quit => {
                // No output — REPL loop handles the exit
            }
        }
    }

    fn show_command_outputs(&mut self, outputs: &[crate::chat::CommandOutput]) {
        for output in outputs {
            self.show_command_output(output);
        }
    }
}

// ── Convenience methods for TerminalView ─────────────────────────────

impl TerminalView {
    /// Display the welcome banner
    ///
    /// This is a convenience method that uses WelcomeInfo internally.
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
        println!("{}", info.to_boxed_string());
    }

    /// Display recent context summary for a resumed session.
    ///
    /// Shows the last 3 exchanges (user+assistant pairs) from the session,
    /// with role labels and truncated content. Only displayed when resuming
    /// a session with messages, not for new or anonymous sessions.
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
            // Truncate each line to CHAT_TERMINAL_WIDTH (80) visual columns,
            // preserving ANSI escape codes
            let chat_width = crate::markdown::CHAT_TERMINAL_WIDTH;
            for line in summary.lines() {
                let truncated = super::truncate_visual(line, chat_width);
                println!("{}", truncated);
            }
        }
    }
}

// ── Render methods for structured CommandOutput variants ──────────────

impl TerminalView {
    fn render_fact_list(&mut self, data: &FactListData) {
        match data.scope {
            FactListScopeData::All => {
                if !data.global_facts.is_empty() {
                    println!("\n{}Global facts:{}", term_colors::BOLD, term_colors::RESET);
                    for fact in &data.global_facts {
                        println!(
                            "  {}#{} {}[{}]{} {}",
                            term_colors::CYAN,
                            fact.id,
                            term_colors::DIM,
                            fact.category,
                            term_colors::RESET,
                            fact.content
                        );
                    }
                }
                if !data.project_facts.is_empty() {
                    println!(
                        "\n{}Project facts:{}",
                        term_colors::BOLD,
                        term_colors::RESET
                    );
                    for fact in &data.project_facts {
                        println!(
                            "  {}#{} {}[{}]{} {}",
                            term_colors::CYAN,
                            fact.id,
                            term_colors::DIM,
                            fact.category,
                            term_colors::RESET,
                            fact.content
                        );
                    }
                }
                if data.global_facts.is_empty() && data.project_facts.is_empty() {
                    println!("No facts stored.");
                }
            }
            FactListScopeData::Global => {
                if data.global_facts.is_empty() {
                    println!("No global facts stored.");
                } else {
                    println!("\n{}Global facts:{}", term_colors::BOLD, term_colors::RESET);
                    for fact in &data.global_facts {
                        println!(
                            "  {}#{} {}[{}]{} {}",
                            term_colors::CYAN,
                            fact.id,
                            term_colors::DIM,
                            fact.category,
                            term_colors::RESET,
                            fact.content
                        );
                    }
                }
            }
            FactListScopeData::Project => {
                if data.project_facts.is_empty() {
                    println!("No project facts stored.");
                } else {
                    println!(
                        "\n{}Project facts:{}",
                        term_colors::BOLD,
                        term_colors::RESET
                    );
                    for fact in &data.project_facts {
                        println!(
                            "  {}#{} {}[{}]{} {}",
                            term_colors::CYAN,
                            fact.id,
                            term_colors::DIM,
                            fact.category,
                            term_colors::RESET,
                            fact.content
                        );
                    }
                }
            }
        }
    }

    fn render_fact_added(&mut self, data: &FactAddResult) {
        match data.outcome {
            FactAddOutcome::Stored => {
                eprintln!(
                    "{}✓ Fact stored: #{}{} {}",
                    term_colors::GREEN,
                    data.fact.id,
                    term_colors::RESET,
                    data.fact.content
                );
            }
            FactAddOutcome::Updated(_) => {
                eprintln!(
                    "{}✓ Fact updated: #{}{} {}",
                    term_colors::CYAN,
                    data.fact.id,
                    term_colors::RESET,
                    data.fact.content
                );
            }
            FactAddOutcome::ExactDuplicate => {
                eprintln!(
                    "{}⚠️ Exact duplicate — skipped{}",
                    term_colors::YELLOW,
                    term_colors::RESET
                );
            }
            FactAddOutcome::NormalizedDuplicate => {
                eprintln!(
                    "{}⚠️ Normalized duplicate — skipped{}",
                    term_colors::YELLOW,
                    term_colors::RESET
                );
            }
            FactAddOutcome::SemanticDuplicate => {
                eprintln!(
                    "{}⚠️ Semantic duplicate — skipped{}",
                    term_colors::YELLOW,
                    term_colors::RESET
                );
            }
            FactAddOutcome::Fts5Conflict => {
                eprintln!(
                    "{}⚠️ FTS5 conflict — skipped{}",
                    term_colors::YELLOW,
                    term_colors::RESET
                );
            }
            FactAddOutcome::ContentTooLong(max) => {
                eprintln!(
                    "{}✗ Fact content exceeds {} character limit{}",
                    term_colors::RED,
                    max,
                    term_colors::RESET
                );
            }
        }
    }

    fn render_fact_removed(&mut self, data: &FactRemoveResult) {
        if data.success {
            if let Some(content) = &data.content {
                eprintln!(
                    "{}✓ Removed fact #{}: {}{}",
                    term_colors::GREEN,
                    data.id,
                    content,
                    term_colors::RESET
                );
            }
        } else if let Some(error) = &data.error {
            eprintln!("{}✗ {}{}", term_colors::RED, error, term_colors::RESET);
        }
    }

    fn render_fact_search(&mut self, data: &FactSearchData) {
        if data.results.is_empty() {
            println!("No facts found matching '{}'.", data.query);
            return;
        }
        println!(
            "\n{}Facts matching '{}' ({} results):{}",
            term_colors::BOLD,
            data.query,
            data.total,
            term_colors::RESET
        );
        for result in &data.results {
            println!(
                "  {}#{} {}[{:.2}]{} {}",
                term_colors::CYAN,
                result.id,
                term_colors::DIM,
                result.score,
                term_colors::RESET,
                result.content
            );
        }
    }

    fn render_note_list(&mut self, data: &NoteListData) {
        if data.notes.is_empty() {
            println!("No notes stored.");
            return;
        }
        println!(
            "{}Notes (page {}/{}, {} total):{}",
            term_colors::BOLD,
            data.page + 1,
            data.total_pages,
            data.total_notes,
            term_colors::RESET
        );
        for note in &data.notes {
            let title = note.title.as_deref().unwrap_or("(untitled)");
            println!(
                "  {}#{} {}{}",
                term_colors::CYAN,
                note.id,
                title,
                term_colors::RESET
            );
        }
        if data.total_pages > 1 {
            println!(
                "  {}Use /note list --page N to see more{}",
                term_colors::DIM,
                term_colors::RESET
            );
        }
    }

    fn render_note_added(&mut self, data: &NoteAddResult) {
        if data.success {
            eprintln!(
                "{}✓ {}{}",
                term_colors::GREEN,
                data.message,
                term_colors::RESET
            );
        } else {
            eprintln!(
                "{}✗ {}{}",
                term_colors::RED,
                data.message,
                term_colors::RESET
            );
        }
    }

    fn render_note_removed(&mut self, data: &NoteRemoveResult) {
        if data.success {
            eprintln!(
                "{}✓ {}{}",
                term_colors::GREEN,
                data.message,
                term_colors::RESET
            );
        } else {
            eprintln!(
                "{}✗ {}{}",
                term_colors::RED,
                data.message,
                term_colors::RESET
            );
        }
    }

    fn render_todo_list(&mut self, data: &TodoListData) {
        if data.count == 0 {
            println!("No tasks.");
        } else {
            println!("{}", data.formatted_list);
        }
    }

    fn render_session_list(&mut self, data: &SessionListData) {
        if data.is_empty {
            println!("No saved sessions for this project.");
            return;
        }
        println!("Sessions for this project:");
        for entry in &data.sessions {
            let marker = if entry.is_current { " (current)" } else { "" };
            println!(
                "  {}• {}{} {}[{} messages]{}",
                term_colors::CYAN,
                entry.name,
                marker,
                term_colors::DIM,
                entry.message_count,
                term_colors::RESET
            );
        }
    }

    fn render_compact_result(&mut self, data: &CompactData) {
        if data.preserved_first > 0 || data.preserved_last > 0 {
            eprintln!(
                "{}✓ Compacted {} messages{} (preserved {} first, {} last).{}",
                term_colors::GREEN,
                data.count,
                term_colors::RESET,
                data.preserved_first,
                data.preserved_last,
                term_colors::RESET
            );
        } else {
            eprintln!(
                "{}✓ Compacted all {} messages.{}",
                term_colors::GREEN,
                data.count,
                term_colors::RESET
            );
        }
    }

    fn render_export_result(&mut self, data: &ExportData) {
        if let Some(path) = &data.file_path {
            println!("Conversation exported to: {}", path);
        }
        println!("{}", data.content);
    }

    fn render_skill_list(&mut self, data: &SkillListData) {
        if data.skills.is_empty() {
            println!("No skills available.");
            return;
        }
        println!("Available skills:");
        for skill in &data.skills {
            println!("  {} - {}", skill.name, skill.description);
        }
        println!("\nUse /skill <name> to activate a skill.");
    }

    fn render_document_list(&mut self, data: &DocumentListData) {
        if data.is_empty {
            println!("No documents imported.");
            return;
        }
        println!("Imported documents:");
        for doc in &data.documents {
            println!(
                "  {}#{} {} {}[{}]{} {}[{} chunks]{}",
                term_colors::CYAN,
                doc.id,
                doc.title,
                term_colors::DIM,
                doc.source_type,
                term_colors::RESET,
                term_colors::DIM,
                doc.chunk_count,
                term_colors::RESET
            );
        }
    }

    fn render_content_prune(&mut self, data: &ContentPruneData) {
        if data.success {
            eprintln!(
                "{}✓ Pruned {}/{} content items.{}",
                term_colors::GREEN,
                data.pruned_count,
                data.total_count,
                term_colors::RESET
            );
        } else if let Some(error) = &data.error {
            eprintln!(
                "{}✗ Failed to prune content: {}{}",
                term_colors::RED,
                error,
                term_colors::RESET
            );
        }
    }

    fn render_reindex_result(&mut self, data: &ReindexData) {
        if data.success {
            eprintln!(
                "{}✓ Regenerated {} of {} embeddings.{}",
                term_colors::GREEN,
                data.regenerated,
                data.total,
                term_colors::RESET
            );
        } else if let Some(error) = &data.error {
            eprintln!(
                "{}✗ Reindex failed: {}{}",
                term_colors::RED,
                error,
                term_colors::RESET
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_view_show_system() {
        let mut view = TerminalView::new();
        // This would print to stdout, we just verify it compiles
        view.show_system("Test message");
    }

    #[test]
    fn test_terminal_view_show_error() {
        let mut view = TerminalView::new();
        // This would print to stderr in red, we just verify it compiles
        view.show_error("Test error");
    }

    #[test]
    fn test_terminal_view_show_welcome() {
        let mut view = TerminalView::new();
        view.show_welcome(
            "qwen3.5:4b",
            true,
            true,
            true,
            "enabled",
            "my-project",
            "default",
            false,
            "0.40.0",
            "127.0.0.1:11434",
            3,
            2,
            0,
            4,
        );
    }

    #[test]
    fn test_terminal_view_compact_complete() {
        let mut view = TerminalView::new();
        view.show_compact_complete(10, 3, 3);
        view.show_compact_complete(5, 0, 0);
    }

    #[test]
    fn test_command_output_variants_compile() {
        // Verify all CommandOutput variants can be created
        let _info = CommandOutput::Info("test".to_string());
        let _success = CommandOutput::Success("test".to_string());
        let _warning = CommandOutput::Warning("test".to_string());
        let _error = CommandOutput::Error("test".to_string());
        let _progress = CommandOutput::Progress("test".to_string());
        let _quit = CommandOutput::Quit;
    }

    #[test]
    fn test_command_output_helper_constructors() {
        let info = CommandOutput::info("test info");
        let success = CommandOutput::success("test success");
        let warning = CommandOutput::warning("test warning");
        let error = CommandOutput::error("test error");
        let progress = CommandOutput::progress("test progress");
        let quit = CommandOutput::quit();

        assert!(matches!(info, CommandOutput::Info(_)));
        assert!(matches!(success, CommandOutput::Success(_)));
        assert!(matches!(warning, CommandOutput::Warning(_)));
        assert!(matches!(error, CommandOutput::Error(_)));
        assert!(matches!(progress, CommandOutput::Progress(_)));
        assert!(matches!(quit, CommandOutput::Quit));
    }

    #[test]
    fn test_terminal_view_show_command_output() {
        let mut view = TerminalView::new();
        view.show_command_output(&CommandOutput::Info("Info message".to_string()));
        view.show_command_output(&CommandOutput::Success("Success message".to_string()));
        view.show_command_output(&CommandOutput::Warning("Warning message".to_string()));
        view.show_command_output(&CommandOutput::Error("Error message".to_string()));
        view.show_command_output(&CommandOutput::Progress("Progress message".to_string()));
        view.show_command_output(&CommandOutput::Quit);
    }

    #[test]
    fn test_terminal_view_show_command_outputs() {
        let mut view = TerminalView::new();
        let outputs = vec![
            CommandOutput::Info("First message".to_string()),
            CommandOutput::Success("Second message".to_string()),
        ];
        view.show_command_outputs(&outputs);
    }
}
