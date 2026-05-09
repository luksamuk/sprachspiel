# Sprach 2.0: CAS Research — Technical Design Document

**Status:** 🟡 RESEARCH NEEDED  
**Created:** 2026-04-11  
**Priority:** P7 (after all P1-P5 current items)

## Overview

This document provides the technical design details for the Sprach 2.0 proposals, based on the self-analysis article that identifies sprachspiel as a Complex Adaptive System (CAS). The article identifies key CAS properties already present (feedback loops, decay, emergence) and proposes extensions to increase open-endedness and adaptive behavior.

**Source article key findings:**

1. ✅ CAS properties present: multiple agents (50+ tools), feedback (retrieval → context → response → memory), emergence (fact importance from usage)
2. ❌ CAS properties absent: open-endedness (tools registered at compile-time), autopoiesis (doesn't produce own components), full embodiment (no continuous sensors or homeostasis)
3. Proposals S2.1-S2.6 aim to address these gaps
4. All DEC-001 to DEC-007 decisions validated by state-of-the-art research

**Prerequisite:** All P1-P5 current items must be completed before starting P7 work.

---

## S2.1: Visualize Connections Tool

### Summary

LLM tool that, given an item ID or query, finds top-N most similar items via embedding similarity and returns a Mermaid graph visualization.

### Existing Infrastructure

| Component | Location | Status |
|-----------|----------|--------|
| Vector search | `src/content/db.rs:637-715` (`search_content_semantic`) | ✅ Works |
| Embedding storage | `src/content/db.rs` (`content_embeddings` vec0 table) | ✅ Works |
| Embedding client | `src/embeddings/client.rs` (`EmbeddingClient`) | ✅ Works |
| Search scoring | `ContentSearchResult.score` (cosine distance) | ✅ Works |
| Hybrid search | `src/content/db.rs:817-850` (`search_content_hybrid`) | ✅ Works |
| Item ID lookup | `content_items` table with `id` column | ✅ Works |
| Source types | `src/db/operations.rs` (`SourceType` enum) | ✅ Works |

### What's Missing

- **No `search_similar_to_item(item_id)` function** — current search is query-based, not item-based
- **No Mermaid graph output** — no tool generates graph visualizations
- **No tool exposing neighbor relationships** — `remember(query=...)` searches, but doesn't visualize

### Implementation Sketch

```rust
// 1. New function in src/content/db.rs
pub fn search_similar_to_item(
    &self,
    item_id: i64,
    limit: usize,
) -> Result<Vec<ContentSearchResult>> {
    // Look up the embedding for the given item_id
    // Search content_embeddings with that embedding
    // Return top-N results excluding the item itself
}

// 2. New tool in src/tools/connections.rs (new file)
/// Visualize connections between content items.
///
/// Given an item ID or search query, finds the most similar items
/// and returns a Mermaid graph showing the connections.
///
/// # Arguments
/// * `id` - The content item ID (format: "msg:N", "note:N", "doc:N")
/// * `query` - Alternative: search query instead of ID
/// * `limit` - Number of connections to show (default: 10, max: 20)
///
/// # Returns
/// Mermaid graph and text summary of connections.
#[ollama_rs::function]
pub async fn visualize_connections(
    id: Option<String>,
    query: Option<String>,
    limit: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Resolve item ID or query to an embedding
    // Call search_similar_to_item or search_content_semantic
    // Format results as Mermaid graph
    // Return graph + summary
}
```

### Open Questions

1. **Items without embeddings:** What happens when an item doesn't have an embedding? Return error message suggesting `remember(id="...")` first?
2. **Mermaid rendering:** Terminal output (text diagram) vs. file output (`.md` file) vs. markdown code block in response? Consider that the LLM can render Mermaid in its response.
3. **Caching strategy:** Should connections be calculated on-the-fly every time or cached? (DEC-001 says: cache incrementally, on-demand)
4. **Optimal N:** What N is meaningful? Too few = trivial; too many = noise. Default 10 seems reasonable, max 20.
5. **Cross-type connections:** Should we allow connections between different content types (message ↔ note ↔ document)? Or restrict to same type?

---

## S2.2: Content Relations Graph

### Summary

Persistent `content_relations` table storing explicit connections between content items, with a two-layer architecture: automatic embedding-based discovery and LLM-based classification on demand.

### Two-Layer Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Layer 2: Classification (Top-Down, On-Demand)          │
│  - LLM classifies relation type when user requests      │
│  - Results cached in content_relations                   │
│  - Adds: relation_type, confidence, justification        │
├─────────────────────────────────────────────────────────┤
│  Layer 1: Discovery (Bottom-Up, Automatic)               │
│  - Embedding-based proximity search                      │
│  - Already works via search_content_semantic()           │
│  - Returns: (item_id, strength) pairs                    │
└─────────────────────────────────────────────────────────┘
```

### Relation Types

Inspired by Zettelkasten linking patterns:

| Type | Definition | Zettelkasten Equivalent |
|------|-----------|------------------------|
| `extends` | B develops or elaborates on A | Folgezettel (sequence note) |
| `contradicts` | B contests or challenges A | Gegenposition |
| `instantiates` | B is a specific case of A | Beispiel (example) |
| `cites` | B references A explicitly | Verweis (reference link) |
| `presupposes` | B assumes A as foundational | Voraussetzung |
| `resolves` | B dissolves a tension in A | Synthese |
| `questions` | B problematizes A | Fragestellung |

### Schema Migration (v8 → v9)

```sql
-- New table for content relations
CREATE TABLE IF NOT EXISTS content_relations (
    source_id INTEGER NOT NULL,
    target_id INTEGER NOT NULL,
    relation_type TEXT NOT NULL,      -- extends, contradicts, instantiates, cites, presupposes, resolves, questions
    strength REAL NOT NULL,           -- cosine similarity (0-1)
    confidence REAL NOT NULL DEFAULT 0.0,  -- LLM confidence (0-1), 0.0 for auto-discovered
    justification TEXT,               -- 1-sentence LLM explanation
    created_at INTEGER NOT NULL,
    PRIMARY KEY (source_id, target_id),
    FOREIGN KEY (source_id) REFERENCES content_items(id) ON DELETE CASCADE,
    FOREIGN KEY (target_id) REFERENCES content_items(id) ON DELETE CASCADE
);

-- Index for lookups by source
CREATE INDEX IF NOT EXISTS idx_relations_source
    ON content_relations(source_id);

-- Index for lookups by target (reverse direction)
CREATE INDEX IF NOT EXISTS idx_relations_target
    ON content_relations(target_id);

-- Index for lookups by relation type
CREATE INDEX IF NOT EXISTS idx_relations_type
    ON content_relations(relation_type);
```

### Existing Infrastructure

| Component | Location | Notes |
|-----------|----------|-------|
| Schema migration system | `src/db/connection.rs:71-495` | Proven v2→v8 incremental migration |
| Content items | `src/content/db.rs` | Unified storage with CRUD |
| Embedding search | `src/content/db.rs:637-715` | Returns `ContentSearchResult` with `score` |
| Embedding client | `src/embeddings/client.rs` | `nomic-embed-text-v2-moe`, 256d |
| Fact decay system | `src/facts/decay.rs` | Model for relation aging |

### Implementation Sketch

```rust
// src/content/relations.rs (new file)

/// Relation types between content items
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RelationType {
    Extends,
    Contradicts,
    Instantiates,
    Cites,
    Presupposes,
    Resolves,
    Questions,
}

/// A relation between two content items
pub struct ContentRelation {
    pub source_id: i64,
    pub target_id: i64,
    pub relation_type: RelationType,
    pub strength: f32,        // cosine similarity, 0-1
    pub confidence: f32,      // LLM confidence, 0-1 (0.0 = auto-discovered)
    pub justification: Option<String>,
    pub created_at: DateTime<Utc>,
}

// Database operations
pub fn insert_relation(&self, relation: &ContentRelation) -> Result<()>;
pub fn get_relations_for_item(&self, item_id: i64) -> Result<Vec<ContentRelation>>;
pub fn get_relations_by_type(&self, relation_type: RelationType) -> Result<Vec<ContentRelation>>;
pub fn delete_relation(&self, source_id: i64, target_id: i64) -> Result<()>;
```

### LLM Classification Prompt (Layer 2)

```
Given these two content items:

Source: [excerpt of source item, truncated to 500 chars]
Target: [excerpt of target item, truncated to 500 chars]

And their similarity score: {strength}

Classify their relationship. Choose exactly one:
- extends: Target develops or elaborates on Source
- contradicts: Target contests or challenges Source
- instantiates: Target is a specific case of Source
- cites: Target references Source explicitly
- presupposes: Target assumes Source as foundational
- resolves: Target dissolves a tension in Source
- questions: Target problematizes Source

Output JSON:
{"relation_type": "...", "confidence": 0.0-1.0, "justification": "one sentence"}
```

### Open Questions

1. **When to create relations?**
   - **Lazy (on-query):** Only store results when `visualize_connections` is called. Cheaper, but limited.
   - **Eager (on-insert):** Calculate top-N similar items when a new item is stored. More complete, but costs tokens per insert.
   - **Batch (periodic):** Periodically compute relations for recent items. Middle ground.
   - **DEC-001 answer:** Cache incrementally — compute on demand, cache the result.

2. **Should unused relations decay?**
   - The fact decay system (`src/facts/decay.rs`) provides a model (Ebbinghaus curve).
   - Could apply similar decay to relations that aren't accessed.

3. **Is persistent storage better than S2.1 (on-the-fly)?**
   - S2.1 alone recalculates every time → expensive for large item sets.
   - S2.2 caches results → cheaper for repeated queries, but requires migration.
   - Recommendation: S2.1 first, then S2.2 as optimization.

4. **Scalability:**
   - 10K items × 10 relations = 100K rows → acceptable for SQLite.
   - 100K items × 10 relations = 1M rows → may need indices, which are planned above.

5. **Bidirectionality:**
   - If A extends B, does B "is extended by A"? Store once or twice?
   - Recommendation: Store once (source→target), allow reverse lookup via index.

6. **LLM cost for classification:**
   - Each classification call costs ~200-400 tokens.
   - 10K items × 10 relations = 100K calls = 20-40M tokens at classification time.
   - Mitigated by on-demand caching (DEC-001).

---

## S2.3: Reflection on Triggers + Curation

### Summary

Self-reflection triggered by specific events (not periodic), with a curation pipeline that saves drafts requiring human approval.

### Trigger Detection Mechanisms

| Trigger | Detection Strategy | Implementation Location |
|---------|-------------------|----------------------|
| **Error** | Tool call returns error message | `src/chat/custom_coordinator.rs` (after tool execution) |
| **Surprise** | Embedding distance > threshold from expected results | `src/retrieval/context_builder.rs` (after retrieval) |
| **Conflict** | Two retrieved items have contradictory signals | New: `detect_content_conflict()` in `src/content/` |
| **Pattern** | Same query embedding appears N times in M sessions | New: query pattern tracker in `src/content/db.rs` |
| **On-demand** | User types `/reflect` or `/reflect <topic>` | New: command handler in `src/chat/commands.rs` |

### Curation Pipeline

```
┌─────────────────────────────────────────────────────────────┐
│  TRIGGER DETECTED                                            │
│  (error, surprise, conflict, pattern, or on-demand)         │
│         ↓                                                    │
│  GENERATE REFLECTION                                          │
│  - LLM call with specialized prompt                         │
│  - Includes: recent context, trigger details, session info  │
│         ↓                                                    │
│  QUALITY CHECK                                               │
│  1. Novelty: cosine sim < 0.85 with existing notes?          │
│  2. Actionability: ≥1 concrete implication?                 │
│  3. Density: ≥200 words, ≥1 Zettelkasten connection?       │
│         ↓                                                    │
│  SAVE AS DRAFT                                               │
│  - ContentSource::Llm, status="draft"                        │
│  - Visible to user via /notes command with --draft flag     │
│         ↓                                                    │
│  HUMAN APPROVAL                                              │
│  - /approve-patch <id> → publishes draft                   │
│  - /reject-patch <id> → deletes draft                       │
│  - Drafts auto-expire after 30 days                         │
└─────────────────────────────────────────────────────────────┘
```

### Reflection Prompt Template

```
You are reflecting on your own cognitive process. Based on the following trigger:

TRIGGER: {trigger_type}
DETAILS: {trigger_details}

RECENT CONTEXT:
{recent_context_summary}

EXISTING NOTES ON SIMILAR TOPICS:
{existing_notes}

Generate a reflection that:
1. Identifies what surprised you or what pattern you noticed
2. Connects this to existing knowledge (cite specific note IDs)
3. Proposes at least one concrete action (new note, tool improvement, behavior change)
4. Is at least 200 words

Format as a structured note with sections: OBSERVATION, CONNECTIONS, ACTIONS.
```

### Existing Infrastructure

| Component | Location | Notes |
|-----------|----------|-------|
| Note creation | `src/tools/notes.rs` | `note_add` with `ContentSource::Llm` |
| Session state | `src/chat/session.rs` | Message counting exists |
| Tool error recovery | `src/chat/custom_coordinator.rs` | Error detection for tool calls |
| Embedding search | `src/content/db.rs` | Distance calculation for surprise detection |
| Fact decay | `src/facts/decay.rs` | Model for reflection aging |

### Open Questions

1. **Surprise threshold tuning:** What distance constitutes "surprise"? Start with 0.7, adjustable in config?
2. **Conflict detection:** How to identify contradictory notes? Embedding similarity + content analysis? Or simple keyword overlap?
3. **Pattern tracking:** Store query embeddings per-project? Per-session? How many repeats trigger reflection?
4. **Draft storage:** Use `content_items` with `status='draft'` column? Separate table? Or prefix in `message_type`?
5. **Token cost:** Each reflection costs ~500-1000 output tokens. How to budget? Max reflections per session?

---

## S2.4: Plugin System (WASM)

This is tracked as PRIORITY 15 in `IMPLEMENTATION.md`. The Sprach 2.0 article adds the following architectural details that should be incorporated when P15 research begins:

### 4-Layer Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Layer 4: Runtime WASM (wasmer/wasmtime)                │
│  - Sandboxing by capabilities                           │
│  - Memory/CPU limits (e.g., 16MB max)                  │
└─────────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────┐
│  Layer 3: Host Interface (Rust)                          │
│  - Safe APIs: db_read, note_read, web_search             │
│  - Blocked: fs_write, network_raw, process_spawn        │
└─────────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────┐
│  Layer 2: Plugin Manifest (TOML)                         │
│  - Metadata: name, version, author, capabilities        │
│  - Dependencies: minimum host version                  │
└─────────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────┐
│  Layer 1: Plugin Code (Rust → WASM)                     │
│  - Tool implementation                                  │
│  - No direct system access                              │
└─────────────────────────────────────────────────────────┘
```

### Security: Capabilities Model (DEC-004)

```toml
# plugin-manifest.toml
name = "visualize-connections"
version = "0.1.0"

[capabilities]
allowed = ["db_read", "note_read"]
denied = ["fs_write", "network_raw", "process_spawn"]
```

⚠️ **CRITICAL SECURITY NOTE (2026-04-19):** The DEC-004 `process_spawn` denied capability is now **essential, not optional**. The Anthropic MCP SDK `StdioServerParameters` has a by-design vulnerability (CVE-2025-65720 and related) that allows arbitrary command execution via STDIO transport configuration. Any MCP server connection that uses STDIO spawns an OS process with the parent application's privileges — even if the connection fails. This means `denied = ["process_spawn"]` in a plugin manifest is meaningless if we allow MCP STDIO servers, because MCP STDIO *itself* is process spawning.

**Mitigation strategy for sprachspiel:**
1. MCP STDIO servers MUST be explicitly approved by the user (no auto-discovery, no zero-click install)
2. MCP server configurations containing `command` fields MUST be treated as arbitrary code execution
3. An allowlist of approved MCP server commands MUST be maintained in `config.toml`
4. MCP servers SHOULD prefer Streamable HTTP transport over STDIO when available
5. When STDIO is required, the server process MUST run in a sandboxed environment (seccomp/cgroups/namespace)

See ADR-007 in IMPLEMENTATION.md for full details.

### Semantic Versioning (DEC-005)

- Host v0.8 accepts plugins v0.7.x (backward compatible)
- Major version must match, minor ≥ required
- Deprecation announced with 1 version advance notice

### Alternatives Research Needed

| System | Approach | Security | Notes |
|--------|----------|----------|-------|
| **WASM (wasmer/wasmtime)** | Sandboxed bytecode | Capability-based | Emerging standard (DEC-004) |
| E2B | Cloud sandbox | Full isolation | Requires network, latency |
| Daytona | Dev environment | Container-based | Heavier, more ops overhead |

> **DEC-007 (2026-04-19): MCP STDIO Security** — See IMPLEMENTATION.md ADR-007 for the full decision. In summary: the Anthropic MCP SDK `StdioServerParameters` has a by-design RCE vulnerability (CVE-2025-65720 et al.) that executes arbitrary commands before any validation. This affects the DEC-004 capabilities model: `denied = ["process_spawn"]` is meaningless if we allow MCP STDIO servers, because STDIO *is* process spawning. The mitigation requires (1) explicit user approval for every MCP server install, (2) command allowlist in config.toml, (3) HTTP transport preference over STDIO, and (4) sandbox for STDIO processes.

---

## S2.5: SOUL.md Patching with Approval

### Summary

Dynamic personality adaptation through LLM-generated patches to SOUL.md, with mandatory human approval via `/apply-patch` command.

### Patch Flow (DEC-006)

```
┌─────────────────────────────────────────────────────────┐
│  1. USER GIVES FEEDBACK                                  │
│     "Your responses are too verbose" / "Be more concise" │
│         ↓                                                │
│  2. SPRACH GENERATES PATCH SUGGESTION                    │
│     - Analyzes current SOUL.md                           │
│     - Proposes specific changes                           │
│     - Shows diff before/after                             │
│         ↓                                                │
│  3. LUCAS REVIEWS VIA /apply-patch                        │
│     - /suggest-patch → view proposed changes             │
│     - /apply-patch <id> → accept and apply              │
│     - /reject-patch <id> → discard                      │
│         ↓                                                │
│  4. PATCH APPLIED + GIT COMMIT                            │
│     - Timestamped backup created                          │
│     - SOUL.md updated atomically                        │
│     - If git repo: auto-commit with message              │
└─────────────────────────────────────────────────────────┘
```

### Relationship to P5 (Feedback Infrastructure)

| Aspect | P5 (Feedback) | S2.5 (SOUL.md Patching) |
|--------|---------------|------------------------|
| What it captures | What happened (signal + weight) | Who I am (personality style) |
| Mechanism | Weight propagation on messages | Text patching on SOUL.md |
| Scope | Retrieval quality | Behavior style |
| Risk | Low (data change) | Medium (personality corruption) |
| Human approval | No (implicit) | Yes (mandatory) |
| Persistence | Database weights | File modification |

Both are complementary: P5 improves *retrieval quality* (what context to surface), S2.5 improves *behavior style* (how to respond).

### Implementation Sketch

```rust
// src/soul.rs additions

/// Generate a patch suggestion for SOUL.md based on user feedback
pub async fn suggest_soul_patch(
    feedback: &str,
    current_soul: &str,
    llm: &Ollama,
) -> Result<PatchSuggestion> {
    // 1. Build prompt with feedback + current SOUL.md
    // 2. Ask LLM to propose specific changes
    // 3. Validate patch doesn't corrupt structure
    // 4. Return PatchSuggestion with diff
}

/// Apply a validated patch to SOUL.md
pub fn apply_soul_patch(patch: &Patch) -> Result<()> {
    // 1. Create timestamped backup: SOUL.md.{timestamp}.bak
    // 2. Apply patch atomically (temp file + rename)
    // 3. Validate result has ## sections
    // 4. If git repo: auto-commit
}
```

### Open Questions

1. **Should SOUL.md be in git?** What about users without git? Recommendation: make git optional, always create `.bak` backup.
2. **Patch format:** Search-replace strings? Section-level replacement? Line-level diffs? Recommendation: section-level replacement is safest.
3. **Validation:** How to ensure patches don't corrupt SOUL.md? Check for `## ` sections, non-empty content, no HTML injection.
4. **Backup mechanism:** Timestamped copies before patching? Git history? Both?
5. **Scope of patches:** Should patches only modify certain sections (e.g., `## Behavior` but not `## Identity`)?

---

## S2.6: Skills Auto-Registration and Meta-Architecture

**Status:** 🕐 AWAITING MATURATION

This proposal is intentionally vague. Skills that create and register other skills (meta-architecture) requires S2.1-S2.5 to be operational and well-tested before meaningful design.

**Why wait:** The meta-level (skills creating skills) can only be designed after we have empirical experience with:
- How LLMs use `visualize_connections` (S2.1)
- How the relation graph grows organically (S2.2)
- How triggers fire and reflections are curated (S2.3)
- How plugins are sandboxed (S2.4/P15)
- How personality patches work in practice (S2.5)

No implementation sketch at this time.

---

## Code Analysis References

### Key files for each proposal

| Proposal | Primary files to modify | Primary files to create |
|----------|------------------------|------------------------|
| S2.1 | `src/content/db.rs`, `src/tools/registry.rs`, `IMPLEMENTATION.md` | `src/tools/connections.rs` |
| S2.2 | `src/content/db.rs`, `src/db/schema.rs`, `src/db/connection.rs` | `src/content/relations.rs`, schema v9 migration |
| S2.3 | `src/chat/custom_coordinator.rs`, `src/chat/repl.rs` | `src/reflection/` (triggers, pipeline, prompts) |
| S2.4 | See P15 in IMPLEMENTATION.md | `src/plugins/` (host, registry, manifest) |
| S2.5 | `src/soul.rs`, `src/chat/commands.rs` | Patch generation and application logic |

### Database schema considerations

| Proposal | Schema changes | Migration |
|----------|---------------|-----------|
| S2.1 | None (uses existing `content_embeddings`) | None |
| S2.2 | New `content_relations` table | v8 → v9 |
| S2.3 | `status` column on `content_items` or separate `drafts` table | v9 → v10 or extend v9 |
| S2.4 | None (plugins are external) | None |
| S2.5 | None (SOUL.md is filesystem) | None |

---

## Reference

- **Sprach 2.0 Article:** Self-analysis identifying sprachspiel as a Complex Adaptive System (original in private notes, design details in this document)
- **State of Art Research:** See DEC-001 to DEC-007 in this document
- **Competitors:** Joplin GSoC 2026 (note graphs), OpenClaw (WASM sandbox)
- **Related roadmap items:** P5 (Feedback Infrastructure), P15 (Plugin System)