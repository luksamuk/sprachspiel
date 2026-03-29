//! Chat commands - handles internal REPL commands
//!
//! Parses and executes commands like /quit, /new, /model, etc.

use super::session::ChatSession;
use crate::debug_tools::toggle_debug;
use crate::tokens::ContextMetrics;

/// Parse a document ID from user input.
/// Accepts formats: "N", "#N", "doc:N" (all return the numeric ID).
fn parse_document_id(input: &str) -> Result<i64, String> {
    let trimmed = input.trim();
    
    // Try to strip # prefix first
    let after_hash = trimmed.strip_prefix('#').unwrap_or(trimmed);
    
    // Then try to strip doc: prefix
    let after_doc = after_hash.strip_prefix("doc:").unwrap_or(after_hash);
    
    // Parse the remaining number
    after_doc
        .trim()
        .parse::<i64>()
        .map_err(|_| format!("Invalid document ID '{}'. Use: #N, doc:N, or just N", trimmed))
}

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
    /// Show context metrics (handled in REPL)
    Context,
    /// Search conversation history (handled in REPL)
    Search { query: String, limit: usize },
    /// Reindex embeddings (handled in REPL)
    Reindex,
    /// Toggle retrieval mode (returns new state)
    RetrievalToggled(bool),
    /// Prune old facts using decay cycle
    FactPrune,
    /// Add a new fact
    FactAdd {
        content: String,
        global: bool,
    },
    /// List facts
    FactList {
        global: bool,
    },
    /// Remove a fact by ID
    FactRemove { id: i64 },
    /// Search facts
    FactSearch {
        query: String,
        global: bool,
        limit: usize,
    },
    /// Add a new todo task
    TodoAdd { description: String },
    /// List todo tasks
    TodoList,
    /// Update todo task status
    TodoUpdate { id: usize, status: String },
    /// Clear completed todo tasks
    TodoClearDone,
    /// Clear all todo tasks
    TodoClearAll,
    /// Add a new note
    NoteAdd {
        content: String,
        title: Option<String>,
        global: bool,
    },
    /// List notes (with optional page)
    NoteList {
        global: bool,
        page: Option<usize>,
    },
    /// Show a note by ID
    NoteShow { id: i64 },
    /// Edit a note
    NoteEdit {
        id: i64,
        title: Option<String>,
        content: Option<String>,
    },
    /// Delete a note by ID
    NoteDelete { id: i64 },
    /// Search notes
    NoteSearch {
        query: String,
        global: bool,
        limit: usize,
    },
    /// Import a document
    DocumentImport {
        path: String,
        global: bool,
        nowait: bool,
    },
    /// List documents
    DocumentList {
        global: bool,
    },
    /// Show a document by ID
    DocumentShow { id: i64 },
    /// Delete a document by ID
    DocumentDelete { id: i64 },
    /// Activate a skill (loaded skill contents)
    Skill {
        name: String,
        content: String,
    },
}

/// Parsed chat command
#[derive(Debug, Clone)]
pub enum ChatCommand {
    /// Exit the chat session
    Quit,
    /// Start a new conversation session
    New,
    /// Forget everything (clear + delete from database)
    Forget,
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
    /// Session management commands
    Session { subcommand: SessionSubcommand },
    /// Show session information
    Info,
    /// Show context metrics and token usage
    Context,
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
    /// Search conversation history
    Search { query: String, limit: usize },
    /// Reindex embeddings for all content
    Reindex,
    /// Toggle retrieval mode
    Retrieval,
    /// Prune old facts using decay cycle
    FactPrune,
    /// Add a new fact
    FactAdd {
        content: String,
        global: bool,
    },
    /// List facts
    FactList {
        global: bool,
    },
    /// Remove a fact by ID
    FactRemove { id: i64 },
    /// Search facts
    FactSearch {
        query: String,
        global: bool,
        limit: usize,
    },
    /// Add a new todo task
    TodoAdd { description: String },
    /// List todo tasks
    TodoList,
    /// Update todo task status
    TodoUpdate { id: usize, status: String },
    /// Clear completed todo tasks
    TodoClearDone,
    /// Clear all todo tasks
    TodoClearAll,
    /// Add a new note
    NoteAdd {
        content: String,
        title: Option<String>,
        global: bool,
    },
    /// List notes (with optional page)
    NoteList {
        global: bool,
        page: Option<usize>,
    },
    /// Show a note by ID
    NoteShow { id: i64 },
    /// Edit a note
    NoteEdit {
        id: i64,
        title: Option<String>,
        content: Option<String>,
    },
    /// Delete a note by ID
    NoteDelete { id: i64 },
    /// Search notes
    NoteSearch {
        query: String,
        global: bool,
        limit: usize,
    },
    /// Import a document
    DocumentImport {
        path: String,
        global: bool,
        nowait: bool,
    },
    /// List documents
    DocumentList {
        global: bool,
    },
    /// Show a document by ID
    DocumentShow { id: i64 },
    /// Delete a document by ID
    DocumentDelete { id: i64 },
    /// Activate a skill by name
    Skill { name: String },
}

/// Export format for /export command
#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    Markdown,
    Json,
}

/// Session subcommands for /session command
#[derive(Debug, Clone, PartialEq)]
pub enum SessionSubcommand {
    New,
    Load { name: String },
    List,
    Save { name: Option<String> },
    Forget,
}

/// Parse note add command with proper quote handling
///
/// Handles:
/// - `/note add content --title "Title with spaces"`
/// - `/note add "content with spaces" --title Title`
/// - `/note add --title "Title" content`
/// - `/note add "test\nmultiline" --title Test` (expands \n inside quotes)
/// - Escaped -- as \-\-
fn parse_note_add(args: &str) -> Result<(String, Option<String>, bool), String> {
    let mut content_parts: Vec<String> = Vec::new();
    let mut title: Option<String> = None;
    let mut global = false;
    
    // State machine for parsing
    let chars: Vec<char> = args.chars().collect();
    let mut i = 0;
    let mut current_token = String::new();
    let mut in_quotes = false;
    let mut current_param: Option<&str> = None; // "title" when inside --title
    
    while i < chars.len() {
        let c = chars[i];
        
        if in_quotes {
            // Inside quotes - accumulate until closing quote
            if c == '"' {
                in_quotes = false;
                // Token complete
                if let Some(param) = current_param {
                    if param == "title" {
                        // Check for newlines in title
                        if current_token.contains('\n') || current_token.contains("\\n") {
                            return Err("Error: Title cannot contain newlines. Remove \\n or line breaks from title.".to_string());
                        }
                        title = Some(current_token.clone());
                    }
                    current_param = None;
                } else {
                    content_parts.push(current_token.clone().replace("\\n", "\n"));
                }
                current_token.clear();
            } else if c == '\\' && i + 1 < chars.len() {
                // Handle escapes inside quotes
                let next = chars[i + 1];
                if next == 'n' {
                    current_token.push('\n');
                    i += 1;
                } else if next == '\\' {
                    current_token.push('\\');
                    i += 1;
                } else if next == '"' {
                    current_token.push('"');
                    i += 1;
                } else if next == '-' {
                    current_token.push('-'); // Just push the dash, not \-
                    i += 1;
                } else {
                    current_token.push(c);
                }
            } else {
                current_token.push(c);
            }
            i += 1;
            continue;
        }
        
        // Not in quotes
        if c == '"' {
            in_quotes = true;
            i += 1;
            continue;
        }
        
        if c == '\\' && i + 1 < chars.len() && chars[i + 1] == '-' {
            // Escaped dash outside quotes - keep as literal
            current_token.push_str("\\-");
            i += 2;
            continue;
        }
        
        if c == ' ' || c == '\t' {
            // Token boundary
            if current_token == "--global" {
                global = true;
                current_token.clear();
            } else if current_token == "--title" {
                current_param = Some("title");
                current_token.clear();
            } else if !current_token.is_empty() {
                if let Some(_param) = current_param {
                    // Title token (unquoted)
                    // Check for newlines in title
                    if current_token.contains('\n') {
                        return Err("Error: Title cannot contain newlines. Remove line breaks from title.".to_string());
                    }
                    title = Some(current_token.clone());
                    current_param = None;
                } else {
                    content_parts.push(current_token.clone());
                }
                current_token.clear();
            }
            i += 1;
            continue;
        }
        
        // Regular character
        current_token.push(c);
        i += 1;
    }
    
    // Handle last token
    if current_token == "--global" {
        global = true;
    } else if current_token == "--title" {
        return Err("Error: --title requires a value. Usage: --title <title>".to_string());
    } else if !current_token.is_empty() {
        if let Some(_param) = current_param {
            // Unquoted title at end
            if current_token.contains('\n') {
                return Err("Error: Title cannot contain newlines. Remove line breaks from title.".to_string());
            }
            title = Some(current_token.clone());
        } else {
            content_parts.push(current_token.clone());
        }
    }
    
    // If we were expecting a title parameter but didn't get it
    if current_param.is_some() && title.is_none() {
        return Err("Error: --title requires a value. Usage: --title <title>".to_string());
    }
    
    let content = content_parts.join(" ");
    Ok((content, title, global))
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
        "new" | "n" => ChatCommand::New,
        "forget" | "f" => ChatCommand::Forget,
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
        "context" | "ctx" => ChatCommand::Context,
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
        "search" | "find" => {
            if args.is_empty() {
                return Some(Err("Usage: /search <query> [limit]".to_string()));
            }
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            let query = parts.first().unwrap_or(&"").to_string();
            let limit: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);
            ChatCommand::Search { query, limit }
        }
        "reindex" => {
            ChatCommand::Reindex
        }
        "retrieval" => ChatCommand::Retrieval,
        "fact" => {
            let subcmd_parts: Vec<&str> = args.splitn(2, ' ').collect();
            let subcmd = subcmd_parts.first().unwrap_or(&"");
            let subargs = subcmd_parts.get(1).copied().unwrap_or("");

            match *subcmd {
                "prune" | "p" => ChatCommand::FactPrune,
                "add" | "a" => {
                    if subargs.is_empty() {
                        return Some(Err("Usage: /fact add <content> [--global]".to_string()));
                    }
                    let global = subargs.trim().ends_with(" --global");
                    let content = if global {
                        subargs.trim().strip_suffix("--global").unwrap_or(subargs.trim()).trim().to_string()
                    } else {
                        subargs.trim().to_string()
                    };
                    if content.is_empty() {
                        return Some(Err("Usage: /fact add <content> [--global]".to_string()));
                    }
                    ChatCommand::FactAdd { content, global }
                }
                "list" | "l" => {
                    let global = subargs.trim() == "--global";
                    ChatCommand::FactList { global }
                }
                "remove" | "r" => {
                    if subargs.is_empty() {
                        return Some(Err("Usage: /fact remove <id>".to_string()));
                    }
                    match subargs.trim().parse::<i64>() {
                        Ok(id) => ChatCommand::FactRemove { id },
                        Err(_) => return Some(Err("Invalid fact ID. Must be a number.".to_string())),
                    }
                }
                "search" | "s" => {
                    if subargs.is_empty() {
                        return Some(Err("Usage: /fact search <query> [--global] [limit]".to_string()));
                    }
                    let global = subargs.contains("--global");
                    let args_without_global = subargs.replace("--global", "");
                    let args_trimmed = args_without_global.trim();
                    let parts: Vec<&str> = args_trimmed.splitn(2, ' ').collect();
                    let query = parts.first().unwrap_or(&"").to_string();
                    let limit: usize = parts.get(1).and_then(|s| s.trim().parse().ok()).unwrap_or(10);
                    if query.is_empty() {
                        return Some(Err("Usage: /fact search <query> [--global] [limit]".to_string()));
                    }
                    ChatCommand::FactSearch { query, global, limit }
                }
                _ => return Some(Err("Usage: /fact <add|list|remove|search|prune>".to_string())),
            }
        }
        "fp" => ChatCommand::FactPrune,
        "fa" => {
            if args.is_empty() {
                return Some(Err("Usage: /fa <content> [--global]".to_string()));
            }
            let global = args.trim().ends_with(" --global");
            let content = if global {
                args.trim().strip_suffix("--global").unwrap_or(args.trim()).trim().to_string()
            } else {
                args.trim().to_string()
            };
            if content.is_empty() {
                return Some(Err("Usage: /fa <content> [--global]".to_string()));
            }
            ChatCommand::FactAdd { content, global }
        }
        "fl" => {
            let global = args.trim() == "--global";
            ChatCommand::FactList { global }
        }
        "fr" => {
            if args.is_empty() {
                return Some(Err("Usage: /fr <id>".to_string()));
            }
            match args.trim().parse::<i64>() {
                Ok(id) => ChatCommand::FactRemove { id },
                Err(_) => return Some(Err("Invalid fact ID. Must be a number.".to_string())),
            }
        }
        "fs" => {
            if args.is_empty() {
                return Some(Err("Usage: /fs <query> [--global] [limit]".to_string()));
            }
            let global = args.contains("--global");
            let args_without_global = args.replace("--global", "");
            let args_trimmed = args_without_global.trim();
            let parts: Vec<&str> = args_trimmed.splitn(2, ' ').collect();
            let query = parts.first().unwrap_or(&"").to_string();
            let limit: usize = parts.get(1).and_then(|s| s.trim().parse().ok()).unwrap_or(10);
            if query.is_empty() {
                return Some(Err("Usage: /fs <query> [--global] [limit]".to_string()));
            }
            ChatCommand::FactSearch { query, global, limit }
        }
        "todo" => {
            let subcmd_parts: Vec<&str> = args.splitn(2, ' ').collect();
            let subcmd = subcmd_parts.first().unwrap_or(&"");
            let subargs = subcmd_parts.get(1).copied().unwrap_or("");

            match *subcmd {
                "add" | "a" => {
                    if subargs.is_empty() {
                        return Some(Err("Usage: /todo add <description>".to_string()));
                    }
                    ChatCommand::TodoAdd {
                        description: subargs.trim().to_string(),
                    }
                }
                "list" | "l" => ChatCommand::TodoList,
                "update" | "u" => {
                    let update_parts: Vec<&str> = subargs.splitn(2, ' ').collect();
                    if update_parts.len() < 2 {
                        return Some(Err("Usage: /todo update <id> <status>".to_string()));
                    }
                    let id: usize = match update_parts[0].trim().parse() {
                        Ok(id) => id,
                        Err(_) => return Some(Err("Invalid task ID. Must be a number.".to_string())),
                    };
                    let status = update_parts[1].trim().to_string();
                    ChatCommand::TodoUpdate { id, status }
                }
                "clear-done" | "cd" => ChatCommand::TodoClearDone,
                "clear-all" | "ca" => ChatCommand::TodoClearAll,
                _ => return Some(Err("Usage: /todo <add|list|update|clear-done|clear-all>".to_string())),
            }
        }
        "ta" => {
            if args.is_empty() {
                return Some(Err("Usage: /ta <description>".to_string()));
            }
            ChatCommand::TodoAdd {
                description: args.trim().to_string(),
            }
        }
        "tl" => ChatCommand::TodoList,
        "tu" => {
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            if parts.len() < 2 {
                return Some(Err("Usage: /tu <id> <status>".to_string()));
            }
            let id: usize = match parts[0].trim().parse() {
                Ok(id) => id,
                Err(_) => return Some(Err("Invalid task ID. Must be a number.".to_string())),
            };
            let status = parts[1].trim().to_string();
            ChatCommand::TodoUpdate { id, status }
        }
        "note" | "no" => {
            let subcmd_parts: Vec<&str> = args.splitn(2, ' ').collect();
            let subcmd = subcmd_parts.first().unwrap_or(&"");
            let subargs = subcmd_parts.get(1).copied().unwrap_or("");

            match *subcmd {
                "add" | "a" => {
                    if subargs.is_empty() {
                        return Some(Err("Usage: /note add <content> [--title <title>] [--global]".to_string()));
                    }
                    
                    // Parse note arguments with proper quote handling
                    match parse_note_add(subargs) {
                        Ok((content, title, global)) => {
                            if content.is_empty() {
                                return Some(Err("Usage: /note add <content> [--title <title>] [--global]".to_string()));
                            }
                            ChatCommand::NoteAdd { content, title, global }
                        }
                        Err(e) => return Some(Err(e)),
                    }
                }
                "list" | "l" => {
                    let mut global = false;
                    let mut page: Option<usize> = None;
                    
                    // Parse arguments: [--global] [page]
                    for part in subargs.split_whitespace() {
                        if part == "--global" {
                            global = true;
                        } else if let Ok(p) = part.parse::<usize>() {
                            if p == 0 {
                                return Some(Err("Page must be >= 1. Use /note list 1 for first page.".to_string()));
                            }
                            page = Some(p);
                        }
                    }
                    ChatCommand::NoteList { global, page }
                }
                "show" | "s" => {
                    if subargs.is_empty() {
                        return Some(Err("Usage: /note show <id>".to_string()));
                    }
                    match subargs.trim().parse::<i64>() {
                        Ok(id) => ChatCommand::NoteShow { id },
                        Err(_) => return Some(Err("Invalid note ID. Must be a number.".to_string())),
                    }
                }
                "edit" | "e" => {
                    let edit_parts: Vec<&str> = subargs.splitn(2, ' ').collect();
                    if edit_parts.len() < 2 {
                        return Some(Err("Usage: /note edit <id> [--title <title>] [--content <content>]".to_string()));
                    }
                    let id: i64 = match edit_parts[0].trim().parse() {
                        Ok(id) => id,
                        Err(_) => return Some(Err("Invalid note ID. Must be a number.".to_string())),
                    };
                    let rest = edit_parts[1].trim();
                    let has_title = rest.contains("--title");
                    let has_content = rest.contains("--content");
                    
                    let (title, content) = if has_title && has_content {
                        let title_idx = rest.find("--title").unwrap();
                        let content_idx = rest.find("--content").unwrap();
                        let (first, second) = if title_idx < content_idx {
                            (("--title", title_idx), ("--content", content_idx))
                        } else {
                            (("--content", content_idx), ("--title", title_idx))
                        };
                        let first_val_start = rest[first.1 + first.0.len()..].trim();
                        let first_end = first_val_start.find(" --").unwrap_or(first_val_start.len());
                        let first_val = first_val_start[..first_end].to_string();
                        
                        let second_val_start = rest[second.1 + second.0.len()..].trim();
                        let second_val = second_val_start.to_string();
                        
                        if first.0 == "--title" {
                            (Some(first_val), Some(second_val))
                        } else {
                            (Some(second_val), Some(first_val))
                        }
                    } else if has_title {
                        let title_idx = rest.find("--title").unwrap();
                        let title = rest[title_idx + 7..].trim().to_string();
                        (Some(title), None)
                    } else if has_content {
                        let content_idx = rest.find("--content").unwrap();
                        let content = rest[content_idx + 9..].trim().to_string();
                        (None, Some(content))
                    } else {
                        (None, None)
                    };
                    
                    if title.is_none() && content.is_none() {
                        return Some(Err("Usage: /note edit <id> [--title <title>] [--content <content>]".to_string()));
                    }
                    ChatCommand::NoteEdit { id, title, content }
                }
                "delete" | "d" => {
                    if subargs.is_empty() {
                        return Some(Err("Usage: /note delete <id>".to_string()));
                    }
                    match subargs.trim().parse::<i64>() {
                        Ok(id) => ChatCommand::NoteDelete { id },
                        Err(_) => return Some(Err("Invalid note ID. Must be a number.".to_string())),
                    }
                }
                "search" | "f" => {
                    if subargs.is_empty() {
                        return Some(Err("Usage: /note search <query> [--global] [limit]".to_string()));
                    }
                    let global = subargs.contains("--global");
                    let args_without_global = subargs.replace("--global", "");
                    let args_trimmed = args_without_global.trim();
                    let parts: Vec<&str> = args_trimmed.splitn(2, ' ').collect();
                    let query = parts.first().unwrap_or(&"").to_string();
                    let limit: usize = parts.get(1).and_then(|s| s.trim().parse().ok()).unwrap_or(10);
                    if query.is_empty() {
                        return Some(Err("Usage: /note search <query> [--global] [limit]".to_string()));
                    }
                    ChatCommand::NoteSearch { query, global, limit }
                }
                _ => return Some(Err("Usage: /note <add|list|show|edit|delete|search>".to_string())),
            }
        }
        "na" => {
            if args.is_empty() {
                return Some(Err("Usage: /na <content> [--title <title>] [--global]".to_string()));
            }
            match parse_note_add(args) {
                Ok((content, title, global)) => {
                    if content.is_empty() {
                        return Some(Err("Usage: /na <content> [--title <title>] [--global]".to_string()));
                    }
                    ChatCommand::NoteAdd { content, title, global }
                }
                Err(e) => return Some(Err(e)),
            }
        }
        "nl" => {
            let mut global = false;
            let mut page: Option<usize> = None;
            for part in args.split_whitespace() {
                if part == "--global" {
                    global = true;
                } else if let Ok(p) = part.parse::<usize>() {
                    if p == 0 {
                        return Some(Err("Page must be >= 1. Use /note list 1 for first page.".to_string()));
                    }
                    page = Some(p);
                }
            }
            ChatCommand::NoteList { global, page }
        }
        "ns" => {
            if args.is_empty() {
                return Some(Err("Usage: /ns <id>".to_string()));
            }
            match args.trim().parse::<i64>() {
                Ok(id) => ChatCommand::NoteShow { id },
                Err(_) => return Some(Err("Invalid note ID. Must be a number.".to_string())),
            }
        }
        "nd" => {
            if args.is_empty() {
                return Some(Err("Usage: /nd <id>".to_string()));
            }
            match args.trim().parse::<i64>() {
                Ok(id) => ChatCommand::NoteDelete { id },
                Err(_) => return Some(Err("Invalid note ID. Must be a number.".to_string())),
            }
        }
        "doc" | "document" | "docs" | "documents" => {
            let subcmd_parts: Vec<&str> = args.splitn(2, ' ').collect();
            let subcmd = subcmd_parts.first().unwrap_or(&"list");
            let subargs = subcmd_parts.get(1).copied().unwrap_or("");
            
            match *subcmd {
                "import" | "i" => {
                    if subargs.is_empty() {
                        return Some(Err("Usage: /doc import <path> [--global] [--nowait]".to_string()));
                    }
                    let global = subargs.contains("--global");
                    let nowait = subargs.contains("--nowait");
                    let path = subargs.replace("--global", "").replace("--nowait", "").trim().to_string();
                    if path.is_empty() {
                        return Some(Err("Usage: /doc import <path> [--global] [--nowait]".to_string()));
                    }
                    ChatCommand::DocumentImport { path, global, nowait }
                }
                "list" | "l" => {
                    let global = subargs.contains("--global");
                    ChatCommand::DocumentList { global }
                }
                "show" | "s" => {
                    if subargs.is_empty() {
                        return Some(Err("Usage: /doc show <id>".to_string()));
                    }
                    match parse_document_id(subargs.trim()) {
                        Ok(id) => ChatCommand::DocumentShow { id },
                        Err(e) => return Some(Err(e)),
                    }
                }
                "delete" | "d" | "remove" | "rm" => {
                    if subargs.is_empty() {
                        return Some(Err("Usage: /doc delete <id>".to_string()));
                    }
                    match parse_document_id(subargs.trim()) {
                        Ok(id) => ChatCommand::DocumentDelete { id },
                        Err(e) => return Some(Err(e)),
                    }
                }
                _ => return Some(Err("Usage: /doc <import|list|show|delete>".to_string())),
            }
        }
        "di" => {
            if args.is_empty() {
                return Some(Err("Usage: /di <path> [--global] [--nowait]".to_string()));
            }
            let global = args.contains("--global");
            let nowait = args.contains("--nowait");
            let path = args.replace("--global", "").replace("--nowait", "").trim().to_string();
            if path.is_empty() {
                return Some(Err("Usage: /di <path> [--global] [--nowait]".to_string()));
            }
            ChatCommand::DocumentImport { path, global, nowait }
        }
        "dl" => {
            let global = args.contains("--global");
            ChatCommand::DocumentList { global }
        }
        "ds" => {
            if args.is_empty() {
                return Some(Err("Usage: /ds <id>".to_string()));
            }
            match parse_document_id(args.trim()) {
                Ok(id) => ChatCommand::DocumentShow { id },
                Err(e) => return Some(Err(e)),
            }
        }
        "dd" => {
            if args.is_empty() {
                return Some(Err("Usage: /dd <id>".to_string()));
            }
            match parse_document_id(args.trim()) {
                Ok(id) => ChatCommand::DocumentDelete { id },
                Err(e) => return Some(Err(e)),
            }
        }
        "session" => {
            let subcmd_parts: Vec<&str> = args.splitn(2, ' ').collect();
            let subcmd = subcmd_parts.first().unwrap_or(&"");
            let subargs = subcmd_parts.get(1).copied().unwrap_or("");

            match *subcmd {
                "new" => ChatCommand::Session { subcommand: SessionSubcommand::New },
                "load" => {
                    if subargs.is_empty() {
                        return Some(Err("Usage: /session load <name>".to_string()));
                    }
                    ChatCommand::Session { subcommand: SessionSubcommand::Load { name: subargs.trim().to_string() } }
                }
                "list" => ChatCommand::Session { subcommand: SessionSubcommand::List },
                "save" => {
                    let name = if subargs.is_empty() {
                        None
                    } else {
                        Some(subargs.trim().to_string())
                    };
                    ChatCommand::Session { subcommand: SessionSubcommand::Save { name } }
                }
                "forget" => ChatCommand::Session { subcommand: SessionSubcommand::Forget },
                _ => return Some(Err("Usage: /session <new|load|list|save|forget>".to_string())),
            }
        }
        // Dynamic skill commands: /<skill-name> [args...]
        // Check if command matches a skill name (e.g., /document-processing)
        _ => {
            // Try to match against available skill names
            let skill_names = crate::skills::get_available_skill_names();
            if skill_names.iter().any(|s| s == cmd) {
                ChatCommand::Skill { name: cmd.to_string() }
            } else {
                return Some(Err(format!("Unknown command: /{}. Use /help for available commands.", cmd)));
            }
        }
    };

    Some(Ok(command))
}

/// Execute a chat command
pub fn execute_command(command: ChatCommand, session: &mut ChatSession) -> CommandResult {
    match command {
        ChatCommand::Quit => {
            println!("Goodbye!");
            CommandResult::Exit
        }

        ChatCommand::New => {
            // Check if there are searchable messages in database (any conversation)
            let has_searchable_messages = if let Some(ref db) = session.db {
                db.count_all_content_items()
                    .map(|count| count > 0)
                    .unwrap_or(false)
            } else {
                false
            };

            // Clear session state
            session.compacted_summary = None;
            session.messages.clear();
            session.messages_sent_to_llm = 0;
            session.compacted_range = None;
            session.name = None;

            // Generate new session ID
            use std::time::{SystemTime, UNIX_EPOCH};
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            session.id = format!("session-{}", timestamp);

            // Reset timestamps
            let now = chrono::Utc::now();
            session.created_at = now;
            session.updated_at = now;

            // Note: Session is NOT saved here - will be persisted on first message
            // This allows creating new sessions without polluting the database with empty sessions

            println!("New session started.");
            if has_searchable_messages {
                println!("\x1B[90m[i] Previous conversations remain searchable via /search or remember().\x1B[0m");
            }
            CommandResult::Continue
        }

        ChatCommand::Forget => {
            session.forget_session();

            if let Some(ref db) = session.db
                && !session.anonymous
                && !session.id.is_empty()
            {
                print!("Removing conversation from database... ");
                std::io::Write::flush(&mut std::io::stdout()).ok();
                match db.delete_conversation(&session.id) {
                    Ok(_) => println!("Done."),
                    Err(e) => eprintln!("\nWarning: Could not delete conversation: {}", e),
                }
            }

            if !session.anonymous {
                // Generate new session ID using timestamp
                use std::time::{SystemTime, UNIX_EPOCH};
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                session.id = format!("session-{}", timestamp);
                if let Err(e) = session.save_sqlite() {
                    eprintln!("Warning: Could not save new session: {}", e);
                }
            }

            println!("Session forgotten. Starting fresh conversation.");
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

            match session.save_sqlite() {
                Ok(()) => {
                    let session_name = session.name.as_deref().unwrap_or(&session.id);
                    println!("Session saved: {}", session_name);
                    CommandResult::Continue
                }
                Err(e) => CommandResult::Error(format!("Failed to save session: {}", e)),
            }
        }

        ChatCommand::Load { name } => {
            // Need database for load
            let db = match &session.db {
                Some(d) => std::sync::Arc::clone(d),
                None => {
                    return CommandResult::Error(
                        "Cannot load session: database not initialized.".to_string(),
                    );
                }
            };

            // Save current session if it has messages
            if !session.anonymous
                && !session.messages.is_empty()
                && let Err(e) = session.save_sqlite()
            {
                eprintln!("Warning: Could not save current session: {}", e);
            }

            match ChatSession::load_sqlite(&db, &name) {
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
                Some(path) => {
                    let expanded_path = crate::utils::expand_tilde_path(&path);
                    match std::fs::write(&expanded_path, &output) {
                        Ok(()) => {
                            println!("Conversation exported to: {}", path);
                            CommandResult::Continue
                        }
                        Err(e) => CommandResult::Error(format!("Failed to write file: {}", e)),
                    }
                }
                None => {
                    println!("{}", output);
                    CommandResult::Continue
                }
            }
        }

        ChatCommand::List => {
            // Need database for list
            let db = match &session.db {
                Some(d) => std::sync::Arc::clone(d),
                None => {
                    return CommandResult::Error(
                        "Cannot list sessions: database not initialized.".to_string(),
                    );
                }
            };

            match db.list_sessions(session.project_id.as_deref()) {
                Ok(sessions) => {
                    if sessions.is_empty() {
                        println!("No saved sessions for this project.");
                    } else {
                        println!("Sessions for this project:");
                        for info in sessions {
                            let time = info.updated_at.format("%Y-%m-%d %H:%M");
                            let name = info.name.as_deref().unwrap_or(&info.id);
                            // Mark current session with arrow
                            let marker = if info.id == session.id { "→" } else { " " };
                            println!("{} {} - {} ({} messages) {}", marker, name, info.model, info.message_count, time);
                        }
                    }
                }
                Err(e) => eprintln!("Warning: Could not list sessions: {}", e),
            }
            CommandResult::Continue
        }

        ChatCommand::Session { subcommand } => {
            // Delegate session subcommands to their respective handlers
            match subcommand {
                SessionSubcommand::New => {
                    // Same as ChatCommand::New
                    let has_searchable_messages = if let Some(ref db) = session.db {
                        db.count_all_content_items()
                            .map(|count| count > 0)
                            .unwrap_or(false)
                    } else {
                        false
                    };

                    session.compacted_summary = None;
                    session.messages.clear();
                    session.messages_sent_to_llm = 0;
                    session.compacted_range = None;
                    session.name = None;

                    use std::time::{SystemTime, UNIX_EPOCH};
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    session.id = format!("session-{}", timestamp);

                    let now = chrono::Utc::now();
                    session.created_at = now;
                    session.updated_at = now;

                    println!("New session started.");
                    if has_searchable_messages {
                        println!("\x1B[90m[i] Previous conversations remain searchable via /search or remember().\x1B[0m");
                    }
                    CommandResult::Continue
                }
                SessionSubcommand::Load { name } => {
                    // Same as ChatCommand::Load
                    let db = match &session.db {
                        Some(d) => std::sync::Arc::clone(d),
                        None => {
                            return CommandResult::Error(
                                "Cannot load session: database not initialized.".to_string(),
                            );
                        }
                    };

                    if !session.anonymous
                        && !session.messages.is_empty()
                        && let Err(e) = session.save_sqlite()
                    {
                        eprintln!("Warning: Could not save current session: {}", e);
                    }

                    match ChatSession::load_sqlite(&db, &name) {
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
                SessionSubcommand::List => {
                    // Same as ChatCommand::List
                    let db = match &session.db {
                        Some(d) => std::sync::Arc::clone(d),
                        None => {
                            return CommandResult::Error(
                                "Cannot list sessions: database not initialized.".to_string(),
                            );
                        }
                    };

                    match db.list_sessions(session.project_id.as_deref()) {
                        Ok(sessions) => {
                            if sessions.is_empty() {
                                println!("No saved sessions for this project.");
                            } else {
                                println!("Sessions for this project:");
                                for info in sessions {
                                    let time = info.updated_at.format("%Y-%m-%d %H:%M");
                                    let name = info.name.as_deref().unwrap_or(&info.id);
                                    // Mark current session with arrow
                                    let marker = if info.id == session.id { "→" } else { " " };
                                    println!("{} {} - {} ({} messages) {}", marker, name, info.model, info.message_count, time);
                                }
                            }
                        }
                        Err(e) => eprintln!("Warning: Could not list sessions: {}", e),
                    }
                    CommandResult::Continue
                }
                SessionSubcommand::Save { name } => {
                    // Same as ChatCommand::Save
                    if session.anonymous {
                        return CommandResult::Error(
                            "Cannot save anonymous session. Use /save without --anonymous flag.".to_string(),
                        );
                    }

                    if let Some(n) = name {
                        session.rename(n);
                    }

                    match session.save_sqlite() {
                        Ok(()) => {
                            let session_name = session.name.as_deref().unwrap_or(&session.id);
                            println!("Session saved: {}", session_name);
                            CommandResult::Continue
                        }
                        Err(e) => CommandResult::Error(format!("Failed to save session: {}", e)),
                    }
                }
                SessionSubcommand::Forget => {
                    // Same as ChatCommand::Forget
                    session.forget_session();

                    if let Some(ref db) = session.db
                        && !session.anonymous
                        && !session.id.is_empty()
                    {
                        print!("Removing conversation from database... ");
                        std::io::Write::flush(&mut std::io::stdout()).ok();
                        match db.delete_conversation(&session.id) {
                            Ok(_) => println!("Done."),
                            Err(e) => eprintln!("\nWarning: Could not delete conversation: {}", e),
                        }
                    }

                    if !session.anonymous {
                        use std::time::{SystemTime, UNIX_EPOCH};
                        let timestamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        session.id = format!("session-{}", timestamp);
                        if let Err(e) = session.save_sqlite() {
                            eprintln!("Warning: Could not save new session: {}", e);
                        }
                    }

                    println!("Session forgotten. Starting fresh conversation.");
                    CommandResult::Continue
                }
            }
        }

        ChatCommand::Info => {
            print_session_info(session, None);
            CommandResult::Continue
        }

        ChatCommand::Context => CommandResult::Context,

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

        ChatCommand::Search { query, limit } => CommandResult::Search { query, limit },

        ChatCommand::Reindex => CommandResult::Reindex,

        ChatCommand::Retrieval => {
            session.retrieval_enabled = !session.retrieval_enabled;
            CommandResult::RetrievalToggled(session.retrieval_enabled)
        }

        ChatCommand::FactPrune => CommandResult::FactPrune,

        ChatCommand::FactAdd { content, global } => {
            CommandResult::FactAdd { content, global }
        }

        ChatCommand::FactList { global } => CommandResult::FactList { global },

        ChatCommand::FactRemove { id } => CommandResult::FactRemove { id },

        ChatCommand::FactSearch { query, global, limit } => {
            CommandResult::FactSearch { query, global, limit }
        }

        ChatCommand::TodoAdd { description } => CommandResult::TodoAdd { description },

        ChatCommand::TodoList => CommandResult::TodoList,

        ChatCommand::TodoUpdate { id, status } => CommandResult::TodoUpdate { id, status },

        ChatCommand::TodoClearDone => CommandResult::TodoClearDone,

        ChatCommand::TodoClearAll => CommandResult::TodoClearAll,

        ChatCommand::NoteAdd { content, title, global } => CommandResult::NoteAdd { content, title, global },

        ChatCommand::NoteList { global, page } => CommandResult::NoteList { global, page },

        ChatCommand::NoteShow { id } => CommandResult::NoteShow { id },

        ChatCommand::NoteEdit { id, title, content } => CommandResult::NoteEdit { id, title, content },

        ChatCommand::NoteDelete { id } => CommandResult::NoteDelete { id },

        ChatCommand::NoteSearch { query, global, limit } => CommandResult::NoteSearch { query, global, limit },

        ChatCommand::DocumentImport { path, global, nowait } => CommandResult::DocumentImport { path, global, nowait },

        ChatCommand::DocumentList { global } => CommandResult::DocumentList { global },

        ChatCommand::DocumentShow { id } => CommandResult::DocumentShow { id },

        ChatCommand::DocumentDelete { id } => CommandResult::DocumentDelete { id },

        ChatCommand::Skill { name } => {
            // Load skill content and return it for injection into session
            let skill = crate::skills::get_skill_content(&name);
            match skill {
                Some(skill) => CommandResult::Skill {
                    name: skill.name,
                    content: skill.content,
                },
                None => CommandResult::Error(format!(
                    "Skill '{}' not found. Use one of: {}",
                    name,
                    crate::skills::get_available_skill_names().join(", ")
                )),
            }
        }
    }
}

/// Print help message
fn print_help() {
    println!(
        r#"Available commands:
  /quit, /exit     Exit the chat session
  /new, /n         Start a new conversation (previous messages remain searchable)
  /forget          Delete conversation completely and start fresh
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
  /session        Session management commands:
    /session new     Same as /new
    /session load <name>  Same as /load
    /session list    Same as /list
    /session save [name]  Same as /save
    /session forget  Same as /forget
  /export <fmt>    Export conversation (md, json)
  /list            List saved sessions for this project
  /info            Show current session information
  /context         Show context metrics and token usage
  /search <query>  Search current conversation (keyword + semantic)
  /reindex         Regenerate embeddings for all content
  /retrieval       Toggle semantic retrieval from conversation history

Factual Memory:
  /fact add <text> [--global]   Add a fact (project scope by default)
  /fact list [--global]         List facts (project scope by default)
  /fact remove <id>             Remove a fact by ID
  /fact search <query> [--global] [limit]   Search facts
  /fact prune      Prune old facts using decay cycle

  Subcommand shortcuts: /fact a, /fact l, /fact r, /fact s, /fact p

Notes:
  /note add <content> [--title <title>] [--global]   Add a note
  /note list [--global] [page]                        List notes (8 per page)
  /note show <id>                                    Show a note
  /note edit <id> [--title <title>] [--content <content>]   Edit a note
  /note delete <id>                                  Delete a note
  /note search <query> [--global] [limit]            Search notes

  Subcommand shortcuts: /no = /note, /na = /note add
  /nl = /note list, /ns = /note show, /nd = /note delete

Documents:
  /doc import <path> [--global]   Import a document (TXT, MD, ORG, PDF, EPUB)
  /doc list [--global]            List documents
  /doc show <id>                  Show a document
  /doc delete <id>                Delete a document

  Subcommand shortcuts: /di = /doc import, /dl = /doc list
  /ds = /doc show, /dd = /doc delete

Todo List:
  /todo add <description>    Add a new task
  /todo list                 List all tasks
  /todo update <id> <status> Update task status (pending|in_progress|done)
  /todo clear-done           Clear completed tasks
  /todo clear-all            Clear all tasks

  Subcommand shortcuts: /ta = /todo add, /tl = /todo list, /tu = /todo update

Shortcuts:
  /q = /quit, /n = /new, /h = /help
  /m = /model, /s = /system, /l = /load
  /t = /think, /e = /export, /ls = /list, /i = /info
  /r = /retry, /to = /tools-output, /u = /undo
  /ctx = /context, /f = /search (find)
  /fp = /fact prune, /fa = /fact add
  /fl = /fact list, /fr = /fact remove, /fs = /fact search"#
    );
}

/// Print session information
pub fn print_session_info(session: &ChatSession, metrics: Option<&ContextMetrics>) {
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

    if let Some(m) = metrics {
        println!(
            "  Context:   {} / {} tokens ({:.1}%)",
            m.total_tokens,
            m.context_window,
            m.utilization * 100.0
        );
    }

    println!("  Think:     {}", session.think);
    println!("  Tools:     {}", session.tools);

    // Show sandbox status
    let sandbox_status = crate::external::get_sandbox_status();
    println!("  Sandbox:   {}", sandbox_status);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_document_id_numeric() {
        assert_eq!(parse_document_id("1").unwrap(), 1);
        assert_eq!(parse_document_id("42").unwrap(), 42);
        assert_eq!(parse_document_id("-5").unwrap(), -5);
    }

    #[test]
    fn test_parse_document_id_hashtag() {
        assert_eq!(parse_document_id("#1").unwrap(), 1);
        assert_eq!(parse_document_id("#42").unwrap(), 42);
        assert_eq!(parse_document_id("  #10  ").unwrap(), 10);
    }

    #[test]
    fn test_parse_document_id_prefixed() {
        assert_eq!(parse_document_id("doc:1").unwrap(), 1);
        assert_eq!(parse_document_id("doc:42").unwrap(), 42);
        assert_eq!(parse_document_id("  doc:10  ").unwrap(), 10);
    }

    #[test]
    fn test_parse_document_id_invalid() {
        assert!(parse_document_id("abc").is_err());
        assert!(parse_document_id("").is_err());
        assert!(parse_document_id("doc:abc").is_err());
        assert!(parse_document_id("#abc").is_err());
    }
}
