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

I identify the next implementation demand for the sprachspiel project by:
1. Reading `IMPLEMENTATION.md` to understand completed and planned work
2. Reading `doc/src/development/roadmap.md` for strategic context
3. Querying open demands from **Linear** (project "Sprachspiel") — issues migrated from GitHub on 2026-08-19
4. Cross-referencing priorities, milestones (M1-M4), and dependencies (Linear issue relations)
5. Presenting candidates with effort, dependencies, and rationale
6. After user selection, initiating the PR-PROCESS.md workflow

## Issue Source of Truth: Linear

GitHub issues are **closed history** (migrated to Linear on 2026-08-19). PRs and reviews stay on GitHub; *demands* are queried from Linear.

**Transport: MCP-first, HTTP-fallback.** If the Linear MCP tools are available (`mcp__linear__list_issues` etc.), use them — OAuth, no key management. Otherwise load the `linear` skill (Productivity category) for the GraphQL HTTP path (declares `env_vars: [LINEAR_API_KEY]`; Hermes injects it from the profile `.env`; never read the key via shell).

- **Discover, don't hardcode:** project/label/milestone ids are resolved at runtime by name (`mcp__linear__list_projects` / `projects(filter: { name: { eq: "Sprachspiel" } })`). MCP `list_issues` already returns `project`/`projectMilestone` per issue, so filtering is usually client-side.
- Every Linear issue migrated from GitHub has `Ref: gh#N` in its description; cite demands as `LUC-N (ex gh#N)`.
- `list_issues` returns `gitBranchName` per issue — use it for branch naming (`luc-NNN-slug`); the Linear GitHub integration auto-links those branches/PRs to the issue and moves status on PR open/merge. Magic words (`Fixes LUC-141`) in commit messages/PR bodies also work.
- Linear priority: 0=None, 1=Urgent, 2=High, 3=Medium, 4=Low.

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

Read the four mandatory documents above, then query Linear (MCP-first, else the `linear` skill's GraphQL fallback):

**MCP:**
```
mcp__linear__list_projects            → find "Sprachspiel"
mcp__linear__list_issues              → keep: project == "Sprachspiel",
                                        statusType not in {completed, canceled, duplicate}
mcp__linear__get_issue LUC-N          → full description per candidate
```

**HTTP fallback (GraphQL):**
```graphql
query { issues(
  filter: {
    project: { name: { eq: "Sprachspiel" } },
    state: { type: { nin: ["completed", "canceled", "duplicate"] } }
  }, first: 250) {
  nodes { identifier title priority state { name type } labels { nodes { name } } description url }
} }
```

GitHub remains for PRs:
```bash
gh pr list --state open --limit 20
```

### Step 2: Analyze and Prioritize

Create a priority table with these columns:
| # | Title | Issue | Priority | Effort | Blockers | Status |

Priority ordering rules:
1. **Bug fixes** with priority 1 (Urgent) or 2 (High) come first — MCP returns `priority {value, name}`; use `value`. (Old GitHub equivalences: `priority:critical`→1, `priority:high`→2)
2. **M1 Wave items** — follow W1→W2→W3→W4→W5 order (see IMPLEMENTATION.md "M1 Implementation Waves"); milestone is the issue's `projectMilestone`
3. Items with **no blockers** and **lower effort** are preferred for quick wins
4. Items that **unblock other items** get priority boost — check `relations` / `inverseRelations` (e.g., embedding chain LUC-92→LUC-93→…→LUC-96)

Exclude from candidates:
- Items already `COMPLETED` in IMPLEMENTATION.md
- Items with **unresolved `blocked_by` relations** — surface the blocking chain in the table instead of listing them as actionable
- Items in `M2` milestone (TUI) — design-only until M1 complete
- Items in `M3` milestone (Sprach 2.0) — research-only until M1 complete

### Step 2.5: Duplicate Check (MANDATORY)

Before presenting candidates to the user, verify that each issue is not a duplicate — search Linear by title keyword (`mcp__linear__list_issues` with `query`, or GraphQL `issueSearch`). For archaeology against pre-migration work, the closed GitHub issues remain queryable: `gh issue list --state closed | grep -i "<keyword>"`.

If a duplicate is found:
1. **If the original is CLOSED** — check whether the PR that closed it fully addressed the issue. If yes, skip this candidate. If the PR only partially addressed it, note the residual work.
2. **If the original is OPEN** — present only the canonical issue, not the duplicate.
3. **Close duplicate issues** — leave a comment explaining the duplication and referencing the canonical issue.

### Step 3: Present Options to User

Present the top 3-5 candidates with:
- **Title** and **Issue number** (card #)
- **Status** (`🟡 RESEARCH NEEDED` or `📋 PLANNED`/`📋 READY`)
- **M1 Wave** (W1-W5)
- **Estimated effort** (days/weeks)
- **Dependencies/blockers**
- **Open questions** (if 🟡 RESEARCH — list key unresolved questions)
- **Why it's a good candidate** (no blockers, quick win, high value, etc.)
- **Brief implementation outline** (files to create/modify, approach)

**Flag research cards explicitly.** When presenting a `🟡 RESEARCH NEEDED` candidate, note that Phase 0 (Research) will be required before implementation, and estimate the research effort separately from implementation effort.

Then **WAIT for user selection**. Do NOT proceed without explicit choice.

### Step 3.5: Draft Pipeline Check (MANDATORY)

After presenting issue candidates, check for **drafts** — issues sitting in Linear `Backlog` with no milestone and no priority — that could be promoted with minimal effort. This prevents Backlog from becoming an "idea cemetery" and ensures quick wins get refined.

**MCP:** `mcp__linear__list_issues` → filter `statusType == "backlog"`, `priority.value == 0`, no `projectMilestone` in project Sprachspiel.
**HTTP fallback:** same via `issues(filter: { project: { name: { eq: "Sprachspiel" } }, state: { type: { eq: "backlog" } } }, first: 250)`.

**Classify drafts by refinement level:**

| Refinement Level | Description | Typical Effort | Action |
|------------------|-------------|----------------|--------|
| **Level 1: Ready to promote** | Draft has clear description, no open questions, no code dependencies | ~30min to write issue | Offer to create issue immediately |
| **Level 2: Needs research** | Draft has open questions, needs architecture validation | ~4h to 1 day | Offer to start Phase 0 research (🟡 RESEARCH) |
| **Level 3: Needs design** | Draft is a concept, needs significant design before any implementation | Days | Leave as draft, suggest scheduling a design session |

**Present quick-win drafts to the user:**

After the issue candidates, show:

> **📋 Backlog Drafts Available for Refinement**
>
> There are N unrefined items in Linear Backlog (no priority, no milestone). Some are quick wins that could be promoted in under an hour:
>
> | Draft | Milestone | Refinement Level | Why promote now? |
> |-------|-----------|-----------------|-------------------|
> | [title] | [M3/M4] | Level 1 (30min) | [reason: no blockers, clear scope, prerequisite for X] |
> | [title] | [M3/M4] | Level 2 (4h-1d) | [reason: needs architecture validation, blocks Y] |
> | ... | ... | ... | ... |
>
> Would you like to refine any of these drafts into issues? If so, specify which one(s) and the refinement level.

**Draft → Issue promotion process:**

When the user selects a draft to promote:

1. **Level 1 (Ready):** Promote the Linear issue — set milestone (`save_issue`), priority, and move from Backlog to a planned state; update IMPLEMENTATION.md if needed
2. **Level 2 (Needs research):** Follow Phase 0 of the pr-workflow — mark the issue `🟡 RESEARCH NEEDED` (comment), investigate, produce Research Summary, then promote to planned
3. **Level 3 (Needs design):** Schedule a design discussion — do NOT create an issue yet

**Important:** Do NOT promote drafts without explicit user authorization. Present the options and WAIT.

### Step 4: Initiate PR Process (AFTER user selection)

Once the user picks a demand, determine the card's status:

**If the card is `🟡 RESEARCH NEEDED`:**
→ **Load the `pr-workflow` skill and start at Phase 0 (Research).**
Phase 0 is MANDATORY for research cards — it answers open questions before any branch is created. The pr-workflow skill covers the complete Phase 0 process: identify questions, investigate, produce Research Summary, update documentation, gate approval.

**If the card is `📋 PLANNED` or `📋 READY`:**
→ **Load the `pr-workflow` skill and start at Phase 1 (Setup).**
The card's open questions are already answered; proceed directly to branch creation.

**Do NOT determine this yourself.** Always check the card's status in IMPLEMENTATION.md before selecting the starting phase.

## Key Rules

1. **NEVER skip the PR-PROCESS.md steps** — follow them in order
2. **NEVER skip Phase 0** — if a card is `🟡 RESEARCH NEEDED`, research MUST complete before Phase 1
3. **NEVER close issues before PR merge** — they auto-close via the Linear GitHub integration when the PR with `Fixes LUC-N` (magic word) merges; GitHub-side "Closes #N" no longer applies to new work
4. **NEVER move issues to "Done" manually** — the Linear GitHub integration moves them when the PR merges (verify afterward)
5. **ALWAYS create PR as DRAFT first** — then implement, then mark ready
6. **ALWAYS read PR-PROCESS.md before starting** — the process has been updated multiple times
7. **ALWAYS present candidates before choosing** — let the user decide
8. **ALWAYS wait for authorization between phases** — no autonomous progression
9. **NEVER merge without approval** — PRs must be reviewed
10. **ALWAYS flag research cards** — mark `🟡 RESEARCH NEEDED` candidates explicitly with open questions
11. **ALWAYS check board drafts** — after presenting issue candidates, present quick-win drafts that could be promoted to issues (Step 3.5). Drafts must not become an idea cemetery.

## Priority Reference

Linear native `priority` int (MCP: `priority.value`):

| Value | Name | Old GH label | Meaning |
|-------|------|--------------|---------|
| 1 | Urgent | `priority:critical` | Must fix now (bugs, security) |
| 2 | High | `priority:high` | Important, next sprint |
| 3 | Medium | `priority:medium` | Nice to have, planned |
| 4 | Low | `priority:low` | Backlog, future |
| 0 | No priority | — | Unrefined (treat as draft material) |

Old GH `status:*` labels are retired. Status = Linear workflow state (Backlog/Todo/In Progress/In Review/Done). Blocking = Linear issue relations (`blocks`/`blocked_by`).

## Milestone Mapping

| Milestone | Codename | Description | Waves/Cards |
|-----------|----------|-------------|-------------|
| M1 | Core Evolution | All work before TUI and Sprach 2.0 | W1 (Quick Wins: #105, #36) → W2 (Provider Chain: #116-#123, #72) → W3 (Feedback: #90-#97) → W4 (Embedding: #106, #107) → W5 (Backlog: #13, #14, #49, #50, #52, #74-#76) |
| M2 | UX & Pre-Launch | TUI design + implementation, benchmarks, learned patterns | #16, #117, #124, #125 |
| M3 | Sprach 2.0 | CAS research, cognitive extensions, plugin system | #15, #77-#80, #99-#101 + Privacy Filter, ADR: Empathy, meta_cognize, Behavioral Conflict |
| M4 | Future | Deferred features and research | B2-B5, B8 + Attention Priming, Semantic Chunking, Metadata Enrichment, Semantic Dedup, HyDE, Behavioral Embeddings, Behavioral RRF |

## Draft Refinement Guide

When assessing drafts for promotion, use these quick-win criteria:

| Draft | Milestone | Refinement Level | Quick Win? | Why |
|-------|-----------|-----------------|------------|-----|
| ADR: Empathy ≠ Failure | M3 | Level 1 (30min) | ✅ Yes | Zero code, prerequisite for #99/#100/#101 |
| Attention Priming | M4 | Level 1 (30min) | ✅ Yes | ~1 day implementation, zero dependencies, zero architecture change |
| Privacy Filter | M3 | Level 2 (4h-1d) | ⚠️ Half-day | Has PoC, but open questions on lifecycle and caching |
| Context-Aware Chunking | M4 | Level 2 (4h-1d) | ⚠️ Half-day | Needs migration strategy for existing chunks |
| Semantic Dedup | M4 | Level 1 (30min) | ✅ Yes | Offline batch job, well-scoped, no hot-path interaction |
| Metadata Enrichment | M4 | Level 2 (4h-1d) | ❌ No | Requires schema v13 migration, complex |
| HyDE / Q&A Pairing | M4 | Level 2 (4h-1d) | ❌ No | Depends on #106 (embedding model config) |
| Behavioral Embeddings | M4 | Level 3 (design) | ❌ No | Premature — needs Layer 2 data first |
| Behavioral RRF | M4 | Level 3 (design) | ❌ No | Depends on #100 and #101 being stable |
| meta_cognize() Tool | M3 | Level 2 (4h-1d) | ⚠️ Half-day | Depends on #100 for data structure |
| Behavioral Conflict | M3 | Level 2 (4h-1d) | ❌ No | Depends on #77/#78 (relations graph) |

## Project Info

- **Issue tracking:** Linear — project "Sprachspiel", milestones "M1 - Core Evolution" … "M4 - Future & Cultural Grounding"
- **GitHub:** `luksamuk/ask-ollama-rs` (PRs, reviews, CI only; issues are closed history)
- **Old project board #4:** retired (legacy references kept in closed issues' history)
- **Priority within milestones:** Linear `priority` + board order in the Linear triage view