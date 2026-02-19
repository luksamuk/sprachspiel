//! Tab completion for chat commands
//!
//! Provides command and model name completion for the REPL.

use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::Validator;
use rustyline::{Context, Helper};

/// Commands available for completion
const COMMANDS: &[&str] = &[
    "/quit", "/exit", "/clear", "/help", "/model", "/system", "/think", "/tools", "/compact",
    "/save", "/load", "/export", "/list", "/info",
];

/// Shortcuts for commands
const SHORTCUTS: &[&str] = &["/q", "/c", "/h", "/m", "/s", "/t", "/l", "/e", "/ls", "/i"];

/// Chat command completer
pub struct ChatCompleter {
    /// Model names for completion after /model
    models: Vec<String>,
}

impl ChatCompleter {
    /// Create a new completer with the list of available models
    pub fn new(models: Vec<String>) -> Self {
        Self { models }
    }

    /// Update the list of models
    #[allow(dead_code)]
    pub fn set_models(&mut self, models: Vec<String>) {
        self.models = models;
    }
}

impl Completer for ChatCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let slice = &line[..pos];

        if slice.starts_with('/') {
            if slice.starts_with("/model ") || slice.starts_with("/m ") {
                let prefix = slice.split_whitespace().last().unwrap_or("");
                let matches: Vec<Pair> = self
                    .models
                    .iter()
                    .filter(|m| m.starts_with(prefix))
                    .map(|m| Pair {
                        display: m.clone(),
                        replacement: m.clone(),
                    })
                    .collect();
                return Ok((pos - prefix.len(), matches));
            }

            let mut matches: Vec<Pair> = COMMANDS
                .iter()
                .chain(SHORTCUTS.iter())
                .filter(|cmd| cmd.starts_with(slice))
                .map(|cmd| Pair {
                    display: cmd.to_string(),
                    replacement: cmd.to_string(),
                })
                .collect();

            matches.sort_by(|a, b| a.display.cmp(&b.display));
            Ok((0, matches))
        } else {
            Ok((pos, Vec::new()))
        }
    }
}

impl Hinter for ChatCompleter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        if !line.starts_with('/') {
            let hinter = HistoryHinter::new();
            return hinter.hint(line, pos, ctx);
        }
        None
    }
}

impl Highlighter for ChatCompleter {}

impl Validator for ChatCompleter {
    fn validate(
        &self,
        _: &mut rustyline::validate::ValidationContext<'_>,
    ) -> rustyline::Result<rustyline::validate::ValidationResult> {
        Ok(rustyline::validate::ValidationResult::Valid(None))
    }
}

impl Helper for ChatCompleter {}
