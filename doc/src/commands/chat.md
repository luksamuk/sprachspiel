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
| `/clear`, `/c` | Clear conversation history (keeps session) |
| `/forget` | Delete session completely from database |
| `/help`, `/h`, `/?` | Show available commands |
| `/save [name]` | Save current session (optionally named) |
| `/load <name>`, `/l <name>` | Load a saved session |
| `/list`, `/ls` | List saved sessions for this project |
| `/info`, `/i` | Show current session information |
| `/restore <file>` | Restore session from JSON backup |
| `/export <format> [file]` | Export conversation (md, json) |

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