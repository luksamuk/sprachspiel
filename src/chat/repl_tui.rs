//! TUI-based REPL loop for the chat (Phase 2.10 — WIP)
//!
//! This module provides `run_chat_repl_tui()`, which replaces the
//! blocking rustyline loop with a crossterm-based event loop that
//! renders via ratatui.
//!
//! # Status
//!
//! This is a work-in-progress skeleton. The full wiring of the REPL
//! loop (command handling, LLM calls, session management) will be
//! integrated incrementally. The event loop and rendering infrastructure
//! are functional.

use crossterm::event::{self, Event as CrosstermEvent};

use super::app::LlmState;
use super::command_handlers;
use super::command_output::CommandOutput;
use super::commands::{ChatCommand, parse_command};
use super::input::InputResult;
use super::repl_state::ReplState;
use super::tui::components::chat_area::ChatMessage;
use super::view::ChatView;
use super::view::RatatuiView;

/// Run the chat REPL using the TUI (ratatui + crossterm).
///
/// This replaces the blocking rustyline loop with a crossterm event loop
/// that renders via ratatui. All view operations go through `RatatuiView`,
/// which implements `ChatView` and delegates to `App::add_message()`.
///
/// # Architecture
///
/// The event loop:
/// 1. Polls for crossterm events with a 100ms timeout (for spinner animation)
/// 2. Processes key events via `App::handle_key()`
/// 3. Re-renders after each event or tick
/// 4. On Enter: delegates to command handlers or LLM message sending
/// 5. On Ctrl+C: cancels current operation or shows interrupt
/// 6. On Ctrl+D: quits the session
///
/// # Errors
///
/// Returns an error if terminal setup fails or an irrecoverable error occurs.
pub async fn run_chat_repl_tui(
    state: &mut ReplState,
    capabilities: &crate::capabilities::ModelCapabilities,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create the TUI view (initializes terminal, installs panic hook)
    let model_names = crate::user_models::list_all_model_names();
    let model_name = state.model_config.model_id.clone();
    let think_enabled = state.session.think;
    let tools_enabled = state.session.tools && capabilities.tools;
    let used_tokens = state.session.history_real_tokens();
    let max_tokens = state.model_config.num_ctx as usize;

    let theme = super::tui::markdown::MarkdownTheme::from_config(&state.settings.display.skin);

    let mut view = RatatuiView::new(theme, model_names);

    // Show welcome banner
    {
        let session = &state.session;
        let model_config = &state.model_config;
        let settings = &state.settings;

        let project = session.project_id.as_deref().unwrap_or("anonymous");
        let session_name = session.name.as_deref().unwrap_or(&session.id);
        let sandbox_status = crate::external::get_sandbox_status();
        let version = env!("CARGO_PKG_VERSION");
        let server_url = format!(
            "{}:{}",
            settings.model.ollama_host, settings.model.ollama_port
        );

        let (fact_count, note_count, doc_count) = if let Some(db_ref) = &state.db {
            (
                db_ref.count_facts().unwrap_or(0),
                db_ref.count_notes().unwrap_or(0),
                db_ref.count_documents().unwrap_or(0),
            )
        } else {
            (0, 0, 0)
        };

        let skill_count = if tools_enabled {
            crate::skills::load_skill_indexes().len()
        } else {
            0
        };

        view.show_welcome(
            &model_config.model_id,
            session.tools && capabilities.tools,
            session.think && capabilities.thinking,
            capabilities.vision,
            &sandbox_status,
            project,
            session_name,
            session.anonymous,
            version,
            &server_url,
            fact_count,
            note_count,
            doc_count,
            skill_count,
        );
    }

    // Show help line
    view.show_help_line();

    // Initial status bar update
    let percent = if max_tokens > 0 {
        ((used_tokens as f64 / max_tokens as f64) * 100.0).min(100.0) as u8
    } else {
        0
    };
    view.update_status_model(&model_name, think_enabled, tools_enabled);
    view.update_status_tokens(used_tokens, max_tokens, percent);

    // Main event loop
    loop {
        // Poll for events with a 100ms timeout (allows spinner animation)
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                CrosstermEvent::Key(key) => {
                    let result = view.app_mut().handle_key(key);

                    match result {
                        Some(InputResult::Line(line)) => {
                            let line = line.trim().to_string();
                            if line.is_empty() {
                                continue;
                            }

                            // Add user message to chat area
                            view.app_mut().add_message(ChatMessage::user(line.clone()));

                            // Check for slash commands
                            if line.starts_with('/') {
                                match parse_command(&line) {
                                    Some(Ok(cmd)) => {
                                        // Handle model switch specially
                                        if let ChatCommand::Model { name } = &cmd {
                                            let outputs = command_handlers::handle_model_switch(
                                                state,
                                                name,
                                                capabilities,
                                            )
                                            .await;
                                            view.show_command_outputs(&outputs);
                                            if outputs
                                                .iter()
                                                .any(|o| matches!(o, CommandOutput::Quit))
                                            {
                                                view.restore();
                                                return Ok(());
                                            }
                                            continue;
                                        }

                                        // Handle other commands
                                        let mut dummy_input =
                                            super::input::CrosstermInput::default();
                                        let outputs = command_handlers::handle_command(
                                            cmd,
                                            state,
                                            &mut dummy_input,
                                            &mut view as &mut dyn ChatView,
                                        )
                                        .await;

                                        view.show_command_outputs(&outputs);

                                        if outputs.iter().any(|o| matches!(o, CommandOutput::Quit))
                                        {
                                            view.restore();
                                            return Ok(());
                                        }
                                        continue;
                                    }
                                    Some(Err(e)) => {
                                        view.show_error(&e.to_string());
                                        continue;
                                    }
                                    None => {}
                                }
                            }

                            // Send as user message to LLM
                            handle_user_message_tui(&line, state, &mut view).await;
                        }
                        Some(InputResult::Interrupted) => {
                            // Ctrl+C — cancel current operation or ignore
                            continue;
                        }
                        Some(InputResult::Eof) => {
                            // Ctrl+D — quit
                            view.restore();
                            return Ok(());
                        }
                        Some(InputResult::Error(_)) => {
                            // Should not happen in TUI mode
                            continue;
                        }
                        None => {
                            // Other key event (buffer updated, cursor moved)
                        }
                    }
                }
                CrosstermEvent::Resize(_, _) => {
                    // Terminal resize — just re-render
                }
                _ => {
                    // Ignore other events (mouse, etc.)
                }
            }
        }

        // Tick spinner if LLM is processing
        view.app_mut().tick_spinner();

        // Re-render after each event or tick
        view.render();
    }
}

/// Handle a user message in TUI mode.
///
/// Sends the message to the LLM and displays the response via the TUI view.
/// This is a simplified version that delegates to the existing `handle_user_message()`
/// from `repl.rs`.
async fn handle_user_message_tui(line: &str, state: &mut ReplState, view: &mut RatatuiView) {
    // Set LLM state to thinking
    view.set_llm_state(LlmState::Thinking);

    // Delegate to the existing handler which uses the view for output
    super::repl::handle_user_message(line, state, view as &mut dyn ChatView).await;

    // Update token usage after response
    let used_tokens = state.session.history_real_tokens();
    let max_tokens = state.model_config.num_ctx as usize;
    let percent = if max_tokens > 0 {
        ((used_tokens as f64 / max_tokens as f64) * 100.0).min(100.0) as u8
    } else {
        0
    };
    view.update_status_tokens(used_tokens, max_tokens, percent);

    // Return to idle
    view.set_llm_state(LlmState::Idle);
}
