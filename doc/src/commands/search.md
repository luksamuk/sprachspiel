# /search - Semantic Search

Search conversation history using hybrid search (keyword + semantic).

## Synopsis

```
/search <query>
/search <query> <limit>
/find <query>
/f <query>
```

## Description

The `/search` command performs a hybrid search across all conversation history using:

1. **BM25 (Keyword Search)** - Full-text search via FTS5
2. **Semantic Search** - Vector similarity via sqlite-vec
3. **Reciprocal Rank Fusion (RRF)** - Combines results with weights 0.4/0.6

Results are ranked by relevance and displayed with metadata.

## Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `query` | Yes | Search query (any text) |
| `limit` | No | Maximum number of results (default: 10) |

## Prerequisites

The `/search` command requires:

1. **Ollama running** with the embedding model:
   ```bash
   ollama pull nomic-embed-text-v2-moe
   ```

2. **Messages indexed** - Messages must be saved to the database to be searchable.
   See [Migration](#migration) for details.

## Examples

### Basic Search

```
> /search "LED control"
🔍 🔗 **user** (score: 0.0423)
   How do I control the LED strip?
   _default_ _2026-03-02 14:30_

🧠 🔗 **assistant** (score: 0.0387)
   You can use the LED tools to control...
   _default_ _2026-03-02 14:31_
```

### Limit Results

```
/search "Rust async" 5
```

### Using Alias

```
/find "how to configure"
/f "error handling" 20
```

## Output Format

Each result shows:

| Icon | Meaning |
|------|---------|
| 🔍 | Keyword match (BM25 only) |
| 🧠 | Semantic match (vector only) |
| 🔗 | Hybrid match (both keyword and semantic) |

| Icon | Role |
|------|------|
| 👤 | User message |
| 🤖 | Assistant message |
| ⚙️ | System message |
| 🔧 | Tool message |

## How It Works

1. **Query Embedding** - Your query is converted to a 256-dimensional vector using `nomic-embed-text-v2-moe`
2. **BM25 Search** - Full-text search using FTS5 with escaped query
3. **Semantic Search** - Vector similarity using cosine distance
   - Searches both message embeddings (short messages) and chunk embeddings (long messages)
   - Combines results from both sources
4. **RRF Fusion** - Results combined with Reciprocal Rank Fusion
5. **Ranking** - Final results sorted by combined score

## Chunking

Messages longer than 1024 characters are automatically split into overlapping 
chunks for better semantic search:

- **Chunk size**: 1024 characters
- **Overlap**: 200 characters (20%)
- **All roles**: Embeddings generated for user, assistant, system, and tool messages

### Example

A 3000-character assistant response is split into:
- Chunk 0: characters 0-1024
- Chunk 1: characters 824-1848  (overlaps with chunk 0)
- Chunk 2: characters 1648-2672 (overlaps with chunk 1)
- Chunk 3: characters 2472-3000 (final chunk)

**Search results** show the matched chunk content (with ellipsis for context) 
but clicking/viewing reveals the full message.

### Why Chunking?

1. **Granularity**: 3000+ character embeddings lose precision
2. **Context**: Query about "Wittgenstein's later work" matches chunk 
   discussing specifically that, not entire response
3. **Recall**: Chunks with overlap ensure boundary terms are searchable

**Why Overlap?**

Without overlap, search terms split across chunk boundaries are invisible:
- Chunk 0: "...Wittgenstein's philosophical inves-"
- Chunk 1: "tigationss demonstrate..."

A search for "philosophical investigations" wouldn't match either chunk.

With 20% overlap (200 chars):
- Chunk 0: "...Wittgenstein's philosophical investigationss demonst-"
- Chunk 1: "...tionss demonstrate..."

The full phrase appears in both chunks, ensuring matches.

**Chunk vs Full Message Storage:**

| Aspect | Chunk Storage | Full Message |
|--------|---------------|--------------|
| Embedding granularity | High | Low |
| Match precision | Sentence-level | Document-level |
| Storage overhead | 2-3x for long msgs | 1x |
| Search relevance | Better for long responses | Good for short queries |

We store **both**: chunks for search, parent message for display.

## Migration

Messages are automatically indexed when:
- Added during chat session (real-time)
- Migrated via `/migrate` command (historical)

**Note:** Run `/migrate` after upgrading to v0.22.0+ to re-index all messages with chunking.

## Storage

- Database: `~/.local/share/ask-ai/embeddings.db`
- Embedding dimensions: 256 (Matryoshka truncation from 768)
- Storage per message: ~3KB base (text + embedding)
- Storage per chunk: ~3KB additional (chunk embedding)

Messages > 1024 chars: overhead is ~3KB per chunk

## Limitations

- Searches all conversations (project filter not yet implemented)
- Requires `nomic-embed-text-v2-moe` model to be downloaded
- Messages saved only in JSON format are not indexed

## See Also

- `/context` - View context metrics and token usage
- `/compact` - Compact conversation history
- [Roadmap](../development/roadmap.md) - Future search improvements