---
name: terminal-debug
description: Use terminal-use (tu) to interactively debug and instrument sprachspiel. Covers RUST_LOG tracing, DB inspection, REPL smoke tests, runtime state checks, and debug builds. Essential when the bug needs a live running session to reproduce or inspect.
license: MIT
compatibility: opencode
metadata:
  audience: developers
  workflow: terminal-debug
---

## What I do

I guide interactive debugging and instrumentation of sprachspiel using the **terminal-use** (`tu`) headless terminal tool. I cover when to use `tu` versus plain terminal, how to build and run debug/instrumented builds, environment variables for tracing, DB inspection, REPL smoke tests, and common debug workflows.

## When to use me

Load me when:
- Debugging a bug that requires a live REPL session to reproduce
- Setting up tracing/logging to see internal state (RUST_LOG, custom levels)
- Inspecting the sprachspiel database at runtime
- Running smoke tests interactively with `tu`
- Profiling or instrumenting a running sprachspiel instance
- Verifying a fix by stepping through the chat loop

**Don't use me for:** simple one-off commands (use regular `terminal` tool instead), or automated test runs (use `cargo test`).

---

# terminal-use (`tu`) Crash Course

`tu` spawns a headless virtual terminal. You can read the screen, send keystrokes, and take PNG screenshots. It is the ONLY way to interact with TUI programs (like the sprachspiel REPL) from an AI agent.

## Core Commands

```
# Spawn a session
tu run --name <session> --size <WxH> --cwd <path> -- <command>

# Read the screen
tu screenshot --name <session>
tu screenshot --png --name <session>            # As image for visual debugging

# Send input
tu type --name <session> -- "text to type"
tu press --name <session> Enter
tu press --name <session> Escape : w q Enter    # Multi-key sequence

# Wait for conditions
tu wait --name <session> --text "regex" --timeout 10000
tu wait --name <session> --stable 500           # Wait for screen to stop changing

# Manage sessions
tu list
tu status --name <session>
tu resize 160x50 --name <session>
tu kill --name <session>
```

## Key Rules

1. **Always use `--name`** — prevents collisions with the default session.
2. **Always use `--` before text** — prevents flag parsing issues with `tu type`.
3. **Wait after sending input** — simple keypress: 50-200ms; command execution: 500-2000ms; LLM response: 10-60s.
4. **Kill sessions when done** — `tu kill --name <session>` to clean up.
5. **Splash screen takes ~3s** — always `sleep 3` after `tu run` before sending commands.

---

# Debug Builds

## Release Build (with all tools)

```bash
cd ~/git/sprachspiel
cargo build --release --features all-tools
```

## Debug Build (with symbols, no optimization)

```bash
cargo build --features all-tools
```

Use debug builds when you need:
- Backtraces with line numbers (`RUST_BACKTRACE=1`)
- GDB/LLDB attachment
- Symbol resolution in panics

**⚠️ Debug builds are 5-10x slower and produce larger binaries.** Only use when you need stack traces.

## Instrumented Build (overflow checks + debug assertions)

```bash
# Debug assertions are ON by default in debug builds.
# For release with debug assertions:
CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true cargo build --release --features all-tools
```

---

# Environment Variables for Tracing

sprachspiel uses `log` crate macros (`info!`, `warn!`, `debug!`, `trace!`). The custom `MultiLogger` (SF4) routes logs to:
- **stderr** (colored, level controlled by verbosity)
- **File**: `~/.local/share/sprachspiel/sprachspiel.log` (rotation at 5 MB)

## Verbosity Levels

| Flag | stderr level | File level |
|------|-------------|------------|
| (none) | `warn` | `warn+` |
| `-v` | `info` | `info+` |
| `-vv` | `debug` | `debug+` |
| `-vvv` / `--trace` | `trace` | `trace+` |

## RUST_LOG Override

The `RUST_LOG` env var overrides the default level and enables **module-level filtering**:

```bash
# All modules at debug level
RUST_LOG=debug sprach chat

# Specific module at trace, rest at warn
RUST_LOG=warn,sprachspiel::chat::custom_coordinator=trace sprach chat

# Multiple modules
RUST_LOG=warn,sprachspiel::db=debug,sprachspiel::retrieval=debug sprach chat

# Trace the tool loop (most common debug target)
RUST_LOG=warn,sprachspiel::chat::custom_coordinator=trace sprach chat
```

### Key Modules to Trace

| Module | What it shows |
|--------|---------------|
| `sprachspiel::chat::custom_coordinator` | Tool loop iterations, message flow, overflow detection |
| `sprachspiel::db` | Database queries, fact storage, embeddings |
| `sprachspiel::retrieval` | RAG context building, hybrid search (BM25 + cosine) |
| `sprachspiel::facts` | Fact extraction, contradiction detection, decay |
| `sprachspiel::tools` | Tool registration, external tool checking |
| `sprachspiel::chat::repl_state` | REPL state mutations, model switches |
| `sprachspiel::embeddings` | Embedding generation, chunking, fallback |

### Viewing Log Files

```bash
# Tail the log file in real-time
tail -f ~/.local/share/sprachspiel/sprachspiel.log

# Search for errors
grep -i "error" ~/.local/share/sprachspiel/sprachspiel.log

# Search for specific module traces
grep "custom_coordinator" ~/.local/share/sprachspiel/sprachspiel.log | tail -50
```

---

# Interactive Debug Workflows

## Workflow 1: Basic REPL Smoke Test

```bash
# Build first
cd ~/git/sprachspiel && cargo build --release --features all-tools

# Spawn REPL session (soulless = faster startup, no SOUL.md)
tu run --name debug-repl --size 80x50 --cwd ~/git/sprachspiel -- \
  target/release/sprach --soulless --ignore-agents chat

# Wait for splash screen
sleep 3
tu screenshot --name debug-repl

# Test a command
tu type --name debug-repl -- "/help"
tu press --name debug-repl Enter
sleep 1
tu screenshot --name debug-repl

# Clean up
tu kill --name debug-repl
```

## Workflow 2: Trace the Tool Loop

```bash
# Spawn with trace logging on coordinator
tu run --name trace-session --size 120x60 --cwd ~/git/sprachspiel -- \
  env RUST_LOG=warn,sprachspiel::chat::custom_coordinator=trace \
  target/release/sprach --trace chat

# Wait for startup
sleep 3

# Send a prompt that triggers tool use
tu type --name trace-session -- "What time is it?"
tu press --name trace-session Enter

# Wait for response (LLM calls tool, gets result)
sleep 15
tu screenshot --name trace-session

# Check the log file for tool loop details
# (exit the session first, or read in parallel)
tail -100 ~/.local/share/sprachspiel/sprachspiel.log | grep -i "tool_call\|tool_result\|overflow"
```

## Workflow 3: Debug Fact Extraction

```bash
# Trace the facts pipeline
tu run --name facts-debug --size 120x60 --cwd ~/git/sprachspiel -- \
  env RUST_LOG=warn,sprachspiel::facts=debug \
  target/release/sprach chat

sleep 3

# Tell it something to remember
tu type --name facts-debug -- "My name is Alice and I prefer dark mode"
tu press --name facts-debug Enter
sleep 10

# Ask it to recall
tu type --name facts-debug -- "What's my name?"
tu press --name facts-debug Enter
sleep 10
tu screenshot --name facts-debug

# Verify in DB
# (in a separate terminal, not tu)
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db \
  "SELECT predicate, object, importance, category FROM facts ORDER BY rowid DESC LIMIT 10;"
```

## Workflow 4: Debug DB Issues

```bash
# Full DB instrumentation
tu run --name db-debug --size 120x60 --cwd ~/git/sprachspiel -- \
  env RUST_LOG=warn,sprachspiel::db=trace \
  target/release/sprach chat

sleep 3

# Trigger DB operations
tu type --name db-debug -- "/fact list"
tu press --name db-debug Enter
sleep 2
tu screenshot --name db-debug

# Direct DB inspection (separate terminal)
# Check schema version
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA user_version;"

# Check content items
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db \
  "SELECT id, content_type, LENGTH(content), importance, created_at FROM content_items ORDER BY id DESC LIMIT 10;"

# Check embeddings
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db \
  "SELECT COUNT(*) FROM vec_embeddings;"

# Check facts
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db \
  "SELECT id, predicate, object, importance, category FROM facts ORDER BY id DESC LIMIT 20;"
```

## Workflow 5: Debug Context Overflow

```bash
# Trace overflow thresholds (75% warning, 88% compaction, 94% inter-tool, 97% emergency)
tu run --name overflow-debug --size 120x60 --cwd ~/git/sprachspiel -- \
  env RUST_LOG=warn,sprachspiel::chat::custom_coordinator=trace,sprachspiel::context_overflow=debug \
  target/release/sprach chat

sleep 3

# Send a long conversation to trigger overflow
# (paste multi-line content with tu paste)
tu paste --name overflow-debug -- "$(cat /tmp/long_document.txt)"
tu press --name overflow-debug Enter

# Monitor overflow decisions in logs
tail -f ~/.local/share/sprachspiel/sprachspiel.log | grep -i "overflow\|compact\|threshold\|emergency"
```

## Workflow 6: Debug Model Switching

```bash
# Trace model state changes
tu run --name model-debug --size 120x60 --cwd ~/git/sprachspiel -- \
  env RUST_LOG=warn,sprachspiel::chat::repl_state=debug,sprachspiel::chat::model_switch=debug \
  target/release/sprach chat

sleep 3

# Switch models
tu type --name model-debug -- "/model qwen3.5:0.8b"
tu press --name model-debug Enter
sleep 2
tu screenshot --name model-debug

# Check model is loaded
tu type --name model-debug -- "/info"
tu press --name model-debug Enter
sleep 1
tu screenshot --name model-debug
```

---

# Database Inspection Quick Reference

The sprachspiel DB lives at `~/.local/share/sprachspiel/sprachspiel.db`.

```bash
# Schema version (must be ≥ 12)
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA user_version;"

# List all tables
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db ".tables"

# Content items (messages, notes, docs)
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db \
  "SELECT id, content_type, role, LENGTH(content), importance FROM content_items ORDER BY id DESC LIMIT 20;"

# Facts with decay info
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db \
  "SELECT id, predicate, object, importance, category, half_life_days FROM facts ORDER BY id DESC LIMIT 20;"

# Embedding counts per table
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db \
  "SELECT 'content' as kind, COUNT(*) FROM vec_content_embeddings
   UNION ALL SELECT 'fact', COUNT(*) FROM vec_fact_embeddings
   UNION ALL SELECT 'chunk', COUNT(*) FROM vec_chunk_embeddings;"

# Conversations
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db \
  "SELECT id, title, created_at FROM conversations ORDER BY id DESC LIMIT 10;"
```

---

# Common Flags for Debug Sessions

| Flag | Purpose |
|------|---------|
| `--soulless` | Skip SOUL.md loading (faster startup, neutral personality) |
| `--ignore-agents` | Skip AGENTS.md loading (faster startup, no project context) |
| `--trace` | Set RUST_LOG to trace (equivalent to `-vvv`) |
| `-vv` | Debug-level logging |
| `-v` | Info-level logging |
| `--db <PATH>` | Use custom DB path (isolated testing, won't touch real data) |
| `--model <MODEL>` | Override default model for this session |

## Workflow 7: Diagnose Busy-Wait in Event Loop (High Idle CPU)

High CPU usage in idle TUI mode often means the event loop is busy-waiting.
Zero-duration polls (`poll(0)`) or unconditional yields (`yield_now()`) are
typical culprits. This workflow measures the idle loop rate to confirm.

```bash
# 1. Instrument: add loop counter instrumentation to the event loop
#    (add before the event loop in src/chat/repl_tui.rs)
#    let mut _loop_count = 0u64;
#    let mut _loop_start = Instant::now();
#
# 2. Build and spawn session
tu run --name busywait-debug --size 80x50 --cwd ~/git/sprachspiel -- \
  target/release/sprach --soulless --ignore-agents chat

sleep 3

# 3. Let it idle for 10 seconds, then inspect scrollback
tu screenshot --name busywait-debug
# Check scrollback for [PERF] output if you added eprintln instrumentation

# 4. Confirm rate: if you see >1000 iters/sec, it's a busy-wait.
#    Normal rate for a 120ms poll should be ~8 iters/sec.

# 5. Fix: change poll timeout from 0ms to SPINNER_TICK_MS (e.g. 120ms)
#    Also remove/replace any yield_now() that follows a poll(0).

# 6. Rebuild and re-measure to confirm the fix
tu kill --name busywait-debug
```

### Isolated Debug Session (no data pollution)

```bash
# Use a temporary database — won't affect your real data
tu run --name isolated-debug --size 80x50 --cwd ~/git/sprachspiel -- \
  target/release/sprach --soulless --ignore-agents --db /tmp/debug-sprachspiel.db chat

# Clean up after
rm -f /tmp/debug-sprachspiel.db
```

---

# Common Pitfalls

1. **Not waiting after `tu run`**: The splash screen (ASCII art banner) takes ~3s to render. Always `sleep 3` (or `tu wait --text ">>>"`) before sending commands.

2. **Quote handling in `tu type`**: Always use `--` before text with special characters. For commands with `--flags`, wrap carefully: `tu type --name s -- "/todo add \"Buy groceries\""`.

3. **Session leaks**: Always `tu kill --name <session>` when done. Use `tu list` to check for stale sessions.

4. **Wrong terminal size**: Some TUI elements (status bars, ASCII art) need minimum 80x40. Use `--size 80x50` or larger. For trace-heavy sessions, use `120x60`.

5. **LLM response latency**: Model responses can take 10-60 seconds. Use `tu wait --text "regex" --timeout 30000` instead of fixed `sleep` when possible.

6. **DB path confusion**: `--db` only works in chat mode, not query mode. Query mode always uses `~/.local/share/sprachspiel/sprachspiel.db`.

7. **Log file rotation**: When `sprachspiel.log` exceeds 5 MB, it rotates to `sprachspiel.log.1`. Old logs are in the `.1` file.

8. **RUST_LOG vs `--trace`**: `--trace` sets ALL modules to trace level. Use `RUST_LOG` for targeted module-level tracing — it's much more readable.

9. **`--soulless --ignore-agents` for debug**: Always use these flags in debug sessions unless you specifically need to test SOUL.md or AGENTS.md behavior. They skip unnecessary I/O and make sessions reproducible.

10. **Backtraces**: For panics, set `RUST_BACKTRACE=1` (or `RUST_BACKTRACE=full`). Without it, you only get the panic message, no stack trace.

---

# Verification Checklist

- [ ] `tu` is installed and daemon is running (`tu daemon status`)
- [ ] Build succeeds: `cargo build --release --features all-tools`
- [ ] Session spawns without errors: `tu run --name test -- target/release/sprach --soulless chat`
- [ ] Splash screen appears after 3s wait
- [ ] `/help` command responds
- [ ] `tu kill` cleans up the session
- [ ] Log file exists: `~/.local/share/sprachspiel/sprachspiel.log`
- [ ] DB is reachable: `sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA user_version;"`