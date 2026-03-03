# Changelog

All notable changes to Ask-AI will be documented in this file.

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
