# Manual Test: Feedback Infrastructure

**Issue:** #23 - Feedback infrastructure
**Branch:** feat/feedback-infrastructure
**PR:** #98

---

## Prerequisites

```bash
cd /path/to/ask-ai-rs
cargo build --release --features all-tools
ollama serve  # In another terminal
```

## Test Model

```bash
# Use environment variable or default
MODEL=${SMOKE_MODEL:-qwen3.5:4b}
ollama list | grep -q "$MODEL" || ollama pull "$MODEL"
echo "Test model: $MODEL"
```

---

## LLM Tool Refusal Policy

Some tests require the LLM to call tools. If the model **refuses** to call a tool:

1. **Retry with an explicit instruction** — rephrase the prompt to make the tool call unavoidable (e.g., "You MUST call the tool right now, do not just describe it").
2. **If the model still refuses persistently** — **FAIL the test** and note which model refused which tool.
3. **Switch to an abliterated model** — check `models.toml` for an abliterated variant (e.g., `qwen3.5:4b-abliterated`). If no abliterated model is listed, request one from the user before retrying.

**Rationale:** Abliterated models have safety refusals removed, ensuring deterministic tool calling during tests. A model that refuses to call tools is a valid test failure, not a bug in the application.

---

## 1. Feedback Commands - Happy Path

**Objective:** Verify all feedback command variants produce correct output.

In chat with LLM (model with tools support):

- [ ] `/feedback good` → `↑↑ good feedback recorded for msg:N` + excerpt (dim) + `Importance: +0.05`
- [ ] `/feedback bad` → `↓↓ bad feedback recorded for msg:N` + excerpt (dim) + `Importance: -0.10`
- [ ] `/feedback correction:The correct answer is X` → `✎ correction feedback recorded for msg:N` + excerpt (dim) + `Correction: The correct answer is X`
- [ ] `/feedback msg:42 good` → targets specific message by ID
- [ ] `/feedback msg:42 correction:The capital is Canberra` → correction on specific message
- [ ] Verify feedback persists in database:
  ```bash
  sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT * FROM feedback_signals ORDER BY id DESC LIMIT 5;"
  ```

**Cleanup:**
```bash
# Optionally clear test feedback signals
sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM feedback_signals;"
```

---

## 2. Feedback Commands - Shortcuts

**Objective:** Verify shortcut commands work identically to full commands.

- [ ] `/fb good` → same output as `/feedback good`
- [ ] `/fb bad` → same output as `/feedback bad`
- [ ] `/fb correction:Typo` → same output as `/feedback correction:Typo`
- [ ] `/fg` → same output as `/feedback good`

---

## 3. Feedback Commands - Error Cases

**Objective:** Verify error handling for invalid inputs.

- [ ] `/feedback` (no subcommand) → `Usage: /feedback <good|bad|correction:text> [msg:id]`
- [ ] `/feedback msg:abc good` → `Invalid message ID 'abc'. Use msg:<number> (e.g., msg:42).`
- [ ] `/feedback correction:` (empty correction) → `Correction requires text. Usage: /feedback correction:<text>`
- [ ] `/feedback msg:5 correction:` → `Correction requires text. Usage: /feedback msg:<id> correction:<text>`
- [ ] `/feedback good` in anonymous mode (`ask-ai chat --anonymous`) → `Error: Cannot give feedback in anonymous mode.`
- [ ] `/feedback good` before any assistant message → `No assistant message to give feedback on.`
- [ ] `/fb` (empty) → error message (should show usage)

---

## 4. Content Prune

**Objective:** Verify content decay pruning works correctly.

- [ ] `/content prune` → shows `⏳ Running content decay cycle...` then result
- [ ] With items pruned: `✓ Pruned N content item(s), N remaining (avg retention: X.XX).`
- [ ] With no items pruned: `✓ No content to prune. N item(s) remaining (avg retention: X.XX).`
- [ ] `/cp` → same as `/content prune` (shortcut)
- [ ] `/content prune` in anonymous mode → `Error: Cannot prune content in anonymous mode.`
- [ ] Verify pruned items:
  ```bash
  sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT id, pruned FROM content_items WHERE pruned > 0 LIMIT 5;"
  ```
- [ ] High-importance items (importance >= 0.8) should NOT be pruned

**Cleanup:**
```bash
# Optionally reset pruned items for re-testing
sqlite3 ~/.local/share/ask-ai/embeddings.db "UPDATE content_items SET pruned = 0 WHERE pruned > 0;"
```

---

## 5. Context Decay Stats

**Objective:** Verify /context shows Content Memory section with decay statistics.

- [ ] `/context` → shows `Content Memory:` section with:
  - `Total items: N`
  - `Avg importance: X.XX`
  - `⚠ Items at risk: N (low decay score)` (only when items_at_risk > 0)
  - `Feedback signals: N`
- [ ] After `/content prune`, `/context` shows updated stats
- [ ] Verify numbers match database:
  ```bash
  sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT COUNT(*) FROM content_items WHERE pruned = 0;"
  sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT AVG(importance) FROM content_items WHERE pruned = 0;"
  sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT COUNT(*) FROM feedback_signals;"
  ```

---

## 6. feedback_submit LLM Tool

**Objective:** Verify the LLM can submit feedback via the feedback_submit tool.

> **LLM Refusal:** If the model refuses to call the tool, rephrase explicitly.
> If refusal persists, switch to an abliterated model (see LLM Tool Refusal Policy above).

- [ ] Ask LLM: "Give me positive feedback on your last response" → LLM calls `feedback_submit` with `good`
- [ ] Verify response includes: `Feedback submitted: good signal for item N (weight: 30%)`
- [ ] Ask LLM: "Give me negative feedback on that response" → LLM calls `feedback_submit` with `bad`
- [ ] Ask LLM: "Correct your previous answer: the capital is Canberra" → LLM calls `feedback_submit` with `correction`
- [ ] Error: `feedback_submit("0", "good", None)` → `Error: Invalid item_id '0'. Must be a positive integer.`
- [ ] Error: `feedback_submit("42", "invalid", None)` → `Error: Unknown feedback signal type 'invalid'. Use 'good', 'bad', or 'correction'.`
- [ ] Error: `feedback_submit("42", "correction", None)` → `Error: correction_text is required when signal_type is 'correction'.`
- [ ] Verify `source = 'llm'` in database:
  ```bash
  sqlite3 ~/.local/share/ask-ai/embeddings.db "SELECT * FROM feedback_signals WHERE source = 'llm';"
  ```

**Cleanup:**
```bash
# Optionally clear LLM-originated feedback signals
sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM feedback_signals WHERE source = 'llm';"
```

---

## 7. [feedback] Configuration Verification

**Objective:** Verify all feedback configuration defaults.

- [ ] Default values when `[feedback]` section is omitted from config.toml:

  | Setting | Default |
  |---------|----------|
  | `enabled` | `true` |
  | `implicit_capture` | `true` |
  | `llm_feedback_weight` | `0.3` |
  | `decay_half_life_good` | `30.0` |
  | `decay_half_life_bad` | `7.0` |
  | `decay_half_life_correction` | `14.0` |
  | `content_decay` | `true` |
  | `access_reinforcement` | `true` |
  | `access_reinforcement_boost` | `0.001` |
  | `content_prune_threshold` | `0.05` |

- [ ] Test adding custom `[feedback]` section to config.toml and verifying overrides work
- [ ] Remove `[feedback]` section and verify defaults return

**Cleanup:**
```bash
# Restore original config.toml if modified
git checkout ~/.config/ask-ai/config.toml 2>/dev/null || true
```

---

## 8. Schema v10 Verification

**Objective:** Verify database schema migration succeeded.

- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA user_version;"` → ≥ 10
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db ".tables"` → includes `feedback_signals`
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA table_info(feedback_signals);"` → has columns: id, item_id, session_id, signal_type, base_value, correction_text, source, created_at
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db "PRAGMA table_info(content_items);"` → has `pruned` column (INTEGER NOT NULL DEFAULT 0)
- [ ] `sqlite3 ~/.local/share/ask-ai/embeddings.db ".indexes"` → includes idx_feedback_signals_* indexes

---

## Cleanup

```bash
# Remove test feedback signals if desired
sqlite3 ~/.local/share/ask-ai/embeddings.db "DELETE FROM feedback_signals;"

# Remove test documents if desired
# /doc list to see IDs
# /doc delete N for each test document
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
- [ ] `cargo test --lib` passed
- [ ] `cargo clippy -- -D warnings` passed
- [ ] Smoke test (SMOKE_TEST.md) passed
- [ ] Documentation reviewed (CHANGELOG updated)
- [ ] PR reviewed and approved
