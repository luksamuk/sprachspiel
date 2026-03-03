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
4. **RRF Fusion** - Results combined with Reciprocal Rank Fusion
5. **Ranking** - Final results sorted by combined score

## Migration

Messages are **not automatically indexed** yet. This feature is planned for v0.21.0.

To index existing conversations, you would need to:

1. Run `/migrate` (not yet implemented)
2. Or manually insert messages via SQLite

## Storage

- Database: `~/.local/share/ask-ai/embeddings.db`
- Embedding dimensions: 256 (Matryoshka truncation from 768)
- Storage per message: ~3KB (text + embedding)

## Limitations

- Searches all conversations (project filter not yet implemented)
- Requires `nomic-embed-text-v2-moe` model to be downloaded
- Messages saved only in JSON format are not indexed

## See Also

- `/context` - View context metrics and token usage
- `/compact` - Compact conversation history
- [Roadmap](../development/roadmap.md) - Future search improvements