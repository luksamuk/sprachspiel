//! Chat module - Interactive multi-line chat mode
//!
//! This module provides an interactive REPL for conversing with Ollama models,
//! with persistent session storage per project.
//!
//! # Architecture
//!
//! ```text
//! Layer 0: input.rs (trait), view.rs (trait) - NO dependencies
//! Layer 1: session.rs, cli.rs
//! Layer 2: input/rustyline.rs, view/terminal.rs
//! Layer 3: repl_state.rs
//! Layer 4: core.rs, command_handlers.rs
//! Layer 5: repl.rs (coordinator)
//! ```

pub mod cli;
pub mod command_handlers;
pub mod commands;
pub mod completion;
pub mod continuation;
pub mod coordinator;
pub mod core;
pub mod custom_coordinator;
pub mod input;
pub mod model_switch;
pub mod repl;
pub mod repl_state;
pub mod session;
pub mod subagent;
pub mod thinking;
pub mod todo_state;
pub mod view;

pub use cli::ChatArgs;
pub use custom_coordinator::{ContinuationTag, CustomCoordinator, parse_continuation_tag};
pub use repl::run_chat_repl;
#[allow(unused_imports)]
pub use subagent::{SubagentConfig, SubagentRunner, SubagentType};
pub use thinking::{display_thinking, strip_thinking_tags};
