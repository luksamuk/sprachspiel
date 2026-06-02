//! Query execution with retry logic
//!
//! Provides execute_query_with_retry to handle Ollama errors with retry logic.
//!
//! **W2 Wave Context:** This module's retry loop is migrated to the
//! per-category classification in #116. The `is_ollama_error_recoverable()`
//! call is replaced by `crate::retry::classify_for_retry()` + `is_retryable()`.

#![expect(clippy::print_stderr)] // Query executor output
use std::sync::Arc;

use indicatif::ProgressBar;
use ollama_rs::Ollama;
use ollama_rs::generation::chat::ChatMessage;

use crate::chat::coordinator::{classify_ollama_error, format_recovery_message};
use crate::chat::custom_coordinator::CustomCoordinator;
use crate::chat::recovery::push_tool_result;
use crate::db::Database;
use crate::embeddings::EmbeddingClient;
use crate::retry::{classify_for_retry, retry_delay, sleep_or_cancel};
use crate::settings::Settings;
use crate::tools::context::{with_full_context, with_tool_context};

/// Execute a query with retry logic.
///
/// Handles recoverable Ollama errors by retrying up to MAX_RETRIES times.
/// Automatically wraps with full context if available for the remember tool
/// and agent spawning tools.
#[expect(clippy::too_many_arguments)]
pub async fn execute_query_with_retry(
    coordinator: CustomCoordinator<Vec<ChatMessage>>,
    messages: Vec<ChatMessage>,
    db: Option<Arc<Database>>,
    embedding_client: Option<Arc<EmbeddingClient>>,
    ollama: Ollama,
    settings: Arc<Settings>,
    tool_names: &[String],
    spinner: ProgressBar,
) -> Result<ollama_rs::generation::chat::ChatMessageResponse, String> {
    if let (Some(db), Some(embedding)) = (&db, &embedding_client) {
        execute_with_context(
            coordinator,
            messages,
            db.clone(),
            embedding.clone(),
            ollama,
            settings,
            tool_names,
            spinner,
        )
        .await
    } else {
        execute_without_context(coordinator, messages, ollama, settings, tool_names, spinner).await
    }
}

/// Execute with DB context (for remember tool support).
#[expect(clippy::too_many_arguments)]
async fn execute_with_context(
    coordinator: CustomCoordinator<Vec<ChatMessage>>,
    messages: Vec<ChatMessage>,
    db: Arc<Database>,
    embedding: Arc<EmbeddingClient>,
    ollama: Ollama,
    settings: Arc<Settings>,
    tool_names: &[String],
    spinner: ProgressBar,
) -> Result<ollama_rs::generation::chat::ChatMessageResponse, String> {
    with_full_context(db, embedding, ollama, settings, async {
        execute_retry_loop(coordinator, messages, tool_names, spinner).await
    })
    .await
}

/// Execute without DB context (code mode or anonymous).
async fn execute_without_context(
    coordinator: CustomCoordinator<Vec<ChatMessage>>,
    messages: Vec<ChatMessage>,
    ollama: Ollama,
    settings: Arc<Settings>,
    tool_names: &[String],
    spinner: ProgressBar,
) -> Result<ollama_rs::generation::chat::ChatMessageResponse, String> {
    with_tool_context(ollama, settings, async {
        execute_retry_loop(coordinator, messages, tool_names, spinner).await
    })
    .await
}

/// Core retry loop shared by both execution paths.
async fn execute_retry_loop(
    mut coordinator: CustomCoordinator<Vec<ChatMessage>>,
    messages: Vec<ChatMessage>,
    tool_names: &[String],
    spinner: ProgressBar,
) -> Result<ollama_rs::generation::chat::ChatMessageResponse, String> {
    let mut attempts = 0;
    let mut messages = messages;

    loop {
        let current_result = coordinator.chat(messages.clone()).await;

        match current_result {
            Ok(response) => break Ok(response),
            Err(e) => {
                let category = classify_for_retry(&e);
                if category.is_retryable() && attempts < category.max_attempts() {
                    attempts += 1;

                    let recovery_err = classify_ollama_error(&e, tool_names);
                    let error_msg = format_recovery_message(&recovery_err);

                    if log::log_enabled!(log::Level::Debug) {
                        log::debug!(
                            "🔧 [Recovery] Attempt {}/{} - {}",
                            attempts,
                            category.max_attempts(),
                            recovery_err.description()
                        );
                    }

                    push_tool_result(&mut messages, error_msg);

                    if attempts == 1 {
                        crate::spinner::finish_spinner(spinner.clone());
                        let delay = retry_delay(&category, attempts);
                        if delay > std::time::Duration::ZERO {
                            eprintln!("  Retrying in {}s...", delay.as_secs());
                        } else {
                            eprintln!("  Retrying after error...");
                        }
                    }

                    // Query mode has no cancel token — unconditional sleep
                    let _completed = sleep_or_cancel(retry_delay(&category, attempts), None).await;

                    continue;
                } else {
                    break Err(e.to_string());
                }
            }
        }
    }
}
