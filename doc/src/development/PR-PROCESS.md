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
5. **ALWAYS add new issues to roadmap** - When creating new issues, add them to IMPLEMENTATION.md with priority label

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

9. Create PR as DRAFT with issue reference:
   gh pr create --draft --title "<type>: <description>" --body "..."
    
   IMPORTANT: Include "Closes #N" or "Related #N" in PR body to:
   - Link PR to Issue automatically
   - Auto-close Issue when PR is merged (Closes #N)

10. Link PR to Issue (Development field):
    gh issue comment <issue_number> --body "PR #<pr_number> criado para resolver esta issue."
    
   This creates a visible link in the Issue's "Development" section.

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

### Phase 2.5: Planning Mode (AFTER Authorization)

**After user authorizes continuation, enter planning mode:**

```
12. Analyze codebase to understand the implementation context
    - Read relevant files (READ-ONLY, no modifications)
    - Identify patterns and conventions
    - Check for existing abstractions to reuse

13. Create implementation plan:
    - Specific files to modify/create
    - Function signatures
    - Estimated line counts
    - Complexity reduction targets

14. Ask user questions to clarify approach:
    - File location preferences
    - Priority order
    - Any constraints or preferences

15. WAIT for user approval of plan

    DO NOT make any file modifications during planning mode.
    The plan is discussed and approved before implementation begins.
    
    User will then authorize implementation.
    
16. After user approves plan:
    a. Update IMPLEMENTATION.md with detailed plan
    b. Update PR body with implementation plan
    c. Update TODO list with implementation tasks
    d. Commit documentation changes
    e. Push changes
    f. Proceed to Phase 3 (Implementation)
```

**Why this step exists:**
- Ensures implementation follows agreed architecture
- Catches issues before code is written
- Creates clear record of what will be changed
- Allows user to course-correct the plan
- **READ-ONLY during planning** - no file modifications until approved

### Phase 3: Implementation (AFTER Plan Approval)

**Only start Phase 3 after plan is approved and documentation updated.**

```
17. Implement tasks from TODO list in order

18. Run tests: cargo test --all-features

19. Run clippy: cargo clippy --all-features -- -D warnings

20. Commit code changes (conventional commits):
    feat: <description>
    fix: <description>
    refactor: <description>

21. Push commits: git push
```

### Phase 4: Mark PR Ready for Review

```
22. Mark PR as "ready for review":
    gh pr ready <number>

23. Move GitHub Project card to "In Review" (both Status and Scrum Status)

24. Add comment to issue: "PR #N ready for review"
```

### Phase 5: Review & Iteration (COLLABORATIVE)

This phase repeats until all review comments are resolved.

```
25. Reviewer adds comments to PR

26. Agent fetches ALL unresolved review comments:
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

27. Agent responds to EACH unresolved comment individually:
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

28. If implementation changes needed:
    a. Create a todo list overview of changes
    b. Wait for user confirmation before implementing
    c. Implement approved changes
    d. Update documentation as needed
    e. Push changes

29. If scope creep detected:
    - Discuss with user
    - May need to open separate issues

30. Update PR description and documentation if changes were made

31. Agent checks for unresolved comments again:
    - If unresolved comments exist → return to step 27
    - If all resolved → inform user and wait for approval

32. User reviews and either:
    - Approves and proceeds to Phase 6 (manual testing)
    - Adds more comments → return to step 27
```

### Phase 6: Manual Testing (REVIEWER)

After all review comments are resolved, the reviewer performs manual testing.

```
33. Reviewer marks all review comments as resolved

34. Reviewer tests the application manually:
    - Build and run: cargo build --all-features && cargo run
    - Test the specific feature/fix
    - Verify edge cases
    - Check for regressions

35. If bugs found during testing:
    a. Reviewer documents bugs in PR comments
    b. Agent creates todo list of fixes
    c. User confirms fixes
    d. Agent implements fixes
    e. Agent documents bugs fixed in PR body
    f. Agent pushes changes
    g. **Return to Step 27 (review iteration)** - new commits need review

36. If testing passes:
    - Proceed to Phase 7 (merge)

**IMPORTANT:** Every time the agent pushes commits (whether fixing review comments, 
bugs from testing, or any other changes), the PR returns to Step 27 for a new 
review iteration. The reviewer must review the new commits and either:
- Add more comments → continue iteration
- Approve → proceed to Phase 7

**CRITICAL:** The reviewer can only review after the agent has pushed ALL commits.
Before informing the reviewer that changes are ready, the agent MUST ensure:
- All commits are pushed to the remote branch
- `git status` shows "up to date with 'origin/<branch>'"
- No local commits remain unpushed
```

### Phase 6.5: Smoke Test (OPTIONAL, HERMES AGENT)

After code review approval, the user may request a smoke test execution. This is
optional and typically requested for significant features or before releases.

```
37. User requests smoke test from Hermes Agent:
    "Execute smoke test on this PR"

38. Hermes Agent executes SMOKE_TEST.md:
    - Preserves user's existing database (backup)
    - Creates temporary test files
    - Runs automated checklist (build, unit tests)
    - Executes manual test sections interactively
    - Reports all test results with checkmarks
    - Notes any failures with detailed error messages

39. If smoke test passes:
    - Hermes reports "Aprovado para merge"
    - Proceed to Phase 7 (Merge)

40. If smoke test fails:
    - Hermes documents failures in PR comments
    - Agent creates todo list of fixes
    - User confirms fixes
    - Agent implements fixes
    - Agent pushes changes
    - **Return to Step 27 (review iteration)**
```

**Smoke Test Principles:**

1. **Database Isolation**
   - MUST backup user's existing database before testing
   - MUST use temporary database for tests
   - MUST restore user's database after testing

2. **Test Coverage**
   - Binary execution and version
   - Chat mode basic functionality
   - Document import (including bug fixes: tilde, ID formats, titles)
   - Memory and facts
   - Notes (regression test)
   - Query mode
   - File tools (regression test)
   - Database schema verification

3. **Bug Fix Verification**
   - Each bug fix must have explicit test case
   - Bug #1 (tilde expansion): Test `/doc import ~/path`
   - Bug #2 (ID formats): Test `/doc show #N`, `doc:N`, `N`
   - Bug #3 (org title): Test `#+TITLE:` extraction

4. **Regression Testing**
   - Notes still work after changes
   - File tools still work after changes
   - Memory/search still work after changes

**Manual Tests (Require Interaction):**
- Document import via chat
- Embedding synchronous verification
- Memory/facts via chat
- Notes via chat
- File tools via LLM

**Automated Tests (Script):**
- Build verification
- Unit tests
- Binary version check

### Phase 6.5.5: Manual Test (HERMES AGENT)

After smoke test passes, the Hermes Agent may execute manual tests for specific bug fixes
or features that cannot be tested automatically.

**IMPORTANT:** Manual test files (e.g., `MANUAL-TEST-<PR_NUMBER>.md`) are **NOT versioned**.
They should be:
- Created locally by the Hermes Agent during testing
- Stored outside the repository (e.g., `~/` or `/tmp`)
- Deleted after the PR is merged
- NEVER committed to git

```
40.5. Hermes Agent creates manual test file LOCALLY (not in repo):
     - File: ~/MANUAL-TEST-PR_NUMBER.md (or /tmp/MANUAL-TEST-PR_NUMBER.md)
     - Based on template: doc/src/development/MANUAL-TEST-TEMPLATE.md
     - Customized for specific bug fixes in the PR
     - Uses temporary files for test data
     - Results documented in the file itself
     - NEVER add to git

40.6. Hermes Agent executes manual test:
     - Creates temporary test files
     - Interacts with LLM to test tool behavior
     - Verifies error messages are correct
     - Checks unit consistency (MB/Mb, KB/Kb)
     - Reports all test results with checkmarks
     - Notes any failures with detailed error messages

40.7. After manual test completes:
     - Hermes reports results in PR comments (not in file)
     - Manual test file is deleted or kept locally (not committed)
     - If tests pass, proceed to Phase 7 (Merge)
```

**Manual Test Template:**

Located at `doc/src/development/MANUAL-TEST-TEMPLATE.md`

Each manual test includes:
1. Objective and expected behavior
2. Test setup (file creation, commands)
3. Step-by-step verification checklist
4. Cleanup instructions
5. Result documentation

**Manual Test Principles:**

1. **Bug Fix Verification**
   - Each bug fix gets its own test section
   - Test verifies the fix, not just the feature

2. **Error Message Quality**
   - Check for vague errors (e.g., "Some(1)")
   - Verify actionable suggestions are present
   - Confirm unit consistency (MB vs Mb, KB vs Kb)

3. **Tool Behavior**
   - Test synchronous operations work immediately
   - Verify parameters are passed correctly
   - Check that limits are enforced

### Phase 7: Merge (AGENT, after authorization)

```
45. User authorizes merge (all comments resolved, testing passed, smoke test passed if requested)

46. Agent merges PR using regular merge (NOT squash) with branch deletion:
    gh pr merge PR_NUMBER --merge --delete-branch
    
    IMPORTANT: 
    - Use --merge (NOT --squash) to preserve commit history
    - Use --delete-branch to clean up after merge
    - Branch is deleted after merge
    - PR is automatically closed

47. Card moves to "Done" automatically (if PR references the card)

48. Issue is closed automatically (via "Closes #N" in PR body)
```

## GitHub Project Status Flow

```
Todo → In Progress → In Review → Done
           ↑            ↑           ↑
       (start)     (PR ready)    (merged)
```

- **Todo**: Task is planned but not started
- **In Progress**: Task is being implemented
- **In Review**: PR ready for review (not draft)
- **Done**: PR merged (after merge)

## Review Loop Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    REVIEW ITERATION                          │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   Step 25: Reviewer adds comments                            │
│           ↓                                                  │
│   Step 26: Agent fetches ALL comments                        │
│           ↓                                                  │
│   Step 27: Agent responds to each comment                    │
│           ↓                                                  │
│   Step 28-30: Implementation if needed                       │
│           ↓                                                  │
│   Step 31: Agent pushes changes →──────────┐                 │
│           ↓                                │                 │
│   Step 32: User reviews                    │                 │
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
│   Step 33-34: Reviewer tests application                     │
│           ↓                                                  │
│   ┌─────────────────────┐                                    │
│   │ Bugs found?         │──── Yes ──→ Document bugs in PR   │
│   └─────────────────────┘                 ↓                 │
│           ↓                            Agent fixes          │
│           ↓                                ↓                 │
│           ↓                         Agent pushes ────────────┐│
│           ↓                                           ↓     ││
│           ↓                               Return to Step 27 ─┘│
│           ↓ No bugs                                         │
├─────────────────────────────────────────────────────────────┤
│                          ↓                                   │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│                    SMOKE TEST (OPTIONAL)                     │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   Step 37: User requests smoke test (optional)               │
│           ↓                                                  │
│   Step 38-40: Hermes executes SMOKE_TEST.md                │
│           ↓                                                  │
│   ┌─────────────────────┐                                    │
│   │ Smoke test fails?   │──── Yes ──→ Document in PR       │
│   └─────────────────────┘                 ↓                 │
│           ↓                            Agent fixes          │
│           ↓                                ↓                 │
│           ↓                         Agent pushes ────────────┐│
│           ↓                                           ↓     ││
│           ↓                               Return to Step 27 ─┘│
│           ↓ Pass (or skipped)                               │
├─────────────────────────────────────────────────────────────┤
│                          ↓                                   │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│                         MERGE                                │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   Step 45: User authorizes merge                             │
│           ↓                                                  │
│   Step 46: Agent runs: gh pr merge N --merge --delete-branch│
│           ↓                                                  │
│   Step 47-48: Cleanup (branch deleted, PR closed)           │
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