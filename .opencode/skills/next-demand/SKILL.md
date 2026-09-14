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
1. Reading `IMPLEMENTATION.md` (an **index** — it no longer tracks status) for links to the architecture docs
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
- `list_issues` returns `gitBranchName`, but **do not use it verbatim** — it contains a username prefix and preserves shell-hostile characters from the title (literal `=`, etc.). Use the repo's historical pattern `{type}/{LUC-N}-{slug-kebab}` instead, where `type` matches the conventional-commit type (`feat`/`fix`/`docs`/`refactor`/`test`/`chore`) and the slug is a short kebab-case paraphrase of the title (3-5 words max). Examples: `feat/151-thinking-preserve`, `docs/103-adr-empathy-reframing`, `fix/233-search-scope`. Linear's GitHub integration still auto-links via the magic word `Fixes LUC-N` in the PR body or commit message (auto-moves status on open, auto-closes on merge) — the branch name is cosmetic.
- Linear priority: 0=None, 1=Urgent, 2=High, 3=Medium, 4=Low.

## When to use me

Use this skill when the user asks "What's the next demand?", "Qual a próxima demanda?", "What should we work on next?", or similar questions about prioritizing work.

## Mandatory Documents to Read

**ALWAYS read these documents in order before doing anything:**

1. **`AGENTS.md`** — Project guidelines, code style, tool development rules
2. **`doc/src/development/PR-PROCESS.md`** — The mandatory PR workflow. READ THIS COMPLETELY before starting any implementation.
3. **`IMPLEMENTATION.md`** — **Index only** (version, links, pointer to the tracker). Per-issue status is NOT here any more — it lives in Linear. Do not expect to find completion state in this file.
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
- Items already `Done` in Linear (`statusType == completed`)
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

1. **Level 1 (Ready):** Promote the Linear issue — set milestone (`save_issue`), priority, and move from Backlog to a planned state. Do **not** add a section to `IMPLEMENTATION.md` (a test enforces it stays an index)
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

**Do NOT determine this yourself.** Check the issue's state and labels in **Linear** before selecting the starting phase — `IMPLEMENTATION.md` no longer carries status.

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

| Milestone | Codename | Theme | Waves / Cards |
|-----------|----------|-------|---------------|
| M1 | Core Evolution | Everything before the TUI and Sprach 2.0 — provider chain, embedding geometry, feedback, thinking traces | W1 Quick Wins → W2 Provider Chain → W3 Feedback Completion → W4 Embedding Geometry → W5 M1 Backlog → W6 Responsive Chat Rebuild → W7 Thinking Trace Pipeline (see `roadmap.md` for what each wave means) |
| M2 | UX & Pre-Launch | TUI design + implementation, benchmarks, learned patterns, diff rendering | LUC-63 (TUI), LUC-86 (interaction modes), LUC-87 (benchmark infrastructure), LUC-88 (learned patterns) |
| M3 | Sprach 2.0 | CAS research, cognitive extensions, plugin system | LUC-62 (plugin system), LUC-69..LUC-72 (S2.1–S2.5), LUC-81..LUC-83 (meta-cognition layers), LUC-115 (behavioral conflict), LUC-146 (meta_cognize), LUC-147 (privacy filter), LUC-103 (ADR Empathy — ✅ done) |
| M4 | Future | Deferred features and research | LUC-116 (attention-based, R-04), LUC-117 (chunking), LUC-118 (metadata), LUC-119 (dedup), LUC-120 (HyDE), LUC-121 (behavioral embeddings), LUC-122 (behavioral RRF), LUC-109 (ACP), LUC-110..LUC-114 |

> **On issue numbers:** waves are described by **theme**, not by `#NNN`. The M1 wave table in
> `roadmap.md` dropped its issue numbers deliberately — citing them here is how this skill and the
> tracker drifted apart before. Query Linear for the current state of any given item; do not treat a
> bare `#N` as a live issue (GitHub issues are closed history since 2026-08-19).

### Never infer a Linear identifier from a GitHub number

`gh#N` and `LUC-N` are **independent counters**. There is no arithmetic relationship:

```
gh#74 → LUC-66     gh#99 → LUC-81     gh#117 → LUC-86     gh#172 → LUC-116
```

The only source is the `Ref: gh#N` line in the issue description — read it per issue, or query
Linear and build the map. Writing `LUC-84..LUC-89` as a range because the source said `#84-#89`
produces identifiers that are plausible and wrong.

Two related traps, both hit in one session:

- **Assuming ordering is positional.** Reading "LUC-86" and "LUC-87" from a list and assigning them
  to the wrong descriptions (86 was *interaction modes*, 87 was *benchmark infrastructure*). Read the
  issue title back before describing what it contains.
- **Trusting a `Ref:`-only snapshot.** Fetching descriptions with a narrow field selection returns
  only the `Ref:` stub, not the title — so content checks silently compare against the wrong text.
  Select `title` explicitly.

**Rule:** every identifier written into a doc must be verified against Linear in the same pass, by
reading back `title` + `milestone` + `state`. If a mapping cannot be established, describe the item
by name and move on — a wrong `LUC-N` is worse than none, because it sends a future reader to an
unrelated issue.

## Draft Refinement Guide

When assessing drafts for promotion, use these quick-win criteria:

| Item | Issue | Milestone | Refinement Level | Quick Win? | Why |
|------|-------|-----------|-----------------|------------|-----|
| Semantic Deduplication Pre-Indexing | LUC-119 | M4 | Level 1 (30min) | ✅ Yes | Offline batch job, well-scoped, no hot-path interaction |
| Context-Aware Chunking (SemanticChunker) | LUC-117 | M4 | Level 2 (4h-1d) | ⚠️ Half-day | Needs migration strategy for existing chunks |
| Privacy Filter (PII Redaction Sidecar) | LUC-147 | M3 | Level 2 (4h-1d) | ⚠️ Half-day | Has PoC, but open questions on lifecycle and caching |
| meta_cognize() Active Behavioral Tool | LUC-146 | M3 | Level 2 (4h-1d) | ⚠️ Half-day | Depends on LUC-82 for data structure |
| Metadata Enrichment (Chunk Authority & Recency) | LUC-118 | M4 | Level 2 (4h-1d) | ❌ No | Requires schema migration, complex |
| Q&A Pairing / HyDE-like Embedding | LUC-120 | M4 | Level 2 (4h-1d) | ❌ No | Depends on the embedding-model config work |
| Behavioral Conflict Detection | LUC-115 | M3 | Level 2 (4h-1d) | ❌ No | Depends on LUC-69/LUC-70 (relations graph) |
| Behavioral Embeddings | LUC-121 | M4 | Level 3 (design) | ❌ No | Premature — needs Layer 2 data first |
| Behavioral RRF | LUC-122 | M4 | Level 3 (design) | ❌ No | Depends on LUC-81..LUC-83 being stable |
| Attention-Based Prompt Optimization (R-04) | LUC-116 | M4 | Level 3 (design) | ❌ No | Blocked on attention-weight access — Ollama exposes none. Its positional-reordering half was **dropped** (LUC-144 / D-12) |
| ADR: Empathy ≠ Failure | LUC-103 | M3 | — | — | ✅ **Done** — shipped, no longer a candidate |

## Project Info

- **Issue tracking:** Linear — project "Sprachspiel", milestones "M1 - Core Evolution" … "M4 - Future & Cultural Grounding"
- **GitHub:** `luksamuk/sprachspiel` (PRs, reviews, CI only; issues are closed history)
- **Old project board #4:** retired (legacy references kept in closed issues' history)
- **Priority within milestones:** Linear `priority` + board order in the Linear triage view