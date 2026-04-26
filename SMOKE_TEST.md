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

### 4.3 Embedding Startup Resilience (Bug #40)

**Objective:** Verify that embedding failures during startup do not crash the application.

- [ ] With Ollama **running**: `./target/release/ask-ai chat` starts without panic
- [ ] With Ollama **stopped** (pkill ollama): `./target/release/ask-ai chat` starts without panic — graceful error messages only
- [ ] After restarting Ollama and re-entering chat, "Recovering N missing embedding(s)" message appears and completes successfully

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
- [ ] Schema version correct (11 or higher)

**Explicit verification:**
```bash
SCHEMA_VER=$(sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA user_version;")
[ "$SCHEMA_VER" -ge 11 ] && echo "✓ schema v$SCHEMA_VER" || echo "✗ schema v$SCHEMA_VER < 11"
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

**Verify v11 additions:**
```bash
# Verify has_embedding column in facts table (v11)
sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA table_info(facts);" | grep -q "has_embedding" && echo "✓ has_embedding column" || echo "✗ has_embedding column missing"
# Verify fact_embeddings vec0 table (v11)
sqlite3 ~/.local/share/ask-ai/embeddings.db ".tables" | grep -q "fact_embeddings" && echo "✓ fact_embeddings table" || echo "✗ fact_embeddings table missing"
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

---

## 18. Feedback Boost Integration & Decay Accuracy (PR #98 Refactoring)

Verify end-to-end feedback boost in retrieval and fractional-day decay accuracy (bug fix).

### 18.1 Feedback Boost Affects Search Ranking

- [ ] Start chat: `ask-ai chat`
- [ ] Have the LLM respond to two different questions (creates 2+ assistant messages)
- [ ] Submit positive feedback on message 1: `/feedback good`
- [ ] Submit negative feedback on message 2: `/feedback bad`
- [ ] Ask a broad question that could match both messages
- [ ] Verify message with positive feedback ranks higher in search results
- [ ] Verify database shows feedback signals with correct boost values:
  ```bash
  sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT item_id, signal_type, base_value, source FROM feedback_signals;"
  ```

### 18.2 Facts Prune Cycle

- [ ] Add fact: `/fact add "Test decay fact"`
- [ ] Run `/fact prune` → fresh fact NOT pruned
- [ ] Age a fact in DB: `sqlite3 ~/.local/share/ask-ai/embeddings.db "UPDATE facts SET last_accessed = strftime('%s','now','-365 days') WHERE id = (SELECT MAX(id) FROM facts);"`
- [ ] Run `/fact prune` → aged fact IS pruned
- [ ] Add preference with high importance (>=0.8) and age it → NOT pruned

### 18.3 Fractional-Day Decay Verification

Verify the `num_days()` truncation fix produces accurate values at non-boundary times.

- [ ] Insert Good signal at 30.5 days ago:
  ```bash
  SIGNAL_TS=$(sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT strftime('%s','now','-30.5 days');")
  ITEM_ID=$(sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT id FROM content_items WHERE content_type='message' ORDER BY id DESC LIMIT 1;")
  sqlite3 ~/.local/share/ask-ai/embeddings.db "INSERT INTO feedback_signals (item_id, session_id, signal_type, base_value, source, created_at) VALUES ($ITEM_ID, 'test', 'good', 1.0, 'user', $SIGNAL_TS);"
  ```
- [ ] Computed boost should be ≈ 0.484 (NOT 0.5 which truncated `num_days()` would give)
- [ ] Verify no hardcoded `2.0` for boost clamping — `MAX_FEEDBACK_BOOST` constant used everywhere:
  ```bash
  grep -r "clamp.*2\.0" src/db/feedback_ops.rs src/feedback/decay.rs
  # Should show MAX_FEEDBACK_BOOST, not hardcoded 2.0
  ```

### 18.4 Decay Formula Centralization

- [ ] `src/feedback/decay.rs` contains `decayed_weight_raw()` (canonical calculation)
- [ ] `src/db/feedback_ops.rs::compute_feedback_boost()` calls `decayed_weight_raw()`
- [ ] Both feedback and content decay use the same `2^(-days/half_life)` formula
- [ ] `HALF_LIFE_GOOD/BAD/CORRECTION` and `MAX_FEEDBACK_BOOST` are `pub(crate)` constants, not duplicated magic numbers

---

## 19. Fact & Content Prune Command Shortcuts (PR #98 Routing)

Verify consolidated command routing works after the refactoring.

### 19.1 /fact Shortcuts

- [ ] `/fp` → same as `/fact prune` (shortcut)
- [ ] `/fa "I prefer dark mode"` → same as `/fact add` (shortcut)

### 19.2 /content Shortcut

- [ ] `/cp` → same as `/content prune` (shortcut verified in 16.1, repeated for completeness)

---

## 20. Auto Fact Extraction (P6.1 — autoDream-lite)

Verify that preference and identity facts are auto-extracted from user messages and stored.

> **⚠️ Clean database recommended before starting this section.**  
> ```bash
> rm -f ~/.local/share/ask-ai/embeddings.db
> ```
> This ensures a clean state for dedup and embedding tests.

> **⚠️ Bug #2 (DEFERRED to issue #106):** PT noun translation after the prefix is NOT handled by heuristic mode. "Eu prefiro respostas curtas" → "User prefers respostas curtas" (noun "respostas curtas" remains in PT). Full noun translation requires LLM-mode (M2).

### 20.1 Auto-Extraction Happy Path (English)

- [ ] Start chat: `ask-ai chat`
- [ ] Send: "I prefer dark mode" → response includes `[Auto-extracted: N fact(s)]` notification (gray text)
- [ ] `/fact list` shows the extracted fact **"User prefers dark mode"** (NOT "I prefer dark mode" — ADR-E4 revised)

### 20.2 Multiple Preferences Per Message

- [ ] Send: "I like Python and I hate verbose errors" → extraction notification appears
- [ ] `/fact list` shows both extracted facts, both in **third person** ("User likes Python", "User hates verbose errors")

### 20.3 Deduplication

- [ ] Send: "I prefer dark mode" again → no new duplicate fact created
- [ ] `/fact list` shows only one "prefer dark mode" fact

### 20.4 Contradiction Resolution

- [ ] Send: "I prefer light mode" → extraction notification says 1 fact extracted/updated
- [ ] `/fact list` shows "User prefers light mode" (old "dark mode" fact removed)

### 20.5 No Extraction in Anonymous Mode

- [ ] Start: `ask-ai chat --anonymous`
- [ ] Send: "I prefer dark mode" → NO extraction notification appears
- [ ] Exit: `/exit`

### 20.6 Config: Disable Notification

- [ ] Edit `~/.config/ask-ai/config.toml`, add `[facts] auto_extract_notify = false`
- [ ] Start chat, send preference → fact is extracted but NO `[Auto-extracted]` notification
- [ ] Restore config

### 20.7 Config: Disable Auto-Extract

- [ ] Edit `~/.config/ask-ai/config.toml`, add `[facts] auto_extract = false`
- [ ] Start chat, send preference → NO extraction, NO notification
- [ ] Restore config

### 20.8 Third-Person Normalization (English — ADR-E4 Revised)

> **ADR-E4 revised:** All facts are now stored in third person ("User prefers X"), not just rendered in third person. `normalize_to_storage_format()` in `lang.rs` applies EN 1st→3rd person normalization at storage time. `normalize_to_third_person()` in `prompt.rs` remains as defense-in-depth for legacy data.

- [ ] Send: "My name is Lucas" → extraction notification
- [ ] `/fact list` → verify stored as **"User's name is Lucas"** (NOT "My name is Lucas")
- [ ] New session: ask "What are my preferences/identity?" → model references third-person form
- [ ] Send: "I prefer dark mode" → extraction notification
- [ ] `/fact list` → verify stored as **"User prefers dark mode"** (NOT "I prefer dark mode")

### 20.9 Portuguese Preference Extraction (ADR-L1: PT→EN Storage)

> **⚠️ Bug #2 (DEFERRED):** PT nouns after the prefix remain in original language. "Eu prefiro respostas curtas" → "User prefers respostas curtas" (noun preserved). This is expected behavior until LLM-mode.

- [ ] Send: "Eu prefiro respostas curtas" → extraction notification
- [ ] `/fact list` → stored as **"User prefers respostas curtas"** (EN prefix, PT noun — known limitation)
- [ ] Send: "Adoro Rust" → extraction notification
- [ ] `/fact list` → stored as **"User loves Rust"** (English, not "User adora")
- [ ] Send: "Não gosto de bugs" → extraction notification
- [ ] `/fact list` → stored as **"User doesn't like bugs"** (English)
- [ ] Verify: NO Portuguese prefixes in stored facts (no "User prefere", "User gosta", etc.)

### 20.10 Portuguese Identity Extraction

- [ ] Send: "Meu nome é Ana" → extraction notification
- [ ] `/fact list` → stored as **"User's name is Ana"** (EN, NOT "My name is Ana" — ADR-E4)
- [ ] Send: "Eu moro em Brasília" → extraction notification
- [ ] `/fact list` → stored as **"User lives in Brasília"** (EN, NOT "I live in Brasília")
- [ ] Send: "Eu trabalho no Google" → extraction notification
- [ ] `/fact list` → stored as **"User works at Google"** (EN, NOT "I work at Google")

### 20.11 Portuguese Exclusions (Commands & Fillers)

- [ ] Send: "Mostre os logs" → NO extraction (PT command)
- [ ] Send: "Busca o arquivo" → NO extraction (PT command)
- [ ] Send: "Beleza" → NO extraction (PT filler)
- [ ] Send: "Valeu" → NO extraction (PT filler)

### 20.12 fact_add LLM Tool: Content Validation (Bug #4/#5 fix)

- [ ] Ask LLM: "Remember that I like cats" → LLM calls `fact_add(content="I like cats")`, tool adds successfully
- [ ] Ask LLM: "Remember this: What time is it?" → LLM calls `fact_add(content="What time is it?")`, tool returns **Skipped: question**
- [ ] Ask LLM: "Remember: Thanks" → LLM calls `fact_add(content="Thanks")`, tool returns **Skipped: filler**
- [ ] Ask LLM: "Remember: Show me the logs" → LLM calls `fact_add(content="Show me the logs")`, tool returns **Skipped: command**
- [ ] Ask LLM: "Remember: hi" → LLM calls `fact_add(content="hi")`, tool returns **Skipped: too short** (min 10 chars)

### 20.13 fact_add LLM Tool: PT→EN Translation (Bug #2 retest)

> **⚠️ Bug #2 (DEFERRED):** PT nouns after the prefix remain in original language. "Lembre que prefere respostas curtas" → "User prefers respostas curtas".

- [ ] Ask LLM (in Portuguese): "Lembre que eu prefiro respostas curtas" → LLM calls `fact_add(content="Eu prefiro respostas curtas")`
- [ ] `/fact list` → stored as **"User prefers respostas curtas"** (EN prefix, PT noun — known limitation)
- [ ] Ask LLM: "Remember: adoro Rust" → `fact_add(content="adoro Rust")`
- [ ] `/fact list` → stored as **"User loves Rust"** (English)
- [ ] Verify: NO Portuguese-only prefixes in stored facts (no "Prefere", no "O nome do usuário")

### 20.14 Deduplication: Gap Fix & Cross-Scope (Bug #1 fix)

- [ ] Send: "I prefer dark mode" → extraction notification, stored as Global preference
- [ ] Ask LLM: "Remember that I prefer dark mode" → `fact_add` returns **Skipped: duplicate** (normalized match catches "User prefers dark mode")
- [ ] `/fact list` → only ONE "prefer dark mode" fact exists (no duplicates)
- [ ] If a Project-scope "prefer dark mode" exists, adding a Global-scope one should **replace** the Project one

### 20.15 /fact list: Scope Separation (Bug #3 fix)

- [ ] `/fact list` → shows **Global** and **Project** sections with headers
- [ ] Global facts listed under "Global Preferences" and "Global Facts"
- [ ] Project facts listed under "Project Preferences" and "Project Facts"
- [ ] `/fact list --global` → shows only Global facts
- [ ] `/fact list --project` → shows only Project facts

### 20.16 System Prompt: Scope Separation (user request)

- [ ] Store a Global preference: "I prefer dark mode"
- [ ] Store a Project fact: "The project uses Rust"
- [ ] New session → ask: "What do you know about me?"
- [ ] Model response should reference "User prefers dark mode" (Global)
- [ ] `/system` or check logs → system prompt has **"### Global Preferences"** and **"### Project Facts"** headers

### 20.17 Global Facts: project_id=None (Bug #1/6 fix)

- [ ] Send: "I prefer dark mode" → fact auto-extracted as Global
- [ ] Check database: `sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT id, content, scope, project_id FROM facts WHERE content LIKE '%dark mode%'"`
- [ ] Verify: `project_id` column is **NULL** for Global scope facts

### 20.18 Exact Content Dedup (Retest #1 fix)

- [ ] Send: "I prefer dark mode" → extraction notification
- [ ] Send: "I prefer dark mode" again (exact same text) → NO duplicate created
- [ ] Send: "i prefer dark mode" (lowercase) → NO duplicate (case-insensitive exact match)
- [ ] `/fact list` → only ONE dark mode fact

### 20.19 Normalized Content Dedup (Retest #1 fix)

- [ ] Send: "I prefer dark mode" → extraction notification
- [ ] Ask LLM: "Remember that I prefer dark mode" → fact_add returns **Skipped: Similar fact already exists** (normalized match catches "User prefers dark mode" ≈ "User prefers dark mode")
- [ ] `/fact list` → only ONE dark mode fact

### 20.20 Contradiction: Preference Override (Retest #3 fix)

- [ ] Send: "I prefer dark mode" → stored as preference
- [ ] Send: "I prefer light mode" → extraction should detect **contradiction** and **update** the existing fact
- [ ] `/fact list` → "User prefers light mode" replaces "User prefers dark mode" (NOT both present)

### 20.21 Third-Person PT Translation (Retest #2 fix — ADR-E4)

> **ADR-E4 revised:** Storage-time normalization. All facts stored as "User prefers X", never "I prefer X".

- [ ] Ask LLM (in Portuguese): "Lembre que prefere respostas curtas" → `fact_add(content="Prefere respostas curtas")`
- [ ] Verify: stored as **"User prefers respostas curtas"** (EN prefix, PT noun — known limitation)
- [ ] Ask LLM: "Remember that o nome do usuário é Ana" → stored as **"User's name is Ana"** (EN, NOT "My name is Ana")
- [ ] `/fact list` → verify ALL facts stored in third person ("User prefers...", "User's name is...", NOT "I prefer...")

### 20.22 Conflict Threshold (Retest #1 fix)

- [ ] This is indirectly tested by 20.14, 20.18, and 20.19
- [ ] Verify: `CONFLICT_THRESHOLD` is 0.75 (reduced from 0.85) in `src/facts/conflict.rs`

### 20.23 Deduplicate Extracted Threshold (Retest explosion fix)

- [ ] Send: "Eu moro em Brasília e meu nome é Ana e trabalho no Google" (3 identity facts)
- [ ] Verify: at most 3 facts extracted per message (respecting `max_facts = 3` limit)
- [ ] `/fact list` → no obvious duplicates from single message

---

## 21. Fact Embedding & Semantic Dedup (P6.7)

**Prerequisites:** Ollama must be running with the embedding model available.

> **⚠️ Clean database recommended before starting this section.**  
> ```bash
> rm -f ~/.local/share/ask-ai/embeddings.db
> ```
> This ensures a clean state for embedding and dedup tests.

### 21.1 Schema Migration: v10 → v11

- [ ] Start a fresh chat session → no errors
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA user_version;"` → returns **11**
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA table_info(facts);"` → includes **has_embedding** column (type INTEGER, default 0)
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db ".tables"` → includes **fact_embeddings** (vec0 virtual table)

### 21.2 Fact Insertion Generates Embedding (Serialized, 30s Timeout)

> **Bug #4 fix:** All embedding requests now go through `Semaphore(1)` and have a 30-second timeout. No more fire-and-forget `tokio::spawn`.

- [ ] Ask LLM: "Remember that I prefer concise output" (triggers `fact_add`)
- [ ] Wait 5 seconds for embedding generation (serialized, no concurrent overload)
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT id, has_embedding FROM facts WHERE content LIKE '%concise%'"` → **has_embedding = 1**
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT COUNT(*) FROM fact_embeddings"` → **≥ 1** row

### 21.3 Auto-Extraction Generates Embedding (Serialized)

- [ ] Send: "I prefer dark mode" → wait for `[Auto-extracted]` notification
- [ ] Wait 5 seconds for embedding generation
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT id, has_embedding FROM facts WHERE content LIKE '%dark mode%'"` → **has_embedding = 1**

### 21.4 Startup Recovery: Missing Embeddings

- [ ] Manually reset embedding flag: `sqlite3 ~/.local/share/ask-ai/embeddings.db "UPDATE facts SET has_embedding = 0"`
- [ ] Quit and restart chat → should see `Recovering N missing fact embedding(s)` in logs (or silent if no output)
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT COUNT(*) FROM facts WHERE has_embedding = 0 AND invalidated_at IS NULL"` → **0** (all recovered)
- [ ] Check logs for post-recovery verification: should warn if any facts still lack embeddings after recovery

### 21.5 Ollama Offline: Graceful Degradation

- [ ] Stop Ollama (`pkill ollama` or similar)
- [ ] Start chat with `ask-ai chat` → should NOT crash
- [ ] Ask LLM: "Remember that my favorite color is blue" → fact stored, `has_embedding = 0` (no crash)
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT id, has_embedding FROM facts WHERE content LIKE '%blue%'"` → **has_embedding = 0**
- [ ] Restart Ollama
- [ ] Quit and restart chat → recovery generates missing embeddings
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT COUNT(*) FROM facts WHERE has_embedding = 0 AND invalidated_at IS NULL"` → **0** (all recovered)

### 21.6 Semantic Contradiction Detection (Bug #3 fix — Layer 3.5)

> **Bug #3 fix:** When FTS5 doesn't find a conflict and the candidate is a preference, Layer 3.5 generates an embedding and searches `fact_embeddings` via `search_facts_semantic()` (cosine ≥ 0.90). Contradictions are resolved by replacing the old fact.

**Clean state first:**
```bash
# Remove all facts for clean test
sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

- [ ] Send: "I prefer dark mode" → stored as preference "User prefers dark mode"
- [ ] Wait 5 seconds for embedding
- [ ] Send: "I prefer light mode" → should UPDATE (not duplicate) the existing fact via semantic contradiction
- [ ] `/fact list` → shows "User prefers light mode" (NOT both "dark" and "light")
- [ ] Verify embedding: `sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT COUNT(*) FROM facts WHERE content LIKE '%light mode%' AND has_embedding = 1"` → **1**

### 21.7 Semantic Duplicate Detection: Paraphrase (Layer 3.5)

- [ ] Send: "I prefer dark mode" → stored as fact
- [ ] Wait 5 seconds for embedding
- [ ] Ask LLM: "Remember that I like using dark mode" → `fact_add` should return **Skipped: Similar fact already exists** or **duplicate** (FTS5 or Layer 3.5 catches it)
- [ ] `/fact list` → only ONE dark mode preference

### 21.8 Delete Fact Removes Embedding

- [ ] Note the ID of a fact with `has_embedding = 1`: `/fact list`
- [ ] `/fact remove <ID>` → removes fact
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT COUNT(*) FROM fact_embeddings WHERE fact_id = <ID>"` → **0** (embedding also removed)

### 21.9 Shutdown Flush

- [ ] Start chat, extract some facts
- [ ] Immediately `/exit` → should complete without error
- [ ] Restart → no "Recovering" message for facts (embeddings flushed on exit)
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT COUNT(*) FROM facts WHERE has_embedding = 0 AND invalidated_at IS NULL"` → **0**

### 21.10 Startup Semantic Dedup Verification

This test requires manually inserting two semantically similar facts (without embeddings):

```bash
# Insert two similar facts about the same preference
sqlite3 ~/.local/share/ask-ai/embeddings.db "INSERT INTO facts (scope, category, content, importance, decay_score, created_at, last_accessed, source, has_embedding) VALUES ('global', 'preference', 'I prefer dark mode', 0.5, 1.0, $(date +%s), $(date +%s), 'user', 0);"
sqlite3 ~/.local/share/ask-ai/embeddings.db "INSERT INTO facts (scope, category, content, importance, decay_score, created_at, last_accessed, source, has_embedding) VALUES ('global', 'preference', 'I like dark mode', 0.5, 1.0, $(date +%s), $(date +%s), 'user', 0);"
```

- [ ] Insert two similar facts (as above)
- [ ] `/fact list` → should show TWO similar facts initially
- [ ] Restart chat (triggers `verify_and_dedup_facts()`)
- [ ] `/fact list` → should show ONE fact (duplicate removed by semantic dedup)
- [ ] Optionally check logs for "Fact verification: removed 1 duplicates"

### 21.11 Embedding Serialization: No Concurrent Overload (Bug #4)

> **Bug #4 fix:** `EmbeddingClient` now serializes all embedding requests through `Semaphore(1)` with a 30-second timeout. Previously, concurrent `tokio::spawn` fire-and-forget tasks could overwhelm Ollama.

- [ ] **Rapid-fire test:** Send 5+ preference messages in quick succession:
  ```
  "I prefer dark mode"
  "I like Python"
  "I hate verbose errors"
  "I want short responses"
  "I love Rust"
  ```
- [ ] NO crash or panic during rapid insertion
- [ ] After 10 seconds, all 5 facts should have embeddings:
  ```bash
  sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT COUNT(*) FROM facts WHERE has_embedding = 1 AND invalidated_at IS NULL"
  ```
  Should be **≥ 5** (more if previous facts exist)
- [ ] Check for timeout errors in logs (should be none or very rare under normal conditions)

### 21.12 Post-Recovery Verification Warning (Bug #4)

- [ ] Stop Ollama (`pkill ollama`)
- [ ] Start chat: `ask-ai chat`
- [ ] Ask: "Remember that my favorite color is purple" → stored with `has_embedding = 0`
- [ ] `/exit`
- [ ] Restart Ollama
- [ ] Start chat: `ask-ai chat`
- [ ] If embedding recovery succeeds for all facts, no warning should appear
- [ ] If some facts remain without embeddings after recovery, a `log::warn!` message should appear (visible with `-v` verbose mode)

### 21.13 Regression — Existing Fact Features Still Work

- [ ] `/fact add` via LLM tool → works as before
- [ ] `/fact list` → shows facts correctly with scope headers
- [ ] `/fact search <query>` → returns matching facts
- [ ] `/fact remove <id>` → removes fact and its embedding
- [ ] Auto-extraction still works and generates embeddings
- [ ] Preference override contradiction still works ("prefer X" → "prefer Y" replaces)
- [ ] Global-wins-project rule still works

### 21.14 `/fact add` CLI: Full Dedup Parity (Bug #3 smoke test #2)

> **Bug #3 fix (smoke test #2):** `/fact add` CLI command now uses the same 5-layer dedup pipeline as `fact_add` LLM tool and auto-extraction: normalization (ADR-E4), Layer 1 (exact), Layer 2 (normalized), Layer 3 (FTS5), Layer 3.5 (semantic), plus eager embedding generation.

- [ ] `/fact add I prefer dark mode` → stores "User prefers dark mode" (normalized per ADR-E4)
- [ ] Wait 3 seconds for embedding generation
- [ ] `/fact add I prefer dark mode` → **Skipped: Exact duplicate** (Layer 1)
- [ ] `/fact add User prefers dark mode` → **Skipped: Similar fact** (Layer 2, normalized match)
- [ ] `/fact add I like dark mode` → Layer 3.5 should catch as paraphrase or FTS5 as similar
- [ ] `/fact add I prefer light mode` → should **UPDATE** existing preference (Layer 3.5 contradiction)
- [ ] Verify embedding exists: `sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT content, has_embedding FROM facts WHERE content LIKE '%light mode%'"` → **has_embedding = 1**

### 21.15 `/tools` Toggle for Layer 3.5 Testing (Bug #4 smoke test #2)

> **Bug #4 investigation (smoke test #2):** Some LLM models proactively call `fact_add` when they detect a contradiction, which makes it hard to test auto-extraction-based Layer 3.5. The `/tools` command disables LLM tool calls for the session, allowing auto-extraction to be tested independently.

**Procedure:**
1. Clean state: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
2. Start chat: `ask-ai chat`
3. `/tools` → should print **"Tools: disabled"**
4. Send: "I prefer dark mode" → auto-extraction should store via `normalize_to_storage_format()` and `generate_fact_embedding()`
5. Wait 5 seconds for embedding
6. Send: "Actually, I prefer light mode" → auto-extraction should detect contradiction via Layer 3.5 and UPDATE
7. `/fact list` → should show **one** preference: "User prefers light mode"
8. `/tools` → should print **"Tools: enabled"**
9. Verify auto-extraction worked independently of LLM tool calls

---

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
15. **Section 18**: Feedback Boost Integration & Decay Accuracy (end-to-end, DB inspection)
16. **Section 19**: Fact & Content Prune Shortcuts (routing verification)
17. **Section 20**: Auto Fact Extraction (extraction, dedup, config, normalization, PT→EN translation, ADR-E4, Bug #2 DEFERRED)
18. **Section 21**: Fact Embedding & Semantic Dedup (schema v11, embedding generation, recovery, Layer 3.5, Bug #3/#4, serialization)
These tests require chat interaction and visual verification of results.