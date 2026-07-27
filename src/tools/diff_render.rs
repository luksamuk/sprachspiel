//! Diff generation and formatting for file edit tool results.
//!
//! Provides structured diff hunks via the `similar` crate, text formatting
//! for LLM tool results (```diff fences), and ANSI-colored output for
//! query/code mode.

use similar::{ChangeTag, TextDiff};

/// A single line in a diff hunk.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffLine {
    /// Line text (without trailing newline).
    pub text: String,
    /// Line number in the old file (1-indexed, 0 if insert-only).
    pub old_line: usize,
    /// Line number in the new file (1-indexed, 0 if delete-only).
    pub new_line: usize,
    /// Whether this line was added, removed, or unchanged.
    pub tag: ChangeTag,
}

/// A hunk is a contiguous group of diff lines with context.
pub type DiffHunk = Vec<DiffLine>;

/// Maximum context lines around each change.
const MAX_CONTEXT: usize = 3;

/// Maximum diff lines to show (prevents flooding context for large edits).
const MAX_DIFF_LINES: usize = 100;

/// Generate diff hunks from old and new content.
///
/// Uses `similar::TextDiff` with line-level granularity.
/// Returns hunks with ±3 lines of context around each change.
/// Truncated at [`MAX_DIFF_LINES`] total changed lines.
pub fn generate_diff_hunks(original: &str, new: &str) -> Vec<DiffHunk> {
    let diff = TextDiff::from_lines(original, new);
    let mut all_diff_lines: Vec<DiffLine> = Vec::new();
    let mut old_line: usize = 0;
    let mut new_line: usize = 0;

    for change in diff.iter_all_changes() {
        let tag = change.tag();
        let text = change.value().trim_end_matches(['\r', '\n']).to_string();

        let (lo, ln) = match tag {
            ChangeTag::Equal => {
                let lo = old_line + 1;
                let ln = new_line + 1;
                old_line += 1;
                new_line += 1;
                (lo, ln)
            }
            ChangeTag::Delete => {
                let lo = old_line + 1;
                old_line += 1;
                (lo, 0)
            }
            ChangeTag::Insert => {
                let ln = new_line + 1;
                new_line += 1;
                (0, ln)
            }
        };

        all_diff_lines.push(DiffLine {
            text,
            old_line: lo,
            new_line: ln,
            tag,
        });
    }

    // Group into hunks: split on runs of Equal lines longer than 2*MAX_CONTEXT
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut current_hunk: DiffHunk = Vec::new();
    let mut equal_run: usize = 0;

    for line in all_diff_lines {
        if line.tag == ChangeTag::Equal {
            equal_run += 1;
        } else {
            // Flush any accumulated equal lines if the run was too long
            if equal_run > 2 * MAX_CONTEXT && !current_hunk.is_empty() {
                // Trim trailing equal lines to MAX_CONTEXT
                let trailing_equal: usize = current_hunk
                    .iter()
                    .rev()
                    .take_while(|l| l.tag == ChangeTag::Equal)
                    .count();
                if trailing_equal > MAX_CONTEXT {
                    current_hunk.truncate(current_hunk.len() - (trailing_equal - MAX_CONTEXT));
                }
                if !current_hunk.is_empty() {
                    hunks.push(std::mem::take(&mut current_hunk));
                }
                // Skip the long equal run entirely
                equal_run = 0;
                continue;
            }
            equal_run = 0;
        }
        current_hunk.push(line);
    }

    // Flush last hunk — trim trailing equal lines
    if !current_hunk.is_empty() {
        let trailing_equal: usize = current_hunk
            .iter()
            .rev()
            .take_while(|l| l.tag == ChangeTag::Equal)
            .count();
        if trailing_equal > MAX_CONTEXT {
            current_hunk.truncate(current_hunk.len() - (trailing_equal - MAX_CONTEXT));
        }
        // Also trim leading equal lines
        let leading_equal: usize = current_hunk
            .iter()
            .take_while(|l| l.tag == ChangeTag::Equal)
            .count();
        if leading_equal > MAX_CONTEXT {
            current_hunk.drain(0..(leading_equal - MAX_CONTEXT));
        }
        if !current_hunk.is_empty() {
            hunks.push(current_hunk);
        }
    }

    // Trim leading equal from first hunk
    if let Some(first) = hunks.first_mut() {
        let leading_equal: usize = first
            .iter()
            .take_while(|l| l.tag == ChangeTag::Equal)
            .count();
        if leading_equal > MAX_CONTEXT {
            first.drain(0..(leading_equal - MAX_CONTEXT));
        }
    }

    // Remove hunks that contain only Equal lines (no actual changes)
    hunks.retain(|h| h.iter().any(|l| l.tag != ChangeTag::Equal));

    hunks
}

/// Compute the number of added and removed lines.
pub fn compute_diff_stats(original: &str, new: &str) -> (usize, usize) {
    let diff = TextDiff::from_lines(original, new);
    let mut additions = 0;
    let mut removals = 0;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => additions += 1,
            ChangeTag::Delete => removals += 1,
            ChangeTag::Equal => {}
        }
    }
    (additions, removals)
}

/// Format diff hunks as unified diff text (for LLM tool result).
///
/// Uses `+`/`-`/` ` prefixes and `@@ -a,b +c,d @@` hunk headers.
/// Hunk separators show unchanged-line count between hunks.
/// Truncated at [`MAX_DIFF_LINES`] total changed lines with a summary message.
pub fn format_diff_as_text(hunks: &[DiffHunk]) -> String {
    let mut result = Vec::new();
    let mut shown: usize = 0;

    for (hunk_idx, hunk) in hunks.iter().enumerate() {
        if hunk.is_empty() {
            continue;
        }

        // Hunk separator between hunks
        if hunk_idx > 0 {
            result.push(String::new());
        }

        // Hunk header: @@ -start,count +start,count @@
        let first = &hunk[0];
        let old_start = first.old_line;
        let new_start = first.new_line;
        let old_count = hunk.iter().filter(|l| l.tag != ChangeTag::Insert).count();
        let new_count = hunk.iter().filter(|l| l.tag != ChangeTag::Delete).count();
        result.push(format!(
            "@@ -{},{} +{},{} @@",
            old_start, old_count, new_start, new_count
        ));

        for line in hunk {
            let prefix = match line.tag {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            };

            if line.tag != ChangeTag::Equal {
                shown += 1;
                if shown > MAX_DIFF_LINES {
                    let total_changes: usize = hunks
                        .iter()
                        .map(|h| h.iter().filter(|l| l.tag != ChangeTag::Equal).count())
                        .sum();
                    result.push(format!(
                        "... ({} more changes, diff truncated at {} lines)",
                        total_changes - shown + 1,
                        MAX_DIFF_LINES
                    ));
                    return result.join("\n");
                }
            }

            result.push(format!("{}{}", prefix, line.text));
        }
    }

    result.join("\n")
}

/// Render diff text as ANSI-colored string (for query/code mode).
///
/// `\033[32m` (green) for `+` lines, `\033[31m` (red) for `-` lines,
/// `\033[36m` (cyan) for `@@` hunk headers. Resets with `\033[0m`.
pub fn render_diff_ansi(diff_text: &str) -> String {
    let mut output = String::new();
    for line in diff_text.lines() {
        if line.starts_with("@@") {
            output.push_str("\x1b[36m");
            output.push_str(line);
            output.push_str("\x1b[0m");
        } else if line.starts_with('+') {
            output.push_str("\x1b[32m");
            output.push_str(line);
            output.push_str("\x1b[0m");
        } else if line.starts_with('-') {
            output.push_str("\x1b[31m");
            output.push_str(line);
            output.push_str("\x1b[0m");
        } else {
            output.push_str(line);
        }
        output.push('\n');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_diff_stats_replace() {
        let original = "line 1\nline 2\nline 3";
        let new = "line 1\nline 2 modified\nline 3";
        let (additions, removals) = compute_diff_stats(original, new);
        assert_eq!(additions, 1);
        assert_eq!(removals, 1);
    }

    #[test]
    fn test_compute_diff_stats_insert_only() {
        let original = "line 1\nline 3";
        let new = "line 1\nline 2\nline 3";
        let (additions, removals) = compute_diff_stats(original, new);
        assert_eq!(additions, 1);
        assert_eq!(removals, 0);
    }

    #[test]
    fn test_compute_diff_stats_delete_only() {
        let original = "line 1\nline 2\nline 3";
        let new = "line 1\nline 3";
        let (additions, removals) = compute_diff_stats(original, new);
        assert_eq!(additions, 0);
        assert_eq!(removals, 1);
    }

    #[test]
    fn test_generate_diff_hunks_replace() {
        let original = "line 1\nline 2\nline 3";
        let new = "line 1\nline 2 modified\nline 3";
        let hunks = generate_diff_hunks(original, new);
        assert!(!hunks.is_empty(), "Should produce at least one hunk");
        let hunk = &hunks[0];
        let inserts: usize = hunk.iter().filter(|l| l.tag == ChangeTag::Insert).count();
        let deletes: usize = hunk.iter().filter(|l| l.tag == ChangeTag::Delete).count();
        assert_eq!(inserts, 1, "Should have 1 insert");
        assert_eq!(deletes, 1, "Should have 1 delete");
    }

    #[test]
    fn test_generate_diff_hunks_insert_only() {
        let original = "line 1\nline 3";
        let new = "line 1\nline 2\nline 3";
        let hunks = generate_diff_hunks(original, new);
        assert!(!hunks.is_empty());
        let hunk = &hunks[0];
        let inserts: usize = hunk.iter().filter(|l| l.tag == ChangeTag::Insert).count();
        let deletes: usize = hunk.iter().filter(|l| l.tag == ChangeTag::Delete).count();
        assert_eq!(inserts, 1);
        assert_eq!(deletes, 0);
    }

    #[test]
    fn test_generate_diff_hunks_delete_only() {
        let original = "line 1\nline 2\nline 3";
        let new = "line 1\nline 3";
        let hunks = generate_diff_hunks(original, new);
        assert!(!hunks.is_empty());
        let hunk = &hunks[0];
        let inserts: usize = hunk.iter().filter(|l| l.tag == ChangeTag::Insert).count();
        let deletes: usize = hunk.iter().filter(|l| l.tag == ChangeTag::Delete).count();
        assert_eq!(inserts, 0);
        assert_eq!(deletes, 1);
    }

    #[test]
    fn test_format_diff_as_text_hunk_headers() {
        let original = "line 1\nline 2\nline 3";
        let new = "line 1\nline 2 modified\nline 3";
        let hunks = generate_diff_hunks(original, new);
        let text = format_diff_as_text(&hunks);
        assert!(text.contains("@@"), "Should contain hunk header: {}", text);
        assert!(
            text.contains("-line 2"),
            "Should show removed line: {}",
            text
        );
        assert!(
            text.contains("+line 2 modified"),
            "Should show added line: {}",
            text
        );
    }

    #[test]
    fn test_format_diff_as_text_empty_on_no_change() {
        let original = "line 1\nline 2";
        let hunks = generate_diff_hunks(original, original);
        let text = format_diff_as_text(&hunks);
        // No changes → no hunks with Insert/Delete → text should be empty or only context
        // Actually with no changes, all lines are Equal and get trimmed, producing empty hunks
        assert!(
            text.is_empty() || !text.contains('+') && !text.contains('-'),
            "No changes should produce no diff markers: {}",
            text
        );
    }

    #[test]
    fn test_render_diff_ansi_colors() {
        let diff_text = "@@ -1,2 +1,2 @@\n-old line\n+new line\n unchanged";
        let ansi = render_diff_ansi(diff_text);
        assert!(
            ansi.contains("\x1b[36m"),
            "Should have cyan for hunk header"
        );
        assert!(ansi.contains("\x1b[31m"), "Should have red for deletion");
        assert!(ansi.contains("\x1b[32m"), "Should have green for insertion");
        assert!(ansi.contains("\x1b[0m"), "Should have reset codes");
    }
}
