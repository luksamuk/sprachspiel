---
name: triage
description: Triage new demands, issues, and research into the sprachspiel roadmap. Covers milestone assignment, board ordering, issue formalization, and research integration. Use when creating issues, organizing the board, processing research findings, or answering "what did we do?" or "where does this go?"
license: MIT
compatibility: opencode
metadata:
  audience: maintainers
  workflow: triage
---

## What I do

I triage new demands, issues, and research findings into the sprachspiel project roadmap. I ensure every item gets a correct milestone, priority, and position on the board so that execution order is always clear.

## When to use me

Load me when:
- Creating new GitHub issues from research findings or feature proposals
- Reorganizing the project board (milestone assignment, position ordering)
- Processing a batch of research insights and integrating them into the repo
- Answering "where does this feature go?" or "what milestone is this?"
- After completing a research synthesis session (like processing papers or analysis)
- When the user says "triage", "organize", "prioritize", "where does this go?", or "reorder the board"

## Mandatory References

**Read before triaging:**
1. `IMPLEMENTATION.md` — Current status of all priorities, milestones, waves
2. `doc/src/development/roadmap.md` — Strategic direction
3. `doc/src/development/unified-vision.md` — Research synthesis and architectural decisions
4. `doc/src/development/research-icebox.md` — R-XX items and their dependencies
5. `doc/src/development/research/papers-reference.md` — Papers that informed decisions

---

# Triage Process

## Step 1: Classify the Item

Every new item falls into one of these categories:

| Category | Prefix | Milestone | Has code? | Example |
|----------|--------|-----------|-----------|---------|
| **P0-CRITICAL** | `[P0-CRITICAL]` | M1 | Yes | Bug fix, data loss prevention |
| **P0** | `[P0]` | M1 | Yes | Addendum to current wave |
| **P0-HIGH** | `[P0-HIGH]` | M1 | Yes | Dependencies for current work |
| **High** | none | M1 | Yes | Important, next sprint |
| **Medium** | none | M1-M2 | Yes | Planned feature |
| **Low** | none | M3+ | Maybe | Deferred research |
| **Draft** | `[Draft]` | M3-M4 | No | Research, needs investigation |
| **ADR** | `ADR:` | M3 | Doc only | Architectural Decision Record |
| **Benchmark** | `B1.X` | M2 | Test | Benchmark item |
| **Research** | `[Draft]` + `research` label | M3-M4 | No | Research investigation |

### Label Assignment (Linear labels — migrated 2026-08-19; old lowercase GH names for archaeology only)

| Linear Label | When to use | Old GH label |
|-------|-------------|--------------|
| `Enhancement` | New feature or capability | `enhancement` |
| `Research` | Pure investigation with no code deliverable | `research` |
| `Bug` | Fix for existing broken behavior | `bug` |
| `Documentation` | Doc-only change | `documentation` |
| `Feature` / `Improvement` | Fine-grained feature/improvement tagging | — |

**Priority is the Linear `priority` int, not a label:** 1=Urgent (old `priority:critical`), 2=High, 3=Medium, 4=Low, 0=unrefined.

**Status is the Linear workflow state** (Backlog/Todo/In Progress/In Review/Done/Canceled) — old `status:*` labels retired.

**Blocking is a Linear issue relation** (`blocks`/`blocked_by`), never a label.

## Step 2: Assign Milestone

Use the milestone decision tree:

```
Is it needed for M1 (Core Evolution)?
├── Yes → M1 - Core Evolution
│   ├── Is it P0-CRITICAL? → W4.5 (T3, embed diagnostics)
│   ├── Is it P0-HIGH? → Current wave dependency
│   ├── Is it an embedding feature (#133-#140)? → W4.x cluster
│   └── Is it provider migration (#119-#123)? → W5.x cluster
└── No → Is it UX, benchmarks, or TUI?
    ├── Yes → M2 - Sprach 2.0
    │   ├── TUI-related? → Depends on #16
    │   ├── Plugin System? → Depends on #15
    │   └── Benchmark? → B1.X cluster
    └── No → Is it a cognitive extension or meta-cognition?
        ├── Yes → M3 - Sprach 2.0 Extensions
        │   ├── Is it a draft/research item? → M3 (research)
        │   ├── Is it P2 implementation? → M3 (conditional on M1/M2)
        │   └── Is it doc only (ADR)? → M3 (immediate)
        └── No → M4 - Future & Cultural Grounding
            ├── Cultural Grounding? → M4
            ├── Long-term research? → M4 (Draft)
            └── Advanced retrieval? → M4 (Draft)
```

### Milestone Reference

| Milestone | Number | Description | Key Waves/Items |
|-----------|--------|-------------|------------------|
| M1 - Core Evolution | #1 | All work before Sprach 2.0 | W4 (Embeddings), W5 (Providers), W6 (Transition) |
| M2 - Sprach 2.0 | #2 | UX, TUI, benchmarks, pre-launch | #16 (TUI), #15 (Plugins), #124 (Benchmarks) |
| M3 - Sprach 2.0 Extensions | #3 | Cognitive extensions, meta-cognition | #99-101 (S2.meta), #80 (S2.5), TAP/PEEK |
| M4 - Future & Cultural Grounding | #4 | Deferred features, cultural grounding | R-24 (Cultural Grounding), advanced retrieval |

## Step 3: Determine Board Position

**Board order = execution priority within milestone.** Items are ordered by when they should be worked on, not by importance alone.

### Position Rules (within each milestone)

1. **Urgent (priority 1)** items first (no matter what)
2. **High (2)** items next (unblock other work)
3. **Dependency chains** in order — enforced by Linear `blocks`/`blocked_by` relations (if A depends on B, B comes first)
4. **High/Medium** items by dependency order
5. **Low** priority items
6. **Draft/Research** items last (unrefined)

### Positioning in Linear

The GitHub Projects board (`PVT_*` field mutations, `updateProjectV2ItemPosition`) is **retired** — issues live in Linear now. Ordering is expressed as:

- **Milestone** = `projectMilestone` on the Linear issue (M1–M4)
- **Priority** = `priority` int (1-4; 0 = unrefined)
- **Execution sequence within a milestone** = the Linear triage/board view order (drag in the Linear UI; there is no position field to set via the public GraphQL `issueUpdate`)
- **Hard dependencies** = issue relations, not positions

To inspect current order with milestones (read-side):

```
mcp__linear__list_issues → group by projectMilestone, sort by priority.value
```

## Step 4: Create Issues (if needed)

### For new research findings:

1. **Check for duplicates** — search Linear by title keyword (`mcp__linear__list_issues` with `query`, or GraphQL `issueSearch`); for pre-migration archaeology: `gh issue list --state closed | grep -i "<keyword>"`
2. **Use sanitized terminology** — never reference private research directories (`~/papers/`, `~/macro-attention/`, author PII). Describe ideas generically (e.g., "information routing abstraction" not "Macro-Attention framework")
3. **Add to research-icebox.md** as R-XX before or alongside the issue
4. **Add BibTeX** to `papers-reference.md` for any cited papers
5. **Update unified-vision.md** if the finding changes the architecture

### Issue title format:

**CRITICAL RULE: Priority belongs in labels, NOT in titles.** Titles should be descriptive and permanent — priority changes over time, titles should not.

- ✅ `T3-Phase0: Preserve Thinking Content + Schema Foundation`
- ✅ `Norm Correction in Embedding Tables`
- ✅ `B1.5 — Context Strategy Comparison Benchmark`
- ❌ `[P0-CRITICAL] T3-Phase0: Preserve Thinking Content + Schema Foundation` (priority in title)
- ❌ `[P0] Norm Correction in Embedding Tables` (priority in title)
- ❌ `[P0-HIGH] T3-Phase1: ThinkingTrace Pipeline` (priority in title)
- ❌ `[P2] B1.5 — Context Strategy Comparison Benchmark` (priority in title)

Prefixes that ARE allowed in titles:
- `Draft:` — Research items (add `research` label) — indicates this is an investigation, not implementation
- `ADR:` — Architecture Decision Records — indicates this is a decision document, not code

**Set priority via the Linear `priority` int, not titles:**

```
mcp__linear__save_issue (update) → priority = 1|2|3|4
# HTTP fallback: mutation issueUpdate(id:, input: { priority: 2 })
```

Milestone and status likewise: `projectMilestoneId` (resolve via `mcp__linear__list_milestones` on project Sprachspiel) and `stateId` (`mcp__linear__list_issue_statuses`).

### Issue body template for research items:

```markdown
## [Title]

**Status:** Draft — needs research / P0 / P1 / P2
**Milestone:** M1 / M2 / M3 / M4
**Depends on:** #XXX (if applicable)

## Goal
[One paragraph: what this delivers and why it matters]

## Motivation
[Research citations, not file paths. Academic references only.]
[Explain the gap this fills.]

## Implementation phases
[Numbered phases with clear deliverables]

## Related
[Link to related issues, R-XX items, unified-vision sections]
```

### Key Rules for Research Integration:

1. **NEVER reference private files** — no `~/papers/`, `~/macro-attention/`, or author names. Use academic citations or self-contained descriptions.
2. **NEVER leak PII** — no author names from private research, no private directory paths.
3. **NEVER cite unpublished drafts by name** — ideas can be used but described generically. "Information routing abstraction" not "Macro-Attention framework".
4. **ALWAYS add BibTeX** — when citing a published paper, add it to `papers-reference.md`.
5. **ALWAYS add R-XX** — when adding a research finding, add it to `research-icebox.md` with full context.

## Step 5: Ordering — milestone view (Linear)

The old GitHub board reordering machinery (`updateProjectV2ItemPosition`, field IDs, `gh project item-list`) is **retired** — see Step 3 for how order is expressed in Linear (milestone + priority + relations + triage view).

The milestone-grouped execution tables below are the **doctrine** (what should be worked on first, and why); issue numbers are pre-migration GitHub numbers (`gh#N`). Their Linear counterparts are discoverable via each issue's `Ref: gh#N` description (e.g. gh#233 → LUC-141).

### Execution order within M1:

| Priority | Items | Rationale |
|----------|-------|-----------|
| P0-CRITICAL | #151 (T3-Phase0), #157 (Norm Correction) | Data loss prevention, unblock TAP |
| P0-HIGH | #152 (TAP-1), #153 (TAP-2), #148 (W6-PR4) | Pipeline dependencies |
| P0-HIGH other | #105, #116, #118, #133-#135 | Current wave |
| Embedding cluster | #136→#137→#138→#106→#107→#139→#140 | Dependency chain |
| Provider migration | #119→#120→#121→#122→#123→#72 | Dependency chain |
| Feedback cluster | #90-#97 | Parallel, independent |
| Other M1 | #36, #11, #13, #50, #74-#76, #130, #131 | Independent items |

### Execution order within M2:

| Priority | Items | Rationale |
|----------|-------|-----------|
| Foundation | #16 (TUI), #117 (TUI Modes), #15 (Plugins) | Prerequisites for M3 |
| Benchmarks | #124 (Infrastructure), #125 (Learned Patterns), #158 (B1.5) | Validate architecture |

### Execution order within M3:

| Priority | Items | Rationale |
|----------|-------|-----------|
| ADR/principle | #159 (Empathy ≠ Failure) | Doc only, sets orientation |
| Meta-cognition | #99→#100→#101 | Layer 1→2→3 dependency |
| S2.5/Connections | #80, #49, #77-#79 | Personality evolution |
| P2 features | #160-#164 | Conditional on benchmarks |
| Drafts | #179, #165, #166, #171 | Research, no code yet |

### Execution order within M4:

| Priority | Items | Rationale |
|----------|-------|-----------|
| Cultural Grounding | #156, #164 | Phase 1 (doc only) |
| Drafts | #167-#178 | Need M3 validation first |

### Reordering: legacy pattern (retired)

<details>
<summary>Historical GitHub Projects script (kept for archaeology — do NOT run)</summary>

The old flow moved items on GitHub Project board #4 via `updateProjectV2ItemPosition` GraphQL mutations with `PVT_*` project/field IDs. Superseded by Linear ordering (Step 3) as of the 2026-08-19 migration.

</details>

## Step 6: Update Documentation

After triaging, update these files:

### Files that MUST be updated:

1. **`doc/src/development/research-icebox.md`** — Add R-XX entry for new research findings
2. **`doc/src/development/unified-vision.md`** — Add section if architecture changes
3. **`IMPLEMENTATION.md`** — Update status if items change milestone or priority

### Files to update CONDITIONALLY:

4. **`doc/src/development/research/papers-reference.md`** — Add BibTeX if citing new papers
5. **`doc/src/development/roadmap.md`** — Update if strategic direction changes
6. **`doc/src/development/architecture.md`** — Update if architectural decisions change

### PII and Sanitization Audit (MANDATORY)

After updating files, verify:

```bash
# Check for PII: author names, private paths, unpublished draft references
rg "~/papers/" doc/ --type md          # Private paths
rg "~/macro-attention/" doc/ --type md  # Private directories
rg "Lucas" doc/ --type md               # Author PII
rg "Hermes" doc/ --type md              # Author PII (if used as person name)
rg "Macro-Attention" doc/ --type md     # Unpublished framework name
```

If any matches are found, replace with:
- Private paths → Academic citations or self-contained descriptions
- Author names → "et al." or citation references
- Unpublished drafts → Generic terminology ("information routing abstraction", "multi-signal gate", etc.)

## Step 7: Rebuild mdBook

After updating any documentation under `doc/`, rebuild:

```bash
mdbook build doc/
```

This propagates changes to `doc/book/html/` and `doc/book/markdown/`.

---

# Lessons Learned

These lessons come from actual triage sessions and the mistakes made during them.

## Lesson 1: Board Order = Execution Order, Not Priority

**What happened:** The first board reordering used "priority" (P0-CRITICAL first, then P0, etc.) without respecting milestone boundaries. M3 items appeared before M2 items because they had higher-priority labels.

**Correct approach:** Board order reflects **when you work on something**, not how important it is. M1 items (no matter how low priority) come before M2 items (no matter how high priority) because you can't start M2 until M1 is done.

**Rule:** Group by milestone first (M1→M2→M3→M4), then by execution order within each milestone.

## Lesson 2: Set Milestones BEFORE Ordering

**What happened:** Created and reordered issues on the board before setting their milestones. This resulted in M3 items mixed into the M1 section, M2 items in M3, etc. The reorder had to be done twice.

**Correct approach (Linear):**
1. **First:** Set milestones on all issues (`save_issue` / `issueUpdate` with `projectMilestoneId`)
2. **Then:** Verify all milestones are correct (group-by check)
3. **Then:** Order within each milestone group (priority + relations + triage view)

**Rule:** Milestones first, ordering second. Never the other way around.

## Lesson 3: Create Issues, Then Fields, Then Relations

**What happened:** Tried to create issues and add them to the board in the same command, but the GraphQL ID was needed for position updates. Also wrote issue comments before verifying the issue was created.

**Correct approach (Linear):**
1. Create all issues (`mcp__linear__save_issue` / `issueCreate`; collects LUC-N identifiers)
2. Set milestone + priority + labels on each
3. Add `blocks`/`blocked_by` relations for dependency chains
4. Add research synthesis comments to related issues

**Rule:** Create → Milestone → Priority → Relations → Comments

## Lesson 4: Compaction Quality Metric — Tiny Scope Goes Into Existing Issue

**What happened:** Initially planned to create a separate issue (#180) for "Compaction Quality Metric" as a M4 item. The user pointed out that Phase A (recency-weighted quality, ~5 lines of code) is trivially computable now and should be an addendum to #152 (TAP-1), not a separate M4 issue.

**Correct approach:** When a feature has two phases — one trivial (Phase A: ~5 lines, available now) and one complex (Phase B: multi-signal, blocked by future work) — put Phase A as a sub-item of the current work and only create a separate issue/icebox entry for Phase B.

**Rule:** Don't create separate issues for trivially small addendums. Put Phase A where the work is happening, and only Phase B in the icebox.

## Lesson 5: Research Items — Don't Create Issues for Reframed Existing Items

**What happened:** Initially planned to create separate issues for M3.β (Adaptive RRF), M3.γ (Multi-Signal Compaction), and M4.β (Cross-Scale Flow). The user pointed out these are reframings of existing items (#137, B1.5/#158, OC-2), not genuinely new items.

**Correct approach:** When research reveals that an existing item can be seen through a new lens (e.g., "this is really a multi-head retrieval problem" for #137), add a comment to the existing issue with the new design insight. Don't create a new issue for the same work.

**Rule:** New insights about existing items go as comments on those items. Only create new issues for genuinely new work that doesn't overlap with existing cards.

## Lesson 6: PII Sanitization — Never Reference Private Research

**What happened:** Research analysis was done in a private directory (`~/macro-attention/`) containing a draft paper with the author's real name. The ideas from that paper are valuable (unified attention abstraction, multi-signal gates) but the paper itself is unpublished.

**Correct approach:** Ideas from unpublished research can be used but must be described generically:
- ✅ "information routing abstraction" (generic description of the pattern)
- ✅ "multi-signal gate function" (generic description of the mechanism)
- ✅ "Gated DeltaNet-2 (Hatamizadeh et al. 2026, arXiv:2605.22791)" (published citation)
- ❌ "Macro-Attention framework" (name of unpublished paper)
- ❌ "~/macro-attention/PAPER.md" (private file path)
- ❌ "Lucas (luksamuk), Hermes" (author PII from unpublished draft)

**Rule:** Published papers get academic citations. Unpublished ideas get generic descriptions. Private directories and author names stay private.

## Lesson 7: Cross-Reference Everything

**What happened:** Research insights were added to R-21 through R-28 and to issues #137, #152, #153, #158, #124, but the connections between them were only implied by shared concepts.

**Correct approach:** Every new item must reference related items explicitly:
- R-29 icebox entry → #179 (formal issue), R-21 through R-28 (related items)
- #179 issue body → #137, #158, #152, R-21 through R-25
- #152 comment → #158 (B1.5 uses the metric), R-30 (Phase B in icebox)
- #158 comment → #152 (quality metric prerequisite), H1.1 sub-hypothesis

**Rule:** Every new entry must have an explicit "Related" or "Cross-refs" section listing the issues, R-XX items, and unified-vision sections it connects to.

## Lesson 8: Verify Milestone Grouping After Changes

**What happened:** After reordering the old GitHub board, verification showed items from different milestones interleaved, because position updates don't check milestone consistency.

**Correct approach (Linear):** After modifying milestones/priorities, verify with a milestone-grouped listing — `mcp__linear__list_issues` on project Sprachspiel, group by `projectMilestone`, and confirm each group is coherent (no M3 items sitting at High priority while M1 urgent items are Backlog, etc.). The doctrine tables in Step 5 are the reference ordering.

## Lesson 9: Priority Tags Belong in Labels, Not Titles

**What happened:** Issues were created with priority tags in titles like `[P0-CRITICAL]`, `[P0-HIGH]`, `[P2]`. These tags:
1. Make titles noisy and hard to scan
2. Become stale when priorities change (P0 today → P2 next week, but title still says P0)
3. Duplicate information that's already in labels and board priority fields
4. Leak internal priority language into user-facing issue titles

**Correct approach:** Use the Linear `priority` int and workflow states. Keep titles descriptive and permanent.

| Aspect | Wrong | Right |
|--------|-------|-------|
| Title | `[P0-CRITICAL] T3-Phase0: Preserve Thinking Content` | `T3-Phase0: Preserve Thinking Content + Schema Foundation` |
| Title | `[P2] B1.5 — Context Strategy Comparison` | `B1.5 — Context Strategy Comparison Benchmark` |
| Priority | `[P0-HIGH]` in title | Linear `priority = 2` |
| Status | `📋 READY` in title | Linear workflow state (Backlog → Todo) |

The only prefixes allowed in titles are type indicators: `Draft:` for research items and `ADR:` for architecture decisions. These indicate the *nature* of the issue, not its priority.

**When creating new issues:** Set priority via `save_issue`/`issueUpdate` (`priority` int) — never via title prefix.

**When triaging existing issues:** Remove `[P0-CRITICAL]`, `[P0]`, `[P0-HIGH]`, `[P2]`, `[Draft]` prefixes from titles and replace with Linear priority/labels. `Draft:` is the only title prefix allowed because it indicates the issue's nature (investigation vs implementation), not its priority.

## Step 8: Commit and Push

After all triage work:

```bash
# Stage only the documentation files you changed
git add doc/src/development/research-icebox.md \
        doc/src/development/research/papers-reference.md \
        doc/src/development/unified-vision.md \
        IMPLEMENTATION.md  # if status changed

# Verify what you're committing
git diff --cached --stat

# Commit with descriptive message
git commit -m "docs: add R-XX, §X.Y, and Linear reorganization

- Add R-XX (Title) to research-icebox.md
- Add §X.Y (Section) to unified-vision.md
- Add BibTeX for Paper (arXiv:XXXX) to papers-reference.md
- Reordered Linear issues by execution priority (milestones M1→M2→M3→M4)
- Set milestones on N Linear issues"

# Push
git push

# Rebuild mdBook
mdbook build doc/
```

## Project Info

- **Issue tracking:** Linear — project "Sprachspiel"; milestones "M1 - Core Evolution" … "M4 - Future & Cultural Grounding"
- **GitHub:** `luksamuk/ask-ollama-rs` (sprachspiel repository) — PRs, reviews, CI, and closed-issue history only
- **Access:** `mcp__linear__*` tools when the Linear MCP is connected; otherwise the `linear` skill (GraphQL + `LINEAR_API_KEY`)
- **Priority within milestones:** Linear `priority` int (top of triage view = work first)
- **Pre-migration issue numbers** (`gh#N`) map to Linear via `Ref: gh#N` in each issue's description
- **Old GitHub board #4 and its `PVT_*` IDs:** retired — never reference them in new work