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
use crate::context_overflow::{DEFAULT_OVERFLOW_THRESHOLD, check_context_overflow};
use crate::debug_tools::log_debug;
use crate::prompts::builder::{PromptConfig, PromptType, build_system_prompt};
use crate::retrieval::{RetrievalConfig, build_context, update_retrieval_time};
use crate::settings::Settings;
use crate::spinner::{create_spinner, finish_spinner};
use crate::tools::{get_available_tool_names, register_tools};

use super::coordinator::{
    MAX_RETRIES, classify_error_str, format_recovery_message, is_error_str_recoverable,
};
use super::custom_coordinator::CustomCoordinator;
use super::session::ChatSession;
use super::thinking::{display_thinking, strip_thinking_tags};
use super::{parse_continuation_tag, ContinuationTag};

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
    let ctx_status = check_context_overflow(session, "", ctx_window, DEFAULT_OVERFLOW_THRESHOLD);

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
            }),
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
        let continuation_prompt = build_continuation_prompt(tag);
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

/// Build a continuation prompt from a continuation tag
///
/// Creates a system message that tells the LLM to resume from where it paused
/// after context compaction.
pub fn build_continuation_prompt(tag: &ContinuationTag) -> String {
    format!(
        "<continuation_prompt>\n\
        Context has been compacted. Resume from the checkpoint.\n\
        \n\
        Reasoning paused at: {}\n\
        Next step: {}\n\
        \n\
        Continue naturally from where you left off. Do not repeat completed work.\n\
        </continuation_prompt>",
        tag.paused_at, tag.next_step
    )
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

    // Build system prompt
    let system_prompt = build_session_system_prompt(
        session,
        tools_enabled,
        cli_code,
        cli_soulless,
        model_config,
        &blacklist_set,
        agents_md,
    );

    // Check context overflow
    let context_window = model_config.num_ctx as usize;
    let overflow_status = check_context_overflow(
        session,
        &system_prompt,
        context_window,
        DEFAULT_OVERFLOW_THRESHOLD,
    );

    if overflow_status.needs_compaction() {
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
    let mut coordinator = setup_coordinator(
        ollama.clone(),
        model_config,
        model_options,
        think_enabled,
        use_debug,
        tools_enabled,
        settings,
        system_prompt.clone(),
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
                let error_str = e.to_string();

                if is_error_str_recoverable(&error_str) && attempts < MAX_RETRIES {
                    attempts += 1;

                    let recovery_err = classify_error_str(&error_str, &tool_names);
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
                    break Err(error_str);
                }
            }
        }
    };

    finish_spinner(spinner);

    match result {
        Ok(response) => Ok(process_chat_response(
            response,
            think_enabled,
            &mut coordinator,
            context_window,
            system_prompt,
        )),
        Err(e) => Err(e.into()),
    }
}

/// Auto-compact conversation if context overflow threshold reached
#[allow(clippy::too_many_arguments)]
pub async fn auto_compact_if_needed(
    ollama: &ollama_rs::Ollama,
    model_config: &ModelConfig,
    session: &mut ChatSession,
    settings: &Settings,
    agents_md: Option<&str>,
    system_prompt: &str,
    context_window: usize,
    use_debug: bool,
) {
    let status = check_context_overflow(
        session,
        system_prompt,
        context_window,
        DEFAULT_OVERFLOW_THRESHOLD,
    );

    if !status.needs_compaction() {
        return;
    }

    // Show indicator before starting compaction
    let urgency = if status.is_overflow() {
        "urgent"
    } else {
        "auto"
    };
    eprintln!(
        "\x1B[33m⏳ Compacting context ({}% full)...\x1B[0m",
        status.usage_percent()
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

            if !session.anonymous
                && let Err(e) = session.save_sqlite()
                && use_debug
            {
                log_debug(&format!(
                    "Warning: Could not save session after auto-compact: {}",
                    e
                ));
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

    let compact_prompt = format!(
        r#"Summarize the following conversation concisely in MARKDOWN format.

Use this structure:
**Key Topics:**
- Topic 1
- Topic 2

**Decisions Made:**
- Decision 1
- Decision 2

**Technical Details:**
- Important code/technical info

**Action Items:**
- [ ] Pending task 1
- [ ] Pending task 2

Conversation:
{}

Provide a structured markdown summary that captures the essential context."#,
        conversation_text
    );

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
            Ok((summary, range))
        }
        Err(e) => Err(format!("Failed to compact: {}", e).into()),
    }
}