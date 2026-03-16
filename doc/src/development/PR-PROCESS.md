# Pull Request Process

This document describes the mandatory workflow for implementing features and fixes.

## ⚠️ CRITICAL: FOLLOW STEPS IN ORDER

**DO NOT skip steps. DO NOT jump ahead. Each step must be completed before the next.**

- ❌ DO NOT start implementing before Phase 2 is complete
- ❌ DO NOT create PR before Phase 3 is complete
- ❌ DO NOT mark PR "ready for review" before Phase 4 is complete
- ✅ DO read this document BEFORE starting ANY implementation

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

## Workflow: Step by Step

### Phase 1: Setup

```
1. Create branch: git checkout -b <type>/<description>
   Types: feat/, fix/, refactor/, docs/, test/

2. Verify you're on the correct branch

3. Read IMPLEMENTATION.md to understand the task status

4. Move GitHub Project card to "In Progress" (both Status and Scrum Status)
   This signals that work has begun on the task
```

### Phase 2: Documentation FIRST

```
5. Update CHANGELOG.md:
   - Add entry under appropriate version
   - Use "Added", "Changed", "Fixed", "Removed" sections

6. Update IMPLEMENTATION.md:
   - Mark task status as 🔄 IN PROGRESS when starting
   - Will mark as ✅ COMPLETED only after merge

7. Commit documentation: git commit -m "docs: update CHANGELOG for <feature>"
```

### ⚠️ STOP POINT: Create Draft PR and Wait for Authorization

**CRITICAL: After completing Phase 2, you MUST create the draft PR and STOP.**

After documentation is committed:

```
8. Push branch: git push -u origin <branch>

9. Create PR as DRAFT:
   gh pr create --draft --title "<type>: <description>" --body "..."

10. Link PR to issue:
    gh issue comment <issue_number> --body "PR #<pr_number> criado (draft)."

11. STOP AND WAIT for user authorization

    DO NOT proceed to Phase 3 (Implementation) until authorized.
    The user will enter "planning mode" to discuss implementation approach.
    
    DO NOT move card to "In Review" yet - the PR is still in DRAFT status.
    Card stays in "In Progress" until Phase 4 (ready for review).
```

**Why this stop point exists:**
- Allows user to review planned changes before code is written
- Provides opportunity for architecture discussion
- Prevents wasted effort on wrong approach
- Enables course correction early

### Phase 3: Implementation (AFTER Authorization)

**Only start Phase 3 after user authorizes continuation.**

```
12. Implement the feature/fix

13. Run tests: cargo test --all-features

14. Run clippy: cargo clippy --all-features -- -D warnings

15. Commit code changes (conventional commits):
    feat: <description>
    fix: <description>
    refactor: <description>

16. Push commits: git push
```

### Phase 4: Mark PR Ready for Review

```
17. Mark PR as "ready for review":
    gh pr ready <number>

18. Add comment to issue: "PR #N ready for review"
```

### Phase 5: Review & Iteration (COLLABORATIVE)

This phase repeats until all review comments are resolved.

```
19. Reviewer adds comments to PR

20. Agent fetches ALL unresolved review comments:
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

    IMPORTANT: Use 'last: 50' (not 'first: 30') to get ALL threads.
    Verify thread count matches totalCount.

21. Agent responds to EACH unresolved comment individually:
    - Use response prefixes:
      ✅ Resolvido (for fixed code)
      ✅ Verificado (for confirmed correct behavior)
      📋 Acknowledged, deferred (good suggestion, future work)
      ❌ Declined (with explanation)
      ❓ Clarification needed

    Example:
    gh api graphql -f query='
    mutation {
      addPullRequestReviewThreadReply(input: {
        pullRequestReviewThreadId: "THREAD_ID",
        body: "✅ Resolvido. Fixed in commit abc1234."
      }) { comment { id } }
    }'

22. If implementation changes needed:
    a. Create a todo list overview of changes
    b. Wait for user confirmation before implementing
    c. Implement approved changes
    d. Update documentation as needed
    e. Push changes

23. If scope creep detected:
    - Discuss with user
    - May need to open separate issues

24. Update PR description and documentation if changes were made

25. Agent checks for unresolved comments again:
    - If unresolved comments exist → return to step 21
    - If all resolved → inform user and wait for approval

26. User reviews and either:
    - Approves and proceeds to Phase 6 (manual testing)
    - Adds more comments → return to step 21
```

### Phase 6: Manual Testing (REVIEWER)

After all review comments are resolved, the reviewer performs manual testing.

```
27. Reviewer marks all review comments as resolved

28. Reviewer tests the application manually:
    - Build and run: cargo build --all-features && cargo run
    - Test the specific feature/fix
    - Verify edge cases
    - Check for regressions

29. If bugs found during testing:
    a. Reviewer documents bugs in PR comments
    b. Agent creates todo list of fixes
    c. User confirms fixes
    d. Agent implements fixes
    e. Agent documents bugs fixed in PR body
    f. Agent pushes changes
    g. **Return to Step 21 (review iteration)** - new commits need review

30. If testing passes:
    - Proceed to Phase 7 (merge)

**IMPORTANT:** Every time the agent pushes commits (whether fixing review comments, 
bugs from testing, or any other changes), the PR returns to Step 21 for a new 
review iteration. The reviewer must review the new commits and either:
- Add more comments → continue iteration
- Add more comments → continue iteration
- Approve → proceed to Phase 7

**CRITICAL:** The reviewer can only review after the agent has pushed ALL commits.
Before informing the reviewer that changes are ready, the agent MUST ensure:
- All commits are pushed to the remote branch
- `git status` shows "up to date with 'origin/<branch>'"
- No local commits remain unpushed
```

### Phase 7: Merge (AGENT, after authorization)

```
31. User authorizes merge (all comments resolved, testing passed)

32. Agent merges PR using regular merge (NOT squash):
    gh pr merge PR_NUMBER --merge
    - Branch is automatically deleted
    - PR is automatically closed

33. Card moves to "Done" automatically (if PR references the card)

34. Issue is closed automatically (via "Closes #N" in PR body)
```

## GitHub Project Status Flow

```
Todo → In Progress → In Review → Done
          ↑            ↑           ↑
      (start)     (PR created)  (merged)
```

- **Todo**: Task is planned but not started
- **In Progress**: Task is being implemented
- **In Review**: PR created, awaiting review
- **Done**: PR merged (after merge)

## Review Loop Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    REVIEW ITERATION                          │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   Step 17: Reviewer adds comments                            │
│           ↓                                                  │
│   Step 18: Agent fetches ALL comments                        │
│           ↓                                                  │
│   Step 19: Agent responds to each comment                    │
│           ↓                                                  │
│   Step 20-22: Implementation if needed                       │
│           ↓                                                  │
│   Step 23: Agent pushes changes →──────────┐                 │
│           ↓                                │                 │
│   Step 24: User reviews                    │                 │
│           ↓                                │                 │
│   ┌─────────────────────┐                 │                 │
│   │ Need more changes?  │──── Yes ────────┘                 │
│   └─────────────────────┘                                   │
│           ↓ No (all resolved)                                │
│                                                              │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│                    MANUAL TESTING                            │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   Step 25-26: Reviewer tests application                     │
│           ↓                                                  │
│   ┌─────────────────────┐                                    │
│   │ Bugs found?         │──── Yes ──→ Document bugs in PR   │
│   └─────────────────────┘                 ↓                 │
│           ↓ No                            Agent fixes        │
│           ↓                                ↓                 │
│           ↓                         Agent pushes ────────────┐│
│           ↓                                           ↓     ││
│           ↓                               Return to Step 19 ─┘│
│           ↓ No bugs                                               │
├─────────────────────────────────────────────────────────────┤
│                          ↓                                    │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│                         MERGE                                │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   Step 29: User authorizes merge                             │
│           ↓                                                  │
│   Step 30: Agent runs: gh pr merge N --merge                 │
│           ↓                                                  │
│   Step 31-32: Cleanup (branch deleted, PR closed)            │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

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

### Scenario: Implementation Changes Needed

1. Agent creates todo list overview of required changes
2. User confirms or modifies the plan
3. Agent implements changes
4. Agent updates documentation (CHANGELOG, IMPLEMENTATION.md, etc.)
5. Agent pushes changes
6. Return to checking for unresolved comments

### Scenario: Scope Creep Detected

1. Agent identifies scope creep during implementation
2. Discusses with user
3. May open separate issues for additional work
4. Defers to future PRs with user agreement

### Scenario: YAGNI (You Ain't Gonna Need It)

1. Agent identifies unnecessary code during review
2. Explains why code should be removed
3. Removes code with user agreement
4. Documents removal in commit message

### Scenario: Large Issue Detected

1. Agent realizes issue is too large for single PR
2. Discusses with user
3. May split into multiple issues/PRs
4. Documents remaining work in IMPLEMENTATION.md

### Scenario: Bugs Found During Manual Testing

1. Reviewer documents bugs in PR comments during testing
2. Agent creates todo list of fixes needed
3. User confirms the fixes are appropriate
4. Agent implements fixes
5. Agent updates PR body to document bugs fixed
6. Agent pushes changes
7. Return to Step 19 (review iteration)

### Scenario: Scope Creep in PR

Sometimes additional work is needed within a PR that wasn't in the original scope,
but is appropriate to include (e.g., defining guidelines during implementation).

1. Agent identifies out-of-scope work needed
2. Discusses with user and gets approval
3. Agent implements the additional work
4. Agent documents ALL work in PR body:
   - Original scope work
   - Additional out-of-scope work (clearly marked)
5. Agent updates CHANGELOG/IMPLEMENTATION.md as needed
6. Continue with normal review process

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

## Quick Reference

```bash
# Create branch
git checkout -b feat/my-feature

# Create draft PR
gh pr create --draft --title "feat: my feature" --body "..."

# Move card to In Review (requires project item ID)
gh api graphql -f query='
mutation {
  updateProjectV2ItemFieldValue(
    input: {
      projectId: "PVT_kwHOADplIc4BRnZ9"
      itemId: "ITEM_ID"
      fieldId: "PVTSSF_lAHOADplIc4BRnZ9zg_ZGpg"
      value: { singleSelectOptionId: "77520bb7" }
    }
  ) { projectV2Item { id } }
}'

# Mark PR ready for review
gh pr ready PR_NUMBER

# DO NOT: Close issue, move to Done, merge without approval
```

## Project Information

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