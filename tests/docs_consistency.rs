//! Documentation/help consistency sensor.
//!
//! Guards against a recurring defect class: the user-facing documentation and
//! `--help` output advertising commands or flags that do not exist. This has
//! has now happened twice: the docs advertised `-d` and `--list-models` long
//! after both stopped existing, and the CLI source still printed the pre-rename
//! binary name `ask` in help strings and usage errors.
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

/// Collect every `.md` file under `doc/src` (the user-facing mdBook sources),
/// plus the root-level `README.md` and `SMOKE_TEST.md` — they carry the same
/// commands and markers and drifted the same way. `README.md` was missing for a
/// long time: `sprach ocr --detailed` (a flag that has never existed) survived
/// there unnoticed precisely because this walker did not look at it.
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
    for name in ["README.md", "SMOKE_TEST.md"] {
        let p = repo_root().join(name);
        if p.exists() {
            out.push(p);
        }
    }
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
         fix the doc.",
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
            if line_uses_debug_flag(line) {
                offenders.push(format!("{}:{} — `{}`", rel(&path), lineno + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "documentation uses the nonexistent `-d` flag:\n  {}\n\n\
         `sprach -d \"x\"` errors with 'unexpected argument'. Use `-v` (verbose) \
         or `-vv` (trace).",
        offenders.join("\n  ")
    );
}

/// True when `-d` is presented as a CLI flag anywhere on the line.
///
/// Broader than a `sprach ...` prefix scan on purpose: the first version of
/// this sensor only matched lines starting with `sprach `, so prose like
/// "Use `-d` to troubleshoot problems" (query.md) and "Enable debug mode with
/// `-d`" (introduction.md) slipped through — a structural false negative.
///
/// Now matches either a `sprach` invocation token or a backtick-quoted `-d`,
/// while still ignoring unrelated flags: `curl -d`, `exiftool -d`, and
/// hyphenated words (`-debug`, `--dry-run`).
fn line_uses_debug_flag(line: &str) -> bool {
    // Backtick-quoted flag in prose: `-d`
    if line.contains("`-d`") {
        return true;
    }
    // Standalone token in a sprach invocation.
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
            let has_real_prompt =
                extract_quoted(cmd).is_some_and(|q| q.contains(' ') && !q.starts_with('$'));
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
         is parsed as another filename:\n  {}\n\n\
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

/// Every subcommand's `--help` must print the current binary name, and no
/// `--help` may name a model that does not exist. The project was renamed from
/// `ask-ai` to `sprachspiel` (short binary `sprach`), and the rename missed the
/// help strings in several CLI modules.
///
/// Also guards the model-example class: `sprach --help` advertised
/// `-m lfm` as "the default" while `DEFAULT_MODEL` is `qwen3.5:4b` and `lfm` is
/// not a valid model at all.
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

    // The `--list` output carries the examples (not `--help`), and it used to
    // advertise `-m lfm` as the default model long after `lfm` stopped
    // existing — `sprach -m lfm "x"` → "Unknown model 'lfm'".
    if let Ok(out) = Command::new(&bin).arg("--list").output() {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.contains("-m lfm") {
                offenders.push(format!("`sprach --list` prints: {trimmed}"));
            }
        }
    }

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
        "--help output uses the pre-rename binary name `ask` or a nonexistent \
         model example:\n  {}",
        offenders.join("\n  ")
    );
}

/// No documented `sprach <sub> -v` — verbosity is declared only on the top-level
/// `Cli`, so the flag must precede the subcommand. Five command pages had
/// listed `-v`/`-vv` in their per-subcommand option tables.
#[test]
fn verbosity_is_not_documented_as_a_subcommand_flag() {
    let manifest = repo_root();
    let candidates = [
        manifest.join("target/debug/sprach"),
        manifest.join("target/release/sprach"),
    ];
    let Some(bin) = candidates.into_iter().find(|p| p.exists()) else {
        return;
    };

    let subcommands = ["translate", "query", "ocr", "summarize", "chat", "vision"];

    // Only assert the ones that actually reject it, so the test cannot produce
    // a false positive if a subcommand later gains its own verbosity flag.
    let rejects: Vec<&str> = subcommands
        .into_iter()
        .filter(|sub| {
            Command::new(&bin)
                .args([sub, "-v"])
                .output()
                .map(|o| {
                    let err = String::from_utf8_lossy(&o.stderr);
                    err.contains("unexpected argument")
                })
                .unwrap_or(false)
        })
        .collect();

    let mut offenders = Vec::new();
    for path in doc_sources() {
        if is_changelog(&path) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (lineno, line) in content.lines().enumerate() {
            // A line may chain several invocations with pipes; check each.
            for segment in line.split('|') {
                let trimmed = segment.trim();
                let Some(rest) = trimmed.strip_prefix("sprach ") else {
                    continue;
                };
                let toks: Vec<&str> = rest.split_whitespace().collect();
                let Some(pos) = toks.iter().position(|t| rejects.contains(t)) else {
                    continue;
                };
                if toks[pos + 1..].iter().any(|t| *t == "-v" || *t == "-vv") {
                    offenders.push(format!(
                        "{}:{} — `{trimmed}` (verbosity must precede the subcommand)",
                        rel(&path),
                        lineno + 1
                    ));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "docs show `-v` after a subcommand that rejects it:\n  {}\n\n\
         Correct form: `sprach -v <sub> ...`.",
        offenders.join("\n  ")
    );
}

/// Every CLI example in the docs must be accepted by the parser. This is the
/// sensor for the flag-ordering class: docs showed `sprach chat --plain`,
/// which the parser rejects because `--plain` is top-level-only.
///
/// Only commands whose subcommand is known to the test are checked, and only
/// when a binary is available.
#[test]
fn documented_cli_examples_are_parseable() {
    let manifest = repo_root();
    let candidates = [
        manifest.join("target/debug/sprach"),
        manifest.join("target/release/sprach"),
    ];
    let Some(_bin) = candidates.into_iter().find(|p| p.exists()) else {
        return;
    };

    // Flags that exist only on the top-level `Cli` and never on a subcommand.
    // Verified against src/main.rs; `--list` is excluded because `translate`
    // declares its own.
    const TOPLEVEL_ONLY: &[&str] = &["--plain", "--code", "-q", "--db", "--force"];

    let subcommands = [
        "translate",
        "query",
        "ocr",
        "summarize",
        "chat",
        "vision",
        "diagnostics",
    ];

    let mut offenders = Vec::new();
    for path in doc_sources() {
        if is_changelog(&path) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (lineno, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let Some(rest) = trimmed.strip_prefix("sprach ") else {
                continue;
            };
            // Identify the subcommand (the first token that is one).
            let toks: Vec<&str> = rest.split_whitespace().collect();
            let Some(pos) = toks.iter().position(|t| subcommands.contains(t)) else {
                continue;
            };
            let (sub, after) = (toks[pos], &toks[pos + 1..]);
            for flag in TOPLEVEL_ONLY {
                if after.contains(flag) {
                    offenders.push(format!(
                        "{}:{} — `{trimmed}` puts {flag} after `{sub}`, but it is \
                         top-level-only",
                        rel(&path),
                        lineno + 1
                    ));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "documented CLI examples are rejected by the parser because a \
         top-level-only flag follows the subcommand:\n  {}\n\n\
         Global flags must precede the subcommand: `sprach --plain query \"x\"`.",
        offenders.join("\n  ")
    );
}

/// No documented flag that a subcommand does not declare.
///
/// The sibling check above only catches flags that are *misplaced*; it never
/// asked whether a flag **exists**. That blind spot let `sprach ocr --detailed`
/// sit in `README.md` and `sprach chat --context 4096` in the context docs —
/// neither flag has ever existed.
///
/// The set of valid flags is read from the **live clap definitions**, not from a
/// hand-maintained list: a hardcoded list is exactly the kind of thing that goes
/// stale and re-creates the defect this guards. `sprach <sub> -h` is parsed by
/// the binary for the same reason.
///
/// Deliberately narrow — a false positive makes the sensor worthless, so three
/// shapes are skipped rather than guessed at:
///
/// - **Pipelines.** `sprach ocr a.png | sprach summarize --style x` gives
///   `--style` to *summarize*. Scan stops at `|`, `&&`, `;`, `>`.
/// - **Nested subcommands.** `sprach config upgrade --dry-run` gives
///   `--dry-run` to `upgrade`; when the token after the subcommand is another
///   subcommand name, that pair's flags are consulted.
/// - **`--`.** The vision prompt separator, not a flag.
#[test]
fn documented_subcommand_flags_exist() {
    let manifest = repo_root();
    let candidates = [
        manifest.join("target/debug/sprach"),
        manifest.join("target/release/sprach"),
    ];
    let Some(bin) = candidates.into_iter().find(|p| p.exists()) else {
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

    // Top-level-only flags, valid before any subcommand. Sourced from
    // `src/main.rs`; a subcommand that declares its own `--list` still passes
    // because the per-subcommand set is consulted first.
    const TOPLEVEL: &[&str] = &[
        "--plain",
        "--code",
        "-c",
        "-q",
        "--db",
        "--force",
        "--tools",
        "--ignore-agents",
        "--soulless",
        "-m",
        "--model",
        "-t",
        "--think",
        "-v",
        "--verbose",
        "--help",
        "-h",
        "--version",
        "-V",
        "-l",
        "--list",
    ];

    /// Extract the flag-looking tokens from a `-h` rendering.
    fn flags_of(text: &str) -> std::collections::HashSet<String> {
        let mut set = std::collections::HashSet::new();
        for word in text.split(|c: char| c.is_whitespace() || c == ',' || c == '=') {
            let w = word.trim_matches(|c: char| c == '[' || c == ']' || c == '<' || c == '>');
            if w.starts_with('-') && w.len() > 1 {
                set.insert(w.to_string());
            }
        }
        set
    }

    // Learn the flags of `sprach <sub>` and `sprach <sub> <nested>` from clap.
    let mut valid: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();
    for sub in subcommands {
        let Ok(out) = Command::new(&bin).args([sub, "-h"]).output() else {
            continue;
        };
        let text = String::from_utf8_lossy(&out.stdout);
        let set = flags_of(&text);
        // Nested subcommands: any word the help lists as a subcommand.
        for nested in ["upgrade", "up"] {
            if let Ok(nout) = Command::new(&bin).args([sub, nested, "-h"]).output() {
                if nout.status.success() {
                    let ntext = String::from_utf8_lossy(&nout.stdout);
                    let nset = flags_of(&ntext);
                    if !nset.is_empty() {
                        valid.insert(format!("{sub} {nested}"), nset);
                    }
                }
            }
        }
        valid.insert(sub.to_string(), set);
    }
    if valid.is_empty() {
        return;
    }

    // Shell operators that end this command's argument list.
    const PIPES: &[&str] = &["|", "&&", "||", ";", ">", ">>", "2>", "2>&1"];

    let mut offenders = Vec::new();
    for path in doc_sources() {
        if is_changelog(&path) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (lineno, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let Some(rest) = trimmed.strip_prefix("sprach ") else {
                continue;
            };
            let toks: Vec<&str> = rest.split_whitespace().collect();
            let Some(pos) = toks.iter().position(|t| subcommands.contains(t)) else {
                continue;
            };
            let sub = toks[pos];
            // Nested subcommand? (`config upgrade --dry-run`)
            let mut key = sub.to_string();
            let mut args_start = pos + 1;
            if toks.len() > pos + 1 {
                let candidate = format!("{} {}", sub, toks[pos + 1]);
                if valid.contains_key(&candidate) {
                    key = candidate;
                    args_start = pos + 2;
                }
            }
            let Some(known) = valid.get(&key) else {
                continue;
            };
            for tok in &toks[args_start..] {
                if PIPES.contains(tok) {
                    break;
                }
                // `--` is the vision prompt separator, not a flag.
                if *tok == "--" {
                    break;
                }
                if !tok.starts_with('-') || tok.len() < 2 {
                    continue;
                }
                let name = tok.split('=').next().unwrap_or(tok);
                if TOPLEVEL.contains(&name) || known.contains(name) {
                    continue;
                }
                offenders.push(format!(
                    "{}:{} — `{trimmed}` passes `{name}` to `{key}`, which does not \
                     declare it",
                    rel(&path),
                    lineno + 1
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "documentation shows flags that do not exist on the subcommand \
:
  {}",
        offenders.join("\n  ")
    );
}

/// Version and schema-version markers in docs must match the crate and the
/// database. This drifted four separate times (IMPLEMENTATION.md, roadmap.md,
/// implementation-status.md, SMOKE_TEST.md), which is why it gets a sensor
/// rather than another manual sweep.
#[test]
fn docs_version_and_schema_markers_match_code() {
    let manifest = repo_root();

    // Cargo.toml is the source of truth for the version.
    let cargo =
        std::fs::read_to_string(manifest.join("Cargo.toml")).expect("Cargo.toml must be readable");
    let version = cargo
        .lines()
        .find_map(|l| l.strip_prefix("version = "))
        .map(|v| v.trim().trim_matches('"').to_string())
        .expect("Cargo.toml must declare a version");

    // schema.rs is the source of truth for the schema version.
    let schema = std::fs::read_to_string(manifest.join("src/db/schema.rs"))
        .expect("src/db/schema.rs must be readable");
    let schema_version = schema
        .lines()
        .find_map(|l| l.trim().strip_prefix("pub const SCHEMA_VERSION: i32 = "))
        .map(|v| v.trim_end_matches(';').to_string())
        .expect("schema.rs must declare SCHEMA_VERSION");

    let mut offenders = Vec::new();

    // `schema vN` must always name the live version.
    let stale_schema = format!("schema v{}", schema_version.parse::<i32>().unwrap_or(0) - 1);
    for path in doc_sources() {
        if is_changelog(&path) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (lineno, line) in content.lines().enumerate() {
            if line.contains(&stale_schema) {
                offenders.push(format!(
                    "{}:{} says `{stale_schema}` but SCHEMA_VERSION is {schema_version}",
                    rel(&path),
                    lineno + 1
                ));
            }
        }
    }

    // The "Current Version" marker in the trackers must be the crate version.
    for name in [
        "IMPLEMENTATION.md",
        "doc/src/development/implementation-status.md",
    ] {
        let path = manifest.join(name);
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(idx) = content.find("## Current Version") else {
            continue;
        };
        // Look at the few lines following the heading.
        let window: String = content[idx..]
            .lines()
            .take(6)
            .collect::<Vec<_>>()
            .join("\n");
        if !window.contains(&format!("v{version}")) {
            offenders.push(format!(
                "{name}: \"Current Version\" does not mention v{version} (Cargo.toml)"
            ));
        }
    }

    assert!(
        offenders.is_empty(),
        "documentation version/schema markers drifted from the code:\n  {}\n\n\
         Cargo.toml version = {version}; SCHEMA_VERSION = {schema_version}.",
        offenders.join("\n  ")
    );
}
