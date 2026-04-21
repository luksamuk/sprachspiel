# Smoke Test Manual - ask-ai

Run these tests before each release to ensure core functionality works.

**See also:** [PR Process - Phase 6.5: Smoke Test](doc/src/development/PR-PROCESS.md)

---

## Terminal-Use (Optional)

This smoke test is designed to be run by an AI agent using the **terminal-use** tool, which allows automated terminal control.

**Installation:** [github.com/flipbit03/terminal-use](https://github.com/flipbit03/terminal-use)

**Required configuration:** Use a terminal with **80 columns width** to ensure consistent output formatting.

---

## Prerequisites

```bash
cd /home/alchemist/git/ask-ollama-rs
cargo build --release --features all-tools
ollama serve  # In another terminal

# Backup user's existing database
cp ~/.local/share/ask-ai/embeddings.db ~/.local/share/ask-ai/embeddings.db.smoke-backup 2>/dev/null || true

# Use temporary database for tests (isolation)
rm -f ~/.local/share/ask-ai/embeddings.db
```

## Test Model

```bash
# Use environment variable or default
MODEL=${SMOKE_MODEL:-qwen3.5:4b}
ollama list | grep -q "$MODEL" || ollama pull "$MODEL"
echo "Test model: $MODEL"
```

---

## LLM Tool Refusal Policy

Some tests require the LLM to call tools (sections 4.1, 4.2, 5, 6, 6.5.5, 10, 10.5, 15.2, 17.1). If the model **refuses** to call a tool:

1. **Retry with an explicit instruction** — rephrase the prompt to make the tool call unavoidable (e.g., "You MUST call the todo_add tool right now, do not just describe it").
2. **If the model still refuses persistently** — **FAIL the test** and note which model refused which tool.
3. **Switch to an abliterated model** — check `models.toml` for an abliterated variant (e.g., `qwen3.5:4b-abliterated`). If no abliterated model is listed, request one from the user before retrying.

**Rationale:** Abliterated models have safety refusals removed, ensuring deterministic tool calling during smoke tests. A model that refuses to call tools is a valid test failure, not a bug in the application.

---

## 1. Basic Binary

- [ ] Binary runs: `./target/release/ask-ai --help`
- [ ] Version visible: `./target/release/ask-ai --version`
- [ ] Subcommands listed (chat, query, translate)

---

## 2. Chat Mode

- [ ] Starts without errors
- [ ] Shows loaded model
- [ ] `/help` shows available commands (including /doc and /todo)
- [ ] `/exit` quits correctly

---

## 3. Document Import (Critical Feature)

**Prepare test files:**
```bash
echo "txt import test" > /tmp/test.txt
echo "# Markdown Title\n\nContent here." > /tmp/test.md
echo "#+TITLE: Org Title\n\n* Heading\nContent." > /tmp/test.org
touch /tmp/empty.txt
# File for tilde expansion test (Bug #1)
echo "tilde expansion test" > ~/test.txt
```

### 3.1 Basic Tests

- [ ] `/doc import /tmp/test.txt` works (absolute path)
- [ ] `/doc import ~/test.txt` works (path with ~) ← **Bug #1 fixed**
- [ ] `/doc list` shows the document
- [ ] `/doc show 1` works (format N) ← **Bug #2 fixed**
- [ ] `/doc show #1` works (format #N) ← **Bug #2 fixed**
- [ ] `/doc show doc:1` works (format doc:N) ← **Bug #2 fixed**
- [ ] `/doc delete 1` removes correctly

### 3.2 Format Tests

- [ ] `/doc import /tmp/test.md` - Markdown imported
- [ ] MD title extracted from `# heading` (verify with `/doc show`)
- [ ] `/doc import /tmp/test.org` - Org-mode imported
- [ ] ORG title extracted from `#+TITLE:` (not from * heading) ← **Bug #3 fixed**

### 3.3 Error Tests

- [ ] `/doc import /nonexistent.txt` → "File not found"
- [ ] `/doc show 999` → "not found"
- [ ] `/doc import /tmp/empty.txt` → rejected (empty file)

### 3.4 Size Tests (Bug #54)

- [ ] File > 2.5 MB (2,500,000 bytes) is rejected with clear error:
  ```bash
  # Create large file (3 MB = 3,000,000 bytes)
  dd if=/dev/zero bs=1M count=3 of=/tmp/large.txt 2>/dev/null
  # Verify that /doc import rejects it
  /doc import /tmp/3mb.txt
  ```
- [ ] Error message mentions "2.5 MB (2,500,000 bytes) limit" and suggests splitting the file

---

## 4. Synchronous Embedding (New Feature)

- [ ] After `/doc import /tmp/test.txt`, search immediately:
  ```
  Use remember to search for "test"
  ```
- [ ] Result includes the recently imported document (synchronous indexing works)

### 4.1 import_document Tool (via LLM) - Bug #54

**Prepare test files:**
```bash
echo "test import via tool" > /tmp/tool_test.txt
```

Via chat with a model that supports tools:

> **LLM Refusal:** If the model refuses to call `import_document`, rephrase the prompt
> more explicitly. If refusal persists, switch to an abliterated model (see LLM Tool
> Refusal Policy above) and retest.

- [ ] `import_document("/tmp/tool_test.txt", None, Some("Test Document"))` works
- [ ] Import returns "Chunks: N (document indexed and ready for search)"
- [ ] `remember(query="test")` finds the document
- [ ] Tool returns correct title when provided

### 4.2 Large Document Protection (Bug #54)

**Prepare large document without chunks:**
```bash
# Create large document in database manually (simulating broken import)
# Then verify that remember protects against full content return
```

- [ ] `remember(id="doc:N")` on document > 50 KB (50,000 bytes) without chunks returns error
- [ ] Message explains how to re-import the document
- [ ] Suggests `/doc delete N` and re-import

---

## 5. Memory (remember/facts)

**Note:** Use a model that supports tools (qwen3.5:4b or larger). Small models like 0.8b may struggle with tool calling.

> **LLM Refusal:** If the model refuses to call `remember`, rephrase explicitly.
> If refusal persists, switch to an abliterated model (see LLM Tool Refusal Policy above).

- [ ] "Remember that I like coffee" creates a note/fact
- [ ] "What do I like?" returns "coffee"
- [ ] Facts persist between sessions (quit and re-enter)

---

## 6. Notes (Regression)

- [ ] "Remember this is a test note" creates a note
- [ ] `/note list` shows notes
- [ ] `/note show 1` displays note
- [ ] `/note delete 1` removes note

---

## 6.5. Todo Tools (New Feature - Issue #66)

**Note:** Tests via slash commands in chat. Does not require a model with tools.

### 6.5.1 Basic Tests (CRUD)

- [ ] `/todo add "Buy groceries --priority high --tags shopping,food"` creates task with priority and tags
- [ ] `/todo add "Write documentation"` creates task with default priority (medium)
- [ ] `/todo list` shows all tasks
- [ ] `/todo list pending` filters by status
- [ ] `/todo list high` filters by priority
- [ ] `/todo list #shopping` filters by tag

### 6.5.2 Get/Edit/Delete Tests

- [ ] `/todo get 1` shows task details (description, priority, tags, status)
- [ ] `/todo edit 1 --priority critical` updates priority
- [ ] `/todo edit 1 --tags urgent,shopping` updates tags
- [ ] `/todo edit 1 Updated description` updates description
- [ ] `/todo update 1 in_progress` changes status
- [ ] `/todo delete 2` removes a specific task

### 6.5.3 Error Tests

- [ ] `/todo get 999` → "not found" or similar message
- [ ] `/todo get` (no ID) → shows usage
- [ ] `/todo edit` (no ID) → shows usage
- [ ] `/todo delete` (no ID) → shows usage

### 6.5.4 Cleanup Tests

- [ ] `/todo update 1 done` marks as completed
- [ ] `/todo clear-done` removes completed tasks
- [ ] `/todo clear-all` removes all tasks

### 6.5.6 Todo Shortcut Aliases (PR #84)

**Verify 2-letter shortcut commands for todo subcommands work correctly.**

- [ ] `/ta Buy shortcut test` → creates task (same as `/todo add`)
- [ ] `/tl` → lists tasks (same as `/todo list`)
- [ ] `/tu 1 done` → updates task status (same as `/todo update`)
- [ ] `/tg 1` → shows task details (same as `/todo get`)
- [ ] `/te 1 Updated via shortcut` → edits task (same as `/todo edit`)
- [ ] `/td 1` → deletes task (same as `/todo delete`)
- [ ] `/tcd` → clears completed tasks (same as `/todo clear-done`)
- [ ] `/tca` → clears all tasks (same as `/todo clear-all`)

### 6.5.5 Todo Tools via LLM (requires model with tools)

Via chat with a model that supports tools:

> **LLM Refusal:** If the model refuses to call a todo tool, rephrase explicitly
> (e.g., "Call the todo_add tool now"). If refusal persists, switch to an
> abliterated model (see LLM Tool Refusal Policy above).

- [ ] "Add a todo task to review code with high priority" → LLM calls `todo_add`
- [ ] "List my todo tasks" → LLM calls `todo_list`
- [ ] "Mark task 1 as in progress" → LLM calls `todo_update`
- [ ] "Get details of task 1" → LLM calls `todo_get`
- [ ] "Change priority of task 1 to critical" → LLM calls `todo_edit`

---

## 6.6. Command Shortcuts and Safety (PR #84, PR #87)

**Verify shortcut behavior and destructive command safety.**

### 6.6.1 /f Maps to /search (not /forget)

- [ ] `/f test query` → executes search (NOT forget) ← **Bug fix: /f was mapped to /forget**
- [ ] `/forget` → shows warning (requires --yes) ← **Issue #85: /forget confirmation**
- [ ] `/forget --yes` → executes forget, no `FOREIGN KEY constraint` warning ← **Bug fix: save_sqlite FK**

### 6.6.2 Todo After /forget — No FK Warning

**Bug fix:** `save_sqlite()` now calls `ensure_conversation_exists()` before FK-dependent INSERTs.

- [ ] `/forget --yes` → new session ID generated
- [ ] `/todo add FK test` → adds todo without `FOREIGN KEY constraint failed` warning
- [ ] `/todo list` → shows the task, no FK warning

### 6.6.3 Skill Activation via /skill (Issue #86)

- [ ] `/skill` → lists available skills
- [ ] `/skill document-processing` → activates the skill
- [ ] `/document-processing` → "Unknown command" (wildcard removed)

---

## 7. Query Mode

**Note:** Query mode loads full context (AGENTS.md, SOUL.md, tools). For quick testing, use `--soulless --ignore-agents` or increase timeout.

```bash
# Quick test (no heavy context) - global flags BEFORE subcommand
timeout 60 ./target/release/ask-ai --soulless --ignore-agents query "2+2"

# Full test (with context)
timeout 120 ./target/release/ask-ai query "What is 2+2?"
```

- [ ] Returns answer without errors
- [ ] Exit code 0

---

## 8. Translation (optional)

```bash
./target/release/ask-ai translate pt "Hello"
```

- [ ] Returns translation (if model available)

---

## 9. Database

```bash
sqlite3 ~/.local/share/ask-ai/embeddings.db ".tables"
sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA user_version;"
```

- [ ] Tables exist (content, facts, conversations, session_todos, etc.)
- [ ] Schema version correct (10 or higher)

**Explicit verification:**
```bash
SCHEMA_VER=$(sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA user_version;")
[ "$SCHEMA_VER" -ge 10 ] && echo "✓ schema v$SCHEMA_VER" || echo "✗ schema v$SCHEMA_VER < 10"
```

**Verify priority/tags columns in session_todos (v9):**
```bash
sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA table_info(session_todos);"
# Must include columns: priority (TEXT) and tags (TEXT)
```

**Verify v10 additions:**
```bash
# Verify feedback_signals table (v10)
sqlite3 ~/.local/share/ask-ai/embeddings.db ".tables" | grep -q "feedback_signals" && echo "✓ feedback_signals table" || echo "✗ feedback_signals table missing"
# Verify pruned column in content_items (v10)
sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA table_info(content_items);" | grep -q "pruned" && echo "✓ pruned column" || echo "✗ pruned column missing"
```

---

## 10. File Tools (Regression)

**Prepare test files:**
```bash
echo "test content" > /tmp/file_test.txt
# File for tilde expansion test (Bug #1 related)
echo "file tools test" > ~/file_test.txt
```

Via chat with a model that supports tools:

- [ ] `read_file(path="/tmp/file_test.txt")` works
- [ ] `read_file(path="~/file_test.txt")` works (with ~) ← **Bug #1 related**
- [ ] `list_directory(path="~")` works
- [ ] `write_file(path="/tmp/write_test.txt", content="test")` works

---

## 10.5. run_command Error Messages (Bug #54)

Via chat with a model that supports tools:

- [ ] `run_command("pdftotext /nonexistent.pdf -")` returns useful error
- [ ] Message mentions "file does not exist" or similar
- [ ] Message does NOT contain generic "Some(1)"

---

## 11. Memory Staleness Warnings (Issue #70)

**Verify that facts in the system prompt show staleness labels when outdated.**

This test requires inserting facts with different ages into the database and checking
the system prompt injection. Since staleness is based on `decay_score`, `last_accessed`,
`access_count`, and `created_at`, we test via the application's behavior.

### 11.1 Fresh Facts Show No Label

- [ ] Start a new chat session
- [ ] Tell the LLM: "Remember that I prefer dark mode"
- [ ] Verify the fact is stored (via `/fact list` or by asking "What do I prefer?")
- [ ] Fresh facts should appear **without** any staleness label (no `(stale)`, `(N days ago)`, `(unused)`)

### 11.2 Staleness Labels Appear in Prompt (Regression)

> **Note:** Full staleness testing requires database manipulation to set `decay_score`,
> `last_accessed`, `access_count`, and `created_at` to old values. This is not easily
> testable via smoke test without direct DB access.

- [ ] Verify that the `get_staleness_label()` function exists in `src/facts/prompt.rs` (code review)
- [ ] Verify that `build_facts_section()` calls `get_staleness_label()` for each fact (code review)

---

## 12. Truncation Warnings in Tool Outputs (Issue #71)

**Verify that tool outputs include `[TRUNCATED]` notices when content is limited.**

### 12.1 read_file with max_lines

**Prepare test file:**
```bash
# Create a file with 20 lines
for i in $(seq 1 20); do echo "Line $i: This is test content for truncation warnings"; done > /tmp/truncation_test.txt
```

Via chat with a model that supports tools:

- [ ] `read_file(path="/tmp/truncation_test.txt", max_lines="5")` returns only 5 lines
- [ ] Output includes `[TRUNCATED: Showing lines 1-5 of 20. Use read_file_segment to read more.]`
- [ ] `read_file(path="/tmp/truncation_test.txt")` (no max_lines) returns all 20 lines with NO truncation notice
- [ ] `read_file(path="/tmp/truncation_test.txt", max_lines="25")` returns all 20 lines with NO truncation notice (requested lines >= total)

### 12.2search_files Truncation

**Prepare test file:**
```bash
# Create a file with a pattern that appears many times
for i in $(seq 1 200); do echo "UNIQUEPATTERN line $i"; done > /tmp/search_truncation_test.txt
```

- [ ] `search_files(pattern="UNIQUEPATTERN", path="/tmp")` returns results
- [ ] If more than 100 matches, output includes `[TRUNCATED: Showing 100 matches. Refine your search pattern for fewer results.]`

### 12.3 remember Truncation (Notes/Documents)

Via chat with a model that supports tools:

- [ ] Create a long note: "Remember this is a very long note with lots of content that should exceed 150 characters when displayed in search results so that the truncation warning appears in the output"
- [ ] Search for it: `remember(query="long note")`
- [ ] If the content exceeds 150 chars, output includes `[TRUNCATED: 150 of N chars. Use remember(id="note:X") for full content.]`

### 12.4 Unicode Safety (Regression)

- [ ] Create a file with multibyte characters: `echo "Linha 1: Olá mundo 日本語テスト 🎉" > /tmp/unicode_test.txt`
- [ ] `read_file(path="/tmp/unicode_test.txt", max_lines="1")` returns first line without garbled characters
- [ ] Truncation notice is properly formatted (not garbled)

### 12.5 Cleanup

```bash
rm -f /tmp/truncation_test.txt /tmp/search_truncation_test.txt /tmp/unicode_test.txt
```

---

## Cleanup

```bash
# Restore user's database
rm -f ~/.local/share/ask-ai/embeddings.db
mv ~/.local/share/ask-ai/embeddings.db.smoke-backup ~/.local/share/ask-ai/embeddings.db 2>/dev/null || true

# Clean up test files (/tmp and ~)
rm -f /tmp/test.txt /tmp/test.md /tmp/test.org /tmp/empty.txt
rm -f /tmp/file_test.txt /tmp/write_test.txt
rm -f /tmp/tool_test.txt /tmp/large.txt
rm -f ~/test.txt ~/file_test.txt
```

**Note:** All todos created during testing are stored in the session database.
Since we use a temporary database (isolated), they will be discarded when restoring the original database.

---

## 13. Basic Performance

```bash
# Acceptable time for simple query (no context)
# Note: global flags BEFORE subcommand
time (timeout 30 ./target/release/ask-ai --soulless --ignore-agents query "2+2" > /dev/null)
# Should complete in < 15 seconds on normal hardware
```

- [ ] Simple query completes in reasonable time (< 15s)

---


## 14. Subagent Chat Commands

These commands are always available in chat mode (not feature-gated).

### 14.1 /ocr command
- [ ] Start chat: `ask-ai chat`
- [ ] Type `/ocr` with no args → shows usage hint
- [ ] Type `/ocr .env` → "BLOCKED" error (security blocklist)

### 14.2 /vision command
- [ ] Type `/vision` with no args → shows usage hint

### 14.3 /translate command
- [ ] Type `/translate en:pt Hello world` → returns Portuguese translation

### 14.4 /summarize command
- [ ] Type `/summarize Long text about artificial intelligence` → returns concise summary

### 14.5 Feature flag
- [ ] Build without subagent-tools: `cargo build --release --no-default-features --features "weather-tools,file-tools"` → chat commands `/ocr`, `/vision`, `/translate`, `/summarize` still work, but LLM cannot call `spawn_subagent`

---

## 15. Feedback Commands (New Feature - Issue #23)

Test the feedback command infrastructure for recording user feedback on assistant messages.

### 15.1 Basic Feedback Commands

- [ ] Start chat: `ask-ai chat`
- [ ] Type a message and receive an assistant response
- [ ] Type `/feedback good` → `↑↑ good feedback recorded for msg:N` + excerpt (dim) + `Importance: +0.05`
- [ ] Type `/feedback bad` → `↓↓ bad feedback recorded for msg:N` + excerpt (dim) + `Importance: -0.10`
- [ ] Type `/feedback correction:actually, the sky is blue` → `✎ correction feedback recorded for msg:N` + excerpt (dim) + `Correction: actually, the sky is blue`
- [ ] Type `/feedback msg:1 good` → `↑↑ good feedback recorded for msg:1` + excerpt (dim) + `Importance: +0.05`
- [ ] Type `/feedback msg:2 bad` → `↓↓ bad feedback recorded for msg:2` + excerpt (dim) + `Importance: -0.10`
- [ ] Type `/feedback msg:3 correction:fixed text` → `✎ correction feedback recorded for msg:3` + excerpt (dim) + `Correction: fixed text`

### 15.2 Feedback via LLM

**Note:** Use a model that supports tools (qwen3.5:4b or larger).

> **LLM Refusal:** If the model refuses to call `feedback_submit`, rephrase the prompt
> to be more direct, e.g., "Call the feedback_submit tool with subcommand good."

- [ ] Ask the LLM: "Give me good feedback on your last response" → LLM calls `feedback_submit` with `good` → `↑↑ good feedback recorded for msg:N`
- [ ] Ask the LLM: "Give me bad feedback on your last response" → LLM calls `feedback_submit` with `bad` → `↓↓ bad feedback recorded for msg:N`
- [ ] Ask the LLM: "Give correction feedback: actually, it's 42" → LLM calls `feedback_submit` with `correction:actually, it's 42` → `✎ correction feedback recorded for msg:N`

### 15.3 Error Tests

- [ ] Type `/feedback` with no subcommand → `Usage: /feedback <good|bad|correction:text> [msg:id]`
- [ ] Type `/feedback msg:abc good` → `Invalid message ID 'abc'. Use msg:<number> (e.g., msg:42).`
- [ ] Type `/feedback correction:` → `Correction requires text. Usage: /feedback correction:<text>`
- [ ] Type `/feedback msg:5 correction:` → `Correction requires text. Usage: /feedback msg:<id> correction:<text>`
- [ ] Start anonymous chat: `ask-ai chat --anonymous`
- [ ] Type `/feedback good` in anonymous mode → `Error: Cannot give feedback in anonymous mode.`
- [ ] Type `/feedback good` before any assistant message → `No assistant message to give feedback on.`

### 15.4 Shortcut Tests

- [ ] Type `/fb good` → `↑↑ good feedback recorded for msg:N` + excerpt (dim) + `Importance: +0.05`
- [ ] Type `/fb bad` → `↓↓ bad feedback recorded for msg:N` + excerpt (dim) + `Importance: -0.10`
- [ ] Type `/fb correction:typo fix` → `✎ correction feedback recorded for msg:N` + excerpt (dim) + `Correction: typo fix`
- [ ] Type `/fg` → `↑↑ good feedback recorded for msg:N` + excerpt (dim) + `Importance: +0.05`

---

## 16. Content Prune & Context Decay Stats (New Feature - Issue #23)

Test the content decay and pruning infrastructure.

### 16.1 Content Prune

- [ ] Start chat: `ask-ai chat`
- [ ] Import a document first: `/doc import /tmp/test.txt`
- [ ] Type `/content prune` → shows `⏳ Running content decay cycle...` then result
- [ ] After prune with items removed: `✓ Pruned N content item(s), N remaining (avg retention: X.XX).`
- [ ] After prune with no items removed: `✓ No content to prune. N item(s) remaining (avg retention: X.XX).`
- [ ] Type `/cp` → same behavior as `/content prune` (shortcut works)

### 16.2 Context Decay Stats

- [ ] Type `/context` → shows `Content Memory:` section with:
  - `Total items: N`
  - `Avg importance: X.XX`
  - If items at risk: `⚠ Items at risk: N (low decay score)`
  - `Feedback signals: N`
- [ ] After `/content prune`, `/context` shows updated stats

### 16.3 Error Tests

- [ ] Start anonymous chat: `ask-ai chat --anonymous`
- [ ] Type `/content prune` in anonymous mode → `Error: Cannot prune content in anonymous mode.`
- [ ] Type `/cp` in anonymous mode → `Error: Cannot prune content in anonymous mode.`
- [ ] Start chat without database (if possible): `/content prune` without DB → `Error: Database not initialized. Run chat without --anonymous.`

---

## 17. Feedback Tool & Configuration (New Feature - Issue #23)

Test the feedback_submit LLM tool and configuration defaults.

### 17.1 feedback_submit LLM Tool

**Note:** Use a model that supports tools (qwen3.5:4b or larger).

> **LLM Refusal:** If the model refuses to call `feedback_submit`, rephrase the prompt
> to be more direct, e.g., "Call the feedback_submit tool with item_id='42' and signal_type='good'."
> If refusal persists, switch to an abliterated model (see LLM Tool Refusal Policy above).

- [ ] `feedback_submit("42", "good", None)` → `Feedback submitted: good signal for item 42 (weight: 30%)`
- [ ] `feedback_submit("15", "bad", None)` → success message with negative importance adjustment (`Importance adjustment: -0.10`)
- [ ] `feedback_submit("7", "correction", Some("The capital is Canberra"))` → success message with `Correction: The capital is Canberra`
- [ ] Verify response includes `weight: 30%` (default llm_feedback_weight=0.3)
- [ ] `feedback_submit("0", "good", None)` → `Error: item_id must be a positive integer.`
- [ ] `feedback_submit("42", "invalid", None)` → `Error: Unknown feedback signal type: 'invalid'. Expected: good, bad, or correction. Use 'good', 'bad', or 'correction'.`
- [ ] `feedback_submit("42", "correction", None)` → `Error: correction_text is required when signal_type is 'correction'.`

### 17.2 Configuration Verification

Verify `[feedback]` section in config.toml (or defaults work without it):

```bash
# Check schema version (must be 10 or higher)
sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA user_version;"
# Expected: 10 or higher

# Check feedback_signals table exists
sqlite3 ~/.local/share/ask-ai/embeddings.db ".tables"
# Expected: includes feedback_signals

# Check pruned column in content_items
sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA table_info(content_items);"
# Expected: includes pruned column (INTEGER NOT NULL DEFAULT 0)
```

- [ ] Schema version is 10 or higher
- [ ] `feedback_signals` table exists in database
- [ ] `pruned` column exists in `content_items` table

**Default configuration values (in `[feedback]` section of config.toml or built-in defaults):**

| Setting | Default |
|---------|----------|
| `enabled` | `true` |
| `implicit_capture` | `true` |
| `llm_feedback_weight` | `0.3` |
| `decay_half_life_good` | `30.0` |
| `decay_half_life_bad` | `7.0` |
| `decay_half_life_correction` | `14.0` |
| `content_decay` | `true` |
| `access_reinforcement` | `true` |
| `access_reinforcement_boost` | `0.001` |
| `content_prune_threshold` | `0.05` |

- [ ] All 9 default values are correct when `[feedback]` section is omitted from config.toml
- [ ] Adding `[feedback]` section to config.toml overrides defaults correctly

## Results

**IMPORTANT:** Smoke test results must be saved **outside the project** (e.g., PR comment, issue, or external document). **DO NOT MODIFY THIS FILE** with results — it is a reusable template.

**Date:** _______  
**Version:** _______  
**Model used:** _______  
**Status:** [ ] Approved for merge

**Issues found:**

_______________________________________

---

## Quick Checklist (Automated)

Run in sequence:

```bash
#!/bin/bash
set -e

echo "=== Automated Smoke Test ==="

# 1. Backup database
echo "Backing up database..."
cp ~/.local/share/ask-ai/embeddings.db ~/.local/share/ask-ai/embeddings.db.smoke-backup 2>/dev/null || true

# 2. Build
echo "Building..."
cargo build --release --features all-tools || { echo "✗ Build failed"; exit 1; }
echo "✓ Build"

# 3. Quick checks
./target/release/ask-ai --help | grep -q "chat" && echo "✓ chat command"
./target/release/ask-ai --version && echo "✓ version"

# 4. Unit tests
echo "Unit tests..."
cargo test --lib 2>&1 | tail -5
echo "✓ Unit tests"

# 5. Restore
mv ~/.local/share/ask-ai/embeddings.db.smoke-backup ~/.local/share/ask-ai/embeddings.db 2>/dev/null || true

echo ""
echo "=== Automated Smoke Test Complete ==="
echo "Run remaining manual tests per SMOKE_TEST.md"
```

---

## Remaining Manual Tests

The script above runs automated tests. The following tests must be run manually:

1. **Section 3**: Document Import (interactive chat tests)
2. **Section 4**: Synchronous Embedding (verify immediate indexing)
3. **Section 5**: Memory (interactive tests with model >= 4b)
4. **Section 6**: Notes (interactive tests)
5. **Section 6.5**: Todo Tools (CRUD, priority, tags, filters)
6. **Section 6.6**: Command Shortcuts and Safety (/f, /forget, skills)
7. **Section 10**: File Tools (via LLM)
8. **Section 10.5**: run_command Error Messages
9. **Section 11**: Memory Staleness Warnings (code review + fresh fact check)
10. **Section 12**: Truncation Warnings in Tool Outputs (via LLM)
11. **Section 13**: Performance (verify response time)
12. **Section 15**: Feedback Commands (interactive feedback tests)
13. **Section 16**: Content Prune & Context Decay Stats (interactive tests)
14. **Section 17**: Feedback Tool & Configuration (via LLM + database verification)
These tests require chat interaction and visual verification of results.