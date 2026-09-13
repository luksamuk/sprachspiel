# TODO System & Status Bar

> **Status:** documentação técnica viva, extraída de `IMPLEMENTATION.md` durante a
> consolidação do backlog (LUC-140 / `refactor/backlog-consolidation`). O conteúdo é
> **verbatim** da fonte; apenas os cabeçalhos foram normalizados (eram marcados como
> "PRIORITY N … COMPLETED", resíduo de tracker).

O ciclo de vida dos TODOs entre sessões e a barra de status acima do prompt.

## TODO System Activation

**Status:** ✅ COMPLETED (v0.34.0)

**Goal:** Activate the existing TODO system to enable task tracking for both LLM and users.

**Problem Statement:**
- TODO system (`src/chat/todo_state.rs` and `src/tools/todo.rs`) was implemented but not integrated
- LLM tools registered but no synchronization with session state
- No user commands to manage TODOs interactively
- Tasks not persisted across sessions

**Solution:** Activate the TODO system with full integration.

**Implementation:**

| Component | Description | Status |
|-----------|-------------|--------|
| Tools sync | `load_from_session()` / `save_to_session()` functions | ✅ |
| User commands | `/todo add/list/update/clear-done/clear-all` | ✅ |
| Command handlers | `handle_todo_*` functions | ✅ |
| Prompt integration | `format_todos_for_prompt()` in system prompt | ✅ |
| Session persistence | Load/save todos with session in `repl.rs` | ✅ |

**Files Modified:**
- `src/tools/todo.rs` - Added `load_from_session()`, `save_to_session()`, `format_todos_for_prompt()`
- `src/chat/commands.rs` - Added `ChatCommand::TodoAdd/TodoList/TodoUpdate/TodoClearDone/TodoClearAll`
- `src/chat/command_handlers.rs` - Added `handle_todo_*` functions
- `src/chat/repl.rs` - Added command handling and session sync
- `src/prompts/builder.rs` - Added `todos` field to `PromptConfig`
- `src/chat/core.rs` - Added `todos_section` parameter to `build_session_system_prompt()`

**LLM Tools (already registered):**

```
todo_add(description)       // Add a new task
todo_list()                 // List all tasks
todo_update(id, status)     // Update task status
todo_clear_done()            // Clear completed tasks
todo_clear_all()             // Clear all tasks
```

**User Commands:**

```
/todo add <description>            // Add a new task
/todo list                          // List all tasks
/todo update <id> <status>          // Update task status (pending|in_progress|done)
/todo clear-done                    // Clear completed tasks
/todo clear-all                      // Clear all tasks
```

**Architecture:**

```
┌─────────────────────────────────────────┐
│           TODO SYSTEM FLOW              │
├─────────────────────────────────────────┤
│  Session Start                          │
│  └── load_from_session(session.todos)   │
│      └── Copies to global TODO_STATE    │
│                                         │
│  During Session                         │
│  ├── LLM calls todo_* tools            │
│  │   └── Operates on TODO_STATE        │
│  ├── User runs /todo commands          │
│  │   └── Operates on TODO_STATE        │
│  │   └── Syncs to session.todos       │
│  └── System prompt includes todos      │
│      └── format_todos_for_prompt()    │
│                                         │
│  Session End                            │
│  └── save_sqlite()                     │
│      └── session.todos.to_rows()      │
│          └── Database persistence      │
└─────────────────────────────────────────┘
```

**Estimated effort:** 0.5 day → **Actual:** 0.5 day

**Related:** Issue #25

---

---

## Status Bar Above Prompt

**Status:** ✅ COMPLETED (v0.37.2)

**Goal:** Add a dynamic status bar above the prompt input showing real-time context information.

**Implementation:**

| File | Changes |
|------|---------|
| `src/chat/view/mod.rs` | Added `StatusBarInfo` struct, `STATUS_BAR_LINES` constant, `format_status_bar()` method, visual truncation |
| `src/chat/repl_state.rs` | Added `get_status_bar_info()` method to ReplState |
| `src/chat/repl.rs` | Integrated status bar rendering before prompt, ANSI clear codes with terminal width detection, prompt `>>> ` |

**Features:**
- Model name, context usage (XX.XK/YYYK), progress bar with percentage
- Think/Tools indicators (🧠🔧) in status bar
- Colored progress bar: Green (< 50%), Yellow (50-75%), Red (> 75%)
- Fixed width (77 visual characters) to prevent overflow
- Clean prompt: `>>> ` (model and indicators moved to status bar)
- ANSI codes clear status bar and input lines based on terminal width
- Dynamic calculation using `calculate_context_metrics()`
- Unicode-aware width calculation using `unicode-width` crate

**Files Modified:**
- `src/chat/view/mod.rs` - `StatusBarInfo` struct with `format_status_bar()`, `truncate_visual()` helper
- `src/chat/repl_state.rs` - `get_status_bar_info()` method
- `src/chat/repl.rs` - `build_status_bar()`, `calculate_visual_lines()`, `build_clear_code()` helpers

**Technical Details:**
- Uses `termimad::terminal_size()` to detect terminal width
- Uses `unicode_width::UnicodeWidthStr` for proper character width (CJK, etc.)
- Calculates visual lines: `total_width.div_ceil(terminal_width).max(1)`
- Clears correct number of lines: 3 (status bar) + N (visual lines of input)
- Fallback to 1 line if terminal width unavailable

**Commits:**
- `8433736` docs: update CHANGELOG and IMPLEMENTATION for status bar feature
- `c20e2d1` feat: add status bar above prompt
- `a707f02` fix: correct spacing around separators in status bar
- `4bf6a78` fix: remove extra whitespace from status bar content line
- `fd7a28a` fix: use visual truncation for status bar content line
- `d288e50` fix: reduce status bar content width to 77 columns
- `3b51308` revert: remove status bar from spinner
- `5e03f46` feat: change prompt from '>' to '>>>'
- `921bd6f` docs: update CHANGELOG and IMPLEMENTATION with final status bar details
- `716fb50` feat: detect terminal width for ANSI clear codes

**Design Decision:**
Status bar during spinner ("Thinking...") was attempted but caused display issues with ANSI codes across different terminals. Reverted to simpler approach where status bar appears only above prompt.

**Known Limitations:**
- Emoji width may be imprecise (but user input typically doesn't contain emojis)
- Terminal width detection may fail in some environments (fallback to 1 line)
- Long input wrapping to many lines may still leave minor visual artifacts

**Related:** Issue #47

---

---
