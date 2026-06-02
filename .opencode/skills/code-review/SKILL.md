---
name: code-review
description: Perform and respond to code reviews on GitHub PRs. Covers creating inline reviews with multiple comments, responding to review threads, resolving threads, and project-specific review patterns for Sprachspiel. Load this skill whenever reviewing a PR or responding to review comments.
license: MIT
compatibility: opencode
metadata:
  audience: maintainers
  workflow: code-review
---

## What I do

I guide the complete code review workflow for Sprachspiel PRs — both creating reviews (as a reviewer) and responding to reviews (as an author). I cover the exact GitHub API patterns, project-specific review checks, and the critical anti-patterns to avoid.

## When to use me

- When asked to review a PR or code changes
- When responding to review comments on your PR
- When the PR-PROCESS.md or pr-workflow skill reaches Phase 5 (Review & Iteration)

**Load me alongside `pr-workflow` during Phase 5.** The pr-workflow skill handles the overall flow; this skill handles the review interactions.

---

# Part 1: Creating a Code Review

## ⛔ CRITICAL: One Review, Multiple Comments

When submitting a code review, **ALWAYS submit ALL inline comments in a single `POST /repos/:owner/:repo/pulls/:PR_NUMBER/reviews` call with a `comments` array.**

```bash
HEAD_SHA=$(gh pr view PR_NUMBER --json headRefOid --jq '.headRefOid')

curl -s -X POST \
  -H "Authorization: token $TOKEN" \
  -H "Content-Type: application/json" \
  "https://api.github.com/repos/luksamuk/sprachspiel/pulls/PR_NUMBER/reviews" \
  -d '{
    "commit_id": "'$HEAD_SHA'",
    "event": "COMMENT",
    "body": "## Code Review Summary\n\n2 issues, 1 suggestion. See inline comments.",
    "comments": [
      {
        "path": "src/chat/app.rs",
        "line": 358,
        "body": "🔴 **Bug:** Ctrl+D on non-empty buffer returns None instead of deleting.\n\nSuggestion: implement `InputState::delete_char_right()` or delegate to `CrosstermInput`."
      },
      {
        "path": "src/chat/view/ratatui_view.rs",
        "line": 456,
        "body": "⚠️ **Warning:** ANSI codes composed then immediately stripped.\n\n`add_system_message()` calls `strip_ansi_codes()`. Rewrite as plain text."
      },
      {
        "path": "src/chat/app.rs",
        "line": 97,
        "body": "💡 **Suggestion:** Replace magic number `30000` with `u16::MAX`."
      }
    ]
  }'
```

### ❌ NEVER Do This

**NEVER create one review per comment.** This produces empty review bodies and scatters the conversation:

```bash
# ❌ BAD — Creates 7 separate reviews with empty bodies
for each_comment in comments:
    curl -X POST .../reviews -d '{"event": "COMMENT", "body": "", "comments": [each_comment]}'
    # This produces 7 reviews with 1 comment each — NOISY
```

**NEVER create standalone comments for review feedback.** Inline comments are part of reviews, not separate entities.

### ✅ ALWAYS Do This

```bash
# ✅ GOOD — One review, N comments
curl -X POST .../reviews -d '{
    "event": "COMMENT",
    "body": "## Review Summary\n\nFound 3 issues. See inline comments.",
    "comments": [comment1, comment2, comment3]  # ALL comments in one call
}'
```

## Review Comment Severity Prefixes

| Prefix | Icon | When to use | Blocks merge? |
|--------|------|-------------|---------------|
| Critical | 🔴 | Security vulnerabilities, data loss, crashes | Yes |
| Warning | ⚠️ | Bugs in non-critical paths, missing error handling | Usually |
| Suggestion | 💡 | Style improvements, refactoring ideas | No |
| Looks Good | ✅ | Clean patterns, good test coverage | N/A |

## Review Event Types

| Event | When to use |
|-------|-----------|
| `COMMENT` | Most reviews — observations and suggestions |
| `REQUEST_CHANGES` | Any critical or warning item exists |
| `APPROVE` | Zero critical/warning items, only suggestions or all clear |

## Review Checklist for Sprachspiel

### Always Check

1. **`#[allow(dead_code)]` without justification** — Every `#[allow(dead_code)]` MUST have a comment on the same line explaining why. Prefer `#[cfg(test)]` for test-only code. Run `rg '#\[allow\(dead_code\)\]' --glob '*.rs' src/ | grep -v '// '`
2. **Hardcoded role strings** — `"user"`, `"assistant"`, `"system"`, `"tool"` should use constants from `src/consts/roles.rs`
3. **YAGNI** — No code "for future TUI implementation" or "will be used in PR3" without a justification comment referencing the PR
4. **ANSI escape codes outside view module** — `command_handlers.rs` should have ZERO `\x1B[` escape sequences. All styling belongs in `TerminalView` or `RatatuiView`
5. **`unwrap()` and `panic!()` in production code** — Acceptable in tests only
6. **`AppResult<T>`** — Should be `Result<T, Box<dyn std::error::Error + Sync + Send>>`

### TUI/Ratatui-Specific Checks (Post-PR2)

7. **`add_system_message()` strips ANSI** — In `RatatuiView`, any method that composes ANSI codes via `colors::*` then passes through `add_system_message()` is an anti-pattern. The TUI renders as plain text via `Line::raw()`.
8. **Double cleanup guard** — If a type has both `restore(self)` (consuming) and `impl Drop`, it MUST have a `restored: bool` guard to prevent double-restore.
9. **Dual state synchronization** — `InputState` and `CrosstermInput` must not duplicate editable state (buffer, cursor) without a clear sync point. Flag as architectural risk.
10. **Magic numbers in UI** — Scroll offsets, terminal size constants, etc. should use typed constants or `u16::MAX` with comments.

### General Rust Checks

11. **Duplicate logic across modules** — If two structs have the same fields or two functions do similar things, flag it
12. **Error handling** — Silent error swallowing (empty `if let Ok(...)`, `let _ = ...`) should at minimum log or document why
13. **Public fields on structs** — `pub(crate)` fields expose internals; prefer accessor methods
14. **Doc comments** — Function renames should update ALL doc comments referencing the old name

---

# Part 2: Responding to Review Comments

## ⛔ CRITICAL: Reply to EXISTING Threads, NOT Create New Reviews

When responding to review comments on YOUR PR, **ALWAYS reply within the existing thread using `in_reply_to`.** **NEVER create new top-level PR comments or new reviews for responses.**

### The ONLY Correct Way to Reply

Use `gh api` with `in_reply_to` pointing to the original comment ID:

```bash
# ✅ CORRECT — Reply to existing thread (find ID first)
# Step 1: Find the comment ID you want to reply to
gh api repos/luksamuk/sprachspiel/pulls/PR_NUMBER/comments \
  --jq '.[] | "ID=\(.id) replyTo=\(.in_reply_to_id // "null") \(.body[:80])"'

# Step 2: Reply inline using the comment ID
gh api repos/luksamuk/sprachspiel/pulls/PR_NUMBER/comments \
  --method POST \
  --field body="✅ Resolvido em abc1234. Fixed the bug." \
  --field in_reply_to=ORIGINAL_COMMENT_ID
```

### ❌ FORBIDDEN Patterns

```bash
# ❌ NEVER — Creates a standalone PR comment (NOT an inline reply)
gh pr comment PR_NUMBER --body "✅ Resolvido..."

# ❌ NEVER — Creates a new review with empty body
curl -X POST .../reviews -d '{"event": "COMMENT", "body": "", "comments": [response]}'

# ❌ NEVER — Creates a new review summary comment
gh pr comment PR_NUMBER --body "## Review Responses\n### Point 1\n..."
```

**Why this matters:** `gh pr comment` creates a **top-level PR comment** that is NOT attached to any review thread. It appears as a separate conversation, disconnected from the original review point. Reviewers must then search the PR timeline to find your response. The `in_reply_to` field threads your response directly beneath the original comment, keeping the conversation in context.

### How to Find the Original Comment ID

```bash
# List all comments on the PR
curl -s -H "Authorization: token $TOKEN" \
  "https://api.github.com/repos/luksamuk/sprachspiel/pulls/PR_NUMBER/comments?per_page=50" | \
  python3 -c "
import json, sys
for c in json.load(sys.stdin):
    print(f'ID={c[\"id\"]} | {c[\"path\"]}:{c.get(\"line\", \"?\")} | {c[\"body\"][:60]}')
"
```

### Response Prefixes

| Prefix | Meaning | When to Use |
|--------|---------|-------------|
| ✅ Resolvido | Code fixed/removed | Changed code to address the comment |
| ✅ Verificado | Correct as-is | Confirmed the code behavior is intentional |
| 📋 | Deferred | Good suggestion, will address in future PR |
| ❌ | Declined | Not applicable, with explanation |
| ❓ | Clarification | Question about the comment |

### ❌ NEVER Create a Single Summary Comment

````markdown
# ❌ BAD — Wall-of-text summary as PR comment (NOT threaded)
gh pr comment 195 --body "## Review Responses
### Point 1
Fixed in abc123.
### Point 2
Not YAGNI, removed #[allow(dead_code)].
### Point 3
..."

This creates a TOP-LEVEL comment on the PR. It is NOT threaded under
the original review comment. The reviewer must scroll through the entire
PR timeline to find it and match it to the original discussion.

Instead, reply to each thread individually using `in_reply_to` as shown above.
````

---

# Part 3: Resolving Review Threads

After all comments in a thread have been addressed (either fixed or marked as deferred/declined with explanation), resolve the thread.

## Using GraphQL resolveReviewThread

```bash
# First, list all threads to find thread IDs
curl -s -X POST -H "Authorization: token $TOKEN" -H "Content-Type: application/json" \
  "https://api.github.com/graphql" \
  -d '{"query": "{ repository(owner: \"luksamuk\", name: \"sprachspiel\") { pullRequest(number: PR_NUMBER) { reviewThreads(first: 50) { nodes { id isResolved } } } } }"}'

# Resolve a thread
curl -s -X POST -H "Authorization: token $TOKEN" -H "Content-Type: application/json" \
  "https://api.github.com/graphql" \
  -d '{"query": "mutation { resolveReviewThread(input: {threadId: \"PRRT_xxx\"}) { thread { isResolved } } }"}'
```

## When to Resolve

- **✅ Resolvido** → Resolve immediately after pushing fix
- **✅ Verificado** → Resolve after confirming behavior is intentional
- **📋 Deferred** → Resolve only if documented in code comments and IMPLEMENTATION.md. Add a reply noting where it's documented.
- **❌ Declined** → Resolve after explaining why

Do NOT leave threads unresolved after addressing them. Resolved threads signal review progress.

---

# Part 4: Project-Specific Review Patterns

These patterns are specific to the Sprachspiel codebase and should be checked during reviews.

## ANSI Compose-Then-Strip Anti-Pattern

In `RatatuiView`, `add_system_message()` calls `strip_ansi_codes()` before creating `ChatMessage::system()`. Any method that composes ANSI strings (using `colors::BOLD`, `colors::CYAN`, etc.) then passes through `add_system_message()` is wasteful and misleading.

**Detection:** `grep 'use colors::' src/chat/view/ratatui_view.rs` — should return nothing.

**Fix:** Rewrite as plain text directly. The `TerminalView` (non-TUI) maintains its own ANSI version.

## Double Drop/Restore Guard

When a type has both `restore(self)` (consuming) and `impl Drop`, add a `restored: bool` guard. Without it, any refactoring from `self` to `&mut self` would cause double-restore.

```rust
// ✅ Good
pub fn restore(mut self) {
    self.restored = true;
    // cleanup...
}

impl Drop for MyType {
    fn drop(&mut self) {
        if !self.restored {
            // cleanup...
        }
    }
}
```

## Key Handling Consistency

When two code paths handle the same key (e.g., `App::handle_key` and `CrosstermInput::handle_key_event`), they MUST behave consistently. Ctrl+D, Ctrl+C, arrow keys, etc. should have the same semantics in both paths.

## Hardcoded UI Strings

Strings like `"Type /help for commands, /quit to exit"`, `"Thinking..."`, `"Running tool..."` should be constants or centralized, not scattered across multiple files.

## Scope Creep Detection

When a PR includes files unrelated to its stated purpose, flag it. The fix may be valid, but note it as a separate concern.

---

# Part 5: Auth Setup for Review API

When `gh auth status` fails in sandboxed environments:

```bash
# Get token via PTY
TOKEN=$(gh auth token)

# Use token directly with curl
curl -s -H "Authorization: token $TOKEN" \
  "https://api.github.com/repos/luksamuk/sprachspiel/pulls/PR_NUMBER/reviews"
```

Always use `gh auth token` via PTY for sandbox auth. Direct `gh api` calls may fail if keyring auth is unavailable.