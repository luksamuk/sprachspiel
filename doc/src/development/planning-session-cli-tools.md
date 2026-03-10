# Planning Session: CLI Tools and Skills System

**Date:** 2026-03-09  
**Status:** Planning Complete  
**Related:** [Skills System Design](./skills-system-design.md), [CLI Tools Research](./cli-tools-research.md)

## Overview

This document summarizes the planning session for implementing CLI Tools and Skills System as alternatives to embedding heavy Rust PDF/OCR crates.

## Background

### Problem Statement

The roadmap previously planned to use Rust crates (`lopdf`, `pdf-extract`, `pdfium-render`) for PDF processing in the Document Import Tool. This approach has several drawbacks:

1. **Binary size**: Adding these crates increases binary size by 2-10MB
2. **Maintenance burden**: Crates need updates, bug fixes, and compatibility checks
3. **Limited OCR**: Rust crates cannot OCR scanned PDFs (need tesseract anyway)
4. **Termux compatibility**: Some crates don't compile well on Android/Termux

### Proposed Solution

Use external CLI tools (pdftotext, tesseract, exiftool, etc.) instead of Rust crates:

| Approach | Binary Size | Maintenance | OCR Support |
|----------|-------------|-------------|-------------|
| Rust Crates | +2-10MB | Developer | Limited |
| CLI Tools | ~0MB | System packages | Full (tesseract) |

## Research Findings

### CLI Tools Ecosystem

1. **PDF Processing** (Poppler)
   - `pdftotext` - Text extraction
   - `pdfinfo` - Metadata
   - `pdftoppm` - PDF to image conversion

2. **OCR** (Tesseract)
   - 100+ languages
   - Industry standard for OSS OCR

3. **Image Processing**
   - `exiftool` - Metadata (safe)
   - `imagemagick` - Manipulation (needs sandboxing)

4. **Security Considerations**
   - All tools: Use `std::process::Command` (no shell)
   - ImageMagick: Needs policy.xml hardening + sandbox
   - Timeout: Required for all commands

### Skills/Prompt Systems in Other Tools

Research on how other AI agents define skills:

| Tool | Format | Purpose |
|------|--------|---------|
| **Claude Code** | Markdown + YAML frontmatter | Instructions + tool definitions + hooks |
| **Cursor** | Plain Markdown | Project-level instructions |
| **Aider** | Markdown | Coding conventions |
| **OpenAI GPTs** | JSON Schema (web UI) | Instructions + actions |

**Key Insight:** Skills are **instructions for the model**, not executable code. Tools still require code implementation.

### Dynamic Tool Registration

Research on whether tools can be defined purely from data:

| Framework | Schema from Data? | Execution from Data? |
|-----------|-------------------|---------------------|
| ollama-rs | No (macro) | No (compile-time) |
| LangChain | No (decorators) | No |
| OpenAI | **Yes** (JSON) | **No** (code required) |
| Semantic Kernel | Partial (OpenAPI) | No |

**Conclusion:** All frameworks require code for execution. Data files can only define schemas/instructions, not implementation.

## Architecture Decision

### Skills = Instructions (Markdown)

Skills define **how the model should use tools**, not what tools do:

```
┌─────────────────────────────────────────────────┐
│  Skills (Markdown)                              │
│  ~/.config/ask-ai/skills/pdf-processing.md      │
│                                                 │
│  "When asked about PDFs:                        │
│   1. Check tool availability with                │
│      check_tool_availability('pdftotext')       │
│   2. If available, use run_command              │
│   3. If not, inform user about installation"    │
│                                                 │
│  ↑ Injections into system prompt                │
└─────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────┐
│  Tools (Rust Code)                              │
│  src/tools/tool_check.rs                        │
│                                                 │
│  #[ollama_rs::function]                         │
│  pub async fn check_tool_availability(...)      │
│                                                 │
│  ↑ Executable code (compile-time)               │
└─────────────────────────────────────────────────┘
```

### External Tools Configuration

Users configure which commands are allowed:

```toml
# ~/.config/ask-ai/tools.toml

[pdftotext]
enabled = true
timeout = 30

[tesseract]
enabled = true
timeout = 120

[imagemagick]
enabled = false  # Opt-in due to security
timeout = 60
sandbox = true
```

### Implementation Order

Based on dependencies and usefulness:

**Phase 1: CLI Tools Infrastructure** (3-4 days)
- `src/external/` module structure
- `ToolRegistry` with `which` crate
- `CommandExecutor` with async + timeout
- `tools.toml` parser
- `check_tool_availability()` tool
- `run_command()` tool

**Phase 2: Skills System** (2-3 days)
- `src/skills/` module structure
- `SkillsLoader` for Markdown files
- Builtin skills (pdf-processing, ocr-images)
- User skills (`~/.config/ask-ai/skills/`)
- Project skills (`.ask-ai/skills/`)
- Prompt injection

**Phase 3: Document Import** (2-3 days)
- Database schema
- `import_text_file()` for TXT/MD
- Skills for PDF handling
- Commands: `/import-doc`, `/list-docs`, `/remove-doc`

**Phase 4: Notes System** (1-2 days)
- Database schema
- Commands: `/note add/list/show/edit/delete`
- Embeddings for notes

**Phase 5: OCR/Vision Tools** (1-2 days)
- `extract_text_from_image()` via tesseract
- `get_image_metadata()` via exiftool
- Feature flags

## File Structure

```
src/
├── external/               # NEW
│   ├── mod.rs
│   ├── registry.rs        # Tool detection
│   ├── executor.rs        # Command execution
│   ├── config.rs          # tools.toml parser
│   └── sandbox.rs         # Future: landlock
├── skills/                 # NEW
│   ├── mod.rs
│   ├── loader.rs          # File loading
│   ├── types.rs           # Skill struct
│   └── builtin/           # Embedded in binary
│       ├── pdf-processing.md
│       └── ocr-images.md
└── tools/
    ├── tool_check.rs      # NEW
    └── run_command.rs     # NEW
```

## Configuration Files

```
~/.config/ask-ai/
├── config.toml            # Existing
├── models.toml            # Existing
├── tools.toml             # NEW: External tool whitelist
└── skills/                # NEW: User skills
    └── custom-skill.md

.ask-ai/
└── skills/                # NEW: Project skills
    └── project-skill.md
```

## Security Model

### Whitelist Enforcement

```rust
// Only whitelisted commands in tools.toml can execute
pub async fn run_command(command: String, ...) -> Result<String, ...> {
    let tool = registry.get(&command)?;
    if !tool.enabled {
        return Err("Tool disabled in configuration");
    }
    // ...
}
```

### Timeout

All commands have configurable timeout (default: 30s).

### No Shell

Never use shell interpretation:

```rust
// ✅ CORRECT: Direct execution
Command::new("pdftotext").args(["file.pdf", "-"]).output()

// ❌ WRONG: Shell interpretation
Command::new("sh").arg("-c").arg(&user_input)
```

### Future: Sandboxing

For high-risk tools (ImageMagick), consider Linux Landlock:

```rust
// Future implementation
pub fn execute_sandboxed(tool: &ExternalTool, args: &[String]) -> Result<Output> {
    // Restrict filesystem access with landlock crate
}
```

## Implementation Phases

### Phase 1: CLI Tools Infrastructure

**Files to create:**
- `src/external/mod.rs`
- `src/external/registry.rs`
- `src/external/executor.rs`
- `src/external/config.rs`
- `src/tools/tool_check.rs`
- `src/tools/run_command.rs`

**Dependencies to add:**
```toml
which = "8.0"        # Command detection
shell-words = "1.1"  # Safe argument parsing (optional)
```

**Tests:**
- Unit: Tool detection
- Unit: tools.toml parsing
- Integration: Command execution with timeout
- Integration: Error handling

### Phase 2: Skills System

**Files to create:**
- `src/skills/mod.rs`
- `src/skills/loader.rs`
- `src/skills/types.rs`
- `src/skills/builtin/pdf-processing.md`
- `src/skills/builtin/ocr-images.md`

**Changes:**
- `src/prompts/builder.rs` - Inject skills into system prompt

**Tests:**
- Unit: YAML frontmatter parsing
- Unit: Skill loading from paths
- Integration: Prompt composition

### Phase 3: Document Import

**Files to create:**
- `src/db/documents.rs` (new table)
- `src/tools/import_doc.rs`

**Changes:**
- `src/db/schema.rs` - Add documents and document_chunks tables
- `src/retrieval/search.rs` - Include documents in hybrid search
- `src/chat/repl.rs` - Add `/import-doc`, `/list-docs`, `/remove-doc`

**Tests:**
- Unit: Text chunking
- Integration: Import flow
- Integration: Search with documents

### Phase 4: Notes System

**Files to create:**
- `src/db/notes.rs` (new table)
- `src/tools/notes.rs`

**Changes:**
- `src/db/schema.rs` - Add notes table
- `src/retrieval/search.rs` - Include notes in hybrid search
- `src/chat/repl.rs` - Add `/note` commands

### Phase 5: OCR/Vision Tools

**Files to create:**
- `src/tools/ocr_external.rs`
- `src/tools/image_metadata.rs`

**Feature flags:**
```toml
[features]
ocr-tools = []
image-tools = []
```

## Documentation Updates

### Files Updated

- `doc/src/development/roadmap.md` - New priorities
- `doc/src/development/architecture.md` - New components
- `IMPLEMENTATION.md` - Updated plan
- `Cargo.toml` - New dependencies

### Files Created

- `doc/src/development/skills-system-design.md` - Full design
- `doc/src/development/cli-tools-research.md` - Tool reference

## Estimated Timeline

| Phase | Time | Dependencies |
|-------|------|--------------|
| CLI Tools Infrastructure | 3-4 days | None |
| Skills System | 2-3 days | None |
| Document Import | 2-3 days | CLI Tools + Skills |
| Notes System | 1-2 days | None |
| OCR/Vision Tools | 1-2 days | CLI Tools |
| **Total** | **9-14 days** | |

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Tools not installed | Graceful fallback + installation hints |
| Command injection | No shell, whitelist, validation |
| Performance | Timeout on all commands |
| Cross-platform | Document distro-specific install |
| Breaking changes | Semantic versioning + CHANGELOG |

## Success Criteria

1. **CLI Tools Infrastructure**
   - [ ] Tool detection works on all supported platforms
   - [ ] commands execute with timeout
   - [ ] Configuration file parses correctly
   - [ ] Tests pass

2. **Skills System**
   - [ ] Skills load from all paths
   - [ ] Skills inject into prompts correctly
   - [ ] Builtin skills embedded in binary
   - [ ] Tests pass

3. **Document Import**
   - [ ] TXT/MD import works without external tools
   - [ ] PDF import works with pdftotext installed
   - [ ] Graceful error when tools missing
   - [ ] Documents searchable

4. **Notes System**
   - [ ] Notes CRUD works
   - [ ] Notes searchable
   - [ ] Tests pass

## See Also

- [Skills System Design](./skills-system-design.md) - Detailed architecture
- [CLI Tools Research](./cli-tools-research.md) - Tool reference
- [Roadmap](./roadmap.md) - Updated priorities