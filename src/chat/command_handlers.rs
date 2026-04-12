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

use super::commands::CommandResult;
use super::repl_state::ReplState;
use super::session::ToolOutputLevel;

/// Approximate token overhead per tool definition in the system prompt.
const TOKENS_PER_TOOL: usize = 50;
use crate::capabilities::ModelCapabilities;
use crate::config::ModelConfig;
use crate::debug_tools::log_debug;
use crate::embeddings::{
    DEFAULT_CONTEXT_LENGTH, EmbedItemContext, embed_item_with_fallback,
    recover_missing_embeddings_with_progress,
};
use crate::settings::Settings;
use crate::tokens::{calculate_context_metrics, estimate_tokens};

pub use super::session::ChatSession;

/// Flush pending embeddings before exit.
///
/// This ensures that any embeddings that were being generated
/// asynchronously are completed before the application exits.
async fn flush_pending_embeddings(
    db: Arc<crate::db::Database>,
    client: Arc<crate::embeddings::EmbeddingClient>,
) {
    // Check for pending items
    let pending_items = match db.get_content_items_for_reindex() {
        Ok(items) => items.len(),
        Err(_) => 0,
    };

    let pending_chunks = match db.get_content_chunks_for_reindex() {
        Ok(chunks) => chunks.len(),
        Err(_) => 0,
    };

    if pending_items + pending_chunks == 0 {
        return;
    }

    // Complete pending embeddings with progress bar
    let _ = recover_missing_embeddings_with_progress(&db, &client).await;
}

/// Result of handling a command in the REPL loop.
pub enum HandleResult {
    /// Continue the REPL loop
    Continue,
    /// Exit the REPL
    Exit,
}

/// Handle a command result in the REPL loop.
///
/// Dispatches to the appropriate handler based on the command type.
/// Returns `HandleResult::Exit` for exit commands, `HandleResult::Continue` otherwise.
pub async fn handle_command_result(
    result: CommandResult,
    state: &mut ReplState,
    input: &mut (dyn super::input::InputBackend + Send),
) -> HandleResult {
    match result {
        CommandResult::Continue => HandleResult::Continue,
        CommandResult::Exit => {
            let _ = input.save_history();
            if !state.session.anonymous {
                let _ = state.session.save_sqlite();

                // Flush pending embeddings before exit
                if let (Some(db), Some(client)) = (&state.db, &state.embedding_client) {
                    flush_pending_embeddings(Arc::clone(db), Arc::clone(client)).await;
                }
            }
            HandleResult::Exit
        }
        CommandResult::Error(e) => {
            eprintln!("\x1B[31mError: {}\x1B[0m", e);
            HandleResult::Continue
        }
        CommandResult::ThinkToggled(new_state) => {
            handle_think_toggled(state, new_state);
            HandleResult::Continue
        }
        CommandResult::ToolsToggled(new_state) => {
            handle_tools_toggled(state, new_state);
            HandleResult::Continue
        }
        CommandResult::Compact => {
            handle_compact(state).await;
            HandleResult::Continue
        }
        CommandResult::ToolOutputChanged(level) => {
            handle_tool_output_changed(level);
            HandleResult::Continue
        }
        CommandResult::DebugToggled(new_state) => {
            handle_debug_toggled(new_state);
            HandleResult::Continue
        }
        CommandResult::RetrievalToggled(new_state) => {
            handle_retrieval_toggled(state, new_state);
            HandleResult::Continue
        }
        CommandResult::Context => {
            print_context_info(
                &state.session,
                &state.model_config,
                state.tools_active,
                state.agents_md.as_deref(),
                &state.settings,
                state.cli_soulless,
            );
            HandleResult::Continue
        }
        CommandResult::Retry => {
            handle_retry(state).await;
            HandleResult::Continue
        }
        CommandResult::Undo => {
            handle_undo(state);
            HandleResult::Continue
        }
        CommandResult::Search { query, limit } => {
            handle_search(state, query, limit).await;
            HandleResult::Continue
        }
        CommandResult::Reindex => {
            handle_reindex(state).await;
            HandleResult::Continue
        }
        CommandResult::FactPrune => {
            handle_fact_prune(state);
            HandleResult::Continue
        }
        CommandResult::FactAdd { content, global } => {
            handle_fact_add(state, content, global);
            HandleResult::Continue
        }
        CommandResult::FactList { global } => {
            handle_fact_list(state, global);
            HandleResult::Continue
        }
        CommandResult::FactRemove { id } => {
            handle_fact_remove(state, id);
            HandleResult::Continue
        }
        CommandResult::FactSearch {
            query,
            global,
            limit,
        } => {
            handle_fact_search(state, query, global, limit);
            HandleResult::Continue
        }
        CommandResult::TodoAdd {
            description,
            priority,
            tags,
        } => {
            handle_todo_add(description, priority, tags, &mut state.session);
            HandleResult::Continue
        }
        CommandResult::TodoList { filter } => {
            handle_todo_list(filter);
            HandleResult::Continue
        }
        CommandResult::TodoUpdate { id, status } => {
            handle_todo_update(id, status, &mut state.session);
            HandleResult::Continue
        }
        CommandResult::TodoGet { id } => {
            handle_todo_get(id);
            HandleResult::Continue
        }
        CommandResult::TodoEdit {
            id,
            description,
            priority,
            tags,
        } => {
            handle_todo_edit(id, description, priority, tags, &mut state.session);
            HandleResult::Continue
        }
        CommandResult::TodoDelete { id } => {
            handle_todo_delete(id, &mut state.session);
            HandleResult::Continue
        }
        CommandResult::TodoClearDone => {
            handle_todo_clear_done(&mut state.session);
            HandleResult::Continue
        }
        CommandResult::TodoClearAll => {
            handle_todo_clear_all(&mut state.session);
            HandleResult::Continue
        }
        CommandResult::NoteAdd {
            content,
            title,
            global,
        } => {
            handle_note_add(state, content, title, global);
            HandleResult::Continue
        }
        CommandResult::NoteList { global, page } => {
            handle_note_list(state, global, page);
            HandleResult::Continue
        }
        CommandResult::NoteShow { id } => {
            handle_note_show(state, id);
            HandleResult::Continue
        }
        CommandResult::NoteEdit { id, title, content } => {
            handle_note_edit(state, id, title, content);
            HandleResult::Continue
        }
        CommandResult::NoteDelete { id } => {
            handle_note_delete(state, id);
            HandleResult::Continue
        }
        CommandResult::NoteSearch {
            query,
            global,
            limit,
        } => {
            handle_note_search(state, query, global, limit);
            HandleResult::Continue
        }
        CommandResult::DocumentImport {
            path,
            global,
            nowait,
        } => {
            handle_document_import(state, path, global, nowait);
            HandleResult::Continue
        }
        CommandResult::DocumentList { global } => {
            handle_document_list(state, global);
            HandleResult::Continue
        }
        CommandResult::DocumentShow { id } => {
            handle_document_show(state, id);
            HandleResult::Continue
        }
        CommandResult::DocumentDelete { id } => {
            handle_document_delete(state, id);
            HandleResult::Continue
        }
        CommandResult::Skill { name, content } => {
            handle_skill_activated(state, name, content);
            HandleResult::Continue
        }
    }
}

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
            && let Err(e) = db.delete_last_content_items(&state.session.id, removed)
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

/// Handle reindex command (async)
///
/// Regenerates embeddings for ALL content in the database.
pub async fn handle_reindex(state: &mut ReplState) {
    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    let embedding_client = crate::embeddings::EmbeddingClient::new(state.ollama.clone());
    let embedding_client = Arc::new(embedding_client);

    println!("Regenerating embeddings for all content...");
    let stats = crate::embeddings::regenerate_all_embeddings(&db, &embedding_client).await;

    println!(
        "Reindex complete: {} items processed ({} failed), {} chunks processed ({} failed)",
        stats.items_processed, stats.items_failed, stats.chunks_processed, stats.chunks_failed
    );
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
    println!("\x1B[33m⏳ Compacting {} messages...\x1B[0m", msg_count);

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

                // Clear prompt_tokens in database since compaction invalidates old cumulative counts
                if let Some(db) = state.session.db.as_ref() {
                    let _ = db.clear_conversation_prompt_tokens(&state.session.id);
                }
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
        println!("Removed {} assistant message(s). Ready to retry.", removed);
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
                state
                    .session
                    .add_assistant_message(result.response, Some(result.metrics.prompt_tokens));

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
                    result.context_window,
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
                println!(
                    "\x1B[32m✓ No facts to prune. {} fact(s) remaining.\x1B[0m",
                    remaining
                );
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
/// Includes conflict detection (Phase 0.7).
pub fn handle_fact_add(state: &ReplState, content: String, global: bool) {
    use crate::facts::classify::classify_fact;
    use crate::facts::conflict::{CONFLICT_THRESHOLD, detect_conflicts, resolve_conflict};
    use crate::facts::types::{Category, Fact, MAX_FACT_CONTENT_SIZE, Scope, Source};

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
    if content.len() > MAX_FACT_CONTENT_SIZE {
        eprintln!(
            "\x1B[31m✗ Fact content exceeds {} character limit.\x1B[0m",
            MAX_FACT_CONTENT_SIZE
        );
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

    // Check for conflicts (Phase 0.7)
    let scope_for_search = if global {
        Some(Scope::Global)
    } else {
        Some(Scope::Project)
    };

    let conflicts = match db.search_facts(&content, scope_for_search, 5) {
        Ok(results) => detect_conflicts(&content, &results, CONFLICT_THRESHOLD),
        Err(_) => Vec::new(),
    };

    // Handle conflicts
    if !conflicts.is_empty() {
        let conflict = &conflicts[0];

        match resolve_conflict(conflict.clone()) {
            crate::facts::conflict::ResolutionAction::Skip => {
                println!(
                    "\x1B[33m⏭ Skipped: Duplicate fact exists (#{})\x1B[0m",
                    conflict.existing_fact.id
                );
                println!("  Existing: {}", conflict.existing_fact.content);
                println!("  New: {}", content);
                println!(
                    "\n  Use /fact remove {} first if you want to replace it.",
                    conflict.existing_fact.id
                );
                return;
            }
            crate::facts::conflict::ResolutionAction::Update => {
                // Delete old fact
                if let Err(e) = db.delete_fact(conflict.existing_fact.id) {
                    eprintln!("\x1B[31m✗ Error resolving conflict: {}\x1B[0m", e);
                    return;
                }

                // Create new fact
                let fact = match Fact::new(
                    content.clone(),
                    category,
                    scope,
                    project_id.clone(),
                    Source::User,
                ) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("\x1B[31m✗ Failed to create fact: {}\x1B[0m", e);
                        return;
                    }
                };

                // Insert new fact
                match db.insert_fact(&fact) {
                    Ok(id) => {
                        let scope_str = if global { "global" } else { "project" };
                        let category_str = match category {
                            Category::Preference => "preference",
                            Category::Fact => "fact",
                        };
                        println!(
                            "\x1B[32m✓ Updated fact #{} (scope: {}, category: {})\x1B[0m",
                            id, scope_str, category_str
                        );
                        println!("  Replaced: {}", conflict.existing_fact.content);
                        println!("  With: {}", content);
                    }
                    Err(e) => {
                        eprintln!("\x1B[31m✗ Failed to store fact: {}\x1B[0m", e);
                    }
                }
                return;
            }
            crate::facts::conflict::ResolutionAction::Add => {
                // No conflict - continue to insert below
            }
        }
    }

    // Create the fact (no conflicts)
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
            let preferences: Vec<_> = facts
                .iter()
                .filter(|f| f.category == Category::Preference)
                .collect();
            let regular_facts: Vec<_> = facts
                .iter()
                .filter(|f| f.category == Category::Fact)
                .collect();

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
        Ok(Some(fact)) => match db.delete_fact(id) {
            Ok(()) => {
                println!("\x1B[32m✓ Removed fact #{}: {}\x1B[0m", id, fact.content);
            }
            Err(e) => {
                eprintln!("\x1B[31m✗ Failed to remove fact: {}\x1B[0m", e);
            }
        },
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
            println!(
                "\x1B[36mSearch results for '{}' (scope: {}):\x1B[0m",
                query, scope_str
            );

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

/// Handle todo add command
///
/// Adds a new task to the todo list.
pub fn handle_todo_add(
    description: String,
    priority: Option<String>,
    tags: Option<String>,
    session: &mut super::session::ChatSession,
) {
    use crate::chat::todo_state::Priority;
    use crate::tools::todo;

    let priority_val = priority
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse().ok())
        .unwrap_or(Priority::Medium);

    let tags_val: Vec<String> = tags
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.split(',')
                .map(|t| t.trim().to_lowercase())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let id = {
        let state = todo::get_todo_state();
        let mut guard = state.lock().unwrap();
        guard.add_with_options(description.clone(), priority_val, tags_val.clone())
    };

    session.todos = todo::save_to_session();

    let mut msg = format!(
        "Added task {}: {} [pending] [{}]",
        id, description, priority_val
    );
    if !tags_val.is_empty() {
        msg.push_str(&format!(
            " {}",
            tags_val
                .iter()
                .map(|t| format!("#{}", t))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    println!("{}", msg);

    if !session.anonymous
        && let Err(e) = session.save_sqlite()
    {
        eprintln!("Warning: Could not save session: {}", e);
    }
}

/// Handle todo list command
///
/// Lists all tasks in the todo list, optionally filtered.
pub fn handle_todo_list(filter: Option<String>) {
    use crate::chat::todo_state::{Priority, TaskFilter, TaskStatus};
    use crate::tools::todo;

    let filter_val = filter.filter(|s| !s.is_empty());

    let task_filter = if let Some(ref f) = filter_val {
        if let Some(tag) = f.strip_prefix('#') {
            TaskFilter {
                tag: Some(tag.to_lowercase()),
                ..Default::default()
            }
        } else if let Ok(status) = f.parse::<TaskStatus>() {
            TaskFilter {
                status: Some(status),
                ..Default::default()
            }
        } else if let Ok(priority) = f.parse::<Priority>() {
            TaskFilter {
                priority: Some(priority),
                ..Default::default()
            }
        } else {
            TaskFilter {
                tag: Some(f.to_lowercase()),
                ..Default::default()
            }
        }
    } else {
        TaskFilter::default()
    };

    let state = todo::get_todo_state();
    let guard = state.lock().unwrap();
    println!("{}", guard.format_list_filtered(&task_filter));
}

/// Handle todo get command
///
/// Gets a single task by ID.
pub fn handle_todo_get(id: usize) {
    use crate::tools::todo;

    let state = todo::get_todo_state();
    let guard = state.lock().unwrap();

    match guard.get(id) {
        Some(task) => {
            let mut output = format!(
                "Task {}: {}\n  Status: {}\n  Priority: {}",
                task.id, task.description, task.status, task.priority
            );
            if !task.tags.is_empty() {
                output.push_str(&format!(
                    "\n  Tags: {}",
                    task.tags
                        .iter()
                        .map(|t| format!("#{}", t))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            println!("{}", output);
        }
        None => eprintln!("Error: Task {} not found", id),
    }
}

/// Handle todo edit command
///
/// Edits a task's description, priority, and/or tags.
pub fn handle_todo_edit(
    id: usize,
    description: Option<String>,
    priority: Option<String>,
    tags: Option<String>,
    session: &mut super::session::ChatSession,
) {
    use crate::chat::todo_state::Priority;
    use crate::tools::todo;

    // Normalize empty strings to None
    let description = description.filter(|s| !s.is_empty());
    let priority = priority.filter(|s| !s.is_empty());
    let tags = tags.filter(|s| !s.is_empty());

    if description.is_none() && priority.is_none() && tags.is_none() {
        eprintln!("Error: Provide at least one field to update (description, priority, or tags).");
        return;
    }

    let priority_val: Option<Priority> = priority.and_then(|s| s.parse().ok());
    let tags_val: Option<Vec<String>> = tags.map(|s| {
        s.split(',')
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .collect()
    });

    let state = todo::get_todo_state();
    let mut guard = state.lock().unwrap();

    match guard.edit(id, description, priority_val, tags_val) {
        Ok(()) => {
            let task = guard.get(id).unwrap();
            let mut msg = format!("Task {} updated:", id);
            msg.push_str(&format!("\n  Description: {}", task.description));
            msg.push_str(&format!("\n  Status: {}", task.status));
            msg.push_str(&format!("\n  Priority: {}", task.priority));
            if !task.tags.is_empty() {
                msg.push_str(&format!(
                    "\n  Tags: {}",
                    task.tags
                        .iter()
                        .map(|t| format!("#{}", t))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            println!("{}", msg);
            drop(guard);
            session.todos = todo::save_to_session();

            if !session.anonymous
                && let Err(e) = session.save_sqlite()
            {
                eprintln!("Warning: Could not save session: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}

/// Handle todo delete command
///
/// Deletes a specific task by ID.
pub fn handle_todo_delete(id: usize, session: &mut super::session::ChatSession) {
    use crate::tools::todo;

    let state = todo::get_todo_state();
    let mut guard = state.lock().unwrap();

    let task_desc = guard.get(id).map(|t| t.description.clone());

    match guard.delete(id) {
        Ok(()) => {
            if let Some(desc) = task_desc {
                println!("Deleted task {}: {}", id, desc);
            } else {
                println!("Deleted task {}", id);
            }
            drop(guard);
            session.todos = todo::save_to_session();

            if !session.anonymous
                && let Err(e) = session.save_sqlite()
            {
                eprintln!("Warning: Could not save session: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}

/// Handle todo update command
///
/// Updates the status of a task.
pub fn handle_todo_update(id: usize, status: String, session: &mut super::session::ChatSession) {
    use crate::chat::todo_state::TaskStatus;
    use crate::tools::todo;

    let new_status: TaskStatus = match status.parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    let state = todo::get_todo_state();
    let mut guard = state.lock().unwrap();

    match guard.update_status(id, new_status) {
        Ok(()) => {
            println!("Task {} marked as {}", id, new_status);
            drop(guard);
            session.todos = todo::save_to_session();

            if !session.anonymous
                && let Err(e) = session.save_sqlite()
            {
                eprintln!("Warning: Could not save session: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}

/// Handle todo clear-done command
///
/// Clears all completed tasks from the list.
pub fn handle_todo_clear_done(session: &mut super::session::ChatSession) {
    use crate::tools::todo;

    let state = todo::get_todo_state();
    let mut guard = state.lock().unwrap();
    let removed = guard.clear_done();

    if removed == 0 {
        println!("No completed tasks to remove.");
    } else if removed == 1 {
        println!("Removed 1 completed task.");
    } else {
        println!("Removed {} completed tasks.", removed);
    }

    drop(guard);
    session.todos = todo::save_to_session();

    if !session.anonymous
        && let Err(e) = session.save_sqlite()
    {
        eprintln!("Warning: Could not save session: {}", e);
    }
}

/// Handle todo clear-all command
///
/// Clears all tasks from the list.
pub fn handle_todo_clear_all(session: &mut super::session::ChatSession) {
    use crate::tools::todo;

    let state = todo::get_todo_state();
    let mut guard = state.lock().unwrap();
    let count = guard.clear_all();

    if count == 0 {
        println!("The task list was already empty.");
    } else if count == 1 {
        println!("Cleared 1 task from the list.");
    } else {
        println!("Cleared {} tasks from the list.", count);
    }

    drop(guard);
    session.todos = todo::save_to_session();

    if !session.anonymous
        && let Err(e) = session.save_sqlite()
    {
        eprintln!("Warning: Could not save session: {}", e);
    }
}

/// Handle model switch command.
///
/// Uses the centralized `model_switch::switch_model` function to switch
/// to a new model and updates the REPL state accordingly.
pub async fn handle_model_switch(
    state: &mut ReplState,
    model_name: &str,
    current_capabilities: &ModelCapabilities,
) -> Result<(), String> {
    use super::model_switch::switch_model;

    let result = switch_model(
        model_name,
        &state.ollama,
        current_capabilities,
        state.session.think,
        state.tools_active,
    )
    .await?;

    state.current_model_name = result.model_name.clone();
    state.session.set_model(result.model_name.clone());
    state.model_config = result.model_config;
    state.capabilities = result.capabilities.clone();
    state.session.think = result.think_active;
    state.tools_active = result.tools_active;

    for warning in result.warnings {
        eprintln!("{}", warning);
    }

    println!("Switched to model: {}", state.model_config.model_id);

    Ok(())
}

/// Print context information about the current session.
///
/// Shows token usage, message count, and context window utilization.
pub fn print_context_info(
    session: &ChatSession,
    model_config: &ModelConfig,
    tools_enabled: bool,
    agents_md: Option<&str>,
    settings: &Settings,
    soulless: bool,
) {
    use crate::prompts::builder::{PromptConfig, PromptType, build_system_prompt};
    use crate::tools::get_available_tool_names;

    let blacklist_set = settings.blacklist_set();

    let prompt_type = if tools_enabled {
        PromptType::ToolUser
    } else {
        PromptType::Default
    };

    let system_prompt = build_system_prompt(
        PromptConfig::new(prompt_type)
            .with_model_id(Some(&model_config.model_id))
            .with_blacklist(Some(&blacklist_set))
            .with_agents_md(agents_md)
            .with_tools(tools_enabled)
            .with_retrieval(session.retrieval_enabled)
            .with_soulless(soulless),
    );

    let history_messages = session.get_messages_for_llm(&system_prompt);
    let context_window = model_config.num_ctx as usize;

    let tool_count = if tools_enabled {
        get_available_tool_names(settings).len()
    } else {
        0
    };

    let tools_tokens = if tools_enabled && tool_count > 0 {
        tool_count * TOKENS_PER_TOOL
    } else {
        0
    };

    let real_history_tokens = session.history_real_tokens();
    let real_tokens_opt = if real_history_tokens > 0 {
        Some(real_history_tokens)
    } else {
        None
    };

    let metrics = calculate_context_metrics(
        &history_messages,
        context_window,
        &system_prompt,
        tools_tokens,
        real_tokens_opt,
    );

    let context_window_k = context_window / 1024;
    let usage_percent = metrics.utilization * 100.0;

    let bar_width = 20;
    let filled = (usage_percent.min(100.0) as usize * bar_width) / 100;
    let empty = bar_width - filled;

    // Calculate thresholds based on percentage of context window
    // OK: remaining > 25% of context (usage < 75%)
    // MODERATE: remaining > 12% of context (usage < 88%)
    // CRITICAL: remaining <= 12% of context (usage >= 88%)
    let remaining = context_window.saturating_sub(metrics.total_tokens);
    let (pre_tool, compaction, _, _) =
        crate::context_overflow::calculate_thresholds(context_window);

    let (color_code, reset_code, status_text) = if remaining > pre_tool {
        ("\x1B[32m", "\x1B[0m", "OK")
    } else if remaining > compaction {
        ("\x1B[33m", "\x1B[0m", "MODERATE")
    } else {
        ("\x1B[31m", "\x1B[0m", "CRITICAL")
    };

    println!();
    println!("Context Information:");
    println!(
        "  Model:          {} ({}K context)",
        model_config.model_id, context_window_k
    );
    println!();
    println!("  Context Utilization:");
    println!(
        "    {}{}{}{} {:.1}%{}",
        color_code,
        "█".repeat(filled),
        "░".repeat(empty),
        color_code,
        usage_percent,
        reset_code
    );
    println!(
        "    {}{} / {} tokens{}\x1B[0m",
        color_code, metrics.total_tokens, context_window, reset_code
    );
    println!();
    println!("  Status: {}", status_text);
    println!();
    println!("  Token Breakdown:");
    println!("    System prompt:    ~{} tokens", metrics.system_tokens);
    if tools_enabled && tool_count > 0 {
        println!(
            "    Tool definitions: ~{} tokens ({} tools)",
            metrics.tools_tokens, tool_count
        );
    }

    let active_messages = if session.has_compacted_messages() {
        session.messages.len() - session.messages_sent_to_llm
    } else {
        session.messages.len()
    };

    if metrics.total_tokens > 0 {
        println!("    History:          ~{} tokens", metrics.history_tokens);
        if session.has_compacted_messages() {
            println!(
                "                      ({} active messages + summary)",
                active_messages
            );
        } else {
            println!("                      ({} messages)", active_messages);
        }
    } else {
        if session.has_compacted_messages() {
            println!(
                "    Summary:          ~{} tokens",
                estimate_tokens(session.compacted_summary.as_deref().unwrap_or("")) + 4
            );
            println!(
                "    Conversation:     ~{} tokens ({} active messages)",
                metrics.history_tokens, active_messages
            );
        } else {
            println!(
                "    Conversation:     ~{} tokens ({} messages)",
                metrics.history_tokens, active_messages
            );
        }
    }

    println!("    {}", "─".repeat(40));
    println!("    Total used:       ~{} tokens", metrics.total_tokens);
    println!("    Available:        ~{} tokens", metrics.available());
    println!();

    if session.has_compacted_messages() {
        println!("  Session:");
        println!(
            "    Compacted:        {} messages summarized",
            session.compacted_message_count()
        );
        println!("    Active:           {} messages", active_messages);
        println!("    Total:            {} messages", session.messages.len());
    } else {
        println!("  Session:");
        println!("    Total:            {} messages", session.messages.len());
    }
    println!();
}

/// Handle note add command
///
/// Adds a new note with the given content.
/// Generates embedding asynchronously for semantic search.
pub fn handle_note_add(state: &ReplState, content: String, title: Option<String>, global: bool) {
    use crate::content::{ContentScope, ContentSource, MAX_NOTE_CONTENT_SIZE, Note};

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot add notes in anonymous mode.");
        return;
    }

    if content.len() > MAX_NOTE_CONTENT_SIZE {
        eprintln!(
            "\x1B[31m✗ Note content exceeds {} character limit.\x1B[0m",
            MAX_NOTE_CONTENT_SIZE
        );
        println!("  Current length: {} characters", content.len());
        println!("  Use shorter content or split into multiple notes.");
        return;
    }

    let scope = if global {
        ContentScope::Global
    } else {
        ContentScope::Project
    };

    let project_id = if global {
        None
    } else {
        state.session.project_id.clone()
    };

    let note = match Note::new(
        content.clone(),
        scope,
        project_id.clone(),
        ContentSource::User,
        title.clone(),
    ) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to create note: {}\x1B[0m", e);
            return;
        }
    };

    match db.insert_note(&note) {
        Ok(id) => {
            let scope_str = if global { "global" } else { "project" };
            if let Some(t) = &title {
                println!(
                    "\x1B[32m✓ Added note #{} (scope: {}): {}\x1B[0m",
                    id, scope_str, t
                );
            } else {
                println!("\x1B[32m✓ Added note #{} (scope: {})\x1B[0m", id, scope_str);
            }

            // Print content preview with │ prefix on every line
            let lines: Vec<&str> = content.lines().collect();
            let max_lines = 5;
            for line in lines.iter().take(max_lines) {
                let truncated = crate::chat::view::truncate_str(line, 76);
                println!("  │ {}", truncated);
            }

            // Show indication if content was truncated
            if lines.len() > max_lines {
                println!("  │ ... ({} more lines)", lines.len() - max_lines);
            }

            // Generate embedding asynchronously (like messages in session.rs)
            if let Some(ref embedding_client) = state.embedding_client {
                let client = Arc::clone(embedding_client);
                let db_clone = Arc::clone(&db);
                let pid = project_id.clone();
                let note_content = note.content.clone();

                tokio::spawn(async move {
                    // Use fallback for oversized content
                    let ctx = EmbedItemContext::new(
                        &note_content,
                        id,
                        "note",
                        None, // notes don't have conversation_id
                        pid.as_deref(),
                    );
                    if let Err(e) =
                        embed_item_with_fallback(ctx, &db_clone, &client, DEFAULT_CONTEXT_LENGTH)
                            .await
                    {
                        eprintln!("Warning: Failed to generate embedding for note: {}", e);
                    }
                });
            }
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to store note: {}\x1B[0m", e);
        }
    }
}

/// Handle note list command
///
/// Lists notes for the current scope with pagination (8 per page).
pub fn handle_note_list(state: &ReplState, global: bool, page: Option<usize>) {
    use crate::content::ContentScope;
    use chrono::Utc;

    const NOTES_PER_PAGE: usize = 8;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot list notes in anonymous mode.");
        return;
    }

    let scope = if global {
        Some(ContentScope::Global)
    } else {
        Some(ContentScope::Project)
    };

    let project_id = if global {
        None
    } else {
        state.session.project_id.clone()
    };

    match db.list_notes(scope, project_id.as_deref()) {
        Ok(notes) => {
            let scope_str = if global { "global" } else { "project" };

            if notes.is_empty() {
                println!("\x1B[36mNotes ({scope_str}):\x1B[0m");
                println!("  No notes stored.");
                return;
            }

            let total_notes = notes.len();
            let total_pages = total_notes.div_ceil(NOTES_PER_PAGE);

            // Validate page number
            let requested_page = page.unwrap_or(1);
            if requested_page < 1 {
                eprintln!("\x1B[31mPage must be >= 1. Use /note list 1 for first page.\x1B[0m");
                return;
            }
            if requested_page > total_pages {
                eprintln!(
                    "\x1B[31mPage {} does not exist. Total pages: {}. Use /note list {}.\x1B[0m",
                    requested_page, total_pages, total_pages
                );
                return;
            }

            let current_page = requested_page;
            let start_idx = (current_page - 1) * NOTES_PER_PAGE;
            let end_idx = start_idx + NOTES_PER_PAGE.min(total_notes - start_idx);

            println!(
                "\x1B[36mNotes ({scope_str}) - Page {} of {}:\x1B[0m",
                current_page, total_pages
            );

            for note in &notes[start_idx..end_idx] {
                let age_days = (Utc::now() - note.created_at).num_days();
                if let Some(t) = &note.title {
                    println!(
                        "  \x1B[33m#{} {} \x1B[90m({}d)\x1B[0m",
                        note.id, t, age_days
                    );
                } else {
                    println!(
                        "  \x1B[33m#{}\x1B[0m \x1B[90m({}d)\x1B[0m",
                        note.id, age_days
                    );
                }
                // Get first line only for preview, truncated if too long
                let first_line = note.content.lines().next().unwrap_or(&note.content);
                let preview = crate::chat::view::truncate_str(first_line, 76);
                println!("  │ {}", preview);
            }

            println!(
                "\n  \x1B[90mTotal: {} note(s), Page {}/{}\x1B[0m",
                total_notes, current_page, total_pages
            );
            if total_pages > 1 {
                println!(
                    "  \x1B[90mUse /note list {} to see page {}, or /note list --global {} for global\x1B[0m",
                    current_page + 1,
                    current_page + 1,
                    current_page + 1
                );
            }
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to list notes: {}\x1B[0m", e);
        }
    }
}

/// Handle note show command
///
/// Shows a single note by ID.
pub fn handle_note_show(state: &ReplState, id: i64) {
    use crate::content::{ContentScope, ContentSource};
    use chrono::Utc;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot show notes in anonymous mode.");
        return;
    }

    match db.get_note(id) {
        Ok(Some(note)) => {
            let scope_str = match note.scope {
                ContentScope::Global => "global",
                ContentScope::Project => "project",
            };
            let source_str = match note.source {
                ContentSource::User => "user",
                ContentSource::Llm => "llm",
            };
            let age_days = (Utc::now() - note.created_at).num_days();

            // Build header (rendered as markdown)
            let mut header = format!("## Note #{}\n\n", note.id);
            if let Some(t) = &note.title {
                header.push_str(&format!("**Title:** {}\n\n", t));
            }
            header.push_str(&format!(
                "**Scope:** {} | **Source:** {} | **Age:** {}d\n\n",
                scope_str, source_str, age_days
            ));
            if let Some(pid) = &note.project_id {
                header.push_str(&format!("**Project:** {}\n\n", pid));
            }
            header.push_str("---\n");

            // Print header with markdown
            crate::markdown::print_markdown(&header);

            // Print content as markdown (no prefix, let termimad handle it)
            crate::markdown::print_markdown(&note.content);
        }
        Ok(None) => {
            eprintln!("\x1B[31m✗ Note #{} not found.\x1B[0m", id);
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to retrieve note: {}\x1B[0m", e);
        }
    }
}

/// Handle note edit command
///
/// Edits a note's title and/or content.
pub fn handle_note_edit(
    state: &ReplState,
    id: i64,
    title: Option<String>,
    content: Option<String>,
) {
    use crate::content::MAX_NOTE_CONTENT_SIZE;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot edit notes in anonymous mode.");
        return;
    }

    if let Some(ref c) = content
        && c.len() > MAX_NOTE_CONTENT_SIZE
    {
        eprintln!(
            "\x1B[31m✗ Note content exceeds {} character limit.\x1B[0m",
            MAX_NOTE_CONTENT_SIZE
        );
        println!("  Current length: {} characters", c.len());
        return;
    }

    match db.get_note(id) {
        Ok(Some(_)) => match db.update_note(id, title.as_deref(), content.as_deref()) {
            Ok(()) => {
                println!("\x1B[32m✓ Updated note #{}\x1B[0m", id);
                if let Some(t) = &title {
                    println!("  Title: {}", t);
                }
                if let Some(c) = &content {
                    println!("  Content: {}", crate::chat::view::truncate_str(c, 80));
                }
            }
            Err(e) => {
                eprintln!("\x1B[31m✗ Failed to update note: {}\x1B[0m", e);
            }
        },
        Ok(None) => {
            eprintln!("\x1B[31m✗ Note #{} not found.\x1B[0m", id);
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to retrieve note: {}\x1B[0m", e);
        }
    }
}

/// Handle note delete command
///
/// Deletes a note by ID.
pub fn handle_note_delete(state: &ReplState, id: i64) {
    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot delete notes in anonymous mode.");
        return;
    }

    match db.get_note(id) {
        Ok(Some(note)) => match db.delete_note(id) {
            Ok(()) => {
                if let Some(t) = &note.title {
                    println!("\x1B[32m✓ Deleted note #{}: {}\x1B[0m", id, t);
                } else {
                    println!("\x1B[32m✓ Deleted note #{}\x1B[0m", id);
                }
            }
            Err(e) => {
                eprintln!("\x1B[31m✗ Failed to delete note: {}\x1B[0m", e);
            }
        },
        Ok(None) => {
            eprintln!("\x1B[31m✗ Note #{} not found.\x1B[0m", id);
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to retrieve note: {}\x1B[0m", e);
        }
    }
}

/// Handle note search command
///
/// Searches notes by keyword.
pub fn handle_note_search(state: &ReplState, query: String, global: bool, limit: usize) {
    use crate::content::ContentScope;
    use chrono::Utc;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot search notes in anonymous mode.");
        return;
    }

    let scope = if global {
        Some(ContentScope::Global)
    } else {
        Some(ContentScope::Project)
    };

    let project_id = if global {
        None
    } else {
        state.session.project_id.clone()
    };

    match db.search_notes_keyword(&query, scope, project_id.as_deref(), limit) {
        Ok(results) => {
            let scope_str = if global { "global" } else { "project" };
            println!(
                "\x1B[36mSearch results for \"{}\" ({scope_str}):\x1B[0m",
                query
            );

            if results.is_empty() {
                println!("  No notes found.");
                return;
            }

            for result in &results {
                let age_days = (Utc::now() - result.item.created_at).num_days();
                if let Some(t) = &result.item.title {
                    println!(
                        "  \x1B[33m#{} {} \x1B[90m(score: {:.2}, {}d)\x1B[0m",
                        result.item.id, t, result.score, age_days
                    );
                } else {
                    println!(
                        "  \x1B[33m#{}\x1B[0m \x1B[90m(score: {:.2}, {}d)\x1B[0m",
                        result.item.id, result.score, age_days
                    );
                }
                let preview = crate::chat::view::truncate_str(&result.item.content, 80);
                println!("    {}", preview);
            }

            println!("\n  \x1B[90mFound: {} note(s)\x1B[0m", results.len());
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Search failed: {}\x1B[0m", e);
        }
    }
}

// ============================================================
// Document Command Handlers
// ============================================================

/// Handle document import command
#[cfg(feature = "document-tools")]
pub fn handle_document_import(state: &ReplState, path: String, global: bool, nowait: bool) {
    use crate::content::{ContentScope, Document, FileType, MAX_DOCUMENT_SIZE, detect_file_type};
    use crate::utils::expand_tilde_path;
    use std::fs;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot import documents in anonymous mode.");
        return;
    }

    let file_path = expand_tilde_path(&path);
    if !file_path.exists() {
        eprintln!("\x1B[31m✗ File not found: {}\x1B[0m", path);
        return;
    }

    let metadata = match fs::metadata(&file_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("\x1B[31m✗ Cannot read file metadata: {}\x1B[0m", e);
            return;
        }
    };

    if metadata.len() > MAX_DOCUMENT_SIZE as u64 {
        eprintln!(
            "\x1B[31m✗ File exceeds maximum size of {} bytes (got {} bytes).\x1B[0m",
            MAX_DOCUMENT_SIZE,
            metadata.len()
        );
        println!("  Consider splitting the document into smaller files.");
        return;
    }

    let file_type = match detect_file_type(&file_path) {
        Ok(ft) => ft,
        Err(e) => {
            eprintln!("\x1B[31m✗ {}\x1B[0m", e);
            return;
        }
    };

    #[cfg(not(feature = "skills-tools"))]
    if file_type.requires_skills() {
        eprintln!(
            "\x1B[31m✗ Importing '{}' files requires the 'skills-tools' feature.\x1B[0m",
            file_type.extension()
        );
        println!("  Recompile with: cargo build --features skills-tools");
        println!("  Alternatively, convert to TXT/MD/ORG format first.");
        return;
    }

    let content = match file_type {
        FileType::Txt | FileType::Md | FileType::Org => match fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("\x1B[31m✗ Cannot read file: {}\x1B[0m", e);
                return;
            }
        },
        FileType::Pdf | FileType::Epub => {
            #[cfg(feature = "skills-tools")]
            {
                use std::process::Command;

                let (program, args) = match file_type {
                    FileType::Pdf => (
                        "pdftotext",
                        vec![file_path.to_string_lossy().to_string(), "-".to_string()],
                    ),
                    FileType::Epub => (
                        "epub2txt",
                        vec![file_path.to_string_lossy().to_string(), "-".to_string()],
                    ),
                    _ => unreachable!(),
                };

                let output = Command::new(program).args(&args).output();

                match output {
                    Ok(output) => {
                        if output.status.success() {
                            match String::from_utf8(output.stdout) {
                                Ok(c) => c,
                                Err(e) => {
                                    eprintln!(
                                        "\x1B[31m✗ Failed to parse output as UTF-8: {}\x1B[0m",
                                        e
                                    );
                                    return;
                                }
                            }
                        } else {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            eprintln!("\x1B[31m✗ {} failed: {}\x1B[0m", program, stderr.trim());
                            return;
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "\x1B[31m✗ Could not run '{}' - {}. Install with your package manager.\x1B[0m",
                            program, e
                        );
                        return;
                    }
                }
            }
            #[cfg(not(feature = "skills-tools"))]
            {
                unreachable!("Already checked above");
            }
        }
    };

    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&path)
        .to_string();

    let title = Document::extract_title(&content, &filename);

    let scope = if global {
        ContentScope::Global
    } else {
        ContentScope::Project
    };

    let project_id = if global {
        None
    } else {
        state.session.project_id.clone()
    };

    let document = match Document::new(
        content.clone(),
        title.clone(),
        filename.clone(),
        file_type,
        scope,
        project_id.clone(),
    ) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to create document: {}\x1B[0m", e);
            return;
        }
    };

    match db.insert_document(&document) {
        Ok(id) => {
            let scope_str = if global { "global" } else { "project" };
            println!(
                "\x1B[32m✓ Imported document #{} (scope: {}): {}\x1B[0m",
                id, scope_str, title
            );
            println!("  File: {}", filename);
            println!("  Words: {}", document.word_count);
            println!("  Type: {}", file_type.extension());

            if let Some(ref embedding_client) = state.embedding_client {
                if nowait {
                    // Async embedding in background
                    println!("  Indexing in background...");
                    let client = Arc::clone(embedding_client);
                    let db_clone = Arc::clone(&db);
                    let pid = project_id.clone();
                    let doc_content = document.content.clone();

                    tokio::spawn(async move {
                        let ctx = EmbedItemContext::new(
                            &doc_content,
                            id,
                            "document",
                            None,
                            pid.as_deref(),
                        );
                        if let Err(e) = embed_item_with_fallback(
                            ctx,
                            &db_clone,
                            &client,
                            DEFAULT_CONTEXT_LENGTH,
                        )
                        .await
                        {
                            eprintln!("Warning: Failed to generate embedding for document: {}", e);
                        }
                    });
                } else {
                    // Synchronous embedding with progress
                    println!("  Indexing document...");

                    let ctx = EmbedItemContext::new(
                        &document.content,
                        id,
                        "document",
                        None,
                        project_id.as_deref(),
                    );

                    match tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            embed_item_with_fallback(
                                ctx,
                                &db,
                                embedding_client,
                                DEFAULT_CONTEXT_LENGTH,
                            )
                            .await
                        })
                    }) {
                        Ok(result) => {
                            let chunks = result.chunks_created.max(1);
                            println!(
                                "  ✓ Document indexed ({} chunk{})",
                                chunks,
                                if chunks > 1 { "s" } else { "" }
                            );
                        }
                        Err(e) => {
                            eprintln!(
                                "  \x1B[33m⚠ Warning: Failed to index document: {}\x1B[0m",
                                e
                            );
                            println!("  Run '/reindex' to regenerate embeddings.");
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to store document: {}\x1B[0m", e);
        }
    }
}

#[cfg(not(feature = "document-tools"))]
pub fn handle_document_import(_state: &ReplState, _path: String, _global: bool, _nowait: bool) {
    eprintln!("Error: Document import requires 'document-tools' feature.");
    println!("  Recompile with: cargo build --features document-tools");
}

/// Handle document list command
#[cfg(feature = "document-tools")]
pub fn handle_document_list(state: &ReplState, global: bool) {
    use crate::content::ContentScope;
    use chrono::Utc;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot list documents in anonymous mode.");
        return;
    }

    let scope = if global {
        Some(ContentScope::Global)
    } else {
        Some(ContentScope::Project)
    };

    let project_id = if global {
        None
    } else {
        state.session.project_id.clone()
    };

    match db.list_documents(scope, project_id.as_deref()) {
        Ok(documents) => {
            let scope_str = if global { "global" } else { "project" };
            println!("\x1B[36mDocuments (scope: {}):\x1B[0m", scope_str);

            if documents.is_empty() {
                println!("  No documents found.");
                return;
            }

            for doc in &documents {
                let age_days = (Utc::now() - doc.created_at).num_days();
                println!(
                    "  \x1B[33m#{} {} \x1B[90m({}, {} words, {}d)\x1B[0m",
                    doc.id,
                    doc.title,
                    doc.file_type.extension(),
                    doc.word_count,
                    age_days
                );
            }

            println!("\n  \x1B[90mFound: {} document(s)\x1B[0m", documents.len());
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to list documents: {}\x1B[0m", e);
        }
    }
}

#[cfg(not(feature = "document-tools"))]
pub fn handle_document_list(_state: &ReplState, _global: bool) {
    eprintln!("Error: Document listing requires 'document-tools' feature.");
    println!("  Recompile with: cargo build --features document-tools");
}

/// Handle document show command
#[cfg(feature = "document-tools")]
pub fn handle_document_show(state: &ReplState, id: i64) {
    use chrono::Utc;

    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot show document in anonymous mode.");
        return;
    }

    match db.get_document(id) {
        Ok(Some(doc)) => {
            let age_days = (Utc::now() - doc.created_at).num_days();
            let scope_str = match doc.scope {
                crate::content::ContentScope::Global => "global".to_string(),
                crate::content::ContentScope::Project => {
                    doc.project_id.as_deref().unwrap_or("project").to_string()
                }
            };
            println!("\x1B[36mDocument #{}:\x1B[0m", doc.id);
            println!("  \x1B[1m{}\x1B[0m", doc.title);
            println!(
                "  \x1B[90mFile: {} | Type: {} | Words: {} | Age: {}d | Scope: {}\x1B[0m",
                doc.filename,
                doc.file_type.extension(),
                doc.word_count,
                age_days,
                scope_str
            );
            println!();
            println!("{}", doc.content);
        }
        Ok(None) => {
            eprintln!("\x1B[31m✗ Document #{} not found.\x1B[0m", id);
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to retrieve document: {}\x1B[0m", e);
        }
    }
}

#[cfg(not(feature = "document-tools"))]
pub fn handle_document_show(_state: &ReplState, _id: i64) {
    eprintln!("Error: Document viewing requires 'document-tools' feature.");
    println!("  Recompile with: cargo build --features document-tools");
}

/// Handle document delete command
#[cfg(feature = "document-tools")]
pub fn handle_document_delete(state: &ReplState, id: i64) {
    let db = match &state.db {
        Some(d) => Arc::clone(d),
        None => {
            eprintln!("Error: Database not initialized. Run chat without --anonymous.");
            return;
        }
    };

    if state.session.anonymous {
        eprintln!("Error: Cannot delete document in anonymous mode.");
        return;
    }

    match db.get_document(id) {
        Ok(Some(doc)) => match db.delete_document(id) {
            Ok(()) => {
                println!("\x1B[32m✓ Deleted document #{}: {}\x1B[0m", id, doc.title);
            }
            Err(e) => {
                eprintln!("\x1B[31m✗ Failed to delete document: {}\x1B[0m", e);
            }
        },
        Ok(None) => {
            eprintln!("\x1B[31m✗ Document #{} not found.\x1B[0m", id);
        }
        Err(e) => {
            eprintln!("\x1B[31m✗ Failed to retrieve document: {}\x1B[0m", e);
        }
    }
}

#[cfg(not(feature = "document-tools"))]
pub fn handle_document_delete(_state: &ReplState, _id: i64) {
    eprintln!("Error: Document deletion requires 'document-tools' feature.");
    println!("  Recompile with: cargo build --features document-tools");
}

/// Handle skill activation command
///
/// Activates a skill for the current session by setting it in the session state.
/// The skill content will be injected into the system prompt.
pub fn handle_skill_activated(state: &mut ReplState, name: String, content: String) {
    // Store the active skill in session
    state.session.active_skill = Some(super::session::ActiveSkill {
        name: name.clone(),
        content,
    });

    println!(
        "\x1B[32m✓ Skill '{}' activated for this session.\x1B[0m",
        name
    );
    println!(
        "\x1B[90mSkill instructions will be followed when relevant to the conversation.\x1B[0m"
    );
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
