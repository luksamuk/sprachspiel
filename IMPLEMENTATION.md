# Implementation Plan for ask-ai

**Note**: This document has been reorganized. Detailed technical documentation is now in `doc/src/development/`.

## Quick Links

- [Architecture](./doc/src/development/architecture.md) - Design decisions and system architecture
- [Roadmap](./doc/src/development/roadmap.md) - Future features and planned improvements
- [Contributing](./doc/src/development/contributing.md) - How to contribute to the project

## Current Implementation Status

✅ **Completed:**

- Core CLI with 4 subcommands (query, translate, ocr, summarize)
- 14 tools (8 Pokémon, 3 Weather, 3 Web Search)
- Dynamic model selection
- Markdown rendering
- Model capability detection
- Tool integration
- Translation (50+ languages)
- OCR with multiple modes
- Summarization with styles
- Pipe support
- Debug mode
- Think mode
- Code mode
- Complete documentation with mdBook
- Man page

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
2. [Roadmap](./doc/src/development/roadmap.md) - What's coming next
3. [Contributing](./doc/src/development/contributing.md) - How to contribute

## Legacy Content

The original detailed implementation notes have been moved to:

- `doc/src/development/architecture.md` - Architecture decisions
- `doc/src/development/roadmap.md` - Future plans
- `doc/CHANGELOG.md` - Version history

## Last Updated

2026-02-17 - Documentation reorganization complete
