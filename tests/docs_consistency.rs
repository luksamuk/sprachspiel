//! Documentation/help consistency sensor.
//!
//! Guards against a recurring defect class: the user-facing documentation and
//! `--help` output advertising commands or flags that do not exist. This has
//! now happened twice — LUC-140 (ex gh#228) fixed `-d` and `--list-models` in
//! the docs, and LUC-142 tracks ~50 occurrences of the pre-rename binary name
//! `ask` still printed by the CLI source.
//!
//! The checks here are deliberately narrow and cheap: they scan repository text
//! for strings that are known-not-to-exist in the CLI. They do NOT parse the
//! docs — a false negative is acceptable, a false positive is not, so every
//! heuristic below is written to under-match rather than over-match.
//!
//! Per the AGENTS.md steering rule, a repeated bug must produce a sensor.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Repository root, derived from the test binary location so the test works
/// regardless of the invocation directory.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Collect every `.md` file under `doc/src` (the user-facing mdBook sources).
fn doc_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo_root().join("doc/src"), &mut out);
    out
}

/// `CHANGELOG.md` legitimately quotes the old flag names when describing the
/// fix, so it must be excluded from the stale-string scans.
fn is_changelog(path: &Path) -> bool {
    path.file_name().is_some_and(|n| n == "CHANGELOG.md")
}

/// Short repo-relative path for readable failure messages.
fn rel(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Lines in development/planning docs that discuss these very defects are
/// allowed to name them.
fn is_explanatory(line: &str) -> bool {
    line.contains("LUC-140") || line.contains("LUC-142") || line.contains("gh#228")
}

/// Flags removed or never implemented. Verified against the built binary:
/// both error with "unexpected argument".
const PHANTOM_FLAGS: &[&str] = &["--list-models", "--no-tui"];

#[test]
fn docs_do_not_reference_phantom_flags() {
    let mut offenders = Vec::new();
    for path in doc_sources() {
        if is_changelog(&path) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (lineno, line) in content.lines().enumerate() {
            if is_explanatory(line) {
                continue;
            }
            for flag in PHANTOM_FLAGS {
                if line.contains(flag) {
                    offenders.push(format!("{}:{} references `{flag}`", rel(&path), lineno + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "documentation references flags that do not exist in the CLI:\n  {}\n\n\
         Verify with `sprach <flag>` — if it errors with 'unexpected argument', \
         fix the doc (see LUC-140).",
        offenders.join("\n  ")
    );
}

/// `sprach -d` was the pre-verbosity debug flag. The real flags are `-v`
/// (verbose) and `-vv` (trace).
#[test]
fn docs_do_not_use_nonexistent_debug_flag() {
    let mut offenders = Vec::new();
    for path in doc_sources() {
        if is_changelog(&path) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (lineno, line) in content.lines().enumerate() {
            if is_explanatory(line) {
                continue;
            }
            if line_uses_debug_flag(line) {
                offenders.push(format!(
                    "{}:{} — `{}`",
                    rel(&path),
                    lineno + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "documentation uses the nonexistent `-d` flag (LUC-140):\n  {}\n\n\
         `sprach -d \"x\"` errors with 'unexpected argument'. Use `-v` (verbose) \
         or `-vv` (trace).",
        offenders.join("\n  ")
    );
}

/// True when `-d` appears as a standalone token in a `sprach ...` invocation.
///
/// Token-exact matching keeps unrelated `-d` uses (`curl -d '{}'`,
/// `exiftool -d "%Y"`) out of the results.
fn line_uses_debug_flag(line: &str) -> bool {
    let Some(rest) = line.trim().strip_prefix("sprach ") else {
        return false;
    };
    rest.split_whitespace().any(|tok| tok == "-d")
}

/// The `vision` subcommand's custom prompt is declared `last = true`, so it
/// must be separated from the file list by `--`. Examples that pass a bare
/// quoted prompt after a filename are wrong: the prompt is parsed as another
/// FILE (`Error: FILE NOT FOUND: 'prompt'`).
#[test]
fn vision_examples_separate_prompt_with_double_dash() {
    let mut offenders = Vec::new();
    for path in doc_sources() {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (lineno, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            // Only the CLI form: the in-chat `/vision` slash command has its
            // own parser and does NOT need `--`.
            let Some(cmd) = trimmed.strip_prefix("sprach vision ") else {
                continue;
            };
            // A prompt is a quoted string containing a space that is not a
            // shell variable (`"$img"`) and not a redirect target.
            let has_real_prompt = extract_quoted(cmd)
                .is_some_and(|q| q.contains(' ') && !q.starts_with('$'));
            let has_image = cmd.contains(".png")
                || cmd.contains(".jpg")
                || cmd.contains(".jpeg")
                || cmd.contains(".webp");
            let has_separator = cmd.contains(" -- ");
            if has_image && has_real_prompt && !has_separator {
                offenders.push(format!("{}:{} — `{trimmed}`", rel(&path), lineno + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "vision examples pass a prompt without the `--` separator, so the prompt \
         is parsed as another filename (LUC-140):\n  {}\n\n\
         Correct form: sprach vision photo.png -- \"prompt\"",
        offenders.join("\n  ")
    );
}

/// Return the contents of the first double-quoted span in `s`.
fn extract_quoted(s: &str) -> Option<&str> {
    let start = s.find('"')? + 1;
    let rest = &s[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Every subcommand's `--help` must print the current binary name. The project
/// was renamed from `ask-ai` to `sprachspiel` (short binary `sprach`), and the
/// rename missed the help strings in several CLI modules (LUC-142).
#[test]
fn help_output_uses_current_binary_name() {
    let manifest = repo_root();
    let candidates = [
        manifest.join("target/debug/sprach"),
        manifest.join("target/release/sprach"),
    ];
    let Some(bin) = candidates.into_iter().find(|p| p.exists()) else {
        // No built binary available — nothing to assert. Keeps the test useful
        // in CI (where `cargo test` builds one) without failing for a bare
        // `cargo check`.
        return;
    };

    let subcommands = [
        "translate",
        "query",
        "ocr",
        "summarize",
        "chat",
        "vision",
        "diagnostics",
        "config",
        "models",
    ];

    let mut offenders = Vec::new();
    for sub in subcommands {
        let Ok(out) = Command::new(&bin).args([sub, "--help"]).output() else {
            continue;
        };
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("ask ") {
                offenders.push(format!("`sprach {sub} --help` prints: {trimmed}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "--help output still uses the pre-rename binary name `ask` (LUC-142):\n  {}",
        offenders.join("\n  ")
    );
}
