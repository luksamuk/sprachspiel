//! CLI structures for chat subcommand
//!
//! Defines the ChatArgs struct and related CLI parsing for the
//! `ask chat` subcommand.

use clap::Args;

use super::session::ToolOutputLevel;

/// Arguments for the chat subcommand
#[derive(Args, Debug, Clone)]
#[command(
    about = "Interactive chat with conversation history",
    long_about = r#"
Start an interactive chat session with an Ollama model.

Conversations are automatically saved per project (identified by git remote URL
or folder name). Use --anonymous for temporary sessions without persistence.

COMMANDS (inside chat):
  /quit, /exit    Exit the chat session
  /clear          Clear conversation history
  /help           Show available commands
  /model <name>   Switch to a different model
  /system <text>  Change the system prompt
  /save [name]    Save current session (optionally named)
  /load <name>    Load a saved session
  /export <fmt>   Export conversation (markdown, json)
  /list           List saved sessions for this project
  /info           Show current session information

EXAMPLES:
  ask chat                      # Start chat with default model
  ask chat -m lfm               # Start with specific model
  ask chat --anonymous          # Temporary session (no persistence)
  ask chat --load my-session    # Load a named session
  ask chat -t                   # Start with thinking mode enabled
"#
)]
pub struct ChatArgs {
    /// Anonymous session (no history persistence)
    #[arg(short, long)]
    pub anonymous: bool,

    /// Load a named session
    #[arg(short, long, value_name = "SESSION")]
    pub load: Option<String>,

    /// Model preset to use (overrides config)
    #[arg(short, long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Enable think mode for models that support it
    #[arg(short, long)]
    pub think: bool,

    /// Force enable tools even if model doesn't advertise tool support
    #[arg(long)]
    pub tools: bool,

    /// Ignore AGENTS.md file if present in current directory
    #[arg(long)]
    pub ignore_agents: bool,

    /// Skip SOUL.md personality (use neutral personality)
    #[arg(long)]
    pub soulless: bool,

    /// Increase verbosity (-v for verbose/debug, -vv for trace)
    #[arg(short = 'v', long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Tool output verbosity level (compact, full, hidden)
    #[arg(long, value_name = "LEVEL", default_value = "compact")]
    pub tools_output: ToolOutputLevel,
}
