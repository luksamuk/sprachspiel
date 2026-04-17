# Ask-AI (ask-ollama-rs) — Architecture Report

## Overview

Ask-AI is a Rust CLI application that interfaces with Ollama LLMs, providing
query, chat (REPL), translation, OCR, vision, and summarization subcommands.
It features a tools system (feature-flagged), RAG/retrieval via sqlite-vec,
a factual memory system, skills on-demand, and context overflow management
with auto-compaction.

**Version**: 0.40.0 | **Rust edition**: 2024 | **~90 source files | ~44K+ lines**


---

## 1. User Input — How Input Arrives

| Mode | Entry Point | Input Method |
|------|-------------|-------------|
| **Positional query** | `ask-ai "What is Rust?"` | `Cli.query` (clap) — goes to `handle_legacy_query` → `run_query` |
| **Pipe/stdin** | `echo "hello" \| ask-ai` | `get_query_legacy` reads `std::io::stdin()` |
| **Query subcommand** | `ask-ai query "text"` | `Commands::Query` → `handle_query_subcommand` → `run_query` |
| **Chat REPL** | `ask-ai chat` | `Commands::Chat` → `handle_chat` → `chat::run_chat_repl` |
| **Translate** | `ask-ai translate en:pt "hello"` | `Commands::Translate` → `handle_translate` |
| **OCR** | `ask-ai ocr file.png` | `Commands::Ocr` → `handle_ocr` |
| **Vision** | `ask-ai vision file.jpg` | `Commands::Vision` → `handle_vision` |
| **Summarize** | `ask-ai summarize "text"` | `Commands::Summarize` → `handle_summarize` |
| **Completion** | `ask-ai completion bash` | `Commands::Completion` → `handle_completion` (shell completions) |
| **List** | `ask-ai --list` | In `handle_legacy_query`, prints models/prompts/subcommands |
| **Init config** | `ask-ai --init-config` | `Settings::create_sample_config()` |

### CLI Args (key flags)

- `-m MODEL` — model preset name
- `-p PROMPT` — system prompt mode (default: "default")
- `--think` — enable thinking mode
- `--plain` — plain text output
- `--debug` — dry-run/debug mode
- `--tools` — force-enable tools
- `--code` — code mode (minimal explanations)
- `--ignore-agents` — skip AGENTS.md
- `--soulless` — skip SOUL.md personality
- `--soulless` (chat-specific) — per-chat flag

---

## 2. Configuration — Layered Settings System

### Config Files (priority order)

| File | Location | Purpose |
|------|----------|---------|
| `config.toml` | `~/.config/ask-ai/config.toml` | Model defaults, tools blacklist, display skin, LED config |
| `models.toml` | `~/.config/ask-ai/models.toml` | User-defined model presets (overrides/extends builtins) |
| `tools.toml` | `~/.config/ask-ai/tools.toml` | External CLI tool whitelist + file tools config |
| `SOUL.md` | `~/.config/ask-ai/SOUL.md` | Agent personality (replaces former Pepe system) |
| `AGENTS.md` | `./AGENTS.md` (cwd) | Project context injection (sanitized) |
| `skills/<name>/SKILL.md` | `~/.config/ask-ai/skills/` or `.ask-ai/skills/` | On-demand skill behaviors |

### Model Resolution Chain

```
CLI -m flag
  → settings.model.<subcommand>.model
    → settings.model.default ("qwen3.5:4b")
      → Built-in ModelConfig (config.rs: qwen3.5:4b, translategemma, glm-ocr)
        → User model override (models.toml) via user_models::merge_configs
```

### Key Structs

- **`Settings`** (`settings.rs`): `ModelSettings`, `ToolSettings`, `OutputSettings`, `DisplaySettings`, `LedSettings`
- **`ModelConfig`** (`config.rs`): model_id, num_ctx, temperature, top_k, top_p, repeat_penalty, thinking
- **`SubcommandModelConfig`**: Per-subcommand model/thinking/tools overrides
- **`ModelCapabilities`** (`capabilities.rs`): Detected at runtime via Ollama API `show_model_info` (tools, vision, completion, thinking)

### Thinking Mode Resolution

```
CLI --think flag
  → subcommand-specific config (model.query.thinking)
    → global config (model.thinking)
      → model default (ModelConfig.thinking)
        → hardcoded default (true for query, false for others)
          → gated by ModelCapabilities.thinking
```

---

## 3. Core Chat Flow

### Architecture Layers (from chat/mod.rs)

```
Layer 0: input.rs (trait), view.rs (trait)     — NO dependencies
Layer 1: session.rs, cli.rs                    — Session + CLI args
Layer 2: input/rustyline.rs, view/terminal.rs  — Concrete implementations
Layer 3: repl_state.rs                         — Consolidated mutable state
Layer 4: core.rs, command_handlers.rs          — Business logic
Layer 5: repl.rs                              — Coordinator / main loop
```

### Single Query Flow

```
main() → run_query()
  → QueryContextBuilder::new()...build(settings)  // Resolve model, capabilities, system prompt
  → coordinator::build_query_coordinator(&ctx, settings)  // Build CustomCoordinator with tools
  → build_query_context(...)                       // RAG retrieval if enabled
  → executor::execute_query_with_retry(...)         // Send to Ollama with retry
  → display_result(...)                             // Render with markdown/plain
```

### Chat REPL Flow (the main loop)

```
run_chat_repl()
  → init_chat_database()          // SQLite + EmbeddingClient
  → run_startup_tasks()           // Facts decay cycle
  → ChatSession::load_or_create() // Load from SQLite or create new
  → Loop:
    → input.read_line()           // RustylineInput
    → parse_command()             // /quit, /model, /compact, etc.
    → handle_user_message()       // For non-commands:
      → session.add_user_message()            // Persist + chunk + async embed
      → check_and_compact_before_tool()        // Pre-tool context check
      → Loop (retry on overflow):
        → core::send_message()                // === THE CORE PATH ===
          → build_session_system_prompt()     // Assemble system prompt
          → setup_coordinator()               // Create CustomCoordinator + register tools
          → prepare_messages()                // RAG retrieval + continuation
          → coordinator.chat(messages)         // Send to Ollama
          → (on error: classify_ollama_error → retry or bail)
        → process_send_result()               // Handle continuation, auto-compact
      → session.save_sqlite()                 // Persist session metadata
```

### CustomCoordinator (chat/custom_coordinator.rs)

**Purpose**: Custom re-implementation of ollama-rs's Coordinator with:
- **Pre-tool content callbacks** (ChatEvent::PreToolContent)
- **Thinking content callbacks**
- **Inter-tool context overflow detection** (ChatEvent::ContextNearLimit, ContextTruncated, ContextNeedsCompaction)
- **Ephemeral messages** for continuation after compaction
- **Real token count tracking** from Ollama's `prompt_eval_count`

**Key methods**:
- `chat(messages)` → Loop: sends request → if tool calls: execute tools → append results → re-send → repeat until no tool calls
- `add_tool(tool)` → Registers tool with schema generation
- `on_event(callback)` → Subscribe to ChatEvents
- `check_and_handle_context_overflow()` → Per-tool overflow detection with truncation
- `push_ephemeral(msg)` → Add non-persisted continuation prompts

### Continuation System (chat/continuation.rs)

When the LLM emits `<continuation_needed>` (context near full):
1. Parse `ContinuationTag` (paused_at, next_step)
2. Auto-compact context (`auto_compact_if_needed`)
3. Re-send with continuation prompt as ephemeral message
4. Maximum 3 nested continuations
5. Inter-tool compaction: When context overflows mid-tool-chain, compact and continue

### Context Overflow (context_overflow.rs)

Percentage-based thresholds that scale with context window:
| Threshold | Usage | Action |
|-----------|-------|--------|
| PRE_TOOL | 75% | Warning only |
| COMPACTION | 88% | Auto-compact (summarize old messages) |
| INTER_TOOL | 94% | Warning during tool execution |
| EMERGENCY | 97% | Truncate tool results |

Auto-compaction: Summarizes middle messages using LLM, keeps first/last N messages. Produces `compacted_summary` + `compacted_range`.

---

## 4. Tools System

### Feature Flags (Cargo.toml)

| Feature | Tools Included | Default |
|---------|---------------|---------|
| `weather-tools` | get_weather, get_current_weather, get_weather_forecast | Yes |
| `file-tools` | read_file, read_file_segment, count_lines, list_directory, search_files, write_file, edit_file, append_file | Yes |
| `calc-tools` | calculate | Yes |
| `serper-tools` | web_search, web_search_news (requires SERPER_API_KEY) | Yes |
| `system-tools` | get_current_datetime, get_project_context | Yes |
| `skills-tools` | skill_list, skill_view | Yes |
| `document-tools` | import_document | Yes |
| `pokemon-tools` | 9 Pokemon API tools | No |
| `search-tools` | web_search, web_search_news, web_scrape (DuckDuckGo) | No |
| `finance-tools` | get_stock_quote | No |
| `led-tools` | 5 LED control tools (hardware) | No |
| `sandbox` | Landlock filesystem sandbox (Linux 5.13+) | No |

### Always-Available Tools (no feature flag)

- `test_tool` — debug tool for testing tool calling
- `remember` — semantic search in conversation history
- `fact_add`, `fact_search`, `fact_remove` — Facts memory
- `note_add`, `note_edit`, `note_delete` — Notes
- `todo_add/update/get/edit/delete/list/clear_done/clear_all` — Task tracking
- `check_tool_availability` — Check external CLI tools
- `run_command` — Execute whitelisted external commands

### Registration Flow

```
tools::registry::register_tools(coordinator, settings, use_debug)
  → Settings.blacklist_set() determines is_allowed
  → register_core_tools()       — always available
  → register_todo_tools()       — always available
  → register_document_tools()   — if document-tools feature
  → register_skills_tools()     — if skills-tools feature
  → register_weather_tools()     — if weather-tools feature
  → register_calc_tools()        — if calc-tools feature
  → register_file_tools()        — if file-tools feature
  → register_system_tools()      — if system-tools feature
  → register_search_tools_serper — if serper-tools feature
  → register_search_tools_ddg   — if search-tools (no serper)
  → register_finance_tools()    — if finance-tools feature
  → register_led_tools()        — if led-tools + configured
  → register_pokemon_tools()    — if pokemon-tools feature
```

### Tool Check (tools/tool_check.rs)

`check_tool_availability` reads `tools.toml` (cached via OnceLock), checks `which::which(binary)` for installation, respects per-tool enabled/disabled flag and provides platform-specific install hints.

### Dynamic Prompt (prompts/tools.rs)

The system prompt only lists tools that are:
1. **Compiled in** (feature flag enabled at build time)
2. **Not blacklisted** (runtime config.toml blacklist)
3. **Available** (e.g., Serper requires API key, LED requires config)

---

## 5. Retrieval/RAG System

### Architecture

```
retrieval/mod.rs
  ├── context_builder.rs  — Build LLM context with optimal ordering
  └── search.rs           — Interactive search (hybrid BM25+vector)
```

### Context Builder Flow

```
build_context(session, db, embedding_client, query, system_prompt, config)
  1. System prompt (always first)
  2. Retrieved messages (if session has ≥5 messages + min interval)
     → perform_retrieval()
       → EmbeddingClient.embed(query)       — Generate query embedding
       → db.search_messages_hybrid()         — BM25 + vector similarity (RRF)
       → db.enrich_content_results_with_context() — Add assistant responses
       → format_retrieved_context()          — XML-formatted with citations
  3. First preserved messages (if middle compaction)
  4. Compacted summary (if present)
  5. Recent messages (last 10)
  6. Current query (added by caller)
```

### Hybrid Search (BM25 + Vector RRF)

- **Keyword weight**: 0.4 (BM25 via FTS5)
- **Semantic weight**: 0.6 (vector similarity via sqlite-vec)
- **Reciprocal Rank Fusion (RRF)**: Merges keyword and semantic results
- **Enrichment**: Pairs user messages with assistant responses

### Config Constants

- `MIN_MESSAGES_FOR_RETRIEVAL`: 5
- `RELEVANT_MESSAGES_COUNT`: 5
- `RECENT_MESSAGES_COUNT`: 10
- `MIN_RETRIEVAL_INTERVAL_SECS`: 5

---

## 6. Content System

### Architecture

```
content/mod.rs
  ├── types.rs    — ContentType enum, Note struct, ContentSearchResult
  ├── document.rs — Document, FileType, detect_file_type
  └── db.rs       — Content-specific DB operations (via db module)
```

### Unified `content_items` Table

Stores three types in a single table:
- **Messages** (from chat sessions) — content_type = "message"
- **Notes** (user-created persistent notes) — content_type = "note"
- **Documents** (imported files) — content_type = "document"

All share:
- FTS5 full-text search index
- Vector embeddings (sqlite-vec) for semantic search
- Decay-based relevance scoring

### Document Ingestion (`import_document` tool + `content/document.rs`)

Supported file types: TXT, MD, ORG, PDF (via pdftotext), EPUB (via ebook-convert/epub2txt)
- File type detection by extension
- Max document size constant
- External tool delegation for PDF/EPUB
- Content stored in `content_items` with chunks + embeddings

---

## 7. Facts/Memory System

### Architecture

```
facts/mod.rs
  ├── classify.rs  — Heuristic fact classification (no LLM)
  ├── conflict.rs  — Conflict detection (same topic, contradictory)
  ├── db.rs        — SQLite CRUD + FTS5 search + decay operations
  ├── decay.rs     — Time-based decay scoring + pruning
  ├── prompt.rs    — build_facts_section() for system prompt injection
  └── types.rs     — Category, Scope, Source, Fact struct
```

### Categories & Scope

| Category | Half-Life | Description |
|----------|-----------|-------------|
| `preference` | 180 days | User preferences |
| `fact` | 30 days | Objective facts about environment/project |

| Scope | Description |
|-------|-------------|
| `project` | Facts specific to current project |
| `global` | Facts that apply to all projects |

### Key Design Decisions
- Heuristic classification only (no LLM for classification)
- FTS5 keyword search only (no embeddings for facts)
- Hard limit: 500 chars per fact
- Soft limit: 2200 chars total in prompt
- Conflict detection: when same key has contradictory values, oldest invalidated
- Decay cycle runs at REPL startup, prunes facts below threshold

### Flow: Fact Injection into Prompt

```
core::send_message()
  → db.get_facts_for_prompt(project_id)  // Get active, valid facts
  → facts::prompt::build_facts_section(&facts)
  → build_session_system_prompt(... facts_section ...)
    → Injected as <facts> section in system prompt
```

---

## 8. Skills System

### Architecture

```
skills/mod.rs
  ├── loader.rs    — load_skill_indexes(), get_skill_content()
  ├── sanitize.rs  — Remove injection patterns from skill content
  └── types.rs     — SkillIndex, Skill, SkillSource enum
```

### Skill Sources (priority)

1. **Project**: `.ask-ai/skills/<name>/SKILL.md` (highest)
2. **User**: `~/.config/ask-ai/skills/<name>/SKILL.md`
3. **Builtin**: Embedded in binary via `include_str!` (lowest)

### Built-in Skills

- `document-processing` — Extract content from PDF/EPUB
- `ocr-images` — OCR processing
- `code-analysis` — Code review/analysis
- `web-scraping` — Web content extraction

### On-Demand Loading

1. System prompt includes `<available_skills>` section (names + descriptions only)
2. LLM sees relevant skill, calls `skill_view(name="document-processing")`
3. Full SKILL.md content returned to LLM as tool result
4. LLM follows skill instructions for subsequent actions

### In-Chat Skill Activation

- `/skill <name>` REPL command activates a skill for the session
- Active skill content injected into system prompt
- Stored in `session.active_skill: Option<ActiveSkill>`

---

## 9. Vision/OCR System

### Vision (`vision/`)

```
vision/mod.rs
  ├── cli.rs       — VisionArgs (files, model, prompt, json, max_tokens, detailed)
  ├── error.rs     — VisionError types
  └── processor.rs — VisionProcessor: base64 encode images, send to vision model
```

Flow:
1. Base64-encode image files
2. Build ChatMessage with image data
3. Send to vision-capable model via Ollama
4. Return structured `VisionResult` (content + metadata)

### OCR (`ocr/`)

```
ocr/mod.rs
  ├── cli.rs       — OcrArgs (files, mode, max_tokens, json)
  ├── error.rs     — OcrError types
  ├── mode.rs      — OcrMode enum (text, table, figure, formula)
  └── processor.rs — OcrProcessor: uses glm-ocr:bf16 model
```

Flow:
1. Load + base64-encode images
2. Build prompt based on OcrMode (text extraction, table, formula, etc.)
3. Send to `glm-ocr:bf16` model
4. Return structured results

Both use `CustomCoordinator` directly (not the full chat session pipeline).

---

## 10. Translation

### Architecture

```
translate/mod.rs
  ├── cli.rs       — Commands enum, TranslateArgs, CompletionArgs, QueryArgs, Shell
  ├── language.rs  — LanguageMapper, parse_language_pair() (100+ languages)
  ├── prompt.rs    — build_translation_prompt()
  └── style.rs     — TranslationStyle (literal, natural, formal, casual)
```

### Flow

1. Parse language pair: `en:pt` → (English, Portuguese), `:pt` → auto-detect
2. Get text from args or stdin
3. Build translation prompt with source/target/style
4. Use `translategemma:4b` model (or configured translate model)
5. Single-shot: system prompt = translation instructions, user message = empty
6. Render result (plain or markdown)

---

## 11. External Models

### Configuration

No OpenRouter/API integration in code — all models go through Ollama.
External models are configured as user model presets in `models.toml`:

```toml
[models.openrouter-gpt4]
model_id = "openrouter-gpt4"  # Must be served by local Ollama
num_ctx = 32768
temperature = 0.7
```

Ollama itself can proxy external models. Ask-AI connects to Ollama only.

### External CLI Tools (`external/`)

```
external/mod.rs
  ├── config.rs  — Load tools.toml (external tool whitelist + file_tools config)
  └── types.rs   — ExternalTool, ExternalToolsConfig, FileToolsConfig, Platform
```

- **tools.toml**: Defines which CLI programs the `run_command` tool can execute
- **Platform detection**: Arch, Debian, Fedora, Termux, Other — for install hints
- **Sandbox**: Landlock (Linux 5.13+) isolates filesystem access
- **FileToolsConfig**: max_file_size (5MB default), blocked_patterns, block_read, block_list

---

## 12. Subcommands

| Subcommand | Handler | Description |
|-----------|---------|-------------|
| `translate` | `handle_translate` | Text translation between languages |
| `query` | `handle_query_subcommand` → `run_query` | One-shot LLM query |
| `ocr` | `handle_ocr` | OCR from images using glm-ocr |
| `summarize` | `handle_summarize` | Text summarization (no tools) |
| `chat` | `handle_chat` → `run_chat_repl` | Interactive chat REPL |
| `vision` | `handle_vision` | Image analysis with vision model |
| `completion` | `handle_completion` | Generate shell completions |

### Chat REPL Commands (`chat/commands.rs`)

| Command | Description |
|---------|-------------|
| `/quit` | Exit |
| `/new` | New conversation session |
| `/forget --yes` | Clear + delete from database |
| `/model NAME` | Switch model |
| `/system PROMPT` | Change system prompt |
| `/save [NAME]` | Save session |
| `/load NAME` | Load session |
| `/export [FORMAT] [FILE]` | Export conversation |
| `/list` | List saved sessions |
| `/info` | Session info |
| `/context` | Context metrics |
| `/think` | Toggle thinking |
| `/tools` | Toggle tools |
| `/compact` | Compact conversation |
| `/tools-output LEVEL` | Set output level |
| `/debug` | Toggle debug |
| `/retry` | Regenerate last response |
| `/undo` | Remove last response |
| `/search QUERY` | Search history |
| `/reindex` | Re-embed all content |
| `/retrieval` | Toggle RAG |
| `/skill-NAME` | Activate skill |

---

## 13. View/UI System

### Architecture

```
chat/view/mod.rs
  ├── ChatView (trait) — Abstraction for output rendering
  └── terminal.rs — TerminalView (current implementation)
```

Future: TUI implementation via ratatui (trait kept for migration).

### Rendering Components

| Component | Module | Description |
|-----------|--------|-------------|
| **Markdown** | `markdown.rs` | termimad-based rendering with global skin (dark/light/mono) |
| **Spinner** | `spinner.rs` | indicatif ProgressBar + rattles presets, suspend/resume for tool output |
| **Status bar** | `chat/view/terminal.rs` | Model name, context %, tool count, session name |
| **Thinking display** | `chat/thinking.rs` | Render `<think>` tags in dim style |
| **Tool output** | `chat/custom_coordinator.rs` → ChatEvent | PreToolContent, ToolResult callbacks |

### Markdown Themes

Set via `display.skin` in config.toml:
- `dark` — Dark background, transparent
- `light` — Light background, transparent
- `mono` — Monochrome, bold/italic only

### Spinner

- Random braille/line/block animation preset per invocation
- `suspend_for_print()` — temporarily hide spinner for tool output
- Global `ACTIVE_SPINNER` (RwLock) for concurrent access

---

## Module Dependency Map

```
main.rs
  ├── chat/       ← core.rs ← coordinator.rs, custom_coordinator.rs, continuation.rs
  │                 ← session.rs ← db/, embeddings/
  │                 ← repl_state.rs ← capabilities, config, settings
  │                 ← repl.rs ← input/, view/, commands/, command_handlers/
  ├── query/       ← context.rs, coordinator.rs, executor.rs
  ├── tools/       ← registry.rs (register_tools), tool_check.rs, context.rs (with_context)
  ├── config.rs    ← ModelConfig (built-in presets)
  ├── user_models.rs ← merges with config.rs
  ├── settings.rs  ← Settings (TOML config)
  ├── capabilities.rs ← Ollama API
  ├── prompts/     ← base.rs, builder.rs, tools.rs, examples.rs
  ├── context.rs   ← AGENTS.md loading + sanitization
  ├── soul.rs      ← SOUL.md loading
  ├── retrieval/   ← context_builder.rs ← db/, embeddings/
  │                 ← search.rs
  ├── db/          ← connection.rs, schema.rs, operations.rs, query.rs, init.rs
  ├── embeddings/  ← client.rs, chunker.rs, chunk_config.rs, fallback.rs
  │                 ← recovery.rs, regenerate.rs, truncate.rs
  ├── content/     ← types.rs, document.rs, db.rs
  ├── facts/       ← classify.rs, conflict.rs, db.rs, decay.rs, prompt.rs, types.rs
  ├── skills/      ← loader.rs, sanitize.rs, types.rs
  ├── external/    ← config.rs (tools.toml), types.rs
  ├── vision/      ← cli.rs, processor.rs, error.rs
  ├── ocr/         ← cli.rs, processor.rs, error.rs, mode.rs
  ├── translate/   ← cli.rs, language.rs, prompt.rs, style.rs
  ├── summarize/   ← cli.rs, processor.rs
  ├── markdown.rs  ← termimad
  ├── spinner.rs   ← indicatif + rattles
  ├── tokens.rs    ← Token estimation
  ├── context_overflow.rs ← Thresholds + auto-compaction logic
  └── utils.rs     ← Shared utilities (parse_bool, format_size, read_stdin, etc.)
```

---

## Main Data Flows

### Flow 1: Single Query (one-shot)

```
User types: ask-ai "What is Rust?"
  → main() → Cli::parse()
  → handle_legacy_query() → get_query_legacy() → run_query()
    → QueryContextBuilder::build()
      → user_models::resolve_model_config()
      → ModelCapabilities::detect()
      → context::load_agents_md()
      → soul::load_soul_md()
      → prompts::builder::build_system_prompt()
      → db::init_database_core()
    → coordinator::build_query_coordinator() (CustomCoordinator + tools)
    → retrieval::build_query_context() (RAG if DB available)
    → executor::execute_query_with_retry()
      → coordinator.chat(messages)
        → Ollama HTTP API (POST /api/chat)
        → Tool execution loop (if tool calls in response)
        → Error recovery (retry up to 3x)
    → display_result() → markdown::print_markdown()
```

### Flow 2: Chat REPL Message Cycle

```
User types message in REPL
  → rustyline::read_line()
  → parse_command() — not a command
  → handle_user_message()
    → session.add_user_message()
      → SQLite insert (sync)
      → Embedding generation (async spawn)
    → check_and_compact_before_tool() — warn if context >75%
    → core::send_message()
      → db.get_facts_for_prompt() → facts::prompt::build_facts_section()
      → tools::todo::format_todos_for_prompt()
      → build_session_system_prompt()
      → setup_coordinator() → register_tools()
      → prepare_messages()
        → retrieval::build_context() (RAG)
        → Inject continuation ephemeral (if resuming)
      → coordinator.chat(messages)
        → Ollama API → tool calls → tool execution → retry
        → ChatEvent callbacks (pre-tool content, tool results, overflow)
      → process_chat_response()
        → strip_thinking_tags()
        → markdown::print_markdown()
        → parse_continuation_tag()
    → process_send_result()
      → handle_continuation() if needed (max 3)
      → auto_compact_if_needed() if context >88%
      → session.save_sqlite()
```

### Flow 3: RAG Retrieval

```
build_context() / build_query_context()
  → if should_retrieve (≥5 messages, ≥5s since last)
    → perform_retrieval()
      → EmbeddingClient.embed(query)         — Ollama embedding API
      → db.search_messages_hybrid()          — FTS5 BM25 + sqlite-vec cosine
        → RRF merge with keyword_weight=0.4, semantic_weight=0.6
      → db.enrich_content_results_with_context() — Add subsequent assistant messages
      → format_retrieved_context()           — XML with citation IDs
    → Insert retrieved context as system message
  → Add compacted summary (if middle compaction)
  → Add recent session messages
  → User query at end
```

### Flow 4: Tool Execution (inside CustomCoordinator.chat())

```
coordinator.chat(messages)
  → POST /api/chat to Ollama
  → Response contains tool_calls?
    → YES: For each tool_call:
      → Emit ChatEvent::PreToolContent (if any text before calls)
      → Look up tool in self.tools HashMap
      → tool.call(parameters) — Execute async
      → check_and_handle_context_overflow()
        → If emergency (97%): truncate result
        → If compaction needed (88%): emit ContextNeedsCompaction
      → Append Tool message to history
      → Re-POST /api/chat with updated history
      → Repeat until no more tool_calls
    → NO: Return final response
  → Error → classify_ollama_error → retry or fail
```

### Flow 5: Facts Memory Cycle

```
LLM calls fact_add(content="User prefers dark theme")
  → facts::classify::classify_fact(content)
    → Category: "preference" (heuristic keyword matching)
    → Scope: "global" (no project-specific keywords)
  → facts::db::insert_fact() → SQLite
  → On next message: db.get_facts_for_prompt()
    → facts::prompt::build_facts_section() → <facts> in system prompt

Decay cycle (run at REPL startup):
  → db.run_decay_cycle() → applies time-based decay_score reduction
  → Prunes facts below threshold (should_prune())
```

### Flow 6: Translation

```
ask translate en:pt "Hello"
  → parse_language_pair("en:pt", mapper) → (English, Portuguese)
  → build_translation_prompt(src, target, text, style)
  → user_models::resolve_model_config("translategemma")
  → CustomCoordinator::new(ollama, "translategemma:4b", [])
  → coordinator.chat([system_msg, empty_user_msg])
  → markdown::print_markdown(translated_text)
```

### Flow 7: Vision/OCR

```
ask vision photo.jpg -d "Describe this"
  → VisionProcessor::process()
    → base64::encode(image)
    → ChatMessage with image data
    → Send to vision-capable model
    → Return VisionResult

ask ocr document.png
  → OcrProcessor::process_batch()
    → base64::encode(image)
    → Build prompt based on OcrMode
    → Send to glm-ocr:bf16
    → Return OCR results
```

---

## Database Schema (Key Tables)

| Table | Purpose |
|-------|---------|
| `content_items` | Unified storage: messages, notes, documents (with role, conversation_id, project_id, content_type, message_type, prompt_tokens) |
| `content_chunks` | Text chunks for long content (chunk_index, content, start_offset, end_offset) |
| `content_embeddings` | sqlite-vec embeddings for chunks and items (vecf32, 768 dimensions, Matryoshka 256d) |
| `conversations` | Session metadata (model, think, tools, compacted_summary, compacted_range) |
| `facts` | Factual memory (scope, category, content, importance, decay_score, source, project_id) |
| `todos` | Per-session todo items |
| FTS5 virtual table | Full-text search index on content_items.content |