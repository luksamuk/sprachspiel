# Chat Mode Design Plan

This document outlines the detailed design for the interactive multi-line chat mode feature.

## Overview

Implement an interactive `chat` subcommand with REPL, persistent conversation history per Git project (identified by remote URL or folder name as fallback), and anonymous in-memory sessions.

## Goals

- Interactive REPL with command history
- Persistent conversation storage by project
- Anonymous sessions (in-memory only)
  - Rich commands: /quit, /new, /model, /help, /system, /save, /load, /export, /list, /info
- Rich UI with indicatif spinner and termimad markdown rendering

## File Structure

```
src/
├── main.rs              # Add handle_chat() handler
├── chat/
│   ├── mod.rs           # Chat module exports
│   ├── cli.rs           # ChatArgs (subcommand)
│   ├── session.rs       # ChatSession, ConversationStorage
│   ├── history.rs       # Legacy JSON storage (for /restore)
  │   ├── commands.rs      # Internal commands (/quit, /new, etc.)
│   ├── core.rs          # Core business logic (send_message, compact)
│   ├── repl_state.rs    # ReplState - consolidated state management
│   ├── model_switch.rs  # Centralized model switching
│   ├── custom_coordinator.rs  # Pre-tool content + ephemeral messages
│   ├── input/           # Input abstraction layer
│   │   ├── mod.rs       # InputBackend trait
│   │   └── rustyline.rs # RustylineInput implementation
│   ├── view/            # Output abstraction layer
│   │   ├── mod.rs       # ChatView trait
│   │   └── terminal.rs  # TerminalView implementation
│   └── repl.rs          # REPL coordinator (entry point)
├── project.rs           # Project identification (get_project_id)
```

### Architecture Layers

The chat module follows a layered architecture for maintainability and future TUI migration:

```
Layer 5: repl.rs           - Entry point, coordinator
Layer 4: core.rs           - Business logic (send_message, compact)
Layer 3: repl_state.rs     - State management (ReplState)
Layer 2: input/, view/     - I/O implementations (rustyline, terminal)
Layer 1: session.rs, cli.rs - Session and CLI handling
Layer 0: input/mod.rs, view/mod.rs - Traits (abstractions)
```

This separation enables:
- **Testing**: Each layer can be tested in isolation
- **TUI Migration**: Swap rustyline for ratatui input/output
- **Maintainability**: 200-400 line modules vs 1100+ line function

## Data Structures

### ChatSession

```rust
pub struct ChatSession {
    pub id: String,                    // UUID
    pub name: Option<String>,          // Optional session name
    pub project_id: Option<String>,    // git remote or folder name
    pub model: String,                 // Model preset name
    pub system_prompt: Option<String>, // Custom system prompt
    pub messages: Vec<ChatMessage>,    // Conversation history
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub anonymous: bool,               // If true, never persist
}
```

### ConversationStorage

**DEPRECATED:** Only used for legacy `/restore` command. SQLite is the primary storage.

```rust
pub struct ConversationStorage {
    base_path: PathBuf,  // ~/.local/share/sprachspiel/conversations/
}
```

## Project Identification

The project ID is used to organize conversations by project.
**Note:** `get_project_id()` moved to `src/project.rs` in v0.28.0.

```rust
// In src/project.rs
pub fn get_project_id() -> Option<String> {
    // 1. Try git remote origin URL
    // 2. Fallback: current folder name
}
```

### Directory Structure

```
~/.local/share/sprachspiel/
└── conversations/
    └── github.com/
        └── user/
            └── repo/
                ├── default.json        # Default session for project
                ├── bugfix-auth.json    # Named sessions
                └── feature-x.json
```

## CLI Arguments

```rust
#[derive(Args, Debug, Clone)]
pub struct ChatArgs {
    /// Anonymous session (no history persistence)
    #[arg(short, long)]
    pub anonymous: bool,

    /// Load a named session
    #[arg(short, long, value_name = "SESSION")]
    pub load: Option<String>,

    /// Model to use (overrides config)
    #[arg(short, long)]
    pub model: Option<String>,

    /// Enable think mode
    #[arg(short, long)]
    pub think: bool,
}
```

## REPL Loop

```rust
pub async fn run_chat_repl(settings: &Settings, args: &ChatArgs) -> AppResult<()> {
    // 1. Initialize session (new or load)
    // 2. Build coordinator with tools
    // 3. Print welcome message
    // 4. Loop: read input, parse command or send message
    // 5. Save session after each message (if not anonymous)
}
```

## Internal Commands

| Command | Description |
|---------|-------------|
| `/quit` `/exit` | Exit chat |
| `/new` | Clear current session history |
| `/help` | Show available commands |
| `/model <name>` | Switch model mid-session |
| `/system <prompt>` | Change system prompt |
| `/think` | Toggle think mode |
| `/tools` | Toggle tools |
| `/compact` | Compact conversation history |
| `/save [name]` | Save session (optional name) |
| `/load <name>` | Load existing session |
| `/export <format> [file]` | Export to markdown/json |
| `/list` | List project sessions |
| `/info` | Show current session info |

## Dependencies

```toml
[dependencies]
rustyline = "14"        # Readline support for REPL
chrono = { version = "0.4", features = ["serde"] }    # Timestamps
serde_json = "1"        # JSON persistence (already exists)
```

## Session Format (v0.13.0)

```json
{
  "id": "default",
  "name": null,
  "project_id": "github.com/user/repo",
  "model": "lfm",
  "system_prompt": null,
  "messages": [...],
  "compacted_summary": null,
  "messages_sent_to_llm": 0,
  "created_at": "2026-02-19T10:00:00Z",
  "updated_at": "2026-02-19T10:30:00Z",
  "anonymous": false,
  "think": false,
  "tools": true
}
```

## Integration with Coordinator

The Coordinator supports passing message history:

```rust
let messages: Vec<ChatMessage> = session.messages.clone();
let response = coordinator.chat(messages).await?;
```

## Rendering

Use existing components:
- `indicatif` for spinner during API calls
- `termimad` for markdown rendering of responses

## Error Handling

All errors should be displayed gracefully without crashing the REPL. Use pattern:

```rust
match result {
    Ok(_) => {},
    Err(e) => {
        eprintln!("Error: {}", e);
        continue;  // Keep REPL running
    }
}
```

## Estimated Time

| Phase | Tasks | Estimate |
|-------|-------|----------|
| 1 | Base structure | 2h |
| 2 | Project identification | 1h |
| 3 | Persistence | 2h |
| 4 | REPL loop | 3h |
| 5 | Commands | 3h |
| 6 | Coordinator integration | 2h |
| 7 | Tests and docs | 2h |
| **Total** | | **~15h** |

## Status

- [x] Design complete
- [x] Implementation complete (v0.12.0)