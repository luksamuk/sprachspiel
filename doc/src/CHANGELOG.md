# Changelog

All notable changes to Ask-AI will be documented in this file.

## [0.31.0] - 2026-03-12

### Added

- **Context Continuity with Graceful Interruption** - Full implementation of LLM pause/resume during context overflow
  - `ContextStatus` injected into prompts when approaching limits (>72% usage)
  - `CONTEXT_MANAGEMENT_INSTRUCTION` teaches LLM to emit `<continuation_needed>` tag
  - `ContinuationTag` struct for parsing pause/checkpoint information
  - `parse_continuation_tag()` function extracts and strips continuation tags from responses
  - `ephemeral_messages` in `CustomCoordinator` for non-persisted continuation prompts
  - `ChatEvent::ContinuationNeeded` variant for event-based handling
  - `SendMessageResult.continuation_needed` field for continuation detection
  - `build_continuation_prompt()` creates resume instructions from checkpoint
  - Continuation loop in REPL automatically resumes after compaction
  - Supports nested continuations (up to 3) for extreme context pressure
  - Merges continuation responses with original for seamless output

- **Prompt Configuration**
  - `PromptConfig.context_status` field for injecting context usage
  - Context status section shows usage % and critical/warning indicators
  - Context management instructions when overflow is detected

### Fixed

- **Landlock Sandbox E2BIG Error** - Fixed crash when running multiple commands
  - Added thread-local tracking to prevent stacking Landlock rulesets
  - E2BIG error now treated as success (thread already sandboxed)
  - Documented 16-layer limit in Kernel Landlock API
  - Prevents "Argument list too long" errors after ~16 command executions

### Changed

- `ContextStatus::max_tokens()` - New method to get context window size
- `build_request()` in `CustomCoordinator` now prepends ephemeral messages
- `send_message()` now accepts optional `continuation_tag` parameter for resume
- REPL continuation handling merges responses and accumulates token metrics

## [0.30.0] - 2026-03-12

### Added

- **PreToolContent Persistence** - Intermediate assistant messages (generated before tool calls) are now saved for semantic search
  - `SavedMessage.message_type` field distinguishes `"normal"` vs `"pre_tool_content"` messages
  - `previous_message_id` links pre-tool content back to the user question
  - `subsequent_messages` in search results shows follow-up messages contextually
  - Navigation hints in `remember` tool output

- **Database Schema v5** - New columns for message metadata
  - `message_type TEXT DEFAULT 'normal'` - Distinguishes normal vs intermediate messages
  - `previous_message_id INTEGER` - Links assistant messages to preceding user message

- **Session Methods**
  - `add_pre_tool_message()` - Stores pre-tool content with `previous_message_id` linkage
  - `add_user_message()` now returns `Option<i64>` (message ID) for linking

- **Database Methods**
  - `update_message_previous_id()` - Sets previous_message_id for navigation
  - `get_conversation_messages()` now includes `message_type` column

- **MEMORY TOOLS Navigation** - Enhanced prompt section with navigation instructions
  - Explains `previous_message_id` and `subsequent_messages` fields
  - Guides LLM on contextual message navigation

### Changed

- **`remember` Tool Output** - Shows `message_type` indicator for intermediate messages
  - `[Intermediate]` prefix for `pre_tool_content` messages
  - Subsequent messages displayed with proper indentation

- **`get_conversation_messages()`** - Now retrieves `message_type` column from database

- **Retrieval Enrichment** - `subsequent_messages` includes `message_type` for each message

## [0.29.0] - 2026-03-11

### Breaking Changes

- **SOUL.md Personality System** - User-configurable agent personality replaces hardcoded Pepe personality
  - `~/.config/ask-ai/SOUL.md` defines agent identity, behavior, and limits
  - Falls back to `PERSONALITY_DEFAULT` when no SOUL.md exists
  - Use `--soulless` flag to skip personality entirely
  - **Removed:** Pepe personality (`PERSONALITY_PEPE`) - users should create their own SOUL.md

### Added

- **SOUL.md Module** (`src/soul.rs`)
  - Loads personality from `~/.config/ask-ai/SOUL.md` or `XDG_CONFIG_HOME/ask-ai/SOUL.md`
  - Removes HTML comments (`<!-- ... -->`) for developer notes
  - Normalizes whitespace
  - Validates structure (requires at least one `## ` section)

- **PERSONALITY_DEFAULT** - Fallback personality when SOUL.md is missing

- **`--soulless` CLI Flag** - Skip personality layer for neutral responses
  - Available for `chat` and `query` commands
  - Useful for debugging or when personality is not desired

- **Multiple Personalities** - Documentation for switching between personality files
  - See `doc/src/soul.md` for example personalities (PEPE, SPRACH, ANGEMON)
  - Symlink or copy approach for switching

- **Example Personalities** - Three complete example personalities in documentation:
  - **SPRACH** - Cognitive companion for research and Zettelkasten work
  - **PEPE** - Sarcastic senior developer (replaces hardcoded Pepe)
  - **ANGEMON** - Guardian administrator for system protection

- **Documentation** - New `doc/src/soul.md` with examples and best practices
  - Updated `doc/src/commands/chat.md` and `doc/src/commands/query.md`

### Changed

- **Prompt Assembly** - New layered architecture:
  1. SOUL LAYER (SOUL.md or PERSONALITY_DEFAULT or empty if --soulless)
  2. OPERATION LAYER (Role + Behavior + Tool Usage)
  3. CONTEXT LAYER (Platform + System + AGENTS.md)
  4. CAPABILITY LAYER (Tools + Memory + Examples)
  5. FINAL INSTRUCTION

- **Removed `src/prompts/personality.rs`** - Pepe personality code deleted

- **Updated `src/prompts/mod.rs`** - Removed personality exports, added PERSONALITY_DEFAULT export

### Migration Guide

If you used Pepe personality before, create `~/.config/ask-ai/SOUL.md` with your desired personality.

Example personalities are available in `doc/src/soul.md`:
- **SPRACH** - Thoughtful research companion
- **PEPE** - Sarcastic senior developer
- **ANGEMON** - Guardian administrator

See the [SOUL.md documentation](./soul.md) for complete examples and best practices.

## [0.28.0] - 2026-03-11

### Fixed

- **CRITICAL: run_command Parameter Types** - Fixed crash when LLM sends strings for numeric parameters
  - Changed `head`, `tail`, `timeout_seconds` from `Option<usize>`/`Option<u32>` to `Option<String>`
  - LLMs frequently send `"null"` (string) instead of `null` (JSON), causing deserialization failures
  - Internal parsing with `.parse().ok()` handles all variations (`"5"`, `5`, `"null"`, `null`, `""`)
  - Added `"CRITICAL: Parameter Types for LLM Tools"` section to AGENTS.md
  - Updated doc/src/tools.md with guidance on parameter types

- **run_command Timeout Implementation** - Processes now properly killed on timeout
  - Replaced `std::process::Command` with `tokio::process::Command`
  - Added `.kill_on_drop(true)` to ensure process termination
  - Implemented `tokio::time::timeout` wrapper
  - Timeout error messages include actionable suggestions
  - Added unit tests for timeout functionality and string parameter parsing

- **Landlock API Deprecation** - Updated to new Ruleset API
  - Changed `Ruleset::new()` to `Ruleset::default()` (deprecated warning fixed)

### Changed

- **Code Cleanup** - Removed unused code and fixed all clippy warnings
  - Deleted `src/external/executor.rs` (CommandExecutor never used)
  - Deleted `src/external/registry.rs` (only used by executor.rs)
  - Removed `CommandError` and `ToolAvailability` unused types from types.rs
  - Fixed 15+ clippy warnings: `collapsible_if`, `needless_question_mark`, `redundant_locals`, `map_clone`, `let_and_return`, `io_other_error`, `needless_borrow`, `manual_clamp`, `redundant_async_block`

- **API Refactoring** - Improved function signatures for maintainability
  - Created `SearchParams` struct for `search_hybrid()` parameters (9 args → 1 struct)
  - Created `ConversationMetadataParams` struct for `update_conversation_metadata()` (10 args → 1 struct)
  - Both structs exported from `src/db/mod.rs` for external use

### Added

- **Documentation** - Added LLM tool parameter type guidelines
  - `AGENTS.md`: New section "CRITICAL: Parameter Types for LLM Tools" explaining why `Option<String>` is required
  - `doc/src/tools.md`: Updated Tool Error Handling section with guidance
  - Reference tables showing dangerous vs correct parameter types

- **Unit Tests** - Test coverage for timeout and parameter parsing
  - `test_timeout_kills_long_running_command` - Verifies process killed on timeout
  - `test_timeout_allows_fast_command` - Verifies normal execution within timeout
  - `test_timeout_error_message_format` - Verifies error message structure
  - `test_string_parameter_parsing` - Verifies string-to-number conversion

- **Code Organization** - SQLite cleanup
  - Created `src/project.rs` with `get_project_id()` and `normalize_git_url()`
  - Updated `history.rs` to be purely a migration module (deprecated)
  - Clear separation: project identification vs. legacy storage
  - `history.rs` still contains `ConversationStorage` for `/restore` command

- **User Documentation** - Updated storage model
  - `doc/src/commands/chat.md`: Updated session storage documentation
  - Clarified SQLite as primary storage, JSON for backup/restore only
  - Added `/restore` command documentation
  - Added database tables explanation

## [0.27.3] - 2026-03-09

### Added

- **Compaction Visual Indicator** - Clear feedback during context compaction
  - Shows yellow "⏳ Compacting context (X% full)..." before starting
  - Shows dimmed result: "[auto/urgent-compacted: N messages summarized]"
  - `/compact` command now shows checkmark "✓ Compacted" on success
  - Error messages show "✗ Compaction failed" in red

### Fixed

- **Context Not Cleared After /compact** - Token count now correctly reflects reduced context
  - `prompt_tokens` are cleared from messages after compaction
  - Next interaction will have fresh token count
  - `/context` now shows accurate reduced token usage after `/compact`

- **Markdown in Compaction Summary** - Summary now renders in markdown format
  - Compaction prompt requests structured markdown output
  - Uses `print_markdown()` for proper rendering in terminal
  - Sections include: Key Topics, Decisions Made, Technical Details, Action Items

- **Web Scraping Content Quality** - Improved HTML-to-markdown conversion
  - Extracts main content area (`<main>`, `<article>`, etc.) when available
  - Prioritizes semantic content over navigation/sidebars
  - Limits content to 50,000 characters to prevent memory issues
  - Safe UTF-8 boundary handling for Unicode content
  - Shows "(truncated)" indicator when content is limited

### Changed

- **Roadmap Reorganization** - Memory Enhancement Phases 2-3 moved to Blocked
  - Phase 2 (Query Routing) blocked by Document Import Tool + Notes System
  - Phase 3 (Timestamp Filtering) blocked by Phase 2
  - New priorities: Document Import Tool and Notes System first
  - Chat Module Integration remains high priority

## [0.27.1] - 2026-03-09

### Fixed

- **Token Count Bug** - Fixed incorrect token calculation in `/context` display
  - `history_real_tokens()` now uses the LAST cumulative `prompt_tokens` value (Ollama's `prompt_eval_count`)
  - Previous code incorrectly SUMMED all `prompt_tokens` values, causing ~184K tokens when actual was ~22K
  - `check_context_overflow()` now correctly handles fallback path (includes tools estimate)
  - Context status simplified to "OK", "MODERATE", "CRITICAL" (removed confusing "auto-compact triggered")

### Fixed

- **Token Persistence** - Added `prompt_tokens` column to messages table
  - Messages now store `prompt_eval_count` from Ollama responses
  - Token counts persist across sessions
  - `/context` shows accurate token usage on startup

### Added

- **Automatic JSON Migration** - One-time automatic migration on startup
  - Detects all JSON sessions in `~/.local/share/ask-ai/conversations/`
  - Migrates sessions not yet in SQLite (with embeddings)
  - Archives ALL JSON files to `~/.local/share/ask-ai/archived/`
  - Removes empty project directories
  - Does NOT touch `OLD/` directory

### Changed

- **SQLite-Only Storage** - Removed dual-write to JSON files
  - `/save` and `/load` now use SQLite exclusively
  - Removed `/migrate` command (automatic migration replaces it)
  - `/restore <id>` imported from JSON as disaster recovery option

### Removed

- **Dead Code Cleanup**
  - Removed `migrate_project()` function (replaced by automatic migration)
  - Deprecated `Session.save()` (JSON) in favor of `Session.save_sqlite()`

## [0.26.8] - 2026-03-09

### Fixed

- **Context Utilization After Compaction** - Fixed token count calculation after `/compact`
  - `history_real_tokens()` now skips compacted messages
  - `check_context_overflow()` now respects `messages_sent_to_llm`
  - `/context` display now shows correct active messages and summary tokens
  - Context utilization bar reflects post-compaction state

### Details

Before this fix, `/context` showed incorrect token counts after compaction:
- Counted ALL messages (including compacted ones)
- Showed 100%+ utilization even after successful compaction
- Displayed wrong message count

Now correctly calculates:
- Tokens from summary + active messages only
- Skips messages before `messages_sent_to_llm`
- Shows summary token estimate in output

## [0.27.0] - PLANNED

### Changed

**SQLite as Single Storage** - Major architecture change

This release consolidates session storage from dual (JSON + SQLite) to SQLite-only, improving reliability and eliminating data synchronization issues.

#### Architecture Changes

- **Session storage** - Moved from JSON files to SQLite database
  - Session metadata (model, think, tools, system_prompt) now in `conversations` table
  - Todo list state now in `session_todos` table
  - Compaction metadata (summary, range) persisted in database
  - Messages remain in SQLite (no change)

- **Commands updated** - All session commands now use SQLite
  - `/save` - Saves to SQLite only (no more JSON)
  - `/load` - Loads from SQLite only
  - `/list` - Queries SQLite for sessions
  - `/forget` - Deletes from SQLite only

#### New Features

- **`/restore <file>`** - Restore session from JSON backup
  - Imports backup files (from `/export json`)
  - Deletes JSON after successful import
  - Useful for disaster recovery

- **Legacy session detection** - Automatic notification on startup
  - Detects uncommitted JSON sessions
  - Prompts to use `/restore` command

#### Removed

- **`/migrate` command** - Replaced by `/restore` and auto-detection
  - Legacy JSONs now imported via `/restore <file>`
  - Automatic detection on startup replaces manual migration

#### Schema Changes

- Added `conversations` table columns:
  - `system_prompt TEXT`
  - `compacted_summary TEXT`
  - `compacted_range_start INTEGER`
  - `compacted_range_end INTEGER`
  - `think INTEGER DEFAULT 0`
  - `tools INTEGER DEFAULT 1`
  - `tool_output_level TEXT DEFAULT 'compact'`

- Added `messages` table column:
  - `prompt_tokens INTEGER`

- Added `session_todos` table for task tracking

#### Benefits

- **Reliability** - ACID transactions prevent data corruption
- **Consistency** - Single source of truth, no sync issues
- **Performance** - SQLite faster than filesystem writes
- **RAG Access** - Compacted messages remain searchable

#### Migration

Users with existing JSON sessions will see a notification:
```
[!] Found 3 uncommitted session(s): session1, session2, default
[!] Use /restore <file> to import them.
```

---

## [0.26.7] - 2026-03-09

### Changed

- **Dead Code Cleanup** - Removed unused code for better maintainability
  - Removed `MIN_PRESERVE_LAST` constant from `context_overflow.rs` (only used in tests, now local)
  - Removed unused `count_embedded_messages()` from `db/operations.rs`
  - Removed unused `get_message_chunks()` and `ChunkRow` struct from `db/operations.rs`
  - Removed legacy `set_compacted_summary()` from `chat/session.rs` (replaced by `set_compacted_summary_with_range()`)
  - Removed unused `clear_compacted_summary()` from `chat/session.rs`
  - Removed duplicate `as_chat_messages()` from `chat/session.rs` (same as `get_messages_for_llm()`)
  - Removed unused `set_todo_state()` and `get_todo_copy()` from `tools/todo.rs`
  - Converted test-only methods to `#[cfg(test)]` in `chat/todo_state.rs` (`get()`, `all()`, `count()`, `is_empty()`)
  - Removed `#[allow(dead_code)]` from `to_info()` in `chat/session.rs` (actually used in `history.rs`)

### Notes

- Functions and structs with roadmap justifications were kept:
  - `list_conversations()` - planned for `/reindex all`
  - `embed_batch()`, `with_model()`, `model()`, `embedding_dimension()` - planned for future use
  - `normalize()`, `cosine_similarity()` - planned for MMR/reranking

## [0.26.6] - 2026-03-08

### Added

- **Integration Tests for Context Overflow** - Comprehensive test coverage for overflow protection
  - `tests/context_recovery_flow.rs` - 9 integration tests
  - `tests/context_tool_overflow.rs` - 13 integration tests
  - Tests for threshold hierarchy, Unicode truncation, recovery cycles
  - Tests for message removal, turn preservation, multiple recovery cycles

### Fixed

- **Context Builder Panic After /compact + /clear** - Session crash fixed
  - `clear_messages()` now resets `compacted_range` to prevent stale indices
  - Added bounds checking in `context_builder.rs` with `.min(session.messages.len())`
  - Prevents "range end index X out of range for slice of length Y" panic

## [0.26.5] - 2026-03-08

### Added

- **Error Recovery During Tool Execution** - Automatic recovery from context overflow
  - Detects "Context overflow during tool execution" error from coordinator
  - Removes failed assistant messages from session
  - Auto-compacts immediately after error
  - Saves session after recovery
  - Prompts user to retry with clear message

- **Pre-Tool Context Check** - Proactive compaction before tool execution
  - Checks context at 75% threshold before creating coordinator
  - Auto-compacts if needed to prevent overflow during tools
  - User message preserved during compaction (already saved)
  - Prevents context exhaustion during multi-tool turns

- **Turn Preservation in Compaction** - Current turn never compacted
  - `MIN_PRESERVE_LAST` constant ensures at least 1 message preserved
  - User message saved before pre-tool check runs
  - Compaction preserves `DEFAULT_KEEP_LAST = 5` recent messages

### Fixed

- **/undo Incomplete Cleanup** - Embeddings now deleted from database
  - Added `delete_last_messages()` function in Database
  - `/undo` calls database cleanup for both messages and embeddings
  - Prevents orphaned embeddings in SQLite

- **User Prompt Included in Hybrid Search** - Current prompt excluded
  - Added `exclude_ids` parameter to `search_hybrid()`
  - Prepared for future use (not yet wired - current message not in DB at search time)

- **Code Mode (-c Flag) Not Working in Chat** - Now functional
  - `cli_code` parameter passed through to `run_chat_repl()`
  - Code mode now correctly disables retrieval and uses code prompts

## [0.26.4] - 2026-03-08

### Added

- **Token Estimation in Coordinator** - Context overflow detection during tool execution
  - `estimate_messages_tokens()` for SavedMessage (session history)
  - `estimate_chat_messages_tokens()` for ChatMessage (coordinator history)
  - `context_window` and `system_prompt` fields added to CustomCoordinator
  - Context check in `process_next()` at 90% threshold
  - Returns clear error when overflow detected during tools

- **Unicode-Safe Tool Result Truncation** - Prevents unbounded context growth
  - `truncate_tool_result()` with `.chars().take()` for Unicode safety
  - `MAX_TOOL_RESULT_TOKENS = 4000` limit for tool results
  - `CHARS_PER_TOKEN = 4` conservative ratio
  - Truncation notice includes original token count
  - Debug logging when truncation occurs

- **Unit Tests** - 7 new tests in `src/context_overflow.rs`
  - Token estimation accuracy tests
  - Unicode truncation tests (Japanese, Chinese, Arabic, Emoji)
  - Threshold hierarchy tests
  - Context status percentage tests

## [0.26.3] - 2026-03-08

### Fixed

- **Multiple Bug Fixes**:
  - `/undo` now deletes embeddings from database (not just memory)
  - Fix crash after `/compact` + `/clear` (reset compacted_range on clear)
  - Add bounds checking in context_builder to prevent panics
  - Code mode (-c flag) now works in chat mode
  - Hybrid search supports `exclude_ids` parameter (prepared for future use)

### Added

- **delete_last_messages()** in Database for proper cleanup

## [0.26.2] - 2026-03-05

### Fixed

- **remember() tool ID display** - Fixed missing source type prefix in search results
  - IDs now properly display as `msg:N` instead of just `N`
  - Affects query results, specific message retrieval, and error messages
  - Prevents confusion when AI tries to use returned IDs in subsequent calls

## [0.26.1] - 2026-03-05

### Added

- **Centralized String Constants Module** - `src/consts/`
  - `roles.rs` - Message role constants (`ROLE_USER`, `ROLE_ASSISTANT`, `ROLE_SYSTEM`, `ROLE_TOOL`)
  - `api.rs` - API URL constants (`OPEN_METEO_BASE`, `OPEN_METEO_GEOCODING`, `SERPER_API_URL`)
  - Helper functions: `format_role_label()`, `format_role_label_md()`

- **AGENTS.md Guidelines** - New "Constants and String Management" section
  - Rules for preventing string duplication
  - Categories of regulated strings (roles, source prefixes, API URLs)
  - Checklist for adding new string literals
  - Policy for rejecting `#[allow(dead_code)]` on constants

### Changed

- **Refactored 11 files to use centralized constants**
  - `retrieval/context_builder.rs` - Use `SourceType::prefix()` dynamically
  - `tools/remember.rs` - Use `format_role_label()` and role constants
  - `retrieval/search.rs` - Use `format_role_label_md()`
  - `tools/weather.rs` - Use `OPEN_METEO_BASE/GEOCODING` constants
  - `tools/serper.rs` - Use `SERPER_API_URL` constant
  - `db/operations.rs` - Use `ROLE_USER/ASSISTANT` constants
  - `db/migration.rs` - Use role constants
  - `chat/session.rs` - Use `ROLE_USER/ASSISTANT` constants
  - Test files updated accordingly

## [0.26.0] - 2026-03-04

### Added

- **Improved Distribution System** - Easier installation for Linux and Termux
  - One-liner installation via curl|bash
  - Intelligent install script with automatic platform detection
  - Manpage installation support for Termux
  - Detailed installation instructions in README-TERMUX.txt

- **Installation Scripts**
  - `scripts/install.sh` - Portable installer with --prefix, --bin, --man options
  - `scripts/uninstall.sh` - Clean uninstallation
  - `scripts/install-ask-ai.sh` - Remote installer for curl|bash one-liner

- **New Makefile Targets**
  - `tarball-linux` - Linux tarball with install scripts
  - `tarball-termux` - Termux tarball with README-TERMUX.txt
  - `all-tarballs` - Create all distribution tarballs

- **Documentation Improvements**
  - Consolidated version history into `implementation-history.md`
  - Integrated `/search` and `/context` commands into `chat.md`
  - Updated installation documentation with one-liner instructions
  - Added architecture diagrams to `architecture.md`
  - New `retrieval-design.md` explaining hybrid search

### Changed

- **Tarball Structure** - Now includes install/uninstall scripts inside
  - `ask-ai.1` manpage renamed from `man/ask-ai.1` to top-level
  - Added `README-TERMUX.txt` for Termux tarballs
  - Install script detects Termux and adjusts default paths

- **Documentation Cleanup**
  - Removed obsolete version plan files (v0.22.x, v0.23.0, v0.24.0, v0.25.0)
  - Removed `context_v2_plan.md` (superseded by `context_composition_design.md`)
  - Removed `markdown_skin_plan.md` (completed, documented elsewhere)
  - Removed separate `search.md` and `context.md` (integrated into `chat.md`)

### Fixed

- **Translate Model Configuration** - Fixed "Translate model configuration not found"
  - Added `translate: SubcommandModelConfig` to settings
  - Falls back to "translategemma" builtin model
  - Sample config now includes `[model.translate]` section

- **Code Cleanup**
  - Removed duplicate `is_led_configured()` function in led.rs
  - Removed unused `SummarizeArgs::get_text()` method
  - Added `normalize_input()` utility for unicode-safe lowercase+trim
  - Fixed stdin duplication by consolidating to `utils::read_stdin()`

### Technical Details

**Install Script Features:**
- Platform detection: Linux, Termux, macOS
- Default paths: `~/.local/bin` (Linux/macOS), `~/bin` (Termux)
- Manpage installation: `~/.local/share/man/man1/ask-ai.1`
- PATH/MANPATH detection and instructions
- Manpage access verification

**Tarball Contents:**
```
ask-ai-VERSION-linux-x86_64.tar.gz
├── ask-ai
├── ask-ai.1
├── install.sh
├── uninstall.sh
├── README.md
└── LICENSE.txt
```

**One-liner Installation:**
```bash
curl -sL https://raw.githubusercontent.com/luksamuk/ask-ai/main/scripts/install-ask-ai.sh | bash
curl -sL ... | bash -s -- --version 0.26.0
curl -sL ... | bash -s -- --tools all
curl -sL ... | bash -s -- --prefix /usr
```

### Files Modified

- `Cargo.toml` - Version bump to 0.26.0
- `Makefile` - New tarball targets with install scripts
- `README.md` - Reorganized installation section
- `doc/src/installation.md` - New installation methods
- `doc/src/commands/chat.md` - Integrated /search and /context
- `doc/src/SUMMARY.md` - Updated structure
- `doc/src/development/implementation-history.md` - NEW: Consolidated decisions
- `doc/src/development/architecture.md` - Major rewrite with diagrams
- `doc/src/development/retrieval-design.md` - NEW: Retrieval system design
- `scripts/install.sh` - NEW: Portable installer
- `scripts/uninstall.sh` - NEW: Uninstaller
- `scripts/install-ask-ai.sh` - NEW: One-liner installer
- `README-TERMUX.txt` - NEW: Termux-specific instructions
- `src/settings.rs` - Added translate model config
- `src/main.rs` - Translate model fallback
- `src/utils.rs` - Added normalize_input()
- `src/summarize/cli.rs` - Removed dead code
- `src/tools/led.rs` - Removed duplicate, use normalize_input()
- `src/tools/calc.rs` - Use normalize_input()
- `src/translate/style.rs` - Use normalize_input()

## [0.25.0] - 2026-03-03

### Added

- **Project-Aware Query Mode** - Query mode now retrieves context from project history
  - Access to all conversations in the project (read-only)
  - Same RAG retrieval as chat mode
  - Same 5-message context limit via RRF
  - Same enrichment with assistant responses

### Changed

- `query` and `legacy` modes initialize DB + EmbeddingClient
- `search_hybrid()` now accepts `project_id` parameter for project-wide search
- Prompt includes MEMORY section when retrieval is available
- `--code` continues without DB/history (unchanged)

### Technical Details

**Problem:** Query mode had no access to conversation history, making it less useful
for quick questions that benefit from project context.

**Solution:** Enable retrieval from project's conversation history using the same
RAG system as chat mode, but without persisting new messages.

**Implementation:**
1. `project_id` determined same way as chat (git remote or folder name)
2. DB + EmbeddingClient initialized for query (except --code)
3. `build_query_context()` retrieves from all sessions in project
4. Task-local context enables remember tool in query
5. Graceful degradation if DB unavailable

**Example:**
```
Query before: [system_prompt] + [user_query]
Query after:  [system_prompt] + [retrieved_context] + [user_query]
                            ↑ from project history (read-only)
```

### Files Modified

- `src/db/operations.rs` - Add `project_id` to `search_hybrid()`
- `src/retrieval/context_builder.rs` - New `build_query_context()` function
- `src/query.rs` - Initialize DB, use context, task-local for remember tool

## [0.24.0] - 2026-03-03

### Added

- **Conversation-Aware Retrieval** - Enrich retrieved user messages with assistant responses
  - `next_message` field in SearchResult for user messages
  - `get_next_message_by_role()` database method
  - `enrich_with_context()` to attach assistant responses to user questions
  - Both auto-context and remember tool use the same enrichment
  - `/search` command shows question-answer pairs together

### Changed

- `SearchResult` struct now has optional `next_message` field
- Context builder formats question-answer pairs together
- Remember tool shows assistant response when retrieving user message

### Technical Details

**Problem:** Short user questions have high semantic similarity (concentrated) while
long assistant responses have low similarity (dispersed). Retrieval returned only
questions, not the answers that contain the actual information.

**Solution:** Post-retrieval enrichment:
1. Retrieve messages as before (semantic + keyword hybrid)
2. For each user message, query DB for next assistant message
3. Include both in context for complete question-answer pairs

**Example:**
```
Before: Retrieved "What about Wittgenstein?" (question only)
After:  Retrieved "What about Wittgenstein?" + Assistant response (complete info)
```

**Token Overhead:** +5 assistant responses (acceptable within 198K context)

### Files Modified

- `src/db/operations.rs` - Add `next_message` field, `get_next_message_by_role()`, `enrich_with_context()`
- `src/retrieval/context_builder.rs` - Enrich results, format question-answer pairs
- `src/tools/remember.rs` - Show assistant response for user messages
- `src/retrieval/search.rs` - Enrich results in search command

## [0.23.0] - 2026-03-03

### Added

- **Remember Tool** - LLM can actively retrieve conversation history
  - `remember(id="42")` - Get full message by ID
  - `remember(query="topic")` - Search by topic
  - Default 5 results, max 10
  - Task-local storage for async-safe DB access (via `tokio::task_local!`)

- **Context Enhancement** - Retrieved messages now show database IDs
  - Messages include `id="N"` attribute
  - Clear framing explains tool usage
  - MEMORY TOOLS section in system prompt

- **Retrieval Enabled by Default** - `retrieval_enabled: true` in new sessions

### Changed

- Retrieved context uses `message_id` instead of enumeration index
- Anonymous sessions don't register the remember tool (no database available)

### Technical Details

The GLM-5:cloud model (198K context) was still responding "I have no memory" after v0.22.9 because:
1. LLM couldn't request MORE context (only received 5 messages)
2. LLM couldn't reference specific messages (no IDs)
3. LLM couldn't search for topics (no tool)

**Solution:** Give the LLM both IDs and an explicit tool to request more context.

**Token Overhead:** ~130 tokens (0.06% of 198K context)

### Implementation Status

**Phase 1: Database**
- [x] Add `get_message_by_id()` to `src/db/operations.rs`

**Phase 2: Task-Local Storage**
- [x] Create `src/tools/context.rs` with `tokio::task_local!`

**Phase 3: Remember Tool**
- [x] Create `src/tools/remember.rs`
- [x] Implement `remember(id)` function
- [x] Implement `remember(query)` function

**Phase 4: Update Retrieved Context**
- [x] Change `context_builder.rs` to use `message_id`
- [x] Update framing text with ID explanation
- [x] Add remember tool usage instructions

**Phase 5: Default Retrieval Enabled**
- [x] Change `retrieval_enabled: false` to `true` in `session.rs`

**Phase 6: Conditional Tool Registration**
- [x] Update `src/tools/mod.rs` to export new modules
- [x] Update `src/tools/registry.rs` for conditional registration
- [x] Add context wrapper in `src/chat/repl.rs`

**Phase 7: MEMORY TOOLS Section**
- [x] Add MEMORY TOOLS section to `src/prompts/builder.rs`

**Phase 8: Testing**
- [x] Update test for `retrieval_enabled: true` default
- [x] All tests pass

**Phase 9: Finalization**
- [x] Update CHANGELOG.md
- [x] Update version in Cargo.toml
- [x] Update version in man page
- [x] Build release binary (pending)

### Files Modified

- `src/db/operations.rs` - Add `get_message_by_id()`
- `src/tools/context.rs` - NEW: Task-local storage
- `src/tools/remember.rs` - NEW: Remember tool
- `src/tools/mod.rs` - Export new modules
- `src/tools/registry.rs` - Register remember tool
- `src/retrieval/context_builder.rs` - ID format + new framing
- `src/prompts/builder.rs` - MEMORY TOOLS section
- `src/chat/session.rs` - Default `retrieval_enabled: true`
- `src/chat/repl.rs` - Context wrapper for remember tool

## [0.22.9] - 2026-03-03

### Fixed

- **Context Framing for Semantic Retrieval** - LLM now understands retrieved context
  - Added explicit framing text in `<retrieved_context>` explaining the context is from the conversation history
  - Added MEMORY section to system prompt explaining the retrieval mechanism
  - Models now correctly reference past conversations after `/clear`

### Technical Details

After v0.22.7, semantic retrieval was working correctly (session ID stable, messages preserved in SQLite, proper detection of post-clear state). However, the LLM still said "I have no memory of previous conversations" because it didn't understand what `<retrieved_context>` represented.

**Solution:**
1. Added framing text (~50 tokens):
   ```
   The following messages are from YOUR conversation history with this user.
   They represent topics you have discussed together earlier.
   Reference these when the user asks about previous topics.
   ```

2. Added MEMORY section in system prompt (~30 tokens):
   ```
   ### MEMORY
   When <retrieved_context> appears in our conversation, it contains 
   messages from our prior conversation. Reference them when the user 
   asks about topics we discussed earlier.
   ```

**Token overhead:** ~80 tokens (0.04% of 198K context for glm-5:cloud)

### Files Modified

- `src/retrieval/context_builder.rs` - Added framing text to retrieved context
- `src/prompts/builder.rs` - Added MEMORY section, `retrieval_enabled` flag
- `src/chat/repl.rs` - Pass retrieval flag to prompt builder
- `src/query.rs` - Pass retrieval flag to prompt builder
- `src/summarize/processor.rs` - Pass retrieval flag (false) for summarize

## [0.22.8] - 2026-03-03

### Added

- **Markdown Skin Configuration** - Theme support for markdown rendering
  - New `src/markdown.rs` module with global skin initialization
  - Supports `dark`, `light`, and `mono` themes from `config.toml`
  - `dark`: Transparent background, optimized for dark terminals
  - `light`: Transparent background, optimized for light terminals
  - `mono`: Monochrome with gray bold/italic, no colors
  - Config: `display.skin = "mono"` in `~/.config/ask-ai/config.toml`

### Changed

- **All markdown output now respects skin setting**
  - `main.rs`: translate, summarize, vision commands
  - `query.rs`: query output
  - `chat/repl.rs`: chat responses
  - `retrieval/search.rs`: search results
  - `thinking.rs`: Keeps its own skin (unaffected by global skin)

### Technical

- Added `markdown::init_markdown_skin()` call at startup
- Created `markdown::print_markdown()` as replacement for `termimad::print_text()`
- Added `markdown::get_markdown_skin()` for custom rendering needs
- All modules now use `markdown::print_markdown()` instead of `print_text()`

### Fixed

- **CRITICAL: Retrieval after /clear now works!**
  - Bug: `should_force_retrieve()` checked if session was empty, but user already added 1+ messages
  - Fix: Compare DB message count vs session message count
  - If DB has more messages than session, retrieval is forced
  - This correctly handles: `/clear` → user asks question → retrieval happens

### Changed

- **`should_force_retrieve()` logic rewritten**
  - Old: Only triggered when session.messages.is_empty()
  - New: Triggers when DB count > session count (after /clear with new messages)
  - Also triggers when session is empty AND has compacted_summary

### Technical

- Added test `test_should_force_retrieve_after_clear_with_new_messages`
- `MIN_RETRIEVAL_FORCE_COUNT` now deprecated (kept with `#[allow(dead_code)]`)

## [0.22.6] - 2026-03-03

### Fixed

- **Retrieval After /clear Debug Logging** - Added comprehensive debug logs
  - Logs show: `enabled`, `should_retrieve`, `force_retrieve` values
  - Logs show: session ID, anonymous status, message count, summary status
  - Logs show: DB and embedding client availability
  - Logs show: search results count
  - Use `/debug` to see detailed retrieval diagnostics

### Changed

- **build_context() parameter** - Added `use_debug: bool` parameter
  - Required for debug logging in context builder
  - Updated call sites in `repl.rs`

## [0.22.5] - 2026-03-03

### Fixed

- **Retrieval After /clear** - Critical bug fixed
  - Model lost all memory after `/clear` even though SQLite data persisted
  - Root causes: retrieval disabled by default, minimum threshold too high (20)
  
### Changed

- **Forced Retrieval After Clear** - Automatic context recovery
  - Added `should_force_retrieve()` function
  - Retrieval activates when session empty but DB has 2+ messages
  - Ignores `retrieval_enabled` flag and `MIN_MESSAGES` threshold
  - Gray system message: `[i] You may ask about previous topics.`

- **Lowered Retrieval Threshold**
  - `MIN_MESSAGES_FOR_RETRIEVAL`: 20 → 5 (more useful for short conversations)
  - `MIN_RETRIEVAL_FORCE_COUNT`: 2 (minimum for forced retrieval)

### Technical

- Modified `build_context()` to check both normal and forced retrieval conditions
- Added `MIN_RETRIEVAL_FORCE_COUNT` constant for post-clear threshold
- Updated `/clear` command to check DB for messages and show appropriate messages

## [0.22.4] - 2026-03-03

### Added

- **Persistent Memory** - Context survives `/clear` command
  - `/clear` and `/new` preserve compacted summary
  - SQLite history intact for RAG retrieval
  - Users can ask about previous topics after clearing

- **/forget Command** - Complete session reset
  - Clears all messages and summary
  - Deletes conversation from SQLite synchronously
  - Generates new session ID
  - Truly fresh start

### Changed

- **/clear Command** - Now preserves context instead of destroying it
  - Messages cleared from session memory
  - Compacted summary preserved for retrieval
  - SQLite conversation history preserved
  
- **should_retrieve()** - Now checks database message count
  - Works even when session.messages is empty (after /clear)
  - Considers both session and database for retrieval decisions

## [0.22.3] - 2026-03-03

### Added

- **Auto-Compaction** - Silent automatic context compaction
  - Triggers at 72% warning threshold
  - Triggers at 80% overflow threshold
  - Runs after assistant response
  - No user confirmation required
  - Shows `[auto-compacted context at 72%]` message in gray

- **Visual Metrics in /context** - Bar chart with colors
  - Green for <72% usage
  - Yellow for 72-80% usage  
  - Red for ≥80% usage
  - Shows token counts and percentage
  - Status text indicates current state

### Changed

- **ContextStatus** - New methods `is_warning()`, `is_overflow()`
- **needs_compaction()** - Now returns true for Warning OR Overflow
- **SendMessageResult** - Now includes `system_prompt` and `context_window` for auto-compact

## [0.22.2] - 2026-03-03

### Fixed

- **Synchronous Chunking** - Chunks are now saved synchronously, guaranteeing persistence
  - Previously: fire-and-forget async could lose chunks if process terminated
  - Now: chunks always saved, embeddings generated asynchronously
  - Addresses stress test finding: 2 of 6 long messages had incomplete chunking

- **Embedding Recovery** - Automatic recovery of missing embeddings on startup
  - New `get_chunks_without_embedding()` database function
  - Recovery runs silently on REPL startup, reports count if recovered
  - Database now has `has_embedding` flag for chunks

- **Middle Compaction** - `/compact` now preserves first N + last N messages
  - Previously: summarized ALL messages, losing important context
  - Now: preserves first 5 and last 5 messages, summarizes middle
  - Based on "Lost in the Middle" research for optimal LLM performance

### Changed

- **Context Builder** - New context order for middle compaction
  - Order: System → Retrieved → First N → Summary → Recent → Query
  - Uses `compacted_range` for middle compaction context
  - Falls back to `messages_sent_to_llm` for legacy sessions

- **Database Schema** - Version bumped to 3
  - Added `has_embedding` column to `message_chunks` table
  - Added index for finding chunks without embeddings

### Technical

- **Recovery Module** - New `src/embeddings/recovery.rs`
  - `recover_missing_embeddings()` function for startup recovery
  - Handles both messages and chunks without embeddings

- **Session Field** - Added `compacted_range: Option<(usize, usize)>`
  - Tuple format: `(first_preserved, last_preserved_start)`
  - Backward compatible: defaults to None, uses `messages_sent_to_llm` fallback

- **Compaction Function** - `compact_conversation()` now returns `(summary, range)`
  - Uses `get_compaction_range_default()` for middle compaction
  - Falls back to full compaction for small message counts

## [0.22.1] - 2026-03-03

### Fixed

- **UTF-8 Chunking Crash** - Fixed panic when splitting strings at multi-byte character boundaries
  - Chunker now correctly handles UTF-8 multi-byte characters (accents, emojis, CJK)
  - Added `find_char_boundary()` and `find_char_boundary_forward()` helpers
  - Fixed `find_sentence_boundary()` to use `.chars()` instead of byte indexing
  - Bug: "byte index 1024 is not a char boundary; it is inside 'ó'"
  - Reported when assistant response with Portuguese accents triggered chunking

### Technical

- **Tests**: Added 4 UTF-8 boundary tests (Portuguese accents, emojis, Chinese, boundary edge case)

## [0.22.0] - 2026-03-03

### Added

- **Message Chunking** - Automatic splitting of long messages for better semantic search
  - Messages > 1024 characters are split into overlapping chunks (20% overlap)
  - Each chunk gets its own embedding for precise matching
  - All message roles (user/assistant/system/tool) now get embeddings
  - Search results show matched chunk with context ellipsis

- **Chunk Storage** - New `message_chunks` table
  - Stores chunk content, offsets, and links to parent message
  - Enables reconstructing full message from chunk matches
  - Automatic cleanup when parent message is deleted (CASCADE)

### Changed

- **Embedding Generation** - Now applies to ALL roles, not just user messages
  - Fixes issue where assistant responses about Wittgenstein weren't searchable
  - System and tool messages also benefit from semantic search

- **Search Results** - Improved display for chunked messages
  - Shows matched chunk content with `...` ellipsis for boundary context
  - Full message content available for viewing
  - Better relevance scoring with chunk-level precision

- **Database Schema** - Version bumped to 2
  - Added `message_chunks` table
  - Added `chunk_embeddings` virtual table (sqlite-vec)
  - Separate embedding tables for messages and chunks

### Technical

- **New module**: `src/embeddings/chunker.rs` - Text chunking with overlap
  - `chunk_text()` - Split text into overlapping chunks
  - `needs_chunking()` - Check if message needs chunking
  - Sentence boundary detection for clean splits
  
- **Constants**:
  - `DEFAULT_CHUNK_SIZE`: 1024 characters
  - `DEFAULT_CHUNK_OVERLAP`: 200 characters
  - `DEFAULT_CHUNK_MIN_SIZE`: 256 characters

- **Database operations**:
  - `insert_chunk()` - Insert a message chunk
  - `update_chunk_embedding()` - Store chunk embedding
  - `get_message_chunks()` - Retrieve all chunks for a message
  
- **Search operations**:
  - `search_semantic()` now queries both `message_embeddings` and `chunk_embeddings`
  - Result deduplication by `message_id` (keep best score)
  - `SearchResult` now includes `chunk_content`, `chunk_start`, `chunk_end` fields

## [0.21.0] - 2026-03-03

### Added

- **ChatSession SQLite Integration** - Automatic message persistence
  - Messages saved to SQLite immediately when added
  - Embeddings generated asynchronously in background
  - Database attached via `attach_db()` method
  - Fields: `db`, `embedding_client`, `retrieval_enabled`, `last_retrieval_time`

- **Context Overflow Detection** - Automatic warning when context fills
  - `check_context_overflow()` function monitors token usage
  - Warning at 80% of context window (72% = early warning)
  - Constants: `DEFAULT_OVERFLOW_THRESHOLD`, `DEFAULT_KEEP_FIRST`, `DEFAULT_KEEP_LAST`
  - Suggests `/compact` when approaching limits
  - Future: Auto-compaction will use `get_compaction_range()` and `estimate_compaction_savings()`

- **Context Builder** - Optimal message ordering for LLM
  - `build_context()` implements "lost in the middle" research
  - Order: System → Retrieved → Summary → Recent → Query
  - Research shows up to 30% better performance with this ordering
  - Constants: `MIN_MESSAGES_FOR_RETRIEVAL`, `RELEVANT_MESSAGES_COUNT`, `RECENT_MESSAGES_COUNT`

- **Retrieval Configuration** - Configurable context retrieval
  - `RetrievalConfig` with sensible defaults
  - Min 20 messages before activation
  - 5 relevant messages retrieved + 10 recent messages
  - 5-second throttle between retrievals
  - `/retrieval` command to toggle on/off

- **Migration Commands** - JSON to SQLite migration
  - `/migrate` - Migrate all project sessions or specific session
  - `/reindex` - Rebuild embeddings for current conversation
  - Progress reporting for long migrations

### Changed

- **`send_message()`** - Now uses `build_context()` instead of `get_messages_for_llm()`
  - Integrated overflow detection with warning display
  - Integrated retrieval context building
  - Added `db` and `embedding_client` parameters
  - Returns `ContextResult` with retrieval status

- **Embeddings** - Documented future-use functions
  - `embed_batch()` for bulk embedding (future: `/migrate` performance)
  - `embedding_dimension()` for validation (test use)
  - `normalize()` and `cosine_similarity()` for future diversity filtering

- **Database operations** - Documented future-use functions
  - `list_conversations()` for `/reindex all` command
  - `get_messages_for_reindex()` for bulk reindexing
  - `delete_conversation()` for conversation management
  - `count_embedded_messages()` for statistics
  - `with_connection_mut()` for DDL operations

### Technical

- **New modules:**
  - `src/db/migration.rs` - Session migration logic
  - `src/context_overflow.rs` - Overflow detection and compaction planning
  - `src/retrieval/context_builder.rs` - Context composition with optimal ordering

- **Database operations:**
  - `get_messages_for_reindex()` - Fetch messages needing embeddings
  - `list_conversations()` - List all conversation IDs
  - `insert_message()` with embedding support

- **Context constants:**
  - `DEFAULT_OVERFLOW_THRESHOLD`: 0.8 (80%)
  - `DEFAULT_KEEP_FIRST`: 5 messages
  - `DEFAULT_KEEP_LAST`: 5 messages
  - `MIN_MESSAGES_FOR_RETRIEVAL`: 20 messages
  - `RELEVANT_MESSAGES_COUNT`: 5 messages
  - `RECENT_MESSAGES_COUNT`: 10 messages

- **Future-use functions (documented with `#[allow(dead_code)]`):**
  - `check_context_overflow_default()` - Auto-overflow detection
  - `get_compaction_range_default()` - Auto-compaction planning
  - `estimate_compaction_savings()` - Compaction benefit calculation
  - `should_position_summary_after_system()` - Summary placement
  - CompactionSuggestion struct fields: `keep_first`, `keep_last`, `middle_count`

## [0.20.0] - 2026-03-03

### Added

- **Semantic Search** - New `/search` (alias `/find`, `/f`) command for chat
  - Hybrid search combining BM25 (keyword) and semantic (vector similarity)
  - Reciprocal Rank Fusion (RRF) for result ranking
  - Search across all conversation history
  - Usage: `/search <query>` or `/search <query> <limit>`
  - Requires `nomic-embed-text-v2-moe` model from Ollama

- **Database Module** - New SQLite storage with sqlite-vec extension
  - `src/db/mod.rs` - Database initialization and exports
  - `src/db/schema.rs` - SQL schema (conversations, messages, embeddings, FTS5)
  - `src/db/connection.rs` - sqlite-vec global initialization
  - `src/db/operations.rs` - CRUD operations and hybrid search
  - Storage location: `~/.local/share/ask-ai/embeddings.db`

- **Embeddings Module** - New embedding generation for semantic search
  - `src/embeddings/client.rs` - Ollama embedding client
  - `src/embeddings/truncate.rs` - Matryoshka truncation (768d → 256d)
  - Validates embedding dimensions before truncation
  - L2 normalization for cosine similarity

- **Retrieval Module** - New search module
  - `src/retrieval/search.rs` - Hybrid search implementation
  - Formatted results with icons and metadata
  - Integration with `/search` command

- **FTS5 Query Sanitization** - SQL injection protection
  - `fts5_escape()` function for safe FTS5 queries
  - Wraps queries in double quotes, escapes embedded quotes
  - Prevents FTS5 syntax errors and injection attacks

### Dependencies

- `rusqlite` 0.32 (bundled) - SQLite database
- `sqlite-vec` 0.1 - Vector similarity extension
- `zerocopy` 0.8 - Safe byte casting for embeddings

### Technical

- Embedding dimensions: 768 (full) → 256 (truncated, Matryoshka)
- RRF weights: Keyword 0.4, Semantic 0.6
- sqlite-vec KNN syntax: `WHERE embedding MATCH ? AND k = ?`
- Database initialized on startup via `db::init()`

## [0.19.0] - 2026-03-02

### Added

- **Context Metrics Command** - New `/context` (alias `/ctx`) command for chat
  - Displays estimated token usage breakdown (system, tools, conversation)
  - Shows context window utilization percentage
  - Helps users understand context pressure and plan compaction

- **Token Counting Module** - New `src/tokens.rs` module
  - Word-based token estimation (~0.75 words/token for English)
  - Message overhead calculation (~4 tokens per message)
  - `ContextMetrics` struct for context usage tracking
  - `calculate_context_metrics()` for full context analysis

- **Todo List Tools** - New tool category for task tracking
  - `todo_add(description)` - Add a new task to the list
  - `todo_update(id, status)` - Update task status (pending/in_progress/done)
  - `todo_list()` - List all tasks with current status
  - `todo_clear_done()` - Remove completed tasks
  - `todo_clear_all()` - Clear all tasks
  - Reduces need to search conversation history for task tracking
  - Enabled via `todo-tools` feature flag (enabled by default)

- **Todo State Persistence** - Todo list persists with chat session
  - New `todos` field in `ChatSession`
  - `TodoState` struct with `Task` and `TaskStatus` enums
  - Automatically saved/restored with session

- **HTTP Helpers** - New utilities for tool implementations
  - `fetch_json<T>()` for GET requests with JSON parsing
  - `fetch_json_with_headers<T>()` for requests with custom headers
  - `post_json_with_headers<T>()` for POST requests
  - All helpers include automatic error logging

- **Logging Macros** - Boilerplate reduction for tools
  - `log_tool_call!` macro for tool call logging
  - `log_tool_result!` macro for result logging
  - `tool_wrapper!` macro for automatic logging wrapper

### Fixed

- **Code Quality** - Clippy warnings and dead code cleanup
  - Fixed collapsible if statements in platform.rs and personality.rs
  - Fixed `.map(|x| x.clone())` to `.cloned()` in builder.rs
  - Fixed manual RangeInclusive::contains patterns in tests
  - Removed duplicate ContextMetrics struct
  - Fixed MESSAGE_OVERHEAD constant visibility

### Changed

- **Feature Flags** - `todo-tools` added to default features
  - `all-tools` now includes `todo-tools`

- **Refactoring** - Unified HTTP handling across tools (~370 lines reduced)
  - pokemon.rs: 9 functions refactored using shared helper
  - weather.rs: 3 functions + helper refactored
  - serper.rs: 2 functions refactored with POST helper
  - finance.rs: collapsible_if fix

## [0.18.2] - 2026-03-01

### Added

- **Undo Command** - New `/undo` (alias `/u`) command for chat
  - Removes last assistant response(s) from conversation
  - Displays last user message for easy re-editing
  - Use arrow up (↑) to retrieve from history and edit
  - Workflow: `/undo` → see message → ↑ to edit → resend

## [0.18.1] - 2026-03-01

### Fixed

- **Chat Model Configuration** - `[model.chat].model` is now properly respected
  - Chat subcommand now uses `model.chat.model` from config.toml as default
  - Previously fell back directly to global `model.default`, ignoring chat-specific model
  - Affects anonymous mode (`-a`), failed session loads, and new sessions

## [0.18.0] - 2026-03-01

### Added

- **LED Control Tools** - New optional tool category for NeoPixel LED control
  - 5 tools: `led_get_status`, `led_set_power`, `led_set_program`, `led_set_brightness`, `led_set_color`
  - Control LED strips via Raspberry Pi Pico W HTTP server
  - Requires `led-tools` feature flag and `[led]` configuration in config.toml
  - Color manipulation with hex or RGB values (LLM-friendly)
  - Configuration: `ip` (required) and `port` (optional, default: 80)

- **LED Tools Documentation** - Comprehensive docs in `doc/src/tools.md`
  - Tool reference with examples
  - Configuration guide
  - Color manipulation tips for LLMs
  - Example workflows for natural language control

- **Chat Configuration** - New `[model.chat]` section in config.toml
  - Configure default model, thinking, and tools for chat subcommand
  - Falls back to global `[model]` settings if not specified

- **Thinking Mode Priority** - Improved thinking mode resolution
  - Priority: Model capability → CLI flags → Subcommand config → Global config → Model default
  - Configurable via `model.thinking` (global) and `model.chat.thinking` (subcommand)
  - Warning shown if thinking enabled but model doesn't support it

### Configuration

New config.toml options:
```toml
[model]
# Global default for thinking mode (optional)
thinking = false

[model.chat]
# Chat-specific model and settings
# model = "llama3.1"
# thinking = false
# tools = true
```

New `[led]` section in config.toml:
```toml
[led]
ip = "192.168.1.100"  # Required for LED tools
port = 80             # Optional, default: 80
```

### Feature Flags

- `led-tools` - Enable LED control tools (disabled by default)
- `all-tools` now includes `led-tools`

### Few-shot Examples

Added 3 new LED tool examples to demonstrate:
- Basic power and color control
- Color adjustment workflow (get status → modify RGB → set color)
- Brightness and power control

## [0.17.0] - 2026-02-28

### Added

- **Prompt Refactoring** - Complete system prompt overhaul based on prompt engineering best practices
  - Created modular prompt system in `src/prompts/` with hierarchical structure
  - Added 13 ReAct-style few-shot examples (replaced arrow notation)
  - Removed all negative instructions (DO NOT, NEVER, etc.)
  - Created benchmark tests (10 passing) for prompt validation
  - Token count reduced from ~1700 to ~890 tokens

- **Platform Detection** - Dynamic OS/distro detection in `src/platform.rs`
  - Detects Linux distros (Arch, Ubuntu, Debian, Fedora, etc.)
  - Detects Termux on Android
  - Detects macOS and Windows
  - Platform info added to system context

- **Retry Command** - New `/retry` (alias: `/r`) command
  - Removes assistant messages since last user message
  - Regenerates response with same context
  - Useful for getting different answers

### Fixed

- **Anonymous Chat Mode** - Now truly anonymous
  - Fixed bug where anonymous mode was loading sessions from "anonymous" directory
  - Anonymous sessions now start completely fresh, no history persistence

- **Immediate Message Saving** - User messages saved immediately after sending
  - Previous behavior: saved only after receiving response
  - New behavior: saved immediately, preventing message loss on crash/interrupt

### Changed

- **Chat Session API** - Added new methods:
  - `remove_last_assistant_messages()` - removes messages since last user message
  - `get_last_user_message()` - retrieves last user message for retry

### Technical

- Created `src/lib.rs` for library module exports (test infrastructure)
- Added `PromptConfig` builder pattern for flexible prompt generation
- Deprecated old prompt functions (`get_prompt`, `get_prompt_with_blacklist`)

## [0.16.2] - 2026-02-25

### Fixed

- **CLI Model Override in Chat** - CLI model parameter now takes precedence over saved session model
  - Fixed bug where `-m <model>` was ignored when resuming a saved session
  - Added validation: if CLI model doesn't exist, show error and exit gracefully
  - Added fallback: if saved session model no longer exists, use default with warning

## [0.16.1] - 2026-02-25

### Fixed

- **Model Switching in Chat** - Centralized model switching logic to prevent state inconsistencies
  - Created `src/chat/model_switch.rs` as single point for all model switching
  - Fixed bug where `session.tools` and `session.think` could diverge from internal state
  - All model validation, capability detection, and state updates now happen in one place
  - Removed duplicate model switching code from `commands.rs`

- **Man Page Updated** - Updated to v0.16.1 with:
  - New `vision` command documentation
  - New `completion` command documentation
  - Updated model list with default vision model (moondream)
  - Correction: translategemma default changed to 4b model

## [0.16.0] - 2026-02-24

### Added

- **Vision Command** - New `ask vision` subcommand for image analysis
  - Default model: moondream:1.8b (lightweight, 1.7GB)
  - Multi-image support via Ollama API `images` array
  - Three modes: default (brief), --detailed (comprehensive), custom prompt
  - JSON output with --json flag
  - Markdown rendering with --plain global flag for plain text
  - Configurable via `[model.vision]` in config.toml
  - Documentation in `doc/src/commands/vision.md`

- **Shared Image Utilities** in `src/utils.rs`
  - `validate_image_file()` - validates file existence and extension
  - `read_file_as_base64()` - async file reading with base64 encoding
  - Used by both OCR and Vision modules

### Changed

- **Translation Model Updated** - Changed default from translategemma:12b to translategemma:4b
  - Smaller, faster model with same translation quality
  - Updated all documentation and config defaults

- **Vision Models Tested** - Updated documentation with verified working models:
  - moondream:1.8b - Default, lightweight
  - llava:13b - Better quality (llava:7b doesn't work)
  - llama3.2-vision:11b - Large context, good interpretation
  - ministral-3:14b - Multi-image support

- **Code Deduplication** - Shared utilities between OCR and Vision modules

## [0.15.0] - 2026-02-23

### Added

- **Custom Coordinator** - New `CustomCoordinator` implementation
  - Pre-tool content forwarding - model's thinking/intro text before tool calls is now displayed
  - Event callbacks for `PreToolContent`, `ToolCall`, `ToolResult`, `FinalResponse`
  - Replaces ollama-rs Coordinator for full control over tool execution flow

- **Thinking Display Improvements**
  - Lighter gray color (`\x1B[37m`) for better readability
  - Markdown rendering support with `termimad::MadSkin`
  - Proper word wrapping respecting terminal width
  - No more words cut in half on narrow terminals

- **Retry Logic for Query Mode**
  - Query and legacy query now have same retry logic as REPL
  - Recoverable errors (unknown tool, invalid args, network) trigger retry
  - Model receives error message and can correct tool calls
  - Up to 3 retry attempts

### Changed

- **Tool Output Display**
  - Tool calls show function name with parameters (from debug_tools.rs)
  - Tool results show abbreviated preview in normal mode
  - Debug mode shows full detailed output
  - No duplicate logging

- **Refactored `display_thinking()`**
  - New `render_markdown` parameter for markdown rendering
  - Automatically detects terminal width for proper wrapping
  - Word-wrap algorithm respects word boundaries

### Fixed

- **Thinking Text Wrapping** - Now properly wraps at word boundaries
  - Uses terminal width detection via `termimad::terminal_size()`
  - Accounts for 2-character indentation
  - Supports both markdown and plain text modes
  - Created `src/query.rs` module with shared query logic:
    - `run_query()` - unified function for query, legacy query, and chat message handling
    - `ChatContext` - builder for coordinator with event callbacks
    - `OutputFlags` - resolved debug/plain flags from CLI and config
    - `handle_chat_event()` - centralized event handling for tool execution
  - Consolidated `handle_query()` and `handle_legacy_query()` in `main.rs`
  - `main.rs` reduced from 1175 lines to 572 lines (51% reduction)
  - Chat REPL now uses `ChatContext` builder pattern
  - Created `src/tools/registry.rs` with centralized tool registration
  - Created `src/utils.rs` with shared utility functions
  - Moved `build_model_options()` to `ModelConfig` as instance method
  - Added `detect_or_default()` to `ModelCapabilities`
  - Added `display_thinking()` helper to `chat/thinking.rs`
  - Added `resolve_model_config()` and `resolve_think_mode()` to `user_models.rs`
  - Added `SpinnerGuard` RAII pattern to `spinner.rs`

### Removed

- **Dead Code** - Removed unused code and false-positive `#[allow(dead_code)]`
  - Removed `OutputFormat` enum and unused methods from `ocr/cli.rs`
  - Removed false `#[allow(dead_code)]` from `NamedApiResource.url` and `Settings::blacklist_set()`

### Fixed

- **Chat Mode CLI Flags** - Model and flags from CLI now work correctly
  - `ask chat -m <model>` now properly sets the initial model
  - `ask chat -t` now enables think mode from CLI
  - `ask chat --tools` now enables tools from CLI
  - `ask chat --ignore-agents` now ignores AGENTS.md from CLI

## [0.14.2] - 2026-02-22

### Fixed

- **Tool Error Handling** - Tools now return errors as `Ok(String)` instead of `Err()`
  - Model sees tool errors and can react/retry
  - Previously, `Err()` would immediately fail the entire request
  - Now the model receives the error message and decides how to proceed

- **test_tool** - Debug tool now returns error message as success
  - Allows testing tool error recovery scenarios
  - Model can see error and retry with different parameters

### Added

- **Error Recovery Helpers** - New utilities in `chat/coordinator.rs`
  - `RecoverableError` enum for classifying errors
  - `classify_error_str()` for string-based error classification
  - `format_recovery_message()` for model-friendly error messages
  - Prepared for future network/Ollama-level error recovery

## [0.14.1] - 2026-02-20

### Fixed

- **Thinking Output** - Now uses API-provided `thinking` field from Ollama
  - Previously only extracted thinking from content via regex
  - Now checks `response.message.thinking` first, then falls back to regex
  - Works correctly with cloud models that support thinking

### Changed

- **Model Parameters** - `top_k`, `top_p`, `repeat_penalty` now optional
  - If not specified in config, uses Ollama's defaults
  - Updated defaults from docs.ollama.com: `temperature=0.8`, `repeat_penalty=1.1`
  - Previous defaults were too low (`temperature=0.2`)

- **Cloud Model Thinking** - Enable thinking via config
  - Add `thinking = true` in `models.toml` for cloud models
  - Model's `thinking` field checked alongside capability detection
  - Priority: CLI flag > model config > subcommand config

## [0.14.0] - 2026-02-19

### Added

- **Custom Models** - Define your own models or override built-in presets
  - Create `~/.config/ask-ai/models.toml` to add custom models
  - Override parameters for built-in models (partial override)
  - Custom models shown with `[user]` marker in `--list` output
  - See [Configuration - Custom Models](./configuration.md#custom-models)

- **Thinking Output** - Thinking content now visible in chat mode
  - When think mode is enabled, thinking content shown in gray/dim text
  - Supports multiple thinking tag formats: unicode, `<tool_call>`, `<thinking>`
  - Handles malformed tags (orphan `</thinking>`)

- **Token Metrics** - Response now shows token usage
  - Displays: `[Tokens: X prompt + Y response = Z total]`
  - Helps track context window usage

- **Error Formatting** - Improved error messages
  - JSON errors from Ollama formatted with red status codes
  - Clear guidance for common error scenarios

### Changed

- **Tool Output Control**:
  - New `/tools-output <level>` command: compact, full, or hidden
  - New `--tools-output` startup flag
  - Compact summary shown after `/compact` command

- **Built-in Models Simplified** - Reduced to essential models only
  - Built-in: `llama3.1:8b` (default), `translategemma:4b` (translation), `glm-ocr:bf16` (OCR), `moondream:1.8b` (vision)
  - All other models moved to `~/.config/ask-ai/models.toml`
  - Cloud models have no hardcoded parameters (let Ollama decide)

- **Default Context Size** - User models now default to 32K context
  - Previous: 4K default for user-defined models
  - Now: 32K default for better compatibility with large context models
  - Omit `num_ctx` to let Ollama auto-detect based on available memory

- **Model Naming Convention** - Removed context size suffixes from model IDs
  - Previous: `lfm2.5-thinking:1.2b-32k`, `llama3.2:3b-32k`
  - Now: `lfm2.5-thinking:1.2b`, `llama3.2:3b`
  - Context size configured via `num_ctx` in models.toml, not model tag

- **Default Model Changed** - From `lfm` to `llama3.1`
  - `llama3.1:8b` is more capable and widely available
  - `lfm` still available as user-defined model

- **GPT-OSS Removed** - Model removed due to persistent tool calling issues
  - The model output special tokens after JSON, breaking tool parsing
  - Alternative models: `qwen3-coder`, `mistral-small`, `llama3.1`

### Migration Notes

If upgrading from v0.13.0:
1. Run `ask-ai --list` to see the new model organization
2. Default model is now `llama3.1` (update config if you used `lfm`)
3. Check `~/.config/ask-ai/models.toml` for all available model presets
4. Cloud models no longer have hardcoded parameters - configure as needed

## [0.13.0] - 2026-02-19

### Added

- **Chat Mode Enhancements**:
  - `/think` command - Toggle think mode on/off
  - `/tools` command - Toggle tools on/off
  - `/compact` command - Summarize conversation history to reduce context
  - Tab completion for commands and model names
  - Mode indicators in prompt: `[t]` for think, `[T]` for tools
  - Warning when tools enabled but model doesn't support them

### Changed

- **Session Format** (Breaking Change):
  - Added `compacted_summary` field for conversation summarization
  - Added `messages_sent_to_llm` field to track compacted portion
  - Old session files may need to be deleted (`~/.local/share/ask-ai/conversations/`)

- **UI Improvements**:
  - Welcome message only shows available features (think/tools hidden if unsupported)
  - Prompt shows active modes: `lfm[t][T]>` when think and tools enabled
  - `/info` shows compacted message count if applicable

### Removed

- `uuid` dependency (session IDs are now simple strings)

## [0.12.0] - 2026-02-19

### Added

- **Interactive Chat Mode** - New `chat` subcommand for multi-turn conversations
  - Persistent conversation history per project (identified by git remote URL or folder name)
  - Anonymous sessions with `--anonymous` flag (no persistence)
  - Session management: `/save`, `/load`, `/list` commands
  - Model switching mid-conversation: `/model <name>`
  - Export conversations: `/export md` or `/export json`
  - Rich REPL with command history and line editing (rustyline)
  - Auto-saves after each message to `~/.local/share/ask-ai/conversations/`
  - Commands: `/quit`, `/clear`, `/help`, `/model`, `/system`, `/save`, `/load`, `/export`, `/list`, `/info`

### Changed

- **Dependencies**:
  - Added `rustyline` for REPL with history
  - Made `chrono` non-optional (used for session timestamps)

## [0.11.1] - 2026-02-18

### Fixed

- **Config file ignored by subcommands** - translate, ocr, summarize now respect ollama_host/ollama_port
  - Previously: These subcommands used `Ollama::default()` (localhost:11434) ignoring config
  - Now: All subcommands use `Settings::ollama_client()` for consistent config handling
  - Fixes "Reqwest error" when connecting to remote Ollama server from Termux/Android
- **CLI parameter precedence** - Fixed bug where CLI flags were not properly respected
  - Changed `model`, `plain`, `debug` fields from `String`/`bool` to `Option<String>`/`Option<bool>`
  - Precedence now correctly: CLI arguments > config file > built-in defaults

### Changed

- **CLI flag architecture** - Centralized shared flags at global level
  - Flags like `-m`, `-d`, `--plain`, `-t`, `--tools`, `-c`, `--ignore-agents` now only exist at global level
  - Usage: `ask -d query "text"` (flags BEFORE subcommand)
  - Subcommands retain their specific flags: `translate --list`, `summarize --format bullets`, `ocr --mode table`
  - Updated documentation and manpage to reflect this change

## [0.11.0] - 2026-02-18

### Added

- **Termux/Android support** - Cross-compilation for Android devices
  - New Makefile targets: `termux`, `termux-all-tools`, `tarball-termux`, `tarball-termux-all-tools`
  - Cross.toml configuration for `aarch64-linux-android` target
  - Documentation for Termux installation
- **rustls TLS backend** - Replaced OpenSSL with rustls for better cross-platform support
  - Enables cross-compilation without OpenSSL dependencies
  - Binary size optimized (12MB default, 16MB all tools)

### Fixed

- **ollama_host configuration** - Now accepts IP addresses without `http://` prefix
  - Previously: `ollama_host = "192.168.1.100"` would panic
  - Now: Automatically prepends `http://` if scheme is missing
  - Works with: `"192.168.1.100"`, `"http://192.168.1.100"`, `"https://myserver.local"`

### Changed

- **Dependency optimization**:
  - Aligned `reqwest` version with `ollama-rs` (v0.12) to avoid duplication
  - Removed redundant explicit dependencies (`html2md`, `scraper`) - already provided by `ollama-rs`
- **Documentation updates**:
  - Fixed incorrect web search documentation (serper-tools is working, search-tools is optional)
  - Updated feature flags table in README and AGENTS.md

## [0.10.0] - 2026-02-18

### Added

- **System context injection** - Minimal context (~20 tokens) injected into every prompt
  - Current date (day of week, date)
  - Current working directory
  - Git branch (if in repo)
- **New tool: `get_current_datetime`** - Current date, time, and timezone
  - Feature flag: `system-tools` (enabled by default)
  - Returns: date, time, timezone, day of week, week of year, ISO 8601, Unix timestamp
- **New tool: `get_project_context`** - Project state (languages, git, stack)
  - Feature flag: `system-tools` (enabled by default)
  - Provides: directory, git branch/remote, language detection, stack detection, key files
  - **Relationship with AGENTS.md**: AGENTS.md contains conventions (HOW), get_project_context provides state (WHAT)

### Changed

- **Default feature flags reorganized**:
  - `system-tools` is now enabled by default
  - All tools enabled by default (empty blacklist)
- **Code cleanup** - Fixed all clippy warnings

### Removed

- **Legacy web-search-tools removed** - Replaced by serper-tools

## [0.9.0] - 2026-02-18

### Added

- **New tool: `fetch_pokemon_by_type`** - List all Pokémon of a specific type (limit 100)
- **New tool: `calculate`** - Mathematical expression evaluation
  - Basic arithmetic: +, -, *, /
  - Exponents: ** or ^
  - Percentages: "15% of 850"
  - Functions: sqrt(), sin(), cos(), tan(), log(), etc.
  - Feature flag: `calc-tools` (enabled by default)
- **New tool: `get_stock_quote`** - Stock quotes from Google Finance
  - Feature flag: `finance-tools` (disabled by default)
  - Usage: `get_stock_quote(exchange: "NASDAQ", ticker: "AAPL")`
- **New tool: Web search via Serper** - Google Search results via Serper.dev API
  - Feature flag: `serper-tools` (enabled by default)
  - Requires `SERPER_API_KEY` environment variable
  - Tools: `web_search`, `web_search_news`
  - Automatic fallback to DuckDuckGo if API key not set
  - Debug mode shows: "🔑 [Serper] API key found - enabling Google Search via Serper"
- **All tools now output in English** - Consistent English output across all tools
- **Pokémon tools enabled by default** - No longer need `--features pokemon-tools`
- **Makefile targets for feature builds**:
  - `make build-pokemon` - Build with Pokémon tools
  - `make build-all-tools` - Build with all tools
  - `make install-local-pokemon` - Local install with Pokémon tools
  - `make install-local-all-tools` - Local install with all tools
  - `make test-all` - Run tests with all features

### Changed

- **BREAKING: Tool output language changed to English** - All tool responses now in English
- **Web search now prefers Serper over DuckDuckGo** - Serper uses Google Search with no CAPTCHA issues
- **Feature flag reorganization**:
  - `serper-tools` - Google Search via Serper API (enabled by default)
  - `system-tools` - Date/time and project context (enabled by default)
  - `search-tools` - DuckDuckGo + Web scraper (disabled, used as fallback)
  - `finance-tools` - Stock quotes via Google Finance (disabled by default)
  - `all-tools` now includes all tool categories
- **All numeric/optional tool parameters now accept strings** - LLMs frequently pass `"5"` instead of `5`, tools now handle this gracefully

### Fixed

- Weather tools fixed - API response structs now use optional fields
- All tools have proper error handling - No more crashes from network/API errors
- Raw errors shown with pretty printing in debug mode - Use `{:#?}` for readable output
- Pokémon tools fixed - All 9 tools have proper logging and error handling
- **Web search CAPTCHA issue resolved** - Using Serper for Google Search (no CAPTCHA)
- **Tool parameter parsing fixed** - All tools now accept strings for numeric parameters

### Documentation

- Updated tools.md with Serper configuration, system tools, and usage examples
- Updated CHANGELOG.md with all changes
- Updated README.md with AGENTS.md context and build features
- Updated contributing.md with feature flags and Makefile commands
- Updated man page with --ignore-agents flag
- Updated roadmap.md with ollama-rs integration status

## [Unreleased]

### Planned

- Custom model support - Allow users to define custom models in config
- Termux builds - Android/Termux support

### Added

- Complete documentation with mdBook
- Man page for terminal reference
- Development documentation (architecture, roadmap, contributing)
- Mermaid diagram support
- New `read_file_segment` tool for reading specific line ranges from files
- New `count_lines` tool to check file size before reading
- Tool error handling guidelines in documentation
- Tool calls now always visible to user (even without debug mode)
- **AGENTS.md context injection** - Automatically load project context from current directory
- **`--ignore-agents` flag** - Disable AGENTS.md context loading
- Security sanitization for AGENTS.md content (injection patterns, executable code blocks)

### Fixed

- Tool call errors now return friendly messages instead of crashing
- Code mode now respects `[model.code]` config properly
- Translate subcommand now respects `--plain` flag
- Per-subcommand configuration system (query, summarize, code)
- `read_file` debug output now shows "all" instead of empty when max_lines not specified
- `list_directory` debug output now shows "false" instead of empty when recursive not specified
- **Weather tools fixed** - API response structs now use optional fields
- **All tools now have proper error handling** - No more crashes from network/API errors
- **Raw errors shown with pretty printing in debug mode** - Use `{:#?}` for readable error output
- **Pokémon tools fixed** - All 9 tools now have proper logging and error handling

### Changed

- Tools now return informative error messages to help LLM recover from mistakes
- Tool calls always logged to stderr so users can see what's being executed
- Documentation updated with tool error handling philosophy
- Prompt updated to encourage using `count_lines` before reading large files
- **All tool arguments now use String type** for robustness (LLMs generate inconsistent JSON)
  - Boolean parameters accept: "true", "false", "1", "0", "yes"
  - Numeric parameters accept: "50", "100", etc.
  - Empty/unset parameters use sensible defaults
- **File sizes shown in KB/MB** instead of raw bytes (more intuitive for LLMs)
- **`count_lines`** now shows only line count (removed byte count - LLMs think in lines)
- **`read_file_segment`** now requires both `start_line` and `num_lines` (no defaults)
- Spinner now suspends during tool output (no more frozen spinner text)

## [0.1.0] - 2026-02-17

### Added

- Initial release
- Core CLI with 4 subcommands:
  - `query` - General LLM queries
  - `translate` - Language translation (50+ languages)
  - `ocr` - Image text extraction
  - `summarize` - Text summarization
- 14 tools:
  - 8 Pokémon tools via PokéAPI
  - 3 Weather tools via Open-Meteo
  - 3 Web Search tools via DuckDuckGo (currently blocked)
- 14+ model presets with capability detection
- Markdown rendering via termimad
- Tool integration with auto-detection
- Pipe support for all commands
- Debug mode
- Think mode for reasoning models
- Code mode for code-focused responses
- Model capability detection
- Spinner for UX feedback
- Stdin support
- Plain text output option
- Pepe Easter Egg personality

### Known Issues

- DuckDuckGo web search blocked by CAPTCHA

## Categories

- `Added` - New features
- `Changed` - Changes to existing functionality
- `Deprecated` - Soon-to-be removed features
- `Removed` - Removed features
- `Fixed` - Bug fixes
- `Security` - Security fixes

## Versioning

We follow [Semantic Versioning](https://semver.org/):

- MAJOR: Incompatible API changes
- MINOR: Backward-compatible functionality
- PATCH: Backward-compatible bug fixes

## Format

Based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
