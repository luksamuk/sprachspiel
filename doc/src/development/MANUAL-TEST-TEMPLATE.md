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
cp ~/.local/share/sprachspiel/sprachspiel.db ~/.local/share/sprachspiel/sprachspiel.db.backup 2>/dev/null || true
rm -f ~/.local/share/sprachspiel/sprachspiel.db
```

> **⚠️ Clean database:** Many tests require a clean database to avoid interference from previous test data. Sections requiring this are marked with 🗑️. Reset with:
> ```bash
> rm -f ~/.local/share/sprachspiel/sprachspiel.db
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
sqlite3 ~/.local/share/sprachspiel/sprachspiel.db "PRAGMA user_version;"
# Expected: 12 or higher (v12 adds distance_metric=cosine to vec0 tables)
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

## Cleanup

```bash
# Remove any test data from database (if needed)
# /doc list to see IDs
# /doc delete N for each test document

# Restore original database
rm -f ~/.local/share/sprachspiel/sprachspiel.db
cp ~/.local/share/sprachspiel/sprachspiel.db.backup ~/.local/share/sprachspiel/sprachspiel.db 2>/dev/null || true
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