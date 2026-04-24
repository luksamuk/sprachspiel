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

I guide the complete PR workflow for the ask-ai project, from branch creation through merge. I cover every phase with exact commands and decision points.

## When to use me

Use this skill when you are implementing a feature or fix that requires a PR. Load me after the next-demand skill has identified the task and the user has approved it.

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
# Search for issues with similar titles
gh issue list --state all --limit 100 | grep -i "<keyword from title>"

# For each match, read the full description
gh issue view <number>
```

If a duplicate is found:
1. **Identify the canonical issue** — the one that was created first, or the one with more context
2. **If canonical is CLOSED with a merged PR** — check whether the PR fully addressed the issue. If not, note residual work on the canonical issue.
3. **Close the duplicate** with a comment: `"Closing as duplicate of #<canonical> — both issues describe the same problem."`
4. **Reference the canonical issue** in your branch name, PR title, and PR body (not the duplicate)
5. **Update IMPLEMENTATION.md** to reference the canonical issue number

### Move Card to In Progress

Move the GitHub Project card to "In Progress" (project number 4 = Ask-AI Roadmap):

```bash
# Find the item ID by issue number (use CANONICAL issue number)
ITEM_ID=$(gh issue view <issue_number> --json projectItems --jq '.projectItems[] | select(.project.number == 4) | .id')

# If item is NOT on the board, add it:
gh project item-add 4 --owner luksamuk --url https://github.com/luksamuk/ask-ai-rs/issues/<issue_number>

# Update Status field to "In Progress"
gh api graphql -f query='
mutation {
  updateProjectV2ItemFieldValue(
    input: {
      projectId: "PVT_kwHOADplIc4BRnZ9"
      itemId: "'"$ITEM_ID"'"
      fieldId: "PVTSSF_lAHOADplIc4BRnZ9zg_ZGpg"
      value: { singleSelectOptionId: "47fc9ee4" }
    }
  ) { projectV2Item { id } }
}'

# Update Scrum Status field to "In Progress"
gh api graphql -f query='
mutation {
  updateProjectV2ItemFieldValue(
    input: {
      projectId: "PVT_kwHOADplIc4BRnZ9"
      itemId: "'"$ITEM_ID"'"
      fieldId: "PVTSSF_lAHOADplIc4BRnZ9zg_ZHUY"
      value: { singleSelectOptionId: "c2eae8ae" }
    }
  ) { projectV2Item { id } }
}'
```

**Card management rules:**
- Move card to **In Progress** when starting implementation (Phase 1)
- Move card to **In Review** when PR is ready for review (Phase 4)
- Card moves to **Done** automatically when PR merges (via "Closes #N")
- If the issue is not on the board, **add it** before moving
- Always use the **canonical issue** for board cards (not duplicates)

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

# Create PR as DRAFT with issue reference
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

Closes #<issue_number>
EOF
)"

# Link PR to issue
gh issue comment <issue_number> --body "PR #<pr_number> criado para resolver esta issue."
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

# Commit with conventional commits
git commit -m "feat: <description>"
# or: fix:, refactor:, docs:, test:, chore:

git push
```

## Phase 4: Mark PR Ready for Review

```bash
# Run formatters FIRST
cargo fmt
cargo clippy -- -D warnings

# Mark PR as ready
gh pr ready <pr_number>
```

### Update Project Board Card

Move the card to "In Review":

```bash
# Find item ID (use CANONICAL issue number)
ITEM_ID=$(gh issue view <issue_number> --json projectItems --jq '.projectItems[] | select(.project.number == 4) | .id')

# Status → "In Review"
gh api graphql -f query='
mutation {
  updateProjectV2ItemFieldValue(
    input: {
      projectId: "PVT_kwHOADplIc4BRnZ9"
      itemId: "'"$ITEM_ID"'"
      fieldId: "PVTSSF_lAHOADplIc4BRnZ9zg_ZGpg"
      value: { singleSelectOptionId: "77520bb7" }
    }
  ) { projectV2Item { id } }
}'

# Scrum Status → "In Review"
gh api graphql -f query='
mutation {
  updateProjectV2ItemFieldValue(
    input: {
      projectId: "PVT_kwHOADplIc4BRnZ9"
      itemId: "'"$ITEM_ID"'"
      fieldId: "PVTSSF_lAHOADplIc4BRnZ9zg_ZHUY"
      value: { singleSelectOptionId: "d242b7c7" }
    }
  ) { projectV2Item { id } }
}'
```

### Cross-Reference Related Issues

If the PR complements a previously merged PR on the same issue:

```bash
# Comment on the original issue about the supplementing PR
gh issue comment <issue_number> --body "PR #<pr_number> complements this fix with additional robustness: [list residual fixes]."
```

```bash
# Comment on the issue about PR being ready for review
gh issue comment <issue_number> --body "PR #<pr_number> ready for review"
```

## Phase 5: Review & Iteration

This phase repeats until all review comments are resolved.

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
| ✅ Resolvido | Code fixed | Changed code to address the comment |
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

### If Implementation Changes Needed

1. Create todo list of required changes
2. Wait for user confirmation
3. Implement approved changes
4. Push changes
5. **Return to fetching review threads** (new commits need review)

### After All Threads Resolved

- Inform user
- Wait for reviewer approval → proceed to Phase 6 (testing)

## Phase 7: Merge (AFTER Authorization)

```bash
# User authorizes merge
gh pr merge PR_NUMBER --merge --delete-branch

# IMPORTANT:
# - Use --merge (NOT --squash) to preserve commit history
# - Use --delete-branch to clean up
```

### Post-Merge Cleanup

```bash
# Update IMPLEMENTATION.md — mark task as ✅ COMPLETED
# Find the section and update status markers

# Verify card moved to "Done" automatically (via "Closes #N" in PR body)
# If the issue was CLOSED but card didn't move, manually update:
ITEM_ID=$(gh issue view <issue_number> --json projectItems --jq '.projectItems[] | select(.project.number == 4) | .id')
gh api graphql -f query='
mutation {
  updateProjectV2ItemFieldValue(
    input: {
      projectId: "PVT_kwHOADplIc4BRnZ9"
      itemId: "'"$ITEM_ID"'"
      fieldId: "PVTSSF_lAHOADplIc4BRnZ9zg_ZGpg"
      value: { singleSelectOptionId: "98236657" }
    }
  ) { projectV2Item { id } }
}'
```

### Duplicate Issue Resolution (if applicable)

If the PR addresses a canonical issue that had duplicates:

1. **Verify all duplicates are closed** — check that duplicate issues have been closed with cross-reference comments
2. **Remove duplicate cards from board** — if duplicate issues had their own cards, remove them:
   ```bash
   gh project item-delete <project-number> --id <duplicate-item-id>
   ```
3. **Verify board state** — only the canonical issue card should remain, in "Done"

## Key Rules

1. **NEVER skip the PR-PROCESS.md steps** — follow them in order
2. **NEVER close issues before PR merge** — they auto-close with "Closes #N"
3. **NEVER merge without approval** — PRs must be reviewed
4. **ALWAYS create PR as DRAFT first** — then implement, then mark ready
5. **ALWAYS check for duplicate issues** before creating a branch — use the duplicate check in Phase 1
6. **ALWAYS use canonical issue for references** — branch names, PR titles, PR bodies, board cards should all reference the canonical issue (not a duplicate)
7. **ALWAYS move project board cards** — at every phase transition:
   - Phase 1 (Setup): → "In Progress"
   - Phase 4 (Ready for Review): → "In Review"
   - Phase 7 (Merge): → "Done" (automatic via "Closes #N", verify manually)
8. **ALWAYS cross-reference related issues** — comment on the canonical issue about the PR, close duplicates with explanation
9. **ALWAYS update IMPLEMENTATION.md** — mark status on every phase change
10. **ALWAYS wait for authorization** between phases — no autonomous progression

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