# Roadmap

This document outlines planned features and the current state of Ask-AI.

## Current State

### Implemented Features

**Core CLI:**
- 5 subcommands (query, chat, translate, ocr, summarize)
- 3 built-in model presets (llama3.1, translategemma, glm-ocr)
- User-defined models via `~/.config/ask-ai/models.toml`
- Optional model parameters (top_k, top_p, repeat_penalty)
- Thinking support for cloud models (`thinking = true` in config)
- Markdown rendering via termimad
- Model capability detection (tools, vision, ocr)
- Pipe support for all commands
- Debug mode, Think mode, Code mode
- Configuration file support (`~/.config/ask-ai/config.toml`)
- Per-subcommand model configuration
- AGENTS.md context injection with security sanitization
- Shell argument handling

**Interactive Chat:**
- Persistent conversation history per project
- Anonymous sessions (no persistence)
- Session management (`/save`, `/load`, `/list`)
- Model switching mid-conversation
- Export to Markdown/JSON
- Auto-save after each message
- `/think` and `/tools` toggle commands
- `/tools-output` for controlling tool verbosity
- `/compact` for conversation summarization
- `/retry` (alias `/r`) for regenerating last response
- `/undo` for removing last response (with database cleanup)
- `/search` (alias `/find`, `/f`) for semantic search
- `/context` (alias `/ctx`) for token metrics
- Tab completion for commands and models
- Mode indicators in prompt (`[t]`, `[T]`)
- Token metrics display
- Thinking output visible when enabled
- Error recovery for tool/network errors
- Context overflow protection during tool execution

**Tools (28 total):**

| Category | Count | Feature Flag | Default |
|----------|-------|--------------|---------|
| Pokémon | 9 | `pokemon-tools` | ✅ Enabled |
| Weather | 3 | `weather-tools` | ✅ Enabled |
| File Operations | 5 | `file-tools` | ✅ Enabled |
| Calculator | 1 | `calc-tools` | ✅ Enabled |
| Web Search (Serper) | 2 | `serper-tools` | ✅ Enabled |
| Web Search (DDG) | 3 | `search-tools` | ❌ Disabled |
| System | 2 | `system-tools` | ✅ Enabled |
| LED Control | 5 | `led-tools` | ❌ Disabled* |

*LED tools require `[led]` configuration in config.toml.

**System Tools:**
- `get_current_datetime` - Date, time, timezone, ISO 8601, Unix timestamp
- `get_project_context` - Languages, git info, stack detection, key files

**Translation:**
- 50+ languages via translategemma model

**OCR:**
- Text, tables, formulas, figures via glm-ocr model
- Specialized for text extraction (separate from vision module)

**Documentation:**
- Man page
- mdBook documentation
- AGENTS.md integration

**Build & Distribution:**
- Linux x86_64 builds (default and all-tools)
- Termux/Android builds (aarch64-linux-android)
- GitHub Releases automation

---

## Recent Releases

### v0.26.7 (2026-03-09)

**Dead Code Cleanup:**
- Removed unused constants (`MIN_PRESERVE_LAST`)
- Removed unused functions (`count_embedded_messages`, `get_message_chunks`, etc.)
- Removed legacy functions (`set_compacted_summary`, `clear_compacted_summary`, etc.)
- Converted test-only methods to `#[cfg(test)]`
- Fixed `#[allow(dead_code)]` annotations

### v0.26.6 (2026-03-08)

**Integration Tests for Context Overflow:**
- 22 integration tests for overflow protection
- Tests for threshold hierarchy, Unicode truncation, recovery cycles

**Bug Fix:**
- Context builder panic after `/compact` + `/clear`

### v0.26.5 (2026-03-08)

**Error Recovery During Tool Execution:**
- Detects "Context overflow during tool execution" error
- Removes failed messages, auto-compacts, prompts retry

**Pre-Tool Context Check:**
- Checks context at 75% threshold before tool execution
- Auto-compacts if needed to prevent overflow

**Bug Fixes:**
- `/undo` now deletes embeddings from database
- Code mode (-c flag) now works in chat
- Hybrid search supports `exclude_ids` parameter

### v0.26.4 (2026-03-08)

**Token Estimation in Coordinator:**
- Context overflow detection during tool execution (90% threshold)

**Unicode-Safe Tool Result Truncation:**
- `truncate_tool_result()` with charset-safe truncation
- `MAX_TOOL_RESULT_TOKENS = 4000` limit

### v0.26.3 (2026-03-08)

**Bug Fixes:**
- `/undo` deletes embeddings from database
- Fix crash after `/compact` + `/clear`
- Code mode (-c flag) works in chat
- Hybrid search `exclude_ids` parameter

### v0.26.2 (2026-03-05)

**Bug Fix:**
- Token count mismatch between `/context` and Ollama's `prompt_eval_count`

### v0.26.1 (2026-03-04)

**Feature:**
- Source attribution in memory system
- `SourceType` enum with `[msg:N]`, `[doc:N]`, `[note:N]` prefixes

---

## Known Issues

### GLM-OCR Returns Empty Output ✅

**Status:** Fixed in Ollama v0.17.6 (2026-03-04)

Users on rolling-release distros may need to wait for package updates. For immediate fix:
```bash
curl -fsSL https://ollama.com/install.sh | sh
```

### Semantic Retrieval Context Framing ✅

**Status:** Completed (v0.22.9)

Fixed with framing text, MEMORY section in system prompt, and explicit instructions.

---

## Critical Bugs (All Fixed)

All critical bugs have been resolved in v0.26.2 - v0.26.7:

| Bug | Status | Version |
|-----|--------|---------|
| Context token count mismatch | ✅ Fixed | v0.26.2 |
| `/undo` incomplete cleanup | ✅ Fixed | v0.26.3 |
| User prompt in hybrid search | ✅ Fixed | v0.26.3 |
| Code mode (-c) not working | ✅ Fixed | v0.26.3 |
| Context panic after `/compact` + `/clear` | ✅ Fixed | v0.26.6 |
| Context exhaustion during tools | ✅ Fixed | v0.26.4-v0.26.6 |

---

## Pending Items

### Context Builder Panic After /compact (without /clear)

**Status:** ⚠️ Needs Reproduction

**Problem:**
```
thread 'main' panicked at src/retrieval/context_builder.rs:318:51:
range start index 2 out of range for slice of length 1
```

**Possible Causes:**
1. Race condition during `/compact` execution
2. Session modified between CompactionSuggestion creation and use

### Compaction Visual Indicator

**Status:** Planned

When context threshold is reached:
- User message should be saved temporarily
- Compaction should run
- Visual indicator ("Compacting context...") should show

### Context Not Cleared After /compact

**Status:** Under Analysis

After `/compact`, context remains in overflow state. `/clear` is needed.

### Markdown in Compaction Summary

**Status:** Under Analysis

Context compaction summary does not produce markdown output.

### Web Scraping Content Quality

**Status:** Under Analysis

Web fetch tool sometimes returns raw HTML/CSS instead of clean markdown.

---

## SQLite as Single Storage

**Priority:** HIGH  
**Status:** 🟡 PLANNED (v0.27.0)

**Goal:** Migrate from dual storage (JSON + SQLite) to SQLite as the single source of truth.

### Current State

| Storage | Location | Content |
|---------|----------|---------|
| JSON | `~/.local/share/ask-ai/conversations/<project>/<session>.json` | Full session state |
| SQLite | `~/.local/share/ask-ai/ask-ai.db` | Messages for RAG |

**Problems:**
1. **Redundancy** - Messages stored in both places
2. **Inconsistency** - JSON and SQLite can diverge
3. **Confusion** - `/migrate` imports but doesn't clean up
4. **Orphaned data** - JSONs remain after migration

### Target State

| Storage | Content |
|---------|---------|
| SQLite | Full session state (messages + metadata + todos) |
| JSON (export only) | Backup/sharing format |

**Benefits:**
1. **Single source of truth** - No more sync issues
2. **ACID transactions** - Atomic saves
3. **Better performance** - SQLite faster than filesystem
4. **RAG access** - Compact messages still searchable

### Implementation Plan

#### Phase 1: Schema Migration (2h)

**Goal:** Extend SQLite schema to store all session metadata.

**Schema Changes:**

```sql
-- Version 4
ALTER TABLE conversations ADD COLUMN system_prompt TEXT;
ALTER TABLE conversations ADD COLUMN compacted_summary TEXT;
ALTER TABLE conversations ADD COLUMN compacted_range_start INTEGER;
ALTER TABLE conversations ADD COLUMN compacted_range_end INTEGER;
ALTER TABLE conversations ADD COLUMN think INTEGER DEFAULT 0;
ALTER TABLE conversations ADD COLUMN tools INTEGER DEFAULT 1;
ALTER TABLE conversations ADD COLUMN tool_output_level TEXT DEFAULT 'compact';

ALTER TABLE messages ADD COLUMN prompt_tokens INTEGER;

CREATE TABLE session_todos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL,
    task_id INTEGER NOT NULL,
    description TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at INTEGER NOT NULL,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);

CREATE INDEX idx_todos_conversation ON session_todos(conversation_id);
```

**Files Changed:**
- `src/db/schema.rs` - Add schema version 4
- `src/db/operations.rs` - Add CRUD for todos, metadata columns

**Tasks:**
- [ ] Create `SCHEMA_VERSION = 4`
- [ ] Add migration SQL for new columns
- [ ] Add `update_conversation_metadata()` function
- [ ] Add `get_conversation_metadata()` function
- [ ] Add `save_todos()` and `get_todos()` functions
- [ ] Add tests for schema migration

#### Phase 2: ChatSession SQLite-Only (4h)

**Goal:** Remove JSON dependency from ChatSession save/load.

**Changes:**

```rust
// Current (JSON-based)
impl ChatSession {
    pub fn save(&self, storage: &ConversationStorage) -> Result<()> {
        storage.save_session(&self.project_id, &self.id, self)
    }
    pub fn load(storage: &ConversationStorage, ...) -> Result<Self> {
        storage.load_session(project_id, session_id)
    }
}

// New (SQLite-based)
impl ChatSession {
    pub fn save(&self) -> Result<()> {
        let db = self.db.as_ref().ok_or("No database")?;
        db.update_conversation_metadata(&self.id, ...)?;
        db.save_todos(&self.id, &self.todos)?;
        Ok(())
    }
    pub fn load(db: &Arc<Database>, conversation_id: &str) -> Result<Self> {
        let meta = db.get_conversation_metadata(conversation_id)?;
        let messages = db.get_conversation_messages(conversation_id, None)?;
        let todos = db.get_todos(conversation_id)?;
        Self::reconstruct(meta, messages, todos, db)
    }
}
```

**Files Changed:**
- `src/chat/session.rs` - Remove `ConversationStorage` dependency
- `src/chat/history.rs` - Deprecate or move to `legacy/`
- `src/db/operations.rs` - New metadata functions

**Tasks:**
- [ ] Implement `save()` using SQLite only
- [ ] Implement `load()` using SQLite only
- [ ] Ensure messages already saved to SQLite (current behavior)
- [ ] Add `ensure_conversation_exists()` before save
- [ ] Update `save()` to persist metadata + todos
- [ ] Test session persistence roundtrip

#### Phase 3: Restore Command + Legacy Detection (2h)

**Goal:** Replace `/migrate` with `/restore` and auto-detect legacy sessions.

**New Command:**
```
/restore <file>    Restore session from JSON backup file
```

**Behavior:**
1. Read JSON file (backup or legacy)
2. Validate structure
3. Import conversation metadata to SQLite
4. Import messages to SQLite (skip duplicates by timestamp)
5. Import todos to SQLite
6. Show success/failure
7. **If import successful:** Delete JSON file

**Legacy Detection:**
On REPL startup, check for uncommitted JSON sessions:
```
[!] Found 3 uncommitted session(s): session1, session2, default
[!] Use /restore <file> to import them.
[!] JSONs will be deleted after successful import.
```

**Implementation:**
```rust
// src/db/legacy_check.rs (new module)
pub fn check_legacy_sessions(
    db: &Database,
    storage: &ConversationStorage,
    project_id: &Option<String>,
) -> Option<Vec<String>> {
    let json_sessions = storage.list_sessions(project_id);
    let sqlite_sessions = db.list_conversations().unwrap_or_default();
    
    let uncommitted: Vec<String> = json_sessions
        .iter()
        .filter(|s| !sqlite_sessions.contains(&s.id))
        .map(|s| s.id.clone())
        .collect();
    
    if !uncommitted.is_empty() { Some(uncommitted) } else { None }
}

pub fn restore_session(
    db: &Database,
    json_path: &Path,
) -> Result<(), String> {
    let session = read_session_from_json(json_path)?;
    migrate_session_to_sqlite(&session, db)?;
    std::fs::remove_file(json_path)?;
    Ok(())
}
```

**Files Changed:**
- `src/db/legacy_check.rs` - New module
- `src/db/mod.rs` - Export legacy_check
- `src/chat/commands.rs` - Remove `Migrate` command, add `Restore`
- `src/chat/repl.rs` - Add legacy detection on startup

**Tasks:**
- [ ] Create `src/db/legacy_check.rs`
- [ ] Implement `check_legacy_sessions()`
- [ ] Implement `read_session_from_json()`
- [ ] Implement `migrate_session_to_sqlite()`
- [ ] Add `Restore` command in `ChatCommand` enum
- [ ] Add legacy detection to REPL startup
- [ ] Remove `Migrate` command (keep internal function)
- [ ] Update help text

#### Phase 4: Update Commands (2h)

**Goal:** Commands use SQLite instead of JSON.

| Command | Current | New |
|---------|---------|-----|
| `/save [name]` | `storage.save_session()` | `session.save()` (SQLite) |
| `/load <name>` | `ChatSession::load(storage, ...)` | `ChatSession::load(&db, id)` |
| `/list` | `storage.list_sessions()` | `db.list_conversations()` |
| `/export json` | Same (exports ChatSession) | Same |
| `/forget` | Delete JSON + SQLite | SQLite only |

**Files Changed:**
- `src/chat/commands.rs` - Remove `storage` parameter, use `db` from session
- `src/chat/repl.rs` - Update `execute_command()` calls

**Tasks:**
- [ ] Update `execute_command()` signature (remove `storage`)
- [ ] Update `/save` to use SQLite
- [ ] Update `/load` to load from SQLite
- [ ] Update `/list` to query SQLite
- [ ] Update `/forget` to use SQLite only
- [ ] Test all commands

#### Phase 5: Testing (2h)

**Goal:** Verify all functionality works with SQLite-only storage.

**Test Cases:**

1. **Migration Test:**
```rust
#[test]
fn test_migrate_legacy_json_to_sqlite() {
    // Create JSON session manually
    let legacy_session = create_test_session();
    let json = serde_json::to_string(&legacy_session).unwrap();
    
    // Write to legacy location
    let json_path = storage.session_path(&project_id, "test-session");
    std::fs::write(&json_path, &json).unwrap();
    
    // Restore from JSON
    let db = Database::in_memory().unwrap();
    restore_session(&db, &json_path).unwrap();
    
    // Verify SQLite has all data
    let loaded = ChatSession::load(&db, "test-session").unwrap();
    assert_eq!(loaded.think, legacy_session.think);
    assert_eq!(loaded.tools, legacy_session.tools);
    assert_eq!(loaded.todos.tasks.len(), legacy_session.todos.tasks.len());
    
    // Verify JSON was deleted
    assert!(!json_path.exists());
}
```

2. **Session Roundtrip:**
```rust
#[test]
fn test_session_sqlite_roundtrip() {
    let db = Database::in_memory().unwrap();
    
    let mut session = ChatSession::new("test-model".into(), None, false);
    session.think = true;
    session.tools = false;
    session.add_user_message("Hello".into());
    session.add_assistant_message("Hi!".into());
    session.set_compacted_summary_with_range("Summary".into(), Some((0, 1)));
    
    // Save
    session.save().unwrap();
    
    // Load
    let loaded = ChatSession::load(&db, &session.id).unwrap();
    
    assert_eq!(loaded.model, "test-model");
    assert_eq!(loaded.think, true);
    assert_eq!(loaded.tools, false);
    assert_eq!(loaded.messages.len(), 2);
    assert!(loaded.compacted_summary.is_some());
}
```

3. **Todo Persistence:**
```rust
#[test]
fn test_todos_persist_in_sqlite() {
    let db = Database::in_memory().unwrap();
    let mut session = ChatSession::new("model".into(), None, false);
    
    session.todos.add_task("Task 1".into());
    session.todos.add_task("Task 2".into());
    session.save().unwrap();
    
    let loaded = ChatSession::load(&db, &session.id).unwrap();
    assert_eq!(loaded.todos.tasks.len(), 2);
}
```

4. **Compaction + RAG:**
```rust
#[test]
fn test_compaction_preserves_rag_access() {
    let db = Database::in_memory().unwrap();
    let mut session = ChatSession::new("model".into(), None, false);
    session.db = Some(Arc::new(db.clone()));
    
    // Add many messages
    for i in 0..20 {
        session.add_user_message(format!("Message {}", i).into());
    }
    
    // Compact middle
    session.set_compacted_summary_with_range("Summary".into(), Some((5, 15)));
    session.save().unwrap();
    
    // Search should still find compacted messages
    let results = db.search_keyword("Message", None, None, 10).unwrap();
    assert!(results.len() >= 10); // Compacted messages still searchable
}
```

**Tasks:**
- [ ] Write migration test
- [ ] Write session roundtrip test
- [ ] Write todo persistence test
- [ ] Write compaction + RAG test
- [ ] Add integration test for `/restore` command

#### Phase 6: Cleanup (1h)

**Goal:** Remove deprecated code.

**Tasks:**
- [ ] Remove `src/chat/history.rs` or mark as deprecated
- [ ] Remove `ConversationStorage` parameter from `execute_command()`
- [ ] Update all callers in `src/chat/repl.rs`
- [ ] Dead code elimination for unused JSON functions
- [ ] Update documentation

---

## High Priority

### Memory Enhancement Part 1 (Phases 1-3)

**Priority:** HIGH  
**Status:** Phase 1 Complete, Phase 2-3 Research Needed

#### Phase 1: Source Attribution ✅ (v0.26.1)

- ✅ `SourceType` enum with `prefix()` and `from_prefix()` methods
- ✅ Context formatted with source labels: `[msg:N]`, `[doc:N]`, `[note:N]`
- ✅ `remember` tool updated to use source type prefixes

#### Phase 2: Query Routing Research

**Goal:** Route queries to appropriate search targets.

**Tasks:**
- [ ] Collect real query patterns from usage
- [ ] Prototype regex routing (pt-BR + en)
- [ ] Benchmark embedding-based routing latency
- [ ] Test `whatlang` crate for language detection

#### Phase 3: Timestamp Filtering (1 day)

**Goal:** Filter results by time ("what did I say yesterday?").

**Tasks:**
- [ ] Add `timestamp_range: Option<(i64, i64)>` to `search_hybrid()`
- [ ] Implement temporal reference detection (pt-BR + en)
- [ ] Add SQL WHERE clause for timestamp filtering

---

### Chat Module Integration

**Priority:** HIGH  
**Status:** Planning needed

**Problem:** Users must exit chat to use OCR, Vision, Translate, Summarize features.

**Proposed Features:**
- `/ocr <image>` - Run OCR from chat
- `/vision <image>` - Analyze image
- `/translate <lang> <text>` - Translate
- `/summarize [text]` - Summarize

**Tasks:**
- [ ] Design: Command interface
- [ ] Design: Model switching during commands
- [ ] Implement: `/ocr` command
- [ ] Implement: `/vision` command
- [ ] Implement: `/translate` command
- [ ] Document: Chat module commands

---

### Memory Enhancement Part 2 (Phases 4-5)

**Priority:** HIGH  
**Status:** Blocked by Chat Module Integration

#### Phase 4: Schema Preparation (1-2 days)

**Goal:** Prepare schema for multiple source types.

**Tasks:**
- [ ] Design `documents` table schema
- [ ] Create `document_embeddings` virtual table
- [ ] Update `SearchTarget` enum
- [ ] Add `source_type` filter to hybrid search

#### Phase 5: Document Ingestion

**Goal:** Ingest and index external documents.

**Tasks:**
- [ ] Research: PDF parsing crates (pdf-extract, lopdf)
- [ ] Implement: Document chunking with overlap
- [ ] Implement: Embedding generation for chunks
- [ ] Implement: `/ingest <path>` command

---

### SOUL.md Support (Personality System)

**Priority:** HIGH  
**Status:** Research Needed

**Goal:** Support for `SOUL.md` file to define LLM personality.

**Tasks:**
- [ ] Research: SOUL.md format in other agents
- [ ] Research: Best injection point in system prompt
- [ ] Design: Config option for custom SOUL.md path
- [ ] Implement: SOUL.md file loading
- [ ] Implement: Inject into system prompt

---

### Smart Model List (Installed Only)

**Priority:** MEDIUM  
**Status:** Proposed

**Goal:** Show only installed/available models.

**Tasks:**
- [ ] Implement: `is_model_installed(model_id)` function
- [ ] Modify: `/list` command to filter installed models
- [ ] Modify: Model switch to validate before loading
- [ ] Modify: Chat startup with fallback chain

---

## Medium Priority

### File Session State

**Priority:** Medium  
**Status:** Research needed

**Goal:** Explicit tracking of file operations for context reduction and security.

**Tasks:**
- [ ] Research: File tracking patterns
- [ ] Design: Session state structure
- [ ] Implement: File tracking in session
- [ ] Implement: Security constraints

---

### System Tools - run_command

**Priority:** Medium  
**Status:** Blocked by security concerns

**Tasks:**
- [ ] Research: Secure command execution
- [ ] Design: Whitelist configuration
- [ ] Implement: Security constraints

---

### SQL ORM Evaluation

**Priority:** MEDIUM  
**Status:** Research Needed

**Tasks:**
- [ ] Research: `sqlx` vs `sea-orm` trade-offs
- [ ] Benchmark: Binary size impact
- [ ] Prototype: ORM for simple queries

---

### Skills System

**Priority:** Medium  
**Status:** Research needed

**Goal:** Load custom behaviors from `.ask-ai/skills/` or `~/.config/ask-ai/skills/`.

**Tasks:**
- [ ] Research: Skill systems in other agents
- [ ] Design: Skill file format
- [ ] Implement: Skill parser
- [ ] Implement: `--skill` flag

---

## Low Priority

### OCR Model Customization

**Priority:** Low  
**Status:** Resolved (Ollama v0.17.6)

GLM-OCR fix available in Ollama v0.17.6+.

### Plugin System

**Priority:** Low  
**Status:** Not started

User-defined tools via dynamic loading or compilation.

---

### OpenAPI Compatibility (Direct API Access)

**Priority:** LOW  
**Status:** Research Needed

**Goal:** Support direct interaction with OpenAI-compatible APIs.

**Tasks:**
- [ ] Research: Required API endpoints
- [ ] Design: Provider trait/interface
- [ ] Implement: OpenAI provider
- [ ] Implement: LM Studio provider

---

### TUI (Terminal User Interface)

**Priority:** Low  
**Status:** Research & Planning needed

**Goal:** Build a responsive TUI using Ratatui-rs.

**Tasks:**
- [ ] Research: Ratatui-rs best practices
- [ ] Research: Terminal resize handling patterns
- [ ] Design: UX wireframes for main views
- [ ] Prototype: Basic TUI skeleton

---

### Multilingual Injection Detection

**Priority:** Low  
**Status:** Not started

**Tasks:**
- [ ] Research: Injection patterns in non-English languages
- [ ] Implement: Multilingual pattern detection

---

## Future Tools

### Document Processing
- PDF text extraction
- Document format conversion
- Batch processing

### Data Analysis
- CSV/JSON analysis
- Statistical tools
- ASCII visualization

### Code Tools
- Repository analysis
- Code quality checks
- Documentation generation

---

## Testing

### Test Coverage
- [ ] Unit tests for all commands
- [ ] Integration tests with mock Ollama
- [ ] Tool testing framework
- [ ] OCR/Translation testing

### CI/CD
- [x] GitHub Actions for testing
- [x] Release automation
- [ ] Documentation deployment

---

## Contributing

1. Check [GitHub Issues](https://github.com/luksamuk/ask-ai-rs/issues)
2. Comment on the issue
3. Submit a pull request

## See Also

- [Architecture](./architecture.md) - Technical architecture
- [Contributing](./contributing.md) - How to contribute
- [CHANGELOG](../CHANGELOG.md) - Version history