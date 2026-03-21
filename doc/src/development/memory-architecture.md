# Memory Architecture

**Status:** Active  
**Version:** v0.37.1  
**Updated:** 2026-03-21

This document provides a unified view of Ask-AI's memory systems and how they compose the LLM context.

---

## Overview

Ask-AI has four layers of memory that work together to provide context-aware responses:

1. **Session Memory** — Volatile, in-memory messages for the current conversation
2. **Conversation Memory** — Persistent conversation history with semantic retrieval
3. **Factual Memory** — Long-term storage of user preferences and project facts
4. **Context Assembly** — How all layers combine into the LLM prompt

---

## Memory Layers

```mermaid
graph TB
    subgraph Layer1["Layer 1: Session Memory"]
        A1["Messages in RAM"]
        A2["Lost on exit"]
        A3["Current conversation only"]
    end
    
    subgraph Layer2["Layer 2: Conversation Memory"]
        B1["SQLite Database"]
        B2["Message Embeddings"]
        B3["FTS5 Keyword Index"]
        B4["Semantic Retrieval"]
    end
    
    subgraph Layer3["Layer 3: Factual Memory"]
        C1["Facts Table"]
        C2["FTS5 Facts Index"]
        C3["Decay System"]
        C4["Conflict Resolution"]
    end
    
    subgraph Layer4["Layer 4: Context Assembly"]
        D1["System Prompt"]
        D2["SOUL.md"]
        D3["AGENTS.md"]
        D4["User Facts"]
        D5["Retrieved Context"]
        D6["Recent Messages"]
        D7["Current Query"]
    end
    
    Layer1 --> Layer2
    Layer2 --> Layer4
    Layer3 --> Layer4
    
    style Layer1 fill:#ffcdd2,stroke:#c62828,color:#b71c1c
    style Layer2 fill:#fff3e0,stroke:#ef6c00,color:#e65100
    style Layer3 fill:#e8f5e9,stroke:#2e7d32,color:#1b5e20
    style Layer4 fill:#e3f2fd,stroke:#1565c0,color:#0d47a1
```

---

## Layer 1: Session Memory

**What it stores:** Messages from the current conversation session.

**Characteristics:**
- Stored in RAM only
- Lost when session ends (unless using `/save`)
- Not searchable across sessions

**Lifecycle:**
```
User sends message → Add to session → Display to user → End session → Lost
```

**Commands:**
- `/save [name]` — Persist session to named storage
- `/load <name>` — Load a saved session
- `/clear` — Clear session messages
- `/forget` — Delete session from database

---

## Layer 2: Conversation Memory

**What it stores:** All conversation messages with embeddings for semantic search.

**Characteristics:**
- Persistent SQLite database (`~/.local/share/ask-ai/embeddings.db`)
- Full-text search via FTS5
- Vector embeddings for semantic similarity
- Hybrid retrieval (BM25 + Vector + RRF)

**Architecture:**

```mermaid
graph LR
    subgraph Input["Message Input"]
        M[New Message]
    end
    
    subgraph Processing["Processing"]
        E1[Chunk if > 1024 chars]
        E2[Generate Embedding]
        E3[Store in SQLite]
    end
    
    subgraph Retrieval["Retrieval"]
        Q[User Query]
        R1[Embed Query]
        R2[BM25 Keyword Search]
        R3[Vector Similarity]
        R4[RRF Fusion]
        R5[Enrich with Responses]
    end
    
    M --> E1 --> E2 --> E3
    Q --> R1
    R1 --> R2
    R1 --> R3
    R2 --> R4
    R3 --> R4
    R4 --> R5
    
    style Input fill:#e8f5e9,stroke:#2e7d32,color:#1b5e20
    style Processing fill:#fff3e0,stroke:#ef6c00,color:#e65100
    style Retrieval fill:#e3f2fd,stroke:#1565c0,color:#0d47a1
```

**Key Tables:**

| Table | Purpose |
|-------|---------|
| `conversations` | Session metadata (project, model, timestamps) |
| `messages` | Message content and metadata |
| `messages_fts` | FTS5 full-text search index |
| `message_embeddings` | Vector embeddings (short messages) |
| `chunk_embeddings` | Vector embeddings (long message chunks) |

**Retrieval Flow:**

```mermaid
sequenceDiagram
    participant User
    participant Chat
    participant DB as SQLite + FTS5
    participant Embed as Embedding Client
    participant LLM as Ollama
    
    User->>Chat: Send message
    Chat->>Chat: Save to messages table
    
    opt Message > 1024 chars
        Chat->>Chat: Split into chunks
    end
    
    Chat->>Embed: Generate embedding
    Embed-->>Chat: 256-dim vector
    Chat->>DB: Store embedding
    
    Note over Chat: Next query...
    
    User->>Chat: Ask question
    Chat->>Embed: Embed query
    Chat->>DB: BM25 keyword search
    Chat->>DB: Vector similarity search
    Chat->>Chat: RRF fusion (0.4/0.6 weights)
    Chat->>DB: Enrich with responses
    Chat->>LLM: Send context + retrieved messages
    LLM-->>User: Response
```

### Embedding Fallback (v0.37.1+)

When embedding generation fails due to context overflow, the system automatically retries with smaller chunks:

```mermaid
sequenceDiagram
    participant Chat
    participant Embed as Embedding Client
    participant Ollama
    
    Chat->>Embed: embed(text)
    Embed->>Ollama: API request
    Ollama-->>Embed: Error: context_length_exceeded
    
    Note over Embed: Fallback activated
    
    loop Halve context size (max 3 iterations)
        Embed->>Embed: Split into smaller chunks
        Embed->>Ollama: Retry with smaller chunks
        alt Success
            Ollama-->>Embed: Return embeddings
        else Still exceeds
            Note over Embed: Continue halving
        end
    end
    
    alt All iterations exhausted
        Embed-->>Chat: ContextExceeded error
        Note over Chat: Embedding skipped (recovered later)
    else Success
        Embed-->>Chat: Return embedding(s)
    end
```

**Fallback progression:**
- 512 tokens (default context for nomic-embed-text-v2-moe)
- 256 tokens (first halving)
- 128 tokens (second halving)
- 64 tokens (third halving, minimum)

**Implementation:**
- `embed_with_fallback(text, max_iterations)` in `client.rs`
- Uses `DynamicChunkConfig::halved()` for progressive size reduction
- Called from `session.rs`, `regenerate.rs`, `recovery.rs`, `command_handlers.rs`

**Commands:**
- `/search <query>` — Search conversation history
- `/context` — Show token usage
- `/compact` — Summarize old messages
- `/reindex` — Rebuild embeddings

**Research Foundation:**

This design is based on ["Lost in the Middle: How Language Models Use Long Contexts"](https://arxiv.org/abs/2307.03172) — relevant information should be at the beginning or end of context, never in the middle.

---

## Layer 3: Factual Memory

**What it stores:** Extracted facts and user preferences that persist across sessions.

**Characteristics:**
- SQLite + FTS5 keyword search (no embeddings needed)
- Automatic classification (preference vs fact)
- Ebbinghaus decay curve for automatic pruning
- Conflict resolution for duplicate/contradictory facts
- Two scopes: `global` and `project`

**Architecture:**

```mermaid
graph TB
    subgraph Input["Fact Input"]
        U[User Command: /fact add]
        L[LLM Tool: fact_add]
    end
    
    subgraph Processing["Processing"]
        C[Classify: preference/fact]
        S[Search similar facts]
        R{Conflict?}
    end
    
    subgraph Storage["Storage"]
        F[Facts Table]
        FT[FTS5 Index]
        D[Decay Scores]
    end
    
    subgraph Injection["Context Injection"]
        G[Get global facts]
        P[Get project facts]
        M[Merge and truncate to 2200 chars]
        X[Inject after AGENTS.md]
    end
    
    U --> C
    L --> C
    C --> S
    S --> R
    R -->|Duplicate| SKIP[Skip]
    R -->|Contradiction| UPD[Update existing]
    R -->|New| F
    F --> FT
    F --> D
    G --> M
    P --> M
    M --> X
    
    style Input fill:#e8f5e9,stroke:#2e7d32,color:#1b5e20
    style Processing fill:#fff3e0,stroke:#ef6c00,color:#e65100
    style Storage fill:#fce4ec,stroke:#c2185b,color:#880e4f
    style Injection fill:#e3f2fd,stroke:#1565c0,color:#0d47a1
```

**Categories:**

| Category | Description | Half-Life | Examples |
|----------|-------------|-----------|----------|
| `preference` | User likes/dislikes | 180 days | "I prefer Portuguese", "I like concise responses" |
| `fact` | Objective information | 30 days | "Database is SQLite", "API on port 8080" |

**Decay System:**

Based on the [Ebbinghaus forgetting curve](https://en.wikipedia.org/wiki/Forgetting_curve), facts decay over time:

```
Retention = e^(-t / half_life)

- Preferences: 180-day half-life (remembered longer)
- Facts: 30-day half-life (refreshed more often)
- Access reinforcement: Each access bumps retention
- High-importance preferences: Never pruned
```

**Scopes:**

| Scope | Description | Storage |
|-------|-------------|---------|
| `global` | Applies to all projects | `project_id = NULL` |
| `project` | Specific to current project | `project_id = <git remote or folder name>` |

**Conflict Resolution:**

```mermaid
graph LR
    A[New Fact] --> B[Search Similar FTS5]
    B --> C{Similarity Score}
    C -->|Greater than 0.95| D{Contradiction?}
    C -->|Less than 0.95| E[Insert New]
    D -->|Yes| F[Update Existing]
    D -->|No| G[Skip Duplicate]
    
    style A fill:#e8f5e9,stroke:#2e7d32,color:#1b5e20
    style E fill:#c8e6c9,stroke:#2e7d32,color:#1b5e20
    style F fill:#fff3e0,stroke:#ef6c00,color:#e65100
    style G fill:#ffcdd2,stroke:#c62828,color:#b71c1c
```

**User Commands:**

| Command | Shortcut | Description |
|---------|----------|-------------|
| `/fact add <text> [--global]` | `/fa` | Add a fact |
| `/fact list [--global]` | `/fl` | List stored facts |
| `/fact search <query>` | `/fs` | Search facts |
| `/fact remove <id>` | `/fr` | Remove a fact |
| `/fact prune` | `/fp` | Run decay manually |

**LLM Tools:**

| Tool | Description |
|------|-------------|
| `fact_add(content, category?, scope?)` | Store a fact (auto-classified) |
| `fact_search(query, category?, scope?, limit?)` | Search facts |
| `fact_remove(id)` | Remove a fact by ID |

---

## Layer 4: Context Assembly

**How all layers combine into the LLM prompt:**

```mermaid
graph TB
    subgraph SystemPrompt["SYSTEM PROMPT"]
        S1["SOUL.md<br/>(personality, style)"]
        S2["AGENTS.md<br/>(project guidelines)"]
        S3["USER FACTS<br/>(preferences + facts)"]
        S4["Tools<br/>(available functions)"]
        S5["Platform Info<br/>(date, system)"]
    end
    
    subgraph Retrieved["RETRIEVED CONTEXT"]
        R1["Semantic Search Results<br/>(from Conversation Memory)"]
        R2["Enriched with Responses"]
    end
    
    subgraph Recent["RECENT MESSAGES"]
        M1["Last 10 message pairs"]
        M2["Chronological order"]
    end
    
    subgraph Query["CURRENT QUERY"]
        Q1["User question"]
    end
    
    S1 --> S2 --> S3 --> S4 --> S5
    S5 --> R1
    R1 --> R2
    R2 --> M1
    M1 --> M2
    M2 --> Q1
    
    style S3 fill:#fff3e0,stroke:#ef6c00,color:#e65100
    style R1 fill:#e8f5e9,stroke:#2e7d32,color:#1b5e20
    style Q1 fill:#fce4ec,stroke:#c2185b,color:#880e4f
```

**Order (Research-Based):**

Based on ["Lost in the Middle"](https://arxiv.org/abs/2307.03172) and Anthropic's recommendations:

| Position | Section | Why |
|----------|---------|-----|
| 1 | System Prompt (SOUL + AGENTS + Facts) | Best recall at beginning |
| 2 | Retrieved Context | High relevance, near beginning |
| 3 | Compacted Summary (if any) | Medium priority, never in middle |
| 4 | Recent Messages | Recent context, near end |
| 5 | Current Query | Best comprehension at very end |

**Token Budget:**

| Section | Tokens | Priority |
|---------|--------|----------|
| System Prompt | 500-2000 | Critical |
| User Facts | ~2200 max | High |
| Retrieved Context | 1000-5000 | High |
| Compacted Summary | 500-1000 | Medium |
| Recent Messages | 2000-5000 | High |
| Current Query | Variable | Critical |

**User Facts Format:**

Injected after AGENTS.md:

```
## User Facts

### Preferences
- prefiro respostas em português
- gosto de respostas curtas

### Facts
- o projeto usa SQLite para armazenamento
- a API está na porta 8080
```

**Limits:**
- Per-fact limit: 500 characters (hard limit, rejected at insert)
- Total facts limit: 2200 characters (soft limit, truncated)

---

## When to Use Each System

| Use Case | System | Command/Tool |
|----------|--------|--------------|
| Remember what user said in current session | Session Memory | Automatic |
| Find past discussions about a topic | Conversation Memory | `/search <query>` |
| Remember user preference across sessions | Factual Memory | `/fact add "I prefer..."` |
| Remember project configuration | Factual Memory | `fact_add(scope="project")` |
| AI learns something about user | Factual Memory | `fact_add` (LLM tool) |
| Provide project guidelines | AGENTS.md | Manual file edit |

**Comparison:**

| Feature | Session Memory | Conversation Memory | Factual Memory | AGENTS.md |
|---------|----------------|---------------------|----------------|-----------|
| Scope | Current session | All sessions | Global/Project | Project only |
| Persistence | RAM | SQLite | SQLite | File |
| Search | No | Semantic + Keyword | Keyword | No |
| Decay | No | Compaction | Ebbinghaus | Manual |
| LLM Access | No | Retrieval | Tools | Prompt |

---

## References

- [Ebbinghaus Forgetting Curve](https://en.wikipedia.org/wiki/Forgetting_curve) — The basis for fact decay
- [Lost in the Middle](https://arxiv.org/abs/2307.03172) — Why context ordering matters
- [Anthropic Prompt Engineering](https://docs.anthropic.com/claude/docs/prompt-engineering) — Context ordering best practices

---

## See Also

- [Factual Memory System](./factual-memory-system.md) — Detailed implementation
- [Context Anatomy](./context-anatomy.md) — Context composition details
- [Architecture](./architecture.md) — Overall system architecture