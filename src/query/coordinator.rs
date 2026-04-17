//! Query coordinator builder
//!
//! Provides helper to build a coordinator for query execution.

use ollama_rs::generation::chat::ChatMessage;

use crate::chat::custom_coordinator::CustomCoordinator;
use crate::settings::Settings;

use super::context::QueryContext;

/// Build a coordinator for query execution.
pub fn build_query_coordinator(
    ctx: &QueryContext,
    settings: &Settings,
) -> CustomCoordinator<Vec<ChatMessage>> {
    let model_options = ctx.model_config.build_model_options();

    let coordinator = CustomCoordinator::new(
        ctx.ollama.clone(),
        ctx.model_config.model_id.clone(),
        vec![],
    )
    .options(model_options)
    .think(ctx.use_think)
    .on_event({
        let use_think = ctx.use_think;
        let use_plain = ctx.output_flags.plain;
        move |event| {
            super::handle_chat_event(event, use_think, use_plain);
        }
    });

    let mut coordinator = coordinator.context_window(ctx.model_config.num_ctx as usize);

    coordinator = coordinator.system_prompt(ctx.system_prompt.clone());

    if ctx.use_tools {
        if log::log_enabled!(log::Level::Debug) {
            eprintln!("🔧 [Tools] Tools enabled - will log when called");
        }
        let (coord_new, tool_count) = crate::tools::register_tools(coordinator, settings);
        coordinator = coord_new;
        if log::log_enabled!(log::Level::Debug) {
            eprintln!("   -> {} tools active", tool_count);
        }
    } else if log::log_enabled!(log::Level::Debug) {
        eprintln!("⚠️  [Tools] No tools enabled for this model");
    }

    coordinator
}
