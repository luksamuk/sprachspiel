//! Command handlers for the chat REPL
//!
//! This module provides command handlers that operate on `ReplState`.
//! Each handler is responsible for processing a specific command result.
//!
//! # Architecture
//!
//! ```text
//! Layer 4 (Core): command_handlers.rs
//!     ↓ uses
//! Layer 3 (State): repl_state.rs
//! Layer 1 (Session): session.rs
//! ```
//!
//! # Handler Pattern
//!
//! Handlers return `()` and modify `ReplState` directly. They handle
//! all side effects (printing) internally. After a handler completes,
//! the REPL loop continues.

use std::sync::Arc;

use super::repl_state::ReplState;
use super::session::ToolOutputLevel;
use crate::debug_tools::log_debug;

/// Handle think mode toggle
///
/// Updates state based on the new toggle value. Prints warnings if
/// the model doesn't support thinking.
pub fn handle_think_toggled(state: &mut ReplState, new_state: bool) {
    if new_state && !state.capabilities.thinking {
        eprintln!(
            "Warning: Model '{}' does not support think mode.",
            state.model_config.model_id
        );
        state.session.think = false;
    } else {
        println!(
            "Think mode: {}",
            if new_state { "enabled" } else { "disabled" }
        );
        state.tools_active = state.session.tools && state.capabilities.tools;
    }
}

/// Handle tools toggle
///
/// Updates state based on the new toggle value. Prints warnings if
/// the model doesn't support tools.
pub fn handle_tools_toggled(state: &mut ReplState, new_state: bool) {
    if new_state && !state.capabilities.tools {
        eprintln!(
            "Warning: Model '{}' does not support tools.",
            state.model_config.model_id
        );
        state.session.tools = false;
        state.tools_active = false;
    } else {
        println!("Tools: {}", if new_state { "enabled" } else { "disabled" });
        state.tools_active = new_state && state.capabilities.tools;
    }
}

/// Handle retrieval mode toggle
///
/// Prints status message about the new retrieval state.
pub fn handle_retrieval_toggled(state: &ReplState, new_state: bool) {
    if new_state {
        println!(
            "Semantic retrieval enabled. Messages will be retrieved from history for context."
        );
        if state.session.messages.len() < 20 {
            println!(
                "Note: Retrieval activates after 20 messages (current: {})",
                state.session.messages.len()
            );
        }
    } else {
        println!("Semantic retrieval disabled.");
    }
}

/// Handle tool output level change
///
/// Prints the new tool output level.
pub fn handle_tool_output_changed(level: ToolOutputLevel) {
    println!("Tool output level: {}", level);
}

/// Handle debug mode toggle
///
/// Prints the new debug state.
pub fn handle_debug_toggled(new_state: bool) {
    println!("Debug mode: {}", new_state);
}

/// Handle undo command
///
/// Removes the last assistant messages (including preceding user message)
/// and displays the remaining last user message.
pub fn handle_undo(state: &mut ReplState) {
    let (removed, _) = state.session.remove_last_assistant_messages_with_content();
    if removed > 0 {
        if !state.session.anonymous
            && !state.session.id.is_empty()
            && let Ok(db) = crate::db::Database::new()
            && let Err(e) = db.delete_last_messages(&state.session.id, removed)
        {
            eprintln!("Warning: Failed to delete from database: {}", e);
        }
        println!("Removed {} message(s) from session.", removed);
    } else {
        println!("No messages to remove.");
    }

    if let Some(user_msg) = state.session.get_last_user_message() {
        println!("Last message: \"{}\"", user_msg.content);
        println!("(Press \u{2191} to retrieve and edit, or type a new message)");
    } else {
        println!("No user message to show.");
    }
}

/// Handle search command (async)
///
/// Searches conversation history for matching messages.
pub async fn handle_search(state: &ReplState, query: String, limit: usize) {
    let db = match crate::db::Database::new() {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Error: Failed to open database: {}", e);
            return;
        }
    };

    let conversation_id = state.session.id.clone();

    if state.use_debug {
        log_debug(&format!("Searching in conversation: {}", conversation_id));
    }

    crate::retrieval::run_search(&db, &state.ollama, &query, Some(&conversation_id), limit).await;
}

/// Handle restore command
///
/// Restores a session from the database.
pub fn handle_restore(state: &mut ReplState, session_id: String) {
    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    println!("Restoring session: {}", session_id);
    match crate::db::restore_session(&db, &state.session.project_id, &session_id) {
        Ok(restored) => {
            println!(
                "Session restored: {} ({} messages)",
                session_id,
                restored.messages.len()
            );
            state.session = restored;
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}

/// Handle reindex command (async)
///
/// Rebuilds embeddings for semantic search.
pub async fn handle_reindex(state: &mut ReplState, conversation_id: Option<String>) {
    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    let embedding_client = crate::embeddings::EmbeddingClient::new(state.ollama.clone());
    let embedding_client = Arc::new(embedding_client);

    let conv_id = conversation_id.unwrap_or_else(|| state.session.id.clone());

    println!("Reindexing conversation: {}", conv_id);
    match crate::db::reindex_conversation(&db, &embedding_client, &conv_id).await {
        Ok(stats) => {
            println!(
                "Reindex complete: {} messages, {} embeddings",
                stats.messages_migrated, stats.embeddings_generated
            );
            if !stats.errors.is_empty() {
                eprintln!("Errors:");
                for e in stats.errors {
                    eprintln!("  - {}", e);
                }
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}

/// Handle compact command (async)
///
/// Compacts conversation history by summarizing old messages.
pub async fn handle_compact(state: &mut ReplState) {
    use crate::markdown;

    if state.session.messages.is_empty() {
        println!("No messages to compact.");
        return;
    }

    let msg_count = state.session.messages.len();
    println!(
        "\x1B[33m⏳ Compacting {} messages...\x1B[0m",
        msg_count
    );

    match super::core::compact_conversation(
        &state.ollama,
        &state.model_config,
        &state.session,
        &state.settings,
        state.agents_md.as_deref(),
    )
    .await
    {
        Ok((summary, range)) => {
            let (first_preserved, last_preserved_start) =
                range.unwrap_or((0, state.session.messages.len()));
            let compacted_count = last_preserved_start - first_preserved;

            state
                .session
                .set_compacted_summary_with_range(summary.clone(), range);

            if first_preserved > 0 || last_preserved_start < state.session.messages.len() {
                // Middle compaction
                println!(
                    "\x1B[32m✓ Compacted {} messages\x1B[0m (preserved {} first, {} last).",
                    compacted_count,
                    first_preserved,
                    state.session.messages.len() - last_preserved_start
                );
            } else {
                // Full compaction (backward compatible)
                println!(
                    "\x1B[32m✓ Compacted all {} messages.\x1B[0m",
                    compacted_count
                );
            }

            println!();
            println!("\x1B[90m--- Summary ---\x1B[0m");
            markdown::print_markdown(&summary);
            println!("\x1B[90m---------------\x1B[0m");

            if !state.session.anonymous {
                let _ = state.session.save_sqlite();
            }
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Compaction failed: {}\x1B[0m", e);
        }
    }
}

/// Handle retry command (async)
///
/// Removes last assistant messages and regenerates the response.
pub async fn handle_retry(state: &mut ReplState) {
    use crate::debug_tools::log_debug;
    use crate::tool_robustness::format_tool_error;

    // Remove last assistant messages
    let removed = state.session.remove_last_assistant_messages();
    if removed > 0 {
        println!(
            "Removed {} assistant message(s). Ready to retry.",
            removed
        );
    } else {
        println!("No assistant messages to remove.");
    }

    // Get the last user message
    if let Some(user_msg) = state.session.get_last_user_message() {
        let user_content = user_msg.content.clone();
        println!("Retrying: {}", user_content);

        // Send the message again
        let think_enabled = state.session.think;
        match super::core::send_message(
            &state.ollama,
            &state.model_config,
            &mut state.session,
            &user_content,
            state.tools_active,
            think_enabled,
            false, // cli_code: false for retry (use existing config)
            &state.settings,
            state.agents_md.as_deref(),
            state.use_debug,
            state.db.as_ref(),
            state.embedding_client.as_ref(),
            state.cli_soulless,
            None,
        )
        .await
        {
            Ok(result) => {
                state.session.add_assistant_message(
                    result.response,
                    Some(result.metrics.prompt_tokens),
                );

                if result.metrics.total_tokens > 0 {
                    eprintln!(
                        "\n\x1B[90m[Tokens: {} prompt + {} response = {} total]\x1B[0m",
                        result.metrics.prompt_tokens,
                        result.metrics.response_tokens,
                        result.metrics.total_tokens
                    );
                }

                // Auto-compact if needed (after response, before next input)
                super::core::auto_compact_if_needed(
                    &state.ollama,
                    &state.model_config,
                    &mut state.session,
                    &state.settings,
                    state.agents_md.as_deref(),
                    &result.system_prompt,
                    result.context_window,
                    state.use_debug,
                )
                .await;

                if !state.session.anonymous
                    && let Err(e) = state.session.save_sqlite()
                    && state.use_debug
                {
                    log_debug(&format!("Warning: Could not save session: {}", e));
                }
            }
            Err(e) => {
                let error_str = e.to_string();
                eprintln!("\x1B[31m{}\x1B[0m", format_tool_error(&error_str));
            }
        }
    } else {
        println!("No user message to retry.");
    }
}

/// Handle fact prune command
///
/// Runs the decay cycle and prunes old facts.
pub fn handle_fact_prune(state: &ReplState) {
    use crate::facts::db::DecayStats;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot prune facts in anonymous mode.");
        return;
    }

    println!("\x1B[33m⏳ Running facts decay cycle...\x1B[0m");

    match db.run_decay_cycle() {
        Ok(DecayStats { pruned, remaining }) => {
            if pruned > 0 {
                println!(
                    "\x1B[32m✓ Pruned {} old fact(s), {} remaining.\x1B[0m",
                    pruned, remaining
                );
            } else {
                println!("\x1B[32m✓ No facts to prune. {} fact(s) remaining.\x1B[0m", remaining);
            }
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to prune facts: {}\x1B[0m", e);
        }
    }
}

/// Handle fact add command
///
/// Adds a new fact to the database.
pub fn handle_fact_add(state: &ReplState, content: String, global: bool) {
    use crate::facts::types::{Category, Fact, Scope, Source};
    use crate::facts::classify::classify_fact;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot add facts in anonymous mode.");
        return;
    }

    // Validate content length
    if content.len() > 500 {
        eprintln!("\x1B[31m✗ Fact content exceeds 500 character limit.\x1B[0m");
        println!("  Current length: {} characters", content.len());
        println!("  Use shorter content or split into multiple facts.");
        return;
    }

    // Classify the fact
    let category = classify_fact(&content);

    // Determine scope
    let scope = if global {
        Scope::Global
    } else {
        Scope::Project
    };

    // Get project ID for project-scoped facts
    let project_id = if global {
        None
    } else {
        state.session.project_id.clone()
    };

    // Create the fact
    let fact = match Fact::new(content.clone(), category, scope, project_id, Source::User) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to create fact: {}\x1B[0m", e);
            return;
        }
    };

    // Insert into database
    match db.insert_fact(&fact) {
        Ok(id) => {
            let scope_str = if global { "global" } else { "project" };
            let category_str = match category {
                Category::Preference => "preference",
                Category::Fact => "fact",
            };
            println!(
                "\x1B[32m✓ Added {} fact #{} (scope: {}, category: {})\x1B[0m",
                category_str, id, scope_str, category_str
            );
            println!("  {}", content);
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to add fact: {}\x1B[0m", e);
        }
    }
}

/// Handle fact list command
///
/// Lists all facts for the current scope.
pub fn handle_fact_list(state: &ReplState, global: bool) {
    use crate::facts::types::{Category, Scope};

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot list facts in anonymous mode.");
        return;
    }

    let scope = if global {
        Scope::Global
    } else {
        Scope::Project
    };

    let project_id = if global {
        None
    } else {
        state.session.project_id.clone()
    };

    match db.list_facts(Some(scope), None, project_id.as_deref()) {
        Ok(facts) => {
            let scope_str = if global { "global" } else { "project" };
            println!("\x1B[36mFacts ({scope_str}):\x1B[0m");

            if facts.is_empty() {
                println!("  No facts stored.");
                return;
            }

            // Group by category
            let preferences: Vec<_> = facts.iter().filter(|f| f.category == Category::Preference).collect();
            let regular_facts: Vec<_> = facts.iter().filter(|f| f.category == Category::Fact).collect();

            if !preferences.is_empty() {
                println!("\n  \x1B[33mPreferences:\x1B[0m");
                for f in preferences {
                    let age_days = (chrono::Utc::now() - f.created_at).num_days();
                    println!("    #{} {} \x1B[90m({}d)\x1B[0m", f.id, f.content, age_days);
                }
            }

            if !regular_facts.is_empty() {
                println!("\n  \x1B[33mFacts:\x1B[0m");
                for f in regular_facts {
                    let age_days = (chrono::Utc::now() - f.created_at).num_days();
                    println!("    #{} {} \x1B[90m({}d)\x1B[0m", f.id, f.content, age_days);
                }
            }

            println!("\n  \x1B[90mTotal: {} fact(s)\x1B[0m", facts.len());
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to list facts: {}\x1B[0m", e);
        }
    }
}

/// Handle fact remove command
///
/// Removes a fact by ID.
pub fn handle_fact_remove(state: &ReplState, id: i64) {
    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot remove facts in anonymous mode.");
        return;
    }

    // Get fact first to show content
    match db.get_fact(id) {
        Ok(Some(fact)) => {
            match db.delete_fact(id) {
                Ok(()) => {
                    println!(
                        "\x1B[32m✓ Removed fact #{}: {}\x1B[0m",
                        id, fact.content
                    );
                }
                Err(e) => {
                    eprintln!("\x1B[31m✗ Failed to remove fact: {}\x1B[0m", e);
                }
            }
        }
        Ok(None) => {
            eprintln!("\x1B[31m✗ Fact #{} not found.\x1B[0m", id);
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Error retrieving fact: {}\x1B[0m", e);
        }
    }
}

/// Handle fact search command
///
/// Searches facts using FTS5.
pub fn handle_fact_search(state: &ReplState, query: String, global: bool, limit: usize) {
    use crate::facts::types::Scope;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot search facts in anonymous mode.");
        return;
    }

    let scope = if global {
        Some(Scope::Global)
    } else {
        Some(Scope::Project)
    };

    match db.search_facts(&query, scope, limit) {
        Ok(results) => {
            let scope_str = if global { "global" } else { "project" };
            println!("\x1B[36mSearch results for '{}' (scope: {}):\x1B[0m", query, scope_str);

            if results.is_empty() {
                println!("  No matching facts found.");
                return;
            }

            for result in &results {
                let f = &result.fact;
                let category_str = match f.category {
                    crate::facts::types::Category::Preference => "pref",
                    crate::facts::types::Category::Fact => "fact",
                };
                let score = result.score;
                println!(
                    "  #{} [{}] {} \x1B[90m(score: {:.2})\x1B[0m",
                    f.id, category_str, f.content, score
                );
            }

            println!("\n  \x1B[90mFound {} result(s)\x1B[0m", results.len());
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Search failed: {}\x1B[0m", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::session::ChatSession;
    use super::*;
    use crate::capabilities::ModelCapabilities;
    use crate::config::ModelConfig;
    use crate::settings::Settings;
    use ollama_rs::Ollama;

    fn create_test_state() -> ReplState {
        let session = ChatSession::new("test-model".to_string(), None, false);
        let model_config = ModelConfig::get_default();
        let capabilities = ModelCapabilities::default();
        let ollama = Ollama::new("http://localhost".to_string(), 11434);
        let settings = Settings::default();

        ReplState {
            session,
            current_model_name: "test-model".to_string(),
            model_config,
            capabilities,
            tools_active: false,
            agents_md: None,
            use_debug: false,
            cli_code: false,
            cli_soulless: false,
            ollama,
            db: None,
            embedding_client: None,
            settings,
        }
    }

    #[test]
    fn test_handle_think_toggled_unsupported() {
        let mut state = create_test_state();
        state.capabilities.thinking = false;

        handle_think_toggled(&mut state, true);

        assert!(!state.session.think);
    }

    #[test]
    fn test_handle_tools_toggled_unsupported() {
        let mut state = create_test_state();
        state.capabilities.tools = false;

        handle_tools_toggled(&mut state, true);

        assert!(!state.session.tools);
        assert!(!state.tools_active);
    }

    #[test]
    fn test_handle_tools_toggled_supported() {
        let mut state = create_test_state();
        state.capabilities.tools = true;

        handle_tools_toggled(&mut state, true);

        assert!(state.tools_active);
    }

    #[test]
    fn test_handle_tools_toggled_disables_when_false() {
        let mut state = create_test_state();
        state.capabilities.tools = true;
        state.session.tools = true;
        state.tools_active = true;

        handle_tools_toggled(&mut state, false);

        assert!(!state.tools_active);
    }

    #[test]
    fn test_handle_think_toggled_enabled() {
        let mut state = create_test_state();
        state.capabilities.thinking = true;

        handle_think_toggled(&mut state, true);

        // The handler prints "Think mode: enabled" but doesn't change tools_active
        // It's a simple toggle that just validates capability support
    }

    #[test]
    fn test_handle_retrieval_toggled_enabled() {
        let state = create_test_state();

        // Should not panic when retrieval is enabled
        handle_retrieval_toggled(&state, true);
    }

    #[test]
    fn test_handle_retrieval_toggled_disabled() {
        let state = create_test_state();

        // Should not panic when retrieval is disabled
        handle_retrieval_toggled(&state, false);
    }

    #[test]
    fn test_handle_debug_toggled() {
        // Just verifying it doesn't panic
        handle_debug_toggled(true);
        handle_debug_toggled(false);
    }

    #[test]
    fn test_handle_tool_output_changed() {
        use super::super::session::ToolOutputLevel;

        // Just verifying it doesn't panic
        handle_tool_output_changed(ToolOutputLevel::Compact);
        handle_tool_output_changed(ToolOutputLevel::Full);
        handle_tool_output_changed(ToolOutputLevel::Hidden);
    }

    #[test]
    fn test_handle_undo_empty_session() {
        let mut state = create_test_state();
        state.session.messages.clear();

        handle_undo(&mut state);

        // Should print "No messages to remove" and not panic
    }
}
