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

/// Scope filter for `/fact list` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactListScope {
    /// Show Global + Project facts separated by sections
    All,
    /// Show only Global-scope facts
    Global,
    /// Show only Project-scope facts
    Project,
}

/// Parsed chat command
#[derive(Debug, Clone)]
pub enum ChatCommand {
    /// Exit the chat session
    Quit,
    /// Start a new conversation session
    New,
    /// Forget everything (clear + delete from database)
    /// Requires --yes flag to confirm destructive operation
    Forget { confirmed: bool },
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
    FactList { scope: FactListScope },
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
    /// List available skills
    SkillList,
    /// Activate a skill by name
    Skill { name: String },
    /// Run OCR on an image file (optional mode: text, table, figure, formula)
    Ocr { path: String, mode: Option<String> },
    /// Analyze image(s) with vision model
    Vision {
        paths: Vec<String>,
        prompt: Option<String>,
    },
    /// Translate text between languages
    Translate { lang_pair: String, text: String },
    /// Summarize text
    Summarize { text: String },
    /// Give feedback on an assistant message
    Feedback {
        signal_type: crate::feedback::types::FeedbackSignalType,
        item_id: Option<i64>,
        correction_text: Option<String>,
    },
    /// Prune content based on decay/importance
    ContentPrune,
}

/// Export format for /export command
#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    Markdown,
    Json,
}

/// Parse note add command with proper quote handling
///
/// Handles:
/// - `/note add content --title "Title with spaces"`
/// - `/note add "content with spaces" --title Title`
/// - `/note add --title "Title" content`
/// - `/note add "test\nmultiline" --title Test` (expands \n inside quotes)
/// - Escaped -- as \-\-
#[allow(clippy::too_many_lines)] // State-machine parser: inherently linear character-by-character logic.
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
            let trimmed = subargs.trim();
            if trimmed == "--global" {
                Ok(ChatCommand::FactList {
                    scope: FactListScope::Global,
                })
            } else if trimmed == "--project" {
                Ok(ChatCommand::FactList {
                    scope: FactListScope::Project,
                })
            } else {
                // Default: show all scopes separated by sections
                Ok(ChatCommand::FactList {
                    scope: FactListScope::All,
                })
            }
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

/// Parse content subcommand arguments into a ChatCommand.
///
/// Extracted from the main parse_command to reduce complexity.
/// Handles: prune.
fn parse_content_subcommand(subcmd: &str, _subargs: &str) -> Result<ChatCommand, String> {
    match subcmd {
        "prune" | "p" => Ok(ChatCommand::ContentPrune),
        _ => Err("Usage: /content prune".to_string()),
    }
}

/// Parse note subcommand arguments into a ChatCommand.
///
/// Extracted from the main parse_command to reduce complexity.
/// Handles: add, list, show, edit, delete, search.
#[allow(clippy::too_many_lines)] // Command dispatch table: each arm is linear input parsing.
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
                #[expect(clippy::unwrap_used)]
                // has_title/has_content verified by .contains() above
                let title_idx = rest.find("--title").unwrap();
                #[expect(clippy::unwrap_used)]
                // has_title/has_content verified by .contains() above
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
                #[expect(clippy::unwrap_used)] // has_title verified by .contains() above
                let title_idx = rest.find("--title").unwrap();
                let title = rest[title_idx + 7..].trim().to_string();
                (Some(title), None)
            } else if has_content {
                #[expect(clippy::unwrap_used)] // has_content verified by .contains() above
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

/// Parse feedback subcommand arguments into a ChatCommand.
///
/// Handles signal types: good, bad, correction.
/// Optional target: msg:<id> to target a specific message.
///
/// Examples:
///   `/feedback good`               — positive signal on last assistant message
///   `/feedback bad`                — negative signal on last assistant message
///   `/feedback correction:fix text` — correction on last assistant message
///   `/feedback msg:42 good`        — positive signal on specific message
fn parse_feedback_subcommand(subcmd: &str, subargs: &str) -> Result<ChatCommand, String> {
    use crate::feedback::types::FeedbackSignalType;
    use std::str::FromStr;

    // subcmd is the first argument after /feedback
    // subargs is everything after that
    if subcmd.is_empty() {
        return Err("Usage: /feedback <good|bad|correction:text> [msg:id]".to_string());
    }

    // Check if first arg starts with msg: — parse item_id then signal type
    if let Some(id_str) = subcmd.strip_prefix("msg:") {
        let item_id: i64 = match id_str.parse() {
            Ok(id) => id,
            Err(_) => {
                return Err(format!(
                    "Invalid message ID '{}'. Use msg:<number> (e.g., msg:42).",
                    id_str
                ));
            }
        };

        // Need a signal type after msg:id
        if subargs.is_empty() {
            return Err(format!(
                "Usage: /feedback msg:{} <good|bad|correction:text>",
                item_id
            ));
        }

        // Parse signal type from subargs
        let parts: Vec<&str> = subargs.splitn(2, ' ').collect();
        let signal_str = parts.first().unwrap_or(&"");
        let remainder = parts.get(1).copied().unwrap_or("");

        // Check for correction: prefix
        if let Some(correction_text) = signal_str.strip_prefix("correction:") {
            let text = if correction_text.is_empty() {
                remainder.trim().to_string()
            } else {
                format!("{} {}", correction_text, remainder.trim())
                    .trim()
                    .to_string()
            };
            if text.is_empty() {
                return Err(
                    "Correction requires text. Usage: /feedback msg:<id> correction:<text>"
                        .to_string(),
                );
            }
            return Ok(ChatCommand::Feedback {
                signal_type: FeedbackSignalType::Correction,
                item_id: Some(item_id),
                correction_text: Some(text),
            });
        }

        let signal_type = FeedbackSignalType::from_str(signal_str)?;
        return Ok(ChatCommand::Feedback {
            signal_type,
            item_id: Some(item_id),
            correction_text: None,
        });
    }

    // No msg: prefix — parse signal_type from first arg
    // Check for correction: prefix
    if let Some(correction_text) = subcmd.strip_prefix("correction:") {
        let text = if correction_text.is_empty() {
            subargs.trim().to_string()
        } else {
            format!("{} {}", correction_text, subargs.trim())
                .trim()
                .to_string()
        };
        if text.is_empty() {
            return Err("Correction requires text. Usage: /feedback correction:<text>".to_string());
        }
        return Ok(ChatCommand::Feedback {
            signal_type: FeedbackSignalType::Correction,
            item_id: None,
            correction_text: Some(text),
        });
    }

    // Parse as good/bad signal type
    let signal_type = FeedbackSignalType::from_str(subcmd)?;
    Ok(ChatCommand::Feedback {
        signal_type,
        item_id: None,
        correction_text: None,
    })
}

/// Parse session subcommand arguments into a ChatCommand.
///
/// Extracted from the main parse_command to reduce complexity.
/// Returns canonical ChatCommand variants (New, Load, List, Save, Forget).
fn parse_session_subcommand(subcmd: &str, subargs: &str) -> Result<ChatCommand, String> {
    match subcmd {
        "new" => Ok(ChatCommand::New),
        "load" => {
            if subargs.is_empty() {
                return Err("Usage: /session load <name>".to_string());
            }
            Ok(ChatCommand::Load {
                name: subargs.trim().to_string(),
            })
        }
        "list" => Ok(ChatCommand::List),
        "save" => {
            let name = if subargs.is_empty() {
                None
            } else {
                Some(subargs.trim().to_string())
            };
            Ok(ChatCommand::Save { name })
        }
        "forget" => {
            let confirmed = subargs.trim() == "--yes";
            if !confirmed && !subargs.trim().is_empty() && subargs.trim() != "--yes" {
                return Err("Usage: /session forget [--yes]".to_string());
            }
            Ok(ChatCommand::Forget { confirmed })
        }
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
        "tg" => ("get", args),
        "te" => ("edit", args),
        "td" => ("delete", args),
        "tcd" => ("clear-done", ""),
        "tca" => ("clear-all", ""),
        _ => unreachable!(),
    }
}

/// Parse a command string
/// Parse a command from user input.
///
/// Returns `None` if the input doesn't start with `/`.
/// Returns `Some(Ok(ChatCommand))` for valid commands.
/// Returns `Some(Err(msg))` for invalid input with a usage hint.
#[allow(clippy::too_many_lines)] // Command dispatch table: each arm is linear input parsing.
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
        "forget" => {
            let confirmed = args.trim() == "--yes";
            if !confirmed && !args.trim().is_empty() && args.trim() != "--yes" {
                return Some(Err("Usage: /forget [--yes]".to_string()));
            }
            ChatCommand::Forget { confirmed }
        }
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
        "search" | "find" | "f" => {
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
        "ta" | "tl" | "tu" | "tg" | "te" | "td" | "tcd" | "tca" => {
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
        "skill" | "sk" => {
            if args.trim().is_empty() {
                // /skill (no args) → list available skills
                ChatCommand::SkillList
            } else {
                // /skill <name> → activate a skill
                let skill_name = args.trim().to_string();
                let skill_names = crate::skills::get_available_skill_names();
                if skill_names.iter().any(|s| s == &skill_name) {
                    ChatCommand::Skill { name: skill_name }
                } else {
                    return Some(Err(format!(
                        "Unknown skill: '{}'. Available skills: {}",
                        skill_name,
                        skill_names.join(", ")
                    )));
                }
            }
        }
        "ocr" => {
            if args.is_empty() {
                return Some(Err("Usage: /ocr <file> [mode]".to_string()));
            }
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            let path = parts.first().unwrap_or(&"").to_string();
            let mode = parts
                .get(1)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            ChatCommand::Ocr { path, mode }
        }
        "vision" => {
            if args.is_empty() {
                return Some(Err("Usage: /vision <path> [prompt]".to_string()));
            }
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            let path = parts.first().unwrap_or(&"").to_string();
            let prompt = parts
                .get(1)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            ChatCommand::Vision {
                paths: vec![path],
                prompt,
            }
        }
        "translate" | "tr" => {
            if args.is_empty() {
                return Some(Err("Usage: /translate <source:target> <text>".to_string()));
            }
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            let lang_pair = parts.first().unwrap_or(&"").to_string();
            let text = parts
                .get(1)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if text.is_empty() {
                return Some(Err("Usage: /translate <source:target> <text>".to_string()));
            }
            ChatCommand::Translate { lang_pair, text }
        }
        "summarize" | "sum" => {
            if args.is_empty() {
                return Some(Err("Usage: /summarize <text>".to_string()));
            }
            ChatCommand::Summarize {
                text: args.trim().to_string(),
            }
        }
        "feedback" | "fb" | "fg" => {
            if *cmd == "fg" {
                // Shortcut for /feedback good
                ChatCommand::Feedback {
                    signal_type: crate::feedback::types::FeedbackSignalType::Good,
                    item_id: None,
                    correction_text: None,
                }
            } else {
                let subcmd_parts: Vec<&str> = args.splitn(2, ' ').collect();
                let subcmd = subcmd_parts.first().unwrap_or(&"");
                let subargs = subcmd_parts.get(1).copied().unwrap_or("");
                match parse_feedback_subcommand(subcmd, subargs) {
                    Ok(cmd) => cmd,
                    Err(e) => return Some(Err(e)),
                }
            }
        }

        "content" | "cp" => {
            if *cmd == "cp" {
                // Shortcut for /content prune
                ChatCommand::ContentPrune
            } else {
                let subcmd_parts: Vec<&str> = args.splitn(2, ' ').collect();
                let subcmd = subcmd_parts.first().unwrap_or(&"");
                let subargs = subcmd_parts.get(1).copied().unwrap_or("");
                match parse_content_subcommand(subcmd, subargs) {
                    Ok(cmd) => cmd,
                    Err(e) => return Some(Err(e)),
                }
            }
        }
        _ => {
            return Some(Err(format!(
                "Unknown command: /{}. Use /help for available commands.",
                cmd
            )));
        }
    };

    Some(Ok(command))
}

/// Print help message
pub fn print_help() {
    print!("{}", format_help());
}

/// Format help text as a string (for CommandOutput::HelpText).
///
/// This is the non-printing version of `print_help()`.
pub fn format_help() -> String {
    format!(
        r#"Available commands:
  /quit, /exit     Exit the chat session
  /new, /n         Start a new conversation (previous messages remain searchable)
  /forget [--yes]  Delete conversation completely and start fresh (requires --yes)
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
    /session forget [--yes]  Same as /forget (requires --yes)
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

Feedback:
  /feedback good                    Positive signal on last assistant message
  /feedback bad                     Negative signal on last assistant message
  /feedback correction:fix text      Correction on last assistant message
  /feedback msg:<id> good|bad        Signal on a specific message
  /fb                               Shortcut for /feedback
  /fg                               Shortcut for /feedback good

Content Management:
  /content prune   Prune low-retention content using decay cycle
  /cp              Shortcut for /content prune


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
   /tg = /todo get, /te = /todo edit, /td = /todo delete
   /tcd = /todo clear-done, /tca = /todo clear-all

Skills:
  /skill           List available skills
  /skill <name>    Activate a skill for this session

Subagents:
  /ocr <file> [mode]          Extract text from an image using OCR (modes: text, table, figure, formula)
  /vision <path> [prompt]     Analyze image with vision model
  /translate <src:dst> <text>  Translate text between languages
  /summarize <text>           Summarize text

  Shortcuts: /tr = /translate, /sum = /summarize

Shortcuts:
  /q = /quit, /n = /new, /h = /help
  /m = /model, /s = /system, /l = /load
  /t = /think, /e = /export, /ls = /list, /i = /info
  /r = /retry, /to = /tools-output, /u = /undo
  /ctx = /context, /f = /search (find)
  /sk = /skill
  /fb = /feedback, /fg = /feedback good, /fp = /fact prune, /fa = /fact add
  /fl = /fact list, /fr = /fact remove, /fs = /fact search
  /cp = /content prune
"#
    )
}

/// Print session information
pub fn print_session_info(session: &ChatSession, metrics: Option<&ContextMetrics>) {
    println!("{}", format_session_info(session, metrics));
}

/// Format session information as a string (for CommandOutput).
///
/// This is the non-printing version of `print_session_info()`.
pub fn format_session_info(session: &ChatSession, metrics: Option<&ContextMetrics>) -> String {
    let name = session.name.as_deref().unwrap_or("unnamed");
    let project = session.project_id.as_deref().unwrap_or("none");
    let created = session.created_at.format("%Y-%m-%d %H:%M:%S");
    let updated = session.updated_at.format("%Y-%m-%d %H:%M:%S");

    let mut output = String::new();
    output.push_str("Session Information:\n");
    output.push_str(&format!("  ID:        {}\n", session.id));
    output.push_str(&format!("  Name:      {}\n", name));
    output.push_str(&format!("  Project:   {}\n", project));
    output.push_str(&format!("  Model:     {}\n", session.model));
    output.push_str(&format!(
        "  Messages:  {} (total)\n",
        session.messages.len()
    ));

    if session.has_compacted_messages() {
        output.push_str(&format!(
            "  Compacted: {} messages summarized\n",
            session.compacted_message_count()
        ));
    }

    if let Some(m) = metrics {
        output.push_str(&format!(
            "  Context:   {} / {} tokens ({:.1}%)\n",
            m.total_tokens,
            m.context_window,
            m.utilization * 100.0
        ));
    }

    output.push_str(&format!("  Think:     {}\n", session.think));
    output.push_str(&format!("  Tools:     {}\n", session.tools));

    // Show sandbox status
    let sandbox_status = crate::external::get_sandbox_status();
    output.push_str(&format!("  Sandbox:   {}\n", sandbox_status));

    output.push_str(&format!("  Anonymous: {}\n", session.anonymous));
    output.push_str(&format!("  Created:   {}\n", created));
    output.push_str(&format!("  Updated:   {}\n", updated));

    if let Some(ref prompt) = session.system_prompt {
        let preview = crate::utils::truncate_chars(prompt, 100);
        output.push_str(&format!("  System:    {}\n", preview));
    }

    output
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

    // =========================================================
    // Tests for extracted subcommand parsers
    // =========================================================

    // --- Session subcommand parser ---

    #[test]
    fn test_parse_session_subcommand_new() {
        let cmd = parse_session_subcommand("new", "").unwrap();
        assert!(matches!(cmd, ChatCommand::New));
    }

    #[test]
    fn test_parse_session_subcommand_load() {
        let cmd = parse_session_subcommand("load", "my-session").unwrap();
        assert!(matches!(cmd, ChatCommand::Load { ref name } if name == "my-session"));
    }

    #[test]
    fn test_parse_session_subcommand_load_empty() {
        assert!(parse_session_subcommand("load", "").is_err());
    }

    #[test]
    fn test_parse_session_subcommand_list() {
        let cmd = parse_session_subcommand("list", "").unwrap();
        assert!(matches!(cmd, ChatCommand::List));
    }

    #[test]
    fn test_parse_session_subcommand_save_with_name() {
        let cmd = parse_session_subcommand("save", "my-save").unwrap();
        assert!(
            matches!(cmd, ChatCommand::Save { ref name } if name.as_deref() == Some("my-save"))
        );
    }

    #[test]
    fn test_parse_session_subcommand_save_without_name() {
        let cmd = parse_session_subcommand("save", "").unwrap();
        assert!(matches!(cmd, ChatCommand::Save { ref name } if name.is_none()));
    }

    #[test]
    fn test_parse_session_subcommand_forget() {
        let cmd = parse_session_subcommand("forget", "").unwrap();
        assert!(matches!(cmd, ChatCommand::Forget { confirmed: false }));
    }

    #[test]
    fn test_parse_session_subcommand_forget_with_yes() {
        let cmd = parse_session_subcommand("forget", "--yes").unwrap();
        assert!(matches!(cmd, ChatCommand::Forget { confirmed: true }));
    }

    #[test]
    fn test_parse_session_subcommand_forget_invalid_arg() {
        assert!(parse_session_subcommand("forget", "nope").is_err());
    }

    #[test]
    fn test_parse_session_subcommand_invalid() {
        assert!(parse_session_subcommand("unknown", "").is_err());
    }

    // --- Fact subcommand parser ---

    #[test]
    fn test_parse_fact_subcommand_prune() {
        let cmd = parse_fact_subcommand("prune", "").unwrap();
        assert!(matches!(cmd, ChatCommand::FactPrune));
    }

    #[test]
    fn test_parse_fact_subcommand_prune_shortcut() {
        let cmd = parse_fact_subcommand("p", "").unwrap();
        assert!(matches!(cmd, ChatCommand::FactPrune));
    }

    #[test]
    fn test_parse_fact_subcommand_add() {
        let cmd = parse_fact_subcommand("add", "some fact").unwrap();
        assert!(
            matches!(cmd, ChatCommand::FactAdd { ref content, global: false } if content == "some fact")
        );
    }

    #[test]
    fn test_parse_fact_subcommand_add_shortcut() {
        let cmd = parse_fact_subcommand("a", "some fact").unwrap();
        assert!(
            matches!(cmd, ChatCommand::FactAdd { ref content, global: false } if content == "some fact")
        );
    }

    #[test]
    fn test_parse_fact_subcommand_add_global() {
        let cmd = parse_fact_subcommand("add", "global fact --global").unwrap();
        assert!(
            matches!(cmd, ChatCommand::FactAdd { ref content, global: true } if content == "global fact")
        );
    }

    #[test]
    fn test_parse_fact_subcommand_add_empty() {
        assert!(parse_fact_subcommand("add", "").is_err());
    }

    #[test]
    fn test_parse_fact_subcommand_list() {
        let cmd = parse_fact_subcommand("list", "").unwrap();
        assert!(matches!(
            cmd,
            ChatCommand::FactList {
                scope: FactListScope::All
            }
        ));
    }

    #[test]
    fn test_parse_fact_subcommand_list_global() {
        let cmd = parse_fact_subcommand("list", "--global").unwrap();
        assert!(matches!(
            cmd,
            ChatCommand::FactList {
                scope: FactListScope::Global
            }
        ));
    }

    #[test]
    fn test_parse_fact_subcommand_remove() {
        let cmd = parse_fact_subcommand("remove", "42").unwrap();
        assert!(matches!(cmd, ChatCommand::FactRemove { id: 42 }));
    }

    #[test]
    fn test_parse_fact_subcommand_remove_empty() {
        assert!(parse_fact_subcommand("remove", "").is_err());
    }

    #[test]
    fn test_parse_fact_subcommand_remove_invalid_id() {
        assert!(parse_fact_subcommand("remove", "abc").is_err());
    }

    #[test]
    fn test_parse_fact_subcommand_search() {
        // Note: search parser uses splitn(2, ' '), so "rust" alone -> query="rust", limit=10
        let cmd = parse_fact_subcommand("search", "rust").unwrap();
        assert!(
            matches!(cmd, ChatCommand::FactSearch { ref query, global: false, limit: 10 } if query == "rust")
        );
    }

    #[test]
    fn test_parse_fact_subcommand_search_with_limit() {
        let cmd = parse_fact_subcommand("search", "rust 5").unwrap();
        assert!(
            matches!(cmd, ChatCommand::FactSearch { ref query, global: false, limit: 5 } if query == "rust")
        );
    }

    #[test]
    fn test_parse_fact_subcommand_invalid() {
        assert!(parse_fact_subcommand("unknown", "").is_err());
    }
    // --- Content subcommand parser ---

    #[test]
    fn test_parse_content_prune() {
        let cmd = parse_content_subcommand("prune", "").unwrap();
        assert!(matches!(cmd, ChatCommand::ContentPrune));
    }

    #[test]
    fn test_parse_content_prune_shortcut() {
        let cmd = parse_content_subcommand("p", "").unwrap();
        assert!(matches!(cmd, ChatCommand::ContentPrune));
    }

    #[test]
    fn test_parse_content_error() {
        assert!(parse_content_subcommand("list", "").is_err());
        assert!(parse_content_subcommand("unknown", "").is_err());
    }

    // --- Document subcommand parser ---

    #[test]
    fn test_parse_doc_subcommand_import() {
        let cmd = parse_doc_subcommand("import", "/path/to/file.pdf").unwrap();
        assert!(
            matches!(cmd, ChatCommand::DocumentImport { ref path, global: false, nowait: false } if path == "/path/to/file.pdf")
        );
    }

    #[test]
    fn test_parse_doc_subcommand_import_shortcut() {
        let cmd = parse_doc_subcommand("i", "/path/to/file.pdf").unwrap();
        assert!(
            matches!(cmd, ChatCommand::DocumentImport { ref path, global: false, nowait: false } if path == "/path/to/file.pdf")
        );
    }

    #[test]
    fn test_parse_doc_subcommand_import_with_flags() {
        let cmd = parse_doc_subcommand("import", "/file.pdf --global --nowait").unwrap();
        assert!(
            matches!(cmd, ChatCommand::DocumentImport { ref path, global: true, nowait: true } if path == "/file.pdf")
        );
    }

    #[test]
    fn test_parse_doc_subcommand_import_empty() {
        assert!(parse_doc_subcommand("import", "").is_err());
    }

    #[test]
    fn test_parse_doc_subcommand_list() {
        let cmd = parse_doc_subcommand("list", "").unwrap();
        assert!(matches!(cmd, ChatCommand::DocumentList { global: false }));
    }

    #[test]
    fn test_parse_doc_subcommand_list_global() {
        let cmd = parse_doc_subcommand("list", "--global").unwrap();
        assert!(matches!(cmd, ChatCommand::DocumentList { global: true }));
    }

    #[test]
    fn test_parse_doc_subcommand_show() {
        let cmd = parse_doc_subcommand("show", "5").unwrap();
        assert!(matches!(cmd, ChatCommand::DocumentShow { id: 5 }));
    }

    #[test]
    fn test_parse_doc_subcommand_show_with_prefix() {
        let cmd = parse_doc_subcommand("show", "doc:10").unwrap();
        assert!(matches!(cmd, ChatCommand::DocumentShow { id: 10 }));
    }

    #[test]
    fn test_parse_doc_subcommand_show_empty() {
        assert!(parse_doc_subcommand("show", "").is_err());
    }

    #[test]
    fn test_parse_doc_subcommand_delete() {
        let cmd = parse_doc_subcommand("delete", "3").unwrap();
        assert!(matches!(cmd, ChatCommand::DocumentDelete { id: 3 }));
    }

    #[test]
    fn test_parse_doc_subcommand_delete_shortcuts() {
        let cmd = parse_doc_subcommand("d", "3").unwrap();
        assert!(matches!(cmd, ChatCommand::DocumentDelete { id: 3 }));
        let cmd2 = parse_doc_subcommand("rm", "3").unwrap();
        assert!(matches!(cmd2, ChatCommand::DocumentDelete { id: 3 }));
    }

    #[test]
    fn test_parse_doc_subcommand_invalid() {
        assert!(parse_doc_subcommand("unknown", "").is_err());
    }

    // --- Note subcommand parser ---

    #[test]
    fn test_parse_note_subcommand_add() {
        let cmd = parse_note_subcommand("add", "some note content").unwrap();
        assert!(
            matches!(cmd, ChatCommand::NoteAdd { ref content, ref title, global: false } if content == "some note content" && title.is_none())
        );
    }

    #[test]
    fn test_parse_note_subcommand_add_shortcut() {
        let cmd = parse_note_subcommand("a", "note text").unwrap();
        assert!(matches!(cmd, ChatCommand::NoteAdd { ref content, .. } if content == "note text"));
    }

    #[test]
    fn test_parse_note_subcommand_add_empty() {
        assert!(parse_note_subcommand("add", "").is_err());
    }

    #[test]
    fn test_parse_note_subcommand_list() {
        let cmd = parse_note_subcommand("list", "").unwrap();
        assert!(matches!(
            cmd,
            ChatCommand::NoteList {
                global: false,
                page: None
            }
        ));
    }

    #[test]
    fn test_parse_note_subcommand_list_with_page() {
        let cmd = parse_note_subcommand("list", "2").unwrap();
        assert!(matches!(
            cmd,
            ChatCommand::NoteList {
                global: false,
                page: Some(2)
            }
        ));
    }

    #[test]
    fn test_parse_note_subcommand_list_global_with_page() {
        let cmd = parse_note_subcommand("list", "--global 3").unwrap();
        assert!(matches!(
            cmd,
            ChatCommand::NoteList {
                global: true,
                page: Some(3)
            }
        ));
    }

    #[test]
    fn test_parse_note_subcommand_list_zero_page() {
        assert!(parse_note_subcommand("list", "0").is_err());
    }

    #[test]
    fn test_parse_note_subcommand_show() {
        let cmd = parse_note_subcommand("show", "42").unwrap();
        assert!(matches!(cmd, ChatCommand::NoteShow { id: 42 }));
    }

    #[test]
    fn test_parse_note_subcommand_show_empty() {
        assert!(parse_note_subcommand("show", "").is_err());
    }

    #[test]
    fn test_parse_note_subcommand_show_invalid_id() {
        assert!(parse_note_subcommand("show", "abc").is_err());
    }

    #[test]
    fn test_parse_note_subcommand_delete() {
        let cmd = parse_note_subcommand("delete", "7").unwrap();
        assert!(matches!(cmd, ChatCommand::NoteDelete { id: 7 }));
    }

    #[test]
    fn test_parse_note_subcommand_delete_shortcut() {
        let cmd = parse_note_subcommand("d", "7").unwrap();
        assert!(matches!(cmd, ChatCommand::NoteDelete { id: 7 }));
    }

    #[test]
    fn test_parse_note_subcommand_search() {
        // Note: search parser uses splitn(2, ' '), so single word -> query="query", limit=10
        let cmd = parse_note_subcommand("search", "query").unwrap();
        assert!(
            matches!(cmd, ChatCommand::NoteSearch { ref query, global: false, limit: 10 } if query == "query")
        );
    }

    #[test]
    fn test_parse_note_subcommand_search_shortcut() {
        // Shortcut "f" maps to search
        let cmd = parse_note_subcommand("f", "query").unwrap();
        assert!(matches!(cmd, ChatCommand::NoteSearch { ref query, .. } if query == "query"));
    }

    #[test]
    fn test_parse_note_subcommand_search_empty() {
        assert!(parse_note_subcommand("search", "").is_err());
    }

    #[test]
    fn test_parse_note_subcommand_invalid() {
        assert!(parse_note_subcommand("unknown", "").is_err());
    }

    // --- Todo subcommand parser ---

    #[test]
    fn test_parse_todo_subcommand_add() {
        let cmd = parse_todo_subcommand("add", "Buy groceries").unwrap();
        assert!(
            matches!(cmd, ChatCommand::TodoAdd { ref description, .. } if description == "Buy groceries")
        );
    }

    #[test]
    fn test_parse_todo_subcommand_add_shortcut() {
        let cmd = parse_todo_subcommand("a", "Buy groceries").unwrap();
        assert!(
            matches!(cmd, ChatCommand::TodoAdd { ref description, .. } if description == "Buy groceries")
        );
    }

    #[test]
    fn test_parse_todo_subcommand_add_with_priority() {
        let cmd = parse_todo_subcommand("add", "Fix bug --priority high").unwrap();
        assert!(
            matches!(cmd, ChatCommand::TodoAdd { ref description, ref priority, .. } if description == "Fix bug" && priority.as_deref() == Some("high"))
        );
    }

    #[test]
    fn test_parse_todo_subcommand_add_empty() {
        assert!(parse_todo_subcommand("add", "").is_err());
    }

    #[test]
    fn test_parse_todo_subcommand_list() {
        let cmd = parse_todo_subcommand("list", "").unwrap();
        assert!(matches!(cmd, ChatCommand::TodoList { ref filter } if filter.is_none()));
    }

    #[test]
    fn test_parse_todo_subcommand_list_with_filter() {
        let cmd = parse_todo_subcommand("list", "pending").unwrap();
        assert!(
            matches!(cmd, ChatCommand::TodoList { ref filter } if filter.as_deref() == Some("pending"))
        );
    }

    #[test]
    fn test_parse_todo_subcommand_get() {
        let cmd = parse_todo_subcommand("get", "5").unwrap();
        assert!(matches!(cmd, ChatCommand::TodoGet { id: 5 }));
    }

    #[test]
    fn test_parse_todo_subcommand_get_empty() {
        assert!(parse_todo_subcommand("get", "").is_err());
    }

    #[test]
    fn test_parse_todo_subcommand_update() {
        let cmd = parse_todo_subcommand("update", "1 done").unwrap();
        assert!(matches!(cmd, ChatCommand::TodoUpdate { id: 1, ref status } if status == "done"));
    }

    #[test]
    fn test_parse_todo_subcommand_update_missing_status() {
        assert!(parse_todo_subcommand("update", "1").is_err());
    }

    #[test]
    fn test_parse_todo_subcommand_delete() {
        let cmd = parse_todo_subcommand("delete", "5").unwrap();
        assert!(matches!(cmd, ChatCommand::TodoDelete { id: 5 }));
    }

    #[test]
    fn test_parse_todo_subcommand_delete_shortcuts() {
        let cmd = parse_todo_subcommand("d", "5").unwrap();
        assert!(matches!(cmd, ChatCommand::TodoDelete { id: 5 }));
        let cmd2 = parse_todo_subcommand("del", "5").unwrap();
        assert!(matches!(cmd2, ChatCommand::TodoDelete { id: 5 }));
    }

    #[test]
    fn test_parse_todo_subcommand_clear_done() {
        let cmd = parse_todo_subcommand("clear-done", "").unwrap();
        assert!(matches!(cmd, ChatCommand::TodoClearDone));
    }

    #[test]
    fn test_parse_todo_subcommand_clear_done_shortcut() {
        let cmd = parse_todo_subcommand("cd", "").unwrap();
        assert!(matches!(cmd, ChatCommand::TodoClearDone));
    }

    #[test]
    fn test_parse_todo_subcommand_clear_all() {
        let cmd = parse_todo_subcommand("clear-all", "").unwrap();
        assert!(matches!(cmd, ChatCommand::TodoClearAll));
    }

    #[test]
    fn test_parse_todo_subcommand_clear_all_shortcut() {
        let cmd = parse_todo_subcommand("ca", "").unwrap();
        assert!(matches!(cmd, ChatCommand::TodoClearAll));
    }

    #[test]
    fn test_parse_todo_subcommand_edit() {
        let cmd = parse_todo_subcommand("edit", "3 new description").unwrap();
        assert!(
            matches!(cmd, ChatCommand::TodoEdit { id: 3, ref description, .. } if description.as_deref() == Some("new description"))
        );
    }

    #[test]
    fn test_parse_todo_subcommand_edit_shortcut() {
        let cmd = parse_todo_subcommand("e", "3 updated text").unwrap();
        assert!(matches!(cmd, ChatCommand::TodoEdit { id: 3, .. }));
    }

    #[test]
    fn test_parse_todo_subcommand_edit_missing_id() {
        assert!(parse_todo_subcommand("edit", "").is_err());
    }

    #[test]
    fn test_parse_todo_subcommand_invalid() {
        assert!(parse_todo_subcommand("unknown", "").is_err());
    }

    // --- Shortcut mapping functions ---

    #[test]
    fn test_map_fact_shortcut() {
        assert_eq!(map_fact_shortcut("fp", "x"), ("prune", ""));
        assert_eq!(map_fact_shortcut("fa", "my content"), ("add", "my content"));
        assert_eq!(map_fact_shortcut("fl", ""), ("list", ""));
        assert_eq!(map_fact_shortcut("fr", "5"), ("remove", "5"));
        assert_eq!(map_fact_shortcut("fs", "query"), ("search", "query"));
    }

    #[test]
    fn test_map_note_shortcut() {
        assert_eq!(map_note_shortcut("na", "content"), ("add", "content"));
        assert_eq!(map_note_shortcut("nl", ""), ("list", ""));
        assert_eq!(map_note_shortcut("ns", "5"), ("show", "5"));
        assert_eq!(map_note_shortcut("nd", "5"), ("delete", "5"));
    }

    #[test]
    fn test_map_todo_shortcut() {
        assert_eq!(map_todo_shortcut("ta", "task"), ("add", "task"));
        assert_eq!(map_todo_shortcut("tl", ""), ("list", ""));
        assert_eq!(map_todo_shortcut("tu", "1 done"), ("update", "1 done"));
        assert_eq!(map_todo_shortcut("tg", "5"), ("get", "5"));
        assert_eq!(
            map_todo_shortcut("te", "3 --priority low"),
            ("edit", "3 --priority low")
        );
        assert_eq!(map_todo_shortcut("td", "5"), ("delete", "5"));
        assert_eq!(map_todo_shortcut("tcd", ""), ("clear-done", ""));
        assert_eq!(map_todo_shortcut("tca", ""), ("clear-all", ""));
    }

    // --- Top-level parse_command tests ---

    #[test]
    fn test_parse_forget_no_args() {
        let cmd = parse_command("/forget").unwrap().unwrap();
        assert!(matches!(cmd, ChatCommand::Forget { confirmed: false }));
    }

    #[test]
    fn test_parse_forget_with_yes() {
        let cmd = parse_command("/forget --yes").unwrap().unwrap();
        assert!(matches!(cmd, ChatCommand::Forget { confirmed: true }));
    }

    #[test]
    fn test_parse_forget_invalid_flag() {
        let result = parse_command("/forget --no");
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    #[test]
    fn test_parse_forget_trailing_space() {
        let cmd = parse_command("/forget ").unwrap().unwrap();
        assert!(matches!(cmd, ChatCommand::Forget { confirmed: false }));
    }

    #[test]
    fn test_parse_skill_no_args() {
        let cmd = parse_command("/skill").unwrap().unwrap();
        assert!(matches!(cmd, ChatCommand::SkillList));
    }

    #[test]
    fn test_parse_skill_list_arg_is_skill_name() {
        // "list" is NOT a reserved word — /skill list tries to activate a skill named "list"
        // Since no skill named "list" exists, this returns an error
        let result = parse_command("/skill list");
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
        // To list skills, use /skill (no args)
    }

    #[test]
    fn test_parse_skill_activate_valid() {
        let cmd = parse_command("/skill document-processing")
            .unwrap()
            .unwrap();
        assert!(matches!(cmd, ChatCommand::Skill { ref name } if name == "document-processing"));
    }

    #[test]
    fn test_parse_skill_unknown() {
        let result = parse_command("/skill nonexistent-skill-xyz");
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    #[test]
    fn test_parse_sk_shortcut_no_args() {
        let cmd = parse_command("/sk").unwrap().unwrap();
        assert!(matches!(cmd, ChatCommand::SkillList));
    }

    #[test]
    fn test_parse_sk_shortcut_activate() {
        let cmd = parse_command("/sk document-processing").unwrap().unwrap();
        assert!(matches!(cmd, ChatCommand::Skill { ref name } if name == "document-processing"));
    }

    #[test]
    fn test_parse_skill_name_no_longer_wildcard() {
        // The old wildcard behavior: /document-processing was a valid command.
        // Now it should be an unknown command.
        let result = parse_command("/document-processing");
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    // --- Feedback subcommand parser ---

    use crate::feedback::types::FeedbackSignalType;

    #[test]
    fn test_parse_feedback_good() {
        let result = parse_command("/feedback good");
        assert!(matches!(
            result,
            Some(Ok(ChatCommand::Feedback {
                signal_type: FeedbackSignalType::Good,
                item_id: None,
                correction_text: None
            }))
        ));
    }

    #[test]
    fn test_parse_feedback_bad() {
        let result = parse_command("/feedback bad");
        assert!(matches!(
            result,
            Some(Ok(ChatCommand::Feedback {
                signal_type: FeedbackSignalType::Bad,
                item_id: None,
                correction_text: None
            }))
        ));
    }

    #[test]
    fn test_parse_feedback_correction() {
        let result = parse_command("/feedback correction:fix the capital");
        assert!(matches!(
            result,
            Some(Ok(ChatCommand::Feedback {
                signal_type: FeedbackSignalType::Correction,
                item_id: None,
                correction_text: Some(_)
            }))
        ));
    }

    #[test]
    fn test_parse_feedback_msg_id_good() {
        let result = parse_command("/feedback msg:42 good");
        assert!(matches!(
            result,
            Some(Ok(ChatCommand::Feedback {
                signal_type: FeedbackSignalType::Good,
                item_id: Some(42),
                correction_text: None
            }))
        ));
    }

    #[test]
    fn test_parse_feedback_empty_error() {
        let result = parse_command("/feedback");
        assert!(matches!(result, Some(Err(_))));
    }

    #[test]
    fn test_parse_feedback_msg_no_signal_error() {
        let result = parse_command("/feedback msg:42");
        assert!(matches!(result, Some(Err(_))));
    }

    #[test]
    fn test_parse_feedback_msg_invalid_id_error() {
        let result = parse_command("/feedback msg:abc good");
        assert!(matches!(result, Some(Err(_))));
    }

    #[test]
    fn test_parse_fg_shortcut() {
        use crate::feedback::types::FeedbackSignalType;
        let result = parse_command("/fg");
        assert!(matches!(
            result,
            Some(Ok(ChatCommand::Feedback {
                signal_type: FeedbackSignalType::Good,
                item_id: None,
                correction_text: None,
            }))
        ));
    }
}
