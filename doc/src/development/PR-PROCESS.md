# Pull Request Process

This document describes the mandatory workflow for implementing features and fixes.

## ⚠️ CRITICAL: FOLLOW STEPS IN ORDER

**DO NOT skip steps. DO NOT jump ahead. Each step must be completed before the next.**

- ❌ DO NOT start implementing before Phase 2 is complete
- ❌ DO NOT skip Phase 2.6 (Requirements Checkpoint) — it is NON-NEGOTIABLE
- ❌ DO NOT create PR before Phase 3 is complete
- ❌ DO NOT mark PR "ready for review" before Phase 4 is complete
- ✅ DO read this document BEFORE starting ANY implementation
- ✅ DO present the requirements table (Phase 2.6) before writing ANY code

## ⚠️ CRITICAL RULES

### NEVER Do These

1. **NEVER close issues before PR merge** — Issues are closed automatically when PR is merged via "Closes #N"
2. **NEVER move cards to "Done"** — Cards move to "Done" automatically when PR merges (via "Closes #N"), verify manually afterward
3. **NEVER merge without approval** — PRs must be reviewed before merge

### ALWAYS Do These

1. **ALWAYS create PR as DRAFT first** — Then implement, then mark "ready for review"
2. **ALWAYS move card to "In Review"** — After creating PR and before marking ready
3. **ALWAYS update CHANGELOG and IMPLEMENTATION.md** — Before committing code changes
4. **ALWAYS reference the issue in PR body** — Use "Closes #N" or "Related #N"
5. **ALWAYS add new issues to roadmap** — When creating new issues, add them to IMPLEMENTATION.md with priority label

## Workflow Summary

The PR workflow consists of 7 phases. **For the complete step-by-step instructions, load the appropriate skill.**

| Phase | Description | Skill to Load |
|-------|-------------|---------------|
| 1 | Setup: branch, card to "In Progress" | `pr-workflow` |
| 2 | Documentation FIRST: CHANGELOG, IMPLEMENTATION.md | `pr-workflow` |
| ⛔ | **STOP**: Create draft PR, wait for authorization | `pr-workflow` |
| 2.5 | Planning mode (read-only analysis, plan, approval) | `pr-workflow` |
| 2.6 | Requirements checkpoint (NON-NEGOTIABLE) | `pr-workflow` |
| 3 | Implementation: code, tests, linters, commit, push | `pr-workflow` |
| 4 | Mark PR ready, move card to "In Review" | `pr-workflow` |
| 5 | Review & iteration (respond to each thread) | `pr-workflow` |
| 6 | Testing: manual tests + smoke test | `pr-testing` + `manual-test-verification` |
| 7 | Merge (after authorization) | `pr-workflow` |

**Phase 2.6 is NON-NEGOTIABLE.** Without the requirements checkpoint, the agent may implement features that already exist, make wrong assumptions, or cause conflicts. Present the requirements table and get explicit user approval.

## Testing Phases

**Key distinction:**
- **Manual tests** (Phase 6.2) are task-specific, created per PR, NOT versioned
- **Smoke tests** (Phase 6.4) are generalized, versioned in `SMOKE_TEST.md`, ensure minimum guarantees

**Who does what:**
- **Primary agent (OpenCode):** Creates manual test script, verifies it against source code (see `manual-test-verification` skill), reviews SMOKE_TEST.md, processes results
- **Hermes Agent:** Executes both manual tests and smoke tests, reports results
- **User:** Approves test scripts, requests smoke tests, reviews results

**Load the `pr-testing` skill for complete instructions.** After drafting the manual test script, load `manual-test-verification` to validate all commands, UI strings, and feature references against the source code.

## Quality Gates

Before each commit and each PR, run quality gate sensors in order of cost. **Load the `quality-gates` skill for the complete sensor hierarchy and enforcement scripts.**

Minimum checks:
- Before each commit: `cargo fmt --check`, `cargo check --all-features`
- Before each PR: `cargo clippy --all-features -- -D warnings`, `cargo test --all-features`, bare `#[allow(dead_code)]` check

## Review Comment Response Prefixes

**CRITICAL:** Respond to EACH thread individually, not in a single summary comment. Each comment needs its own reply for the reviewer to mark as resolved.

**For detailed review patterns and API commands (creating reviews, responding to threads, resolving threads, project-specific checks), load the `code-review` skill.**

| Prefix | Meaning | When to Use |
|--------|---------|-------------|
| ✅ Resolvido | Code fixed/removed | Changed code to address the comment |
| ✅ Verificado | Correct as-is | Confirmed the code behavior is intentional |
| 📋 | Acknowledged, deferred | Good suggestion, will address in future PR |
| ❌ | Declined | Not applicable, with explanation |
| ❓ | Clarification needed | Question about the comment |

## Conventional Commits

Format: `<type>: <description>`

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `refactor` | Code refactoring |
| `docs` | Documentation only |
| `test` | Adding/updating tests |
| `chore` | Maintenance tasks |

## Example PR Body

```markdown
## Summary

Brief description of changes.

## Changes

| File | Change |
|------|--------|
| `src/foo.rs` | Added X |
| `doc/src/CHANGELOG.md` | Added entry |

## Testing

- [ ] `cargo build --all-features`
- [ ] `cargo clippy --all-features -- -D warnings`
- [ ] `cargo test --all-features`

## Related

Closes #N  (use only when PR will completely close the issue)
Related #N (use when PR is related but not closing)
```

## Project Information

- **Project Name**: Sprachspiel Roadmap
- **Project URL**: https://github.com/users/luksamuk/projects/4/views/4
- **Project Number**: 4
- **Project ID**: `PVT_kwHOADplIc4BRnZ9`

**For project board field IDs and status option IDs, load the `pr-workflow` skill.**