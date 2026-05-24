---
name: manual-test-verification
description: Validate manual test scripts against the actual codebase before presenting them to the user. Prevents hallucinated commands, wrong UI strings, and references to nonexistent features. Apply this skill AFTER creating a draft manual test script and BEFORE presenting it to the user or the Hermes Agent.
license: MIT
compatibility: opencode
metadata:
  audience: maintainers
  workflow: testing
---

## What I Do

I verify that every command, UI string, and feature reference in a manual test script (`~/MANUAL-TEST-*.md`) actually exists in the codebase. This catches hallucinated slash commands, incorrect error messages, wrong flag syntax, and references to nonexistent features before tests are executed.

## When to Use

Load this skill immediately after drafting a `MANUAL-TEST-*.md` file, before presenting it to the user for approval. This is part of the PR testing workflow (Phase 6.2) described in `pr-testing`.

## Why This Exists

During PR #148, the manual test script contained several errors that were caught by manual review:

1. **`/anonymous`** — referenced as a slash command, but anonymous mode is only a CLI flag (`--anonymous`), not an in-chat command.
2. **`/subagent`** — referenced as a command, but no such slash command exists. The subagent tools (`/ocr`, `/vision`, `/translate`, `/summarize`) are separate commands.
3. **`"Compactando..."`** — hallucinated message in Portuguese. The actual message is `"Compacting context (N% full, NK remaining)..."` in English.
4. **`"Context window N% full"`** — incomplete. The actual UI string is `"⚠ Context N% full"`.

These errors would have caused confusion during test execution by the Hermes Agent and wasted time.

## Verification Checklist

For **each** test case in the manual test script, verify the following against the source code:

### 1. Slash Commands Exist

```bash
# Find all slash commands defined in the codebase
grep -n "pub enum ChatCommand" src/chat/commands.rs
grep -n '"command_name"' src/chat/commands.rs | head -50
```

For every `/command` referenced in the test script:
- Search `src/chat/commands.rs` for the exact command name
- If it doesn't exist as a `ChatCommand` variant, it's invalid
- Check if it's a CLI flag instead (e.g., `--anonymous` in `src/chat/cli.rs`)

### 2. UI Strings Match Source Code

For every quoted string in test assertions:
- `"Saving embeddings..."` → `grep -rn "Saving embeddings" src/`
- `"Compacting context"` → `grep -rn "Compacting context" src/`
- `"⚠ Context"` → `grep -rn "Context.*full" src/`
- Error messages → `grep -rn "ERR_LLM" src/consts/app.rs`

If the string doesn't appear in the source, it's hallucinated. Find the actual string.

### 3. CLI Flags and Arguments

For every `cargo run --` or `sprach` invocation:
- Check `src/chat/cli.rs` for valid CLI flags
- Check `src/main.rs` for top-level flags
- Verify default features vs `--features all-tools`

### 4. Feature References

For every feature described in the test:
- Verify the source file exists (e.g., `src/chat/compaction.rs`, `src/chat/event_loop.rs`)
- Verify function/struct names match (e.g., `CompactionContext` not `AutoCompactor`)
- Check that test-referenced behaviors actually exist in the code

### 5. Test Model Availability

For model references:
- Check `models.toml` or config for available models
- Default test model is usually `${SMOKE_MODEL:-qwen3.4:4b}` — verify this matches SMOKE_TEST.md

## Verification Procedure

1. **Draft the manual test script** using the template at `doc/src/development/MANUAL-TEST-TEMPLATE.md`
2. **Run the verification checklist** above for EVERY test case
3. **Fix all hallucinations** before presenting to the user
4. **Present the verified script** to the user for approval

## Common Hallucination Patterns

| Pattern | Example | Fix |
|---------|---------|-----|
| CLI flag as slash command | `/anonymous` | `cargo run -- --anonymous` |
| Internal tool name as command | `/subagent` | `/translate`, `/ocr`, `/vision`, `/summarize` |
| Translated UI string | `"Compactando..."` | `"Compacting context (N% full..."` |
| Inexact UI string | `"Context window X% full"` | `"⚠ Context X% full"` |
| Nonexistent error message | `"Ollama connection failed"` | `"LLM error: Connection refused"` |
| Wrong function/struct name | `AutoCompactor` | `CompactionContext` |

## Integration with PR-PROCESS.md

The PR process (Phase 6) already references `pr-testing` for test creation. This skill extends Phase 6.2 (manual test script creation) with a verification step:

```
Phase 6.2: Create manual test script
  → Use MANUAL-TEST-TEMPLATE.md
  → Apply manual-test-verification skill (this skill)
  → Present to user for approval
Phase 6.3: Hermes Agent executes manual tests
Phase 6.4: Review and update SMOKE_TEST.md
Phase 6.5: Hermes Agent executes smoke tests
```

## Example Verification

**Before verification (raw draft):**

> - [ ] Type `/anonymous` to enter anonymous mode
> - [ ] Verify: spinner shows "Compactando..."
> - [ ] Verify: error says "Ollama server error"

**After verification (corrected):**

> - [ ] Start `cargo run -- --anonymous` to enter anonymous mode
> - [ ] Verify: spinner shows "Compacting context (N% full, NK remaining)..."
> - [ ] Verify: error says "LLM error: ..." (not "Ollama")