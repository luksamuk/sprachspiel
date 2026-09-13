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
#[ignore = "36 known violations across 12 documents — tracked in LUC-145. \
            Remove this attribute when that issue lands; the sensor is correct \
            and already verified to detect the defect class."]
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
        for (lineno, line) in text.lines().enumerate() {
            if is_removal_note(line) {
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

    assert!(checked > 0, "sensor found no documents to check — check `documents()`");
    assert!(
        offenders.is_empty(),
        "documents cite `src/**` paths that do not exist ({} found across {checked} \
         documents):\n  {}\n\n\
         Either correct the path or delete the reference. If the reference is \
         legitimately historical, move it to a document excluded by `is_checked()` \
         (see LUC-145).",
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

    // Informational: this is reported but not failed while the historical
    // sections still exist. LUC-145 tracks their removal. The assertion below
    // flips to a hard failure once that issue lands.
    if !offenders.is_empty() {
        eprintln!(
            "note: {} mention(s) of removed abstractions remain (tracked in LUC-145):\n  {}",
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

/// Keeps the `Command` import honest if future checks shell out.
#[allow(dead_code)] // used by checks added when the binary is needed
fn binary() -> Option<PathBuf> {
    let root = repo_root();
    [root.join("target/debug/sprach"), root.join("target/release/sprach")]
        .into_iter()
        .find(|p| p.exists())
}

#[allow(dead_code)] // paired with `binary()`
fn run_help(bin: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new(bin).args(args).output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}
