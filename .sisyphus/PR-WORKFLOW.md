# Prometheus + Sisyphus + PR-PROCESS: Consolidated Workflow

> **Permanent reference** for how Prometheus (planning), Sisyphus (execution), and PR-PROCESS.md
> (project review rules) work together. This document is consulted by every plan generated
> under `.sisyphus/plans/`.

---

## Overview

Three systems collaborate on every feature:

| System | Role | When Active | Output |
|--------|------|-------------|--------|
| **Prometheus** | Strategic planner | Before code | `.sisyphus/plans/{name}.md` |
| **Sisyphus** | Task executor | After plan approved | Code changes, commits, test scripts |
| **PR-PROCESS** | Review guardrails | During PR lifecycle | Branch rules, card moves, review loops |

**Golden rule**: Planning (Prometheus) → Execution (Sisyphus) → Review (PR-PROCESS) → Manual QA (Hermes) → Merge (User).

---

## Phase-by-Phase Mapping

### Step 1: Prometheus Phase (Planning)

**Who**: Prometheus (OpenCode planner agent)
**When**: User requests a feature or identifies next demand from roadmap

1. **Interview** user to gather requirements
2. **Research** via explore/librarian agents (codebase patterns, external docs)
3. **Consult Metis** for gap analysis (mandatory before plan generation)
4. **Generate work plan** to `.sisyphus/plans/{name}.md`
5. **Submit to Momus** for high-accuracy review (if requested)
6. **Hand off** — tell user to run `/start-work`

**Key outputs**:
- A complete `.sisyphus/plans/{name}.md` with TODOs, waves, dependencies, QA scenarios
- Draft file at `.sisyphus/drafts/{name}.md` (deleted after plan is approved)

**What the plan MUST include** (for PR-PROCESS compatibility):
- A "PR Process Integration" section mapping PR-PROCESS phases → plan tasks
- Explicit execution boundary: where Sisyphus stops and Hermes takes over
- Branch name convention: `<type>/<description>`
- Issue number reference (for `Closes #N` in PR body)

### Step 2: Sisyphus Phase — Part A (Pre-Implementation Setup)

**Who**: Sisyphus (OpenCode build agent via `/start-work`)
**When**: After user runs `/start-work`

This corresponds to **PR-PROCESS Phase 1 + Phase 2**:

```
1. Create branch: git checkout -b <type>/<description>
2. Move GitHub Project card to "In Progress"
3. Update CHANGELOG.md with feature entry
4. Update IMPLEMENTATION.md: mark task as 🔄 IN PROGRESS
5. Commit docs: git commit -m "docs: update CHANGELOG for <feature>"
6. Push branch: git push -u origin <branch>
7. STOP → report to user for Draft PR creation
```

**CRITICAL STOP POINT**: After step 6, Sisyphus MUST stop and wait for user to:
- Create the Draft PR (`gh pr create --draft`)
- Authorize continuation to implementation

**Why this stop exists** (per PR-PROCESS):
- Allows user to review planned changes before code is written
- Provides opportunity for architecture discussion
- Prevents wasted effort on wrong approach

### Step 3: Sisyphus Phase — Part B (Implementation)

**Who**: Sisyphus
**When**: After user authorizes continuation

This corresponds to **PR-PROCESS Phase 3**:

```
1. Execute tasks from the plan in wave order (max parallelism)
2. Each task: implement → cargo test → cargo clippy → commit
3. After all implementation tasks complete:
   a. cargo build --features all-tools
   b. cargo clippy --all-features -- -D warnings
   c. cargo test --all-features
4. Push all commits
```

**What Sisyphus does NOT do**:
- Sisyphus does NOT skip the docs-first commit (CHANGELOG must be committed before code)
- Sisyphus does NOT merge the PR
- Sisyphus does NOT close the issue
- Sisyphus does NOT move cards to Done

### Step 4: PR Review Phase

**Who**: User (reviewer) + Sisyphus (responds to comments)
**When**: After implementation is pushed

This corresponds to **PR-PROCESS Phase 4 + Phase 5**:

```
1. Sisyphus marks PR "ready for review": gh pr ready <number>
2. Sisyphus moves card to "In Review"
3. User reviews PR, adds comments
4. Sisyphus responds to EACH comment individually:
   - ✅ Resolvido (fixed)
   - ✅ Verificado (confirmed correct)
   - 📋 Acknowledged, deferred
   - ❌ Declined (with explanation)
   - ❓ Clarification needed
5. If implementation changes needed → fix → push → return to step 3
6. User approves when all comments resolved
```

### Step 5: Manual Test Script (Phase 6.1)

**Who**: Sisyphus (creates) → User (approves)
**When**: After PR review is approved

```
1. Sisyphus creates ~/MANUAL-TEST-PR_NUMBER.md
2. Based on template: doc/src/development/MANUAL-TEST-TEMPLATE.md
3. Customized for the specific feature/fix
4. User reviews and approves the script
```

**🔴 SISYPHUS STOPS HERE. HERMES TAKES OVER.**

### Step 6: Hermes Phase (Manual QA)

**Who**: Hermes Agent
**When**: After test script is approved

This corresponds to **PR-PROCESS Phase 6.2 + 6.3 + 6.4**:

```
1. Hermes executes manual test script
2. Hermes reports results in PR comments
3. If bugs found:
   a. Document in PR comments
   b. Return to Sisyphus for fixes (Step 4)
   c. Fix → push → re-review → re-test
4. If tests pass → Hermes reports "Aprovado para merge"
5. Hermes reviews SMOKE_TEST.md, adds sections if needed
6. Hermes executes smoke test (if requested)
7. Report final verdict
```

### Step 7: Merge

**Who**: User (authorizes) → Sisyphus or Hermes (executes)
**When**: After all tests pass and user approves

```
1. User authorizes merge
2. Agent runs: gh pr merge N --merge --delete-branch
3. Card moves to Done automatically
4. Issue closes automatically (via "Closes #N")
```

---

## Responsibility Matrix

| Action | Prometheus | Sisyphus | Hermes | User |
|--------|-----------|----------|--------|------|
| Interview requirements | ✅ | — | — | Answers |
| Research codebase | ✅ | — | — | — |
| Write plan | ✅ | — | — | Approves |
| Create branch | — | ✅ | — | — |
| Update CHANGELOG | — | ✅ | — | — |
| Create Draft PR | — | ✅ | — | Authorizes |
| Implement code | — | ✅ | — | Authorizes |
| Respond to review | — | ✅ | — | Reviews |
| Create manual test script | — | ✅ | — | Approves |
| Execute manual tests | — | — | ✅ | — |
| Execute smoke test | — | — | ✅ | Requests |
| Merge PR | — | — | — | ✅ |
| Close issue | — | — | — | Auto (via Closes #N) |
| Move card to Done | — | — | — | Auto (on merge) |

---

## Plan Template: PR Process Integration Section

Every plan under `.sisyphus/plans/` MUST include this section:

```markdown
## PR Process Integration

### Phase Mapping

| PR-PROCESS Phase | Plan Task(s) | Executor | Notes |
|-----------------|-------------|----------|-------|
| Phase 1: Setup | Pre-task | Sisyphus | Branch + card move |
| Phase 2: Docs FIRST | Task N (partial) | Sisyphus | CHANGELOG before code |
| Phase 2 STOP | — | User | Draft PR, wait for auth |
| Phase 2.5: Planning | ✅ DONE | Prometheus | This plan |
| Phase 2.6: Requirements | ✅ DONE | Prometheus | Metis + Momus |
| Phase 3: Implementation | Tasks 1-N | Sisyphus | After authorization |
| Phase 4: PR Ready | Post-impl | Sisyphus | gh pr ready |
| Phase 5: Review | — | User | Comments + iteration |
| Phase 6.1: Test Script | Final (partial) | Sisyphus | STOP HERE |
| Phase 6.2+: Manual QA | — | Hermes | Hermes executes |

### Key Constraints
1. NEVER close issues before PR merge
2. NEVER move cards to Done (auto on merge)
3. NEVER merge without user approval
4. ALWAYS create PR as DRAFT first
5. ALWAYS commit docs before code
6. STOP at Phase 6.1 → Hermes takes over
```

---

## Quick Reference: PR-PROCESS Constants

| Constant | Value |
|----------|-------|
| Project Number | 4 |
| Project ID | `PVT_kwHOADplIc4BRnZ9` |
| Status Field ID | `PVTSSF_lAHOADplIc4BRnZ9zg_ZGpg` |
| Scrum Status Field ID | `PVTSSF_lAHOADplIc4BRnZ9zg_ZHUY` |
| Status: In Progress | `47fc9ee4` |
| Status: In Review | `77520bb7` |
| Scrum: In Progress | `c2eae8ae` |
| Scrum: In Review | `d242b7c7` |

---

## Failure Modes and Recovery

| What Goes Wrong | Detection | Recovery |
|----------------|-----------|----------|
| Sisyphus skips Draft PR stop | User notices code commits before PR | Stop execution, create PR retroactively |
| Hermes finds bugs in manual test | PR comment with failure details | Return to Step 4 (Sisyphus fixes → re-review) |
| Scope creep during implementation | Scope fidelity check (F4) catches it | Discuss with user, may split into new issue |
| Sisyphus goes past Phase 6.1 | User notices test execution attempt | Recall, force stop, hand off properly to Hermes |
| Plan and implementation diverge | Plan compliance audit (F1) catches it | Re-read plan, fix deviations, re-verify |

---

## Changelog

- **2026-04-17**: Initial creation — consolidated from PR-PROCESS.md + Prometheus workflow