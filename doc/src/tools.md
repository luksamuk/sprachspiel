# Available Tools

Ask-AI provides tools that enhance queries with real-time data from external sources. Tools are automatically enabled for capable models.

## Tool Overview

| Category | Count | Source | Status | Default |
|----------|-------|--------|--------|---------|
| Document Import | 1 | Local files | ✅ Working | ✅ Enabled |
| Pokémon | 9 | PokéAPI | ✅ Working | ❌ Opt-in |
| Weather | 3 | Open-Meteo | ✅ Working | ✅ Enabled |
| Calculator | 1 | ollama-rs built-in | ✅ Working | ✅ Enabled |
| Web Search | 2 | Google via Serper | ✅ Working | ✅ Enabled* |
| Web Scraper | 1 | html2md | ✅ Working | ❌ Disabled |
| Finance | 1 | Google Finance | ✅ Working | ❌ Disabled |
| System | 2 | Local system | ✅ Working | ✅ Enabled |
| File Operations | 8 | Local filesystem | ✅ Working | ✅ Enabled |
| Factual Memory | 3 | SQLite + FTS5 | ✅ Working | ✅ Enabled |
| Notes | 3 | SQLite + FTS5 | ✅ Working | ✅ Enabled |
| Memory Retrieval | 1 | SQLite + FTS5 | ✅ Working | ✅ Enabled |
| Skills | 2 | Local skill files | ✅ Working | ✅ Enabled |
| Todo | 5 | Session state | ✅ Working | ✅ Enabled |
| LED Control | 5 | Raspberry Pi Pico W | ✅ Working | ❌ Disabled** |

\* **Web search requires SERPER_API_KEY environment variable.** If not set, DuckDuckGo is used as fallback (may be blocked by CAPTCHA).

\** **LED tools require configuration.** See [LED Control Tools](#led-control-tools-5) section.

## Compilation Features

Tools are organized into feature flags that can be enabled or disabled at compile time. This allows you to build a leaner binary with only the tools you need.

### Default Features

The default build includes:
- `weather-tools` - Weather lookup tools
- `calc-tools` - Mathematical calculator
- `serper-tools` - Google Search via Serper (requires API key)
- `system-tools` - Date/time and project context
- `file-tools` - File system operations
- `skills-tools` - AI behavior skills (skill_list, skill_view)
- `todo-tools` - Session todo list management
- `document-tools` - Document import

### Available Features

| Feature | Description | Tools Included | Default |
|---------|-------------|----------------|---------|
| `pokemon-tools` | Pokémon data from PokéAPI | fetch_pokemon*, fetch_ability_details, fetch_type_effectiveness, fetch_pokemon_by_type, fetch_move_details | ❌ No |
| `weather-tools` | Weather data from Open-Meteo | get_weather, get_current_weather, get_weather_forecast | ✅ Yes |
| `calc-tools` | Mathematical calculations | calculate | ✅ Yes |
| `serper-tools` | Google Search via Serper API | web_search, web_search_news | ✅ Yes |
| `search-tools` | DuckDuckGo + Web scraper | web_search, web_search_news, web_scrape | ❌ No |
| `finance-tools` | Stock quotes | get_stock_quote | ❌ No |
| `system-tools` | System context | get_current_datetime, get_project_context | ✅ Yes |
| `file-tools` | Local file operations | read_file, read_file_segment, count_lines, list_directory, search_files, write_file, edit_file, append_file | ✅ Yes |
| `skills-tools` | AI behavior patterns | skill_list, skill_view | ✅ Yes |
| `document-tools` | Document import | import_document | ✅ Yes |
| `todo-tools` | Session todo list | todo_add, todo_update, todo_list, todo_clear_done, todo_clear_all | ✅ Yes |
| `led-tools` | NeoPixel LED control | led_get_status, led_set_power, led_set_program, led_set_brightness, led_set_color | ❌ No |
| `all-tools` | Enable all tool categories | All of the above | - |

### Web Search Configuration

**Option 1: Serper (Recommended)** - Google Search results
```bash
# Set environment variable
export SERPER_API_KEY="your-api-key-here"

# Build with serper-tools (default)
cargo build --release
```

Get your free API key at [serper.dev](https://serper.dev) (2,500 free searches/month).

**Option 2: DuckDuckGo** - Free but may be blocked
```bash
# Build with search-tools instead of serper-tools
cargo build --release --no-default-features --features "weather-tools,calc-tools,search-tools,file-tools,system-tools,skills-tools,todo-tools,document-tools"
```

### Building with Custom Features

**Default build (includes Serper tools):**
```bash
cargo build --release
```

**Enable finance tools:**
```bash
cargo build --release --features finance-tools
```

**Enable all tools:**
```bash
cargo build --release --features all-tools
```

**Minimal build (only file tools):**
```bash
cargo build --release --no-default-features --features file-tools
```

**Enable Pokémon tools (opt-in):**
```bash
cargo build --release --features pokemon-tools
```

**Enable all optional tools:**
```bash
cargo build --release --features all-tools
```

### Runtime Filtering

Even when tools are compiled in, you can disable specific tools at runtime using the [blacklist configuration](./configuration.md#tool-configuration):

```toml
# ~/.config/ask-ai/config.toml
[tools]
blacklist = ["fetch_pokemon", "web_search"]
```

When a tool is blacklisted, it won't be registered with the coordinator AND won't appear in the system prompt's tool descriptions. The model won't even know the tool exists.

## Pokémon Tools (9)

**Note:** Pokémon tools are **opt-in** and not included in the default build. Enable with `--features pokemon-tools`.

Powered by [PokéAPI](https://pokeapi.co/).

### fetch_pokemon

Get comprehensive Pokémon data including stats, abilities, moves, and evolution chain.

```
Function: fetch_pokemon
Args: name (string)
Example: fetch_pokemon(name: "pikachu")
```

### fetch_pokemon_basic

Get basic Pokémon information (types, height, weight, abilities).

```
Function: fetch_pokemon_basic
Args: name (string)
Example: fetch_pokemon_basic(name: "charizard")
```

### fetch_pokemon_stats

Get base stats (HP, Attack, Defense, etc.).

```
Function: fetch_pokemon_stats
Args: name (string)
Example: fetch_pokemon_stats(name: "mewtwo")
```

### fetch_pokemon_moves

Get learnable moves with optional limit.

```
Function: fetch_pokemon_moves
Args: name (string), limit (optional string)
Example: fetch_pokemon_moves(name: "pikachu", limit: "10")
```

### fetch_pokemon_evolution

Get evolution chain information.

```
Function: fetch_pokemon_evolution
Args: name (string)
Example: fetch_pokemon_evolution(name: "eevee")
```

### fetch_ability_details

Get ability descriptions and which Pokémon have it.

```
Function: fetch_ability_details
Args: ability (string)
Example: fetch_ability_details(ability: "lightning-rod")
```

### fetch_type_effectiveness

Get type weaknesses, resistances, and immunities.

```
Function: fetch_type_effectiveness
Args: type_name (string)
Example: fetch_type_effectiveness(type_name: "electric")
```

### fetch_pokemon_by_type

List all Pokémon of a specific type.

```
Function: fetch_pokemon_by_type
Args: type_name (string), limit (optional string, default "20", max "100")
Example: fetch_pokemon_by_type(type_name: "water", limit: "50")
```

### fetch_move_details

Get move information (power, accuracy, type, effect).

```
Function: fetch_move_details
Args: move (string)
Example: fetch_move_details(move: "thunderbolt")
```

## Weather Tools (3)

Powered by [Open-Meteo](https://open-meteo.com/) (free, no API key required).

### get_weather

Get current weather conditions and 3-day forecast.

```
Function: get_weather
Args: city (string), country (optional string)
Example: get_weather(city: "Tokyo", country: "Japan")
```

### get_current_weather

Get current conditions only (simpler response).

```
Function: get_current_weather
Args: city (string), country (optional string)
Example: get_current_weather(city: "London")
```

### get_weather_forecast

Get extended 7-day forecast.

```
Function: get_weather_forecast
Args: location (string), days (string, optional)
Example: get_weather_forecast(location: "Paris", days: "7")
```

**Note:** `days` accepts strings like "3", "5", "7".

## Calculator Tool (1)

Built-in mathematical expression evaluator from ollama-rs.

### calculate

Evaluate mathematical expressions.

```
Function: calculate
Args: expression (string)
Example: calculate(expression: "15% of 850")
Example: calculate(expression: "sqrt(144) + 2**8")
Example: calculate(expression: "(100 + 50) * 0.2")
```

**Supported operations:**
- Basic arithmetic: `+`, `-`, `*`, `/`
- Exponents: `**` or `^`
- Percentages: `15% of 850`
- Functions: `sqrt()`, `sin()`, `cos()`, etc.
- Parentheses for grouping

**Example queries:**
- "What is 15% of 850?"
- "Calculate sqrt(144)"
- "What's 2 to the power of 8?"

## Web Search Tools (3)

Powered by DuckDuckGo via ollama-rs built-in DDGSearcher. Works without CAPTCHA issues.

### web_search

Search the web and get results with titles, URLs, and snippets.

```
Function: web_search
Args: query (string), num_results (optional string, default "5", max "10")
Example: web_search(query: "Rust programming language", num_results: "5")
```

**Note:** `num_results` accepts strings like "3", "5", "10".

### web_search_news

Search specifically for news articles.

```
Function: web_search_news
Args: query (string), num_results (optional string, default "3", max "10")
Example: web_search_news(query: "technology", num_results: "5")
```

### web_scrape

Extract content from a webpage URL as Markdown.

```
Function: web_scrape
Args: url (string)
Example: web_scrape(url: "https://www.rust-lang.org")
```

**Use cases:**
- Get detailed content from a URL found via web_search
- Extract article content
- Read documentation pages

**Example workflow:**
1. Use `web_search` to find relevant URLs
2. Use `web_scrape` to get full content from promising results

## Finance Tool (1)

Powered by Google Finance web scraping.

### get_stock_quote

Get stock quote information from Google Finance.

```
Function: get_stock_quote
Args: exchange (string), ticker (string)
Example: get_stock_quote(exchange: "NASDAQ", ticker: "AAPL")
Example: get_stock_quote(exchange: "BVMF", ticker: "PETR4")
```

**Common exchange codes:**
- `NASDAQ` - NASDAQ (US)
- `NYSE` - New York Stock Exchange (US)
- `BVMF` - B3 (Brazil)
- `LON` - London Stock Exchange (UK)
- `TPE` - Tokyo Stock Exchange (Japan)

**Note:** Not enabled by default. Build with `--features finance-tools` to enable.

## System Tools (2)

Tools for getting system and project context information.

### get_current_datetime

Get current date, time, timezone, and related information.

```
Function: get_current_datetime
Args: (none)
Example: get_current_datetime()
```

**Output includes:**
- Date (day of week, month, day, year)
- Time with timezone offset
- Timezone name
- Day of week
- Week of year
- ISO 8601 format
- Unix timestamp

**Example queries:**
- "What time is it?"
- "What day of the week is today?"
- "What's the current timezone?"

### get_project_context

Get information about the current project (languages, stack, git).

```
Function: get_project_context
Args: (none)
Example: get_project_context()
```

**Output includes:**
- Current directory
- Git branch and remote
- Languages detected (file counts and percentages)
- Stack detection (Rust, Node.js, Python, etc.)
- Key files (README, Cargo.toml, package.json, etc.)

**Important:** This tool is **blacklisted by default** because it returns extensive information. Use only when you need detailed project analysis.

**Relationship with AGENTS.md:**
- AGENTS.md contains **guidelines and conventions** (HOW to work on the project)
- get_project_context provides **current state** (WHAT the project is)
- Always follow AGENTS.md for conventions
- Use get_project_context for current state information

**Example queries:**
- "Analyze this project's structure"
- "What languages are used in this project?"
- "What's the current git branch?"

**Security:**
- No shell commands executed
- No environment variables exposed
- Ignores `.env` files and secrets
- Scoped to max 3 directory levels

## Skills Tools (2)

Tools for discovering and loading AI behavior skills. Skills are Markdown files that define instructions for specific tasks (PDF processing, OCR, code analysis, etc.).

These tools are **enabled by default** and provide progressive disclosure - the system prompt contains only skill names and descriptions, while full content is loaded on-demand when needed.

### skill_list

List all available skills with brief descriptions.

```
Function: skill_list
Args: (none)
Example: skill_list()
```

**Output includes:**
- Skill name
- Source (builtin, user, project)
- Brief description

**Example output:**
```
Available skills (4):

- **document-processing** (builtin): Extract content from PDF and ePub files
- **ocr-images** (builtin): Process images with OCR
- **code-analysis** (builtin): Analyze code structure
- **web-scraping** (builtin): Scrape web content

Use skill_view(name="skill-name") to load a specific skill.
```

**Use case:** Discover available skills before loading one that matches your task.

### skill_view

Load and view the full content of a specific skill.

```
Function: skill_view
Args: name (string, required): Skill name (e.g., "document-processing")
Example: skill_view(name="document-processing")
```

**Returns:**
- Skill name and source
- Description
- Full instructions and guidelines
- Examples and patterns

**Example output:**
```
# Skill: document-processing (builtin)

**Description:** Extract and process content from PDF and ePub files

---
[Full skill content with instructions...]
```

**Error handling:**
- Returns error if skill not found (use `skill_list` to see available skills)
- Returns error if skill name is invalid
- Returns error if skill content failed validation (contains injection patterns)

**Skill Locations:**
| Source | Location |
|--------|----------|
| builtin | Embedded in binary (always available) |
| user | `~/.config/ask-ai/skills/<name>/SKILL.md` |
| project | `.ask-ai/skills/<name>/SKILL.md` |

**Priority:** project > user > builtin (project-level skills override user and builtin).

**Example workflow:**
```
User: Extract text from document.pdf
LLM: [Calls skill_list() to see available skills]
     [Calls skill_view(name="document-processing") to load instructions]
     [Follows skill instructions to process PDF]
```

## Factual Memory Tools (3)

Tools for storing and retrieving user/project facts across sessions.

These tools are **always enabled** (no feature flag needed) and allow the LLM to remember preferences and facts.

### fact_add

Store a fact or preference about the user or project.

```
Function: fact_add
Args:
  - content (string, required): The fact to store (max 500 chars)
  - category (string, optional): "preference" or "fact" (auto-detected if omitted)
  - scope (string, optional): "global" or "project" (default: "global")
Example: fact_add(content="I prefer concise responses")
Example: fact_add(content="Project uses PostgreSQL", scope="project")
```

**Auto-classification:**
- "I prefer...", "I like...", "Prefiro...", "Gosto de..." → preference
- Other content → fact

**Conflict Resolution:**
- Duplicate (very similar content) → Skipped
- Contradiction ("I like X" vs "I hate X") → Replaces old fact

### fact_search

Search stored facts using keyword search (FTS5).

```
Function: fact_search
Args:
  - query (string, required): Search query
  - category (string, optional): "preference" or "fact"
  - scope (string, optional): "global" or "project"
  - limit (string, optional): Max results (default: 5, max: 20)
Example: fact_search(query="database")
Example: fact_search(query="prefer", category="preference")
```

### fact_remove

Remove a stored fact by its ID.

```
Function: fact_remove
Args:
  - id (string, required): Fact ID (format: "N" or "fact:N")
Example: fact_remove(id="42")
Example: fact_remove(id="fact:42")
```

**User Commands:**

Users can also manage facts via chat commands:

| Command | Shortcut | Description |
|---------|----------|-------------|
| `/fact add <text> [--global]` | `/fa` | Add fact |
| `/fact list [--global]` | `/fl` | List facts |
| `/fact search <query>` | `/fs` | Search facts |
| `/fact remove <id>` | `/fr` | Remove fact |
| `/fact prune` | `/fp` | Run decay manually |

**Decay:**
- Preferences: 180-day half-life
- Facts: 30-day half-life
- High-importance preferences: Never pruned
- Automatic decay on startup

## Notes Tools (3)

Tools for creating, editing, and deleting notes. Notes are stored in the same database as facts and can be retrieved via the `remember` tool.

These tools allow the LLM to maintain persistent notes across sessions — longer-form content that doesn't fit the fact model (max 500 chars). Notes support titles, content up to 10,000 characters, and are stored with project/global scope.

### note_add

Create a new note with optional title.

```
Function: note_add
Args:
  - content (string, required): Note content (max 10,000 characters)
  - title (string, optional): Note title (auto-generated from content if omitted)
Example: note_add(content="Architecture Decision: We chose PostgreSQL over MySQL for better JSON support.")
Example: note_add(content="Meeting notes from standup...", title="Standup 2024-01-15")
```

**Auto-title:** If title is omitted, the first line of content (up to 200 chars) is used as the title.

**Content validation:** Empty content is rejected. For short facts under 500 characters, use `fact_add` instead.

**Scope:** Notes are stored with the current project scope by default.

**User Commands:**

Users can also manage notes via chat commands:

| Command | Shortcut | Description |
|---------|----------|-------------|
| `/note add <text> [--global]` | `/na` | Add a note |
| `/note list [--global]` | `/nl` | List notes |
| `/note show <id>` | `/ns` | Show note content (accepts #N, note:N, or N) |
| `/note edit <id> [--title TITLE] [--content TEXT]` | `/ne` | Edit a note |
| `/note delete <id>` | `/nd` | Delete a note |

### note_edit

Edit an existing note's title and/or content. At least one of `title` or `content` must be provided.

```
Function: note_edit
Args:
  - id (string, required): Note ID (format: "N" or "note:N")
  - title (string, optional): New title for the note
  - content (string, optional): New content for the note
Example: note_edit(id="42", title="Revised Architecture Decision")
Example: note_edit(id="note:7", content="Updated content here...")
Example: note_edit(id="42", title="New Title", content="New content...")
```

**ID formats:** Accepts both `42` and `note:42` formats.

**Validation:**
- At least one of `title` or `content` must be provided
- Content cannot be empty (use `note_delete` to remove notes)
- Content is limited to 10,000 characters

### note_delete

Permanently delete a note by its ID.

```
Function: note_delete
Args:
  - id (string, required): Note ID (format: "N" or "note:N")
Example: note_delete(id="42")
Example: note_delete(id="note:42")
```

**Returns:** Confirmation with deleted note's title and content preview.

**Tip:** Use `remember(query="...")` first to find the note ID if you don't know it.

## Todo Tools (5)

Tools for managing a task list during a chat session. These tools help the LLM track and manage tasks.

### todo_add

Add a new task to the todo list.

```
Function: todo_add
Args:
  - description (string, required): Task description
Example: todo_add(description="Implement user authentication")
```

### todo_update

Update the status of an existing task.

```
Function: todo_update
Args:
  - id (string, required): Task ID (format: "N" or "todo:N")
  - status (string, required): "pending", "in_progress", or "done"
Example: todo_update(id="1", status="in_progress")
Example: todo_update(id="todo:3", status="done")
```

### todo_list

List all tasks with their current status.

```
Function: todo_list
Args: None
Example: todo_list()
```

Returns a formatted list showing task ID, description, and status.

### todo_clear_done

Remove all completed (done) tasks from the list.

```
Function: todo_clear_done
Args: None
Example: todo_clear_done()
```

### todo_clear_all

Clear all tasks from the list.

```
Function: todo_clear_all
Args: None
Example: todo_clear_all()
```

**User Commands:**

Users can also manage todos via chat commands:

| Command | Shortcut | Description |
|---------|----------|-------------|
| `/todo add <description>` | `/ta` | Add task |
| `/todo list` | `/tl` | List tasks |
| `/todo update <id> <status>` | `/tu` | Update task status |
| `/todo clear-done` | `/tcd` | Clear completed tasks |
| `/todo clear-all` | `/tca` | Clear all tasks |

**Behavior:**
- Todo state is persisted per session
- Tasks are stored in `session_todos` table
- Status values: pending, in_progress, done
- Task IDs are auto-incremented integers

## Memory Retrieval Tool (1)

Retrieve content from conversation history, notes, and imported documents by ID or search query. This tool provides explicit access to stored content, complementing the automatic retrieval that happens during chat.

### remember

Retrieve content from conversation history, notes, and imported documents.

```
Function: remember
Args:
  - id (string, optional): Specific item ID with prefix (e.g., "msg:42", "note:7", "doc:13")
  - chunk (string, optional): Chunk index for large documents (0-based, e.g., "0", "5", "15")
  - query (string, optional): Search query for semantic search
  - limit (string, optional): Max results (default: 5, max: 10)
Example: remember(id="msg:42")
Example: remember(id="note:7")
Example: remember(id="doc:13")
Example: remember(id="doc:13", chunk="5")
Example: remember(query="authentication")
Example: remember(query="database design", limit="10")
```

**Source Types:**
| Prefix | Content Type | Description |
|--------|---------------|-------------|
| `msg:N` | Conversation | Chat message from history |
| `note:N` | Note | User-created note |
| `doc:N` | Document | Imported document (TXT, MD, ORG, PDF, EPUB) |
| `web:N` | Web | Web-scraped content (future) |

**Chunk Retrieval for Large Documents:**

Large documents (>~5000 characters) are split into chunks for storage. When retrieving:

- **Without chunk parameter**: Returns preview (first 3 chunks) with navigation hint
- **With chunk parameter**: Returns specific chunk content without truncation

```
# Large document preview
remember(id="doc:5")
→ **Document 5**: Manual (45000 words)
   ⚠️ Large document. Showing chunks 1-3 of 87.
   Use remember(id="doc:5", chunk="N") to read specific chunks.

# Specific chunk
remember(id="doc:5", chunk="15")
→ **Document 5** — Chunk 16/87
   Position: characters 15000-16000
   [Full chunk content without truncation]
```

**Use Cases:**
- **By ID**: Get full content when you only have an excerpt from search results
- **By ID + Chunk**: Navigate large documents section by section
- **By Query**: Find topics not in current context window
- **Hybrid Search**: Combines BM25 (keyword) and semantic (vector) search with RRF fusion

**Parameter Validation:**

The tool validates parameter combinations to prevent misuse:

| Error | Cause | Fix |
|-------|-------|-----|
| "Cannot use both 'id' and 'query'" | Both parameters specified | Use one or the other |
| "'limit' only valid with 'query'" | limit used with id | Use limit for searches only |
| "'chunk' only valid for documents" | chunk used with msg: or note: | Use chunk with doc: only |

**Valid Parameter Combinations:**

| Call | Valid? | Notes |
|------|--------|-------|
| `remember(id="doc:13")` | ✅ | Retrieves document (or preview) |
| `remember(id="doc:13", chunk="5")` | ✅ | Retrieves chunk 5 |
| `remember(id="msg:42")` | ✅ | Retrieves message |
| `remember(id="msg:42", chunk="5")` | ❌ | Error: chunk only for docs |
| `remember(query="auth", limit="10")` | ✅ | Searches, returns up to 10 results |
| `remember(id="note:7", limit="10")` | ❌ | Error: limit only for queries |
| `remember(id="doc:13", query="auth")` | ❌ | Error: use one or the other |

**Output Format (Document with chunks):**
```
📄 Document #13 — Chunk 15/87
Title: My Research Paper
Type: pdf | Scope: project
Position: characters 15000-16000
---
[Chunk content here...]
---

*Chunk 15 of 87. Use remember(id="doc:13", chunk="N") to navigate.*
```

**Note:** This tool is for explicit retrieval. The AI automatically retrieves relevant context on each message, so you only need to call this when you're looking for something specific that isn't in the current context.

## Document Import Tool (1)

Import documents for semantic search and retrieval. Documents are stored in the database and can be searched alongside conversations and notes.

### import_document

Import a document file (TXT, MD, ORG, PDF, EPUB) for semantic search.

```
Function: import_document
Args:
  - path (string, required): File path (absolute or relative, supports ~ expansion)
  - scope (string, optional): "project" (default) or "global"
Example: import_document(path: "docs/report.pdf")
Example: import_document(path: "~/notes.org", scope: "global")
```

**Supported File Types:**

| Type | Extension | Dependency |
|------|-----------|------------|
| Plain Text | `.txt` | Builtin |
| Markdown | `.md` | Builtin |
| Org Mode | `.org` | Builtin |
| PDF | `.pdf` | `skills-tools` + `pdftotext` |
| EPUB | `.epub` | `skills-tools` + `epub2txt` |

**File Size Limit:** 2.5 MB (2,500,000 bytes) maximum. Larger files are rejected with a helpful error.

**Path Expansion:** Paths starting with `~` are expanded to the home directory.

**Scope:**
- `project` (default): Document visible only in current project
- `global`: Document visible across all projects

**Import Modes:**

| Mode | Behavior |
|------|----------|
| Synchronous (default) | Wait for embedding generation to complete before returning |
| Async (`--nowait`) | Import file immediately, generate embeddings in background |

Use `--nowait` for batch imports when you don't need immediate searchability.

**Title Extraction:**

Documents automatically extract titles from:
1. Org-mode `#+TITLE:` directive
2. Markdown `# heading`
3. Org-mode `* heading`
4. Filename (fallback)

**PDF/EPUB Dependencies:**

PDF and EPUB files require:
1. `skills-tools` feature (enabled by default)
2. External tools installed:
   - PDF: `pdftotext` (poppler-utils package)
   - EPUB: `epub2txt` (AUR: `epub2txt-bin`) or `ebook-convert` (Calibre)

**Installation:**

| Distro | Command |
|--------|---------|
| Arch | `sudo pacman -S poppler-utils` + `yay -S epub2txt-bin` |
| Debian/Ubuntu | `sudo apt install poppler-utils` + download epub2txt from GitHub |
| Void | `sudo xbps-install -S poppler` + compile epub2txt |
| Fedora | `sudo dnf install poppler-utils` + compile epub2txt |

**Error Handling:**

| Situation | Error Message |
|-----------|---------------|
| File not found | `Error: File not found: 'report.pdf'. Please check the path and try again.` |
| File too large | `Error: File too large (3.0 MB = 3,000,000 bytes). Maximum is 2.5 MB (2,500,000 bytes).` |
| Unsupported type | `Error: Unsupported file type '.docx'. Supported: .txt, .md, .org, .pdf, .epub` |
| Missing dependency | `Error: Importing 'pdf' files requires the 'skills-tools' feature. Recompile with: cargo build --features skills-tools` |

**Integration with remember():**

Once imported, documents can be retrieved via the `remember()` tool:

```
# Retrieve by ID (accepts #N, doc:N, or N formats)
remember(id="doc:5")
remember(id="#5")
remember(id="5")

# Get specific chunk of large document
remember(id="doc:5", chunk="0")

# Semantic search across all documents
remember(query="authentication implementation")
```

**User Commands:**

| Command | Shortcut | Description |
|---------|----------|-------------|
| `/doc import <path> [--global] [--nowait]` | `/di` | Import a document |
| `/doc list [--global]` | `/dl` | List imported documents |
| `/doc show <id>` | `/ds` | Show document content (accepts #N, doc:N, or N) |
| `/doc delete <id>` | `/dd` | Delete a document (accepts #N, doc:N, or N) |

**ID Formats:**

Document IDs can be specified in three formats:
- `#N` - Hashtag format: `/doc show #1`
- `doc:N` - Prefixed format: `/doc show doc:1`
- `N` - Numeric format: `/doc show 1`

**Document Flow:**

```mermaid
flowchart LR
    subgraph Import["Import"]
        A[Document Path]
    end
    
    subgraph Extract["Extract"]
        B[Title & Text]
    end
    
    subgraph Process["Process"]
        C[Chunk & Embed<br/>512 tokens/chunk]
    end
    
    subgraph Store["Store"]
        D[(SQLite + FTS5<br/>+ Embeddings)]
    end
    
    subgraph Search["Retrieve"]
        E[Hybrid Search<br/>BM25 + Vector]
    end
    
    subgraph Query["Query"]
        F[remember<br/>query/text]
    end
    
    A --> B --> C --> D
    D --> E --> F
```

## File Operation Tools (8)

Perform local filesystem operations. **Sandboxed by default** to current working directory for security.

**Important:** For large files, use `count_lines` first to check size, then `read_file_segment` to read only what you need. Avoid polluting context with entire large files.

**Security:** All file operations check a blocklist for sensitive files (`.env`, `secrets`, SSH keys, certificates). These files cannot be read or written.

### read_file

Read contents of a file. Files larger than 1MB are rejected.

```
Function: read_file
Args: path (string), max_lines (string, optional), sandbox (string, optional)
Example: read_file(path: "README.md", max_lines: "50")
```

**Note:** All arguments are strings for robustness. `max_lines` accepts numbers like "50" or "100". `sandbox` accepts "true", "false", "1", "0".

**Features:**
- Limit output to N lines with `max_lines`
- Sandbox restricts access to current directory
- Supports relative and absolute paths
- Auto-resolves symlinks

### read_file_segment

Read a specific segment of a file (useful for large files).

```
Function: read_file_segment
Args: path (string), start_line (string, REQUIRED), num_lines (string, REQUIRED), sandbox (string, optional)
Example: read_file_segment(path: "src/main.rs", start_line: "100", num_lines: "50")
```

**Note:** Both `start_line` and `num_lines` are **required**. Line numbers start at 1.

**Features:**
- Line numbers are 1-based
- Output includes line numbers for easy reference
- Useful for reading specific functions or sections without loading entire large files
- Helps keep context window small

**Output format:**
```
Lines 100-150 of 500:
----------------------------------------
   100 | fn my_function() {
   101 |     println!("Hello");
```

### count_lines

Count lines in a file. **Use this before reading large files** to avoid polluting context.

```
Function: count_lines
Args: path (string), sandbox (string, optional)
Example: count_lines(path: "src/main.rs")
```

**Output:**
```
File: src/main.rs
Lines: 1500

Tip: Use read_file_segment(path, start_line, num_lines) to read specific sections and avoid polluting the context window.
```

### list_directory

List files and directories. Shows file types and sizes.

```
Function: list_directory
Args: path (string), recursive (string, optional), sandbox (string, optional)
Example: list_directory(path: "src", recursive: "true")
```

**Note:** `recursive` accepts "true", "false", "1", "0".

**Features:**
- Non-recursive by default (current level only)
- Recursive mode with `recursive: true` (max depth 10)
- Shows file types: [file], [dir], [symlink]
- Displays file sizes in KB/MB (e.g., "1.5 MB", "42 KB")

### search_files

Search file contents with regex pattern.

```
Function: search_files
Args: pattern (string), path (string), file_pattern (string, optional), sandbox (string, optional)
Example: search_files(pattern: "TODO|FIXME", path: "src", file_pattern: "*.rs")
```

**Note:** `sandbox` accepts "true", "false", "1", "0".

**Features:**
- Regex pattern matching (full Rust regex syntax)
- File pattern filtering with glob syntax (`*.rs`, `*.txt`)
- Returns matching lines with file path and line number
- Limited to 100 results and 1MB files
- Searches files within 5 directory levels

### write_file

Write content to a file, creating or overwriting it.

```
Function: write_file
Args: path (string), content (string), overwrite (string, optional), sandbox (string, optional)
Example: write_file(path: "output.txt", content: "Hello, World!")
Example: write_file(path: "config.json", content: json_data, overwrite: "true")
```

**Note:** `overwrite` accepts "true", "false", "1", "0". Default is "false".

**Security:**
- **Blocked patterns ALWAYS enforced** - Cannot write to `.env`, `secrets`, SSH keys, certificates
- **Sandbox respected** - `sandbox=false` allows writing outside CWD, but still enforces blocked patterns
- **Size limit** - Maximum 5MB per write
- **Atomic writes** - Uses temp file + rename for corruption safety

**Behavior:**
- Returns error if file exists and `overwrite=false`
- Creates parent directories must exist
- Only writes valid UTF-8 text content
- Program's own config files (ask-ai config) are always blocked - user must edit manually

### edit_file

Surgically edit an existing file without rewriting entire content.

```
Function: edit_file
Args:
  - path (string): File to edit
  - operation (string): "replace", "insert", or "delete_lines"
  - search (string, optional): Text to find (for "replace")
  - replace (string, optional): Text to replace with (for "replace")
  - after_line (string, optional): Line number to insert after (for "insert")
  - content (string, optional): Content to insert (for "insert")
  - start_line (string, optional): First line to delete (for "delete_lines")
  - end_line (string, optional): Last line to delete (for "delete_lines")
  - create_backup (string, optional): Create .bak file first
  - sandbox (string, optional): Restrict to CWD (default: true)
```

**Security:**
- **Blocked patterns ALWAYS enforced** - Cannot edit `.env`, `secrets`, SSH keys, certificates
- **Sandbox respected** - `sandbox=false` allows editing outside CWD, but blocked patterns still enforced
- Program's own config files (`~/.config/ask-ai/`) are always blocked - user must edit manually for security

### append_file

Append content to the end of an existing file.

```
Function: append_file
Args:
  - path (string): File to append to
  - content (string): Content to append
  - create (string, optional): Create file if not exists (default: "false")
```

**Examples:**
```bash
# Append to existing log file
append_file(path: "log.txt", content: "2024-01-15: New entry\n")

# Create and append (if file doesn't exist)
append_file(path: "output.txt", content: "First line\n", create: "true")
```

**Security:**
- Always sandboxed and blocked patterns enforced
- Maximum total file size: 5MB
- Cannot append to `.env`, secrets, keys, etc.

### File Tool Security

File tools have multiple security layers:

#### Sandbox

```toml
# ~/.config/ask-ai/config.toml
[tools]
file_sandbox = true  # Only allow access to CWD and subdirectories
```

**Sandbox behavior:**
- ✅ Allowed: Files in current directory and subdirectories
- ❌ Blocked: Files outside working directory
- ❌ Blocked: System directories (`/etc`, `/usr`, etc.)
- ❌ Blocked: Symlinks pointing outside sandbox

**Important:** Write operations (`write_file`, `edit_file`, `append_file`) are **always sandboxed** regardless of this setting. Only read operations respect `file_sandbox = false`.

#### Blocked Patterns

Sensitive files are blocked for both reading and writing:

| Category | Patterns |
|----------|----------|
| Environment files | `.env`, `.env.local`, `.env.*` |
| Secrets/credentials | `*secret*`, `*credential*`, `secrets.json`, `credentials.json` |
| SSH keys | `id_rsa`, `id_dsa`, `id_ed25519`, `id_ecdsa`, `.ssh/` |
| Certificates | `*.pem`, `*.key` |
| GPG | `.gnupg/` |
| Cloud credentials | `service-account.json`, `*-credentials.json` |

Configuration:

```toml
# ~/.config/ask-ai/config.toml
[file-tools]
max_file_size = 5242880  # 5MB default
blocked_patterns = [".env.*", "*secret*", "*.pem"]
block_read = true   # Block reading sensitive files
block_list = false  # Allow listing (filenames visible)
# block_write is always true, not configurable
```

#### Security Summary

| Operation | Sandbox | Blocked Patterns |
|-----------|---------|------------------|
| `read_file` | Configurable | Yes (if `block_read=true`) |
| `read_file_segment` | Configurable | Yes (if `block_read=true`) |
| `count_lines` | Configurable | Yes (if `block_read=true`) |
| `list_directory` | Configurable | Filenames visible (not content) |
| `search_files` | Configurable | Yes (if `block_read=true`) |
| `write_file` | **Always enforced** | **Always enforced** |
| `edit_file` | **Always enforced** | **Always enforced** |
| `append_file` | **Always enforced** | **Always enforced** |

**Disabling read sandbox** (not recommended for writes):

```toml
[tools]
file_sandbox = false
```

**Warning:** Disabling sandbox for reads allows the AI to read any accessible file. Write sandbox cannot be disabled.

## Document Processing Tools

External CLI tools for PDF and ePub processing. These are called via `run_command` and require installation.

**Note:** These are not built-in tools. They must be installed separately and configured in `~/.config/ask-ai/tools.toml`.

### PDF Tools (poppler-utils)

| Tool | Description | Example |
|------|-------------|---------|
| `pdftotext` | Extract text from PDF | `pdftotext document.pdf -` |
| `pdfinfo` | Get PDF metadata | `pdfinfo document.pdf` |
| `pdftoppm` | Convert PDF to images | `pdftoppm -png document.pdf output` |

**Common patterns:**
```bash
# Extract specific pages
pdftotext -f 1 -l 10 -layout document.pdf -

# Get page count
pdfinfo document.pdf | grep Pages

# OCR scanned PDF
pdftoppm -png document.pdf output
tesseract output-1.png stdout
```

### ePub Tools

| Tool | Description | Size | Notes |
|------|-------------|------|-------|
| `ebook-convert` | Full-featured ePub converter | ~120MB | Calibre package |
| `epub2txt` | Lightweight ePub to text | ~1MB | Fallback option |

**Common patterns:**
```bash
# Calibre (full conversion)
ebook-convert book.epub .txt

# epub2txt (quick extraction)
epub2txt book.epub -
```

### OCR Tool (tesseract)

```bash
# OCR an image
tesseract image.png stdout

# OCR with language
tesseract image.png stdout -l por
```

### Installation

| Distro | Command |
|--------|---------|
| Arch | `sudo pacman -S poppler calibre tesseract` |
| Debian/Ubuntu | `sudo apt install poppler-utils calibre tesseract-ocr` |
| Void | `sudo xbps-install -S poppler calibre tesseract` |
| Alpine | `sudo apk add poppler tesseract-ocr calibre` |
| Fedora | `sudo dnf install poppler-utils calibre tesseract` |

### Configuration

Default `~/.config/ask-ai/tools.toml` includes:

```toml
[external.tools.pdftotext]
enabled = true
timeout = 30

[external.tools.pdfinfo]
enabled = true
timeout = 5

[external.tools.pdftoppm]
enabled = true
timeout = 60

[external.tools.tesseract]
enabled = true
timeout = 120

[external.tools.ebook-convert]
enabled = true
timeout = 60

[external.tools.epub2txt]
enabled = true
timeout = 30
```

## LED Control Tools (5)

Control NeoPixel LED strips via a Raspberry Pi Pico W HTTP server. These tools allow natural language control of lighting through REST API calls to the device.

**Note:** This is an optional feature for personal IoT projects. Build with `--features led-tools` to enable.

### Configuration

LED tools require configuration before use:

```toml
# ~/.config/ask-ai/config.toml
[led]
ip = "192.168.1.100"  # Required: IP address of your Raspberry Pi Pico W
port = 80             # Optional: HTTP port (default: 80)
```

Tools are only available when `led.ip` is configured. If not set, LED tools won't be registered.

### Prerequisites

1. **Hardware:** Raspberry Pi Pico W with NeoPixel LED strip
2. **Software:** Server running from [led-control project](https://github.com/luksamuk/led-control)
3. **Network:** Device must be reachable from your machine

### led_get_status

Get current LED status (power state, program, brightness, color).

```
Function: led_get_status
Args: (none)
Example: led_get_status()
```

**Output includes:**
- `power`: "on" or "off"
- `program`: Current program (0=Christmas, 1=Trail, 2=Lamp)
- `brightness`: Current brightness level (0.02 to 1.0)
- `color_hex`: Current color in hex format (e.g., "ffa648")
- `color_rgb`: Current color as R, G, B values (0-255 each)

**Example queries:**
- "What's the current LED status?"
- "What color are the LEDs?"
- "Is the LED strip on?"

### led_set_power

Turn LEDs on, off, or toggle the current state.

```
Function: led_set_power
Args: action (string: "on", "off", or "toggle")
Example: led_set_power(action: "on")
Example: led_set_power(action: "toggle")
```

**Actions:**
- `"on"` - Turn LEDs on
- `"off"` - Turn LEDs off
- `"toggle"` - Invert current state

### led_set_program

Set the LED program mode.

```
Function: led_set_program
Args: program (string or number: "0", "1", "2", or "next")
Example: led_set_program(program: "lamp")
Example: led_set_program(program: "next")
```

**Programs:**
- `0` or `"christmas"` - Christmas lights effect (cycling colors)
- `1` or `"trail"` - Trail effect (back-and-forth motion)
- `2` or `"lamp"` - Static lamp mode (solid color, uses brightness and color)
- `"next"` or `"cycle"` - Advance to next program

### led_set_brightness

Set LED brightness level.

```
Function: led_set_brightness
Args: brightness (string, 0.02 to 1.0)
Example: led_set_brightness(brightness: "0.5")   # 50% brightness
Example: led_set_brightness(brightness: "1.0")   # Full brightness
Example: led_set_brightness(brightness: "0.1")   # Dim (10%)
```

**Note:** Brightness affects all programs. Low values (< 0.05) are very dim.

### led_set_color

Set LED color for Lamp mode. Accepts either hex or separate RGB values.

```
Function: led_set_color
Args: 
  - hex (string, optional): Color in hex format like "ff5500"
  - r (string, optional): Red value (0-255)
  - g (string, optional): Green value (0-255)
  - b (string, optional): Blue value (0-255)
Example: led_set_color(hex: "ff5500")                # Orange
Example: led_set_color(r: "255", g: "85", b: "0")    # Orange via RGB
Example: led_set_color(hex: "00ff00")                # Green
```

**Color Tips for LLMs:**
- Use `led_get_status()` first to get current RGB values
- RGB format (r/g/b separate) is easier for calculations
- To make "more red": increase R, decrease G and B
- To make "warmer": increase R, slightly decrease B
- To make "cooler": increase B, slightly decrease R
- Common colors:
  - Red: `ff0000` or `r: 255, g: 0, b: 0`
  - Green: `00ff00` or `r: 0, g: 255, b: 0`
  - Blue: `0000ff` or `r: 0, g: 0, b: 255`
  - White: `ffffff` or `r: 255, g: 255, b: 255`
  - Warm white: `ffa040`
  - Cool white: `f0f8ff`

### Example Workflows

**Natural language color adjustment:**
```
User: "Turn the LEDs slightly more orange"
Action: led_get_status()
Result: { "color_rgb": { "r": 255, "g": 100, "b": 50 }, ... }
Analysis: To make it more orange, I should increase green slightly
Action: led_set_color(r: 255, g: 130, b: 40)
Response: "I've adjusted the LEDs to be more orange."
```

**Setting up lighting:**
```
User: "Set the LEDs to warm white at 50% brightness for reading"
Action: led_set_program(program: "lamp")
Action: led_set_color(hex: "ffa040")
Action: led_set_brightness(brightness: "0.5")
Response: "Done! LEDs are now in lamp mode with warm white at 50% brightness."
```

**Quick toggle:**
```
User: "Goodnight, turn off the lights"
Action: led_set_power(action: "off")
Response: "Goodnight! LEDs turned off."
```

### Error Handling

LED tools handle connection errors gracefully:

| Situation | Error Message |
|-----------|---------------|
| Device unreachable | `Error: Could not connect to LED device. Please check if the device is powered on and connected to the network.` |
| Invalid program | `Error: Invalid program '4'. Use 0 (Christmas), 1 (Trail), 2 (Lamp), or 'next'.` |
| Invalid brightness | `Error: Brightness 1.5 out of range. Must be between 0.02 and 1.0.` |
| Invalid color | `Error: Invalid color value. Red must be between 0 and 255, got 300.` |
| Not configured | LED tools won't be available if `led.ip` is not configured |

### Building with LED Tools

```bash
# Build with LED tools enabled
cargo build --release --features led-tools

# Build with LED tools and other optional features
cargo build --release --features "led-tools,finance-tools"
```

## Using Tools

### Automatic Tool Detection

Tools are automatically enabled for capable models:

```bash
# Tools auto-enabled for qwen2.5-coder:7b
ask-ai -m qwen2.5-coder:7b "Tell me about Pikachu"

# Tools auto-enabled for qwen3-coder
ask-ai -m qwen3-coder "What's the weather in Tokyo?"
```

### Force Enable Tools

Force tools on any model:

```bash
ask-ai --tools "Tell me about Pikachu"
```

### Tool User Prompt

Use enhanced prompt for better tool selection:

```bash
ask-ai -p tool_user "What's the weather?"
```

### Disable Specific Tools

Blacklist tools via configuration:

```toml
# ~/.config/ask-ai/config.toml
[tools]
blacklist = ["web_search", "fetch_pokemon"]
```

## Tool Examples

### Pokémon Queries

```bash
# Comprehensive data
ask-ai "Tell me everything about Charizard"

# Specific information
ask-ai "What are Pikachu's stats?"
ask-ai "Show me Eevee's evolution chain"
ask-ai "What type is super effective against Water?"

# Compare Pokémon
ask-ai "Compare Blastoise and Charizard stats"

# Move information
ask-ai "Tell me about Thunderbolt"
ask-ai "What moves can Pikachu learn?"
```

### Weather Queries

```bash
# Current weather
ask-ai "What's the weather in Tokyo?"

# Forecast
ask-ai "Weather forecast for Paris"

# Specific queries
ask-ai "Is it raining in London?"
ask-ai "What's the temperature in New York?"

# With country
ask-ai "Weather in Sydney, Australia"
```

### Web Search Queries

```bash
# General search
ask-ai "Search for Rust async patterns"

# News
ask-ai "Latest technology news"

# Quick facts
ask-ai "What is quantum computing?"

# Follow up with scraping
ask-ai "Find information about the Rust programming language, then scrape the official website"
```

### Stock Quotes

```bash
# US stocks
ask-ai "Get the stock quote for Apple"
ask-ai "What's Google's stock price?"

# Brazilian stocks
ask-ai "Cotação da Petrobras"
ask-ai "Preço das ações da Vale"
```

### File Operations

```bash
# Read a file
ask-ai "Read the README.md file"

# List directory contents
ask-ai "Show me the files in the src directory"

# Search for code patterns
ask-ai "Find all TODO comments in the codebase"

# Analyze project structure
ask-ai "List all Rust files recursively and tell me what each module does"

# Search and analyze
ask-ai "Search for all functions named 'handle_' in the src directory"

# Multi-file analysis
ask-ai "Read Cargo.toml and tell me what dependencies this project has"
```

**Complex file operations:**

```bash
# Count lines of code
ask-ai "List all .rs files recursively, then count total lines of code"

# Find largest files
ask-ai "List the src directory recursively and identify the 5 largest files"

# Pattern analysis
ask-ai "Search for all 'async fn' declarations in src and summarize the async functions"
```

## Tool Selection

The model automatically selects appropriate tools based on your query:

```mermaid
graph TD
    A[User Query] --> B{File operation?}
    B -->|Yes> C[Use File tools]
    B -->|No> D{Contains Pokémon?}
    D -->|Yes> E[Use Pokémon tools]
    D -->|No> F{Contains Weather?}
    F -->|Yes> G[Use Weather tools]
    F -->|No> H{Contains Stock ticker?}
    H -->|Yes> I[Use Finance tools]
    H -->|No> J[Use Web Search]
```

## Known Issues

None currently.

## Tool Capable Models

| Model | Tools | Notes |
|-------|-------|-------|
| qwen2.5-coder:7b | ✅ | Best for tools |
| qwen3-coder | ✅ | Code + tools |
| qwen3.5:4b | ✅ | General purpose (multimodal) |

## Debug Mode

See tool calls in debug mode:

```bash
ask-ai -d "Tell me about Pikachu"

# Output includes:
# - Tool calls with arguments (detailed format)
# - Tool results
# - Model configuration
# - Raw errors with pretty printing (when errors occur)
```

### Tool Call Visibility

**Tool calls are always visible**, even without debug mode. This is intentional - users have the right to see what tools are being executed on their system.

**Without debug mode:**
```
🔧 Calling: read_file(path=README.md, max_lines=50)
```

**With debug mode (`-d`):**
```
═══════════════════════════════════════════════════════════════
🔧 TOOL CALL: read_file
───────────────────────────────────────────────────────────────
  path: README.md
  max_lines: 50
───────────────────────────────────────────────────────────────
📤 TOOL RESULT for read_file:
[content...]
═══════════════════════════════════════════════════════════════
```

### Error Display in Debug Mode

When errors occur, debug mode shows the raw error with pretty printing:

**Without debug mode:**
```
❌ Tool execution failed: Error calling tool
```

**With debug mode (`-d`):**
```
❌ Tool execution failed (RAW):
ToolCallError(
    InternalToolError(
        reqwest::Error {
            kind: Decode,
            source: Error("missing field `daily`", line: 1, column: 633)
        }
    )
)
```

This helps developers debug API issues and tool failures.

## Best Practices

1. **Use capable models** - qwen2.5-coder:7b works best
2. **Be specific** - "Pikachu stats" vs "Tell me about Pikachu"
3. **Use tool_user prompt** - For complex queries
4. **Check debug mode** - If tools aren't working
5. **Weather doesn't need API key** - Always available
6. **Keep file sandbox enabled** - For security
7. **Use relative paths** - When working with files
8. **Limit search scope** - Use file patterns to narrow searches
9. **Count before reading** - Use `count_lines` before reading large files
10. **Read in segments** - Use `read_file_segment` with BOTH `start_line` AND `num_lines` (required)

## Tool Error Handling

Tools are designed to handle errors gracefully and provide helpful feedback to the LLM:

### Error Philosophy

**Tools never crash the application.** Instead, they return informative error messages that help the LLM understand what went wrong and how to fix it.

### CRITICAL: Parameter Types for LLM Tools

**LLMs frequently pass parameters as strings instead of proper JSON types.** All numeric/optional parameters MUST use `Option<String>` type.

See [AGENTS.md - CRITICAL: Parameter Types for LLM Tools](../../AGENTS.md#critical-parameter-types-for-llm-tools) for detailed guidelines.

**Quick Summary:**

```rust
// ✅ CORRECT: Accept strings, parse internally
pub async fn web_search(
    query: String,
    num_results: Option<String>,  // NOT Option<u8>!
) -> Result<String, ...> {
    let num = parse_num_results(num_results, 5, 10);
    // ...
}

fn parse_num_results(s: Option<String>, default: usize, max: usize) -> usize {
    match s {
        Some(ref val) if !val.trim().is_empty() => {
            val.trim().parse::<usize>().unwrap_or(default).min(max)
        }
        _ => default,
    }
}
```

**Why strings?** LLMs may send:
- `"5"` instead of `5` (string instead of number)
- `"null"` instead of `null` (string literal instead of JSON null)
- `""` instead of omitting the parameter

All of these fail with `Option<usize>` but work correctly with `Option<String>` + internal parsing.

Examples:

| Situation | Error Message |
|-----------|---------------|
| File not found | `Error: File not found: README.md. Please check if the file exists or try a different file name (e.g., README.org instead of README.md).` |
| Invalid line number | `Error: Invalid start_line 500. File has 100 lines. Line numbers start at 1.` |
| Missing required arg | `Error: Invalid num_lines ''. Must be a positive number.` |
| File too large | `Error: File too large (1.5 MB). Use count_lines to check file size, then read_file_segment to read in chunks.` |
| Search pattern error | `Error: Invalid regex pattern '[a-z'. Please check your regex syntax.` |
| API error | `Weather API error: 429. Please try again later.` |

### How the LLM Uses Errors

When a tool returns an error:

1. The error is returned as a string result (not an exception)
2. The LLM sees the error in the tool result
3. The LLM can try a different approach (e.g., try `README.org` if `README.md` not found)
4. The LLM can inform the user about the issue

### When Tools Fail

Only critical/unrecoverable errors cause failures:
- Network timeouts (after retries)
- System-level errors (out of memory)
- Invalid configuration

All other errors return helpful messages so the LLM can adapt.

## Security Considerations

### File Tool Security

File tools are sandboxed to prevent unauthorized access:

- ✅ Can read files in current project
- ❌ Cannot access `/etc/passwd`, `/home/otheruser`, etc.
- ❌ Cannot follow symlinks outside sandbox
- ❌ Cannot read files > 1MB (DoS protection)

### Tool Blacklisting

Disable potentially problematic tools:

```toml
# ~/.config/ask-ai/config.toml
[tools]
# Disable tools you don't want
blacklist = ["web_search"]
```

## See Also

- [Configuration](./configuration.md) - Tool configuration and blacklisting
- [query](./commands/query.md) - Using tools with queries
- [Models](./models.md) - Tool-capable models
- [Prompts](./prompts.md) - Tool user prompt mode