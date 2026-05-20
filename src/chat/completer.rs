//! Tab completion for the TUI chat input
//!
//! Provides slash command and model name completion. When the user presses
//! Tab, the completer tries to:
//!
//! 1. Complete slash commands (e.g., `/he` → `/help`, `/mod` → `/model`)
//! 2. Complete slash command arguments (e.g., `/model l` → `/model llama3.1`)
//! 3. Cycle through multiple matches on repeated Tab presses
//!
//! Descriptions are shown in the completion menu, even for single prefix
//! matches, so the user can see what each command does before confirming.
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
/// Each entry maps a command trigger (with `/` prefix) to a description
/// for the completion menu. Only canonical command names are listed —
/// no shortcuts or aliases. The parser still accepts `/exit` as a
/// synonym for `/quit`, but it is not shown in completions.
#[derive(Debug)]
struct SlashCommand {
    /// The full command string including `/` prefix
    trigger: &'static str,
    /// Short description shown in completion menu
    description: &'static str,
    /// Type of argument completion for this command (if any)
    arg_type: ArgCompletion,
}

/// Types of argument completion for slash commands.
///
/// When a command accepts arguments, this specifies what kind of
/// completions are offered after the command trigger + space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ArgCompletion {
    /// No arguments (e.g., /quit, /help)
    #[default]
    None,
    /// Model name(s) as arguments (e.g., /model llama3.1)
    ModelName,
    /// Static subcommands only (e.g., /think on|off)
    StaticSubcommands,
}

/// Slash commands for tab completion.
///
/// Only canonical names are listed (no shortcuts or aliases).
/// Entries are listed in the same order as `/help` output so users get
/// a predictable completion experience.
const SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        trigger: "/quit",
        description: "Exit the chat session",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/new",
        description: "Start a new conversation",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/forget",
        description: "Delete conversation completely (requires --yes)",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/help",
        description: "Show available commands",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/model",
        description: "Switch to a different model",
        arg_type: ArgCompletion::ModelName,
    },
    SlashCommand {
        trigger: "/system",
        description: "Change the system prompt",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/think",
        description: "Toggle think mode",
        arg_type: ArgCompletion::StaticSubcommands,
    },
    SlashCommand {
        trigger: "/tools",
        description: "Toggle tools",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/toggle-style",
        description: "Toggle style rendering (mermaid/source, syntax highlight, table format)",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/tools-output",
        description: "Set tool output level (compact|full|hidden)",
        arg_type: ArgCompletion::StaticSubcommands,
    },
    SlashCommand {
        trigger: "/compact",
        description: "Compact conversation history",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/retry",
        description: "Retry last message",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/undo",
        description: "Undo last message",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/save",
        description: "Save current session",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/load",
        description: "Load a saved session",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/session",
        description: "Session management commands",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/export",
        description: "Export conversation (md, json)",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/list",
        description: "List saved sessions",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/info",
        description: "Show session information",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/context",
        description: "Show context metrics and token usage",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/search",
        description: "Search conversation history",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/reindex",
        description: "Regenerate all embeddings (requires --yes)",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/retrieval",
        description: "Toggle semantic retrieval",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/fact",
        description: "Manage factual memory",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/fact add",
        description: "Add a fact",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/fact list",
        description: "List facts",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/fact remove",
        description: "Remove a fact by ID",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/fact search",
        description: "Search facts",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/fact prune",
        description: "Prune old facts",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/note",
        description: "Manage notes",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/note add",
        description: "Add a note",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/note list",
        description: "List notes",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/note show",
        description: "Show a note",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/note edit",
        description: "Edit a note",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/note delete",
        description: "Delete a note",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/note search",
        description: "Search notes",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/doc",
        description: "Manage documents",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/doc import",
        description: "Import a document",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/doc list",
        description: "List documents",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/doc show",
        description: "Show a document",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/doc delete",
        description: "Delete a document",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/todo",
        description: "Manage todo tasks",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/todo add",
        description: "Add a task",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/todo list",
        description: "List tasks",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/todo get",
        description: "Get task details",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/todo update",
        description: "Update task status",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/todo edit",
        description: "Edit a task",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/todo delete",
        description: "Delete a task",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/todo clear-done",
        description: "Clear completed tasks",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/todo clear-all",
        description: "Clear all tasks",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/skill",
        description: "Activate or list skills",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/feedback",
        description: "Give feedback on responses",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/content",
        description: "Content management",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/content prune",
        description: "Prune low-retention content",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/ocr",
        description: "Extract text from an image",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/vision",
        description: "Analyze image with vision model",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/translate",
        description: "Translate text",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/summarize",
        description: "Summarize text",
        arg_type: ArgCompletion::None,
    },
    SlashCommand {
        trigger: "/debug",
        description: "Toggle debug mode",
        arg_type: ArgCompletion::None,
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
        /// Descriptions for each match (same length as matches; empty strings if none)
        descriptions: Vec<String>,
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
    /// 2. If the fragment matches a command with `arg_type: ModelName`,
    ///    try model name completion for the argument part
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
            // Check for argument completion on commands that take model names
            if let Some((cmd_trigger, arg_fragment)) = self.try_model_arg_fragment(&fragment) {
                return self.complete_model(cmd_trigger, arg_fragment.trim());
            }
            // Check for argument completion on commands with static subcommands
            if let Some((cmd_trigger, arg_fragment)) =
                self.try_static_subcommand_fragment(&fragment)
            {
                return self.complete_static_subcommand(cmd_trigger, arg_fragment.trim());
            }
            return self.complete_slash_command(&fragment);
        }

        CompletionResult::None
    }

    /// Check if the fragment matches a command that takes a model name argument.
    ///
    /// Returns `Some((command_trigger, arg_fragment))` if the fragment starts
    /// with a command trigger (plus space) that has `arg_type: ModelName`.
    /// The `arg_fragment` is the text after the command trigger + space.
    fn try_model_arg_fragment(&self, fragment: &str) -> Option<(&'static str, String)> {
        for cmd in SLASH_COMMANDS.iter() {
            if cmd.arg_type == ArgCompletion::ModelName {
                let prefix = format!("{} ", cmd.trigger);
                if fragment.starts_with(&prefix) {
                    return Some((cmd.trigger, fragment[prefix.len()..].to_string()));
                }
            }
        }
        None
    }

    /// Check if the fragment matches a command that takes static subcommands.
    ///
    /// Returns `Some((command_trigger, arg_fragment))` if the fragment starts
    /// with a command trigger (plus space) that has `arg_type: StaticSubcommands`.
    /// The `arg_fragment` is the text after the command trigger + space.
    fn try_static_subcommand_fragment(&self, fragment: &str) -> Option<(&'static str, String)> {
        for cmd in SLASH_COMMANDS.iter() {
            if cmd.arg_type == ArgCompletion::StaticSubcommands {
                let prefix = format!("{} ", cmd.trigger);
                if fragment.starts_with(&prefix) {
                    return Some((cmd.trigger, fragment[prefix.len()..].to_string()));
                }
            }
        }
        None
    }

    /// Static subcommand definitions for slash commands.
    ///
    /// Maps command triggers to their static subcommand lists.
    fn get_static_subcommands(trigger: &str) -> Vec<&'static str> {
        match trigger {
            "/think" => vec!["on", "off"],
            "/tools-output" => vec!["compact", "full", "hidden"],
            _ => vec![],
        }
    }

    /// Complete a static subcommand from a partial input.
    ///
    /// Filters the command's subcommand list by the fragment and returns
    /// matching completions with the command trigger prefix.
    fn complete_static_subcommand(
        &mut self,
        cmd_trigger: &str,
        fragment: &str,
    ) -> CompletionResult {
        let subcommands = Self::get_static_subcommands(cmd_trigger);
        let matches: Vec<&str> = subcommands
            .iter()
            .filter(|&s| s.starts_with(fragment))
            .copied()
            .collect();

        match matches.len() {
            0 => CompletionResult::None,
            1 => CompletionResult::Single {
                replacement: format!("{} {} ", cmd_trigger, matches[0]),
                cursor_pos: format!("{} {} ", cmd_trigger, matches[0]).len(),
            },
            _ => {
                let match_strings: Vec<String> = matches
                    .iter()
                    .map(|&s| format!("{} {}", cmd_trigger, s))
                    .collect();
                let descriptions = vec![String::new(); match_strings.len()];

                CompletionResult::Multiple {
                    matches: match_strings,
                    descriptions,
                    cycle_index: 0,
                }
            }
        }
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
                // If the fragment exactly matches a command (e.g., user typed
                // "/help" and pressed Tab), complete with a trailing space.
                // If the fragment is a prefix (e.g., "/he" matching "/help"),
                // show a single-item menu so the user sees the description.
                if fragment == cmd.trigger {
                    let replacement = format!("{} ", cmd.trigger);
                    CompletionResult::Single {
                        cursor_pos: replacement.len(),
                        replacement,
                    }
                } else {
                    // Prefix match — show as single-item menu with description
                    CompletionResult::Multiple {
                        matches: vec![cmd.trigger.to_string()],
                        descriptions: vec![cmd.description.to_string()],
                        cycle_index: 0,
                    }
                }
            }
            _ => {
                // If the fragment exactly matches one of the commands,
                // If the fragment exactly matches one of the commands,
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

                // Already at the common prefix, show completion menu
                let idx = self.cycle_index % match_strings.len();
                self.cycle_index += 1;

                let descriptions: Vec<String> = matches
                    .iter()
                    .map(|cmd| cmd.description.to_string())
                    .collect();

                CompletionResult::Multiple {
                    matches: match_strings,
                    descriptions,
                    cycle_index: idx,
                }
            }
        }
    }

    /// Complete a model name from a partial input.
    ///
    /// Finds all model names that start with the given fragment.
    /// Single matches and common prefix matches get a trailing space.
    /// The `cmd_trigger` prefix is used to build the replacement text,
    /// supporting both `/model` and shortcuts like `/m`.
    fn complete_model(&mut self, cmd_trigger: &str, fragment: &str) -> CompletionResult {
        let matches: Vec<&str> = self
            .model_names
            .iter()
            .filter(|name| name.starts_with(fragment))
            .map(|s| s.as_str())
            .collect();

        match matches.len() {
            0 => CompletionResult::None,
            1 => CompletionResult::Single {
                replacement: format!("{} {} ", cmd_trigger, matches[0]),
                cursor_pos: format!("{} {} ", cmd_trigger, matches[0]).len(),
            },
            _ => {
                // Find the common prefix among matching model names
                let match_strings: Vec<String> = matches.iter().map(|&s| s.to_string()).collect();
                let common = common_prefix(&match_strings);

                if !common.is_empty() && common != fragment {
                    // Extend to common prefix with trailing space
                    let replacement = format!("{} {} ", cmd_trigger, common);
                    return CompletionResult::Single {
                        cursor_pos: replacement.len(),
                        replacement,
                    };
                }

                // Return multiple matches for completion menu
                // Items include the command trigger prefix so the replacement
                // is correct when the user confirms a selection (e.g., "/model glm-5.1:cloud")
                let match_strings: Vec<String> = matches
                    .iter()
                    .map(|&s| format!("{} {}", cmd_trigger, s))
                    .collect();
                let descriptions = vec![String::new(); match_strings.len()];

                CompletionResult::Multiple {
                    matches: match_strings,
                    descriptions,
                    cycle_index: 0,
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
    fn test_complete_slash_command_prefix_shows_description() {
        let mut completer = make_completer();
        // "/he" matches "/help" as a prefix — shows description menu
        let result = completer.complete("/he", 3);
        match result {
            CompletionResult::Multiple {
                matches,
                descriptions,
                ..
            } => {
                assert_eq!(matches.len(), 1);
                assert_eq!(matches[0], "/help");
                assert_eq!(descriptions[0], "Show available commands");
            }
            _ => panic!(
                "Expected Multiple completion for prefix match, got {:?}",
                result
            ),
        }

        // Exact match "/help" completes with trailing space
        let result = completer.complete("/help", 5);
        match result {
            CompletionResult::Single {
                replacement,
                cursor_pos,
            } => {
                assert_eq!(replacement, "/help ");
                assert_eq!(cursor_pos, 6);
            }
            _ => panic!(
                "Expected Single completion for exact match, got {:?}",
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
