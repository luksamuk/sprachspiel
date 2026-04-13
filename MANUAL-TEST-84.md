# Manual Test Plan — PR #84: Refactor parse_command Complexity

Tests the elimination of `CommandResult` enum, `SessionSubcommand` enum,
extracted subcommand parsers, and consolidated shortcut handlers.

**Build:** `cargo build --release --features all-tools`

---

## 1. /session commands (Phase 3.7 — SessionSubcommand elimination)

The `/session` command now returns canonical `ChatCommand` variants directly
instead of wrapping in `ChatCommand::Session { SessionSubcommand }`.

| #  | Command | Expected | Fail if |
|----|---------|----------|---------|
| 1.1 | `/session new` | New session started, "[i] Previous conversations remain searchable" | Error or crash |
| 1.2 | `/session save m84` | Session saved as "m84" | Error |
| 1.3 | `/session list` | Lists sessions including "m84" | Empty list or error |
| 1.4 | `/session new` | New clean session | Messages persist from old session |
| 1.5 | `/session load m84` | Loads session "m84" | Error or not found |
| 1.6 | `/session save` | Saves session (auto-name) | Error |
| 1.7 | `/session forget` | Forgets session, starts fresh | Error or data persists |
| 1.8 | `/session` | Usage: /session <new\|load\|list\|save\|forget> | Crash or silence |
| 1.9 | `/session load` | Usage: /session load <name> | Crash or silence |
| 1.10 | `/session xyz` | Usage: /session <new\|load\|list\|save\|forget> | Crash or silence |

## 2. Direct equivalents (must behave identically to /session subcommands)

| #  | Command | Expected | Fail if |
|----|---------|----------|---------|
| 2.1 | `/new` | Same as `/session new` | Different behavior |
| 2.2 | `/n` | Same as `/new` (shortcut) | Not recognized |
| 2.3 | `/save m84b` | Same as `/session save m84b` | Different behavior |
| 2.4 | `/load m84b` | Same as `/session load m84b` | Different behavior |
| 2.5 | `/l m84b` | Same as `/load m84b` (shortcut) | Not recognized |
| 2.6 | `/list` | Same as `/session list` | Different behavior |
| 2.7 | `/ls` | Same as `/list` (shortcut) | Not recognized |
| 2.8 | `/forget` | Same as `/session forget` | Different behavior |

## 3. Toggle commands (affected by CommandResult elimination — now inline)

These commands now toggle state directly in `handle_command` instead of
returning `ThinkToggled(bool)`, `ToolsToggled(bool)`, etc.

| #  | Command | Expected | Fail if |
|----|---------|----------|---------|
| 3.1 | `/think` | Toggle think mode ON, show confirmation | No feedback or crash |
| 3.2 | `/think` | Toggle think mode OFF, show confirmation | No feedback or crash |
| 3.3 | `/t` | Shortcut for `/think` — same toggle behavior | Not recognized |
| 3.4 | `/tools` | Toggle tools ON, show confirmation | No feedback or crash |
| 3.5 | `/tools` | Toggle tools OFF, show confirmation | No feedback or crash |
| 3.6 | `/retrieval` | Toggle retrieval ON, show confirmation | No feedback or crash |
| 3.7 | `/retrieval` | Toggle retrieval OFF, show confirmation | No feedback or crash |
| 3.8 | `/tools-output compact` | Set tool output to compact, confirm | Error or no feedback |
| 3.9 | `/tools-output full` | Set tool output to full, confirm | Error or no feedback |
| 3.10 | `/to compact` | Shortcut for `/tools-output compact` | Not recognized |
| 3.11 | `/debug` | Toggle debug mode ON, show confirmation | No feedback or crash |
| 3.12 | `/debug` | Toggle debug mode OFF, show confirmation | No feedback or crash |

## 4. Action commands (affected by CommandResult elimination)

These had special `CommandResult` variants (`Compact`, `Retry`, `Undo`,
`Context`, `Search`, `Reindex`) that are now handled inline.

| #  | Command | Expected | Fail if |
|----|---------|----------|---------|
| 4.1 | `/compact` | Compacts conversation, shows progress | Error or crash |
| 4.2 | `/context` | Shows token usage metrics | Error or crash |
| 4.3 | `/ctx` | Shortcut for `/context` — same output | Not recognized |
| 4.4 | `/retry` | Retries last message | Error when no history |
| 4.5 | `/r` | Shortcut for `/retry` | Not recognized |
| 4.6 | `/undo` | Removes last response, shows last input | Error when no history |
| 4.7 | `/u` | Shortcut for `/undo` | Not recognized |
| 4.8 | `/reindex` | Reindexes embeddings, shows progress | Error or crash |
| 4.9 | `/search test` | Searches conversation for "test" | Error or crash |
| 4.10 | `/f test` | Shortcut for `/search test` | Not recognized |

## 5. /export (moved from execute_command to handle_export handler)

| #  | Command | Expected | Fail if |
|----|---------|----------|---------|
| 5.1 | `/export md` | Exports conversation as Markdown to stdout | Error or crash |
| 5.2 | `/export json` | Exports conversation as JSON to stdout | Error or crash |
| 5.3 | `/export md test_export.md` | Exports to file "test_export.md" | Error or crash |
| 5.4 | `/e md` | Shortcut for `/export md` | Not recognized |

## 6. /skill (content loading moved from CommandResult to handler)

Previously: `CommandResult::Skill { name, content }` loaded content in `execute_command`.
Now: `ChatCommand::Skill { name }` loads content in `handle_command`.

| #  | Command | Expected | Fail if |
|----|---------|----------|---------|
| 6.1 | `/skill document-processing` | Activates skill, shows confirmation | Error |
| 6.2 | `/skill nonexistent` | "Skill 'nonexistent' not found" error | Crash or silence |

## 7. /fact + 2-letter shortcuts (extracted parser + shortcut consolidation)

| #  | Command | Expected | Fail if |
|----|---------|----------|---------|
| 7.1 | `/fact add Water boils at 100°C` | Fact added, receives ID | Error |
| 7.2 | `/fa Earth orbits the Sun` | Shortcut for `/fact add` | Not recognized |
| 7.3 | `/fact list` | Lists facts | Error or empty |
| 7.4 | `/fl` | Shortcut for `/fact list` | Not recognized |
| 7.5 | `/fact search water` | Finds relevant facts | Error |
| 7.6 | `/fs water` | Shortcut for `/fact search` | Not recognized |
| 7.7 | `/fact remove 1` | Removes fact ID 1 | Error or not found |
| 7.8 | `/fr 2` | Shortcut for `/fact remove` | Not recognized |
| 7.9 | `/fact prune` | Prunes old facts | Error |
| 7.10 | `/fp` | Shortcut for `/fact prune` | Not recognized |
| 7.11 | `/fact add` | Usage error (no content) | Crash or silence |
| 7.12 | `/fa` | Usage error (no content) | Crash or silence |
| 7.13 | `/fact add Test --global` | Adding global fact, content = "Test" | Error or global=false |
| 7.14 | `/fact list --global` | Lists global facts | Error |

## 8. /note + 2-letter shortcuts (extracted parser + shortcut consolidation)

| #  | Command | Expected | Fail if |
|----|---------|----------|---------|
| 8.1 | `/note add Buy milk --title "Shopping"` | Note added with title | Error |
| 8.2 | `/na Call dentist --title Health` | Shortcut for `/note add` | Not recognized |
| 8.3 | `/note list` | Lists notes | Error |
| 8.4 | `/nl` | Shortcut for `/note list` | Not recognized |
| 8.5 | `/note show 1` | Shows note ID 1 | Error or not found |
| 8.6 | `/ns 1` | Shortcut for `/note show` | Not recognized |
| 8.7 | `/note delete 1` | Deletes note 1 | Error |
| 8.8 | `/nd 2` | Shortcut for `/note delete` | Not recognized |
| 8.9 | `/note search milk` | Searches notes | Error |
| 8.10 | `/note add` | Usage error (no content) | Crash or silence |
| 8.11 | `/note list --global` | Lists global notes | Error |
| 8.12 | `/note list 2` | Shows page 2 of notes | Error |

## 9. /doc + 2-letter shortcuts (extracted parser + shortcut consolidation)

| #  | Command | Expected | Fail if |
|----|---------|----------|---------|
| 9.1 | `/doc list` | Lists documents (may be empty) | Error |
| 9.2 | `/dl` | Shortcut for `/doc list` | Not recognized |
| 9.3 | `/doc show` | Usage error (no ID) | Crash or silence |
| 9.4 | `/doc show abc` | "Invalid document ID" error | Crash |
| 9.5 | `/doc delete` | Usage error (no ID) | Crash or silence |
| 9.6 | `/di` | Usage error (no path) | Crash or silence |

## 10. /todo + 2-letter shortcuts (extracted parser + shortcut consolidation)

| #  | Command | Expected | Fail if |
|----|---------|----------|---------|
| 10.1 | `/todo add Buy groceries` | Todo added, receives ID | Error |
| 10.2 | `/ta Pay bills --priority high` | Todo added with priority | Error |
| 10.3 | `/todo list` | Lists todos | Error |
| 10.4 | `/tl` | Shortcut for `/todo list` | Not recognized |
| 10.5 | `/todo get 1` | Shows todo ID 1 | Error |
| 10.6 | `/tg 1` | Shortcut for `/todo get` | Not recognized |
| 10.7 | `/todo update 1 done` | Updates status to "done" | Error |
| 10.8 | `/tu 2 pending` | Shortcut for `/todo update` | Not recognized |
| 10.9 | `/todo delete 1` | Deletes todo 1 | Error |
| 10.10 | `/td 2` | Shortcut for `/todo delete` | Not recognized |
| 10.11 | `/todo clear-done` | Clears completed todos | Error |
| 10.12 | `/tcd` | Shortcut for `/todo clear-done` | Not recognized |
| 10.13 | `/todo clear-all` | Clears all todos | Error |
| 10.14 | `/tca` | Shortcut for `/todo clear-all` | Not recognized |
| 10.15 | `/todo add` | Usage error (no description) | Crash or silence |
| 10.16 | `/todo edit 3 --priority low` | Edits todo priority | Error |

## 11. Save/Load error paths (CommandResult::Error eliminated)

Previously: errors used `CommandResult::Error(String)`.
Now: errors use `eprintln!` directly in handlers (Try/Ok pattern).

| #  | Command | Expected | Fail if |
|----|---------|----------|---------|
| 11.1 | `/save` (anonymous session) | "Cannot save anonymous session" error | Crash or silent success |
| 11.2 | `/load nonexistent_session` | "Failed to load session" error | Crash |
| 11.3 | `/export xyz` | Error: invalid format (only md/json) | Crash or silent |
| 11.4 | `/model nonexistent_model` | Model switch error or warning | Crash |

## 12. General regression (commands affected indirectly)

| #  | Command | Expected | Fail if |
|----|---------|----------|---------|
| 12.1 | `/help` | Shows full help message | Crash or incomplete |
| 12.2 | `/info` | Shows session info | Crash |
| 12.3 | `/quit` | Exits REPL cleanly | Hang or data loss |
| 12.4 | `/q` | Shortcut for `/quit` | Not recognized |
| 12.5 | `/system You are helpful` | System prompt updated | No feedback or crash |

---

**Total: ~95 manual tests**

**Priority order:**
1. Section 1-2 (session commands — core of Phase 3.7)
2. Section 3-4 (toggles and actions — CommandResult elimination)
3. Section 5-6 (export and skill — moved logic)
4. Section 7-10 (subcommand parsers — extracted parsers)
5. Section 11-12 (error paths and regression)