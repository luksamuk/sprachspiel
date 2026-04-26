# Manual Test Template

Run these tests after automated tests pass, before finalizing a merge.

**Issue:** #N - _Brief description_
**Branch:** _branch-name_
**PR:** #N

---

## Prerequisites

```bash
cd /path/to/ask-ai-rs
cargo build --release --features all-tools
ollama serve  # In another terminal

# Backup and reset the database for clean state
cp ~/.local/share/ask-ai/embeddings.db ~/.local/share/ask-ai/embeddings.db.backup 2>/dev/null || true
rm -f ~/.local/share/ask-ai/embeddings.db
```

> **⚠️ Clean database:** Many tests require a clean database to avoid interference from previous test data. Sections requiring this are marked with 🗑️. Reset with:
> ```bash
> rm -f ~/.local/share/ask-ai/embeddings.db
> ```

## Test Model

```bash
# Use environment variable or default
MODEL=${SMOKE_MODEL:-qwen3.5:4b}
ollama list | grep -q "$MODEL" || ollama pull "$MODEL"
echo "Test model: $MODEL"
```

## Schema Version Check

After starting the application, verify the database schema version:

```bash
sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA user_version;"
# Expected: 11 or higher (v11 adds fact_embeddings and has_embedding column)
```

---

---

## LLM Tool Refusal Policy

Some tests require the LLM to call tools. If the model **refuses** to call a tool:

1. **Retry with an explicit instruction** — rephrase the prompt to make the tool call unavoidable (e.g., "You MUST call the tool right now, do not just describe it").
2. **If the model still refuses persistently** — **FAIL the test** and note which model refused which tool.
3. **Switch to an abliterated model** — check `models.toml` for an abliterated variant (e.g., `qwen3.5:4b-abliterated`). If no abliterated model is listed, request one from the user before retrying.

**Rationale:** Abliterated models have safety refusals removed, ensuring deterministic tool calling during tests. A model that refuses to call tools is a valid test failure, not a bug in the application.

---

## 1. _Test Category Name_

**Objective:** _What this test verifies._

```bash
# Prepare test data
echo "test content" > /tmp/test_file.txt
```

In chat with LLM (model with tools support):

> **LLM Refusal:** If the model refuses to call the tool, rephrase explicitly.
> If refusal persists, switch to an abliterated model (see LLM Tool Refusal Policy above).

- [ ] `tool_name("param")` returns success
- [ ] Result shows expected output
- [ ] Related slash command works (e.g., `/command arg`)

**Cleanup:**
```bash
rm -f /tmp/test_file.txt
```

---

## 2. _Another Test Category_

**Objective:** _What this test verifies._

- [ ] Slash command `/cmd arg` works
- [ ] Error handling: `/cmd invalid` shows useful error message
- [ ] Edge case: _describe edge case_

---

## N. _Repeat as Needed_

_Copy the section structure above for each test category._

---

## Fact System: Adverb Normalization (Bug #1 smoke test #2)

**Objective:** Verify that adverb+verb patterns are correctly normalized to third person.

🗄️ Clean database required.

- [ ] `/fact add I really like dark mode` → stored as "User really likes dark mode"
- [ ] `/fact add I always prefer concise answers` → stored as "User always prefers concise answers"
- [ ] `/fact add I never want verbose output` → stored as "User never wants verbose output"
- [ ] `/fact add Eu sempre prefiro respostas curtas` → stored as "User always prefers respostas curtas"
- [ ] `/fact add Eu realmente gosto de café` → stored as "User really likes café"
- [ ] `/fact add I usually don't like verbose errors` → stored as "User usually doesn't like verbose errors"

---

## Fact System: Layer 2 Verb Lemma (Bug #2 smoke test #2)

**Objective:** Verify that third-person verbs are lemmatized in dedup comparison, so "User prefers X" and "I prefer X" match at Layer 2.

🗄️ Clean database required.

- [ ] `/fact add I prefer dark mode` → stored as "User prefers dark mode"
- [ ] `/fact add I prefer dark mode` → **Skipped: Exact duplicate** (Layer 1)
- [ ] `/fact add User prefers dark mode` → **Skipped: Similar fact** (Layer 2, normalized match)

---

## Fact System: `/fact add` Dedup Parity (Bug #3 smoke test #2)

**Objective:** Verify that `/fact add` CLI command uses the same 6-layer dedup as `fact_add` LLM tool.

🗄️ Clean database required.

- [ ] `/fact add I prefer dark mode` → stored as "User prefers dark mode" (ADR-E4 normalization)
- [ ] Wait 3 seconds for embedding generation
- [ ] `/fact add I prefer dark mode` → **Skipped: Exact duplicate** (Layer 1)
- [ ] `/fact add User prefers dark mode` → **Skipped: Similar fact** (Layer 2)
- [ ] `/fact add I like dark mode` → Layer 3.5 catches as paraphrase or FTS5 as similar
- [ ] Clean database again: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add I prefer dark mode` → stored as "User prefers dark mode"
- [ ] Wait 3 seconds for embedding
- [ ] `/fact add I prefer light mode` → should **UPDATE** (semantic triple contradiction — same predicate "prefers", different object "light mode" vs "dark mode")
- [ ] Verify embedding: `sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT content, has_embedding FROM facts WHERE content LIKE '%light mode%'"` → **has_embedding = 1**

---

## Fact System: Semantic Triple Contradiction (Bug S42.4/S43.1 smoke test #3)

**Objective:** Verify that preference overrides and identity changes are correctly detected by semantic triple disambiguation (Layer 3.5, threshold 0.70), both via `/fact add` CLI and auto-extraction.

### §50 EN Preference Override via `/fact add`

🗄️ Clean database required.

- [ ] Clean: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add I prefer dark mode` → **✓ Added**: "User prefers dark mode"
- [ ] `/fact add I prefer light mode` → **↻ Updated**: "User prefers light mode replaces User prefers dark mode (preference override)"
- [ ] `/fact list` → shows only "User prefers light mode" (dark mode is gone)

### §51 EN Preference Override via Auto-Extraction

🗄️ Clean database required.

- [ ] Clean: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] Start chat: `ask-ai chat`
- [ ] `/tools` → **"Tools: disabled"** (force auto-extraction path)
- [ ] Send: "I prefer dark mode" → auto-extraction stores "User prefers dark mode"
- [ ] Send: "Actually, I prefer light mode" → auto-extraction detects contradiction, updates to "User prefers light mode"
- [ ] `/fact list` → shows only "User prefers light mode"

### §52 EN Adverb+Verb Contradiction via `/fact add`

🗄️ Clean database required.

- [ ] Clean: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add I really like vim` → **✓ Added**: "User really likes vim"
- [ ] `/fact add I really like emacs` → **↻ Updated**: "User really likes emacs" replaces "User really likes vim" (same predicate "really likes", different object)
- [ ] `/fact list` → shows only "User really likes emacs"

### §53 EN Identity Change via `/fact add`

🗄️ Clean database required.

- [ ] Clean: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add I live in São Paulo` → **✓ Added**: "User lives in São Paulo"
- [ ] `/fact add I live in Recife` → **↻ Updated**: "User lives in Recife" replaces "User lives in São Paulo" (same predicate "lives in", different city)
- [ ] `/fact list` → shows only "User lives in Recife"

### §54 EN Identity Name Change via `/fact add`

🗄️ Clean database required.

- [ ] Clean: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add My name is Lucas` → **✓ Added**: "User's name is Lucas"
- [ ] `/fact add My name is João` → **↻ Updated**: "User's name is João" replaces "User's name is Lucas"
- [ ] `/fact list` → shows only "User's name is João"

### §55 EN Negation Contradiction via `/fact add`

🗄️ Clean database required.

- [ ] Clean: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add I don't like verbose output` → **✓ Added**: "User doesn't like verbose output"
- [ ] `/fact add I don't like verbose errors` → **↻ Updated**: "User doesn't like verbose errors" replaces "User doesn't like verbose errors" (same predicate "doesn't like", different object)
- [ ] `/fact list` → shows only "User doesn't like verbose errors"

### §56 EN No False Positive — Different Predicates

🗄️ Clean database required.

- [ ] Clean: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add I like Python` → **✓ Added**: "User likes Python"
- [ ] `/fact add I prefer Rust` → **✓ Added**: "User prefers Rust" (NOT a contradiction — different predicates "likes" vs "prefers")
- [ ] `/fact list` → shows BOTH facts

### §57 PT Preference Override via `/fact add`

🗄️ Clean database required.

- [ ] Clean: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add Eu prefiro modo escuro` → **✓ Added**: "User prefers modo escuro" (PT→EN translation, noun preserved)
- [ ] `/fact add Eu prefiro modo claro` → **↻ Updated**: "User prefers modo claro" replaces "User prefers modo escuro" (same predicate "prefers", different object)
- [ ] `/fact list` → shows only "User prefers modo claro"

### §58 PT Identity Change via `/fact add`

🗄️ Clean database required.

- [ ] Clean: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add Meu nome é Lucas` → **✓ Added**: "User's name is Lucas" (ADR-E4: now third person, was "My name is Lucas")
- [ ] `/fact add Meu nome é João` → **↻ Updated**: "User's name is João" replaces "User's name is Lucas"
- [ ] `/fact list` → shows only "User's name is João"

### §59 PT Adverb+Verb Contradiction via Auto-Extraction

🗄️ Clean database required.

- [ ] Clean: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] Start chat: `ask-ai chat`
- [ ] `/tools` → **"Tools: disabled"**
- [ ] Send: "Eu sempre prefiro vim" → auto-extraction stores "User always prefers vim"
- [ ] Send: "Na verdade, eu sempre prefiro emacs" → auto-extraction detects contradiction (same predicate "always prefers", different object), updates
- [ ] `/fact list` → shows only "User always prefers emacs"

### §60 EN Factual Content Not Affected by Semantic Triple

🗄️ Clean database required.

- [ ] Clean: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add The project uses SQLite` → **✓ Added** (no triple extracted — subject ≠ "user")
- [ ] `/fact add The project uses PostgreSQL` → **✓ Added** (no triple extracted — not a preference/identity fact)
- [ ] `/fact list` → shows BOTH facts (factual content coexists, not affected by semantic triple)

### §61 ADR-E4 PT Identity Normalization via Auto-Extraction

🗄️ Clean database required.

- [ ] Clean: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] Start chat: `ask-ai chat`
- [ ] Send: "Meu nome é Ana" → auto-extraction stores "User's name is Ana" (NOT "My name is Ana" — ADR-E4 fix)
- [ ] Send: "Eu moro em São Paulo" → auto-extraction stores "User lives in São Paulo" (NOT "I live in São Paulo")
- [ ] `/fact list` → shows both facts in third person

---

## Fact System: Layer 3.5 via Auto-Extraction with `/tools` Toggle (Bug #4 smoke test #2)

**Objective:** Verify that contradiction detection works through auto-extraction when LLM tool calls are disabled. With the reordered Layer 3.5, contradictions are caught via semantic search + triple disambiguation (not just FTS5).

🗄️ Clean database required.

> **Why `/tools`:** Some models proactively call `fact_add` to resolve contradictions, bypassing auto-extraction. The `/tools` command disables LLM tool calls, forcing contradiction detection through the auto-extraction path.

1. Clean state: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
2. Start chat: `ask-ai chat`
3. `/tools` → should print **"Tools: disabled"**
4. Send: "I prefer dark mode" → auto-extraction stores via normalization + embedding
5. Wait 5 seconds for embedding
6. Send: "Actually, I prefer light mode" → auto-extraction detects contradiction (semantic triple: same predicate "prefers", different object) and updates
7. `/fact list` → shows ONE preference: "User prefers light mode"
8. `/tools` → should print **"Tools: enabled"**
9. Verify auto-extraction worked independently of LLM tool calls

---

## Cleanup

```bash
# Remove any test documents from database (if needed)
# /doc list to see IDs
# /doc delete N for each test document

# Restore original database
rm -f ~/.local/share/ask-ai/embeddings.db
cp ~/.local/share/ask-ai/embeddings.db.backup ~/.local/share/ask-ai/embeddings.db 2>/dev/null || true
```

---

## Results

**Date:** _______  
**Model used:** _______  
**Status:** [ ] Approved for merge  

**Issues found:**

_______________________________________

---

## Merge Checklist

- [ ] All tests above passed
- [ ] `cargo test --all-features` passed
- [ ] `cargo clippy --all-features -- -D warnings` passed
- [ ] Smoke test (SMOKE_TEST.md) passed
- [ ] Documentation reviewed (CHANGELOG updated)
- [ ] PR reviewed and approved