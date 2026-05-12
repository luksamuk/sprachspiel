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
//! Layer 2: input/crossterm_input.rs, view/terminal.rs, view/ratatui_view.rs
//! Layer 3: repl_state.rs
//! Layer 4: core.rs, command_handlers.rs
//! Layer 5: repl.rs (coordinator)
//! ```
//!
//! # TUI Migration (W6-PR2, Issue #146)
//!
//! The chat REPL now runs via ratatui + crossterm for responsive rendering
//! at any terminal width. `RustylineInput` has been removed because rustyline
//! and ratatui are technically incompatible (both require raw mode and terminal
//! control). `CrosstermInput` handles key events via the crossterm event loop.
//!
//! Non-chat subcommands (query, translate, OCR, summarize) continue using
//! termimad + indicatif and are unaffected by the TUI migration.

pub mod cli;
pub mod command_handlers;
pub mod command_output;
pub mod commands;
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
pub mod tui;
pub mod view;

pub use cli::ChatArgs;
pub use command_output::CommandOutput;
pub use custom_coordinator::{ContinuationTag, CustomCoordinator, parse_continuation_tag};
pub use repl::run_chat_repl;
// Re-exported for external crate usage; not consumed within this crate
#[allow(unused_imports)]
pub use subagent::{SubagentConfig, SubagentRunner, SubagentType};
// Re-exported for external crate usage; not all are consumed within this crate
#[allow(unused_imports)]
pub use thinking::{display_thinking, extract_thinking, strip_thinking_tags};
