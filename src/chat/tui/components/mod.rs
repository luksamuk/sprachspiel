//! TUI components — ratatui widgets for chat rendering
//!
//! This module provides the individual widget components that make up
//! the chat TUI:
//!
//! - `chat_area`: Scrollable message display area
//! - `completion_menu`: Floating overlay for tab completions
//! - `status_bar`: Context usage, model name, and spinner
//! - `input_line`: User input display
//!
//! Each component is a pure function that takes state and returns a
//! ratatui `Block` or `Paragraph`, making them easy to test.

pub mod chat_area;
pub mod completion_menu;
pub mod input_line;
pub mod status_bar;
