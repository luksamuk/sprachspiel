# Changelog

All notable changes to Ask-AI will be documented in this file.

## [Unreleased]

### Added

- Complete documentation with mdBook
- Man page for terminal reference
- Development documentation (architecture, roadmap, contributing)
- Mermaid diagram support
- New `read_file_segment` tool for reading specific line ranges from files
- Tool error handling guidelines in documentation

### Fixed

- Tool call errors now return friendly messages instead of crashing
- Code mode now respects `[model.code]` config properly
- Translate subcommand now respects `--plain` flag
- Per-subcommand configuration system (query, summarize, code)
- `read_file` debug output now shows "all" instead of empty when max_lines not specified

### Changed

- Tools now return informative error messages to help LLM recover from mistakes
- Documentation updated with tool error handling philosophy

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
- GPT-OSS tool calling may fail with encoding errors

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
