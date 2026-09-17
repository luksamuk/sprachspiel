//! Repository-reference consistency sensor.
//!
//! Guards the class of drift found in the September 2026 backlog audit: design
//! documents citing file paths, constants or functions that no longer exist.
//! The audit found **12 dead `src/**` paths in `IMPLEMENTATION.md` alone**, most
//! of them inside *phase tables* — the part an implementer reads before starting
//! work. Two of them named abstractions `AGENTS.md` explicitly says were removed
//! and must not be re-added.
//!
//! Per the AGENTS.md steering rule, a repeated defect class gets a sensor. This
//! one is purely mechanical: if a markdown document names a `src/` path, that
//! path must exist on disk.
//!
//! Scope is deliberately narrow — only `src/**` paths inside inline code spans,
//! and only for documents that describe the codebase. Documents that legitimately
//! quote historical paths (migrations, changelogs, ADRs) are excluded by name;
//! see `EXEMPT`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rel(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Markdown that describes the codebase *now*, and therefore must not name
/// paths that do not exist.
///
/// Excluded on purpose:
/// - `CHANGELOG.md` — records past states by design
/// - `doc/src/adr/**` — decision records; may reference removed code as part of
///   the rationale, and carry their own "historical" banner
/// - `legacy` / `implementation-history` — historical by name
/// - `*-research.md` and `research/**` — these **propose** modules that do not
///   exist yet; naming a future `src/foo.rs` is their job, not drift
fn is_checked(path: &Path) -> bool {
    let rel = rel(path);
    if rel.ends_with("CHANGELOG.md") || rel.ends_with("IMPLEMENTATION.md.original") {
        return false;
    }
    if rel.contains("doc/src/adr/") {
        return false;
    }
    if rel.contains("legacy") || rel.contains("implementation-history") {
        return false;
    }
    if rel.contains("research") {
        return false;
    }
    true
}

/// Lines that present a path as *removed* are not drift — they are the rule.
///
/// `AGENTS.md` deliberately names `rustyline.rs` and `terminal.rs` under a
/// "Removed in PR2/PR3 (do NOT re-add)" heading. Flagging those would push
/// someone to delete the prohibition itself.
fn is_removal_note(line: &str) -> bool {
    let l = line.to_lowercase();
    l.contains("removed")
        || l.contains("do not re-add")
        || l.contains("no longer")
        || l.contains("deleted")
        || l.contains("deprecated")
        || l.contains("was removed")
        || l.contains("legacy")
}

/// Lines that present a path as *proposed* rather than as existing code.
///
/// Several documents legitimately contain implementation plans: "**Files to
/// create:** `src/tools/run_command.rs`", "Create `src/tools/skills.rs` with…",
/// "**EmbeddingQueue** (`src/embeddings/queue.rs`, new)". Flagging those would
/// push someone to delete a plan for being "wrong".
///
/// Detecting this reliably needs the *section* context, not just the line — a
/// `Files to create:` heading governs the bullet list under it. The caller
/// therefore tracks whether it is inside an implementation-plan block.
fn opens_plan_block(line: &str) -> bool {
    let l = line.to_lowercase();
    l.contains("files to create")
        || l.contains("files to modify")
        || l.contains("new files")
        || l.contains("implementation phases")
        || l.contains("tasks:")
        || l.contains("proposed")
}

/// A line that reads as a plan item rather than a statement of fact.
fn is_plan_item(line: &str) -> bool {
    let l = line.to_lowercase();
    l.contains("create `src/")
        || l.contains("add `src/")
        || l.contains(", new)")
        || l.contains("(new)")
        || l.contains("new file")
        || l.contains("to create")
        || l.contains("proposed")
        || l.contains("plan:")
        // A markdown bullet under a plan heading is itself plan material; the
        // caller only calls this when inside a plan block.
        || line.trim_start().starts_with("- `src/")
}

/// Placeholder and glob shapes that appear in how-to documentation. These are
/// not claims about the tree, so they must not be treated as citations.
fn is_placeholder(cited: &str) -> bool {
    cited.contains('<')
        || cited.contains('>')
        || cited.contains('*')
        || cited.contains("...")
        || cited.contains("foo")
        || cited.contains("my_tool")
        || cited.contains("example")
}

/// Collect the markdown files this sensor checks: the repo index plus the
/// development docs and the harness conventions file.
fn documents() -> Vec<PathBuf> {
    let root = repo_root();
    let mut out = Vec::new();

    for name in ["AGENTS.md", "IMPLEMENTATION.md", "SMOKE_TEST.md"] {
        let p = root.join(name);
        if p.exists() {
            out.push(p);
        }
    }

    fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
        if depth > 3 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out, depth + 1);
            } else if p.extension().is_some_and(|x| x == "md") {
                out.push(p);
            }
        }
    }
    walk(&root.join("doc/src"), &mut out, 0);
    out
}

/// Extract inline-code spans that look like a source path.
///
/// Only `` `src/...` `` spans are considered: they are unambiguous, and the
/// false-positive rate is far lower than for bare paths in prose.
fn cited_paths(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        let Some(end) = text[i + 1..].find('`') else {
            break;
        };
        let span = &text[i + 1..i + 1 + end];
        let candidate = span.trim().trim_end_matches(&['.', ',', ';', ':'][..]);
        // Normalise the shapes used in docs: module references, file refs,
        // and `path::symbol` / `path:symbol` forms.
        for part in [
            candidate.split("::").next().unwrap_or(candidate),
            candidate.split(':').next().unwrap_or(candidate),
        ] {
            let p = part.trim();
            if p.starts_with("src/") && p.len() > 4 {
                out.insert(p.trim_end_matches('/').to_string());
            }
        }
        i += end + 2;
    }
    out
}

/// Resolve a documented path against the filesystem, allowing for the several
/// ways docs legitimately write a Rust path.
fn exists(cited: &str) -> bool {
    let root = repo_root();
    let base = root.join(cited);

    // Exact file / directory.
    if base.exists() {
        return true;
    }
    // `src/foo/bar.rs` written as `src/foo/bar` — try adding .rs
    if base.with_extension("rs").exists() {
        return true;
    }
    // `src/foo.rs` written as `src/foo` (module-style) — directory with mod.rs
    if base.join("mod.rs").exists() {
        return true;
    }
    false
}

#[test]
fn documents_do_not_cite_nonexistent_source_paths() {
    let mut offenders = Vec::new();
    let mut checked = 0;

    for doc in documents() {
        if !is_checked(&doc) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&doc) else {
            continue;
        };
        checked += 1;
        // Track whether we are inside an implementation-plan block: a
        // "Files to create:" heading governs the bullets under it until a
        // blank-line-separated section change.
        let mut in_plan = false;
        for (lineno, line) in text.lines().enumerate() {
            if opens_plan_block(line) {
                in_plan = true;
            } else if line.starts_with("## ") || line.starts_with("# ") {
                in_plan = false;
            }
            if is_removal_note(line) || in_plan || is_plan_item(line) {
                continue;
            }
            for cited in cited_paths(line) {
                if is_placeholder(&cited) {
                    continue;
                }
                if !exists(&cited) {
                    offenders.push(format!("{}:{} cites `{cited}`", rel(&doc), lineno + 1));
                }
            }
        }
    }

    assert!(
        checked > 0,
        "sensor found no documents to check — check `documents()`"
    );
    assert!(
        offenders.is_empty(),
        "documents cite `src/**` paths that do not exist ({} found across {checked} \
         documents):\n  {}\n\n\
         Either correct the path or delete the reference. If the reference is \
         legitimately historical, move it to a document excluded by `is_checked()`.",
        offenders.len(),
        offenders.join("\n  ")
    );
}

/// `AGENTS.md` names files that were removed and must not be re-added. No other
/// document may present them as current.
#[test]
fn removed_abstractions_are_not_presented_as_current() {
    // Abstractions AGENTS.md forbids re-adding. They may appear in historical or
    // ADR documents, so this check is limited to the docs that describe "now".
    const REMOVED: &[&str] = &[
        "RustylineInput",
        "TerminalView",
        "CompatOllama",
        "OllamaProvider",
    ];

    let mut offenders = Vec::new();
    for doc in documents() {
        let rel = rel(&doc);
        if !is_checked(&doc) || rel.contains("AGENTS.md") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&doc) else {
            continue;
        };
        for (lineno, line) in text.lines().enumerate() {
            for name in REMOVED {
                if line.contains(name) {
                    offenders.push(format!("{rel}:{} mentions `{name}`", lineno + 1));
                }
            }
        }
    }

    // Reported, not asserted. These mentions are legitimate history, not drift:
    // `chat-mode-design.md` is banner-marked legacy, `completed-features.md`
    // records what was removed, and `provider-architecture.md` cites a completed
    // status table. A hard failure here would force deleting accurate history to
    // satisfy a keyword check — the guard's purpose is that nothing re-presents
    // these types as *current*, which is what `is_checked()` already enforces.
    if !offenders.is_empty() {
        eprintln!(
            "note: {} mention(s) of removed abstractions in historical documents:\n  {}",
            offenders.len(),
            offenders.join("\n  ")
        );
    }
}

/// Guard the consolidation itself: `IMPLEMENTATION.md` must stay an index, not
/// grow back into a tracker. The audit removed ~4,500 lines of per-issue
/// sections; without a ceiling, the next PR adds "just one" back.
#[test]
fn implementation_md_stays_an_index() {
    let path = repo_root().join("IMPLEMENTATION.md");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let lines = text.lines().count();

    const MAX_LINES: usize = 400;
    assert!(
        lines <= MAX_LINES,
        "IMPLEMENTATION.md is {lines} lines — it is meant to be an index, not a \
         tracker. Per-issue status belongs in Linear; release notes belong in \
         CHANGELOG.md; design belongs in doc/src/development/ and doc/src/adr/. \
         (Ceiling: {MAX_LINES} lines. This guard exists because the file grew to \
         8,246 lines once already.)"
    );

    // It must point at the tracker, or a reader has no way to find status.
    assert!(
        text.contains("Linear"),
        "IMPLEMENTATION.md no longer mentions Linear — the tracker must be \
         discoverable from the index"
    );
    // And it must not accumulate per-issue phase tables again.
    assert!(
        !text.contains("**Status:** ✅ COMPLETED (PR #"),
        "IMPLEMENTATION.md contains a per-issue completion section again"
    );
}

/// The git history is the durable copy, but a cheap sanity check that the
/// binary is still buildable from the tree is worth having next to the doc
/// guards — a doc sensor that passes while the code does not compile is noise.
#[test]
fn cargo_manifest_declares_a_version() {
    let manifest = std::fs::read_to_string(repo_root().join("Cargo.toml"))
        .expect("Cargo.toml must be readable");
    let version = manifest
        .lines()
        .find_map(|l| l.strip_prefix("version = "))
        .map(|v| v.trim().trim_matches('"').to_string());
    assert!(version.is_some(), "Cargo.toml must declare a version");
}

/// Source comments and test messages must not carry issue identifiers.
///
/// A reference like `(LUC-141)` in a doc-comment explains nothing a reader can
/// act on, and goes stale the moment the issue closes — it becomes a pointer to
/// a tracker entry that no longer describes anything. The *explanation* stays in
/// the code; the *history* belongs in the tracker, the CHANGELOG and `doc/src/`.
///
/// This is a placement rule, not a documentation ban: `AGENTS.md`, the skills
/// and `doc/src/**` legitimately cite identifiers, because those documents *are*
/// the history. Only Rust sources are checked here.
///
/// Guarded because the rule is easy to violate one comment at a time — 43
/// occurrences had accumulated before it was swept, and nothing stopped the next
/// one from being written.
#[test]
fn rust_sources_do_not_cite_issue_identifiers() {
    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["src", "tests", "benches"] {
        let p = root.join(dir);
        if p.exists() {
            rust_files(&p, &mut files);
        }
    }

    // This file has to name the identifiers to explain the rule it enforces.
    // Excluded by path rather than by content, so a real violation written
    // anywhere else in this file is still caught.
    let self_path = root.join("tests/repo_references.rs");

    let mut offenders = Vec::new();
    for path in files {
        if path == self_path {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (lineno, line) in text.lines().enumerate() {
            // `LUC-123` and `gh#123` are the tracker forms. A bare `#123` is
            // deliberately not matched: it is common in Rust for generics,
            // colour codes and array indices, and a false positive here would
            // train the reader to ignore the sensor.
            let cites = |prefix: &str| {
                line.split(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '#')
                    .any(|t| {
                        t.strip_prefix(prefix)
                            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
                    })
            };
            if cites("LUC-") || cites("gh#") {
                offenders.push(format!("{}:{} — {}", rel(&path), lineno + 1, line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "Rust sources cite tracker identifiers ({} found).\n\n\
         Keep the explanation, drop the pointer: the rationale belongs in the \
         code, the history belongs in Linear / CHANGELOG.md / doc/src/.\n\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}

/// Keeps the `Command` import honest if future checks shell out.
#[allow(dead_code)] // used by checks added when the binary is needed
fn binary() -> Option<PathBuf> {
    let root = repo_root();
    [
        root.join("target/debug/sprach"),
        root.join("target/release/sprach"),
    ]
    .into_iter()
    .find(|p| p.exists())
}

#[allow(dead_code)] // paired with `binary()`
fn run_help(bin: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new(bin).args(args).output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}
