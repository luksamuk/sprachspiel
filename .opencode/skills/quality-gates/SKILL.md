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

- `// Reserved for Phase 2: TUI commands` — planned feature with clear scope
- `// JSON deserialization field — required by serde but unused in app code` — framework requirement
- `// Error enum variant — used by From implementation` — public API completeness
- `// Test-only code, guarded by #[cfg(test)]` — test infrastructure

## NOT Acceptable

- "Might be useful later" — remove it
- "Preparation for future features" — add when the feature is implemented
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
- **Harness Engineering for Coding Agent Users** (Martin Fowler, 2025) — Framework that distinguishes feedforward (guides) from feedback (sensors), and computational (deterministic) from inferential (LLM-based). The sprachspiel analysis is in `~/harness-engineering-sprachspiel-analysis.md`.