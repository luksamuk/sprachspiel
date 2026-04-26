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

**Objective:** Verify that `/fact add` CLI command uses the same 5-layer dedup as `fact_add` LLM tool.

🗄️ Clean database required.

- [ ] `/fact add I prefer dark mode` → stored as "User prefers dark mode" (ADR-E4 normalization)
- [ ] Wait 3 seconds for embedding generation
- [ ] `/fact add I prefer dark mode` → **Skipped: Exact duplicate** (Layer 1)
- [ ] `/fact add User prefers dark mode` → **Skipped: Similar fact** (Layer 2)
- [ ] `/fact add I like dark mode` → Layer 3.5 catches as paraphrase or FTS5 as similar
- [ ] Clean database again: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
- [ ] `/fact add I prefer dark mode` → stored as "User prefers dark mode"
- [ ] Wait 3 seconds for embedding
- [ ] `/fact add I prefer light mode` → should **UPDATE** (Layer 3.5 contradiction)
- [ ] Verify embedding: `sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT content, has_embedding FROM facts WHERE content LIKE '%light mode%'"` → **has_embedding = 1**

---

## Fact System: Layer 3.5 via Auto-Extraction with `/tools` Toggle (Bug #4 smoke test #2)

**Objective:** Verify that Layer 3.5 contradiction detection works through auto-extraction when LLM tool calls are disabled.

🗄️ Clean database required.

> **Why `/tools`:** Some models proactively call `fact_add` to resolve contradictions, bypassing auto-extraction. The `/tools` command disables LLM tool calls, forcing contradiction detection through the auto-extraction path.

1. Clean state: `sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM facts WHERE invalidated_at IS NULL; DELETE FROM fact_embeddings;"`
2. Start chat: `ask-ai chat`
3. `/tools` → should print **"Tools: disabled"**
4. Send: "I prefer dark mode" → auto-extraction stores via normalization + embedding
5. Wait 5 seconds for embedding
6. Send: "Actually, I prefer light mode" → auto-extraction detects contradiction (Layer 3.5) and updates
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