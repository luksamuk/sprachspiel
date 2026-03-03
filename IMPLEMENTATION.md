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

📋 **Planned:**

- Chat module integration (`/ocr`, `/vision`, `/translate` from chat)
- File session state tracking
- Skills system
- Plugin system

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

2026-03-03 - v0.21.0: Design decisions documented
2026-03-03 - v0.22.0: Message chunking for semantic search
2026-03-03 - v0.22.1: UTF-8 safe chunking
