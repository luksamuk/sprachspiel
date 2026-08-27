---
name: pr-workflow
description: Complete PR workflow from branch creation to merge. Covers all phases: setup, documentation, draft PR, planning, requirements checkpoint, implementation, review, card movement, and merge.
license: MIT
compatibility: opencode
metadata:
  audience: maintainers
  workflow: pr-process
---

## What I do

I guide the complete PR workflow for the sprachspiel project, from branch creation through merge. I cover every phase with exact commands and decision points.

**Issue tracking is Linear (since 2026-08-19); PRs/reviews stay on GitHub.** Issues are `LUC-N` (ex gh#N — the Linear description carries `Ref: gh#N`). Access Linear via `mcp__linear__*` tools when the MCP is connected, else the `linear` skill (GraphQL + `LINEAR_API_KEY`). The Linear GitHub integration auto-links branches/PRs named with `LUC-N` and moves issue status on PR open/merge; magic words (`Fixes LUC-NNN`) in commit messages/PR body also close issues.

**If the selected card has `🟡 RESEARCH NEEDED` status, Phase 0 (Research) is MANDATORY before any branch creation.**

## When to use me

Use this skill when you are implementing a feature or fix that requires a PR. Load me after the next-demand skill has identified the task and the user has approved it.

**Research gate:** If the card status is `🟡 RESEARCH NEEDED`, Phase 0 must complete before Phase 1.

## Phase 0: Research (MANDATORY for 🟡 RESEARCH NEEDED cards)

**When:** The selected card has status `🟡 RESEARCH NEEDED` in IMPLEMENTATION.md or the issue description contains "open questions" that are not yet answered.

**Why:** Cards marked 🟡 RESEARCH have unresolved design questions. Without research, Phase 2.5 (planning) and Phase 2.6 (requirements checkpoint) will stall on ⚠️ VAGUE items. Creating a branch before answering these questions wastes effort if the approach changes.

**NO branch is created in this phase.** This is a read-only investigation.

### Step 0.1: Identify Research Questions

1. Read the card's section in `IMPLEMENTATION.md` — look for "Open questions:" lists
2. Read the GitHub issue body — look for checkboxes that are unchecked, "TBD" effort estimates, or "Research needed" labels
3. Compile a **Research Question List**:

```
| # | Question | Source | Answered? |
|---|----------|--------|-----------|
| 1 | ... | IMPLEMENTATION.md §#74 | ❌ |
```

### Step 0.2: Investigate (READ-ONLY)

Investigate each question without modifying any code:

1. **Read codebase** — search for relevant files, patterns, existing abstractions
2. **Check dependencies** — verify if prerequisites exist or are planned
3. **Evaluate trade-offs** — for architecture decisions with multiple approaches
4. **Test assumptions** — write throwaway snippets if needed (do NOT commit)

Research activities:
```bash
# Read relevant source files
rg "pattern" src/ --type rust

# Check existing interfaces and types
rg "struct|trait|enum" src/relevant_module/

# Review dependency docs
cargo doc --open  # if helpful
```

### Step 0.3: Produce Research Summary

Write a Research Summary document (as an issue comment) with:

```markdown
## Research Summary — #<issue_number>

### Questions Answered

| # | Question | Decision | Rationale |
|---|----------|----------|-----------|
| 1 | ... | ✅ Answer | ... |

### Questions Still Open (Deferred)

| # | Question | Why Deferred | Impact on Implementation |
|---|----------|-------------|--------------------------|
| 1 | ... | Needs runtime testing | Low — can implement core first |

### Architecture Proposal
[Detailed architecture if the research produced a design]

### Revised Effort Estimate
[Original] → [Revised] (explain change if any)

### Recommendation
[READY to implement / DEFER to later / SPLIT into smaller issues]
```

### Step 0.4: Update Documentation

1. **Post Research Summary** as a comment on the Linear issue:
   ```
   mcp__linear__save_comment(issueId: LUC-N, body: "## Research Summary\n\n...")
   # HTTP fallback: mutation commentCreate(input: { issueId, body })
   ```

2. **Update IMPLEMENTATION.md** — replace `🟡 RESEARCH NEEDED` with `📋 PLANNED`:
   - Fill in answered open questions
   - Update effort estimate if revised
   - Add architecture proposal section if produced

3. **Update issue state** in Linear: `Backlog → Todo` (or the team's planned state) via `mcp__linear__save_issue` / `issueUpdate` with `stateId` (resolve the UUID from `mcp__linear__list_issue_statuses` — names are discoverable, never hardcoded).

### Step 0.5: ⛔ WAIT for User Approval

**Present the Research Summary to the user and WAIT.** The user must approve before proceeding:

- ✅ **Approved** → Proceed to Phase 1 (Setup). The card is now `📋 PLANNED` / Scrum `Ready`.
- 📋 **Partially approved** — Some questions need more investigation. Loop back to Step 0.2 for specific questions.
- ❌ **Not viable now** — Document why in the issue, move card back to `Backlog`, do NOT create a branch.

**Gate rule:** Phase 0 MUST produce all ⚠️ VAGUE → ✅ CLEAR transitions before Phase 1 starts. If any open questions remain that block implementation, the card is not ready for a branch.

### Research Outcomes

| Outcome | Status Change | Next Step |
|---------|---------------|-----------|
| All questions answered | `🟡 RESEARCH` → `📋 PLANNED` (Scrum: Ready) | Phase 1 |
| Partially answered, core viable | `🟡 RESEARCH` → `📋 PLANNED` with caveats | Phase 1 (document deferred items) |
| Not viable now | `🟡 RESEARCH` → stays `🟡 RESEARCH` (Scrum: Backlog) | Stop, no branch |
| Should be split | Close original, create focused sub-issues | Return to next-demand |

## Phase 1: Setup

```bash
# Create branch with conventional prefix
git checkout master && git pull origin master
git checkout -b <type>/<description>
# Types: feat/, fix/, refactor/, docs/, test/

# Verify you're on the correct branch
git branch --show-current

# Read IMPLEMENTATION.md to understand task status
```

### Pre-Setup: Duplicate Issue Check (MANDATORY)

Before creating a branch, verify the issue is not a duplicate:

```bash
# Search Linear for issues with similar titles (MCP-first):
#   mcp__linear__list_issues(query: "<keyword from title>")   → project == "Sprachspiel"
# HTTP fallback: GraphQL issueSearch(query: "...")
# Pre-migration archaeology: gh issue list --state closed | grep -i "<keyword>"
# For each match, read the full description: mcp__linear__get_issue LUC-N
```

If a duplicate is found:
1. **Identify the canonical issue** — the one that was created first, or the one with more context
2. **If canonical is DONE (closed via merged PR)** — check whether the PR fully addressed the issue. If not, note residual work on the canonical issue.
3. **Mark the duplicate** in Linear with state "Duplicate" + a comment `Closing as duplicate of LUC-<canonical> — both issues describe the same problem.`
4. **Reference the canonical issue** in your branch name, PR title, and PR body (not the duplicate)
5. **Update IMPLEMENTATION.md** to reference the canonical issue

### Move Issue to In Progress

The Linear GitHub integration moves the issue automatically when you open the PR — but when *starting* implementation, update the issue state explicitly:

```
mcp__linear__save_issue(LUC-N, state: "In Progress")   # stateId via list_issue_statuses
```

Use the issue's `gitBranchName` (`luc-NNN-slug`) for the branch — that's what the integration matches.

## Phase 2: Documentation FIRST

**ALWAYS update documentation BEFORE writing code.**

1. **Update CHANGELOG.md:**
   - Add entry under `[Unreleased]`
   - Use "Added", "Changed", "Fixed", "Removed" sections

2. **Update IMPLEMENTATION.md:**
   - Mark task as `🔄 IN PROGRESS`
   - Add implementation plan with phases table
   - Will mark as `✅ COMPLETED` only after merge

3. **Commit documentation:**
   ```bash
   git add doc/src/CHANGELOG.md IMPLEMENTATION.md
   git commit -m "docs: update CHANGELOG for <feature>"
   ```

## ⛔ STOP POINT: Create Draft PR and Wait

**After Phase 2, you MUST create the draft PR and STOP. DO NOT implement yet.**

```bash
# Push branch
git push -u origin <branch>

# Create PR as DRAFT with Linear magic word (auto-links + auto-closes the LUC issue on merge)
gh pr create --draft --title "<type>: <description>" --body "$(cat <<'EOF'
## Summary

Brief description of changes.

## Changes

| File | Change |
|------|--------|
| `src/...` | Added ... |

## Testing

- [ ] `cargo build --all-features`
- [ ] `cargo clippy --all-features -- -D warnings`
- [ ] `cargo test --all-features`

## Related

Fixes LUC-<N>
EOF
)"

# Optional extra ping on the Linear issue (integration already auto-links by branch/PR):
# mcp__linear__save_comment(LUC-N, body: "Draft PR #<pr_number> aberto.")
```

**STOP AND WAIT for user authorization.** The user will enter "planning mode."

## Phase 2.5: Planning Mode (AFTER Authorization)

1. **Analyze codebase** (READ-ONLY, no modifications):
   - Read relevant files
   - Identify patterns and conventions
   - Check for existing abstractions to reuse

2. **Create implementation plan:**
   - Specific files to modify/create
   - Function signatures
   - Estimated line counts
   - Complexity reduction targets

3. **Ask user clarifying questions**

4. **WAIT for user approval of plan**

5. **After approval:**
   - Update IMPLEMENTATION.md with detailed plan
   - Update PR body with implementation plan
   - Commit and push documentation changes

## Phase 2.6: Requirements Checkpoint (NON-NEGOTIABLE) ⛔

**This phase is MANDATORY. NEVER skip it.**

1. Extract ALL requirements from:
   - The issue(s) being addressed
   - The IMPLEMENTATION.md plan
   - The approved Phase 2.5 plan
   - Any AGENTS.md guidelines that apply

2. Classify each requirement:
   - ✅ CLEAR — Well-defined, ready to implement
   - ⚠️ VAGUE — Needs clarification
   - ❌ CONFLICT — Contradicts existing behavior
   - 🔄 REWORK — Duplicates existing code

3. Present requirements table:

| # | Requirement | Source | Status | Notes |
|---|-------------|--------|--------|-------|
| 1 | ... | Issue #N | ✅ CLEAR | ... |

4. For ⚠️ VAGUE or ❌ CONFLICT items: ask user, WAIT for decision
5. For 🔄 REWORK items: explain existing code, ask user
6. After ALL requirements are ✅ CLEAR: ask "May I proceed to implementation?"
7. **WAIT for explicit authorization**

## Phase 3: Implementation

```bash
# Implement tasks from TODO list
# Run tests and linters
cargo test --all-features
cargo clippy --all-features -- -D warnings

# Commit with conventional commits (see below)
git push
```

### Conventional Commits

Format: `<type>: <description>`

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `refactor` | Code refactoring |
| `docs` | Documentation only |
| `test` | Adding/updating tests |
| `chore` | Maintenance tasks |

## Phase 4: Mark PR Ready for Review

```bash
# Run formatters FIRST
cargo fmt
cargo clippy -- -D warnings

# Mark PR as ready
gh pr ready <pr_number>
```

### Update Issue State in Linear

Move the issue to "In Review" (the Linear GitHub integration usually does this when the PR goes ready-for-review — verify, and set explicitly if it didn't):

```
mcp__linear__save_issue(LUC-N, state: "In Review")   # stateId via list_issue_statuses
```

### Cross-Reference Related Issues

If the PR complements a previously merged PR on the same issue, comment on the Linear issue:

```
mcp__linear__save_comment(LUC-N, body: "PR #<pr_number> complements this fix with additional robustness: [...]")
mcp__linear__save_comment(LUC-N, body: "PR #<pr_number> ready for review")
```

## Phase 5: Review & Iteration

This phase repeats until all review comments are resolved.

**Load the `code-review` skill for detailed instructions on:**
- Creating reviews with inline comments (single review, multiple comments)
- Responding to review threads (use `addPullRequestReviewThreadReply`, NOT separate reviews)
- Resolving review threads via GraphQL
- Project-specific review patterns (ANSI migration, dual state, YAGNI, etc.)

### Fetch ALL Review Threads

**CRITICAL: Always use `last: 50` (NOT `first: 30`) to get ALL threads.**

```bash
gh api graphql -f query='
query {
  repository(owner: "OWNER", name: "REPO") {
    pullRequest(number: PR_NUMBER) {
      reviewThreads(last: 50) {
        totalCount
        nodes {
          id
          path
          line
          isResolved
          comments(first: 1) { nodes { body } }
        }
      }
    }
  }
}'
```

**Verify:** Check that `totalCount` matches the number of nodes returned.

### Respond to EACH Thread Individually

Use response prefixes:

| Prefix | Meaning | When to Use |
|--------|---------|-------------|
| ✅ Resolvido | Code fixed/removed | Changed code to address the comment |
| ✅ Verificado | Correct as-is | Behavior is intentional |
| 📋 | Deferred | Good suggestion, future PR |
| ❌ | Declined | Not applicable, with explanation |
| ❓ | Clarification | Question about the comment |

```bash
# Reply to a specific thread
gh api graphql -f query='
mutation {
  addPullRequestReviewThreadReply(input: {
    pullRequestReviewThreadId: "THREAD_ID",
    body: "✅ Resolvido. Fixed in commit abc1234."
  }) { comment { id } }
}'
```

**NEVER reply in a single summary comment** — each thread needs its own reply.

**NEVER create a single large comment that summarizes all review responses.** Instead, reply to each review thread inline. If inline replies are not technically possible (e.g., no thread ID available), create ONE comment per review point with a blockquote of the original comment. Never merge multiple review responses into a single wall-of-text comment.

### ⛔ ANTI-PATTERN: Creating Multiple Reviews for Responses

**NEVER create separate GitHub Review submissions for each comment or response.** This produces empty review bodies and scatters the conversation across N reviews.

```bash
# ❌ BAD — 7 separate reviews with empty bodies = noisy and confusing
for comment in $comments; do
    curl -X POST .../reviews -d '{"event": "COMMENT", "body": "", "comments": [$comment]}'
done

# ✅ GOOD — One review with all inline comments
curl -X POST .../reviews -d '{
    "event": "COMMENT",
    "body": "## Review Summary\n\nFound 3 issues. See inline comments.",
    "comments": [comment1, comment2, comment3]
}'
```

**When RESPONDING to existing review comments, use `addPullRequestReviewThreadReply` or `in_reply_to` to reply within the existing thread.** See the `code-review` skill for the exact API patterns.

### Creating a Code Review (When Reviewing Others' PRs)

**Always submit all inline comments in a single review submission.** The `comments` array in the GitHub Reviews API accepts multiple comments:

```bash
curl -s -X POST \
  -H "Authorization: token $TOKEN" \
  -H "Content-Type: application/json" \
  "https://api.github.com/repos/luksamuk/sprachspiel/pulls/PR_NUMBER/reviews" \
  -d '{
    "commit_id": "'$HEAD_SHA'",
    "event": "COMMENT",
    "body": "## Code Review Summary\n\n3 issues found. See inline comments.",
    "comments": [
      {"path": "src/file.rs", "line": 42, "body": "🔴 **Critical:** Description"},
      {"path": "src/other.rs", "line": 15, "body": "⚠️ **Warning:** Description"},
      {"path": "src/utils.rs", "line": 8, "body": "💡 **Suggestion:** Description"}
    ]
  }'
```

For project-specific review patterns (ANSI migration, dual state, dead_code policy, etc.), **load the `code-review` skill.**

### Good vs Bad Review Responses

```markdown
# BAD — Single large comment addressing everything
## ✅ Review Responses
### Point 1 — cosine_similarity
Fixed in abc123.
### Point 2 — config.rs
Not YAGNI, removed #[allow(dead_code)].
### Point 3 — Some other thing
...

# GOOD — Individual replies per thread
Thread 1 reply: "✅ Resolvido. Fixed in abc123."
Thread 2 reply: "✅ Verificado. Not YAGNI — used in command_handlers.rs:2954."
Thread 3 reply: "📋 Deferred. Good suggestion, will address in follow-up PR."
```

### If Implementation Changes Needed

1. Create todo list of required changes
2. Wait for user confirmation
3. Implement approved changes
4. Push changes
5. **Return to fetching review threads** (new commits need review)

### Review Iteration Scenarios

During review, several scenarios may occur:

- **Implementation changes needed:** Create todo list → user confirms → implement → push → re-review
- **Scope creep detected:** Discuss with user → may open separate issues
- **YAGNI identified:** Explain why code should be removed → remove with agreement
- **Large issue detected:** Discuss splitting into multiple PRs
- **Bugs found during testing:** Document → fix → push → return to review iteration

**Every push triggers a new review cycle.** The reviewer must review new commits before proceeding.

### Multiple Issues per PR

When addressing multiple related issues in a single PR:

1. **Both issues must be related** — don't combine unrelated work
2. **PR title describes both** — e.g., `feat: memory staleness warnings and truncation notices`
3. **PR body references all issues** — use `Fixes LUC-A, Fixes LUC-B` (magic words) for auto-close
4. **Both issues follow the same flow** — both move to "In Progress" at start, both to "In Review" at Phase 4

### Quality Gates

Before each commit and PR, run the quality gate sensors. **Load the `quality-gates` skill for the complete sensor hierarchy.**

Minimum before each commit:
1. `cargo fmt --check` — formatting violations
2. `cargo check --all-features` — compilation errors

Minimum before each PR:
3. `cargo clippy --all-features -- -D warnings` — lints
4. `cargo test --all-features` — regressions
5. Bare `#[allow(dead_code)]` check — unjustified dead code

### After All Threads Resolved

- Inform user
- Wait for reviewer approval → proceed to Phase 6 (testing)

## Phase 7: Merge (⛔ REQUIRES EXPLICIT USER AUTHORIZATION)

**⛔ NON-NEGOTIABLE: The agent MUST NOT merge unless the user explicitly says to merge.**

All quality gates passing, all reviews resolved, and all tests passing is NOT sufficient authorization. The user must give an explicit "merge it" / "pode mergear" / "pronto para merge" command. If in doubt, ASK. Never assume.

```bash
# ONLY after user explicitly authorizes merge
gh pr merge PR_NUMBER --merge --delete-branch

# IMPORTANT:
# - Use --merge (NOT --squash) to preserve commit history
# - Use --delete-branch to clean up
```

### Post-Merge Cleanup

```bash
# Update IMPLEMENTATION.md — mark task as ✅ COMPLETED
# Find the section and update status markers

# Verify the Linear issue moved to "Done" automatically (via "Fixes LUC-N" in PR body + the GitHub integration).
# If it didn't, set explicitly: mcp__linear__save_issue(LUC-N, state: "Done")
```

### Duplicate Issue Resolution (if applicable)

If the PR addresses a canonical issue that had duplicates:

1. **Verify all duplicates are closed** — Linear state "Duplicate" with cross-reference comments
2. **Verify only the canonical issue tracks the work** — duplicates carry no milestone/priority

## Key Rules

1. **NEVER skip the PR-PROCESS.md steps** — follow them in order
2. **NEVER skip Phase 0** — if a card is `🟡 RESEARCH NEEDED`, research MUST complete before creating a branch
3. **NEVER close issues before PR merge** — they auto-close via the Linear GitHub integration when the PR with `Fixes LUC-N` merges
4. **NEVER merge without explicit user authorization** — "All green" is NOT authorization. The user must say "merge it" / "pode mergear" / "pronto para merge". Ask if unsure.
5. **ALWAYS create PR as DRAFT first** — then implement, then mark ready
6. **ALWAYS check for duplicate issues** before creating a branch — use the duplicate check in Phase 1
7. **ALWAYS use canonical issue for references** — branch names, PR titles, PR bodies all reference the canonical Linear issue (not a duplicate)
8. **ALWAYS move the Linear issue state** — at every phase transition:
   - Phase 0 (Research): Backlog → `Todo` (when research complete)
   - Phase 1 (Setup): → `In Progress`
   - Phase 4 (Ready for Review): → `In Review` (integration usually does this on PR ready; verify)
   - Phase 7 (Merge): → `Done` (automatic via `Fixes LUC-N` + integration; verify, fix if missed)
9. **ALWAYS cross-reference related issues** — comment on the canonical Linear issue about the PR, close duplicates with explanation
10. **ALWAYS update IMPLEMENTATION.md** — mark status on every phase change
11. **ALWAYS wait for authorization** between phases — no autonomous progression
12. **ALWAYS run quality gates** before commits and PRs — load `quality-gates` skill for the complete sensor hierarchy

## Project Information

- **Issue tracking:** Linear — project "Sprachspiel" (milestones M1–M4)
- **GitHub:** `luksamuk/sprachspiel` — PRs, reviews, CI only
- **Old GitHub Project board #4:** retired 2026-08-19 (its `PVT_*` IDs and option hashes were scrubbed; see git history if ever needed for archaeology)
- **Linear workflow states** (team-level, resolve at runtime via `mcp__linear__list_issue_statuses`): Backlog, Todo, In Progress, In Review, Done, Canceled, Duplicate