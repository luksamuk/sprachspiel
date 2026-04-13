//! Chat commands - handles internal REPL commands
//!
//! Parses and executes commands like /quit, /new, /model, etc.

use super::session::ChatSession;
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
    after_doc.trim().parse::<i64>().map_err(|_| {
        format!(
            "Invalid document ID '{}'. Use: #N, doc:N, or just N",
            trimmed
        )
    })
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
    FactAdd { content: String, global: bool },
    /// List facts
    FactList { global: bool },
    /// Remove a fact by ID
    FactRemove { id: i64 },
    /// Search facts
    FactSearch {
        query: String,
        global: bool,
        limit: usize,
    },
    /// Add a new todo task
    TodoAdd {
        description: String,
        priority: Option<String>,
        tags: Option<String>,
    },
    /// List todo tasks
    TodoList { filter: Option<String> },
    /// Update todo task status
    TodoUpdate { id: usize, status: String },
    /// Get a single todo task by ID
    TodoGet { id: usize },
    /// Edit a todo task
    TodoEdit {
        id: usize,
        description: Option<String>,
        priority: Option<String>,
        tags: Option<String>,
    },
    /// Delete a todo task
    TodoDelete { id: usize },
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
    NoteList { global: bool, page: Option<usize> },
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
    DocumentList { global: bool },
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
                        return Err(
                            "Error: Title cannot contain newlines. Remove line breaks from title."
                                .to_string(),
                        );
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
                return Err(
                    "Error: Title cannot contain newlines. Remove line breaks from title."
                        .to_string(),
                );
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

/// Parse todo add arguments, extracting --priority and --tags flags.
///
/// Format: "description text --priority high --tags bug,urgent"
/// Returns (description, priority, tags)
fn parse_todo_add_args(args: &str) -> (String, Option<String>, Option<String>) {
    let mut parts: Vec<String> = Vec::new();
    let mut priority: Option<String> = None;
    let mut tags: Option<String> = None;

    let tokens: Vec<&str> = args.split_whitespace().collect();
    let mut i = 0;

    while i < tokens.len() {
        if (tokens[i] == "--priority" || tokens[i] == "-p") && i + 1 < tokens.len() {
            priority = Some(tokens[i + 1].to_string());
            i += 2;
            continue;
        } else if (tokens[i] == "--tags" || tokens[i] == "-t") && i + 1 < tokens.len() {
            tags = Some(tokens[i + 1].to_string());
            i += 2;
            continue;
        }
        parts.push(tokens[i].to_string());
        i += 1;
    }

    let description = parts.join(" ").trim().to_string();
    (description, priority, tags)
}

/// Parse a task ID from a string, returning a helpful error message on failure.
fn parse_task_id_str(input: &str) -> Result<usize, String> {
    input
        .trim()
        .parse::<usize>()
        .map_err(|_| "Invalid task ID. Must be a number.".to_string())
}

/// Parse todo subcommand arguments into a ChatCommand.
///
/// Extracted from the main parse_command to reduce complexity.
fn parse_todo_subcommand(subcmd: &str, subargs: &str) -> Result<ChatCommand, String> {
    match subcmd {
        "add" | "a" => {
            if subargs.is_empty() {
                return Err(
                    "Usage: /todo add <description> [--priority <p>] [--tags <t1,t2>]".to_string(),
                );
            }
            let (description, priority, tags) = parse_todo_add_args(subargs);
            Ok(ChatCommand::TodoAdd {
                description,
                priority,
                tags,
            })
        }
        "list" | "l" => {
            let filter = if subargs.is_empty() {
                None
            } else {
                Some(subargs.trim().to_string())
            };
            Ok(ChatCommand::TodoList { filter })
        }
        "get" | "g" => {
            if subargs.is_empty() {
                return Err("Usage: /todo get <id>".to_string());
            }
            let id = parse_task_id_str(subargs)?;
            Ok(ChatCommand::TodoGet { id })
        }
        "update" | "u" => {
            let update_parts: Vec<&str> = subargs.splitn(2, ' ').collect();
            if update_parts.len() < 2 {
                return Err("Usage: /todo update <id> <status>".to_string());
            }
            let id = parse_task_id_str(update_parts[0])?;
            let status = update_parts[1].trim().to_string();
            Ok(ChatCommand::TodoUpdate { id, status })
        }
        "edit" | "e" => {
            let edit_parts: Vec<&str> = subargs.splitn(2, ' ').collect();
            if edit_parts.is_empty() || edit_parts[0].is_empty() {
                return Err(
                    "Usage: /todo edit <id> [--priority <p>] [--tags <t1,t2>] [description]"
                        .to_string(),
                );
            }
            let id = parse_task_id_str(edit_parts[0])?;
            let rest = edit_parts.get(1).copied().unwrap_or("");
            let (desc, priority, tags) = parse_todo_add_args(rest);
            let description = if desc.is_empty() { None } else { Some(desc) };
            Ok(ChatCommand::TodoEdit {
                id,
                description,
                priority,
                tags,
            })
        }
        "delete" | "d" | "del" => {
            if subargs.is_empty() {
                return Err("Usage: /todo delete <id>".to_string());
            }
            let id = parse_task_id_str(subargs)?;
            Ok(ChatCommand::TodoDelete { id })
        }
        "clear-done" | "cd" => Ok(ChatCommand::TodoClearDone),
        "clear-all" | "ca" => Ok(ChatCommand::TodoClearAll),
        _ => Err("Usage: /todo <add|list|get|update|edit|delete|clear-done|clear-all>".to_string()),
    }
}

/// Parse fact subcommand arguments into a ChatCommand.
///
/// Extracted from the main parse_command to reduce complexity.
/// Handles: prune, add, list, remove, search.
fn parse_fact_subcommand(subcmd: &str, subargs: &str) -> Result<ChatCommand, String> {
    match subcmd {
        "prune" | "p" => Ok(ChatCommand::FactPrune),
        "add" | "a" => {
            if subargs.is_empty() {
                return Err("Usage: /fact add <content> [--global]".to_string());
            }
            let global = subargs.trim().ends_with(" --global");
            let content = if global {
                subargs
                    .trim()
                    .strip_suffix("--global")
                    .unwrap_or(subargs.trim())
                    .trim()
                    .to_string()
            } else {
                subargs.trim().to_string()
            };
            if content.is_empty() {
                return Err("Usage: /fact add <content> [--global]".to_string());
            }
            Ok(ChatCommand::FactAdd { content, global })
        }
        "list" | "l" => {
            let global = subargs.trim() == "--global";
            Ok(ChatCommand::FactList { global })
        }
        "remove" | "r" => {
            if subargs.is_empty() {
                return Err("Usage: /fact remove <id>".to_string());
            }
            match subargs.trim().parse::<i64>() {
                Ok(id) => Ok(ChatCommand::FactRemove { id }),
                Err(_) => Err("Invalid fact ID. Must be a number.".to_string()),
            }
        }
        "search" | "s" => {
            if subargs.is_empty() {
                return Err("Usage: /fact search <query> [--global] [limit]".to_string());
            }
            let global = subargs.contains("--global");
            let args_without_global = subargs.replace("--global", "");
            let args_trimmed = args_without_global.trim();
            let parts: Vec<&str> = args_trimmed.splitn(2, ' ').collect();
            let query = parts.first().unwrap_or(&"").to_string();
            let limit: usize = parts
                .get(1)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(10);
            if query.is_empty() {
                return Err("Usage: /fact search <query> [--global] [limit]".to_string());
            }
            Ok(ChatCommand::FactSearch {
                query,
                global,
                limit,
            })
        }
        _ => Err("Usage: /fact <add|list|remove|search|prune>".to_string()),
    }
}

/// Parse note subcommand arguments into a ChatCommand.
///
/// Extracted from the main parse_command to reduce complexity.
/// Handles: add, list, show, edit, delete, search.
fn parse_note_subcommand(subcmd: &str, subargs: &str) -> Result<ChatCommand, String> {
    match subcmd {
        "add" | "a" => {
            if subargs.is_empty() {
                return Err("Usage: /note add <content> [--title <title>] [--global]".to_string());
            }

            // Parse note arguments with proper quote handling
            match parse_note_add(subargs) {
                Ok((content, title, global)) => {
                    if content.is_empty() {
                        return Err(
                            "Usage: /note add <content> [--title <title>] [--global]".to_string()
                        );
                    }
                    Ok(ChatCommand::NoteAdd {
                        content,
                        title,
                        global,
                    })
                }
                Err(e) => Err(e),
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
                        return Err(
                            "Page must be >= 1. Use /note list 1 for first page.".to_string()
                        );
                    }
                    page = Some(p);
                }
            }
            Ok(ChatCommand::NoteList { global, page })
        }
        "show" | "s" => {
            if subargs.is_empty() {
                return Err("Usage: /note show <id>".to_string());
            }
            match subargs.trim().parse::<i64>() {
                Ok(id) => Ok(ChatCommand::NoteShow { id }),
                Err(_) => Err("Invalid note ID. Must be a number.".to_string()),
            }
        }
        "edit" | "e" => {
            let edit_parts: Vec<&str> = subargs.splitn(2, ' ').collect();
            if edit_parts.len() < 2 {
                return Err(
                    "Usage: /note edit <id> [--title <title>] [--content <content>]".to_string(),
                );
            }
            let id: i64 = match edit_parts[0].trim().parse() {
                Ok(id) => id,
                Err(_) => return Err("Invalid note ID. Must be a number.".to_string()),
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
                return Err(
                    "Usage: /note edit <id> [--title <title>] [--content <content>]".to_string(),
                );
            }
            Ok(ChatCommand::NoteEdit { id, title, content })
        }
        "delete" | "d" => {
            if subargs.is_empty() {
                return Err("Usage: /note delete <id>".to_string());
            }
            match subargs.trim().parse::<i64>() {
                Ok(id) => Ok(ChatCommand::NoteDelete { id }),
                Err(_) => Err("Invalid note ID. Must be a number.".to_string()),
            }
        }
        "search" | "f" => {
            if subargs.is_empty() {
                return Err("Usage: /note search <query> [--global] [limit]".to_string());
            }
            let global = subargs.contains("--global");
            let args_without_global = subargs.replace("--global", "");
            let args_trimmed = args_without_global.trim();
            let parts: Vec<&str> = args_trimmed.splitn(2, ' ').collect();
            let query = parts.first().unwrap_or(&"").to_string();
            let limit: usize = parts
                .get(1)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(10);
            if query.is_empty() {
                return Err("Usage: /note search <query> [--global] [limit]".to_string());
            }
            Ok(ChatCommand::NoteSearch {
                query,
                global,
                limit,
            })
        }
        _ => Err("Usage: /note <add|list|show|edit|delete|search>".to_string()),
    }
}

/// Parse document subcommand arguments into a ChatCommand.
///
/// Extracted from the main parse_command to reduce complexity.
/// Handles: import, list, show, delete.
fn parse_doc_subcommand(subcmd: &str, subargs: &str) -> Result<ChatCommand, String> {
    match subcmd {
        "import" | "i" => {
            if subargs.is_empty() {
                return Err("Usage: /doc import <path> [--global] [--nowait]".to_string());
            }
            let global = subargs.contains("--global");
            let nowait = subargs.contains("--nowait");
            let path = subargs
                .replace("--global", "")
                .replace("--nowait", "")
                .trim()
                .to_string();
            if path.is_empty() {
                return Err("Usage: /doc import <path> [--global] [--nowait]".to_string());
            }
            Ok(ChatCommand::DocumentImport {
                path,
                global,
                nowait,
            })
        }
        "list" | "l" => {
            let global = subargs.contains("--global");
            Ok(ChatCommand::DocumentList { global })
        }
        "show" | "s" => {
            if subargs.is_empty() {
                return Err("Usage: /doc show <id>".to_string());
            }
            match parse_document_id(subargs.trim()) {
                Ok(id) => Ok(ChatCommand::DocumentShow { id }),
                Err(e) => Err(e),
            }
        }
        "delete" | "d" | "remove" | "rm" => {
            if subargs.is_empty() {
                return Err("Usage: /doc delete <id>".to_string());
            }
            match parse_document_id(subargs.trim()) {
                Ok(id) => Ok(ChatCommand::DocumentDelete { id }),
                Err(e) => Err(e),
            }
        }
        _ => Err("Usage: /doc <import|list|show|delete>".to_string()),
    }
}

/// Parse session subcommand arguments into a ChatCommand.
///
/// Extracted from the main parse_command to reduce complexity.
/// Returns canonical ChatCommand variants (New, Load, List, Save, Forget).
fn parse_session_subcommand(subcmd: &str, subargs: &str) -> Result<ChatCommand, String> {
    match subcmd {
        "new" => Ok(ChatCommand::Session {
            subcommand: SessionSubcommand::New,
        }),
        "load" => {
            if subargs.is_empty() {
                return Err("Usage: /session load <name>".to_string());
            }
            Ok(ChatCommand::Session {
                subcommand: SessionSubcommand::Load {
                    name: subargs.trim().to_string(),
                },
            })
        }
        "list" => Ok(ChatCommand::Session {
            subcommand: SessionSubcommand::List,
        }),
        "save" => {
            let name = if subargs.is_empty() {
                None
            } else {
                Some(subargs.trim().to_string())
            };
            Ok(ChatCommand::Session {
                subcommand: SessionSubcommand::Save { name },
            })
        }
        "forget" => Ok(ChatCommand::Session {
            subcommand: SessionSubcommand::Forget,
        }),
        _ => Err("Usage: /session <new|load|list|save|forget>".to_string()),
    }
}

/// Map 2-letter fact shortcuts to their (subcommand, subargs) equivalent.
fn map_fact_shortcut<'a>(cmd: &str, args: &'a str) -> (&'static str, &'a str) {
    match cmd {
        "fp" => ("prune", ""),
        "fa" => ("add", args),
        "fl" => ("list", args),
        "fr" => ("remove", args),
        "fs" => ("search", args),
        _ => unreachable!(),
    }
}

/// Map 2-letter note shortcuts to their (subcommand, subargs) equivalent.
fn map_note_shortcut<'a>(cmd: &str, args: &'a str) -> (&'static str, &'a str) {
    match cmd {
        "na" => ("add", args),
        "nl" => ("list", args),
        "ns" => ("show", args),
        "nd" => ("delete", args),
        _ => unreachable!(),
    }
}

/// Map 2-letter doc shortcuts to their (subcommand, subargs) equivalent.
fn map_doc_shortcut<'a>(cmd: &str, args: &'a str) -> (&'static str, &'a str) {
    match cmd {
        "di" => ("import", args),
        "dl" => ("list", args),
        "ds" => ("show", args),
        "dd" => ("delete", args),
        _ => unreachable!(),
    }
}

/// Map 2-letter todo shortcuts to their (subcommand, subargs) equivalent.
fn map_todo_shortcut<'a>(cmd: &str, args: &'a str) -> (&'static str, &'a str) {
    match cmd {
        "ta" => ("add", args),
        "tl" => ("list", ""),
        "tu" => ("update", args),
        _ => unreachable!(),
    }
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
        "reindex" => ChatCommand::Reindex,
        "retrieval" => ChatCommand::Retrieval,
        "fact" => {
            let subcmd_parts: Vec<&str> = args.splitn(2, ' ').collect();
            let subcmd = subcmd_parts.first().unwrap_or(&"");
            let subargs = subcmd_parts.get(1).copied().unwrap_or("");
            match parse_fact_subcommand(subcmd, subargs) {
                Ok(cmd) => cmd,
                Err(e) => return Some(Err(e)),
            }
        }
        "fp" | "fa" | "fl" | "fr" | "fs" => {
            let (subcmd, subargs) = map_fact_shortcut(cmd, args);
            match parse_fact_subcommand(subcmd, subargs) {
                Ok(cmd) => cmd,
                Err(e) => return Some(Err(e)),
            }
        }
        "todo" => {
            let subcmd_parts: Vec<&str> = args.splitn(2, ' ').collect();
            let subcmd = subcmd_parts.first().unwrap_or(&"");
            let subargs = subcmd_parts.get(1).copied().unwrap_or("");
            match parse_todo_subcommand(subcmd, subargs) {
                Ok(cmd) => cmd,
                Err(e) => return Some(Err(e)),
            }
        }
        "ta" | "tl" | "tu" => {
            let (subcmd, subargs) = map_todo_shortcut(cmd, args);
            match parse_todo_subcommand(subcmd, subargs) {
                Ok(cmd) => cmd,
                Err(e) => return Some(Err(e)),
            }
        }
        "note" | "no" => {
            let subcmd_parts: Vec<&str> = args.splitn(2, ' ').collect();
            let subcmd = subcmd_parts.first().unwrap_or(&"");
            let subargs = subcmd_parts.get(1).copied().unwrap_or("");
            match parse_note_subcommand(subcmd, subargs) {
                Ok(cmd) => cmd,
                Err(e) => return Some(Err(e)),
            }
        }
        "na" | "nl" | "ns" | "nd" => {
            let (subcmd, subargs) = map_note_shortcut(cmd, args);
            match parse_note_subcommand(subcmd, subargs) {
                Ok(cmd) => cmd,
                Err(e) => return Some(Err(e)),
            }
        }
        "doc" | "document" | "docs" | "documents" => {
            let subcmd_parts: Vec<&str> = args.splitn(2, ' ').collect();
            let subcmd = subcmd_parts.first().unwrap_or(&"list");
            let subargs = subcmd_parts.get(1).copied().unwrap_or("");
            match parse_doc_subcommand(subcmd, subargs) {
                Ok(cmd) => cmd,
                Err(e) => return Some(Err(e)),
            }
        }
        "di" | "dl" | "ds" | "dd" => {
            let (subcmd, subargs) = map_doc_shortcut(cmd, args);
            match parse_doc_subcommand(subcmd, subargs) {
                Ok(cmd) => cmd,
                Err(e) => return Some(Err(e)),
            }
        }
        "session" => {
            let subcmd_parts: Vec<&str> = args.splitn(2, ' ').collect();
            let subcmd = subcmd_parts.first().unwrap_or(&"");
            let subargs = subcmd_parts.get(1).copied().unwrap_or("");
            match parse_session_subcommand(subcmd, subargs) {
                Ok(cmd) => cmd,
                Err(e) => return Some(Err(e)),
            }
        }
        // Dynamic skill commands: /<skill-name> [args...]
        // Check if command matches a skill name (e.g., /document-processing)
        _ => {
            // Try to match against available skill names
            let skill_names = crate::skills::get_available_skill_names();
            if skill_names.iter().any(|s| s == cmd) {
                ChatCommand::Skill {
                    name: cmd.to_string(),
                }
            } else {
                return Some(Err(format!(
                    "Unknown command: /{}. Use /help for available commands.",
                    cmd
                )));
            }
        }
    };

    Some(Ok(command))
}

/// Print help message
pub fn print_help() {
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
  /todo add <description> [--priority <p>] [--tags <t1,t2>]    Add a new task
  /todo list [filter]                                            List tasks (filter: status/priority/#tag)
  /todo get <id>                                                 Get task details
  /todo update <id> <status>                                    Update task status (pending|in_progress|done)
  /todo edit <id> [--priority <p>] [--tags <t1,t2>] [desc]     Edit task details
  /todo delete <id>                                              Delete a task
  /todo clear-done                                               Clear completed tasks
  /todo clear-all                                                Clear all tasks

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
