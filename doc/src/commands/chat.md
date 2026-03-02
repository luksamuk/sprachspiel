# chat

Interactive chat with conversation history.

## Synopsis

```bash
ask chat [OPTIONS]
```

## Description

Start an interactive chat session with an Ollama model. Conversations are automatically saved per project (identified by git remote URL or folder name), allowing you to resume conversations where you left off.

## Options

| Option | Description |
|--------|-------------|
| `-a, --anonymous` | Start an anonymous session (no history persistence) |
| `-l, --load <SESSION>` | Load a named session |
| `-m, --model <MODEL>` | Model preset to use (overrides config) |
| `-t, --think` | Enable think mode for models that support it |
| `--tools` | Force enable tools even if model doesn't advertise support |
| `--ignore-agents` | Ignore AGENTS.md file if present |

## Interactive Commands

Once inside the chat, these commands are available:

| Command | Description |
|---------|-------------|
| `/quit`, `/exit`, `/q` | Exit the chat session |
| `/clear`, `/c` | Clear conversation history |
| `/help`, `/h`, `/?` | Show available commands |
| `/model <name>`, `/m <name>` | Switch to a different model |
| `/system <prompt>`, `/s <prompt>` | Change the system prompt |
| `/think`, `/t` | Toggle think mode on/off |
| `/tools` | Toggle tools on/off |
| `/compact` | Compact conversation history (summarize) |
| `/retry`, `/r` | Regenerate the last response |
| `/undo`, `/u` | Undo last message (remove response, show last input) |
| `/save [name]` | Save current session (optionally named) |
| `/load <name>`, `/l <name>` | Load a saved session |
| `/export <format> [file]`, `/e <format>` | Export conversation (md, json) |
| `/list`, `/ls` | List saved sessions for this project |
| `/info`, `/i` | Show current session information |
| `/context`, `/ctx` | Show context metrics and token usage |

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

Sessions are stored in `~/.local/share/ask-ai/conversations/` organized by project:

- Projects are identified by git remote URL (e.g., `github.com/user/repo`)
- If not in a git repo, the folder name is used as fallback
- Anonymous sessions (`--anonymous`) are not persisted

### Storage Location

```
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