# Smoke Test Manual - sprachspiel

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
cd /home/alchemist/git/sprachspiel
cargo build --release --features all-tools
# Start your LLM server (llama-swap, ollama, llama.cpp, etc.)

# Backup user's existing database
cp ~/.local/share/sprachspiel/sprachspiel.db ~/.local/share/sprachspiel/sprachspiel.db.smoke-backup 2>/dev/null || true

# Use temporary database for tests (isolation)
rm -f ~/.local/share/sprachspiel/sprachspiel.db
```

## Test Model

```bash
# Use environment variable or default
MODEL=${SMOKE_MODEL:-qwen3.5-4b}
# Ensure model is available on your LLM server
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

- [ ] Binary runs: `./target/release/sprach --help`
- [ ] Version visible: `./target/release/sprach --version`
- [ ] Subcommands listed (chat, query, translate, diagnostics)

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

- [ ] With Ollama **running**: `./target/release/sprach chat` starts without panic
- [ ] With Ollama **stopped** (pkill ollama): `./target/release/sprach chat` starts without panic — graceful error messages only
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

### 6.6.1 /search and /session forget Confirmation

- [ ] `/f test query` → "Unknown command" (shortcut `/f` removed in PR #154)
- [ ] `/search test query` → executes search
- [ ] `/session forget` → shows warning (requires --yes) ← **Issue #85: /session forget confirmation**
- [ ] `/session forget --yes` → executes forget, no `FOREIGN KEY constraint` warning ← **Bug fix: save_sqlite FK**

### 6.6.2 Todo After /session forget — No FK Warning

**Bug fix:** `save_sqlite()` now calls `ensure_conversation_exists()` before FK-dependent INSERTs.

- [ ] `/session forget --yes` → new session ID generated
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
timeout 60 ./target/release/sprach --soulless --ignore-agents query "2+2"

# Full test (with context)
timeout 120 ./target/release/sprach query "What is 2+2?"
```

- [ ] Returns answer without errors
- [ ] Exit code 0

---

## 8. Translation (optional)

```bash
./target/release/sprach translate pt "Hello"
```

- [ ] Returns translation (if model available)

---

## 9. Database

```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db ".tables"
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA user_version;"
```

- [ ] Tables exist (content, facts, conversations, session_todos, etc.)
- [ ] Schema version correct (15 or higher)

**Explicit verification:**
```bash
SCHEMA_VER=$(sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA user_version;")
[ "$SCHEMA_VER" -ge 15 ] && echo "✓ schema v$SCHEMA_VER" || echo "✗ schema v$SCHEMA_VER < 15"
```

**Verify v15 additions (schema_meta — PR #232, Issue #106):**

```bash
# Verify schema_meta table exists with embedding_dims key
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db ".tables" | grep -q "schema_meta" && echo "✓ schema_meta table" || echo "✗ schema_meta table missing"
# Verify embedding_dims value is set
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT value FROM schema_meta WHERE key = 'embedding_dims';" && echo "✓ embedding_dims recorded" || echo "✗ embedding_dims missing"
```

**Verify v14 additions (thinking_content — PR #189):**

```bash
# Verify thinking_content column exists in content_items
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA table_info(content_items);" | grep -q "thinking_content" && echo "✓ thinking_content column" || echo "✗ thinking_content column missing"
```

**v14 regression: FTS keyword search (PR #189 bug fix):**

After adding `thinking_content` column, the inline FTS SQL in `search_content_keyword()` was
missing `ci.thinking_content` — causing a column index mismatch that broke `/search`. Verify:

- [ ] `/search <query>` (where `<query>` matches a note or message) → returns results, no errors
- [ ] `/search` with a term that exists in `thinking_content` but NOT in `content` → does NOT find the item (FTS5 only indexes `content`, not `thinking_content`)
- [ ] Search results should show `thinking_content` correctly (not garbage from bm25 score column):
  ```bash
  # Insert a test message with thinking, then search for it
  # Verify thinking_content is readable in the result
  sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT id, substr(content, 1, 50), substr(thinking_content, 1, 50) FROM content_items WHERE thinking_content IS NOT NULL LIMIT 1;"
  # Expected: content and thinking_content are both readable strings
  ```

**Verify priority/tags columns in session_todos (v9):**
```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA table_info(session_todos);"
# Must include columns: priority (TEXT) and tags (TEXT)
```

**Verify v10 additions:**
```bash
# Verify feedback_signals table (v10)
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db ".tables" | grep -q "feedback_signals" && echo "✓ feedback_signals table" || echo "✗ feedback_signals table missing"
# Verify pruned column in content_items (v10)
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA table_info(content_items);" | grep -q "pruned" && echo "✓ pruned column" || echo "✗ pruned column missing"
```

**Verify v12 additions:**

```bash
# Verify has_embedding column in facts table
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA table_info(facts);" | grep -q "has_embedding" && echo "✓ has_embedding column" || echo "✗ has_embedding column missing"

# Verify fact_embeddings vec0 table with distance_metric=cosine
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db ".tables" | grep -q "fact_embeddings" && echo "✓ fact_embeddings table" || echo "✗ fact_embeddings table missing"
```

**Verify v13 additions (norm_correction FLOAT — PR #184):**

```bash
# Verify norm_correction FLOAT column in content_embeddings auxiliary table
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db \
  "SELECT typeof(norm_correction), norm_correction FROM content_embeddings_auxiliary LIMIT 1;" | grep -q "real" && echo "✓ norm_correction is FLOAT (real)" || echo "✗ norm_correction not FLOAT"

# Verify norm_correction exists in chunk_embeddings_v2 auxiliary table
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db \
  "SELECT COUNT(*) FROM chunk_embeddings_v2_auxiliary WHERE norm_correction IS NOT NULL;" | grep -qv "0" && echo "✓ chunk_embeddings_v2 has norm_correction values" || echo "✗ chunk_embeddings_v2 missing norm_correction values"

# Verify norm_correction exists in fact_embeddings auxiliary table
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db \
  "SELECT COUNT(*) FROM fact_embeddings_auxiliary WHERE norm_correction IS NOT NULL;" | grep -qv "0" && echo "✓ fact_embeddings has norm_correction values" || echo "✗ fact_embeddings missing norm_correction values"

# Verify norm_correction values are > 1.0 (non-trivial correction factors)
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db \
  "SELECT MIN(norm_correction) FROM content_embeddings_auxiliary;" | awk '{if ($1 > 1.0) print "✓ norm_correction MIN=" $1 " (> 1.0)"; else print "✗ norm_correction MIN=" $1 " (expected > 1.0)"}'
```

---

## 10. File Tools (Regression)

**Prepare test files:**
```bash
echo "test content" > /tmp/file_test.txt
# File for tilde expansion test (Bug #1 related)
echo "file tools test" > ~/file_test.txt
# Delete write-test target so the write creates a NEW file (must-read-before-edit,
# #205, only applies when overwriting an existing file)
rm -f /tmp/write_test.txt
```

Via chat with a model that supports tools:

- [ ] `read_file(path="/tmp/file_test.txt")` works
- [ ] `read_file(path="~/file_test.txt")` works (with ~) ← **Bug #1 related**
- [ ] `list_directory(path="~")` works
- [ ] `write_file(path="/tmp/write_test.txt", content="test")` works (creates new file — must-read does not apply to creation, see #205)

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

### 11.3 Provider Configuration Bail-out (PR #206 — E1)

**Verify that all entry points (`sprach chat`, `sprach query`, `sprach summarize`,
`sprach vision`) fail fast with a clear error when `models.toml` is missing or
has its `[provider.*]` block commented out.**

This catches the bug where the bail-out in `repl_tui.rs:82` was unreachable
because `resolve_model_config`'s `process::exit(1)` fired first with a generic
"Unknown model" message.

**Pré-condições:**
- `~/.config/sprachspiel/models.toml` exists with at least one model entry

**Procedimento:**
```bash
# Backup
cp ~/.config/sprachspiel/models.toml ~/.config/sprachspiel/models.toml.bak.bailout

# Comentar o bloco [provider.*] (qualquer um)
sed -i 's/^\[provider/#[provider/' ~/.config/sprachspiel/models.toml

# Testar chat
sprach chat
echo "Exit: $?"  # Expected: 1

# Testar query
sprach query "test"
echo "Exit: $?"  # Expected: 1

# Testar summarize
sprach summarize "test text"
echo "Exit: $?"  # Expected: 1

# Testar vision (com arquivo PNG dummy)
touch /tmp/test.png
sprach vision /tmp/test.png
echo "Exit: $?"  # Expected: 1
```

**Expected output (all modes):**
```
[ERROR sprach::user_models] Failed to load models.toml: Missing [provider."name"] section in models.toml at <path>. Add at least one [provider."my-llama-swap"] block with `kind = "openai"` and `base_url = "http://localhost:12434/v1"`. Run `sprach models upgrade` to migrate an existing config.
Error: Cannot determine provider: no providers defined in models.toml. Add a [provider."name"] section or run `sprach models upgrade` to migrate.
```

**For `sprach chat` specifically, additional expected output:**
```
[ERROR sprach::chat::repl] No providers configured in models.toml
Error: No providers configured in models.toml.
Hint: Add a [provider."name"] section or run `sprach models upgrade` to migrate.
Error: "Cannot start chat: models.toml is missing providers. Add a [provider.\"name\"] section or run `sprach models upgrade`."
```

**Cleanup:**
```bash
mv ~/.config/sprachspiel/models.toml.bak.bailout ~/.config/sprachspiel/models.toml
rm -f /tmp/test.png
```

- [ ] `sprach chat` retorna exit 1 com mensagem de bail-out
- [ ] `sprach query` retorna exit 1 com mensagem de bail-out
- [ ] `sprach summarize` retorna exit 1 com mensagem de bail-out
- [ ] `sprach vision` retorna exit 1 com mensagem de bail-out

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

### 12.2 File Content Search via run_command (Issue #214)

**Prepare test file:**
```bash
# Create a file with a pattern that appears many times
for i in $(seq 1 200); do echo "UNIQUEPATTERN line $i"; done > /tmp/search_truncation_test.txt
```

- [ ] `run_command("rg -n UNIQUEPATTERN /tmp/search_truncation_test.txt", "50", null, null)` returns results
- [ ] Results use `file:line: content` format (rg output style)
- [ ] `run_command("rg -n --glob *.txt UNIQUEPATTERN /tmp", null, null, null)` filters by file pattern
- [ ] `run_command("rg -n NONEXISTENT /tmp", null, null, null)` returns exit code 1 (no matches — not an error)

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
rm -f ~/.local/share/sprachspiel/sprachspiel.db
mv ~/.local/share/sprachspiel/sprachspiel.db.smoke-backup ~/.local/share/sprachspiel/sprachspiel.db 2>/dev/null || true

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
time (timeout 30 ./target/release/sprach --soulless --ignore-agents query "2+2" > /dev/null)
# Should complete in < 15 seconds on normal hardware
```

- [ ] Simple query completes in reasonable time (< 15s)

---


## 14. Subagent Chat Commands

These commands are always available in chat mode (not feature-gated).

### 14.1 /ocr command
- [ ] Start chat: `sprach chat`
- [ ] Type `/ocr` with no args → shows usage hint
- [ ] Type `/ocr .env` → "BLOCKED" error (security blocklist)

#### With repository assets
- [ ] `/ocr assets/ocr/japanese.jpg text` → extracts Japanese text (printed CJK characters)
- [ ] `/ocr assets/mixed/redacao.png table` → extracts table structure from ENEM exam page

### 14.2 /vision command
- [ ] Type `/vision` with no args → shows usage hint

#### With repository assets
- [ ] `/vision assets/vision/protagonist.jpg Identify the character and the game`
- [ ] `/vision assets/vision/protagonist.jpg,assets/vision/protagonist2.jpg Compare both characters and their games`

### 14.3 /translate command
- [ ] Type `/translate en:pt Hello world` → returns Portuguese translation

### 14.4 /summarize command
- [ ] Type `/summarize Long text about artificial intelligence` → returns concise summary

### 14.5 Feature flag
- [ ] Build without subagent-tools: `cargo build --release --no-default-features --features "weather-tools,file-tools"` → chat commands `/ocr`, `/vision`, `/translate`, `/summarize` still work, but LLM cannot call the spawn tools

### 14.6 LLM Spawn Tools (with repository assets)

**Note:** Use a model that supports tools (qwen3.5:4b or larger).

> **LLM Refusal:** If the model refuses to call a spawn tool, rephrase the prompt
> more explicitly. If refusal persists, switch to an abliterated model (see LLM Tool Refusal Policy above).

- [ ] Ask: "Use spawn_ocr_agent to extract all text from assets/ocr/japanese.jpg with ocr_mode='text'" → LLM calls `spawn_ocr_agent`
- [ ] Ask: "Use spawn_ocr_agent to extract the table from assets/mixed/redacao.png with ocr_mode='table'" → LLM calls `spawn_ocr_agent`
- [ ] Ask: "Use spawn_vision_agent to identify the character in assets/vision/protagonist.jpg" → LLM calls `spawn_vision_agent`
- [ ] Ask: "Use spawn_vision_agent to compare assets/vision/protagonist.jpg,assets/vision/protagonist2.jpg" → LLM calls `spawn_vision_agent` with multi-image paths
- [ ] Ask: "Use spawn_translate_agent to translate 'Bom dia, como vai?' to English" → LLM calls `spawn_translate_agent`
- [ ] Ask: "Use spawn_summarize_agent to summarize: Artificial intelligence is transforming the way we interact with technology, enabling natural language understanding, computer vision, and autonomous decision-making across many domains." → LLM calls `spawn_summarize_agent`

### 14.7 PDF Two-Phase Pipeline (LLM-orchestrated)

**Note:** Use a model that supports tools and has vision capability. Use the cloud model configured in the project's config.toml.

**Prerequisites:** `pdftotext` and `pdftoppm` must be installed (`poppler-utils`).

- [ ] Ask: "I have a PDF at assets/mixed/sprachspiel-architecture.pdf. Please process it — extract all text, and for any pages with diagrams, convert them to images and describe what you see."
- [ ] Verify the LLM calls `run_command("pdftotext", [...])` for Phase 1
- [ ] Verify the LLM identifies that page 2 has a diagram (little text / visual content)
- [ ] Verify the LLM calls `run_command("pdftoppm", [...])` to convert page 2 to an image
- [ ] Verify the LLM calls `spawn_vision_agent` or `spawn_ocr_agent` on the resulting image
- [ ] Verify the LLM combines results and presents a complete answer
- [ ] **Negative test:** Verify the LLM does NOT try to pass a `.pdf` file directly to `spawn_ocr_agent` or `spawn_vision_agent`

### 14.8 PDF Import via Skill (end-to-end with cloud model)

**Note:** Use the cloud model configured in the project's config.toml (checks `[models]` section for a cloud/vision-capable model). This test verifies the complete import pipeline orchestrated by a cloud LLM.

**Prerequisites:** `pdftotext` must be installed (`poppler-utils`).

- [ ] Ask: "I have a PDF at assets/mixed/sprachspiel-architecture.pdf. Process it and import the text as a document."
- [ ] Verify the LLM:
  1. Does NOT attempt `import_document` with the `.pdf` file directly (PDF is not a supported import format)
  2. Uses `run_command("pdftotext", [...])` to extract text from the PDF
  3. Uses `write_file` to save the extracted text to a `.txt` file
  4. Calls `import_document` with the `.txt` file
  5. Returns confirmation with a document ID
- [ ] Verify via `/doc list` that the document was imported successfully with the extracted text

---

## 15. Feedback Commands (New Feature - Issue #23)

Test the feedback command infrastructure for recording user feedback on assistant messages.

### 15.1 Basic Feedback Commands

- [ ] Start chat: `sprach chat`
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
- [ ] Start anonymous chat: `sprach chat --anonymous`
- [ ] Type `/feedback good` in anonymous mode → `Error: Cannot give feedback in anonymous mode.`
- [ ] Type `/feedback good` before any assistant message → `No assistant message to give feedback on.`

### 15.4 Feedback Command Tests

- [ ] Type `/feedback good` → `↑↑ good feedback recorded for msg:N` + excerpt (dim) + `Importance: +0.05`
- [ ] Type `/feedback bad` → `↓↓ bad feedback recorded for msg:N` + excerpt (dim) + `Importance: -0.10`
- [ ] Type `/feedback correction:typo fix` → `✎ correction feedback recorded for msg:N` + excerpt (dim) + `Correction: typo fix`

---

## 16. Content Prune & Context Decay Stats (New Feature - Issue #23)

Test the content decay and pruning infrastructure.

### 16.1 Content Prune

- [ ] Start chat: `sprach chat`
- [ ] Import a document first: `/doc import /tmp/test.txt`
- [ ] Type `/content prune` → shows `⏳ Running content decay cycle...` then result
- [ ] After prune with items removed: `✓ Pruned N content item(s), N remaining (avg retention: X.XX).`
- [ ] After prune with no items removed: `✓ No content to prune. N item(s) remaining (avg retention: X.XX).`

### 16.2 Context Decay Stats

- [ ] Type `/context` → shows `Content Memory:` section with:
  - `Total items: N`
  - `Avg importance: X.XX`
  - If items at risk: `⚠ Items at risk: N (low decay score)`
  - `Feedback signals: N`
- [ ] After `/content prune`, `/context` shows updated stats

### 16.3 Error Tests

- [ ] Start anonymous chat: `sprach chat --anonymous`
- [ ] Type `/content prune` in anonymous mode → `Error: Cannot prune content in anonymous mode.`
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
# Check schema version (must be 14 or higher)
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA user_version;"
# Expected: 14 or higher

# Check feedback_signals table exists
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db ".tables"
# Expected: includes feedback_signals

# Check pruned column in content_items
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA table_info(content_items);"
# Expected: includes pruned column (INTEGER NOT NULL DEFAULT 0)
```

- [ ] Schema version is 13 or higher
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

- [ ] Start chat: `sprach chat`
- [ ] Have the LLM respond to two different questions (creates 2+ assistant messages)
- [ ] Submit positive feedback on message 1: `/feedback good`
- [ ] Submit negative feedback on message 2: `/feedback bad`
- [ ] Ask a broad question that could match both messages
- [ ] Verify message with positive feedback ranks higher in search results
- [ ] Verify database shows feedback signals with correct boost values:
  ```bash
  sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT item_id, signal_type, base_value, source FROM feedback_signals;"
  ```

### 18.2 Facts Prune Cycle

- [ ] Add fact: `/fact add "Test decay fact"`
- [ ] Run `/fact prune` → fresh fact NOT pruned
- [ ] Age a fact in DB: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "UPDATE facts SET last_accessed = strftime('%s','now','-365 days') WHERE id = (SELECT MAX(id) FROM facts);"`
- [ ] Run `/fact prune` → aged fact IS pruned
- [ ] Add preference with high importance (>=0.8) and age it → NOT pruned

### 18.3 Fractional-Day Decay Verification

Verify the `num_days()` truncation fix produces accurate values at non-boundary times.

- [ ] Insert Good signal at 30.5 days ago:
  ```bash
  SIGNAL_TS=$(sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT strftime('%s','now','-30.5 days');")
  ITEM_ID=$(sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT id FROM content_items WHERE content_type='message' ORDER BY id DESC LIMIT 1;")
  sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "INSERT INTO feedback_signals (item_id, session_id, signal_type, base_value, source, created_at) VALUES ($ITEM_ID, 'test', 'good', 1.0, 'user', $SIGNAL_TS);"
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

### 19.1 /fact Canonical Commands

- [ ] `/fact prune` → prunes facts using decay cycle
- [ ] `/fact add "I prefer dark mode"` → adds a fact

### 19.2 /content Canonical Command

- [ ] `/content prune` → runs content decay cycle

---

## 20. Auto Fact Extraction (P6.1 — autoDream-lite)

Verify that preference and identity facts are auto-extracted from user messages and stored.

> **⚠️ Clean database recommended before starting this section.**  
> ```bash
> rm -f ~/.local/share/sprachspiel/sprachspiel.db
> ```
> This ensures a clean state for dedup and embedding tests.

> **⚠️ Bug #2 (DEFERRED to issue #106):** PT noun translation after the prefix is NOT handled by heuristic mode. "Eu prefiro respostas curtas" → "User prefers respostas curtas" (noun "respostas curtas" remains in PT). Full noun translation requires LLM-mode (M2).

### 20.1 Auto-Extraction Happy Path (English)

- [ ] Start chat: `sprach chat`
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

- [ ] Start: `sprach chat --anonymous`
- [ ] Send: "I prefer dark mode" → NO extraction notification appears
- [ ] Exit: `/exit`

### 20.6 Config: Disable Notification

- [ ] Edit `~/.config/sprachspiel/config.toml`, add `[facts] auto_extract_notify = false`
- [ ] Start chat, send preference → fact is extracted but NO `[Auto-extracted]` notification
- [ ] Restore config

### 20.7 Config: Disable Auto-Extract

- [ ] Edit `~/.config/sprachspiel/config.toml`, add `[facts] auto_extract = false`
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
- [ ] Check database: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT id, content, scope, project_id FROM facts WHERE content LIKE '%dark mode%'"`
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

### 20.20 Contradiction: Preference Override (Retest #3 fix — semantic triple)

> **Bug S42.4/S43.1 fix:** Layer 3.5 (after reorder) uses semantic search (cosine ≥ 0.70) with triple disambiguation: extracts (subject, predicate, object) triples; when same predicate but different object → contradiction → replace. Also catches polarity opposition via `is_contradiction()` fallback.

- [ ] Send: "I prefer dark mode" → stored as preference "User prefers dark mode"
- [ ] Send: "I prefer light mode" → extraction should detect **contradiction** (semantic triple: same predicate "prefers", different object) and **update** the existing fact
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
> rm -f ~/.local/share/sprachspiel/sprachspiel.db
> ```
> This ensures a clean state for embedding and dedup tests.

### 21.1 Schema Migration: v12 → v13 → v14

- [ ] Start a fresh chat session → no errors
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA user_version;"` → returns **14**
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA table_info(content_items);" | grep -q "thinking_content"` → **thinking_content column exists**
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA table_info(facts);"` → includes **has_embedding** column (type INTEGER, default 0)
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db ".tables"` → includes **fact_embeddings** (vec0 virtual table)
- [ ] Verify distance_metric=cosine: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT sql FROM sqlite_master WHERE name='fact_embeddings'"` → contains **distance_metric=cosine**
- [ ] Verify norm_correction FLOAT column: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT sql FROM sqlite_master WHERE name='fact_embeddings'"` → contains **+norm_correction FLOAT**
- [ ] Verify distance_metric=cosine: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT sql FROM sqlite_master WHERE name='fact_embeddings'"` → contains **distance_metric=cosine**

### 21.2 Fact Insertion Generates Embedding (Synchronous)

> **Bug #4 fix + race condition fix:** Embedding generation is now **synchronous** (await, not fire-and-forget). After inserting a fact, the embedding is generated and stored before returning. This ensures that subsequent Layer 3.5 searches can find the new fact's embedding. If Ollama is offline, `has_embedding` stays 0 and recovery generates on next startup.

- [ ] Ask LLM: "Remember that I prefer concise output" (triggers `fact_add`)
- [ ] Embedding is generated **synchronously** — no need to wait
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT id, has_embedding FROM facts WHERE content LIKE '%concise%'"` → **has_embedding = 1**
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT COUNT(*) FROM fact_embeddings"` → **≥ 1** row

### 21.3 Auto-Extraction Generates Embedding (Synchronous)

- [ ] Send: "I prefer dark mode" → wait for `[Auto-extracted]` notification
- [ ] Embedding is generated **synchronously** — no need to wait
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT id, has_embedding FROM facts WHERE content LIKE '%dark mode%'"` → **has_embedding = 1**

### 21.4 Startup Recovery: Missing Embeddings

- [ ] Manually reset embedding flag: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "UPDATE facts SET has_embedding = 0"`
- [ ] Quit and restart chat → should see `Recovering N missing fact embedding(s)` in logs (or silent if no output)
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT COUNT(*) FROM facts WHERE has_embedding = 0 AND invalidated_at IS NULL"` → **0** (all recovered)
- [ ] Check logs for post-recovery verification: should warn if any facts still lack embeddings after recovery

### 21.5 Ollama Offline: Graceful Degradation

- [ ] Stop Ollama (`pkill ollama` or similar)
- [ ] Start chat with `sprach chat` → should NOT crash
- [ ] Ask LLM: "Remember that my favorite color is blue" → fact stored, `has_embedding = 0` (no crash)
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT id, has_embedding FROM facts WHERE content LIKE '%blue%'"` → **has_embedding = 0**
- [ ] Restart Ollama
- [ ] Quit and restart chat → recovery generates missing embeddings
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT COUNT(*) FROM facts WHERE has_embedding = 0 AND invalidated_at IS NULL"` → **0** (all recovered)

### 21.6 Semantic Contradiction Detection (Bug #3 fix — Layer 3.5 with triple disambiguation)

> **Bug #3 fix + race condition fix:** After Layer 2, before FTS5, Layer 3.5 generates an embedding and searches `fact_embeddings` (cosine ≥ 0.70). For each result, it extracts triples: same predicate + different object = contradiction → Update; same triple = duplicate → Skip; different predicates → `is_contradiction()` fallback (polarity opposition). Embedding generation is now **synchronous** (await, not fire-and-forget), so fact #1's embedding is guaranteed to exist when fact #2's Layer 3.5 search runs.

**Clean state first:**
```bash
# Remove all facts for clean test
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

- [ ] Send: "I prefer dark mode" → stored as preference "User prefers dark mode"
- [ ] Embedding is generated synchronously — no need to wait
- [ ] Send: "I prefer light mode" → should UPDATE (not duplicate) the existing fact via semantic triple contradiction (same predicate "prefers", different object)
- [ ] `/fact list` → shows "User prefers light mode" (NOT both "dark" and "light")
- [ ] Verify embedding: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT COUNT(*) FROM facts WHERE content LIKE '%light mode%' AND has_embedding = 1"` → **1**

### 21.7 Semantic Duplicate Detection: Paraphrase (Layer 3.5)

- [ ] Send: "I prefer dark mode" → stored as fact
- [ ] Embedding is generated synchronously — no need to wait
- [ ] Ask LLM: "Remember that I like using dark mode" → `fact_add` should return **Skipped: Similar fact already exists** or **duplicate** (FTS5 or Layer 3.5 catches it)
- [ ] `/fact list` → only ONE dark mode preference

### 21.8 Delete Fact Removes Embedding

- [ ] Note the ID of a fact with `has_embedding = 1`: `/fact list`
- [ ] `/fact remove <ID>` → removes fact
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT COUNT(*) FROM fact_embeddings WHERE fact_id = <ID>"` → **0** (embedding also removed)

### 21.9 Shutdown Flush

- [ ] Start chat, extract some facts
- [ ] Immediately `/exit` → should complete without error
- [ ] Restart → no "Recovering" message for facts (embeddings generated synchronously at insert time)
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT COUNT(*) FROM facts WHERE has_embedding = 0 AND invalidated_at IS NULL"` → **0**

### 21.10 Startup Semantic Dedup Verification

This test requires manually inserting two semantically similar facts (without embeddings):

```bash
# Insert two similar facts about the same preference
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "INSERT INTO facts (scope, category, content, importance, decay_score, created_at, last_accessed, source, has_embedding) VALUES ('global', 'preference', 'I prefer dark mode', 0.5, 1.0, $(date +%s), $(date +%s), 'user', 0);"
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "INSERT INTO facts (scope, category, content, importance, decay_score, created_at, last_accessed, source, has_embedding) VALUES ('global', 'preference', 'I like dark mode', 0.5, 1.0, $(date +%s), $(date +%s), 'user', 0);"
```

- [ ] Insert two similar facts (as above)
- [ ] `/fact list` → should show TWO similar facts initially
- [ ] Restart chat (triggers `verify_and_dedup_facts()`)
- [ ] `/fact list` → should show ONE fact (duplicate removed by semantic dedup)
- [ ] Optionally check logs for "Fact verification: removed 1 duplicates"

### 21.11 Embedding Serialization: No Concurrent Overload (Bug #4)

> **Bug #4 fix:** `EmbeddingClient` now serializes all embedding requests through `Semaphore(1)` with a 30-second timeout. Additionally, embedding generation is now **synchronous** (await, not fire-and-forget `tokio::spawn`), so each fact's embedding is guaranteed ready before the next fact's Layer 3.5 search runs. This eliminates the race condition where fact #2's semantic search couldn't find fact #1's embedding.

- [ ] **Rapid-fire test:** Send 5+ preference messages in quick succession:
  ```
  "I prefer dark mode"
  "I like Python"
  "I hate verbose errors"
  "I want short responses"
  "I love Rust"
  ```
- [ ] NO crash or panic during rapid insertion
- [ ] All 5 facts should have embeddings (generated synchronously — no need to wait):
  ```bash
  sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT COUNT(*) FROM facts WHERE has_embedding = 1 AND invalidated_at IS NULL"
  ```
  Should be **≥ 5** (more if previous facts exist)
- [ ] Check for timeout errors in logs (should be none or very rare under normal conditions)

### 21.12 Post-Recovery Verification Warning (Bug #4)

- [ ] Stop Ollama (`pkill ollama`)
- [ ] Start chat: `sprach chat`
- [ ] Ask: "Remember that my favorite color is purple" → stored with `has_embedding = 0`
- [ ] `/exit`
- [ ] Restart Ollama
- [ ] Start chat: `sprach chat`
- [ ] If embedding recovery succeeds for all facts, no warning should appear
- [ ] If some facts remain without embeddings after recovery, a `log::warn!` message should appear (visible with `-v` verbose mode)

### 21.13 Regression — Existing Fact Features Still Work

- [ ] `/fact add` via LLM tool → works as before
- [ ] `/fact list` → shows facts correctly with scope headers
- [ ] `/fact search <query>` → returns matching facts
- [ ] `/fact remove <id>` → removes fact and its embedding
- [ ] Auto-extraction still works and generates embeddings synchronously
- [ ] Preference override contradiction still works ("prefer X" → "prefer Y" replaces via semantic triple)
- [ ] Global-wins-project rule still works

### 21.14 `/fact add` CLI: Full Dedup Parity (Bug #3 smoke test #2)

> **Bug #3 fix (smoke test #2):** `/fact add` CLI command now uses the same 6-layer dedup pipeline as `fact_add` LLM tool and auto-extraction: normalization (ADR-E4), Layer 1 (exact), Layer 2 (normalized), Layer 3.5 (semantic + triple disambiguation, ≥0.70), Layer 3 (FTS5), plus synchronous embedding generation.

- [ ] `/fact add I prefer dark mode` → stores "User prefers dark mode" (normalized per ADR-E4)
- [ ] Embedding is generated synchronously — no need to wait
- [ ] `/fact add I prefer dark mode` → **Skipped: Exact duplicate** (Layer 1)
- [ ] `/fact add User prefers dark mode` → **Skipped: Similar fact** (Layer 2, normalized match)
- [ ] `/fact add I like dark mode` → Layer 3.5 should catch as paraphrase or FTS5 as similar
- [ ] `/fact add I prefer light mode` → should **UPDATE** existing preference (semantic triple contradiction: same predicate "prefers", different object)
- [ ] Verify embedding exists: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT content, has_embedding FROM facts WHERE content LIKE '%light mode%'"` → **has_embedding = 1**

### 21.15 `/tools` Toggle for Layer 3.5 Testing (Bug #4 smoke test #2)

> **Bug #4 investigation (smoke test #2):** Some LLM models proactively call `fact_add` when they detect a contradiction, which makes it hard to test auto-extraction-based Layer 3.5. The `/tools` command disables LLM tool calls for the session, allowing auto-extraction to be tested independently.

**Procedure:**
1. Clean state: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
2. Start chat: `sprach chat`
3. `/tools` → should print **"Tools: disabled"**
4. Send: "I prefer dark mode" → auto-extraction should store via `normalize_to_storage_format()` (embedding generated synchronously)
5. Embedding is generated synchronously — no need to wait
6. Send: "Actually, I prefer light mode" → auto-extraction should detect contradiction via semantic triple (same predicate "prefers", different object) and UPDATE
7. `/fact list` → should show **one** preference: "User prefers light mode"
8. `/tools` → should print **"Tools: enabled"**
9. Verify auto-extraction worked independently of LLM tool calls

### 21.16 Semantic Triple Contradiction: EN Preference Override (Bug S42.4/S43.1 retest)

> **Bug S42.4/S43.1 fix:** Layer 3.5 (reordered, threshold 0.70) finds semantically similar facts, then triple disambiguation extracts (subject, predicate, object). Same predicate + different object → contradiction → replace. Zero ML, sub-millisecond.

**Clean state first:**
```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

- [ ] `/fact add I prefer dark mode` → ✓ Added: "User prefers dark mode"
- [ ] `/fact add I prefer light mode` → ↻ Updated: "User prefers light mode" replaces "User prefers dark mode" (preference override)
- [ ] `/fact list` → shows only ONE preference: "User prefers light mode" (dark mode is gone)

### 21.17 Semantic Triple: EN Adverb+Verb Contradiction

**Clean state first:**
```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

- [ ] `/fact add I really like vim` → ✓ Added: "User really likes vim"
- [ ] `/fact add I really like emacs` → ↻ Updated: "User really likes emacs" replaces "User really likes vim" (same predicate "really likes")
- [ ] `/fact list` → shows only ONE: "User really likes emacs"

### 21.18 Semantic Triple: EN Identity Change

**Clean state first:**
```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

- [ ] `/fact add I live in São Paulo` → ✓ Added: "User lives in São Paulo"
- [ ] `/fact add I live in Recife` → ↻ Updated: "User lives in Recife" replaces "User lives in São Paulo"
- [ ] `/fact list` → shows only ONE: "User lives in Recife"

### 21.19 Semantic Triple: EN Name Change

**Clean state first:**
```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

- [ ] `/fact add My name is Lucas` → ✓ Added: "User's name is Lucas"
- [ ] `/fact add My name is João` → ↻ Updated: "User's name is João" replaces "User's name is Lucas"
- [ ] `/fact list` → shows only ONE: "User's name is João"

### 21.20 Semantic Triple: EN No False Positive (Different Predicates)

**Clean state first:**
```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

- [ ] `/fact add I like Python` → ✓ Added: "User likes Python"
- [ ] `/fact add I prefer Rust` → ✓ Added: "User prefers Rust" (NOT a contradiction — different predicates "likes" vs "prefers")
- [ ] `/fact list` → shows BOTH facts

### 21.21 Semantic Triple: EN Negation Contradiction

**Clean state first:**
```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

- [ ] `/fact add I don't like verbose output` → ✓ Added: "User doesn't like verbose output"
- [ ] `/fact add I don't like verbose errors` → ↻ Updated: "User doesn't like verbose errors" replaces "User doesn't like verbose errors" (same predicate "doesn't like")
- [ ] `/fact list` → shows only ONE: "User doesn't like verbose errors"

### 21.22 Semantic Triple: PT Preference Override

**Clean state first:**
```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

- [ ] `/fact add Eu prefiro modo escuro` → ✓ Added: "User prefers modo escuro" (PT→EN translation, noun preserved)
- [ ] `/fact add Eu prefiro modo claro` → ↻ Updated: "User prefers modo claro" replaces "User prefers modo escuro" (same predicate "prefers")
- [ ] `/fact list` → shows only ONE: "User prefers modo claro"

### 21.23 Semantic Triple: PT Identity Change (ADR-E4 fix)

**Clean state first:**
```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

- [ ] `/fact add Meu nome é Lucas` → ✓ Added: "User's name is Lucas" (NOT "My name is Lucas" — ADR-E4 fix)
- [ ] `/fact add Meu nome é João` → ↻ Updated: "User's name is João" replaces "User's name is Lucas"
- [ ] `/fact list` → shows only ONE: "User's name is João"

### 21.24 Semantic Triple: EN Factual Content Not Affected

**Clean state first:**
```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

- [ ] `/fact add The project uses SQLite` → ✓ Added (no triple extracted — subject ≠ "user")
- [ ] `/fact add The project uses PostgreSQL` → ✓ Added (not a preference/identity fact)
- [ ] `/fact list` → shows BOTH facts (factual content coexists, not affected by semantic triple contradiction)

### 21.25 ADR-E4: PT Identity Normalization via Auto-Extraction

**Clean state first:**
```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

- [ ] Start chat: `sprach chat`
- [ ] Send: "Meu nome é Ana" → auto-extraction stores "User's name is Ana" (NOT "My name is Ana" — ADR-E4 fix)
- [ ] Send: "Eu moro em São Paulo" → auto-extraction stores "User lives in São Paulo" (NOT "I live in São Paulo")
- [ ] `/fact list` → shows both facts in third person

### 21.26 Bug #3 (Hermes): sqlite-vec L2 vs Cosine Metric Fix

> **ROOT CAUSE of S42.4/S43.1:** `search_facts_semantic()` used `1.0 - distance`, which is only correct for cosine distance. sqlite-vec defaults to L2 distance; the correct conversion is `1.0 - (distance² / 2.0)`. The broken formula scored "prefer dark mode" vs "prefer light mode" as 0.6304 instead of 0.9317. This one bug made the entire Layer 3.5 pipeline non-functional from day one. Also fixed in `content/db.rs` (both content and chunk search) and comparison direction (`<` → `>`). *Discovered by Hermes Agent.*

**Clean state first:**
```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

- [ ] `/fact add I prefer dark mode` → ✓ Added (embedding generated synchronously)
- [ ] `/fact add I prefer light mode` → ↻ Updated: "User prefers light mode" replaces "User prefers dark mode" (Layer 3.5 now works because similarity is correctly ~0.93, not ~0.63)
- [ ] `/fact list` → shows only ONE preference: "User prefers light mode"

> **If this fails (both facts coexist), the L2→cosine conversion is still broken.**

### 21.27 Bug #4 (Hermes): Missing Replacement Fact Insertion

> **Bug #4 (Hermes):** In `command_handlers.rs`, after detecting a contradiction and deleting the old fact, `return;` exited the function without inserting the replacement. The old fact was deleted; the new one was lost. Fixed in both triple and polarity paths. *Discovered by Hermes Agent.*

**Clean state first:**
```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

- [ ] `/fact add I prefer dark mode` → ✓ Added: "User prefers dark mode"
- [ ] `/fact add I prefer light mode` → ↻ Updated
- [ ] `/fact list` → shows **"User prefers light mode"** (NOT empty — replacement was inserted)
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT content, has_embedding FROM facts WHERE invalidated_at IS NULL"` → exactly ONE fact with **has_embedding = 1**

**Also test polarity path:**
- [ ] Clean: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add I like hiking` → ✓ Added: "User likes hiking"
- [ ] `/fact add I hate hiking` → ↻ Updated: "User hates hiking" replaces "User likes hiking" (polarity opposition)
- [ ] `/fact list` → shows **"User hates hiking"** (NOT empty — replacement was inserted)

> **If the fact list is empty after a contradiction, the replacement insertion code is missing.**

### 21.28 S42.4/S43.1 End-to-End: Full Pipeline Verification

> **Why all bugs matter together:** The L2→cosine metric bug (Bug #3) was the ROOT CAUSE — without correct similarity scores, Layer 3.5 couldn't find candidates. The race condition meant even with correct scores, fact #1's embedding might not exist. Bug #4 meant even after correct detection, the replacement was lost. All three had to be fixed simultaneously for S42.4/S43.1 to pass.

**Clean state first:**
```bash
rm -f ~/.local/share/sprachspiel/sprachspiel.db
```

- [ ] Start chat: `sprach chat`
- [ ] `/fact add I prefer dark mode` → ✓ Added
- [ ] `/fact add I prefer light mode` → ↻ Updated (triple contradiction)
- [ ] `/fact add I like dark mode` → ↻ Updated or Skipped (polarity/semantic catch)
- [ ] `/fact add My name is Lucas` → ✓ Added
- [ ] `/fact add My name is Maria` → ↻ Updated (identity change via triple)
- [ ] `/fact add I live in São Paulo` → ✓ Added
- [ ] `/fact add I live in Recife` → ↻ Updated (location change via triple)
- [ ] `/fact list` → shows exactly THREE facts: one about display mode, "User's name is Maria", "User lives in Recife", all with has_embedding = 1
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT COUNT(*) FROM facts WHERE invalidated_at IS NULL"` → **3**
- [ ] `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "SELECT COUNT(*) FROM facts WHERE has_embedding = 0 AND invalidated_at IS NULL"` → **0**

### 21.29 Bug #5 (Hermes): Accumulative Predicates False Positives

> **Bug #5 (Hermes):** `contradicts()` treated ALL same-predicate pairs as contradictions, so "likes Python" vs "likes Rust" was incorrectly flagged. Fixed with two-tier logic: exclusive predicates (prefers, name is) → any different object = contradiction; accumulative predicates (likes, loves, hates) → only if objects share content words (overlap > 0.3). Added `EXCLUSIVE_PREDICATES`, `POSITIVE_PREDICATES`, `NEGATIVE_PREDICATES`, `STOP_WORDS` in `lang.rs` with enforcement test.

**Clean state first:**
```bash
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"
```

#### Accumulative predicates coexist (different topics)

- [ ] `/fact add I like Python` → ✓ Added: "User likes Python"
- [ ] `/fact add I like Rust` → ✓ Added: "User likes Rust" (NOT a contradiction — different topics, no word overlap)
- [ ] `/fact list` → shows BOTH facts

#### Accumulative predicates contradict (same category)

- [ ] Clean: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add I like dark mode` → ✓ Added: "User likes dark mode"
- [ ] `/fact add I like light mode` → ↻ Updated: "User likes light mode" replaces "User likes dark mode" (overlap "mode" > 0.3)
- [ ] `/fact list` → shows only ONE fact: "User likes light mode"

#### Exclusive predicates still contradict

- [ ] Clean: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add I prefer dark mode` → ✓ Added: "User prefers dark mode"
- [ ] `/fact add I prefer light mode` → ↻ Updated: "User prefers light mode" (exclusive predicate → always contradiction)
- [ ] `/fact list` → shows only ONE fact: "User prefers light mode"

#### Polarity flip still contradicts

- [ ] Clean: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add I like hiking` → ✓ Added: "User likes hiking"
- [ ] `/fact add I hate hiking` → ↻ Updated: "User hates hiking" replaces "User likes hiking" (polarity flip: likes → hates)
- [ ] `/fact list` → shows only ONE fact: "User hates hiking"

#### Known limitation: vim/emacs

> "likes vim" vs "likes emacs" → overlap = 0, NOT a contradiction. You CAN like both editors, but pragmatically most people pick one. Deferred to Phase 2 (LLM adjudication).

- [ ] Clean: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add I like vim` → ✓ Added
- [ ] `/fact add I like emacs` → ✓ Added (NO word overlap → coexist — this is correct behavior, not a bug)
- [ ] `/fact list` → shows BOTH facts (this is expected)

---

## 22. CommandOutput Rendering Regression (W6-PR1 #145)

Verify that all chat command outputs render correctly after the `CommandOutput` enum migration.
Every command now returns `Vec<CommandOutput>` and rendering is handled by `ChatView::show_command_output()`.
The key risk is **visual regression** — missing icons, wrong colors, or multi-output commands rendering incorrectly.

### 22.1 Simple Output Variants (Info/Success/Warning/Error/Progress)

- [ ] `/help` → renders help text (no truncation, all commands visible)
- [ ] `/think on` → shows ✓ icon with dim message (Success variant)
- [ ] `/think off` → shows ✓ icon with dim message (Success variant)
- [ ] `/tools` → shows "Tools: disabled" as Info (toggle) or "Tools: enabled" as Success
- [ ] `/retrieval` → shows toggle status as Info message
- [ ] `/undo` with empty history → shows Error message with ✗ icon
- [ ] `/session forget` (without --yes) → shows **two** outputs: Warning (⚠ icon) + Warning (⚠ icon)

### 22.2 Fact Commands

- [ ] `/fact list` → renders FactList with scope headers (Global/Project)
- [ ] `/fact list --global` → renders only Global facts
- [ ] `/fact list --project` → renders only Project facts
- [ ] `/fact add Test rendering fact` → shows ✓ success message
- [ ] `/fact search test` → renders FactSearchResults with formatted output
- [ ] `/fact remove 999` → renders FactRemoveResult with ✗ error icon (not found)
- [ ] `/fact prune` → renders Progress then Success/Info result

### 22.3 Note Commands

- [ ] `/note list` → renders NoteList with formatted entries — **must show `page 1/N` (NOT `page 2/N` — off-by-one bug regression)**
- [ ] `/note add "Render test note"` → shows ✓ success with NoteAdded variant
- [ ] `/note show 1` → renders note content via MarkdownContent
- [ ] `/note delete 999` → renders ✗ error (not found)

### 22.3b Todo Commands (CommandOutput::TodoList)

> **Note:** Section 6.5 tests todo CRUD. This section verifies the CommandOutput rendering specifically.

- [ ] `/todo add "Smoke test task"` → renders ✓ success message
- [ ] `/todo list` → renders TodoList variant (formatted task list with counts)
- [ ] `/todo list pending` → renders filtered TodoList
- [ ] `/todo list #<tag>` → renders filtered TodoList
- [ ] `/todo get 1` → renders task details (via MarkdownContent)
- [ ] `/todo update 1 done` → renders ✓ success
- [ ] `/todo delete 1` → renders ✓ or ✗ result

### 22.4 Document Commands

- [ ] `/doc list` → renders DocumentList with entries — **must show `#N title (txt, N words, Nd)` format (NOT `[txt] [0 chunks]` — bug regression)**
- [ ] `/doc import /tmp/test.txt` → renders Success/Info with chunk count
- [ ] `/doc show 999` → renders ✗ error (not found)
- [ ] `/doc delete 999` → renders ✗ error (not found)

### 22.5 Session & Context Commands

- [ ] `/list` → renders SessionList with entries
- [ ] `/context` → renders ContextInfo with token counts and model info
- [ ] `/compact` → renders Progress (⏳ "Compacting...") then CompactResult with counts
- [ ] `/export` → renders ExportResult with format info

### 22.6 Skill Command

- [ ] `/skill` → renders SkillList with available skills
- [ ] `/skill <name>` → renders Success activation message

### 22.7 Search Command

- [ ] `/search <query>` → renders SearchResults with formatted markdown output

### 22.8 Reindex Command

- [ ] `/reindex` → renders Progress then ReindexResult with counts

### 22.9 Multi-Output Commands

> **Critical:** Commands that return `Vec<CommandOutput>` with multiple items must render ALL items in sequence.

- [ ] `/session forget` (without --yes) → renders 2 warnings (both visible, not just one)
- [ ] `/save mysession` → renders Success message
- [ ] `/load mysession` → renders Success message + Info about loaded session
- [ ] `/undo` (with messages in history) → renders Info "Removed N message(s)" + Info "Last message: ..." + Info "(Press ↑ to retrieve...)"
- [ ] `/undo` (with empty history) → renders Info "No messages to remove" + Info "No user message to show"
- [ ] `/model <name>` → renders optional Warning(s) (e.g., think/tools unavailable) then Info "Switched to model: ..."
- [ ] `/exit` → renders Info "Goodbye!" then Quit (cleanly exits)

### 22.10 Token Display

- [ ] Send a message to the LLM → after response, token metrics appear (dimmed, showing prompt/response/total tokens)
- [ ] Token display line format: `Tokens: N prompt + N response = N total`

### 22.11 Content Prune

- [ ] `/content prune` → renders Progress (⏳) then ContentPruneResult

### 22.12 Dead Code Verification (W6-PR1 Cleanup)

The following were removed as dead code in W6-PR1. Verify they don't appear as variant names in any error messages:

- [ ] `NoteRemoved` variant removed — `/note delete` uses Success/Error directly
- [ ] `FactAdded` variant removed — fact addition uses Success/Warning/Info directly
- [ ] `FactAddOutcome` / `FactAddResult` enums removed — no regression

---

## 23. TUI Event Loop & Rendering (W6-PR4 #148)

Verify the decomposed event loop handlers and rendering fixes from PR #148.

### 23.1 Multi-line User Input Rendering

Bug B2 fix: multi-line user messages (Shift+Enter) now render with `>>>` prefix on first line and `    ` (4-space indent) on continuation lines.

- [ ] Start chat, type `line1` → Shift+Enter → `line2` → Enter
- [ ] Verify: `>>> line1` on first line, `    line2` on second line (4-space indent, no `>>>`)
- [ ] Type `Hello, single line` → Enter
- [ ] Verify: Shows `>>> Hello, single line` (single line, `>>>` prefix, no extra blank lines)

### 23.2 Embedding Exit Hint

Bug B3 fix: "Saving embeddings..." message appears before exit when there are pending embeddings.

- [ ] Start chat, send a few messages to generate facts
- [ ] Type `/quit` or press Ctrl+D
- [ ] Verify: "Saving embeddings..." message appears before the app exits
- [ ] Start chat with `sprach chat -- --anonymous`
- [ ] Type `/quit` or press Ctrl+D
- [ ] Verify: No "Saving embeddings..." message (anonymous mode has no DB)

### 23.3 Provider-Agnostic Error Messages

Phase 4.13: Error messages use "LLM" prefix instead of "Ollama" for generic errors.

- [ ] Stop ollama (`pkill ollama` or disable service)
- [ ] Start chat: `sprach chat`
- [ ] Send a message
- [ ] Verify: Error message contains "LLM" (not hardcoded "Ollama" as generic error prefix)
- [ ] Note: `(start it with \`ollama serve\`)` in the hint is correct — that's the actual command

### 23.4 Event Loop Regression

PR #148 refactored the event loop into handler functions. Verify all interactions still work.

- [ ] `/think on` → 🧠 indicator appears in status line
- [ ] `/think off` → 🧠 indicator removed
- [ ] `/tools` → tool list shown
- [ ] Send a message, press Ctrl+C during streaming → "[Interrupted]" message, can send again
- [ ] Send 3 messages in sequence → all responses appear correctly, no missing/duplicated content

---

## 24. Bare #[allow(dead_code)] Check

Run this before release to ensure no dead code is silenced without justification:

```bash
BARE_ALLOWS=$(rg '#\[allow\(dead_code\)\]' --glob '*.rs' src/ | grep -v '// ' | wc -l)
if [ "$BARE_ALLOWS" -gt 0 ]; then
  echo "FAIL: Found $BARE_ALLOWS bare #[allow(dead_code)] without justification:"
  rg '#\[allow\(dead_code\)\]' --glob '*.rs' src/ | grep -v '// '
  exit 1
fi
echo "OK: All #[allow(dead_code)] have justification comments"
```

Every `#[allow(dead_code)]` MUST have a `//` comment on the same line explaining why:
- ✅ `#[allow(dead_code)] // Reserved for Phase 2: TUI commands`
- ✅ `#[allow(dead_code)] // Used in integration tests`
- ❌ `#[allow(dead_code)]` — no justification, will fail the check

---

## 25. Embedding Diagnostics (Issue #133)

**Objective:** Verify the `sprach diagnostics` subcommand works and reports embedding geometry.

```bash
./target/release/sprach diagnostics
```

- [ ] Report header displays "Embedding Diagnostics Report"
- [ ] Model name shows "nomic-embed-text-v2-moe:latest"
- [ ] Nominal dimensions: 256
- [ ] Vector counts shown per source (content, chunks, facts)
- [ ] d_eff (participation ratio) is a positive number
- [ ] Pairwise cosine distance: Mean, Min, Max all numeric
- [ ] Regime classification at 4 thresholds (0.70, 0.75, 0.80, 0.85)
- [ ] Variance explained: PC numbers for 50%, 90%, 95%, 99%
- [ ] No NaN or infinity values

### 25.1 Source Filter

```bash
./target/release/sprach diagnostics --source content
```

- [ ] Only "content" source shown in vector counts
- [ ] No chunks or facts counts

### 25.2 Alias

```bash
./target/release/sprach diag
```

- [ ] Same output as `sprach diagnostics`

### 25.3 Empty Database

```bash
rm -f /tmp/test_diag_empty.db
./target/release/sprach diagnostics --db /tmp/test_diag_empty.db
```

- [ ] No panic or crash
- [ ] Shows "No embedding vectors found" warning
- [ ] All source counts are 0

### 25.4 Invalid Source

```bash
./target/release/sprach diagnostics --source invalid_source
```

- [ ] Error message (not panic)
- [ ] Mentions valid values (content, chunks, facts)

### 25.5 Recommended Configuration Section (PR #184)

**Objective:** Verify that `sprach diagnostics` includes a data-driven threshold recommendation section with config.toml suggestions.

```bash
# Use a database with embeddings (the backup has ~11K)
cp ~/.local/share/sprachspiel/sprachspiel.db.old ~/.local/share/sprachspiel/sprachspiel.db
./target/release/sprach diagnostics
```

- [ ] Report includes a "## Recommended configuration" section
- [ ] Shows `[facts].semantic_threshold:` with a numeric value (e.g., `0.70` or `0.80`)
- [ ] If `adjust_weights` is true: shows `[retrieval].keyword_weight:` and `[retrieval].semantic_weight:` with numeric values
- [ ] If `adjust_weights` is false: shows a message like "Default weights are appropriate"
- [ ] Blockquote at the end says "update your config.toml" (NOT "sprach config edit" or any nonexistent command)

```bash
# Also test with --source filter
./target/release/sprach diagnostics --source facts
```

- [ ] Report includes "## Recommended configuration" section even with single source
- [ ] If vector count is small (< 100), shows a warning about d_eff reliability

### 25.6 Config.toml Settings (PR #184)

**Objective:** Verify that `[facts].semantic_threshold` and `[retrieval].keyword_weight`/`semantic_weight` settings work in config.toml.

**Test default values (no config section):**

```bash
# Ensure [facts] and [retrieval] sections are commented out or absent
cat ~/.config/sprachspiel/config.toml | grep -E "semantic_threshold|keyword_weight|semantic_weight"
# Should be commented out or absent
```

- [ ] Application starts without errors with default settings
- [ ] Default `semantic_threshold = 0.70` used when not configured

**Test custom values:**

```bash
# Add to ~/.config/sprachspiel/config.toml:
cat >> ~/.config/sprachspiel/config.toml << 'EOF'

[facts]
semantic_threshold = 0.80

[retrieval]
keyword_weight = 0.5
semantic_weight = 0.5
EOF
```

- [ ] Application starts without errors with custom settings
- [ ] `/fact add Test fact for threshold` works normally
- [ ] Conversations with retrieval work normally
- [ ] `sprach diagnostics` shows updated recommendations that may differ from defaults

**Reset config after test:**

```bash
# Remove the test settings from config.toml
# (or comment them out, or restore from backup)
```

---

## 26. TUI Tool Call Display & ReAct Resilience (PR #207)

Verify tool call rendering, context count, and ReAct loop resilience fixes from PR #207.

### 26.1 Tool Call Display Format (including BUG-1 fix: args with local models)

**Objective:** Tool calls show name + priority args, no ID in normal mode, ✗ on error. Args must be visible even with local models that don't stream argument_delta.

- [ ] Start chat with tools enabled: `./target/release/sprach --soulless --ignore-agents chat`
- [ ] Send: "List the current directory, then read Cargo.toml"
- [ ] Verify: tool calls show as `🔧  list_directory(path=.)` and `🔧  read_file(path=Cargo.toml)` — args visible
- [ ] **Critical (BUG-1):** If using a local model (e.g., qwen3.5-4b via llama-swap), verify args are NOT empty — should show `🔧  read_file(path=Cargo.toml)`, NOT `🔧  read_file()`
- [ ] Verify: NO `(tool_call_id)` suffix in normal mode
- [ ] Type `/debug` to enable trace mode
- [ ] Send another tool-triggering message
- [ ] Verify: tool call IDs now visible, e.g. `🔧  list_directory(path=.) (\`list_directory_1\`)`
- [ ] Verify: args are STILL visible in debug mode (BUG-1 report noted args missing in debug too)
- [ ] Type `/debug` to disable

### 26.2 Error Indicator on Failed Tool Call

- [ ] Send: "Use read_file to read /tmp/nonexistent_test_file.md"
- [ ] Verify: failed tool call shows `✗` prefix instead of `🔧`
- [ ] Verify: ReAct loop continues (model receives error, responds normally)
- [ ] Verify: NO `⛔` banner error at the top of the chat

### 26.3 Context Count During ReAct Loop

- [ ] Send a message that triggers multiple tool call rounds (e.g., "List subdirectories of ~/git and summarize each")
- [ ] During streaming: verify status bar shows non-zero token count (e.g., `6.4K/128K`)
- [ ] After each round: verify count increases (TurnMetrics events)
- [ ] After final response: verify count does NOT drop to a very low value (e.g., from 16K to 1K)
- [ ] Type `/context` and verify "Total used" is realistic (includes system prompt + tools + history)

### 26.4 Tool Calls Don't Disappear

- [ ] Send a prompt that generates 5+ tool calls across multiple rounds
- [ ] Scroll through the chat: verify ALL tool calls are still visible
- [ ] Verify: earlier rounds' tool calls are NOT overwritten by later calls
- [ ] Verify: same tool called multiple times shows separate entries (unique IDs in debug mode)

### 26.5 Pre-tool Text AND Thinking Block Preservation (including BUG-2 fix)

**Objective:** Verify pre-tool text AND thinking blocks from earlier ReAct rounds are preserved in scrollback.

- [ ] Ensure thinking is enabled (`/think on` if needed)
- [ ] Send a message where the model writes text AND thinking BEFORE calling tools, then more thinking after
  Example: "Read the file Cargo.toml and then search for the word 'test' in the current directory"
- [ ] During streaming, observe: Thinking1 → ToolCall → Thinking2 → Response
- [ ] After final response, scroll through chat:
- [ ] Verify: pre-tool text remains visible (Bug C fix — not replaced by post-tool content)
- [ ] **Critical (BUG-2):** Verify: Thinking1 (from before the first tool call) is STILL VISIBLE in scrollback
- [ ] Verify: Thinking2 (from after the tool call) is ALSO visible
- [ ] Verify: order is preserved: Thinking1 → ToolCall → Thinking2 → Response (not reordered)
- [ ] If only one thinking block is visible, BUG-2 is NOT fixed

### 26.6 Error Ordering (Timeout)

- [ ] Using a cloud model, send a complex prompt that generates many tool calls
- [ ] If a timeout occurs: verify the error (⛔) appears AFTER tool calls, not before
- [ ] Verify: the error does NOT appear at the top of the conversation
- [ ] Note: if no timeout occurs, this test passes by default

### 26.7 ReAct Loop Resilience (Invalid Args + Timeout)

- [ ] Using a model that sometimes sends malformed tool args (e.g., MiniMax M3):
  - Send a complex prompt with many tool calls
  - If malformed args occur: verify the ReAct loop continues (does NOT break)
  - Verify: NO `⛔ invalid tool call arguments` banner at top
  - Verify: prompt is NOT opened for user (conversation continues)
- [ ] If a stream timeout occurs (300s idle):
  - Verify: ReAct loop retries (up to 3 times)
  - Verify: model can continue after retry
  - Verify: prompt is NOT opened for user
- [ ] Note: if neither occurs, these tests pass by default

### 26.8 Clippy Strict Gate (BUG-3 fix)

```bash
cd /home/alchemist/git/sprachspiel
cargo clippy -- -D warnings -A clippy::allow_attributes -A clippy::too_many_lines -A clippy::cognitive_complexity 2>&1 | grep "^error"
# Expected: no output (0 errors — BUG-3 was blocking, now fixed)
```

- [ ] **Critical (BUG-3):** clippy strict gate passes with 0 errors

---

## 27. W2 Provider Chain Closure (#123)

Tests for the removal of `ollama-rs` and migration to provider-agnostic types.

### 27.1 ollama-rs Not in Dependency Tree

```bash
cd /home/alchemist/git/sprachspiel
# Verify no ollama-rs in Cargo.toml
grep -c "ollama-rs" Cargo.toml
# Expected: 0

# Verify no ollama-rs in Cargo.lock
grep -c "ollama-rs" Cargo.lock
# Expected: 0
```

- [ ] `Cargo.toml` has zero `ollama-rs` references
- [ ] `Cargo.lock` has zero `ollama-rs` entries

### 27.2 Feature-Matrix Clippy Clean

```bash
cd /home/alchemist/git/sprachspiel
cargo clippy --no-default-features --features weather-tools 2>&1 | grep "^warning: unused\|never used"
# Expected: no output (0 warnings)
```

- [ ] `cargo clippy --no-default-features --features weather-tools` has 0 unused/never-used warnings

### 27.3 Tool Calling Without Explicit `tools = true`

This tests the bug fix where `detect_capabilities()` was returning `tools: false` by default, preventing tools from being registered.

```bash
# Ensure llama-swap is running
# Start chat with a model (model does NOT need tools = true in models.toml)
./target/release/sprach chat
```

- [ ] Chat starts without errors
- [ ] Ask the model: "Read the file Cargo.toml and tell me the crate name" — model should use `read_file` tool via function calling (not emit tool calls as text/markdown)
- [ ] Tool result is displayed (content of Cargo.toml)
- [ ] `/exit` works

### 27.4 reasoning_effort Only Sent When Thinking Is Enabled

```bash
# Start chat WITHOUT -t flag
RUST_LOG=debug ./target/release/sprach chat
```

- [ ] Send a simple message: "Hello"
- [ ] Check debug logs: `reasoning_effort` field should NOT appear in the request JSON
- [ ] `/think on`
- [ ] Send another message: "What is 2+2?"
- [ ] Check debug logs: `reasoning_effort: "medium"` should appear in the request JSON
- [ ] `/exit`

### 27.5 Provider-Agnostic Configuration

```bash
# Verify models.toml uses kind = "openai" (not "ollama")
grep 'kind' ~/.config/sprachspiel/models.toml
# Expected: kind = "openai" (not kind = "ollama")

# Verify base_url includes /v1 suffix
grep 'base_url' ~/.config/sprachspiel/models.toml
# Expected: all base_url values end with /v1
```

- [ ] `models.toml` uses `kind = "openai"`
- [ ] All `base_url` values include `/v1` suffix
- [ ] `sprach chat` starts without config errors

### 27.6 TTFB Watchdog

Verify that streaming works normally (TTFB watchdog doesn't fire spuriously):

```bash
./target/release/sprach chat
```

- [ ] Send a message that triggers streaming: "Write a short poem about cats"
- [ ] Streaming tokens appear within a few seconds (no 120s timeout)
- [ ] Response completes normally
- [ ] `/exit`

---

## 29. File Session State + Staleness Detection (Issue #205, PR #231)

**Objective:** Verify must-read-before-edit and staleness detection work end-to-end in chat.

**Prepare:**
```bash
mkdir -p /tmp/sprach_smoke_205
echo "alpha beta gamma" > /tmp/sprach_smoke_205/watched.txt
echo "scratch" > /tmp/sprach_smoke_205/scratch.txt
rm -f /tmp/sprach_smoke_205/created.txt
```

Via chat with a model that supports tools:

### 29.1 Must-read rejects edit on never-read file
- [ ] Ask LLM to edit `/tmp/sprach_smoke_205/watched.txt` WITHOUT calling read_file first (e.g., "Replace 'beta' with 'BETA' in /tmp/sprach_smoke_205/watched.txt using edit_file.")
- [ ] Tool error contains "has not been read in this session"
- [ ] File on disk UNCHANGED: `grep 'beta' /tmp/sprach_smoke_205/watched.txt` still matches

### 29.2 Must-read rejects write_file overwrite on never-read file
- [ ] Ask LLM to overwrite `/tmp/sprach_smoke_205/watched.txt` with new content (write_file, overwrite=true)
- [ ] Tool error contains "has not been read in this session"
- [ ] File unchanged

### 29.3 write_file creating a NEW file is allowed without read
- [ ] Ask LLM to create `/tmp/sprach_smoke_205/created.txt` with any content
- [ ] Tool succeeds (no must-read error)
- [ ] File exists with expected content

### 29.4 Read-then-edit happy path
- [ ] Ask LLM to read `/tmp/sprach_smoke_205/watched.txt`, then edit it
- [ ] Edit succeeds; result shows unified diff block (#204)

### 29.5 External modification triggers staleness
- [ ] Ask LLM to read `/tmp/sprach_smoke_205/watched.txt`
- [ ] **In another terminal**, modify externally: `echo "external" >> /tmp/sprach_smoke_205/watched.txt`
- [ ] Ask LLM to edit that file again (without re-reading)
- [ ] Tool error contains "has been modified since it was last read"
- [ ] External change still on disk (`tail -1` shows `external`)

### 29.6 Re-read clears staleness
- [ ] Ask LLM to re-read `/tmp/sprach_smoke_205/watched.txt`, then edit
- [ ] Edit succeeds

### 29.7 append_file is exempt
- [ ] Ask LLM to append to `/tmp/sprach_smoke_205/scratch.txt` WITHOUT reading it first
- [ ] Tool succeeds — no must-read error

### 29.8 Sandbox still wins over staleness/must-read
- [ ] Ask LLM to edit a file outside sandbox (e.g., `/etc/hostname` or `/root/x`)
- [ ] Error is a sandbox error (path outside allowed directory), NOT a must-read error

**Cleanup:**
```bash
rm -rf /tmp/sprach_smoke_205
```

---

## 30. Configurable Embedding Model — Prefix, Context Length, Dynamic vec0 (Issue #106, PR #232)

**Objective:** Verify that embedding prefix, context_length, and vec0 storage dimensions are configurable and that switching embedding models works end-to-end.

**Prerequisites:** At least 2 embedding models available on the LLM server (e.g., `nomic-embed-text-v2-moe` at 256/768d and `lfm2.5-embedding-350m` at 1024d).

### 30.1 Default Config — No Regression

- [ ] `[indexing]` section with `model = "nomic"` (no explicit `prefix`) starts without error
- [ ] `schema_meta` table exists with `embedding_dims` = `256` (or `768` if no Matryoshka)
- [ ] `/search <query>` returns results without error

### 30.2 Configurable Prefix

- [ ] `[indexing].prefix = ""` (empty) is accepted — no prefix prepended
- [ ] `[indexing].prefix = "query: "` is accepted — prefix prepended to each text
- [ ] Search works with both empty and non-empty prefix (results may differ in quality, but no crash)

### 30.3 Dynamic vec0 Dimensions — Model Switch

- [ ] Start with nomic (256d): `schema_meta.embedding_dims` = `256`
- [ ] Import a document and verify `has_embedding = 1` for the content item
- [ ] Switch `[indexing].model` to `lfm2.5-embedding-350m` (1024d)
- [ ] Start chat — logs show "Embedding dimensions changed (256 → 1024)"
- [ ] `schema_meta.embedding_dims` = `1024`
- [ ] vec0 tables recreated: `SELECT sql FROM sqlite_master WHERE name = 'content_embeddings'` shows `FLOAT[1024]`
- [ ] All `has_embedding` flags reset to 0 (background recovery will regenerate)
- [ ] `/search <query>` works at 1024d after regeneration

### 30.4 Switching Back (1024 → 256)

- [ ] Switch `[indexing].model` back to nomic (256d)
- [ ] Start chat — vec0 recreated at `FLOAT[256]`
- [ ] `schema_meta.embedding_dims` = `256`
- [ ] All `has_embedding` flags reset to 0

### 30.5 Config Upgrade Detects New `prefix` Field

- [ ] `sprach config upgrade --dry-run` lists `indexing.prefix` as a new field
- [ ] Suggested default is `"search_document: "`

### 30.6 Context Length from Model Config

- [ ] With LFM2.5 (32K context via `num_ctx`), a 2000-char text does NOT trigger "context exceeded"
- [ ] With nomic (8K or fallback 512), the same text may or may not trigger (depends on `num_ctx`)

---

## 28. Known False Alarms

These behaviors may appear as bugs during testing but are NOT Sprachspiel issues.

### 28.1 Thinking Blocks Despite /think off

**Symptom:** After `/think off`, thinking blocks (🧠) still appear in responses.
**Cause:** When using llama-swap model aliases with `:think` suffix (e.g., `qwen3.5-4b:think`), the backend always generates thinking content server-side, regardless of `reasoning_effort`. Sprachspiel correctly omits `reasoning_effort` from the request, but the model sends thinking anyway.
**Verification:** Check debug logs with `RUST_LOG=debug` — `reasoning_effort` should NOT be in the request JSON when `/think off` is active. If it IS present, that's a real bug.
**Workaround:** Use a model alias without `:think` suffix if you want to control thinking via Sprachspiel.

### 28.2 Model Loading Timeout on First Request

**Symptom:** First request to a subcommand (OCR, Vision, Summarize) or `/model` switch times out or takes very long.
**Cause:** llama-swap needs to load (or swap) the model into VRAM. The `read_timeout_secs` (default 300s, configurable in `models.toml [provider.*]`) controls the HTTP timeout. If the model takes longer to load than `read_timeout_secs`, the request fails with a timeout error.
**Verification:** Retry the same request — the second attempt should be fast (model already loaded). If it consistently times out even on retry, check that `read_timeout_secs` is high enough for your hardware.
**Configuration:** Increase `read_timeout_secs` in `models.toml [provider.*]` section (e.g., 900s for slow hardware with model offloading). All timeout fields are per-provider:
- `connect_timeout_secs` (default 5s) — TCP connection establishment
- `read_timeout_secs` (default 300s) — HTTP response timeout (non-streaming)
- `stream_idle_timeout_secs` (default 300s) — Max gap between SSE chunks in streaming
- `ttfb_timeout_secs` (default 120s) — Time to first byte in streaming

### 28.3 read_file with Relative Path Fails

**Symptom:** `read_file("Cargo.toml")` fails with "file not found" when chat is started without `--cwd`.
**Cause:** `read_file` uses the current working directory (CWD). When `sprach chat` is started from `~`, the CWD is the home directory, not the project.
**Verification:** Use `--cwd /path/to/project` when starting chat, or use absolute paths in tool calls. The model typically recovers via ReAct (calls `list_directory` to discover the correct path).
**Not a bug:** This is expected CWD-dependent behavior.

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
cp ~/.local/share/sprachspiel/sprachspiel.db ~/.local/share/sprachspiel/sprachspiel.db.smoke-backup 2>/dev/null || true

# 2. Build
echo "Building..."
cargo build --release --features all-tools || { echo "✗ Build failed"; exit 1; }
echo "✓ Build"

# 3. Quick checks
./target/release/sprach --help | grep -q "chat" && echo "✓ chat command"
./target/release/sprach --version && echo "✓ version"

# 4. Unit tests
echo "Unit tests..."
cargo test --lib 2>&1 | tail -5
echo "✓ Unit tests"

# 5. Restore
mv ~/.local/share/sprachspiel/sprachspiel.db.smoke-backup ~/.local/share/sprachspiel/sprachspiel.db 2>/dev/null || true

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
6. **Section 6.6**: Command Safety ( /session forget, /search, skills)
7. **Section 9**: Database (schema v14, norm_correction FLOAT verification)
8. **Section 10**: File Tools (via LLM)
9. **Section 10.5**: run_command Error Messages
10. **Section 11**: Memory Staleness Warnings (code review + fresh fact check)
11. **Section 11.3**: Provider Configuration Bail-out (PR #206 E1 — all 4 entry points)
12. **Section 12**: Truncation Warnings in Tool Outputs (via LLM)
13. **Section 13**: Performance (verify response time)
13. **Section 15**: Feedback Commands (interactive feedback tests)
14. **Section 16**: Content Prune & Context Decay Stats (interactive tests)
15. **Section 17**: Feedback Tool & Configuration (via LLM + database verification)
16. **Section 18**: Feedback Boost Integration & Decay Accuracy (end-to-end, DB inspection)
17. **Section 19**: Fact & Content Prune Shortcuts (routing verification)
18. **Section 20**: Auto Fact Extraction (extraction, dedup, config, normalization, PT→EN translation, ADR-E4, Bug #2 DEFERRED)
19. **Section 21**: Fact Embedding & Semantic Dedup (schema v14, norm_correction FLOAT, synchronous embedding, recovery, Layer 3.5, Bug #3/#4/#5, semantic threshold, end-to-end verification)
20. **Section 22**: CommandOutput Rendering Regression (W6-PR1 — all command output variants, multi-output, token display, dead code removal)
21. **Section 22b**: Bare #[allow(dead_code)] Check (automated, no justification = fail)
22. **Section 23**: TUI Event Loop & Rendering (multi-line rendering, embedding exit hint, provider-agnostic errors, event loop regression)
23. **Section 25**: Embedding Diagnostics (read-only subcommand, recommended configuration section, no LLM needed)
24. **Section 25.5**: Recommended Configuration Output (threshold and weight suggestions from diagnostics)
25. **Section 25.6**: Config.toml Settings (semantic_threshold, keyword_weight, semantic_weight)
26. **Section 26**: TUI Tool Call Display & ReAct Resilience (PR #207 — tool call format, ✗ error indicator, context count, tool calls don't disappear, pre-tool text preservation, error ordering, ReAct resilience)
27. **Section 29**: File Session State + Staleness Detection (Issue #205 — must-read, staleness, append exempt, sandbox priority)
28. **Section 30**: Configurable Embedding Model (Issue #106 — prefix config, context_length from num_ctx, dynamic vec0 dimensions, model switch 256→1024 and back, config upgrade detects prefix)
These tests require chat interaction and visual verification of results.