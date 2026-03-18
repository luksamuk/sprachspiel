# chat

Interactive chat with conversation history.

## Synopsis

```bash
ask chat [OPTIONS]
```

## Description

Start an interactive chat session with an Ollama model. Conversations are automatically saved per project (identified by git remote URL or folder name), allowing you to resume conversations where you left off.

## Key Features

- **Persistent History**: Conversations saved per project
- **Semantic Search**: Find past discussions with `/search`
- **Context Awareness**: Automatic retrieval of relevant messages
- **Model Switching**: Change models mid-conversation with `/model`
- **Tool Integration**: Automatic tool use for capable models

## Options

| Option | Description |
|--------|-------------|
| `-a, --anonymous` | Start an anonymous session (no history persistence) |
| `-l, --load <SESSION>` | Load a named session |
| `-m, --model <MODEL>` | Model preset to use (overrides config) |
| `-t, --think` | Enable think mode for models that support it |
| `--tools` | Force enable tools even if model doesn't advertise support |
| `--ignore-agents` | Ignore AGENTS.md file if present |
| `--soulless` | Skip SOUL.md personality (use neutral personality) |

## Interactive Commands

Once inside the chat, these commands are available:

### Session Management

| Command | Description |
|---------|-------------|
| `/quit`, `/exit`, `/q` | Exit the chat session |
| `/new`, `/n` | Start a new conversation (previous messages remain searchable) |
| `/forget` | Delete conversation completely and start fresh |
| `/help`, `/h`, `/?` | Show available commands |
| `/save [name]` | Save current session (optionally named) |
| `/load <name>`, `/l <name>` | Load a saved session |
| `/list`, `/ls` | List saved sessions for this project |
| `/info`, `/i` | Show current session information |
| `/restore <file>` | Restore session from JSON backup |
| `/export <format> [file]` | Export conversation (md, json) |

#### Session Command Group

The `/session` command provides an alternative syntax for session management:

| Command | Description |
|---------|-------------|
| `/session new` | Same as `/new` - start a new conversation |
| `/session load <name>` | Same as `/load` - load a saved session |
| `/session list` | Same as `/list` - list saved sessions |
| `/session save [name]` | Same as `/save` - save current session |
| `/session forget` | Same as `/forget` - delete and start fresh |

### Model & Mode

| Command | Description |
|---------|-------------|
| `/model <name>`, `/m <name>` | Switch to a different model |
| `/think`, `/t` | Toggle think mode on/off |
| `/tools` | Toggle tools on/off |
| `/tools-output <level>` | Set tool output verbosity: compact, full, hidden |

### Conversation

| Command | Description |
|---------|-------------|
| `/system <prompt>`, `/s <prompt>` | Change the system prompt |
| `/compact` | Compact conversation history (summarize old messages) |
| `/retry`, `/r` | Regenerate the last response |
| `/undo`, `/u` | Undo last message (remove response, show last input) |
| `/export <format> [file]` | Export conversation (md, json) |

### Context & Search

| Command | Description |
|---------|-------------|
| `/context`, `/ctx` | Show context metrics and token usage |
| `/search <query>`, `/find <query>`, `/f <query>` | Search conversation history (semantic search) |

### Facts & Memory

| Command | Description |
|---------|-------------|
| `/fact add <text> [--global]`, `/fa` | Add a fact (project scope by default) |
| `/fact list [--global]`, `/fl` | List stored facts |
| `/fact remove <id>`, `/fr` | Remove a fact by ID |
| `/fact search <query>`, `/fs` | Search stored facts |
| `/fact prune`, `/fp` | Prune old facts using decay |

Subcommand shortcuts: `/fact a` (add), `/fact l` (list), `/fact r` (remove), `/fact s` (search), `/fact p` (prune)

### Notes

| Command | Description |
|---------|-------------|
| `/note add <content> [--title <title>] [--global]`, `/na` | Add a note (project scope by default) |
| `/note list [--global] [page]`, `/nl` | List stored notes (8 per page) |
| `/note show <id>`, `/ns` | Show a note by ID |
| `/note edit <id> [--title <title>] [--content <content>]` | Edit a note |
| `/note delete <id>`, `/nd` | Delete a note by ID |
| `/note search <query> [--global] [limit]` | Search stored notes |

Subcommand shortcuts: `/no` (note), `/na` (add), `/nl` (list), `/ns` (show), `/nd` (delete)

Notes support project-level (default) and global scope. Global notes are visible across all projects on the same machine.

**Pagination:** `/note list` shows 8 notes per page. Use `/note list 2` to see page 2, `/note list 3` for page 3, etc.

## /context - Context Metrics

Show token usage and context utilization for the current session:

```
> /context
Context Information:
  Model:          llama3.1:8b (32K context)

  Token Breakdown:
    System prompt:    ~890 tokens
    Tool definitions: ~450 tokens (23 tools)
    Conversation:     ~1,250 tokens (15 messages)
    ────────────────────────────────────────────
    Total used:       ~2,590 tokens
    Available:        ~29,506 tokens
    Utilization:      8.1%

  Session:
    Total:           15 messages
```

### Token Estimation

Token counts are estimates based on:
- **Text**: ~0.75 words per token (GPT-style)
- **Message overhead**: ~4 tokens per message (role markers, formatting)
- **Code**: ~0.5 tokens per character (higher density)

Actual token usage may vary depending on the model's tokenizer.

## /search - Semantic Search

Search conversation history using hybrid search (keyword + semantic):

```bash
/search authentication           # Basic search
/search "error handling" 5      # Limit to 5 results
/f database design               # Alias
```

### How It Works

The search combines three techniques:

1. **BM25 (Keyword Search)** - Full-text search via FTS5
2. **Semantic Search** - Vector similarity using `nomic-embed-text-v2-moe`
3. **Reciprocal Rank Fusion (RRF)** - Combines results with weights 0.4/0.6

### Output Format

```
🔍 🔗 **user** (score: 0.0423)
   How do I control the LED strip?
   _default_ _2026-03-02 14:30_

🧠 🔗 **assistant** (score: 0.0387)
   You can use the LED tools to control...
   _default_ _2026-03-02 14:31_
```

| Icon | Meaning |
|------|---------|
| 🔍 | Keyword match (BM25 only) |
| 🧠 | Semantic match (vector only) |
| 🔗 | Hybrid match (both keyword and semantic) |

### Chunking

Messages longer than 1024 characters are automatically split into overlapping chunks:

- **Chunk size**: 1024 characters
- **Overlap**: 200 characters (20%)
- **Why overlap**: Ensures search terms split across boundaries are still found

Example: A 3000-character message creates 4 overlapping chunks, ensuring phrases like "Wittgenstein's philosophical investigations" match even if split across chunks.

### Prerequisites

1. **Ollama running** with embedding model:
   ```bash
   ollama pull nomic-embed-text-v2-moe
   ```

2. **Messages indexed** - Messages are automatically indexed when saved to SQLite.

## /fact - Factual Memory

The factual memory system allows the AI to remember preferences and facts across sessions.

### Adding Facts

Add facts that the AI should remember:

```bash
/fact add I prefer concise responses
/fact add Project uses SQLite for storage --global
/fa The API rate limit is 100 req/min    # Shortcut
```

**Options:**
- `--global` - Store as global fact (applies to all projects)
- Without flag - Store as project-specific fact

**Limits:**
- Maximum content size: 500 characters
- Facts exceeding this limit will be rejected with an error

**Auto-classification:** Facts are automatically classified as:
- `preference` - User preferences ("I prefer...", "I like...")
- `fact` - Objective information ("The API is...", "Database uses...")

**Conflict Resolution:** When adding a similar fact:
- **Duplicate** (very similar, no contradiction): Skipped
- **Contradiction** ("I like X" vs "I hate X"): Replaces old fact

### Listing Facts

View all stored facts:

```bash
/fact list           # Project facts
/fact list --global  # Global facts
/fl                  # Shortcut
```

**Output:**
```
Facts (project):

  Preferences:
    #1 I prefer short explanations (5d)
    #2 I like code examples (3d)

  Facts:
    #3 Project uses SQLite (7d)
    #5 API endpoint is /v1/users (2d)

  Total: 4 fact(s)
```

### Searching Facts

Search stored facts using keyword search:

```bash
/fact search database
/fact search API --global 5    # Global scope, limit 5
/fs prefer                      # Shortcut
```

### Removing Facts

Remove a fact by its ID:

```bash
/fact remove 3
/fr 5                          # Shortcut
```

### Pruning Old Facts

Facts automatically decay based on age and importance:

- **Preferences**: 180-day half-life
- **Facts**: 30-day half-life
- **High-importance preferences**: Never pruned

Run manual cleanup:

```bash
/fact prune
/fp                            # Shortcut
```

### How It Works

1. **Storage**: Facts stored in SQLite with FTS5 full-text search
2. **Prompt Injection**: Facts injected into system prompt (max 2200 chars)
3. **Decay**: Ebbinghaus forgetting curve with access reinforcement
4. **Conflict Detection**: Similar facts detected via FTS5, contradictions resolved

### Fact Scope

| Scope | Description | Use Case |
|-------|-------------|----------|
| `project` | Current project only | "API uses port 8080", "Database is SQLite" |
| `global` | All projects | "I prefer Portuguese", "I like concise responses" |

### LLM Integration

The LLM can also store facts autonomously using the `fact_add`, `fact_search`, and `fact_remove` tools. These tools are available to models with tool support.

### Anonymous Mode

Facts are **disabled in anonymous mode** (`--anonymous` flag). The `/fact` commands will show an error:

```
Error: Cannot add facts in anonymous mode.
```

Facts are only available in persistent sessions where they can be stored across conversations. In anonymous mode, no data is persisted, so fact storage is unavailable.

## Prompt Indicators

The prompt shows active modes:

- `lfm>` - Normal mode
- `lfm[t]>` - Think mode enabled
- `lfm[T]>` - Tools enabled
- `lfm[t][T]>` - Both think and tools enabled

## Tab Completion

Press Tab to complete:
- Commands: Type `/mod` + Tab → `/model`
- Model names: Type `/model l` + Tab → shows models starting with "l"

## Session Storage

Sessions are stored in a SQLite database at `~/.local/share/ask-ai/embeddings.db`:

- **Primary storage**: SQLite database with full-text search (FTS5) and vector embeddings
- **Automatic persistence**: Every message is saved immediately
- **Project organization**: Sessions grouped by git remote URL or folder name
- **Anonymous sessions** (`--anonymous`): Not persisted, in-memory only

### Storage Location

```
~/.local/share/ask-ai/
├── embeddings.db              # SQLite database (conversations + embeddings)
├── chat_history.txt           # Readline history
└── archived/                  # Archived JSON sessions (after migration)
    └── github.com/
        └── user/
            └── repo/
                └── session.json
```

### Project Identification

1. **Git repository with remote**: Uses normalized git remote URL
   - `git@github.com:user/repo.git` → `github.com/user/repo`
   - `https://github.com/user/repo.git` → `github.com/user/repo`

2. **Git repository without remote**: Uses folder name

3. **Not a git repository**: Uses current folder name

### Database Tables

The SQLite database contains:

| Table | Purpose |
|-------|---------|
| `conversations` | Session metadata (model, name, system_prompt, etc.) |
| `messages` | Conversation history with embeddings |
| `message_chunks` | Split message content for semantic search |
| `session_todos` | Todo list state per session |

### /restore - Disaster Recovery

If you have JSON backup files from older versions, restore them with:

```bash
/restore path/to/session.json
```

This imports the JSON session into SQLite and deletes the original file.

### /export - Backup Conversations

Export sessions for backup or transfer:

```bash
/export json              # Export current session to JSON
/export md conversation.md # Export as Markdown
```

**Note**: JSON export is for backup purposes. Sessions are stored in SQLite and don't need manual export.
~/.local/share/ask-ai/
├── chat_history.txt           # Readline history
└── conversations/
    └── github.com/
        └── user/
            └── repo/
                ├── default.json        # Default session for project
                ├── bugfix-auth.json    # Named sessions
                └── feature-x.json
```

### Project Identification

1. **Git repository with remote**: Uses normalized git remote URL
   - `git@github.com:user/repo.git` → `github.com/user/repo`
   - `https://github.com/user/repo.git` → `github.com/user/repo`

2. **Git repository without remote**: Uses folder name

3. **Not a git repository**: Uses current folder name

### Session File Format

Sessions are stored as JSON with the following structure:

```json
{
  "id": "default",
  "name": null,
  "project_id": "github.com/user/repo",
  "model": "lfm",
  "system_prompt": null,
  "messages": [
    {"role": "user", "content": "...", "timestamp": "..."},
    {"role": "assistant", "content": "...", "timestamp": "..."}
  ],
  "compacted_summary": null,
  "messages_sent_to_llm": 0,
  "created_at": "2026-02-19T10:00:00Z",
  "updated_at": "2026-02-19T10:30:00Z",
  "anonymous": false,
  "think": false,
  "tools": true
}
```

### Conversation Compaction

Use `/compact` to summarize old messages and free up context:

1. The LLM generates a summary of the conversation
2. The summary is stored in `compacted_summary`
3. Old messages are kept in `messages` (for logs/export) but not sent to LLM
4. `messages_sent_to_llm` tracks how many messages were compacted

After compaction, only the summary + new messages are sent to the LLM.

## Examples

Start a new chat session:
```bash
ask chat
```

Start with a specific model:
```bash
ask chat -m lfm
```

Start an anonymous session (temporary):
```bash
ask chat --anonymous
```

Load a previously saved session:
```bash
ask chat --load my-session
```

## Inside Chat

Once inside the chat, type your message and press Enter to send. The model will respond with the conversation context maintained.

```
lfm> What is Rust?
[Response about Rust]

lfm> What about its memory safety features?
[Response that includes context from previous message]

lfm> /model llama3.2
Model switched to: llama3.2

llama3.2> /quit
Goodbye!
```

## Exporting Conversations

Export conversation as Markdown:
```bash
/export md
```

Export to a file:
```bash
/export md conversation.md
```

Export as JSON (full session data):
```bash
/export json session.json
```

## See Also

- [query](./query.md) - Single-shot queries
- [Models](../models.md) - Available model presets
- [Configuration](../configuration.md) - Per-subcommand model settings