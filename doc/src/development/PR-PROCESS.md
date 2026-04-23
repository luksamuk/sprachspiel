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

1. **NEVER close issues before PR merge** - Issues are closed automatically when PR is merged
2. **NEVER move cards to "Done"** - Only the reviewer moves cards to "Done" after approval
3. **NEVER merge without approval** - PRs must be reviewed before merge

### ALWAYS Do These

1. **ALWAYS create PR as DRAFT first** - Then implement, then mark "ready for review"
2. **ALWAYS move card to "In Review"** - After creating PR and before marking ready
3. **ALWAYS update CHANGELOG and IMPLEMENTATION.md** - Before committing code changes
4. **ALWAYS reference the issue in PR body** - Use "Closes #N" or "Related #N"
5. **ALWAYS add new issues to roadmap** - When creating new issues, add them to IMPLEMENTATION.md with priority label

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
| 6 | Testing: manual tests + smoke test | `pr-testing` |
| 7 | Merge (after authorization) | `pr-workflow` |

**Phase 2.6 is NON-NEGOTIABLE.** Without the requirements checkpoint, the agent may implement features that already exist, make wrong assumptions, or cause conflicts. Present the requirements table and get explicit user approval.

## Review & Iteration

After Phase 4, the review loop begins:

```
┌──────────────────────────────────────┐
│         REVIEW ITERATION              │
│  Reviewer comments → Agent responds   │
│  → Implementation if needed → Push    │
│  → Return for re-review               │
└──────────────────────────────────────┘
                   ↓
┌──────────────────────────────────────┐
│         MANUAL TESTING                │
│  Reviewer tests → Bugs? → Fix → Loop │
└──────────────────────────────────────┘
                   ↓
┌──────────────────────────────────────┐
│    MANUAL TEST SCRIPT (Phase 6.1)     │
│  Primary agent creates script         │
│  Hermes Agent executes                │
└──────────────────────────────────────┘
                   ↓
┌──────────────────────────────────────┐
│    SMOKE TEST UPDATE (Phase 6.3)      │
│  Agent reviews SMOKE_TEST.md          │
└──────────────────────────────────────┘
                   ↓
┌──────────────────────────────────────┐
│    SMOKE TEST (Phase 6.4, OPTIONAL)   │
│  Hermes Agent executes SMOKE_TEST.md │
└──────────────────────────────────────┘
                   ↓
               MERGE
```

**For detailed review response format, exact GraphQL commands, and iteration handling, load the `pr-workflow` skill.**

**For manual test script creation, smoke test updates, and result processing, load the `pr-testing` skill.**

## Testing Phases

**Key distinction:**
- **Manual tests** (Phase 6.2) are task-specific, created per PR, NOT versioned
- **Smoke tests** (Phase 6.4) are generalized, versioned in `SMOKE_TEST.md`, ensure minimum guarantees

**Who does what:**
- **Primary agent (OpenCode):** Creates manual test script, reviews SMOKE_TEST.md, processes results
- **Hermes Agent:** Executes both manual tests and smoke tests, reports results
- **User:** Approves test scripts, requests smoke tests, reviews results

**Load the `pr-testing` skill for complete instructions.**

## Review Comment Response Prefixes

When responding to review comments, use these prefixes:

| Prefix | Meaning | When to Use |
|--------|---------|-------------|
| ✅ Resolvido | Code fixed/removed | Changed code to address the comment |
| ✅ Verificado | Code is correct as-is | Confirmed the code behavior is intentional |
| 📋 | Acknowledged, deferred | Good suggestion, will address in future PR |
| ❌ | Declined | Suggestion not applicable, with explanation |
| ❓ | Clarification needed | Question about the comment |

**CRITICAL:** Respond to EACH thread individually, not in a single summary comment. Each comment needs its own reply for the reviewer to mark as resolved.

## Iteration Scenarios

During review, several scenarios may occur:

- **Implementation changes needed:** Create todo list → user confirms → implement → push → re-review
- **Scope creep detected:** Discuss with user → may open separate issues
- **YAGNI identified:** Explain why code should be removed → remove with agreement
- **Large issue detected:** Discuss splitting into multiple PRs
- **Bugs found during testing:** Document → fix → push → return to review iteration

**Every push triggers a new review cycle.** The reviewer must review new commits before proceeding.

## Multiple Issues per PR

When addressing multiple related issues in a single PR:

1. **Both issues must be related** — don't combine unrelated work
2. **PR title describes both** — e.g., `feat: memory staleness warnings and truncation notices`
3. **PR body references all issues** — use `Closes #A, Closes #B` for auto-close
4. **Both cards follow the same flow** — both move to "In Progress" at start, both to "In Review" at Phase 4

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

- **Project Name**: Ask-AI Roadmap
- **Project URL**: https://github.com/users/luksamuk/projects/4/views/4
- **Project Number**: 4
- **Project ID**: `PVT_kwHOADplIc4BRnZ9`
- **Status Field ID**: `PVTSSF_lAHOADplIc4BRnZ9zg_ZGpg`
- **Scrum Status Field ID**: `PVTSSF_lAHOADplIc4BRnZ9zg_ZHUY`

**Status Options:**
| Name | ID |
|------|-----|
| Todo | `f75ad846` |
| In Progress | `47fc9ee4` |
| In Review | `77520bb7` |
| Done | `98236657` |

**Scrum Status Options:**
| Name | ID |
|------|-----|
| Backlog | `94ed2e0f` |
| Ready | `70e88e2e` |
| In Progress | `c2eae8ae` |
| In Review | `d242b7c7` |
| Done | `a456e7a8` |