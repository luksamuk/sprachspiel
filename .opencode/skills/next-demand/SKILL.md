---
name: next-demand
description: Identify the next demand/feature to implement by consulting the roadmap, open issues, and PR process. Follows the mandatory PR-PROCESS.md workflow from branch creation to draft PR.
license: MIT
compatibility: opencode
metadata:
  audience: maintainers
  workflow: pr-process
---

## What I do

I identify the next implementation demand for the ask-ai project by:
1. Reading `IMPLEMENTATION.md` to understand completed and planned work
2. Reading `doc/src/development/roadmap.md` for strategic context
3. Checking open GitHub issues via `gh issue list` and `gh issue view`
4. Checking the GitHub Project board status
5. Cross-referencing priorities (P0-P15) and effort estimates
6. Presenting candidates with effort, dependencies, and rationale
7. After user selection, initiating the PR-PROCESS.md workflow

## When to use me

Use this skill when the user asks "What's the next demand?", "Qual a próxima demanda?", "What should we work on next?", or similar questions about prioritizing work.

## Mandatory Documents to Read

**ALWAYS read these documents in order before doing anything:**

1. **`AGENTS.md`** — Project guidelines, code style, tool development rules
2. **`doc/src/development/PR-PROCESS.md`** — The mandatory PR workflow. READ THIS COMPLETELY before starting any implementation.
3. **`IMPLEMENTATION.md`** — Current status of all priorities, completed work, what's planned
4. **`doc/src/development/roadmap.md`** — Strategic direction, milestones, future plans

## Step-by-Step Process

### Step 1: Gather Information (READ-ONLY)

Read the four mandatory documents above, then:

```bash
# List all open issues
gh issue list --state open --limit 100

# Check project board (project number 4 = Ask-AI Roadmap)
gh project item-list 4 --owner luksamuk --format json

# Check recent PRs
gh pr list --state open --limit 20
```

For each relevant open issue, read its full description:
```bash
gh issue view <number>
```

### Step 2: Analyze and Prioritize

Create a priority table with these columns:
| # | Title | Issue | Priority | Effort | Blockers | Status |

Priority ordering rules:
1. **Bug fixes** with `priority:critical` or `priority:high` come first
2. **P0-P5** items that are `NOT STARTED` or `PLANNED` 
3. **P6+** items that are `RESEARCH` or `PLANNED`
4. Items with **no blockers** and **lower effort** are preferred for quick wins
5. Items that **unblock other items** get priority boost

Exclude from candidates:
- Items already `COMPLETED` in IMPLEMENTATION.md
- Items with `status:blocked` label
- Items in `M2` (Sprach 2.0) milestone — research-only until M1 complete
- Items in `M3` (Future) — explicitly deferred

### Step 2.5: Duplicate Check (MANDATORY)

Before presenting candidates to the user, verify that each issue is not a duplicate:

```bash
# For each candidate issue, check for duplicates:
gh issue list --state all --limit 100 | grep -i "<keyword from title>"
```

If a duplicate is found:
1. **If the original is CLOSED** — check whether the PR that closed it fully addressed the issue. If yes, skip this candidate. If the PR only partially addressed it, note the residual work.
2. **If the original is OPEN** — present only the canonical issue, not the duplicate.
3. **Close duplicate issues** — leave a comment explaining the duplication and referencing the canonical issue.

### Step 3: Present Options to User

Present the top 3-5 candidates with:
- **Title** and **Issue number**
- **Priority** (P0-P15)
- **Estimated effort** (days/weeks)
- **Dependencies/blockers**
- **Why it's a good candidate** (no blockers, quick win, high value, etc.)
- **Brief implementation outline** (files to create/modify, approach)

Then **WAIT for user selection**. Do NOT proceed without explicit choice.

### Step 4: Initiate PR Process (AFTER user selection)

Once the user picks a demand, **load the `pr-workflow` skill** to continue from Phase 1 (Setup) through the complete PR workflow.

The pr-workflow skill covers: branch creation, documentation, draft PR, planning mode, requirements checkpoint, implementation, review, testing, and merge.

**Do NOT duplicate the workflow steps here.** The pr-workflow skill is the single source of truth for all PR process steps.

## Key Rules

1. **NEVER skip the PR-PROCESS.md steps** — follow them in order
2. **NEVER close issues before PR merge** — they auto-close with "Closes #N"
3. **NEVER move cards to "Done" manually** — cards move to "Done" automatically when PR merges (via "Closes #N"), verify manually afterward
4. **ALWAYS create PR as DRAFT first** — then implement, then mark ready
5. **ALWAYS read PR-PROCESS.md before starting** — the process has been updated multiple times
6. **ALWAYS present candidates before choosing** — let the user decide
7. **ALWAYS wait for authorization between phases** — no autonomous progression
8. **NEVER merge without approval** — PRs must be reviewed

## Priority Labels Reference

| Label | Meaning |
|-------|---------|
| `priority:critical` | Must fix now (bugs, security) |
| `priority:high` | Important, next sprint |
| `priority:medium` | Nice to have, planned |
| `priority:low` | Backlog, future |
| `status:planned` | Accepted, not started |
| `status:in-progress` | Currently being worked on |
| `status:blocked` | Cannot proceed until blocker resolved |

## Milestone Mapping

| Milestone | Codename | Description | Priorities |
|-----------|----------|-------------|------------|
| M1 | Core Evolution | All work before Sprach 2.0 | P0-P6, P8-P13 |
| M2 | Sprach 2.0 | CAS research, cognitive extensions | P7, P14, P15 |
| M3 | Future | Deferred, no current priority | Cost tracking, TUI, plugins |

## Project Info

- **GitHub:** `luksamuk/ask-ollama-rs`
- **Project Board:** Number 4 (Ask-AI Roadmap)

For project board field IDs and status option IDs, load the `pr-workflow` skill.