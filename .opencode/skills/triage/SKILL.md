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

### Label Assignment

| Label | When to use |
|-------|-------------|
| `enhancement` | New feature or capability |
| `research` | Pure investigation with no code deliverable |
| `bug` | Fix for existing broken behavior |
| `documentation` | Doc-only change |
| `priority:critical` | Must fix now (data loss, security) |
| `priority:high` | Important, next sprint |
| `priority:medium` | Nice to have, planned |
| `priority:low` | Backlog, future |

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

1. **P0-CRITICAL** items first (no matter what)
2. **P0** and **P0-HIGH** items next (unblock other work)
3. **Dependency chains** in order (if A depends on B, B comes first)
4. **High** priority items by dependency order
5. **Medium** priority items by dependency order
6. **Low** priority items
7. **Draft/Research** items last (they haven't been validated yet)

### Positioning Commands

```bash
# Get current board order with milestones
gh issue list --repo luksamuk/sprachspiel --state open --limit 200 \
  --json number,title,milestone --jq '.[] | "\(.number)\t\(.milestone.title // "NO MILESTONE")\t\(.title)"'

# Move item to position in board (after a specific item)
gh api graphql -f query='
mutation {
  updateProjectV2ItemPosition(input: {
    projectId: "PVT_kwHOADplIc4BRnZ9",
    itemId: "<ITEM_ID>",
    afterId: "<AFTER_ITEM_ID>"   # omit for top position
  }) {
    clientMutationId
  }
}'

# Get item IDs from board
gh project item-list 4 --owner "@me" --limit 200 --format json | \
  python3 -c "
import json, sys
data = json.load(sys.stdin)
for item in data['items']:
    content = item.get('content') or {}
    if isinstance(content, dict):
        print(f'#{content.get(\"number\", \"??\")}: {item[\"id\"]}')"
```

## Step 4: Create Issues (if needed)

### For new research findings:

1. **Check for duplicates** — search existing issues: `gh issue list --state all | grep -i "<keyword>"`
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

**Set priority via labels, not titles:**

```bash
# Priority labels (use these, not title prefixes)
gh issue edit <N> --repo luksamuk/sprachspiel --label "priority:critical"
gh issue edit <N> --repo luksamuk/sprachspiel --label "priority:high"
gh issue edit <N> --repo luksamuk/sprachspiel --label "priority:medium"
gh issue edit <N> --repo luksamuk/sprachspiel --label "priority:low"
```

**Set priority via project board fields:**

```bash
# Using gh project item-edit (field IDs from project #4)
gh project item-edit --id <ITEM_ID> --project-id PVT_kwHOADplIc4BRnZ9 \
  --field-id PVTSSF_lAHOADplIc4BRnZ9zg_ZHWU \
  --single-select-option-id <OPTION_ID>
# Critical=63eaf02a, High=02a9e1dd, Medium=44f71207, Low=8efef8c9
```

**When triaging existing issues with priority tags in titles, remove the tags:**

If an existing issue has `[P0-CRITICAL]`, `[P0]`, `[P0-HIGH]`, or `[P2]` in its title, remove the prefix and set the priority via label/board field instead:

```bash
# Remove priority prefix from title
gh issue edit <N> --repo luksamuk/sprachspiel --title "New title without prefix"

# Set priority via label
gh issue edit <N> --repo luksamuk/sprachspiel --label "priority:high"
```

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

## Step 5: Board Reordering

After creating issues and setting milestones, reorder the board so that:

```
Board order = M1 items (execution order) → M2 items (execution order) → M3 items → M4 items
```

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

### Reordering Script Pattern

```bash
# Define order as array of issue numbers (execution order)
DESIRED_ORDER=(151 157 152 153 148 105 116 118 133 134 135 136 137 138 106 107 139 140 119 120 121 122 123 72 ...)

# Get item IDs
gh project item-list 4 --owner "@me" --limit 200 --format json > /tmp/board.json

# Move each item to position (using GraphQL updateProjectV2ItemPosition)
# First item: afterId omitted (top position)
# Subsequent items: afterId = previous item's ID
```

### Full reordering procedure (Python):

```python
import subprocess, json, time

PROJECT_ID = "PVT_kwHOADplIc4BRnZ9"
DESIRED_ORDER = [151, 157, ...]  # Issue numbers in execution order

# Get item IDs from board
result = subprocess.run(
    ["gh", "project", "item-list", "4", "--owner", "@me", "--limit", "200", "--format", "json"],
    capture_output=True, text=True
)
data = json.loads(result.stdout)

num_to_id = {}
for item in data["items"]:
    content = item.get("content") or {}
    if isinstance(content, dict) and content.get("number"):
        num_to_id[content["number"]] = item["id"]

# Move items into position
for i, num in enumerate(DESIRED_ORDER):
    item_id = num_to_id[num]
    if i == 0:
        # Move to top (no afterId)
        mutation = f'''
        mutation {{
          updateProjectV2ItemPosition(input: {{
            projectId: "{PROJECT_ID}",
            itemId: "{item_id}"
          }}) {{ clientMutationId }}
        }}'''
    else:
        # Move after previous item
        after_id = num_to_id[DESIRED_ORDER[i-1]]
        mutation = f'''
        mutation {{
          updateProjectV2ItemPosition(input: {{
            projectId: "{PROJECT_ID}",
            itemId: "{item_id}",
            afterId: "{after_id}"
          }}) {{ clientMutationId }}
        }}'''

    subprocess.run(["gh", "api", "graphql", "-f", f"query={mutation}"], capture_output=True)
    if (i + 1) % 20 == 0:
        time.sleep(1)  # Rate limiting
```

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

## Lesson 2: Set Milestones BEFORE Reordering

**What happened:** Created and reordered issues on the board before setting their milestones. This resulted in M3 items mixed into the M1 section, M2 items in M3, etc. The reorder had to be done twice.

**Correct approach:**
1. **First:** Set milestones on all issues (`gh issue edit <N> --milestone "M1 - Core Evolution"`)
2. **Then:** Verify all milestones are correct (`gh issue list --json number,milestone`)
3. **Then:** Reorder the board by position within milestone groups

**Rule:** Milestones first, positions second. Never the other way around.

## Lesson 3: Create Issues, Then Comments, Then Board

**What happened:** Tried to create issues and add them to the board in the same command, but the GraphQL ID was needed for position updates. Also wrote issue comments before verifying the issue was created.

**Correct approach:**
1. Create all issues with `gh issue create` (collects issue numbers)
2. Add issues to board with `gh project item-add`
3. Set milestones with `gh issue edit`
4. Set Priority and Scrum Status fields
5. Add research synthesis comments to existing issues
6. **Last:** Reorder the board by position

**Rule:** Create → Board → Milestone → Priority → Comments → Reorder

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

## Lesson 8: Verify Board Position with Milestone Check

**What happened:** After reordering the board, the verification showed items from different milestones interleaved (M3 items between M2 items, M4 items between M3 items). This happened because position updates don't check milestone consistency.

**Correct approach:** After reordering, always verify with a milestone-grouped listing:

```bash
gh issue list --repo luksamuk/sprachspiel --state open --limit 200 \
  --json number,title,milestone | python3 -c "
import json, sys
data = json.loads(sys.stdin.read())
by_ms = {}
for i in data:
    ms = i.get('milestone', {}) or {}
    title = ms.get('title', 'NO MILESTONE') if ms else 'NO MILESTONE'
    by_ms.setdefault(title, []).append((i['number'], i['title'][:70]))
for ms in ['M1 - Core Evolution', 'M2 - Sprach 2.0',
           'M3 - Sprach 2.0 Extensions', 'M4 - Future & Cultural Grounding',
           'NO MILESTONE']:
    if ms in by_ms:
        print(f'\n=== {ms} ({len(by_ms[ms])} issues) ===')
        for num, t in sorted(by_ms[ms]):
            print(f'  #{num:>3d} | {t}')
"
```

If items from different milestones are interleaved on the board, re-move them.

**Rule:** After reordering, verify that board positions match milestone groups. The board should read M1 block → M2 block → M3 block → M4 block without interleaving.

## Lesson 9: Priority Tags Belong in Labels, Not Titles

**What happened:** Issues were created with priority tags in titles like `[P0-CRITICAL]`, `[P0-HIGH]`, `[P2]`. These tags:
1. Make titles noisy and hard to scan
2. Become stale when priorities change (P0 today → P2 next week, but title still says P0)
3. Duplicate information that's already in labels and board priority fields
4. Leak internal priority language into user-facing issue titles

**Correct approach:** Use GitHub labels and project board fields for priority. Keep titles descriptive and permanent.

| Aspect | Wrong | Right |
|--------|-------|-------|
| Title | `[P0-CRITICAL] T3-Phase0: Preserve Thinking Content` | `T3-Phase0: Preserve Thinking Content + Schema Foundation` |
| Title | `[P2] B1.5 — Context Strategy Comparison` | `B1.5 — Context Strategy Comparison Benchmark` |
| Priority | `[P0-HIGH]` in title | `priority:high` label + board Priority field |
| Status | `📋 READY` in title | Scrum Status field on project board |

The only prefixes allowed in titles are type indicators: `Draft:` for research items and `ADR:` for architecture decisions. These indicate the *nature* of the issue, not its priority.

**When creating new issues:** Set priority via `--label "priority:high"` and board fields, never via title prefix.

**When triaging existing issues:** Remove `[P0-CRITICAL]`, `[P0]`, `[P0-HIGH]`, `[P2]`, `[Draft]` prefixes from titles and replace with labels. `Draft:` is the only title prefix allowed because it indicates the issue's nature (investigation vs implementation), not its priority.

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
git commit -m "docs: add R-XX, §X.Y, and board reordering

- Add R-XX (Title) to research-icebox.md
- Add §X.Y (Section) to unified-vision.md
- Add BibTeX for Paper (arXiv:XXXX) to papers-reference.md
- Reordered GitHub project board by execution priority (M1→M2→M3→M4)
- Set milestones on N issues"

# Push
git push

# Rebuild mdBook
mdbook build doc/
```

## Project Info

- **GitHub:** `luksamuk/ask-ollama-rs` (sprachspiel repository)
- **Project Board:** Number 4 (Sprachspiel Roadmap)
- **Board ID:** `PVT_kwHOADplIc4BRnZ9`
- **Priority within milestones:** determined by board position (top = highest priority, work first)
- **Items referenced by issue number** (e.g., #72, #116) — P-code prefixes retired
- **Milestone field IDs:**
  - Priority: `PVTSSF_lAHOADplIc4BRnZ9zg_ZHWU` (Critical/High/Medium/Low)
  - Scrum Status: `PVTSSF_lAHOADplIc4BRnZ9zg_ZHUY` (Backlog/Ready/In Progress/In Review/Done)
  - Estimate: `PVTF_lAHOADplIc4BRnZ9zg_ZHWY` (number)
- **Milestone IDs:** M1 = `#1`, M2 = `#2`, M3 = `#3`, M4 = `#4`