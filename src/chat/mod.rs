//! Chat module - Interactive multi-line chat mode
//!
//! This module provides an interactive REPL for conversing with Ollama models,
//! with persistent session storage per project.

pub mod cli;
pub mod commands;
pub mod completion;
pub mod coordinator;
pub mod custom_coordinator;
pub mod history;
pub mod model_switch;
pub mod repl;
pub mod session;
pub mod thinking;

pub use cli::ChatArgs;
pub use custom_coordinator::CustomCoordinator;
pub use repl::run_chat_repl;
pub use thinking::{display_thinking, strip_thinking_tags};