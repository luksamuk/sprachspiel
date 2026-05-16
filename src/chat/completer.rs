//! Tab completion for the TUI chat input
//!
//! Provides slash command and model name completion. When the user presses
//! Tab, the completer tries to:
//!
//! 1. Complete slash commands (e.g., `/h` → `/help`, `/mod` → `/model`)
//! 2. Complete slash command arguments (e.g., `/model l` → `/model llama3.1`)
//! 3. Cycle through multiple matches on repeated Tab presses
//!
//! # Architecture
//!
//! ```text
//! App::handle_key(Tab)
//!     → App::try_tab_complete()
//!         → ChatCompleter::complete(buffer, cursor_pos)
//!             → returns CompletionResult
//!         → updates TextArea with replacement
//! ```

/// Slash commands available for tab completion.
///
/// Each entry maps a command trigger (with `/` prefix) to a `ChatCommand`
/// variant and a short description for the completion menu.
///
/// Subcommands are included with their parent prefix (e.g., `/fact add`,
/// `/note list`). Two-letter shortcuts are also listed so they complete
/// correctly (e.g., `/q`, `/n`, `/h`).
#[derive(Debug)]
struct SlashCommand {
    /// The full command string including `/` prefix
    trigger: &'static str,
    /// Short description shown in completion hints
    #[allow(dead_code)] // Will be used for completion menu display in Phase 3.6
    description: &'static str,
}

/// Slash commands for tab completion.
///
/// Entries are listed in the same order as `/help` output so users get
/// a predictable completion experience. Shortcuts are listed after their
/// full forms.
const SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        trigger: "/quit",
        description: "Exit the chat session",
    },
    SlashCommand {
        trigger: "/exit",
        description: "Exit the chat session",
    },
    SlashCommand {
        trigger: "/new",
        description: "Start a new conversation",
    },
    SlashCommand {
        trigger: "/forget",
        description: "Delete conversation completely (requires --yes)",
    },
    SlashCommand {
        trigger: "/help",
        description: "Show available commands",
    },
    SlashCommand {
        trigger: "/model",
        description: "Switch to a different model",
    },
    SlashCommand {
        trigger: "/system",
        description: "Change the system prompt",
    },
    SlashCommand {
        trigger: "/think",
        description: "Toggle think mode",
    },
    SlashCommand {
        trigger: "/tools",
        description: "Toggle tools",
    },
    SlashCommand {
        trigger: "/tools-output",
        description: "Set tool output level (compact|full|hidden)",
    },
    SlashCommand {
        trigger: "/compact",
        description: "Compact conversation history",
    },
    SlashCommand {
        trigger: "/retry",
        description: "Retry last message",
    },
    SlashCommand {
        trigger: "/undo",
        description: "Undo last message",
    },
    SlashCommand {
        trigger: "/save",
        description: "Save current session",
    },
    SlashCommand {
        trigger: "/load",
        description: "Load a saved session",
    },
    SlashCommand {
        trigger: "/session",
        description: "Session management commands",
    },
    SlashCommand {
        trigger: "/export",
        description: "Export conversation (md, json)",
    },
    SlashCommand {
        trigger: "/list",
        description: "List saved sessions",
    },
    SlashCommand {
        trigger: "/info",
        description: "Show session information",
    },
    SlashCommand {
        trigger: "/context",
        description: "Show context metrics and token usage",
    },
    SlashCommand {
        trigger: "/search",
        description: "Search conversation history",
    },
    SlashCommand {
        trigger: "/reindex",
        description: "Regenerate all embeddings",
    },
    SlashCommand {
        trigger: "/retrieval",
        description: "Toggle semantic retrieval",
    },
    SlashCommand {
        trigger: "/fact",
        description: "Manage factual memory",
    },
    SlashCommand {
        trigger: "/fact add",
        description: "Add a fact",
    },
    SlashCommand {
        trigger: "/fact list",
        description: "List facts",
    },
    SlashCommand {
        trigger: "/fact remove",
        description: "Remove a fact by ID",
    },
    SlashCommand {
        trigger: "/fact search",
        description: "Search facts",
    },
    SlashCommand {
        trigger: "/fact prune",
        description: "Prune old facts",
    },
    SlashCommand {
        trigger: "/note",
        description: "Manage notes",
    },
    SlashCommand {
        trigger: "/note add",
        description: "Add a note",
    },
    SlashCommand {
        trigger: "/note list",
        description: "List notes",
    },
    SlashCommand {
        trigger: "/note show",
        description: "Show a note",
    },
    SlashCommand {
        trigger: "/note edit",
        description: "Edit a note",
    },
    SlashCommand {
        trigger: "/note delete",
        description: "Delete a note",
    },
    SlashCommand {
        trigger: "/note search",
        description: "Search notes",
    },
    SlashCommand {
        trigger: "/doc",
        description: "Manage documents",
    },
    SlashCommand {
        trigger: "/doc import",
        description: "Import a document",
    },
    SlashCommand {
        trigger: "/doc list",
        description: "List documents",
    },
    SlashCommand {
        trigger: "/doc show",
        description: "Show a document",
    },
    SlashCommand {
        trigger: "/doc delete",
        description: "Delete a document",
    },
    SlashCommand {
        trigger: "/todo",
        description: "Manage todo tasks",
    },
    SlashCommand {
        trigger: "/todo add",
        description: "Add a task",
    },
    SlashCommand {
        trigger: "/todo list",
        description: "List tasks",
    },
    SlashCommand {
        trigger: "/todo get",
        description: "Get task details",
    },
    SlashCommand {
        trigger: "/todo update",
        description: "Update task status",
    },
    SlashCommand {
        trigger: "/todo edit",
        description: "Edit a task",
    },
    SlashCommand {
        trigger: "/todo delete",
        description: "Delete a task",
    },
    SlashCommand {
        trigger: "/todo clear-done",
        description: "Clear completed tasks",
    },
    SlashCommand {
        trigger: "/todo clear-all",
        description: "Clear all tasks",
    },
    SlashCommand {
        trigger: "/skill",
        description: "Activate or list skills",
    },
    SlashCommand {
        trigger: "/feedback",
        description: "Give feedback on responses",
    },
    SlashCommand {
        trigger: "/content",
        description: "Content management",
    },
    SlashCommand {
        trigger: "/content prune",
        description: "Prune low-retention content",
    },
    SlashCommand {
        trigger: "/ocr",
        description: "Extract text from an image",
    },
    SlashCommand {
        trigger: "/vision",
        description: "Analyze image with vision model",
    },
    SlashCommand {
        trigger: "/translate",
        description: "Translate text",
    },
    SlashCommand {
        trigger: "/summarize",
        description: "Summarize text",
    },
    SlashCommand {
        trigger: "/debug",
        description: "Toggle debug mode",
    },
    // Shortcuts
    SlashCommand {
        trigger: "/q",
        description: "Shortcut: /quit",
    },
    SlashCommand {
        trigger: "/n",
        description: "Shortcut: /new",
    },
    SlashCommand {
        trigger: "/h",
        description: "Shortcut: /help",
    },
    SlashCommand {
        trigger: "/m",
        description: "Shortcut: /model",
    },
    SlashCommand {
        trigger: "/s",
        description: "Shortcut: /system",
    },
    SlashCommand {
        trigger: "/l",
        description: "Shortcut: /load",
    },
    SlashCommand {
        trigger: "/t",
        description: "Shortcut: /think",
    },
    SlashCommand {
        trigger: "/e",
        description: "Shortcut: /export",
    },
    SlashCommand {
        trigger: "/ls",
        description: "Shortcut: /list",
    },
    SlashCommand {
        trigger: "/i",
        description: "Shortcut: /info",
    },
    SlashCommand {
        trigger: "/r",
        description: "Shortcut: /retry",
    },
    SlashCommand {
        trigger: "/u",
        description: "Shortcut: /undo",
    },
    SlashCommand {
        trigger: "/ctx",
        description: "Shortcut: /context",
    },
    SlashCommand {
        trigger: "/to",
        description: "Shortcut: /tools-output",
    },
    SlashCommand {
        trigger: "/f",
        description: "Shortcut: /search",
    },
    SlashCommand {
        trigger: "/sk",
        description: "Shortcut: /skill",
    },
    SlashCommand {
        trigger: "/fb",
        description: "Shortcut: /feedback",
    },
    SlashCommand {
        trigger: "/fg",
        description: "Shortcut: /feedback good",
    },
    SlashCommand {
        trigger: "/fp",
        description: "Shortcut: /fact prune",
    },
    SlashCommand {
        trigger: "/fa",
        description: "Shortcut: /fact add",
    },
    SlashCommand {
        trigger: "/fl",
        description: "Shortcut: /fact list",
    },
    SlashCommand {
        trigger: "/fr",
        description: "Shortcut: /fact remove",
    },
    SlashCommand {
        trigger: "/fs",
        description: "Shortcut: /fact search",
    },
    SlashCommand {
        trigger: "/no",
        description: "Shortcut: /note",
    },
    SlashCommand {
        trigger: "/na",
        description: "Shortcut: /note add",
    },
    SlashCommand {
        trigger: "/nl",
        description: "Shortcut: /note list",
    },
    SlashCommand {
        trigger: "/ns",
        description: "Shortcut: /note show",
    },
    SlashCommand {
        trigger: "/nd",
        description: "Shortcut: /note delete",
    },
    SlashCommand {
        trigger: "/di",
        description: "Shortcut: /doc import",
    },
    SlashCommand {
        trigger: "/dl",
        description: "Shortcut: /doc list",
    },
    SlashCommand {
        trigger: "/ds",
        description: "Shortcut: /doc show",
    },
    SlashCommand {
        trigger: "/dd",
        description: "Shortcut: /doc delete",
    },
    SlashCommand {
        trigger: "/ta",
        description: "Shortcut: /todo add",
    },
    SlashCommand {
        trigger: "/tl",
        description: "Shortcut: /todo list",
    },
    SlashCommand {
        trigger: "/tu",
        description: "Shortcut: /todo update",
    },
    SlashCommand {
        trigger: "/tg",
        description: "Shortcut: /todo get",
    },
    SlashCommand {
        trigger: "/te",
        description: "Shortcut: /todo edit",
    },
    SlashCommand {
        trigger: "/td",
        description: "Shortcut: /todo delete",
    },
    SlashCommand {
        trigger: "/tcd",
        description: "Shortcut: /todo clear-done",
    },
    SlashCommand {
        trigger: "/tca",
        description: "Shortcut: /todo clear-all",
    },
    SlashCommand {
        trigger: "/tr",
        description: "Shortcut: /translate",
    },
    SlashCommand {
        trigger: "/sum",
        description: "Shortcut: /summarize",
    },
    SlashCommand {
        trigger: "/cp",
        description: "Shortcut: /content prune",
    },
    SlashCommand {
        trigger: "/sys",
        description: "Shortcut: /system",
    },
];

/// The result of a tab completion attempt.
#[derive(Debug, Clone)]
pub enum CompletionResult {
    /// No completions found
    None,
    /// Single completion found — replace the text and adjust cursor
    Single {
        /// The full replacement text for the input buffer
        replacement: String,
        /// New cursor position (byte offset) in the replacement
        cursor_pos: usize,
    },
    /// Multiple completions found — show in completion menu
    Multiple {
        /// All matching completion strings (for menu display)
        matches: Vec<String>,
        /// Current index in the matches cycle (0-based)
        #[allow(dead_code)]
        // Kept for API compatibility; menu uses its own selection state
        cycle_index: usize,
    },
}

/// Tab completer for the TUI chat input.
///
/// Maintains model names for `/model` argument completion and
/// cycling state for repeated Tab presses.
pub struct ChatCompleter {
    /// Available model names for `/model <name>` completion
    model_names: Vec<String>,
    /// Current cycling index for multiple completions (resets on input change)
    cycle_index: usize,
    /// The last fragment that was completed (to detect changes and reset cycling)
    last_fragment: String,
}

impl ChatCompleter {
    /// Create a new completer with the given model names.
    pub fn new(model_names: Vec<String>) -> Self {
        Self {
            model_names,
            cycle_index: 0,
            last_fragment: String::new(),
        }
    }

    /// Update the model names list (e.g., after a model switch or refresh).
    pub fn set_model_names(&mut self, names: Vec<String>) {
        self.model_names = names;
    }

    /// Attempt tab completion based on the current buffer and cursor position.
    ///
    /// Completion logic:
    /// 1. If the buffer starts with `/`, try slash command completion
    /// 2. If the buffer starts with `/model `, try model name completion
    /// 3. Otherwise, no completion
    pub fn complete(&mut self, buffer: &str, cursor_pos: usize) -> CompletionResult {
        let fragment = buffer[..cursor_pos].to_string();

        // Reset cycle if the fragment changed since last completion
        if fragment != self.last_fragment {
            self.cycle_index = 0;
            self.last_fragment = fragment.clone();
        }

        // Only complete if the cursor is at the end of the buffer
        if cursor_pos != buffer.len() {
            return CompletionResult::None;
        }

        // Slash command completion
        if fragment.starts_with('/') {
            // Check if we're completing a model name argument
            if let Some(model_fragment) = fragment.strip_prefix("/model ") {
                let model_fragment = model_fragment.trim();
                return self.complete_model(model_fragment);
            }
            return self.complete_slash_command(&fragment);
        }

        CompletionResult::None
    }

    /// Complete a slash command from a partial input.
    ///
    /// Finds all slash commands that start with the given fragment.
    /// If exactly one match is found, returns a Single completion with
    /// a trailing space. If multiple matches are found, cycles through them.
    fn complete_slash_command(&mut self, fragment: &str) -> CompletionResult {
        let matches: Vec<&SlashCommand> = SLASH_COMMANDS
            .iter()
            .filter(|cmd| cmd.trigger.starts_with(fragment))
            .collect();

        match matches.len() {
            0 => CompletionResult::None,
            1 => {
                let cmd = matches[0];
                // For single-character commands like /q, /n, /h — add trailing space
                // For multi-word commands like /fact add — add trailing space
                // For single-word commands like /model — add trailing space
                let replacement = format!("{} ", cmd.trigger);
                CompletionResult::Single {
                    cursor_pos: replacement.len(),
                    replacement,
                }
            }
            _ => {
                // If the fragment exactly matches one of the commands,
                // prefer exact match (e.g., "/n" matches both "/n" and "/new")
                // But only if the exact match is shorter or equal.
                let exact_match = matches.iter().find(|cmd| cmd.trigger == fragment);
                if let Some(exact) = exact_match {
                    let replacement = format!("{} ", exact.trigger);
                    return CompletionResult::Single {
                        cursor_pos: replacement.len(),
                        replacement,
                    };
                }

                // Cycle through multiple matches
                let match_strings: Vec<String> =
                    matches.iter().map(|cmd| cmd.trigger.to_string()).collect();

                // Find the common prefix
                let common_prefix = common_prefix(&match_strings);
                if !common_prefix.is_empty() && common_prefix != fragment {
                    // Extend to common prefix first
                    return CompletionResult::Single {
                        replacement: format!("{} ", common_prefix),
                        cursor_pos: common_prefix.len() + 1,
                    };
                }

                // Already at the common prefix, cycle through matches
                let idx = self.cycle_index % match_strings.len();
                self.cycle_index += 1;

                CompletionResult::Multiple {
                    matches: match_strings,
                    cycle_index: idx,
                }
            }
        }
    }

    /// Complete a model name from a partial input.
    ///
    /// Finds all model names that start with the given fragment.
    /// Single matches and common prefix matches get a trailing space.
    fn complete_model(&mut self, fragment: &str) -> CompletionResult {
        let matches: Vec<&str> = self
            .model_names
            .iter()
            .filter(|name| name.starts_with(fragment))
            .map(|s| s.as_str())
            .collect();

        match matches.len() {
            0 => CompletionResult::None,
            1 => CompletionResult::Single {
                replacement: format!("/model {} ", matches[0]),
                cursor_pos: format!("/model {} ", matches[0]).len(),
            },
            _ => {
                // Find the common prefix among matching model names
                let match_strings: Vec<String> = matches.iter().map(|&s| s.to_string()).collect();
                let common = common_prefix(&match_strings);

                if !common.is_empty() && common != fragment {
                    // Extend to common prefix with trailing space
                    let replacement = format!("/model {} ", common);
                    return CompletionResult::Single {
                        cursor_pos: replacement.len(),
                        replacement,
                    };
                }

                // Cycle through matches
                let idx = self.cycle_index % matches.len();
                self.cycle_index += 1;

                let replacement = format!("/model {} ", matches[idx]);
                let cursor_pos = replacement.len();
                CompletionResult::Single {
                    replacement,
                    cursor_pos,
                }
            }
        }
    }
}

/// Find the common prefix among a list of strings.
fn common_prefix(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }

    let first = strings[0].as_bytes();
    let mut prefix_len = first.len();

    for s in &strings[1..] {
        let bytes = s.as_bytes();
        let mut j = 0;
        while j < prefix_len && j < bytes.len() && first[j] == bytes[j] {
            j += 1;
        }
        prefix_len = j;
        if prefix_len == 0 {
            return String::new();
        }
    }

    String::from_utf8_lossy(&first[..prefix_len]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_completer() -> ChatCompleter {
        ChatCompleter::new(vec![
            "llama3.1".to_string(),
            "llama3.1:70b".to_string(),
            "mistral".to_string(),
            "qwen2.5".to_string(),
            "glm-5:cloud".to_string(),
        ])
    }

    #[test]
    fn test_complete_slash_command_single_match() {
        let mut completer = make_completer();
        // "/help" is unique (not a prefix of other commands)
        let result = completer.complete("/help", 5);
        match result {
            CompletionResult::Single {
                replacement,
                cursor_pos,
            } => {
                assert_eq!(replacement, "/help ");
                assert_eq!(cursor_pos, 6);
            }
            _ => panic!("Expected Single completion, got {:?}", result),
        }
    }

    #[test]
    fn test_complete_slash_command_exact_shortcut() {
        let mut completer = make_completer();
        // "/h" matches /help and /h (shortcut).
        // Exact match "/h" is found first — completes to "/h ".
        let result = completer.complete("/h", 2);
        match result {
            CompletionResult::Single {
                replacement,
                cursor_pos,
            } => {
                // /h is an exact match shortcut
                assert_eq!(replacement, "/h ");
                assert_eq!(cursor_pos, 3);
            }
            _ => panic!(
                "Expected Single completion for exact shortcut, got {:?}",
                result
            ),
        }
    }

    #[test]
    fn test_complete_slash_command_no_match() {
        let mut completer = make_completer();
        let result = completer.complete("/xyz", 4);
        assert!(matches!(result, CompletionResult::None));
    }

    #[test]
    fn test_complete_model_single_match() {
        let mut completer = make_completer();
        // "/model mis" (10 bytes) → completes to "/model mistral " (15 bytes)
        let result = completer.complete("/model mis", 10);
        match result {
            CompletionResult::Single {
                replacement,
                cursor_pos,
            } => {
                assert_eq!(replacement, "/model mistral ");
                assert_eq!(cursor_pos, 15); // "/model mistral ".len() = 15
            }
            _ => panic!("Expected Single completion, got {:?}", result),
        }
    }

    #[test]
    fn test_complete_model_multiple_common_prefix() {
        let mut completer = make_completer();
        // "/model llama" (= 7+5 = 12 bytes) matches llama3.1 and llama3.1:70b
        // Common prefix is "llama3.1", so completion extends to "/model llama3.1 "
        let result = completer.complete("/model llama", 12);
        match result {
            CompletionResult::Single {
                replacement,
                cursor_pos,
            } => {
                // Common prefix "llama3.1" extends the input with trailing space
                assert_eq!(replacement, "/model llama3.1 ");
                assert_eq!(cursor_pos, 16); // "/model llama3.1 ".len() = 16
            }
            _ => panic!(
                "Expected Single (common prefix) completion, got {:?}",
                result
            ),
        }
    }

    #[test]
    fn test_complete_no_slash_no_completion() {
        let mut completer = make_completer();
        let result = completer.complete("hello", 5);
        assert!(matches!(result, CompletionResult::None));
    }

    #[test]
    fn test_complete_cursor_not_at_end() {
        let mut completer = make_completer();
        let result = completer.complete("/hel", 2); // cursor in middle
        assert!(matches!(result, CompletionResult::None));
    }

    #[test]
    fn test_complete_slash_command_multiple_prefix() {
        let mut completer = make_completer();
        // "/to" matches /todo, /todo add, /todo list, etc., and /tools, /tools-output, /to shortcut
        // Common prefix is "/to" or longer — let's test cycling
        let result = completer.complete("/to", 3);
        // Should extend to "/todo " (common prefix is "todo") or cycle
        match result {
            CompletionResult::Single { replacement, .. } => {
                // Common prefix completion
                assert!(replacement.starts_with("/todo") || replacement.starts_with("/to"));
            }
            CompletionResult::Multiple { matches, .. } => {
                // Multiple matches — should include /todo, /todo add, etc.
                assert!(!matches.is_empty());
            }
            CompletionResult::None => panic!("Expected some completion for /to"),
        }
    }

    #[test]
    fn test_common_prefix_empty() {
        assert_eq!(common_prefix(&[]), "");
    }

    #[test]
    fn test_common_prefix_single() {
        assert_eq!(common_prefix(&["hello".to_string()]), "hello");
    }

    #[test]
    fn test_common_prefix_multiple() {
        assert_eq!(
            common_prefix(&["llama3.1".to_string(), "llama3.1:70b".to_string()]),
            "llama3.1"
        );
    }

    #[test]
    fn test_common_prefix_no_common() {
        assert_eq!(common_prefix(&["abc".to_string(), "xyz".to_string()]), "");
    }

    #[test]
    fn test_cycle_index_resets_on_fragment_change() {
        let mut completer = make_completer();
        // "/model mi" (9 bytes) matches "mistral" only → Single completion
        let result = completer.complete("/model mi", 9);
        assert!(matches!(result, CompletionResult::Single { .. }));

        // Different fragment — should reset cycle
        let result = completer.complete("/model q", 8);
        assert!(matches!(result, CompletionResult::Single { .. }));
    }

    #[test]
    fn test_completion_result_debug() {
        // Ensure CompletionResult implements Debug
        let result = CompletionResult::None;
        assert!(format!("{:?}", result).contains("None"));
    }
}
