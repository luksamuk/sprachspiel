//! Core chat functionality
//!
//! This module provides `ChatCore`, encapsulating the core logic for sending
//! messages and managing chat conversations. This enables:
//!
//! 1. Clear separation of concerns (I/O vs business logic)
//! 2. Easier testing (core logic in isolation)
//! 3. Future TUI compatibility (stateless processing)
//!
//! # Architecture
//!
//! ```text
//! Layer 4 (Core): core.rs
//!     ↓ uses
//! Layer 3 (State): repl_state.rs
//! Layer 1 (Session): session.rs
//! Layer 0 (Base): capabilities, config
//! ```

use std::sync::Arc;

use ollama_rs::generation::chat::ChatMessage;

use crate::config::ModelConfig;
use crate::context_overflow::{
    MAX_SUMMARY_TOKENS, check_context_overflow, needs_buffered_compaction,
};
use crate::debug_tools::log_debug;
use crate::facts::prompt::build_facts_section;
use crate::prompts::builder::{
    PromptConfig, PromptType, build_compaction_prompt, build_continuation_prompt,
    build_system_prompt,
};
use crate::retrieval::{RetrievalConfig, build_context, update_retrieval_time};
use crate::settings::Settings;
use crate::spinner::{create_spinner, finish_spinner};
use crate::tokens::estimate_tokens;
use crate::tools::{get_available_tool_names, register_tools};
use crate::utils::truncate_to_budget;

use super::coordinator::{
    MAX_RETRIES, classify_ollama_error, format_recovery_message, is_ollama_error_recoverable,
};
use super::custom_coordinator::CustomCoordinator;
use super::session::ChatSession;
use super::thinking::{display_thinking, strip_thinking_tags};
use super::{ContinuationTag, parse_continuation_tag};

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Token usage metrics for a chat response
#[derive(Debug, Clone, Default)]
pub struct TokenMetrics {
    pub prompt_tokens: u64,
    pub response_tokens: u64,
    pub total_tokens: u64,
}

/// Result of sending a message to the LLM
///
/// Contains the response, any pre-tool content (for tool calls),
/// token metrics, and continuation information.
pub struct SendMessageResult {
    pub response: String,
    pub pre_tool_content: Option<String>,
    pub pre_tool_thinking: Option<String>,
    pub metrics: TokenMetrics,
    pub context_window: usize,
    pub system_prompt: String,
    /// Parsed continuation tag if LLM requested to continue after compaction
    pub continuation_needed: Option<ContinuationTag>,
}

/// Build system prompt for the session
#[allow(clippy::too_many_arguments)]
pub fn build_session_system_prompt(
    session: &ChatSession,
    tools_enabled: bool,
    cli_code: bool,
    cli_soulless: bool,
    model_config: &ModelConfig,
    blacklist_set: &std::collections::HashSet<&str>,
    agents_md: Option<&str>,
    facts_section: Option<&str>,
    todos_section: Option<&str>,
) -> String {
    if let Some(ref custom_prompt) = session.system_prompt {
        return custom_prompt.clone();
    }

    let prompt_type = if cli_code && tools_enabled {
        PromptType::CodeWithTools
    } else if cli_code {
        PromptType::Code
    } else if tools_enabled {
        PromptType::ToolUser
    } else {
        PromptType::Default
    };

    let ctx_window = model_config.num_ctx as usize;
    let ctx_status = check_context_overflow(session, "", ctx_window);

    build_system_prompt(
        PromptConfig::new(prompt_type)
            .with_model_id(Some(&model_config.model_id))
            .with_blacklist(Some(blacklist_set))
            .with_agents_md(agents_md)
            .with_tools(tools_enabled)
            .with_retrieval(session.retrieval_enabled && !cli_code)
            .with_soulless(cli_soulless)
            .with_context_status(if ctx_status.needs_compaction() {
                Some(ctx_status.clone())
            } else {
                None
            })
            .with_facts_section(facts_section)
            .with_anonymous(session.anonymous)
            .with_todos(todos_section),
    )
}

/// Setup coordinator with optional tools
#[allow(clippy::too_many_arguments)]
pub fn setup_coordinator(
    ollama: ollama_rs::Ollama,
    model_config: &ModelConfig,
    model_options: ollama_rs::models::ModelOptions,
    think_enabled: bool,
    use_debug: bool,
    tools_enabled: bool,
    settings: &Settings,
    system_prompt: String,
    real_history_tokens: Option<usize>,
) -> CustomCoordinator<Vec<ChatMessage>> {
    let coordinator = crate::query::ChatContext {
        ollama,
        model_id: model_config.model_id.clone(),
        model_options,
        use_think: think_enabled,
        use_debug,
        use_plain: false,
        context_window: Some(model_config.num_ctx as usize),
        system_prompt: Some(system_prompt),
    }
    .build_coordinator();

    let mut coordinator = coordinator;

    // Set real token count for accurate overflow detection
    if let Some(tokens) = real_history_tokens {
        coordinator = coordinator.real_history_tokens(tokens);
    }

    if tools_enabled {
        let (coord_new, tool_count) = register_tools(coordinator, settings, use_debug);
        coordinator = coord_new;
        if use_debug {
            log_debug(&format!("{} tools active", tool_count));
        }
    }
    coordinator
}

/// Prepare messages with retrieval and optional continuation
#[allow(clippy::too_many_arguments)]
pub async fn prepare_messages(
    session: &mut ChatSession,
    db: Option<&Arc<crate::db::Database>>,
    embedding_client: Option<&Arc<crate::embeddings::EmbeddingClient>>,
    user_input: &str,
    system_prompt: &str,
    use_debug: bool,
    coordinator: &mut CustomCoordinator<Vec<ChatMessage>>,
    continuation_tag: Option<&ContinuationTag>,
) -> Vec<ChatMessage> {
    let retrieval_config = if session.retrieval_enabled {
        RetrievalConfig::default()
    } else {
        RetrievalConfig {
            enabled: false,
            ..RetrievalConfig::default()
        }
    };

    let context_result = build_context(
        session,
        db,
        embedding_client,
        user_input,
        system_prompt,
        &retrieval_config,
        use_debug,
    )
    .await;

    if context_result.retrieval_performed {
        update_retrieval_time(session);
        if use_debug {
            log_debug(&format!(
                "Retrieved {} relevant messages",
                context_result.retrieved_count
            ));
        }
    }

    let mut messages = context_result.messages;
    messages.push(ChatMessage::user(user_input.to_string()));

    if let Some(tag) = continuation_tag {
        let continuation_prompt = build_continuation_prompt(&tag.paused_at, &tag.next_step);
        coordinator.push_ephemeral(ChatMessage::user(continuation_prompt));
        if use_debug {
            log_debug("Injected continuation prompt as ephemeral message");
        }
    }

    messages
}

/// Process chat response into SendMessageResult
pub fn process_chat_response(
    response: ollama_rs::generation::chat::ChatMessageResponse,
    think_enabled: bool,
    coordinator: &mut CustomCoordinator<Vec<ChatMessage>>,
    context_window: usize,
    system_prompt: String,
) -> SendMessageResult {
    let content = response.message.content.clone();

    let metrics = if let Some(ref final_data) = response.final_data {
        TokenMetrics {
            prompt_tokens: final_data.prompt_eval_count,
            response_tokens: final_data.eval_count,
            total_tokens: final_data.prompt_eval_count + final_data.eval_count,
        }
    } else {
        TokenMetrics::default()
    };

    if think_enabled {
        display_thinking(&content, response.message.thinking.as_ref(), true);
    }

    let display_content = strip_thinking_tags(&content);
    crate::markdown::print_markdown(&display_content);

    let pre_tool = coordinator.take_pre_tool_content();
    let (pre_tool_content, pre_tool_thinking) = match pre_tool {
        Some(ptc) => (Some(ptc.content), ptc.thinking),
        None => (None, None),
    };

    let (cleaned_response, continuation_needed) = parse_continuation_tag(&display_content);

    if continuation_needed.is_some() {
        eprint!("\x1B[2K\r");
        crate::markdown::print_markdown(&cleaned_response);
    }

    SendMessageResult {
        response: cleaned_response,
        pre_tool_content,
        pre_tool_thinking,
        metrics,
        context_window,
        system_prompt,
        continuation_needed,
    }
}

/// Send a message to the LLM and process the response
///
/// This is the core function for chat interaction, handling:
/// - System prompt building
/// - Context overflow checking
/// - Tool registration
/// - Message preparation with retrieval
/// - Retry logic for recoverable errors
/// - Response processing
#[allow(clippy::too_many_arguments)]
pub async fn send_message(
    ollama: &ollama_rs::Ollama,
    model_config: &ModelConfig,
    session: &mut ChatSession,
    user_input: &str,
    tools_enabled: bool,
    think_enabled: bool,
    cli_code: bool,
    settings: &Settings,
    agents_md: Option<&str>,
    use_debug: bool,
    db: Option<&Arc<crate::db::Database>>,
    embedding_client: Option<&Arc<crate::embeddings::EmbeddingClient>>,
    cli_soulless: bool,
    continuation_tag: Option<&ContinuationTag>,
) -> AppResult<SendMessageResult> {
    let model_options = model_config.build_model_options();
    let blacklist_set = settings.blacklist_set();

    // Load facts from Factual Memory System
    let facts_section = if let Some(db_ref) = db {
        match db_ref.get_facts_for_prompt(session.project_id.as_deref()) {
            Ok(facts) if !facts.is_empty() => {
                let section = build_facts_section(&facts);
                if use_debug && !section.is_empty() {
                    log_debug(&format!("Loaded {} facts for prompt", facts.len()));
                }
                Some(section)
            }
            Ok(_) => None,
            Err(e) => {
                if use_debug {
                    log_debug(&format!("Warning: Failed to load facts: {}", e));
                }
                None
            }
        }
    } else {
        None
    };

    // Build system prompt
    // Get todos section from global state
    let todos_section = crate::tools::todo::format_todos_for_prompt();

    let system_prompt = build_session_system_prompt(
        session,
        tools_enabled,
        cli_code,
        cli_soulless,
        model_config,
        &blacklist_set,
        agents_md,
        facts_section.as_deref(),
        todos_section.as_deref(),
    );

    // Check context overflow
    let context_window = model_config.num_ctx as usize;
    let overflow_status = check_context_overflow(session, &system_prompt, context_window);

    // Show context warning only if tools are disabled.
    // When tools are enabled, check_and_compact_before_tool in continuation.rs
    // will show a more informative warning with remaining tokens.
    if overflow_status.needs_compaction() && !tools_enabled {
        eprintln!(
            "\x1B[33m⚠ Context {}% full. Consider using /compact to summarize old messages.\x1B[0m",
            overflow_status.usage_percent()
        );
    } else if use_debug {
        log_debug(&format!(
            "Context usage: {} / {} tokens ({:.1}%)",
            overflow_status.total_tokens(),
            context_window,
            overflow_status.usage_percent() as f32
        ));
    }

    // Setup coordinator with optional tools
    // Get real token count from session for accurate overflow detection
    let real_history_tokens = session.history_real_tokens();

    if use_debug {
        // Collect prompt_tokens state for debugging
        let prompt_tokens_state: Vec<(usize, Option<u64>)> = session
            .messages
            .iter()
            .enumerate()
            .map(|(i, m)| (i, m.prompt_tokens))
            .take(10) // First 10
            .collect();
        let has_nonzero_tokens = session
            .messages
            .iter()
            .any(|m| m.prompt_tokens.map(|t| t > 0).unwrap_or(false));

        log_debug(&format!(
            "[setup_coordinator] real_history_tokens={} messages={} has_compacted={} messages_sent_to_llm={}",
            real_history_tokens,
            session.messages.len(),
            session.has_compacted_messages(),
            session.messages_sent_to_llm
        ));
        log_debug(&format!(
            "[setup_coordinator] has_nonzero_prompt_tokens={} first_10_prompt_tokens={:?}",
            has_nonzero_tokens, prompt_tokens_state
        ));
        if session.has_compacted_messages() {
            log_debug(&format!(
                "[setup_coordinator] summary_len={} compacted_range={:?}",
                session
                    .compacted_summary
                    .as_ref()
                    .map(|s| s.len())
                    .unwrap_or(0),
                session.compacted_range
            ));
        }
    }

    let mut coordinator = setup_coordinator(
        ollama.clone(),
        model_config,
        model_options,
        think_enabled,
        use_debug,
        tools_enabled,
        settings,
        system_prompt.clone(),
        Some(real_history_tokens),
    );

    // Prepare messages with retrieval and continuation
    let mut messages = prepare_messages(
        session,
        db,
        embedding_client,
        user_input,
        &system_prompt,
        use_debug,
        &mut coordinator,
        continuation_tag,
    )
    .await;

    if use_debug {
        log_debug(&format!("Sending {} messages to model", messages.len()));
        if session.has_compacted_messages() {
            log_debug(&format!(
                "(includes compacted summary of {} messages)",
                session.compacted_message_count()
            ));
        }
    }

    let spinner = create_spinner("Thinking...");

    let tool_names: Vec<String> = if tools_enabled {
        get_available_tool_names(settings)
    } else {
        vec![]
    };

    // Execute with retry logic
    let mut attempts = 0;
    let result = loop {
        let current_result = if let (Some(db), Some(embedding)) = (db, embedding_client) {
            crate::tools::context::with_context(
                db.clone(),
                embedding.clone(),
                coordinator.chat(messages.clone()),
            )
            .await
        } else {
            coordinator.chat(messages.clone()).await
        };

        match current_result {
            Ok(response) => break Ok(response),
            Err(e) => {
                if is_ollama_error_recoverable(&e) && attempts < MAX_RETRIES {
                    attempts += 1;

                    let recovery_err = classify_ollama_error(&e, &tool_names);
                    let error_msg = format_recovery_message(&recovery_err);

                    if use_debug {
                        log_debug(&format!(
                            "🔧 [Recovery] Attempt {}/{} - {}",
                            attempts,
                            MAX_RETRIES,
                            recovery_err.description()
                        ));
                    }

                    messages.push(ChatMessage::tool(error_msg));

                    if attempts == 1 {
                        finish_spinner(spinner.clone());
                        eprintln!("\x1B[90m  Retrying after error...\x1B[0m");
                    }

                    continue;
                } else {
                    let error_str = e.to_string();
                    break Err(error_str);
                }
            }
        }
    };

    finish_spinner(spinner);

    // Process response and display it
    let processed_result = match result {
        Ok(response) => process_chat_response(
            response,
            think_enabled,
            &mut coordinator,
            context_window,
            system_prompt,
        ),
        Err(e) => return Err(e.into()),
    };

    Ok(processed_result)
}

/// Auto-compact conversation if context reaches buffer threshold
/// Uses buffer-based approach (15K tokens remaining) for predictable overflow prevention.
pub async fn auto_compact_if_needed(
    ollama: &ollama_rs::Ollama,
    model_config: &ModelConfig,
    session: &mut ChatSession,
    settings: &Settings,
    agents_md: Option<&str>,
    context_window: usize,
) {
    // Use buffer-based compaction trigger (more predictable than percentages)
    // Compacts when there are only COMPACTION_BUFFER tokens remaining
    if !needs_buffered_compaction(session, context_window) {
        return;
    }

    // Calculate usage percentage for display purposes
    let real_tokens = session.history_real_tokens();
    let usage_percent = ((real_tokens as f32 / context_window as f32) * 100.0).min(100.0) as u8;

    // Show indicator before starting compaction
    let urgency = if usage_percent >= 95 {
        "urgent"
    } else {
        "auto"
    };
    eprintln!(
        "\x1B[33m⏳ Compacting context ({}% full, {}K remaining)...\x1B[0m",
        usage_percent,
        (context_window.saturating_sub(real_tokens)) / 1000
    );

    // Attempt auto-compaction
    match compact_conversation(ollama, model_config, session, settings, agents_md).await {
        Ok((summary, range)) => {
            session.set_compacted_summary_with_range(summary, range);

            // Get compacted count
            let (first_preserved, last_preserved_start) =
                range.unwrap_or((0, session.messages.len()));
            let compacted_count = last_preserved_start - first_preserved;

            eprintln!(
                "\x1B[90m[{}-compacted: {} messages summarized]\x1B[0m",
                urgency, compacted_count
            );

            if !session.anonymous {
                let _ = session.save_sqlite();

                // Clear prompt_tokens in database since compaction invalidates old cumulative counts
                if let Some(db) = session.db.as_ref() {
                    let _ = db.clear_conversation_prompt_tokens(&session.id);
                }
            }
        }
        Err(e) => {
            eprintln!("\x1B[31mAuto-compaction failed: {}\x1B[0m", e);
        }
    }
}

/// Compact conversation by summarizing old messages
#[allow(clippy::too_many_arguments)]
pub async fn compact_conversation(
    ollama: &ollama_rs::Ollama,
    model_config: &ModelConfig,
    session: &ChatSession,
    _settings: &Settings,
    _agents_md: Option<&str>,
) -> AppResult<(String, Option<(usize, usize)>)> {
    use crate::context_overflow::get_compaction_range_default;

    if session.messages.is_empty() {
        return Err("No messages to compact.".into());
    }

    // Determine which messages to summarize (middle compaction)
    let (messages_to_summarize, range) = match get_compaction_range_default(session) {
        Some(suggestion) => {
            // Middle compaction: preserve first N + last N, summarize middle
            let middle: Vec<_> = session.messages[suggestion.middle_indices.clone()].to_vec();
            let range = Some((
                suggestion.keep_first,
                session.messages.len() - suggestion.keep_last,
            ));
            (middle, range)
        }
        None => {
            // Not enough messages for middle compaction, summarize all
            let all = session.messages.clone();
            let range = Some((0, session.messages.len()));
            (all, range)
        }
    };

    // Build conversation text for summarization
    let mut conversation_text = String::new();
    for msg in &messages_to_summarize {
        match msg.role {
            super::session::MessageRole::User => {
                conversation_text.push_str(&format!("User: {}\n", msg.content));
            }
            super::session::MessageRole::Assistant => {
                conversation_text.push_str(&format!("Assistant: {}\n", msg.content));
            }
            super::session::MessageRole::System => {}
            super::session::MessageRole::Tool => {
                conversation_text.push_str(&format!("Tool call: {}\n", msg.content));
            }
        }
    }

    let compact_prompt = build_compaction_prompt(&conversation_text);

    let mut model_cfg = model_config.clone();
    model_cfg.temperature = 0.3;
    model_cfg.top_p = Some(0.9);
    let model_options = model_cfg.build_model_options();

    let mut coordinator =
        CustomCoordinator::new(ollama.clone(), model_config.model_id.clone(), vec![])
            .options(model_options);

    let messages = vec![
        ChatMessage::system("You are a helpful assistant that summarizes conversations in clean Markdown format. Always use headers, bullets, and formatting to make the summary readable and scannable.".to_string()),
        ChatMessage::user(compact_prompt),
    ];

    let spinner = create_spinner("Compacting...");
    let result = coordinator.chat(messages).await;
    finish_spinner(spinner);

    match result {
        Ok(response) => {
            let summary = strip_thinking_tags(&response.message.content);

            // Truncate summary if it exceeds MAX_SUMMARY_TOKENS
            // This prevents infinite compaction loops caused by oversized summaries
            let summary = if estimate_tokens(&summary) > MAX_SUMMARY_TOKENS {
                eprintln!(
                    "\x1B[33m⚠ Summary exceeds {} tokens, truncating...\x1B[0m",
                    MAX_SUMMARY_TOKENS
                );
                truncate_to_budget(&summary, MAX_SUMMARY_TOKENS)
            } else {
                summary
            };

            Ok((summary, range))
        }
        Err(e) => Err(format!("Failed to compact: {}", e).into()),
    }
}
