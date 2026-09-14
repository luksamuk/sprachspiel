# Context Overflow & Multi-Tool Execution

> **Status:** documentação técnica viva, extraída de `IMPLEMENTATION.md` durante a
> consolidação do backlog (LUC-140 / `refactor/backlog-consolidation`). O conteúdo é
> **verbatim** da fonte; apenas os cabeçalhos foram normalizados (eram marcados como
> "PRIORITY N … COMPLETED", resíduo de tracker).

Como o subsistema de overflow de contexto decide quando avisar, compactar e truncar.

## Context Overflow During Multi-Tool Execution

**Status:** ✅ COMPLETED (v0.37.0)

**Goal:** Prevent context overflow when LLM calls multiple tools in sequence AND fix infinite compaction loop caused by oversized summaries.

**Problems:**

1. **Multi-Tool Overflow:** Auto-compaction only happens BEFORE the first message. When tools execute sequentially, results accumulate in history without token checks. Large tool outputs (file reads, command outputs) can overflow context during multi-tool chains.

2. **Compaction Loop (Critical Bug):** Compaction summaries had no size limit. With 368 messages being summarized, the LLM generated ~18,000 token summaries, causing immediate re-compaction in an infinite loop.

**Root Cause Analysis:**
- Trigger was too late (95%+ context usage)
- No buffer reserved before overflow
- Summary had no token limit, generating massive summaries
- Template was generic, not structured for context preservation

**Solution:** Three-layer protection with percentage-based thresholds:

**Layer 1: Percentage-Based Compaction Triggers**
```rust
// Scales with context window size (32K, 128K, 200K)
MODERATE_USAGE_PERCENT = 0.75  // Warning at 75% (8K remaining for 32K)
CRITICAL_USAGE_PERCENT = 0.88  // Auto-compact at 88% (4K remaining for 32K)
INTER_TOOL_USAGE_PERCENT = 0.94 // Inter-tool warning at 94% (2K remaining)
EMERGENCY_USAGE_PERCENT = 0.97  // Emergency truncation at 97% (1K remaining)

// Absolute minimums for small contexts:
PRE_TOOL_MIN = 2_000 tokens
COMPACTION_MIN = 1_000 tokens
INTER_TOOL_MIN = 512 tokens
EMERGENCY_MIN = 256 tokens
```

**Layer 2: Structured Summary with Hard Limit**
```rust
MAX_SUMMARY_TOKENS = 3_000 tokens
Template: Goal, Instructions, Progress, Discoveries, Relevant Files
Auto-truncate if LLM ignores limit
```

**Layer 3: Inter-Tool Protection** (from Phase 1 implementation)
```rust
MODERATE_USAGE = 75%  → Warning before first tool
CRITICAL_USAGE = 88%   → Auto-compact threshold
INTER_TOOL_USAGE = 94% → Warning during tool execution
EMERGENCY_USAGE = 97%  → Truncate result as last resort
```

**Critical Token Calculation Bugs Fixed (v0.37.0):**

Three separate double-counting bugs were discovered and fixed:

1. **`calculate_context_metrics()` double-counted system + tools**
   - Comments said `real_history_tokens` was "history only"
   - But it's actually the TOTAL from Ollama's `prompt_eval_count`
   - Function was adding system + tools again, causing double-count
   - Fix: Use total directly, derive history by subtraction

2. **`needs_inter_tool_compaction()` and related functions**
   - Received `history_tokens + system_tokens` and summed them again
   - Fix: Accept single `total_tokens` parameter

3. **Pre-tool warning showed wrong remaining tokens**
   - Used `context_window - history_real_tokens()` 
   - Missing system + tools in remaining calculation
   - Fix: Use `total_tokens` from `ContextStatus`

4. **Pre-tool warning said "Auto-compacting..." but didn't compact**
   - Logic showed warning at 75%, called `auto_compact_if_needed()`
   - But `auto_compact_if_needed()` only compacts at 88%
   - Fix: Split logic - warning at 75%, compact at 88%

**Implementation:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Add inter-tool context check (80% threshold) | ✅ Done |
| 2 | Add emergency truncation (90% threshold) | ✅ Done |
| 3 | Add `needs_inter_tool_compaction()` function | ✅ Done |
| 4 | Add `truncate_to_budget()` for emergency truncation | ✅ Done |
| 5 | Add `ContextNearLimit` and `ContextTruncated` events | ✅ Done |
| 6 | Add tests for new functions | ✅ Done |
| 7 | Add percentage-based thresholds | ✅ Done |
| 8 | Restructure `COMPACTION_PROMPT` with structured template | ✅ Done |
| 9 | Add summary truncation in `compact_conversation()` | ✅ Done |
| 10 | Update `auto_compact_if_needed()` to use percentage thresholds | ✅ Done |
| 11 | Add `needs_buffered_compaction()` function | ✅ Done |
| 12 | Fix `calculate_context_metrics()` double-counting | ✅ Done |
| 13 | Fix `needs_inter_tool_compaction()` signature | ✅ Done |
| 14 | Fix pre-tool warning remaining calculation | ✅ Done |
| 15 | Split warning vs compact logic in continuation.rs | ✅ Done |
| 16 | Remove duplicate warning in core.rs | ✅ Done |

**Files Modified:**
- `src/context_overflow.rs` - Percentage thresholds, `calculate_thresholds()`, fixed function signatures
- `src/tokens.rs` - Fixed `calculate_context_metrics()` to not double-count
- `src/chat/continuation.rs` - Split warning/compact logic, fixed remaining calculation
- `src/chat/core.rs` - Removed duplicate warning when tools enabled
- `src/chat/coordinator.rs` - Updated function calls for new signatures
- `src/prompts/base.rs` - Restructured `COMPACTION_PROMPT` with structured template
- `src/utils.rs` - Added `truncate_to_budget()` for emergency truncation
- `tests/context_tool_overflow.rs` - Updated for percentage-based thresholds
- `tests/context_recovery_flow.rs` - Updated for percentage-based thresholds

**Constants:**
- `MODERATE_USAGE_PERCENT = 0.75` - Warning threshold (75%)
- `CRITICAL_USAGE_PERCENT = 0.88` - Auto-compact threshold (88%)
- `INTER_TOOL_USAGE_PERCENT = 0.94` - Inter-tool warning (94%)
- `EMERGENCY_USAGE_PERCENT = 0.97` - Emergency truncation (97%)
- `PRE_TOOL_MIN = 2_000` - Minimum buffer for warning
- `COMPACTION_MIN = 1_000` - Minimum buffer for compaction
- `INTER_TOOL_MIN = 512` - Minimum buffer for inter-tool
- `EMERGENCY_MIN = 256` - Minimum buffer for emergency
- `RESPONSE_MARGIN = 2_000` - Tokens reserved for model response
- `MAX_SUMMARY_TOKENS = 3_000` - Hard limit on summary size
- `DEFAULT_OVERFLOW_THRESHOLD = 0.75` - For display purposes

**New Compaction Template:**
```markdown
## Goal
[1-2 sentences: What is the user trying to accomplish?]

## Instructions
- [Important user constraints and preferences, max 3 items]

## Progress
**Completed:** [Work done, max 5 items]
**Pending:** [Work remaining, max 3 items]

## Discoveries
[Key insights learned, max 3 items]

## Relevant Files
- [Files read/edited/concerned, max 5 items]
- Root path: [Project root if relevant]
```

**Flow Implemented:**
1. Before first message: Check if context needs compaction (trigger at `context - COMPACTION_BUFFER`)
2. Between tools: Check if context > 80% → emit `ContextNearLimit` event
3. Emergency: If context > 90% → truncate result → emit `ContextTruncated` event
4. After compaction: Generate summary with template, truncate if > MAX_SUMMARY_TOKENS

**Research Sources:**
- OpenCode compaction.ts: `COMPACTION_BUFFER = 20,000`, structured template
- LangChain: Token-based triggers, summary best practices
- sprachspiel context: Zettelkasten and learning focus (smaller buffer than code agents)

**Note for Future:** When implementing parallel tool execution, the nudge mechanism via `continuation_prompt` should be reviewed to handle multiple concurrent tool completions.

**v0.37.0 Addition - Inter-Tool Compaction:**

Automatic context compaction during multi-tool execution (implemented in PR #45):

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Add `ChatEvent::ContextNeedsCompaction` | ✅ Done |
| 2 | Add `needs_compaction` flag to `ContextCheckResult` | ✅ Done |
| 3 | Modify `process_response()` to stop tool execution on compaction needed | ✅ Done |
| 4 | Add `OverflowHandleResult` enum for error classification | ✅ Done |
| 5 | Add automatic continuation loop in `handle_user_message()` | ✅ Done |
| 6 | Add MAX_COMPACTION_CYCLES limit (3) | ✅ Done |

**New Files/Functions:**
- `src/chat/coordinator.rs`: Added `ChatEvent::ContextNeedsCompaction`, error string format with `CONTEXT_NEEDS_COMPACT:` prefix
- `src/chat/continuation.rs`: Added `OverflowHandleResult`, `is_inter_tool_compaction_error()`, `parse_inter_tool_compaction_error()`, `handle_inter_tool_compaction_error()`, `build_inter_tool_compaction_prompt()`
- `src/prompts/base.rs`: Added `CONTINUATION_PROMPT_INTER_TOOL` for continuation after compaction
- `src/chat/repl.rs`: Added `MAX_COMPACTION_CYCLES` constant (module level)

**Flow:**
1. During multi-tool execution, check if `remaining < COMPACTION_BUFFER` after each tool
2. If true, emit `ContextNeedsCompaction` event and return error string with `CONTEXT_NEEDS_COMPACT:` prefix
3. `handle_overflow_error()` detects the error, returns `OverflowHandleResult::InterToolCompaction`
4. `handle_user_message()` detects `InterToolCompaction`, compacts, sends continuation prompt
5. LLM continues automatically (max 3 compaction cycles per message)

**Refactoring (v0.37.0):**
- Removed unused `CoordinatorError` enum (never used)
- Removed unused `CompactionStats` struct and `compaction_stats()` method
- Removed unused `_threshold` parameter from `check_context_overflow()`
- Removed unused `_system_prompt` and `_use_debug` parameters from `auto_compact_if_needed()`
- Simplified `check_and_handle_context_overflow()` signature (removed `_tool_name`)
- Moved `MAX_COMPACTION_CYCLES` to module level in `repl.rs`

**Related:** Issue #43

---

---

# Compaction Internals — constants, functions and error recovery

> Extraído de `IMPLEMENTATION.md` (seção "Bug Fix: Compaction Overflow — #187") durante a
> consolidação de backlog (LUC-140). Conteúdo **verbatim**, com os nomes/valores das
> constantes de prune corrigidos para os atuais (`…_TOKENS`, não `…_CHARS` — a unidade
> mudou de caracteres para tokens estimados). Os demais valores foram verificados contra
> `src/context_overflow.rs` e conferem.

## Bug Fix: Compaction Overflow (Issue #187) — ✅ COMPLETED

When a conversation's context exceeds the model's context window, `/compact` fails with `"The prompt is too long"` because the compaction prompt itself exceeds the window. This is a chicken-and-egg problem: you need compaction to reduce context, but compaction requires sending the full context to the model.

**Root Cause:** `compact_conversation()` in `src/chat/core.rs` constructs a single prompt with ALL middle messages and sends it to the model. If the middle section exceeds the model's context window, the API call fails. There is NO handling for this case.

**Research:** Claude Code implements a 5-tier cascade (microcompact → snip → context collapse → auto compact → reactive compact with PTL retry). OpenCode has aggressive pre-pruning of tool outputs before summarization. Academic literature (arXiv:2308.15022) validates recursive summarization for long dialogue memory.

**3-Layer Strategy:**

| Layer | Name | Mechanism | Status |
|-------|------|-----------|--------|
| 1 | Pre-pruning | Strip long tool outputs (>200 estimated tokens) before sending to LLM | ✅ COMPLETED |
| 2 | Chunked recursive summarization | Split into chunks, summarize each, combine summaries, recurse if needed | ✅ COMPLETED |
| 3 | Fallback truncation | Drop oldest middle messages until prompt fits in 50% of window | ✅ COMPLETED |

**Implementation phases:**

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Pre-pruning: `pre_prune_messages()` truncates tool outputs > `PRUNE_TOOL_RESULT_THRESHOLD` | ✅ COMPLETED |
| 2 | Fallback truncation: `fallback_truncate()` drops oldest middle messages to fit window | ✅ COMPLETED |
| 3 | Integration: `compact_conversation()` uses Layer 1 then Layer 2 before LLM call | ✅ COMPLETED |
| 4 | Chunked recursive summarization: `split_into_chunks()` + `compact_recursive()` | ✅ COMPLETED |
| 5 | Tests for all 3 layers | ✅ COMPLETED (14 new tests) |
| 6 | Documentation (CHANGELOG, IMPLEMENTATION, context-anatomy.md, architecture.md, ADR) | ✅ COMPLETED |

**New constants (`src/context_overflow.rs`):**

| Constant | Value | Purpose |
|----------|-------|---------|
| `PRUNE_TOOL_RESULT_THRESHOLD_TOKENS` | 200 | Min **estimated tokens** before truncating tool outputs (was 500 chars — `chars != tokens`) |
| `PRUNE_TOOL_RESULT_KEEP_TOKENS` | 40 | **Estimated tokens** to keep from a truncated tool output (was 100 chars) |
| `COMPACTION_MAX_CONTEXT_RATIO` | 0.60 | Max ratio of context window per chunk |
| `MAX_RECURSION_DEPTH` | 3 | Max recursion levels for chunked summarization |
| `TRUNCATION_TARGET_RATIO` | 0.50 | Target ratio after fallback truncation |
| `COMPACTION_PROMPT_OVERHEAD` | 3000 | Reserved tokens for prompt + response (was 2500, increased for accuracy) |
| `COMPACT_MSG_OVERHEAD` | 10 | Per-message overhead in compaction (vs. `MESSAGE_OVERHEAD=4` elsewhere) |
| `ESTIMATION_SAFETY_MARGIN` | 1.20 | 20% buffer on token estimates to compensate for underestimation |

**New functions (`src/context_overflow.rs`):**

| Function | Purpose |
|----------|---------|
| `pre_prune_messages()` | Strip long tool outputs, keep first 40 estimated tokens + notice |
| `estimate_messages_tokens()` | Token estimation for SavedMessage list (standard, used for thresholds) |
| `estimate_compaction_tokens()` | Token estimation for compaction with 20% safety margin and `COMPACT_MSG_OVERHEAD` |
| `fits_in_context()` | Check if messages fit within context window (uses `estimate_compaction_tokens`) |
| `max_chunk_tokens()` | Calculate chunk size for recursive summarization |
| `split_into_chunks()` | Split messages into token-bounded chunks with overlap |
| `fallback_truncate()` | Drop oldest middle messages to fit context window |
| `is_prompt_too_long_error()` | Detect Ollama overflow errors for error-retry |

**New functions (`src/chat/core.rs`):**

| Function | Purpose |
|----------|---------|
| `compact_recursive()` | Recursively summarize chunks using `Box::pin` for async recursion |
| `build_conversation_text()` | Format messages into conversation text (extracted from `compact_conversation`) |
| `compact_with_llm()` | Send compaction prompt to LLM and return summary (extracted from `compact_conversation`) |

**Defense in depth (error-retry):**

| Layer | Error Recovery | Behavior |
|-------|---------------|----------|
| 1 (single-pass) | `is_prompt_too_long_error()` | Catches "prompt too long" from Ollama, falls through to Layer 2 |
| 2 (chunked) | Already catches all errors | Falls through to Layer 3 |
| 3 (truncation) | `is_prompt_too_long_error()` | Catches "prompt too long", returns detailed diagnostics |

**Bug Fix: Estimation Undercount & Error Recovery (post-PR#188)**

**Problem:** `fits_in_context()` used `estimate_tokens()` (words/0.75 heuristic) with `MESSAGE_OVERHEAD=4` per message and `COMPACTION_PROMPT_OVERHEAD=2500`. This underestimated real token counts by 15-40% for mixed-content conversations (code, Portuguese text, tool JSON). When the estimate said "fits" but the LLM rejected the prompt as "too long", compaction failed with no recovery — Layer 2 and 3 were never reached because the decision was made *before* the LLM call.

**Fix:**
1. `is_prompt_too_long_error()` detects overflow errors from Ollama
2. Layer 1 (single-pass) catches overflow errors and falls through to Layer 2
3. Layer 3 (truncation) catches overflow errors and returns actionable diagnostics
4. `ESTIMATION_SAFETY_MARGIN = 1.20` — 20% buffer on token estimates
5. `COMPACT_MSG_OVERHEAD = 10` — realistic per-message overhead for compaction
6. `COMPACTION_PROMPT_OVERHEAD = 3000` — accounts for all prompt components

| What changed | Old | New |
|--------------|-----|-----|
| `fits_in_context()` | Used `estimate_messages_tokens()` (MESSAGE_OVERHEAD=4) | Uses `estimate_compaction_tokens()` (COMPACT_MSG_OVERHEAD=10, ×1.20 safety) |
| `COMPACTION_PROMPT_OVERHEAD` | 2500 (local const in core.rs) | 3000 (public const in context_overflow.rs) |
| `compact_conversation()` | Layer 1 returns `Err` on LLM overflow | Layer 1 catches overflow, falls through to Layer 2 |
| Layer 3 | Returns raw error on LLM overflow | Catches overflow, returns detailed diagnostics |

**Affected Code:**

| File | Change |
|------|--------|
| `src/context_overflow.rs` | Added `COMPACT_MSG_OVERHEAD`, `COMPACTION_PROMPT_OVERHEAD` (public, was local), `ESTIMATION_SAFETY_MARGIN`, `estimate_compaction_tokens()`, `is_prompt_too_long_error()`. Modified `fits_in_context()` to use `estimate_compaction_tokens()`. Added 11 new tests. |
| `src/chat/core.rs` | Layer 1 error-retry: catches "prompt too long" and falls through to Layer 2. Layer 3 error-retry: catches "prompt too long" and returns detailed diagnostics. Removed 2 local `COMPACTION_PROMPT_OVERHEAD` constants (replaced by public const). Uses `estimate_compaction_tokens()` instead of `estimate_messages_tokens()`. |
| `doc/src/CHANGELOG.md` | Added defense-in-depth and estimation fix details to #187 entry |

2026-05-30 - Bug Fixes: Context Prompt Corrections (PR #188). User message duplication, /retry wrong message, continuation empty message, system prompt clarity improvements.

 ## Bug Fixes: Context Prompt Corrections (PR #188) — ✅ COMPLETED

Six fixes targeting LLM prompt construction bugs and system prompt clarity issues identified during compaction analysis.
