# Embedding Diagnostics & Model Geometry

> **Status:** documentação técnica viva, extraída de `IMPLEMENTATION.md` durante a
> consolidação do backlog (LUC-140 / `refactor/backlog-consolidation`). O conteúdo é
> **verbatim** da fonte; apenas os cabeçalhos foram normalizados (eram marcados como
> "PRIORITY N … COMPLETED", resíduo de tracker).

O subcomando `sprach diagnostics`, o cálculo de d_eff, e a geometria dos modelos de embedding.

## Embedding Diagnostics Subcommand

**Status:** ✅ COMPLETED (PR #181 merged)
**Issue:** #133
**Branch:** `feat/embedding-diagnostics`
**Depends on:** None
**Estimated effort:** 2-3 days

**Goal:** Add `sprach diagnostics` command that performs spectral analysis on stored embeddings, reporting d_eff, mean cosine distance (d̄), regime classification, and variance distribution. This is the W4.0 gateway card — foundational infrastructure for all subsequent W4 phases.

**Design Decisions:**

| Decision | Rationale |
|----------|-----------|
| Subcommand name `diagnostics` (alias `diag`) | Long form for clarity, short alias for convenience |
| Default: combine all 3 embedding sources | Matches issue spec; `--source` flag for granular analysis |
| Pure-Rust power iteration SVD (no new crate) | Avoids ~500KB binary increase + 15 deps; d_eff needs only top ~20 eigenvalues |
| `zerocopy::FromBytes` for BLOB deserialization | Already in Cargo.toml; consistent with write path using `IntoBytes::as_bytes()` |
| Output includes source breakdown in header | Even default (combined) mode shows "content: N, chunks: M, facts: K" |

**Implementation Phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | New module `src/diagnostics/` + DB read functions + CLI subcommand | ✅ |
| 2 | Spectral analysis: d_eff, d̄, eigenvalues, regime classification | ✅ |
| 3 | Terminal display formatting + warnings + tests | ✅ |

**Files to Create:**

| File | Content |
|------|---------|
| `src/diagnostics/mod.rs` | Module root, re-exports |
| `src/diagnostics/embeddings.rs` | Spectral analysis: d_eff, d̄, eigenvalues, regime |
| `src/diagnostics/display.rs` | Terminal output formatting |

**Files to Modify:**

| File | Change |
|------|--------|
| `src/translate/cli.rs` | Add `Diagnostics(DiagArgs)` to `Commands` enum + `DiagArgs` struct |
| `src/main.rs` | Add `mod diagnostics;`, `Commands::Diagnostics` handler, `handle_diag()` |
| `src/content/db.rs` | Add `get_all_content_embedding_vectors()`, `get_all_chunk_embedding_vectors()` |
| `src/facts/db.rs` | Add `get_all_fact_embedding_vectors()` |

**Output Format (default — all sources combined):**

```
Embedding Diagnostics — nomic-embed-text-v2-moe
══════════════════════════════════════════
Vectors: 23 (content: 18, chunks: 2, facts: 3)
Nominal dimensions: 256
d_eff (participation ratio): 7.0 (2.74%)
Mean cosine distance (d̄): 0.353
Min/max cosine distance: 0.073 / 0.592

Regime Analysis:
  θ=0.70 → SPREAD (d̄ >= θ' = 0.30)
  θ=0.75 → SPREAD (d̄ >= θ' = 0.25)
  θ=0.80 → SPREAD (d̄ >= θ' = 0.20)
  θ=0.85 → SPREAD (d̄ >= θ' = 0.15)

Variance Explained:
  50% → PC #3
  90% → PC #10
  95% → PC #12
  99% → PC #15

⚠️  d_eff/25 ≈ 1 — vector search has minimal discriminative power.
    BM25 is silently compensating. Consider RRF weight adjustment.
```

**Output Format (`--source content`):**

```
Embedding Diagnostics — nomic-embed-text-v2-moe [content]
═════════════════════════════════════════════════
Vectors: 18
...
```

**CLI Syntax:**

```bash
sprach diagnostics                            # All sources combined
sprach diagnostics --source content           # content_embeddings only
sprach diagnostics --source chunks            # chunk_embeddings_v2 only
sprach diagnostics --source facts             # fact_embeddings only
sprach diag                                   # Shortcut (alias)
```

**Algorithms (no external dependencies):**

1. **d_eff (Participation Ratio):** `d_eff = (Σλᵢ)² / Σλᵢ²` — covariance matrix trace + power iteration for eigenvalues
2. **d̄ (Mean Cosine Distance):** Gram matrix `G = X·X^T`, `d̄ = 1 - mean(Gᵢⱼ)` for i≠j
3. **Regime Classification:** SPREAD if `d̄ ≥ (1 - θ)`, TIGHT otherwise
4. **Variance Explained:** Cumulative eigenvalue sum / total

**Warnings:**

| Condition | Message |
|-----------|---------|
| N < 100 | `⚠ Corpus is small (N=X). d_eff estimates are unreliable (max d_eff from PCA is N-1).` |
| d_eff/25 < 2 | `⚠ d_eff/25 ≈ N — vector search has minimal discriminative power.` |
| N = 0 | `No embeddings found in database. Run a chat session first.` |

**New DB methods (BLOB → Vec\<f32\> deserialization):**

```rust
// src/content/db.rs
pub fn get_all_content_embedding_vectors(&self) -> Result<Vec<Vec<f32>>>
pub fn get_all_chunk_embedding_vectors(&self) -> Result<Vec<Vec<f32>>>

// src/facts/db.rs
pub fn get_all_fact_embedding_vectors(&self) -> Result<Vec<Vec<f32>>>
```

**Key insight:** The codebase currently NEVER reads embedding vectors back from vec0 tables — only KNN distances are queried. These new methods are the first to perform bulk SELECT + BLOB deserialization.

**Bug Fix: Schema Migration Forward-Reference (discovered during DB upgrade testing)**

When opening a database at schema version ≤ 8, the `init_connection()` function executes `SCHEMA_SQL` before running incremental migrations. `SCHEMA_SQL` contained `CREATE INDEX IF NOT EXISTS idx_facts_embedding ON facts(has_embedding)` — but the `has_embedding` column is only added by `migrate_v10_to_v11()`. For databases at v8, this index creation failed with `no such column: has_embedding`, preventing the database from opening at all. The error was logged at `log::debug!` (silenced by default) and surfaced as a generic "DATABASE INITIALIZATION FAILED" with no actionable detail.

**Fix (4 files):**
- `src/db/schema.rs` — Removed `idx_facts_embedding` from `SCHEMA_SQL` (already created in `migrate_v10_to_v11()`)
- `src/db/init.rs` — Changed `log::debug!` → `log::error!`; replaced tuple return with `DatabaseInitResult` struct that includes the original error message
- `src/chat/repl.rs` — Use `DatabaseInitResult.error_detail` instead of generic message
- `src/query/context.rs` — Adapted to `DatabaseInitResult`

**Verified:** Database at `user_version=8` now migrates successfully to `user_version=12` with all columns, indexes, and vec0 tables intact.

**Bug Fix: TUI Hang During Startup Embedding Recovery (discovered after migration fix)**

After the migration v11→v12 fix, databases upgrading from v8 now have all `has_embedding` flags reset to 0, causing the startup embedding recovery pipeline to process 1700+ items. The pipeline (`regenerate_all_embeddings`, `recover_missing_embeddings`, `recover_missing_fact_embeddings`, `verify_and_dedup_facts`) ran synchronously via `.await` before the event loop in `run_chat_repl_tui()`, freezing the TUI for minutes. The `⚙ 0/1` indicator appeared but the prompt was unreachable.

**Fix:** Moved the entire embedding recovery pipeline to `tokio::spawn` in `src/chat/repl_tui.rs`. The TUI event loop starts immediately and is fully interactive. Progress is reported via the existing `EmbeddingProgressTx` channel (e.g., `⚙ 12/1741`). The indicator clears automatically when `poll_embedding_progress()` receives `current >= total`. Removed dead `clear_embedding_progress()` method from `App`.

**Verified:** TUI is interactive from the first frame; `⚙ N/M` indicator updates in real time during background embedding generation.

**Bug Fix: Application Blocks on Exit During Embedding Flush**

`/quit` and Ctrl+D called `flush_pending_embeddings()` and `flush_pending_fact_embeddings()` synchronously before exiting. After schema migration v11→v12 resets all `has_embedding` flags, this could block for minutes. The startup recovery pipeline already handles missing embeddings on next boot, making the exit flush redundant.

**Fix:** Removed the synchronous embedding flush from both exit paths (`handle_eof` and `handle_quit`). Exit is now instantaneous. Removed dead code: `flush_pending_embeddings()`, `flush_pending_fact_embeddings()`, `recover_missing_embeddings_with_progress()`, `clear_embedding_progress()`.

**Verified:** `/quit` exits immediately; next boot recovers pending embeddings via the background pipeline.

**Bug Fix: Embedding Progress Indicator Shows `current > total`**

The `⚙ N/M` indicator in the TUI status bar could show `processed` exceeding `total` (e.g., `⚙ 1800/1743`). Root causes:

1. **`regenerate_all_embeddings`**: `total = items.len() + chunks.len()` was calculated once at startup. When an item was split into N chunks, the original `total` only counted the item as 1 unit of work, but each chunk was processed independently. Similarly, `embed_item_with_fallback` could trigger recursive fallback chunking, creating more work.
2. **`recover_missing_embeddings`**: `total_missing = items.len()` only counted items, ignoring pre-existing chunks. When items were split into chunks, the total didn't grow. Skipped items (empty content, already-chunked) didn't increment `processed`, causing `processed < total` at completion.
3. **`recover_missing_fact_embeddings` and `verify_and_dedup_facts`**: Did not report progress via the `EmbeddingProgressTx` channel, leaving the indicator stale or invisible during these phases.

**Fix:**
1. `total` is now a mutable value that grows dynamically when items are split into chunks (`total += num_chunks - 1`).
2. Each chunk within a multi-chunk item increments `processed` individually (no longer relying on `progress.position()` which only counts `.inc()` calls).
3. Skipped items now increment `processed` so it always converges to `total`.
4. All four recovery functions (`regenerate_all_embeddings`, `recover_missing_embeddings`, `recover_missing_fact_embeddings`, `verify_and_dedup_facts`) now report progress via the `EmbeddingProgressTx` channel.

**Verified:** TUI shows `⚙ 50/1807` → `⚙ 161/1875` → ... → `⚙ 9205/11442` → indicator cleared. `current ≤ total` invariant holds throughout.

**Bug Fix: ANSI Escape Codes Appearing as Literal Text in TUI Error Messages**

Error messages like `✗ ␛[31mError:␛[0m Internal Server Error (ref: ...)` displayed raw ANSI escape codes instead of being rendered as colors. The TUI already applies red styling via `Span::styled(line, error_style())`, but `format_tool_error()` generated ANSI codes (e.g., `\x1B[31m` for red) when `is_plain_mode()` was false. In TUI mode, these codes appeared as garbled text.

**Root cause:** Double coloring — ratatui applies `error_style()` (bold red), and `format_error_with_ansi()` also wraps the text with `\x1B[31m...\x1B[0m`. The ANSI codes are not interpreted by ratatui widgets and appear as literal characters.

**Fix (2 layers):**
1. **Layer 1:** `format_error_with_status()` now uses `format_error_plain()` when `is_tui_mode()` is true, since the TUI renderer handles styling via `Span::styled()`.
2. **Layer 2 (defense-in-depth):** `show_error()`, `CommandOutput::Error`, `LlmEvent::Error`, and all other `ChatMessage::error()` call sites in `ratatui_view.rs` strip ANSI codes via `strip_ansi_codes()`. This catches ANSI from any source, not just `format_tool_error()`.

**Tests:** Added `test_format_error_tui_mode_no_ansi` and `test_format_tool_error_no_ansi_in_tui` to verify TUI mode produces no ANSI codes.

**Related:** Issue #133

**Bug Fix: Empty Assistant Messages from Ctrl+C Cancellation (Issue #185)** — ✅ COMPLETED

Three interrelated bugs discovered via production database investigation (12 items with `has_embedding = 0`):

1. **Empty assistant messages from Ctrl+C:** When the user pressed Ctrl+C during LLM streaming, `chat_stream()` in `custom_coordinator.rs` broke the streaming loop but returned `Ok(ChatMessageResponse)` with `full_content = ""`. This propagated as success through `process_send_result()` → `add_assistant_message("")`, persisting empty assistant messages. These messages: have no semantic value, confuse the LLM with empty turns, and can never receive embeddings (permanently stuck at `has_embedding = 0`). Evidence: 5 empty assistant messages (id 123, 232, 239, 278, 326) in production DB.

2. **Short content infinite recovery loop:** Items with `content.len() < 10` or `content.trim().is_empty()` were skipped by recovery/regenerate code but left with `has_embedding = 0`. On every startup, recovery queries `WHERE has_embedding = 0`, found these items, skipped them, and left them as `has_embedding = 0` — forever. Evidence: 7 short user messages ("Vai." at 4 chars, "Prossiga." at 9 chars × 6). Fix: filter by content length in recovery/reindex SQL queries (`AND length(content) >= 10 AND content != ''`), extract `MIN_EMBED_CONTENT_LEN` constant.

3. **No cleanup command:** Added `/gc` command for on-demand database garbage collection (empty messages, orphan chunks, orphan embeddings). Not automatic — user decides when to clean.

4. **Fact embedding regeneration on every startup:** `verify_and_dedup_facts()` called `generate_fact_embedding()` for ALL active facts on every startup, making N Ollama API calls even when all facts already had embeddings in the vec0 table. This caused the "indexing N facts" progress message on every boot. Fix: Verification now reads existing embeddings from DB via `get_all_fact_embedding_vectors()` and only generates new embeddings for facts with missing vec0 rows.

5. **vec0 re-embedding UNIQUE constraint failure:** `update_fact_embedding()`, `update_content_item_embedding()`, and `update_content_chunk_embedding()` used bare `INSERT INTO` for vec0 tables. If called for an entity that already had an embedding, the INSERT would fail with `UNIQUE constraint failed` because vec0 virtual tables use the entity ID as PRIMARY KEY and do not support `INSERT OR REPLACE`. Fix: All three methods now use `DELETE + INSERT` pattern.

**Implementation phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Prevent empty assistant messages: `add_assistant_message()` validation + `process_send_result()` skip | ✅ COMPLETED |
| 2 | Filter by content length in recovery/reindex queries + `MIN_EMBED_CONTENT_LEN` constant | ✅ COMPLETED |
| 3 | `/gc` command: ChatCommand::Gc, parser, handler, DB method, help text | ✅ COMPLETED |
| 4 | Read existing fact embeddings from DB instead of regenerating on every startup | ✅ COMPLETED |
| 5 | `DELETE + INSERT` pattern for all vec0 embedding update methods | ✅ COMPLETED |
| 6 | Orphan embedding cleanup in `/gc` (content, chunk, and fact embeddings) | ✅ COMPLETED |

---

## Matryoshka-Capable Embedding Models (Ollama)

| Model | Full Dims | Matryoshka → 256? | Context | Size | MTEB | Recommendation |
|---|---|---|---|---|---|---|
| nomic-embed-text-v2-moe | 768 | ✅ (64-768) | 8192 | 957MB | ~62 | Current default, multilingual |
| nomic-embed-text (v1.5) | 768 | ✅ (64-768) | 8192 | 274MB | 62.39 | English-only, lighter |
| mxbai-embed-large | 1024 | ✅ (64-1024) | 512 | 700MB | 64.68 | Best retrieval, short context |
| qwen3-embedding (0.6B) | 4096 | ✅ (32-4096) | 8192 | ~400MB | ~60 | Instruction-aware |
| qwen3-embedding (8B) | 4096 | ✅ (32-4096) | 8192 | ~5GB Q4 | 70.58 | SOTA quality |
| snowflake-arctic-embed2 | 1024 | ✅ (256) | 8192 | 1.2GB | 55.98 | Multilingual |
| embeddinggemma | 768 | ✅ (128-768) | 8192 | ~300MB | good/size | Google, no special prefix |

---

## Matryoshka-Capable Embedding Models (llama.cpp / OpenAI-compatible)

These models work with llama.cpp server's `/v1/embeddings` endpoint which also supports the `dimensions` parameter:

| Provider | Model | Full Dims | Matryoshka? | Context | Notes |
|---|---|---|---|---|---|
| OpenAI | text-embedding-3-small | 1536 | ✅ (512) | 8191 | $0.02/M tokens |
| OpenAI | text-embedding-3-large | 3072 | ✅ (256-3072) | 8191 | $0.13/M tokens |
| Any HF GGUF | nomic-embed-text-v1.5-GGUF | 768 | ✅ | 8192 | Can load custom fine-tunes |
| Any HF GGUF | bge-m3-GGUF | 1024 | ✅ | 8192 | Multilingual, dense+sparse+ColBERT |
| Any HF GGUF | snowflake-arctic-embed-m-GGUF | 768/1024 | ✅ | 8192 | Size variants 22M-335M |

---
