//! Tools module — LLM-callable tool implementations.
//!
//! Provides all tools that the model can invoke during a conversation, including
//! file operations, web search, weather, todos, fact management, and more.
//! Tools are feature-gated and registered at startup via [`registry`].
//!
//! # Feature gates
//!
//! - `all-tools` — enables all optional tools
//! - `calc-tools` — calculator
//! - `file-tools` / `file-write-tools` — filesystem operations and write access
//! - `system-tools` — system info commands
//! - `document-tools` — document import (PDF, DOCX, etc.)
//!
//! Always-available tools: [`fact_tools`], [`notes`], [`feedback_tools`], [`remember`],
//! [`mod@todo`], [`context`], [`misc`], [`files`], [`files_blocklist`], [`registry`].

#[cfg(feature = "calc-tools")]
pub mod calc;
pub mod context;
pub mod diff_render;
pub mod file_state;
pub mod files;
pub mod files_blocklist;
#[cfg(feature = "file-tools")]
pub mod files_write;
pub mod misc;
pub mod registry;
pub mod remember;
#[cfg(feature = "system-tools")]
pub mod system;
pub mod todo;
pub mod tool_trait;
pub mod weather;

// Fact tools (always available)
pub mod fact_tools;

// Notes tools (always available)
pub mod notes;

// Feedback tools (always available, gated by config)
pub mod feedback_tools;

// Document import tool
#[cfg(feature = "document-tools")]
pub mod documents;

// External tool wrappers (always available)
pub mod run_cmd;
pub mod tool_check;

// Skills tools (on-demand skill loading)
#[cfg(feature = "skills-tools")]
pub mod skill_tools;

// Subagent spawning tools (dedicated per agent type)
#[cfg(feature = "subagent-tools")]
pub mod subagent_tools;

#[cfg(feature = "calc-tools")]
pub use calc::*;
#[cfg(feature = "file-tools")]
pub use files::*;
#[cfg(feature = "file-tools")]
pub use files_write::*;
pub use misc::*;
pub use registry::{get_available_tool_names, register_tools};
pub use remember::*;
#[cfg(feature = "system-tools")]
pub use system::*;
pub use weather::*;

// Re-export the Tool trait at the tools module level for the
// `#[sprachspiel::tool]` proc-macro. The macro emits `impl crate::tools::Tool
// for ...` which resolves to this re-export at the call site. Without this,
// the macro cannot find the trait when expanded in the parent crate.
pub use tool_trait::{Tool, ToolResult};
// Documents tools are imported directly in registry.rs

pub use run_cmd::run_command;
pub use tool_check::check_tool_availability;

// Skills tools
#[cfg(feature = "skills-tools")]
#[allow(unused_imports)]
pub use skill_tools::*;

// Subagent tools are imported directly in registry.rs
