//! Chat commands - handles internal REPL commands
//!
//! Parses and executes commands like /quit, /clear, /model, etc.

use super::history::ConversationStorage;
use super::session::ChatSession;
use crate::debug_tools::toggle_debug;

/// Result of executing a command
#[derive(Debug, Clone, PartialEq)]
pub enum CommandResult {
    /// Continue the REPL loop
    Continue,
    /// Exit the REPL
    Exit,
    /// An error occurred (message included)
    Error(String),
    /// Toggle think mode (returns new state)
    ThinkToggled(bool),
    /// Toggle tools (returns new state)
    ToolsToggled(bool),
    /// Compact command (needs async handling in REPL)
    Compact,
    /// Tool output level changed
    ToolOutputChanged(super::session::ToolOutputLevel),
    /// Toggle debug (returns new state)
    DebugToggled(bool),
    /// Retry last message (regenerate response)
    Retry,
    /// Undo last message (remove response, show last input)
    Undo,
}

/// Parsed chat command
#[derive(Debug, Clone)]
pub enum ChatCommand {
    /// Exit the chat session
    Quit,
    /// Clear conversation history
    Clear,
    /// Show help message
    Help,
    /// Switch to a different model
    Model { name: String },
    /// Change the system prompt
    System { prompt: String },
    /// Save the current session
    Save { name: Option<String> },
    /// Load a session
    Load { name: String },
    /// Export conversation
    Export {
        format: ExportFormat,
        file: Option<String>,
    },
    /// List saved sessions
    List,
    /// Show session information
    Info,
    /// Toggle think mode
    Think,
    /// Toggle tools
    Tools,
    /// Compact conversation history
    Compact,
    /// Set tool output level
    ToolsOutput {
        level: super::session::ToolOutputLevel,
    },
    /// Enable debug
    Debug,
    /// Retry last message (regenerate response)
    Retry,
    /// Undo last message (remove response, show last input)
    Undo,
}

/// Export format for /export command
#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    Markdown,
    Json,
}

/// Parse a command string
pub fn parse_command(input: &str) -> Option<Result<ChatCommand, String>> {
    let input = input.trim();

    if !input.starts_with('/') {
        return None;
    }

    let input = input.strip_prefix('/')?;
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let cmd = parts.first().unwrap_or(&"");
    let args = parts.get(1).copied().unwrap_or("");

    let command = match *cmd {
        "quit" | "exit" | "q" => ChatCommand::Quit,
        "clear" | "c" => ChatCommand::Clear,
        "help" | "h" | "?" => ChatCommand::Help,
        "model" | "m" => {
            if args.is_empty() {
                return Some(Err("Usage: /model <name>".to_string()));
            }
            ChatCommand::Model {
                name: args.trim().to_string(),
            }
        }
        "system" | "sys" | "s" => {
            if args.is_empty() {
                return Some(Err("Usage: /system <prompt>".to_string()));
            }
            ChatCommand::System {
                prompt: args.trim().to_string(),
            }
        }
        "save" => {
            let name = if args.is_empty() {
                None
            } else {
                Some(args.trim().to_string())
            };
            ChatCommand::Save { name }
        }
        "load" | "l" => {
            if args.is_empty() {
                return Some(Err("Usage: /load <session-name>".to_string()));
            }
            ChatCommand::Load {
                name: args.trim().to_string(),
            }
        }
        "export" | "e" => {
            let args_trimmed = args.trim();
            if args_trimmed.is_empty() {
                return Some(Err("Usage: /export <format> [file]".to_string()));
            }
            let parts: Vec<&str> = args_trimmed.splitn(2, ' ').collect();
            let format_str = parts.first().unwrap_or(&"md");
            let file = parts.get(1).map(|s| s.trim().to_string());

            let format = match *format_str {
                "md" | "markdown" => ExportFormat::Markdown,
                "json" => ExportFormat::Json,
                _ => {
                    return Some(Err(format!(
                        "Unknown format: {}. Use 'md' or 'json'.",
                        format_str
                    )));
                }
            };

            ChatCommand::Export { format, file }
        }
        "list" | "ls" => ChatCommand::List,
        "info" | "i" => ChatCommand::Info,
        "think" | "t" => ChatCommand::Think,
        "debug" | "d" => ChatCommand::Debug,
        "tools" => ChatCommand::Tools,
        "compact" => ChatCommand::Compact,
        "tools-output" | "to" => {
            if args.is_empty() {
                return Some(Err("Usage: /tools-output <compact|full|hidden>".to_string()));
            }
            match args.trim().parse::<super::session::ToolOutputLevel>() {
                Ok(level) => ChatCommand::ToolsOutput { level },
                Err(e) => return Some(Err(e)),
            }
        }
        "retry" | "r" => ChatCommand::Retry,
        "undo" | "u" => ChatCommand::Undo,
        _ => return Some(Err(format!("Unknown command: /{}", cmd))),
    };

    Some(Ok(command))
}

/// Execute a chat command
pub fn execute_command(
    command: ChatCommand,
    session: &mut ChatSession,
    storage: &ConversationStorage,
) -> CommandResult {
    match command {
        ChatCommand::Quit => {
            println!("Goodbye!");
            CommandResult::Exit
        }

        ChatCommand::Clear => {
            session.clear_messages();
            println!("Conversation history cleared.");
            CommandResult::Continue
        }

        ChatCommand::Help => {
            print_help();
            CommandResult::Continue
        }

        // Note: Model switching is handled directly in repl.rs via model_switch module
        // This ensures consistent state management for capabilities, tools, and think mode
        ChatCommand::Model { name: _ } => CommandResult::Continue,

        ChatCommand::System { prompt } => {
            session.set_system_prompt(prompt.clone());
            println!("System prompt updated.");
            CommandResult::Continue
        }

        ChatCommand::Save { name } => {
            if session.anonymous {
                return CommandResult::Error(
                    "Cannot save anonymous session. Use /save without --anonymous flag."
                        .to_string(),
                );
            }

            if let Some(n) = name {
                session.rename(n);
            }

            match session.save(storage) {
                Ok(()) => {
                    let session_name = session.name.as_deref().unwrap_or(&session.id);
                    println!("Session saved: {}", session_name);
                    CommandResult::Continue
                }
                Err(e) => CommandResult::Error(format!("Failed to save session: {}", e)),
            }
        }

        ChatCommand::Load { name } => {
            match ChatSession::load(storage, &session.project_id, &name) {
                Ok(loaded) => {
                    *session = loaded;
                    let display_name = session.name.as_deref().unwrap_or(&session.id);
                    println!(
                        "Loaded session: {} ({} messages)",
                        display_name,
                        session.messages.len()
                    );
                    CommandResult::Continue
                }
                Err(e) => CommandResult::Error(format!("Failed to load session: {}", e)),
            }
        }

        ChatCommand::Export { format, file } => {
            let output = match format {
                ExportFormat::Markdown => export_markdown(session),
                ExportFormat::Json => export_json(session),
            };

            match file {
                Some(path) => match std::fs::write(&path, &output) {
                    Ok(()) => {
                        println!("Conversation exported to: {}", path);
                        CommandResult::Continue
                    }
                    Err(e) => CommandResult::Error(format!("Failed to write file: {}", e)),
                },
                None => {
                    println!("{}", output);
                    CommandResult::Continue
                }
            }
        }

        ChatCommand::List => {
            let sessions = storage.list_sessions(&session.project_id);
            if sessions.is_empty() {
                println!("No saved sessions for this project.");
            } else {
                println!("Saved sessions:");
                for info in sessions {
                    let name = info.name.as_deref().unwrap_or(&info.id);
                    let time = info.updated_at.format("%Y-%m-%d %H:%M");
                    println!(
                        "  {} - {} messages, {} (model: {})",
                        name, info.message_count, time, info.model
                    );
                }
            }
            CommandResult::Continue
        }

        ChatCommand::Info => {
            print_session_info(session);
            CommandResult::Continue
        }

        ChatCommand::Think => {
            session.think = !session.think;
            CommandResult::ThinkToggled(session.think)
        }

        ChatCommand::Tools => {
            session.tools = !session.tools;
            CommandResult::ToolsToggled(session.tools)
        }

        ChatCommand::Compact => CommandResult::Compact,

        ChatCommand::ToolsOutput { level } => {
            session.tool_output_level = level;
            CommandResult::ToolOutputChanged(level)
        }

        ChatCommand::Debug => CommandResult::DebugToggled(toggle_debug()),

        ChatCommand::Retry => CommandResult::Retry,

        ChatCommand::Undo => CommandResult::Undo,
    }
}

/// Print help message
fn print_help() {
    println!(
        r#"Available commands:
  /quit, /exit     Exit the chat session
  /clear           Clear conversation history
  /help            Show this help message
  /model <name>    Switch to a different model
  /system <text>   Change the system prompt
  /think           Toggle think mode
  /tools           Toggle tools
  /tools-output    Set tool output level (compact|full|hidden)
  /compact         Compact conversation history (summarize)
  /retry           Retry last message (regenerate response)
  /undo            Undo last message (remove response, show last input)
  /save [name]     Save current session (optionally named)
  /load <name>     Load a saved session
  /export <fmt>    Export conversation (md, json)
  /list            List saved sessions for this project
  /info            Show current session information

Shortcuts:
  /q = /quit, /c = /clear, /h = /help
  /m = /model, /s = /system, /l = /load
  /t = /think, /e = /export, /ls = /list, /i = /info
  /r = /retry, /to = /tools-output, /u = /undo"#
    );
}

/// Print session information
fn print_session_info(session: &ChatSession) {
    let name = session.name.as_deref().unwrap_or("unnamed");
    let project = session.project_id.as_deref().unwrap_or("none");
    let created = session.created_at.format("%Y-%m-%d %H:%M:%S");
    let updated = session.updated_at.format("%Y-%m-%d %H:%M:%S");

    println!("Session Information:");
    println!("  ID:        {}", session.id);
    println!("  Name:      {}", name);
    println!("  Project:   {}", project);
    println!("  Model:     {}", session.model);
    println!("  Messages:  {} (total)", session.messages.len());

    if session.has_compacted_messages() {
        println!(
            "  Compacted: {} messages summarized",
            session.compacted_message_count()
        );
    }

    println!("  Think:     {}", session.think);
    println!("  Tools:     {}", session.tools);
    println!("  Anonymous: {}", session.anonymous);
    println!("  Created:   {}", created);
    println!("  Updated:   {}", updated);

    if let Some(ref prompt) = session.system_prompt {
        let preview = crate::utils::truncate_chars(prompt, 100);
        println!("  System:    {}", preview);
    }
}

/// Export session as markdown
fn export_markdown(session: &ChatSession) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "# Chat Session: {}\n\n",
        session.name.as_deref().unwrap_or(&session.id)
    ));
    output.push_str(&format!("- **Model:** {}\n", session.model));
    output.push_str(&format!(
        "- **Created:** {}\n",
        session.created_at.format("%Y-%m-%d %H:%M")
    ));
    output.push_str(&format!("- **Messages:** {}\n\n", session.messages.len()));
    output.push_str("---\n\n");

    for msg in &session.messages {
        match msg.role {
            super::session::MessageRole::User => {
                output.push_str(&format!("**User:** {}\n\n", msg.content));
            }
            super::session::MessageRole::Assistant => {
                output.push_str(&format!("**Assistant:**\n\n{}\n\n", msg.content));
            }
            super::session::MessageRole::System => {
                output.push_str(&format!("**System:** {}\n\n", msg.content));
            }
            super::session::MessageRole::Tool => {
                output.push_str(&format!("**Tool:** {}\n\n", msg.content));
            }
        }
    }

    output
}

/// Export session as JSON
fn export_json(session: &ChatSession) -> String {
    serde_json::to_string_pretty(session).unwrap_or_else(|_| "{}".to_string())
}
