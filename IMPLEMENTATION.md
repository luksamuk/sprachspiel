# Implementation Plan for ask-ai

**Note**: This document has been reorganized. Detailed technical documentation is now in `doc/src/development/`.

## Quick Links

- [Architecture](./doc/src/development/architecture.md) - Design decisions and system architecture
- [Roadmap](./doc/src/development/roadmap.md) - Future features and planned improvements
- [Contributing](./doc/src/development/contributing.md) - How to contribute to the project

## Current Implementation Status

✅ **Completed:**

- Core CLI with 5 subcommands (query, chat, translate, ocr, summarize)
- Interactive chat mode with persistent sessions
- Custom models support via `~/.config/ask-ai/models.toml`
- Built-in models: llama3.1, translategemma, glm-ocr (user models in config)
- Thinking support for cloud models (configurable via `thinking = true`)
- Dynamic model selection with capability detection
- Tool integration with error recovery (28 tools in 8 categories)
- Translation (50+ languages)
- OCR with multiple modes
- Summarization with styles
- Vision analysis
- Markdown rendering
- Pipe support
- Debug mode
- Think mode with visible thinking output
- Code mode
- Token metrics display (`/context`)
- Context management foundation
- Semantic search (`/search`) with hybrid retrieval (BM25 + vector + RRF)
- SQLite storage with sqlite-vec extension
- Embedding generation with Matryoshka truncation (768d → 256d)
- AGENTS.md context injection with security sanitization
- Complete documentation with mdBook
- Man page
- Termux/Android builds
- Error recovery for tool/network errors
- ChatSession integration (auto-save messages + embeddings) - v0.21.0
- `/migrate` command (JSON → SQLite) - v0.21.0
- `/reindex` command (rebuild embeddings) - v0.21.0
- Context overflow handling (auto-compaction at 80%) - v0.21.0
- Auto-retrieval (M relevant + N recent messages) - v0.21.0
- Context composition based on "Lost in the Middle" research - v0.21.0
- Message chunking for long messages (>1024 chars) - v0.22.0
- UTF-8 safe chunking with char boundary detection - v0.22.1
- Synchronous chunking (guaranteed persistence) - v0.22.2
- Embedding recovery on startup - v0.22.2
- Middle compaction (preserve first N + last N) - v0.22.2
- Auto-compaction at 72% warning and 80% overflow - v0.22.3
- Visual context utilization bar in /context - v0.22.3
- Remember tool for conversation recall - v0.23.0
- Conversation-aware retrieval (enrichment) - v0.24.0
- Project-aware query mode - v0.25.0
- `/clear` and `/forget` commands for session management - v0.22.5
- Source attribution in memory system (`SourceType` enum) - v0.26.1
- SQLite as primary storage (schema v4, `/restore`, auto-migration) - v0.26.x

 📋 **Planned:**

### High Priority

- **SQLite as Single Storage** (~85% complete)
  - ✅ Schema v4 with session metadata
  - ✅ `save_sqlite()` / `load_sqlite()` implemented
  - ✅ `/restore` command
  - ✅ Auto-migration on startup
  - ✅ Commands use SQLite (`/save`, `/load`, `/list`)
  - ❌ Cleanup: Remove `ConversationStorage` dependency from REPL

- **Memory Enhancement Part 1**
  - ✅ Phase 1: Source Attribution (v0.26.1)
  - ❌ Phase 2: Query Routing - Route queries to appropriate search targets
  - ❌ Phase 3: Timestamp Filtering - "what did I say yesterday?"

- **Chat Module Integration** - `/ocr`, `/vision`, `/translate`, `/summarize` from chat
  - Required dependency for Memory Enhancement Part 2
  - OCR/Vision needed to process scanned documents

- **Memory Enhancement Part 2** (Phases 4-5) - Blocked by Chat Module Integration
  - Phase 4: Schema Preparation - Multi-source support (docs, notes)
  - Phase 5: Document Ingestion - PDF/markdown indexing

### Medium Priority

- File session state tracking
- Skills system

### Low Priority

- Plugin system
- TUI (Terminal User Interface) with Ratatui-rs

## Streaming Architecture (Future)

The `ollama-rs` library (already included with `stream` feature) provides streaming capabilities:

```rust
// Streaming API
pub async fn send_chat_messages_stream(
    &self,
    request: ChatMessageRequest,
) -> Result<ChatMessageResponseStream>

// ChatMessage includes thinking content
pub struct ChatMessage {
    pub content: String,
    pub thinking: Option<String>,  // For DeepSeek R1, etc.
    // ...
}
```

**Current Status:** Non-streaming only (`send_chat_messages()`)
**Streaming Path:** `send_chat_messages_stream()` or `send_chat_messages_with_history_stream()`

**Implementation Considerations:**

1. **CLI Mode (current):** `termimad` is synchronous, requires block buffering
2. **TUI Mode (future):** Ratatui supports incremental rendering via `tui-markdown`
3. **Thinking Display:** Separate pane in TUI, inline dimmed text in CLI

See: `doc/src/development/roadmap.md` - TUI section for detailed streaming approach

## Documentation

Full documentation is available in the `doc/` directory:

```bash
# View user documentation
cd doc
mdbook serve

# Or build static site
mdbook build

# View man page
man ask-ai
```

## For Developers

See the development documentation:

1. [Architecture](./doc/src/development/architecture.md) - Technical architecture
2. [Roadmap](./doc/src/development/roadmap.md) - Future plans
3. [Contributing](./doc/src/development/contributing.md) - How to contribute
4. [Context Composition Design](./doc/src/development/context_composition_design.md) - v0.21.0 design decisions

## Legacy Content

The original detailed implementation notes have been moved to:

- `doc/src/development/architecture.md` - Architecture decisions
- `doc/src/development/roadmap.md` - Future plans
- `doc/src/CHANGELOG.md` - Version history

## Last Updated

2026-03-09 - v0.26.7: Dead code cleanup; updated roadmap to reflect actual implementation status
