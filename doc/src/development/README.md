# Development Documentation

Welcome to the Sprachspiel development documentation. This section contains technical information for contributors and developers.

## Quick Navigation

### Canonical Documents

| Document | Purpose | Status |
|----------|---------|--------|
| **[Implementation Directive](./implementation-directive.md)** | Definitive implementation direction for continuous learning | CANONICAL |
| **[Roadmap](./roadmap.md)** | Current development status and future plans | ACTIVE |
| **[Architecture](./architecture.md)** | Design decisions and patterns | ACTIVE |
| **[Contributing](./contributing.md)** | How to contribute | ACTIVE |

### Design Documents

| Document | Description |
|----------|-------------|
| [Skills System Design](./skills-system-design.md) | Skills architecture and implementation |
| [Context Continuity](./context-continuity.md) | Graceful interruption handling |
| [File Write Tools](./file-write-tools.md) | Planned file write tools |
| [Run Command Redesign](./run-command-redesign.md) | Security redesign for shell commands |
| [Chat Mode Design](./chat-mode-design.md) | Chat UX improvements |
| [Context Anatomy](./context-anatomy.md) | Context components breakdown |

### Research Background

| Document | Description |
|----------|-------------|
| [Research Index](./research/index.md) | Overview of research documents |
| [Papers Reference](./research/papers-reference.md) | arXiv links for MemOS, OpenClaw-RL, MemGPT |
| [Research Synthesis](./research/research-appendix.md) | Complete research synthesis |

## Current Development Focus

See **[Implementation Directive](./implementation-directive.md)** for the canonical implementation direction.

### Phase 1 (Current Priority)

1. `/feedback` command (good/bad/correction)
2. Feedback signal storage
3. Weight propagation
4. Context statistics enhancement

### Phase 2 (Next)

1. Feedback-weighted retrieval
2. Temporal decay implementation
3. Context composition improvements

### Phase 3 (Future)

1. Tool outcome tracking
2. Skill success tracking
3. User pattern learning

## Project Structure

```
sprachspiel/
├── Cargo.toml              # Dependencies
├── Makefile                # Build automation
├── man/
│   └── sprach.1                  # Man page
├── doc/
│   ├── book.toml          # mdBook configuration
│   └── src/               # Documentation source
│       ├── development/   # <-- You are here
│       │   ├── implementation-directive.md  # CANONICAL
│       │   ├── architecture.md
│       │   ├── provider-architecture.md     # Provider-agnostic layer
│       │   ├── research/  # Research background
│       │   └── ...
│       └── ...
├── src/
│   ├── main.rs            # Entry point
│   ├── config.rs          # Model configurations
│   ├── prompts.rs         # System prompts
│   ├── capabilities.rs    # Model capability detection
│   ├── chat/              # Chat module
│   ├── tools/             # Tool implementations
│   └── ...
├── AGENTS.md              # Development guidelines
├── IMPLEMENTATION.md      # Implementation status
└── README.md              # Project readme
```

## Build Commands

```bash
# Build
cargo build

# Build release
cargo build --release --features all-tools

# Run
cargo run -- "Query"

# Test
cargo test

# Format
cargo fmt

# Lint
cargo clippy -- -D warnings -A clippy::allow_attributes -A clippy::too_many_lines -A clippy::cognitive_complexity
```

## Documentation

Build documentation:

```bash
cd doc
mdbook serve          # Development server
mdbook build          # Build static site
```

## Contributing

See [Contributing Guide](./contributing.md).