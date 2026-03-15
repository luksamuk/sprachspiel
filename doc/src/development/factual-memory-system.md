# Factual Memory System Design

**Status:** PLANNED  
**Priority:** P0 (before Feedback System)  
**Created:** 2026-03-14  
**Depends on:** None (standalone feature)

---

## Executive Summary

This document defines the implementation plan for a **Factual Memory System** that enables ask-ai to remember user preferences, project facts, and environment details across sessions.

**Problem:** Users currently need to repeat contextual information (e.g., "my docs are in ~/docs") in every session.

**Solution:** A persistent fact storage system with automatic decay, LLM-autonomous management, and intelligent conflict resolution.

---

## 1. Problem Statement

### Current State

- ask-ai has session-based conversation memory (SQLite + embeddings)
- No persistent storage for user/project facts
- Users must repeat preferences and environment details each session
- AGENTS.md is static and project-level only

### Desired State

- LLM autonomously learns and stores facts about user/project
- Facts persist across sessions and projects
- Decay system prevents stale information
- User can inspect/correct via commands
- Project facts + global facts with proper merging

---

## 2. Architecture Overview

### 2.1 Storage Model

```
┌─────────────────────────────────────────────────────────────┐
│                    FACTUAL MEMORY SYSTEM                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────────────┐    ┌─────────────────────────┐  │
│  │   STORAGE            │    │   TOOLS                  │  │
│  │                      │    │                          │  │
│  │   SQLite table:      │    │   fact_add()             │  │
│  │   - id               │    │   fact_list()            │  │
│  │   - scope            │    │   fact_remove()          │  │
│  │   - category         │    │   fact_search()          │  │
│  │   - content          │    │                          │  │
│  │   - importance       │    │   LLM tools (autonomous) │  │
│  │   - access_count     │    │                          │  │
│  │   - created_at       │    └─────────────────────────┘  │
│  │   - last_accessed    │                                 │
│  │   - decay_score      │    ┌─────────────────────────┐  │
│  │   - source           │    │   DECAY MAINTENANCE     │  │
│  │   - invalidated_at   │    │                         │  │
│  │                      │    │   - Background task      │  │
│  └──────────────────────┘    │   - Runs on startup     │  │
│                              │   - Prunes < 5% decay    │  │
│  ┌──────────────────────┐    │   - Periodic (daily)    │  │
│  │   SCOPES             │    │                         │  │
│  │                      │    └─────────────────────────┘  │
│  │   Project (default) │                                 │
│  │   .ask-ai/facts.db   │    ┌─────────────────────────┐  │
│  │                      │    │   PROMPT INJECTION       │  │
│  │   Global (optional)  │    │                         │  │
│  │   ~/.config/ask-ai/  │    │   System prompt section:│  │
│  │   facts_global.db    │    │   "## User Facts"       │  │
│  │                      │    │   - project facts first  │  │
│  └──────────────────────┘    │   - global facts last   │  │
│                              │   - merges duplicates   │  │
│                              │   - resolves conflicts  │  │
│                              └─────────────────────────┘  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Categories and Scopes

**Categories** (determine decay half-life):

| Category | Description | Half-Life | Examples |
|----------|-------------|-----------|----------|
| `preference` | User preferences, likes/dislikes | 180 days | "prefiro português", "gosto de respostas curtas" |
| `fact` | Objective facts about environment/project | 30 days | "docs estão em ~/docs", "projeto usa SQLite" |
| `context` | Temporary conversation state | 7 days | "estamos trabalhando no issue #7" |

**Scopes** (determine visibility):

| Scope | Description | Storage | Override |
|-------|-------------|----------|----------|
| `project` | Facts specific to current project | `.ask-ai/facts.db` | Can be overridden by global |
| `global` | Facts that apply to all projects | `~/.config/ask-ai/facts_global.db` | Takes precedence in conflicts |

### 2.3 Interaction with Feedback System

Factual Memory and Feedback System are **orthogonal** and operate at different layers:

```
┌─────────────────────────────────────────────────────────────────┐
│                    CONTEXT ASSEMBLY                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Layer 1: SYSTEM PROMPT                                         │
│  ├── SOUL.md (personality)                                     │
│  ├── AGENTS.md (project context)                                │
│  └── [FACTUAL MEMORY] ←──── Factual Memory injects HERE         │
│      "User prefers Portuguese"                                    │
│      "Docs are in ~/docs"                                         │
│      "Project uses Rust"                                          │
│                                                                 │
│  Layer 2: RETRIEVED CONTEXT (past messages)                     │
│  ├── Hybrid Search (BM25 + Semantic)                            │
│  └── [FEEDBACK WEIGHT] ←──── Feedback System acts HERE          │
│      Message #42: +1.2 (good feedback, recent)                  │
│      Message #15: -0.3 (bad feedback, decayed)                  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Key insight:** 
- Factual Memory answers "What do I know about the user/project?"
- Feedback System answers "How should I weight retrieved messages?"

They don't conflict because they operate on orthogonal dimensions.

---

## 3. Database Schema

### 3.1 Facts Table

```sql
CREATE TABLE facts (
    id INTEGER PRIMARY KEY,
    
    -- Classification
    scope TEXT NOT NULL CHECK(scope IN ('project', 'global')),
    category TEXT NOT NULL CHECK(category IN ('preference', 'fact', 'context')),
    
    -- Content
    content TEXT NOT NULL,
    
    -- Decay parameters
    importance REAL DEFAULT 0.5 CHECK(importance BETWEEN 0 AND 1),
    access_count INTEGER DEFAULT 0,
    decay_score REAL DEFAULT 1.0,
    
    -- Timestamps
    created_at REAL NOT NULL,
    last_accessed REAL NOT NULL,
    
    -- Source tracking
    source TEXT DEFAULT 'user' CHECK(source IN ('user', 'llm')),
    
    -- Conflict resolution
    invalidated_at REAL,  -- Set when superseded by newer fact
    
    -- Project association (for project-scoped facts)
    project_id TEXT,
    
    FOREIGN KEY (project_id) REFERENCES projects(id)
);

-- Full-text search
CREATE VIRTUAL TABLE facts_fts USING fts5(
    content, 
    content='facts', 
    content_rowid='id'
);

-- Indexes for common queries
CREATE INDEX idx_facts_scope_category ON facts(scope, category);
CREATE INDEX idx_facts_decay ON facts(decay_score);
CREATE INDEX idx_facts_project ON facts(project_id) WHERE scope = 'project';
```

### 3.2 Scope Storage

```
Project Facts (default):
  Location: .ask-ai/facts.db (per-project SQLite)
  Query: Automatically loaded when in project directory
  
Global Facts (optional):
  Location: ~/.config/ask-ai/facts_global.db
  Query: Always loaded, merged with project facts
```

---

## 4. Classification System

### 4.1 Heuristic Classification (Primary)

90%+ of facts are classifiable via pattern matching, avoiding LLM calls:

```rust
fn quick_classify(content: &str) -> Option<FactClassification> {
    let lower = content.to_lowercase();
    
    // Preferences
    if lower.contains("prefiro") || lower.contains("prefere") 
       || lower.contains("gosto") || lower.contains("gosta")
       || lower.contains("odeio") || lower.contains("não gosto")
       || lower.contains("quero") || lower.contains("nao quero")
       || lower.contains("i prefer") || lower.contains("i like")
       || lower.contains("i hate") || lower.contains("i want") {
        return Some(FactClassification {
            category: Category::Preference,
            confidence: 0.95,
            ..default()
        });
    }
    
    // Facts (paths, environment, project info)
    if lower.contains("está em") || lower.contains("esta em")
       || lower.contains("localizado") || lower.contains("located at")
       || lower.contains("caminho") || lower.contains("path")
       || lower.contains("usamos") || lower.contains("we use")
       || lower.contains("o projeto") || lower.contains("the project") {
        return Some(FactClassification {
            category: Category::Fact,
            confidence: 0.9,
            ..default()
        });
    }
    
    // Context (current state, temporary)
    if lower.contains("estamos") || lower.contains("we are")
       || lower.contains("trabalhando") || lower.contains("working on")
       || lower.contains("agora") || lower.contains("currently")
       || lower.contains("hoje") || lower.contains("today") {
        return Some(FactClassification {
            category: Category::Context,
            confidence: 0.85,
            ..default()
        });
    }
    
    // Ambiguous - need LLM
    None
}
```

### 4.2 LLM Classification (Fallback)

For ambiguous cases, use stateless LLM call:

```rust
async fn classify_with_llm(content: &str, similar: &[Fact]) -> FactClassification {
    let prompt = format!(
        r#"Classify this fact: "{content}"

Similar existing facts:
{similar_facts}

Classify:
1. Category: "preference" (user likes/dislikes), "fact" (objective info), "context" (temporary)
2. Scope: "project" (current project only) or "global" (all projects)
3. Action: "ADD" (new), "UPDATE" (replace existing), "SKIP" (duplicate)
4. Confidence: 0.0-1.0

Return JSON:
{{"category": "...", "scope": "...", "action": "...", "confidence": 0.0}}

Examples:
- "I prefer dark mode" → {{"category": "preference", "scope": "global", "action": "ADD", "confidence": 0.95}}
- "API key is in .env" → {{"category": "fact", "scope": "project", "action": "ADD", "confidence": 0.9}}
- "We're fixing bug #7" → {{"category": "context", "scope": "project", "action": "ADD", "confidence": 0.85}}
- "I like Python" (but existing: "I hate Python") → {{"category": "preference", "scope": "global", "action": "UPDATE", "confidence": 0.8}}
"#,
        similar_facts = similar.iter()
            .map(|f| format!("- {} [{}]", f.content, f.category))
            .collect::<Vec<_>>()
            .join("\n")
    );
    
    let response = llm.generate(prompt).await?;
    parse_classification(&response)
}
```

**Cost:** ~200-300 tokens per ambiguous classification (rare)

### 4.3 Hybrid Approach

```rust
pub async fn add_fact(content: String, scope: Option<String>) -> Result<String, Error> {
    // 1. Search for similar existing facts
    let similar = search_similar_facts(&content, limit=5).await?;
    
    // 2. Try heuristic classification (90%+ hit rate, 0 tokens)
    if similar.is_empty() {
        if let Some(classification) = quick_classify(&content) {
            return insert_fact(content, classification, scope).await;
        }
    }
    
    // 3. Fallback to LLM for ambiguous/conflict cases
    let classification = classify_with_llm(&content, &similar).await?;
    
    // 4. Execute action
    match classification.action {
        Action::Add => insert_fact(content, classification, scope).await,
        Action::Update => update_fact(similar[0].id, content).await,
        Action::Skip => Ok(format!("Fact already exists: {}", similar[0].content)),
    }
}
```

---

## 5. Decay System

### 5.1 Decay Formula

Based on Ebbinghaus forgetting curve with access reinforcement:

```rust
const HALF_LIVES: &[(&str, f32)] = &[
    ("preference", 180.0),  // days
    ("fact", 30.0),
    ("context", 7.0),
];

const ACCESS_BOOST: f32 = 0.1;  // 10% per access
const MIN_RETENTION: f32 = 0.05; // 5% threshold for pruning

fn compute_retention(fact: &Fact, now: DateTime<Utc>) -> f32 {
    let half_life = HALF_LIVES.iter()
        .find(|(cat, _)| *cat == fact.category)
        .map(|(_, hl)| *hl)
        .unwrap_or(30.0);
    
    let days_since_access = (now - fact.last_accessed).num_days() as f32;
    
    // Exponential decay: R = 2^(-t / half_life)
    let decay = 2f32.powf(-days_since_access / half_life);
    
    // Importance multiplier (important facts retain longer)
    let importance_mult = 1.0 + fact.importance * 0.5;
    
    // Access boost (frequently accessed facts retain longer)
    let access_mult = 1.0 + ACCESS_BOOST * (fact.access_count as f32).log2().max(0.0);
    
    (decay * importance_mult * access_mult).min(1.0)
}

fn should_prune(fact: &Fact, now: DateTime<Utc>) -> bool {
    // Never prune high-importance preferences
    if fact.category == "preference" && fact.importance >= 0.8 {
        return false;
    }
    
    compute_retention(fact, now) < MIN_RETENTION
}
```

### 5.2 Access Reinforcement

```rust
fn on_fact_access(fact: &mut Fact) {
    fact.access_count += 1;
    fact.last_accessed = Utc::now();
    
    // Optionally boost importance on access
    fact.importance = (fact.importance + 0.05).min(1.0);
}
```

### 5.3 Periodic Maintenance

```rust
fn run_decay_cycle(conn: &Connection) -> Result<usize, Error> {
    let now = Utc::now();
    let mut pruned = 0;
    
    // 1. Find facts below retention threshold
    let facts_to_prune: Vec<Fact> = conn
        .query("SELECT * FROM facts WHERE invalidated_at IS NULL", [])?
        .filter(|f| should_prune(f, now))
        .collect();
    
    // 2. Archive before deletion (optional, for debugging)
    for fact in &facts_to_prune {
        conn.execute(
            "INSERT INTO facts_archive SELECT * FROM facts WHERE id = ?",
            [fact.id]
        )?;
    }
    
    // 3. Delete pruned facts
    for fact in &facts_to_prune {
        conn.execute("DELETE FROM facts WHERE id = ?", [fact.id])?;
        pruned += 1;
    }
    
    // 4. Update decay scores for remaining facts
    conn.execute(
        "UPDATE facts SET decay_score = ? WHERE id = ?",
        // Computed per-fact
    )?;
    
    Ok(pruned)
}
```

**Schedule:**
- On startup: Run once
- Background: Every 24 hours

---

## 6. Conflict Resolution

### 6.1 Detection

```rust
fn detect_conflicts(new_fact: &str, existing: &[Fact]) -> Vec<Conflict> {
    let mut conflicts = Vec::new();
    
    for fact in existing {
        let similarity = semantic_similarity(new_fact, &fact.content);
        
        if similarity > 0.85 {
            // Check for contradiction
            if is_contradictory(new_fact, &fact.content) {
                conflicts.push(Conflict {
                    existing_fact: fact.clone(),
                    conflict_type: ConflictType::Contradiction,
                    similarity,
                });
            } else if similarity > 0.95 {
                conflicts.push(Conflict {
                    existing_fact: fact.clone(),
                    conflict_type: ConflictType::Duplicate,
                    similarity,
                });
            }
        }
    }
    
    conflicts
}

fn is_contradictory(a: &str, b: &str) -> bool {
    // Pattern detection for contradictions
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    
    // "I like X" vs "I hate X"
    // "X is A" vs "X is B" (where A != B)
    // "X is located at A" vs "X is located at B"
    
    // Use LLM for complex cases
    false // Simplified
}
```

### 6.2 Resolution Strategy

Based on Mem0's approach:

```rust
enum ResolutionAction {
    Add,      // New fact, no conflict
    Update,   // Replace existing fact (temporal change)
    Skip,     // Duplicate or equivalent
}

fn resolve_conflict(conflict: Conflict, new_fact: &str) -> ResolutionAction {
    match conflict.conflict_type {
        ConflictType::Duplicate => ResolutionAction::Skip,
        ConflictType::Contradiction => {
            // Temporal resolution: newer wins
            // Unless existing is a preference with high importance
            if conflict.existing_fact.category == "preference" 
               && conflict.existing_fact.importance >= 0.8 {
                // Ask LLM to adjudicate
                ResolutionAction::Update // Simplified
            } else {
                ResolutionAction::Update
            }
        }
    }
}
```

### 6.3 Merging for Prompt Injection

When injecting facts into prompts, resolve scope conflicts:

```rust
fn merge_facts(project_facts: Vec<Fact>, global_facts: Vec<Fact>) -> Vec<Fact> {
    let mut merged = Vec::new();
    
    // 1. Add project facts
    for fact in project_facts {
        merged.push(fact);
    }
    
    // 2. Add global facts, checking for conflicts
    for global_fact in global_facts {
        let conflict = merged.iter().find(|f| {
            f.scope == "project" && 
            semantic_similarity(&f.content, &global_fact.content) > 0.85
        });
        
        match conflict {
            Some(existing) => {
                // Project fact takes precedence in conflicts
                // Log conflict for debugging
                log_conflict(existing, &global_fact);
            }
            None => {
                merged.push(global_fact);
            }
        }
    }
    
    merged
}
```

---

## 7. LLM Tools

### 7.1 Tool Definitions

```rust
/// Add a fact to memory. Use proactively when you learn something 
/// important about the user, their preferences, or their environment.
///
/// Default scope is 'project' (facts specific to current project).
/// Use 'global' for facts that apply across all projects.
///
/// # Arguments
/// * `content` - The fact to remember (e.g., "user prefers Portuguese")
/// * `category` - Optional: "preference", "fact", or "context" (default: auto-detect)
/// * `scope` - Optional: "project" (default) or "global"
#[ollama_rs::function]
pub async fn fact_add(
    content: String,
    category: Option<String>,
    scope: Option<String>,
) -> Result<String, Box<dyn Error>> {
    // Implementation
}

/// Search for facts in memory.
///
/// # Arguments
/// * `query` - Search query
/// * `scope` - Optional: "project", "global", or null (searches both)
#[ollama_rs::function]
pub async fn fact_search(
    query: String,
    scope: Option<String>,
) -> Result<String, Box<dyn Error>> {
    // Implementation
}

/// Remove a fact by ID.
///
/// # Arguments
/// * `id` - The fact ID to remove
#[ollama_rs::function]
pub async fn fact_remove(id: i64) -> Result<String, Box<dyn Error>> {
    // Implementation
}
```

### 7.2 System Prompt Integration

```rust
fn build_facts_section(facts: &[Fact]) -> String {
    if facts.is_empty() {
        return String::new();
    }
    
    let mut section = String::from("\n## User Facts\n\n");
    
    // Group by category
    let preferences: Vec<_> = facts.iter()
        .filter(|f| f.category == "preference")
        .collect();
    let project_facts: Vec<_> = facts.iter()
        .filter(|f| f.category == "fact" && f.scope == "project")
        .collect();
    let global_facts: Vec<_> = facts.iter()
        .filter(|f| f.scope == "global")
        .collect();
    
    if !preferences.is_empty() {
        section.push_str("### Preferences\n");
        for fact in preferences {
            section.push_str(&format!("- {}\n", fact.content));
        }
    }
    
    if !project_facts.is_empty() {
        section.push_str("### Project\n");
        for fact in project_facts {
            section.push_str(&format!("- {}\n", fact.content));
        }
    }
    
    if !global_facts.is_empty() {
        section.push_str("### Global\n");
        for fact in global_facts {
            section.push_str(&format!("- {}\n", fact.content));
        }
    }
    
    // Enforce character limit (~2200 chars)
    if section.len() > 2200 {
        section = format!("{}\n... (truncated)\n", &section[..2100]);
    }
    
    section
}
```

---

## 8. User Commands

### 8.1 Command Definitions

```
/fact add <text>                        # Add project-scoped fact
/fact add --global <text>               # Add global fact
/fact add --category <cat> <text>       # Add with specific category
/fact list                              # List all facts
/fact list --global                     # List global facts only
/fact list --category <cat>             # List by category
/fact remove <id>                       # Remove a fact
/fact search <query>                    # Search facts
/fact set-importance <id> <0.0-1.0>     # Set importance (affects decay)
```

### 8.2 User Experience Rationale

| Command | Purpose | Why User-facing? |
|---------|---------|------------------|
| `/fact add` | Explicitly add what user knows | Bootstrap, override LLM |
| `/fact list` | See what's stored | Inspection, debugging |
| `/fact remove` | Remove incorrect facts | Correction, privacy |
| `/fact search` | Find specific facts | Debugging |
| `/fact set-importance` | Adjust decay | User control |

**NOT user-facing:** `/fact set-category`, `/fact set-scope` (LLM infers these).

---

## 9. Implementation Phases

### Phase 0.1: Schema (0.5 day)

- Create `facts` table in SQLite
- Create `facts_fts` virtual table
- Add indexes
- Create `facts_archive` table for soft deletes

### Phase 0.2: Core Module (1 day)

- `src/db/facts.rs` - CRUD operations
- `src/facts/decay.rs` - Decay calculations
- `src/facts/classify.rs` - Heuristic + LLM classification
- Unit tests for decay formulas

### Phase 0.3: LLM Tools (1 day)

- `fact_add()` tool
- `fact_search()` tool
- `fact_remove()` tool
- Tool registration
- Integration with existing tool system

### Phase 0.4: Prompt Injection (0.5 day)

- Load facts on session start
- Merge project + global facts
- Build `## User Facts` section
- Inject into system prompt
- Character limit enforcement (~2200 chars)

### Phase 0.5: Decay Maintenance (1 day)

- Background task for decay
- Startup decay run
- Periodic decay (every 24h)
- Archive before delete
- Metrics logging

### Phase 0.6: User Commands (0.5 day)

- `/fact add` command
- `/fact list` command
- `/fact remove` command
- `/fact search` command
- `/fact set-importance` command

### Phase 0.7: Conflict Resolution (1 day)

- Similarity detection
- Conflict classification
- Update vs Skip logic
- Scope merging
- Conflict logging for debugging

### Phase 0.8: Testing & Documentation (0.5 day)

- Unit tests for classification
- Integration tests for decay
- Documentation in doc/
- Update IMPLEMENTATION.md

**Total Estimate:** 6 days

---

## 10. Interaction with Feedback System

### When Both Exist

```
User Query: "How do I configure my web server?"

Context Assembly:
├── System Prompt
│   └── [FACTUAL MEMORY] 
│       - "User uses Nginx" (fact)
│       - "User prefers Portuguese" (preference)
│
├── Retrieved Context (weighted by feedback)
│   ├── Message #42: "To configure nginx..." (score: 1.10)
│   │   └── +0.2 feedback boost
│   └── Message #15: "Ah yes, the server..." (score: 0.77)
│       └── -0.1 feedback penalty
│
└── Response combines:
    - Fact from memory
    - Weighted retrieved messages
```

### Future Integration Points

1. **Facts can receive feedback** - Extension after P2:
   ```sql
   ALTER TABLE facts ADD COLUMN feedback_score REAL DEFAULT 0.0;
   ```

2. **Feedback can correct facts** - When user says "that's wrong":
   ```rust
   // "/feedback bad: the path is actually /home/user/docs"
   // → Creates correction signal
   // → Triggers fact_update("docs path", "/home/user/docs")
   ```

3. **Unified decay** - Both systems use similar decay logic:
   ```rust
   fn compute_weight(memory: &Memory) -> f32 {
       let decay = compute_retention(memory);
       let feedback = memory.feedback_score;
       decay * (1.0 + feedback * 0.5)
   }
   ```

---

## 11. Success Metrics

| Metric | Baseline | Target (1 month) | Target (3 months) |
|--------|----------|------------------|-------------------|
| Facts stored per session | 0 | 2-3 facts | 5-10 facts |
| Fact retrieval accuracy | N/A | 80% | 90% |
| User corrections (fact_remove) | N/A | < 5% | < 2% |
| Decay pruning rate | N/A | 10-20% | 15-25% |
| LLM classification fallback | N/A | < 10% | < 5% |
| Prompt token overhead | 0 | +200 tokens | +200 tokens |

---

## 12. Research References

### Hermes Agent
- Storage: Plain text Markdown files (`MEMORY.md`, `USER.md`)
- Character limits: 2200 chars (memory), 1375 chars (user)
- No decay mechanism
- LLM tool with `add/replace/remove` actions

### Mem0
- Storage: Vector DB + graph DB
- Four operations: ADD, UPDATE, DELETE, NOOP
- Conflict detection via semantic similarity
- Feedback as ranking weight

### Letta/MemGPT
- Storage: Memory blocks in context window
- Self-editing via agent tools
- Implicit learning through sleep-time agents

### Decay Research
- Ebbinghaus forgetting curve: R = e^(-t/S)
- Access reinforcement: importance * log(access_count)
- Half-life by category: preference (180 days), fact (30 days), context (7 days)

---

## 13. Appendices

### A. Character Limits

Based on Hermes research:
- Project facts: 2200 characters total
- Global facts: 2200 characters total
- Per-fact: No explicit limit, but typically < 200 chars

### B. Security Considerations

1. **Prompt Injection Detection** - Scan fact content for:
   - Invisible unicode characters
   - Prompt injection patterns
   - Exfiltration attempts

2. **Atomic Writes** - Use temp file + rename pattern:
   ```rust
   fs::write(temp_path, content)?;
   fs::rename(temp_path, facts_db)?;
   ```

3. **Input Validation** - Reject facts containing:
   - SQL injection patterns
   - Excessive length (> 1000 chars)
   - Binary content

### C. Performance Considerations

1. **Decay Computation** - O(n) where n = number of facts
   - Run asynchronously
   - Cache decay scores

2. **Similarity Search** - Use FTS5 for text, vector for semantic
   - Limit to top 5 similar facts for classification
   - Pre-compute embeddings on insert

3. **Prompt Injection** - Lazy load, only when building prompt
   - Cache facts for session duration
   - Re-compute on `/fact add` or `/fact remove`

---

**Document Status:** CANONICAL - Implementation should follow this design.

**Last Updated:** 2026-03-14