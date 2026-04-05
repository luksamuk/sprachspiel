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
    .debug(ctx.output_flags.debug)
    .on_event({
        let use_think = ctx.use_think;
        let use_plain = ctx.output_flags.plain;
        let use_debug = ctx.output_flags.debug;
        move |event| {
            super::handle_chat_event(event, use_think, use_plain, use_debug);
        }
    });

    let mut coordinator = coordinator.context_window(ctx.model_config.num_ctx as usize);

    coordinator = coordinator.system_prompt(ctx.system_prompt.clone());

    if ctx.use_tools {
        if ctx.output_flags.debug {
            eprintln!("🔧 [Tools] Tools enabled - will log when called");
        }
        let (coord_new, tool_count) =
            crate::tools::register_tools(coordinator, settings, ctx.output_flags.debug);
        coordinator = coord_new;
        if ctx.output_flags.debug {
            eprintln!("   -> {} tools active", tool_count);
        }
    } else if ctx.output_flags.debug {
        eprintln!("⚠️  [Tools] No tools enabled for this model");
    }

    coordinator
}
