//! Session fact tracker for compaction: facts the code knows, not facts a model recalls.
//!
//! The compaction summary is written by a language model, and measured retention of
//! facts in such prose is poor — a fact the model may or may not restate. Facts
//! extracted from the session's own tool-call records are not a recall problem.
//!
//! The tracker is fed at tool-execution time, because that is the only point where
//! the tool name, its arguments and its result are simultaneously in scope. The
//! conversation history cannot serve as the source: tool messages are persisted as
//! bare result strings with no tool name attached.
//!
//! Wire-up into the compaction driver is a follow-up task; until that lands
//! the module's items are not yet constructed by the binary target, which
//! carries its own private module tree.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// Tool names that create or modify a file. `edit_file` covers in-place change;
/// `append_file` extends. Kept as a `const` so adding a tool is one edit.
const MODIFYING_TOOLS: &[&str] = &["write_file", "edit_file", "append_file"];

/// Tool names that only inspect. `read_file` and its siblings never change the tree.
const READING_TOOLS: &[&str] = &["read_file", "read_file_segment", "count_lines"];

/// Largest each bucket keeps; past the cap the least-recently-touched path
/// is evicted. Bounds the staple, which rides `compacted_summary` into every
/// future context — an unbounded bucket would inflate the very budget the
/// compaction exists to save.
const MAX_PATHS_PER_BUCKET: usize = 50;

/// Facts about a session, accumulated from tool executions.
///
/// Paths are stored in a `BTreeMap<path, seq>` so the rendered block is
/// deterministic (iteration is path-sorted — stable order, no duplicates)
/// while the value records recency for the cap eviction. Downstream grading
/// compares the summary against these paths by containment, and a collection
/// that reorders per run would make an identical summary read as a mismatch.
///
/// Cumulative across compactions by design: a file modified before an earlier
/// compaction is still modified for the rest of the session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionFactTracker {
    #[serde(default)]
    modified: BTreeMap<String, u64>,
    #[serde(default)]
    read: BTreeMap<String, u64>,
    /// The most recent command, with its outcome. `None` until one runs.
    /// "Last" means last — no significance heuristic is applied.
    #[serde(default)]
    last_run: Option<LastRun>,
    /// Monotonic counter stamping every recorded touch with its sequence.
    #[serde(default)]
    next_seq: u64,
}

/// The last command the session ran, and whether it succeeded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastRun {
    /// The command exactly as issued — quoted verbatim in the staple block,
    /// never paraphrased, so a reader can re-run it.
    pub command: String,
    pub passed: bool,
}

impl SessionFactTracker {
    /// Fold one tool execution into the tracker.
    ///
    /// `args_json` is the raw argument object; a parse failure is not fatal —
    /// the execution still happened — but it is never silent either: a fact
    /// the tracker could have named is logged so the loss stays visible.
    pub fn record_tool(&mut self, tool_name: &str, args_json: &str, result: &str, is_error: bool) {
        if tool_name == "run_command" {
            if let Some(cmd) = extract_string_arg(args_json, "command_line") {
                self.last_run = Some(LastRun {
                    command: cmd,
                    passed: !is_error,
                });
            } else {
                log::warn!(
                    "fact tracker: run_command args without a parseable command_line — fact skipped"
                );
            }
            return;
        }

        let Some(raw_path) = extract_string_arg(args_json, "path") else {
            log::warn!(
                "fact tracker: tool {tool_name} ran without a parseable path — fact skipped"
            );
            return;
        };
        // Normalize before storing: `src/a.rs`, `./src/a.rs` and an absolute
        // path are the SAME file. Without one key per file the
        // modified-wins-over-read precedence breaks, and a single file can
        // appear in both lists of an "authoritative" block.
        let path = normalize_path(&raw_path);

        self.next_seq = self.next_seq.saturating_add(1);
        let seq = self.next_seq;

        if MODIFYING_TOOLS.contains(&tool_name) {
            if !is_error {
                // Only record a modification that actually succeeded. A failed
                // write is not a modified file, and recording it would put a
                // path in the staple block that no reader could find changed.
                insert_capped(&mut self.modified, path.clone(), seq);
                // A path both written and read is a modification; the weaker
                // claim wins so the block never lists the same path twice.
                self.read.remove(&path);
            }
        } else if READING_TOOLS.contains(&tool_name) && !self.modified.contains_key(&path) {
            insert_capped(&mut self.read, path, seq);
        }
        let _ = result;
    }

    pub fn modified_files(&self) -> Vec<String> {
        self.modified.keys().cloned().collect()
    }

    pub fn read_files(&self) -> Vec<String> {
        self.read.keys().cloned().collect()
    }

    pub fn last_run(&self) -> Option<&LastRun> {
        self.last_run.as_ref()
    }

    /// True when there is nothing worth stapling — lets the caller skip
    /// appending an empty block to the summary.
    pub fn is_empty(&self) -> bool {
        self.modified.is_empty() && self.read.is_empty() && self.last_run.is_none()
    }
}

/// Pull one top-level string field out of a JSON object.
///
/// Deliberately not a typed deserializer: tool arguments arrive as an opaque
/// `serde_json::Value` and their shapes differ per tool. A missing or
/// non-string field means "cannot name it", never an error.
fn extract_string_arg(args_json: &str, key: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(args_json).ok()?;
    parsed.get(key)?.as_str().map(|s| s.to_string())
}

/// Lexically normalize a path to one key per file: join with the current
/// directory when relative, resolve `.` and `..`. Deliberately NOT
/// `fs::canonicalize` — that fails for a file the session is about to create.
pub fn normalize_path(raw: &str) -> String {
    let joined = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        std::env::current_dir().unwrap_or_default().join(raw)
    };
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out.to_string_lossy().into_owned()
}

/// Insert into a bucket under the recency cap: past the cap, the entry with
/// the lowest sequence number (least recently touched) is evicted.
fn insert_capped(bucket: &mut BTreeMap<String, u64>, path: String, seq: u64) {
    if bucket.len() >= MAX_PATHS_PER_BUCKET
        && !bucket.contains_key(&path)
        && let Some(oldest) = bucket
            .iter()
            .min_by_key(|(_, s)| **s)
            .map(|(k, _)| k.clone())
    {
        bucket.remove(&oldest);
    }
    bucket.insert(path, seq);
}

/// Literal used when a block has no content.
///
/// A fixed string, not a blank: downstream grading distinguishes "there was no
/// unresolved error" from "the producer forgot the block", and only a literal
/// makes those different. Blank/`N/A`/`-`/omitted would all collapse together.
const ABSENT: &str = "None";

/// Render the machine-built facts block appended after the model's summary.
///
/// `latest_user_request` is passed in rather than read from the tracker: the
/// compactor does not receive the conversation tail, so the model cannot supply
/// it, and the caller holds the one authoritative copy
/// (`ChatSession::get_last_user_message`).
///
/// The block is appended, never merged into the model's prose: a fact the code
/// wrote is not something a later summarization pass should be free to reword.
pub fn render_staple_block(
    tracker: &SessionFactTracker,
    latest_user_request: Option<&str>,
) -> String {
    fn lines(items: &[String]) -> String {
        if items.is_empty() {
            ABSENT.to_string()
        } else {
            items.join("\n")
        }
    }

    let last_run = match tracker.last_run() {
        Some(r) => format!(
            "{} -> {}",
            r.command,
            if r.passed { "PASS" } else { "FAIL" }
        ),
        None => ABSENT.to_string(),
    };

    format!(
        "<compaction_facts>\n\
         These facts were extracted by the harness from the session's own tool records. \
         They are authoritative for tool-mediated activity — files and commands invoked \
         through tools — but not exhaustive: activity outside tools is invisible to them. \
         Prefer them over any conflicting statement about tool activity in the summary above. \
         The summary is reference; the latest user message always wins.\n\n\
         <modified-files>\n{}\n</modified-files>\n\n\
         <read-files>\n{}\n</read-files>\n\n\
         <last-run>\n{}\n</last-run>\n\n\
         <latest-user-request>\n{}\n</latest-user-request>\n\
         </compaction_facts>",
        lines(&tracker.modified_files()),
        lines(&tracker.read_files()),
        last_run,
        latest_user_request.unwrap_or(ABSENT),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_modified_and_read_paths_separately() {
        let mut t = SessionFactTracker::default();
        t.record_tool(
            "write_file",
            r#"{"path":"src/a.rs"}"#,
            "Successfully created 'src/a.rs'.",
            false,
        );
        t.record_tool("read_file", r#"{"path":"src/b.rs"}"#, "1|hello", false);

        // Paths are stored normalized (REVIEW DELTA) — assert on the
        // normalizer's output, never on the raw argument.
        assert_eq!(t.modified_files(), vec![normalize_path("src/a.rs")]);
        assert_eq!(t.read_files(), vec![normalize_path("src/b.rs")]);
    }

    #[test]
    fn a_failed_write_is_not_a_modification() {
        let mut t = SessionFactTracker::default();
        t.record_tool(
            "write_file",
            r#"{"path":"src/a.rs"}"#,
            "BLOCKED: path denied",
            true,
        );

        assert!(
            t.modified_files().is_empty(),
            "a write that failed changed nothing"
        );
    }

    #[test]
    fn a_path_written_after_being_read_is_a_modification() {
        let mut t = SessionFactTracker::default();
        t.record_tool("read_file", r#"{"path":"src/a.rs"}"#, "1|hello", false);
        t.record_tool(
            "edit_file",
            r#"{"path":"src/a.rs","operation":"replace"}"#,
            "Updated.",
            false,
        );

        assert_eq!(t.modified_files(), vec![normalize_path("src/a.rs")]);
        assert!(
            t.read_files().is_empty(),
            "a modified path is not also reported as merely read"
        );
    }

    #[test]
    fn output_order_is_stable() {
        let mut a = SessionFactTracker::default();
        let mut b = SessionFactTracker::default();
        for p in ["src/z.rs", "src/a.rs", "src/m.rs"] {
            let args = format!(r#"{{"path":"{p}"}}"#);
            a.record_tool("write_file", &args, "ok", false);
        }
        for p in ["src/m.rs", "src/z.rs", "src/a.rs"] {
            let args = format!(r#"{{"path":"{p}"}}"#);
            b.record_tool("write_file", &args, "ok", false);
        }

        assert_eq!(
            a.modified_files(),
            b.modified_files(),
            "insertion order must not leak into output"
        );
    }

    #[test]
    fn records_the_last_command_with_its_outcome() {
        let mut t = SessionFactTracker::default();
        t.record_tool(
            "run_command",
            r#"{"command_line":"cargo test"}"#,
            "ok",
            false,
        );
        t.record_tool(
            "run_command",
            r#"{"command_line":"cargo build"}"#,
            "Error: no",
            true,
        );

        let last = t.last_run().expect("a command ran");
        assert_eq!(last.command, "cargo build");
        assert!(!last.passed);
    }

    #[test]
    fn each_bucket_keeps_at_most_50_paths() {
        let mut t = SessionFactTracker::default();
        for i in 0..60 {
            let args = format!(r#"{{"path":"src/f{i}.rs"}}"#);
            t.record_tool("write_file", &args, "ok", false);
        }
        assert_eq!(
            t.modified_files().len(),
            50,
            "the cap bounds the staple's size"
        );
    }

    #[test]
    fn renders_every_block_with_explicit_absence() {
        let t = SessionFactTracker::default();
        let out = render_staple_block(&t, None);

        assert!(
            out.contains("<modified-files>"),
            "block header always present"
        );
        assert!(out.contains("<last-run>"), "block header always present");
        assert!(
            out.contains("None"),
            "absence is stated, not omitted — grading must tell 'none' from 'forgot'"
        );
    }

    #[test]
    fn renders_a_command_verbatim_with_pass_fail() {
        let mut t = SessionFactTracker::default();
        t.record_tool(
            "run_command",
            r#"{"command_line":"make lint"}"#,
            "ok",
            false,
        );
        let out = render_staple_block(&t, Some("fix the parser"));

        assert!(
            out.contains("make lint"),
            "the command is quoted, not summarized"
        );
        assert!(out.contains("PASS"), "outcome is a fixed literal");
    }

    #[test]
    fn carries_the_user_request_when_the_caller_supplies_it() {
        let t = SessionFactTracker::default();
        let out = render_staple_block(&t, Some("fix the parser"));

        assert!(out.contains("<latest-user-request>"));
        assert!(out.contains("fix the parser"));
    }

    #[test]
    fn the_staple_never_asserts_an_unresolved_error() {
        // The harness cannot know whether an error was later fixed, so a
        // harness-written `None` would be a false claim of completeness —
        // the defect class this change exists to remove. The prompt keeps
        // asking the model for the block; the staple must not carry it.
        let t = SessionFactTracker::default();
        let out = render_staple_block(&t, None);

        assert!(
            !out.contains("<unresolved-error>"),
            "the harness staple must omit the block it cannot know"
        );
    }
}
