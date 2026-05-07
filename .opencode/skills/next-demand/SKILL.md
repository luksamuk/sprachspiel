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
5. Cross-referencing priorities and effort estimates
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
2. **M1 Wave items** — follow W1→W2→W3→W4→W5 order (see IMPLEMENTATION.md "M1 Implementation Waves")
3. Items with **no blockers** and **lower effort** are preferred for quick wins
4. Items that **unblock other items** get priority boost (e.g., dependency chain #116→#123)

Exclude from candidates:
- Items already `COMPLETED` in IMPLEMENTATION.md
- Items with `status:blocked` label
- Items in `M2` (TUI) — design-only until M1 complete
- Items in `M3` (Sprach 2.0) — research-only until M1 complete

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

After presenting issue candidates, check if there are board drafts that could be promoted to issues with minimal effort. This prevents drafts from becoming an "idea cemetery" and ensures quick wins get refined.

**Query drafts from the project board:**

```bash
gh api graphql -f query='
query {
  user(login: "luksamuk") {
    projectV2(number: 4) {
      items(first: 100) {
        nodes {
          id
          type
          fieldValues(first: 20) {
            nodes {
              ... on ProjectV2ItemFieldTextValue { text }
              ... on ProjectV2ItemFieldSingleSelectValue { name }
            }
          }
        }
      }
    }
  }
}' | python3 -c "
import json, sys
data = json.load(sys.stdin)
items = data['data']['user']['projectV2']['items']['nodes']
drafts = [i for i in items if i.get('type') == 'DRAFT_ISSUE']
for d in drafts:
    texts = [fv.get('text','') for fv in d.get('fieldValues',{}).get('nodes',[]) if 'text' in fv]
    names = [fv.get('name','') for fv in d.get('fieldValues',{}).get('nodes',[]) if 'name' in fv]
    title = texts[0] if texts else 'Unknown'
    status = names[0] if names else 'Unknown'
    print(f'  DRAFT: {title} (Status: {status})')
"
```

**Classify drafts by refinement level:**

| Refinement Level | Description | Typical Effort | Action |
|------------------|-------------|----------------|--------|
| **Level 1: Ready to promote** | Draft has clear description, no open questions, no code dependencies | ~30min to write issue | Offer to create issue immediately |
| **Level 2: Needs research** | Draft has open questions, needs architecture validation | ~4h to 1 day | Offer to start Phase 0 research (🟡 RESEARCH) |
| **Level 3: Needs design** | Draft is a concept, needs significant design before any implementation | Days | Leave as draft, suggest scheduling a design session |

**Present quick-win drafts to the user:**

After the issue candidates, show:

> **📋 Board Drafts Available for Refinement**
>
> There are N drafts on the board. Some are quick wins that could be promoted to issues in under an hour:
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

1. **Level 1 (Ready):** Create a GitHub issue with the draft's title and body, add it to the project board, move to "Ready" status, update IMPLEMENTATION.md if needed
2. **Level 2 (Needs research):** Follow Phase 0 of the pr-workflow — create issue, mark as `🟡 RESEARCH NEEDED`, investigate, produce Research Summary, then promote to `📋 PLANNED`
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
3. **NEVER close issues before PR merge** — they auto-close with "Closes #N"
4. **NEVER move cards to "Done" manually** — cards move to "Done" automatically when PR merges (via "Closes #N"), verify manually afterward
5. **ALWAYS create PR as DRAFT first** — then implement, then mark ready
6. **ALWAYS read PR-PROCESS.md before starting** — the process has been updated multiple times
7. **ALWAYS present candidates before choosing** — let the user decide
8. **ALWAYS wait for authorization between phases** — no autonomous progression
9. **NEVER merge without approval** — PRs must be reviewed
10. **ALWAYS flag research cards** — mark `🟡 RESEARCH NEEDED` candidates explicitly with open questions
11. **ALWAYS check board drafts** — after presenting issue candidates, present quick-win drafts that could be promoted to issues (Step 3.5). Drafts must not become an idea cemetery.

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

- **GitHub:** `luksamuk/ask-ollama-rs`
- **Project Board:** Number 4 (Ask-AI Roadmap)
- **Priority within milestones:** determined by card order on the board (top = highest priority)
- **Cards referenced by issue number** (e.g., #72, #116) — P-code prefixes retired