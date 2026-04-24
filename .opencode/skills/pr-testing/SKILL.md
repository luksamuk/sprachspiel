---
name: pr-testing
description: Create and manage PR testing. The primary agent creates the manual test script, then the programmer manually invokes the Hermes Agent (a separate program) to execute both manual tests and smoke tests. The primary agent processes results and handles bugs found.
license: MIT
compatibility: opencode
metadata:
  audience: maintainers
  workflow: testing
---

## What I do

I guide the PR testing workflow from the primary agent's perspective. I create task-specific manual test scripts, then **the programmer manually invokes the Hermes Agent** to execute them. After execution, I process results and handle any bugs found.

## Critical Understanding: What Is the Hermes Agent?

**The Hermes Agent is a SEPARATE PROGRAM** with its own skills, routines, and system access. It is NOT a sub-agent or a tool that the primary agent (OpenCode) can invoke programmatically.

Key facts:
- **Hermes Agent runs as a separate process** — it has its own CLI, its own session, its own tools
- **The programmer invokes it manually** — the primary agent CANNOT call it, schedule it, or simulate it
- **It has independent system access** — it can interact with the application, filesystem, and network
- **Testing via Hermes Agent is MANDATORY** — a PR cannot be merged without manual + smoke test results

**The primary agent (OpenCode) NEVER executes tests.** It creates scripts and processes results. The programmer launches the Hermes Agent separately.

## When to use me

Use this skill when the PR has been approved in review (Phase 5 complete) and it's time for testing (Phase 6 in the PR workflow).

**Project board state during testing:** The card should already be in "In Review" (moved in Phase 4 of the PR workflow). Do NOT move it back or forward during testing — it stays "In Review" until Phase 7 (merge).

## Overview: Who Does What

| Step | Who | Action |
|------|-----|--------|
| Create manual test script | **Primary agent** (OpenCode) | Write `~/MANUAL-TEST-PR_NUMBER.md` |
| Approve test script | **User** | Review and confirm |
| Invoke Hermes Agent | **Programmer (manually)** | Launches Hermes Agent as a separate program |
| Execute manual tests | **Hermes Agent** | Runs the test script interactively |
| Review SMOKE_TEST.md | **Primary agent** (OpenCode) | Check if updates needed |
| Invoke Hermes Agent again | **Programmer (manually)** | Launches Hermes Agent for smoke tests |
| Execute smoke test | **Hermes Agent** | Runs SMOKE_TEST.md |
| Process results | **Primary agent** (OpenCode) | Read reports, fix bugs |
| Delete test files | **Primary agent** (OpenCode) | Clean up after merge |

## Step 1: Create Manual Test Script

**The primary agent creates the test script.** The Hermes Agent only executes it — it does not decide what to test.

Create `~/MANUAL-TEST-PR_NUMBER.md` (NOT in the repository, NEVER committed to git):

```bash
# Create the file
# File: ~/MANUAL-TEST-PR_NUMBER.md (or /tmp/MANUAL-TEST-PR_NUMBER.md)
```

Base the script on the template at `doc/src/development/MANUAL-TEST-TEMPLATE.md`.

### Required Test Sections

1. **Feature functionality (happy path)**
   - Test the specific change this PR introduces
   - Verify the main use case works

2. **Edge cases**
   - Unicode input, empty input, boundary values
   - Very long input, special characters
   - Missing or null parameters

3. **Error handling**
   - Invalid input, wrong types
   - Missing dependencies
   - Check error messages are clear and actionable (not vague like "Some(1)")
   - Verify unit consistency (MB vs Mb, KB vs Kb)

4. **Regression tests**
   - Existing features still work
   - Related features not broken by this change

5. **Tool behavior** (if PR touches tools)
   - Synchronous operations work immediately
   - Parameters passed correctly
   - Limits enforced
   - `log_tool_call` and `log_tool_result` present

### Group by Issue Number (for Multi-Issue PRs)

```markdown
## 1. Issue #70: Memory Staleness Warnings
...

## 2. Issue #71: Truncation Warnings
...
```

## Step 2: Wait for User Approval

Present the test script to the user. They may:
- Suggest additions or modifications
- Approve as-is
- Request focus on specific areas

**DO NOT proceed until user confirms.**

## Step 3: Programmer Invokes Hermes Agent for Manual Tests

### MANDATORY Step — No Exceptions

**The programmer must manually invoke the Hermes Agent to execute the manual tests.** This is not optional and cannot be skipped.

The primary agent (OpenCode):
1. Presents the approved test script path to the programmer
2. Asks the programmer to launch the Hermes Agent
3. **WAITS** — does nothing until the programmer provides results

The programmer:
1. Launches the Hermes Agent separately (it is an independent program)
2. The Hermes Agent reads `~/MANUAL-TEST-PR_NUMBER.md`
3. The Hermes Agent executes the test sections interactively
4. The Hermes Agent reports results back

### What the Hermes Agent Does

The Hermes Agent:
- Reads `~/MANUAL-TEST-PR_NUMBER.md`
- Creates temporary test files
- Interacts with the application to test tool behavior
- Verifies error messages
- Reports all test results with checkmarks
- Notes failures with detailed error messages
- Reports results (typically as a file or PR comment)

### ⛔ STOP AND WAIT

**You (OpenCode) must wait for the programmer to provide test results.** Do NOT:
- Simulate test execution yourself
- Attempt to call the Hermes Agent programmatically
- Skip the testing phase
- Assume tests passed without explicit results

The results may come:
- As a report file (e.g., `~/MANUAL-TEST-RESULTS-PR_NUMBER.md`)
- As PR comments from Hermes
- As the programmer pasting results directly
- Combined with smoke test results in a single report

## Step 4: Process Manual Test Results

### If Tests Pass

Proceed to Step 5 (review SMOKE_TEST.md).

### If Tests Find Bugs

1. **Read the failure report** from Hermes
2. **Create a todo list** of fixes needed
3. **Get user confirmation** for the fixes
4. **Implement fixes**
5. **Push changes**
6. **Return to review iteration** (PR-PROCESS.md Step 27) — new commits need review
7. After review, the programmer must invoke Hermes Agent again to re-run tests

## Step 5: Review and Update SMOKE_TEST.md

**Key distinction:** The smoke test is a **generalized regression suite** versioned in `SMOKE_TEST.md`. It is NOT task-specific.

Review `SMOKE_TEST.md` and check if the PR adds features that need minimum regression guarantees.

### When to Add Smoke Test Sections

**Add sections when:**
- New user-visible features
- New tools (must verify tool calls work)
- New CLI commands
- New error messages that need verification
- Bug fixes that could regress

**Do NOT add sections when:**
- Internal refactors
- Documentation-only changes
- Test-only changes
- Features already covered by existing sections

### If Updates Needed

```bash
# Add sections to SMOKE_TEST.md
git add SMOKE_TEST.md
git commit -m "test: add smoke test sections for <feature>"
git push
```

**Wait for user confirmation before proceeding to smoke test.**

## Step 6: Programmer Invokes Hermes Agent for Smoke Test

### MANDATORY Step — No Exceptions

**The programmer must manually invoke the Hermes Agent again to execute the smoke test.** This is not optional.

The primary agent (OpenCode):
1. Confirms SMOKE_TEST.md is updated (if needed)
2. Asks the programmer to launch the Hermes Agent for smoke testing
3. **WAITS** — does nothing until the programmer provides results

### What the Hermes Agent Does

The Hermes Agent:
- Preserves user's existing database (backup)
- Creates temporary database for tests
- Runs automated checklist (build, unit tests)
- Executes manual test sections from SMOKE_TEST.md interactively
- Reports results with checkmarks
- Restores user's database after testing
- Writes report to `~/SMOKE-TEST-RESULTS-PR_NUMBER.md`

### ⛔ STOP AND WAIT

**You (OpenCode) must wait for the programmer to provide smoke test results.** Do NOT skip or simulate.

### If Smoke Test Passes

Hermes reports "Aprovado para merge" in results. You read the report and **proceed to Phase 7 (merge).**

### If Smoke Test Fails

Same flow as manual test bugs:
1. Read failure report
2. Create todo list of fixes
3. Get user confirmation
4. Implement fixes, push
5. Return to review iteration (Step 27)
6. Programmer must invoke Hermes Agent again to re-run

## Step 7: Cleanup (AFTER Merge)

After the PR is merged:
- Delete `~/MANUAL-TEST-PR_NUMBER.md`
- Delete `~/MANUAL-TEST-RESULTS-PR_NUMBER.md` (if exists)
- Delete `~/SMOKE-TEST-RESULTS-PR_NUMBER.md` (if exists)

These files are temporary and should NOT remain after merge.

**Also verify:**
- The project board card moved to "Done" (automatic via "Closes #N", verify manually)
- Any duplicate issues are closed with cross-reference comments
- IMPLEMENTATION.md is updated to `✅ COMPLETED`

## Manual Test Principles

1. **Task-Specific Testing** — Each feature/bug gets its own test section
2. **Error Message Quality** — Check for vague errors, verify actionable suggestions
3. **Tool Behavior** — Test synchronous operations, parameter passing, limits
4. **Database Isolation** — Hermes MUST backup/restore user's database
5. **Bug Verification** — Each bug fix must have explicit test case
6. **Mandatory Execution** — Tests are NEVER optional. Every PR must be tested by the Hermes Agent before merge.
7. **Separate Program** — The Hermes Agent is invoked manually by the programmer, NOT by OpenCode.

## Summary: What OpenCode Does NOT Do

| Action | Who Does It | Why |
|--------|-------------|-----|
| Execute manual tests | Hermes Agent (invoked by programmer) | Independent verification |
| Execute smoke tests | Hermes Agent (invoked by programmer) | Independent verification |
| Simulate test results | NOBODY | Real execution required |
| Skip testing phase | NOBODY | Testing is mandatory |
| Call Hermes Agent programmatically | NOBODY | It's a separate program |