---
name: quality-gates
description: Sensor hierarchy for commit and PR quality, dead code policy, and the steering rule that repeated bugs must produce guides or sensors. Load before committing or when review requires quality checks.
license: MIT
compatibility: opencode
metadata:
  audience: developers
  workflow: quality-gates
---

## What I do

I define the quality gates (sensors) that must pass before each commit and each PR, plus the steering rule for harness engineering. I am the authoritative source for these checks — AGENTS.md and PR-PROCESS.md reference me but do not duplicate me.

## When to use me

Load me when:
- About to commit code (run commit sensors)
- About to mark a PR ready for review (run PR sensors)
- A bug has repeated and you need to add a guide or sensor (steering rule)
- Reviewing code for `#[allow(dead_code)]` compliance

---

# Sensor Hierarchy — Run in Order of Cost

Sensors must be run **in order of cost**. Cheapest first — if a cheap sensor fails, don't bother running expensive ones.

## Before Each Commit

| Order | Command | Cost | What it catches |
|-------|---------|------|-----------------|
| 1 | `cargo fmt --check` | Instant (<1s) | Formatting violations |
| 2 | `cargo check --all-features` | Fast (<1min) | Compilation errors, type errors |

If either fails: **fix before committing. No exceptions.**

# Documentation Sensors

These are integration tests (`tests/*.rs`), not unit tests: they read the repo's own
documents and assert that what those documents claim is still true. They are part of
`cargo test --all-features`, and their failure means a document is lying — fix the
document, not the sensor.

| Sensor file | Guards |
|-------------|--------|
| `tests/repo_references.rs` | Documents name `src/**` paths that exist; removed abstractions are not presented as current; `IMPLEMENTATION.md` stays an index; `src/**` does not cite tracker identifiers |
| `tests/docs_consistency.rs` | Documented CLI flags exist in the live clap definition; examples parse; version/schema markers match the code |
| `tests/citation_integrity.rs` | Cited arXiv IDs are real and in the manifest; attributed surnames appear on the paper's real author list; a withdrawn paper is never cited as support; the bibliography table's author and year columns match arXiv |
| `tests/repo_references.rs` + `tests/docs_consistency.rs` | See AGENTS.md for the placement rule on tracker identifiers — out of `src/**`/`tests/**` assertion messages, allowed in `AGENTS.md`, skills, `doc/src/development/**`, `doc/src/CHANGELOG.md` |

**A new sensor is not done until it has been shown to fail.** Green on first run
proves only that the parser runs. Reinject the defect the sensor exists to catch,
watch it fail with a useful message, then restore. Two sensors written during the
September 2026 audits passed clean while covering nothing, because they excluded
the very file they lived in, or matched `LUC-140` as an exemption key.

## Citation sensor: what it can and cannot see

`citation_integrity.rs` verifies **identity** — the ID is real, the surname is on the
paper, the paper is not retracted. It cannot verify a **number**: "reduces latency by
~40%" against a paper reporting "up to 20%" is invisible to it, because reading a
claim back to its source is judgement, not string matching. Of the seven citation
defects found in the September 2026 audit, this sensor catches one class; the rest
are found by reading the paper.

Two consequences, both load-bearing:

- **Never describe the sensor as "citations verified."** That wording is exactly the
  false confidence the audit was about: an artifact that looks checked, so nobody
  re-checks it.
- **The manifest is generated, never hand-written** —
  `python3 scripts/generate-citation-manifest.py`. It is not in `cargo test` because
  the arXiv rate-limits by IP window: the audit hit HTTP 429 on 19 of 58 requests
  with 3s spacing. Refresh it when adding a citation, and read the diff — a changed
  author list on an unchanged ID is either a correction or a new misattribution.

A new test sensor must assert its own coverage, by the way. `bibliography_table_matches_the_manifest`
asserts `checked > 10` rows: if the table shape changes so the parser matches nothing,
the test would otherwise pass vacuously on an empty set.

## Before Each PR

| Order | Command | Cost | What it catches |
|-------|---------|------|-----------------|
| 3 | `cargo clippy --all-features -- -D warnings` | Medium (1-5min) | Lints, code smells, anti-patterns |
| 4 | `cargo test --all-features` | Medium (1-5min) | Logic errors, regressions |
| 5 | `cargo doc --no-deps 2>&1 \| grep warning` | Medium | Broken doc links, missing docs |
| 6 | Bare `#[allow(dead_code)]` check (see below) | Instant | Unjustified dead code silencing |

## Weekly (Automated via Hermes Cronjob)

| Frequency | Command | What it catches |
|-----------|---------|-----------------|
| Weekly | `cargo +nightly udeps` | Unused dependencies |
| Weekly | `cargo audit` | Known vulnerability advisories |
| Weekly | `rg '#\[allow\(dead_code\)\]' --glob '*.rs' src/ \| grep -v '// ' \| wc -l` | Dead code accumulation |

---

# Bare `#[allow(dead_code)]` Check

Every `#[allow(dead_code)]` **MUST** have a justification comment on the same line.

## Acceptable Justifications

- `// JSON deserialization field — required by serde but unused in app code` — framework requirement
- `// Error enum variant — used by From implementation` — public API completeness
- `// Public API method, cannot gate behind #[cfg(test)]` — rare, for methods that need to exist for trait impls

## Prefer `#[cfg(test)]` Over `#[allow(dead_code)]`

**If a function is only called from tests, gate it with `#[cfg(test)]` instead of `#[allow(dead_code)]`.**

This makes the scope explicit and prevents accidental reliance in production code. The `#[allow(dead_code)]` approach hides the problem; `#[cfg(test)]` solves it.

```rust
// BAD: Silences the warning but keeps dead code in production builds
#[allow(dead_code)] // Test-only helper
fn format_for_test() -> String { ... }

// GOOD: Removes the function entirely from production builds
#[cfg(test)]
fn format_for_test() -> String { ... }
```

## NOT Acceptable

- "Might be useful later" — remove it
- "Preparation for future features" — add when the feature is implemented
- "Reserved for Phase 2" — add when Phase 2 starts
- No comment at all — add justification or remove the dead code

## Enforcement Script

```bash
BARE_ALLOWS=$(rg '#\[allow\(dead_code\)\]' --glob '*.rs' src/ | grep -v '// ' | wc -l)
if [ "$BARE_ALLOWS" -gt 0 ]; then
  echo "FAIL: Found $BARE_ALLOWS bare #[allow(dead_code)] without justification"
  rg '#\[allow\(dead_code\)\]' --glob '*.rs' src/ | grep -v '// '
  exit 1
fi
```

Run this as part of the PR quality gate (order 6 above).

---

# Steering Rule — Bugs and Harness Failure

**Every bug that repeats MUST produce a guide or sensor.** This is the core principle of harness engineering:

- **One-off bugs** are acceptable — they happen, you fix them, move on.
- **Repeated bugs** are a harness failure — if the same type of bug happens twice, the harness (guides + sensors) was insufficient.

When a bug repeats:
1. **Add a computational sensor** that catches it automatically (test, linter rule, script check), OR
2. **Add a feedforward guide** that prevents it (rule in AGENTS.md, clippy configuration, documentation)

**You must do at least one.** Both is better.

### Examples from Sprachspiel Bugs

- **Bug #3** (L2→cosine conversion error) — repeated because no test validated similarity calculations → add a test
- **Bug #4** (facts extracted but never persisted) — repeated because no integration test verified the full pipeline → add a golden file
- **Bug #5** (false positive contradictions) — repeated because no test checked accumulative vs exclusive predicates → add test cases

### Scope

This rule applies to the **development harness** only. The product harness (SOUL.md, skills, facts) has its own feedback loop via the memory system.

---

# External References

These external resources provide additional harness patterns:

- **rust-magic-linter** (vicnaum/rust-magic-linter) — Strict Clippy configs for AI-assisted Rust development. Key lints: `allow_attributes = "deny"` (prevents silencing lints without justification), `unwrap_used = "warn"`, cognitive complexity thresholds. Useful as a reference for incremental adoption.
- **rust-skills** (leonardomso/rust-skills) — 179 Rust rules organized in 14 categories with examples. Feedforward guide for AI coding agents covering ownership, error handling, API design, async, testing, etc. Can be installed as a skill for OpenCode.
- **Harness Engineering for Coding Agent Users** (Martin Fowler, 2025) — Framework that distinguishes feedforward (guides) from feedback (sensors), and computational (deterministic) from inferential (LLM-based).