# Changelog

All notable changes to Sprachspiel will be documented in this file.

## [Unreleased]

### Added

- **TTFB Watchdog for SSE Streaming (Issue #123)** — New `ttfb_timeout_secs` field on `ProviderConfig` (default: 120s) implementing a time-to-first-byte watchdog in `parse_sse_stream`. If no SSE chunk arrives within the TTFB window, a `ProviderError::Timeout("SSE TTFB timeout after {ttfb}s")` is emitted and the stream aborts, instead of waiting the full `idle_timeout` (300s). After the first byte, the existing `idle_timeout` applies for inter-chunk gaps. Inspired by Hermes Agent's `HERMES_CODEX_TTFB_TIMEOUT_SECONDS=120`. Complementary to Issue #209 (which tracks per-generation slowness, not startup delay).

- **Retry Loop Migration to ProviderError (Issue #123)** — The retry loops in `src/chat/core.rs` (2 sites) and `src/query/executor.rs` (1 site) now use `ProviderError::retry_category()` directly instead of converting to `OllamaError` and calling `classify_for_retry`. This eliminates the "KNOWN BUG" where `ProviderError::Timeout` and `Connection` were converted to `OllamaError::Other` (mapped to `NoRetry`), breaking the ReAct loop instead of retrying. Timeout and connection errors now correctly map to `NetworkRetry { max_attempts: 5 }` with exponential backoff (100ms→1.6s). The `convert_provider_error` bridge in `ollama_shim.rs` and the string-sniffing workaround (`error_str.contains("Timeout:")`) in `custom_coordinator.rs` are eliminated.

- **Relocated retry.rs to provider/retry.rs (Issue #123)** — The `RetryCategory` enum, `retry_delay`, and `sleep_or_cancel` functions are relocated from `src/retry.rs` to `src/provider/retry.rs` as part of the W2 closure. The old `src/retry.rs` is removed. `classify_for_retry(&OllamaError)` is removed in favor of `ProviderError::retry_category()` (already defined in `provider/types.rs`).

### Removed

- **ollama-rs Dependency (Issue #123)** — The `ollama-rs` crate (v0.3.4) has been removed from `Cargo.toml`. All LLM communication now goes through the agnostic `LlmProvider` trait and `OpenAICompatibleProvider` (HTTP `/v1/chat/completions`). The 730-line compatibility shim `src/provider/ollama_shim.rs` is deleted. All `use ollama_rs` statements (17 production files + 2 test files) are migrated to `crate::provider::types` equivalents (`LlmMessage`, `LlmResponse`, `ProviderError`, `ProviderOptions`, `LlmToolCall`, `LlmRole`). This completes the W2 Provider Chain (#116 → #123). Binary size reduced.

- **W2 dead_code exemptions (Issue #123)** — All 19 `#[allow(dead_code)]` annotations introduced during W2 (#116–#122) are resolved. The W2 policy relaxation that permitted forward-referencing dead code is expired. Module-level `#![allow(dead_code)]` on `provider/mod.rs`, `provider/factory.rs`, `provider/types.rs`, and `provider/ollama_shim.rs` are removed (the shim is deleted entirely). Item-level annotations on `RetryCategory::RateLimitRetry`, `ProviderError::RateLimit::retry_after`, `LlmResponse`, `ProviderCapabilities`, `ToolInfo`, `ToolType`, `ToolFunctionInfo`, `retry_delay`, `detect_capabilities`, `provider_name`, and `is_available` are removed — all are now consumed by production code.

- **Stale W2 narrative comments (Issue #123)** — All "W2 Wave Context" comment blocks (15+ files) are removed from source code. They narrated the migration as in-progress and belong in git history, not the source. All 5 `TODO #123` comments are resolved. Portuguese comments in `src/chat/model_switch.rs:19-24` are translated to English.

- **Feature-matrix clippy warnings (Issue #123)** — Fixed 4 root causes of clippy warnings in individual feature combinations: (1) `DocumentEntry`/`DocumentListData` imports in `command_handlers.rs` moved inside `#[cfg(feature = "document-tools")]` blocks; (2) `fn mermaid_style` in `tui/markdown.rs` gated with `#[cfg(feature = "mermaid")]`; (3) `lang`/`render_mermaid`/`render_special` variables renamed to `_lang`/`_render_*` in outer scopes where only consumed by gated blocks.

### Changed

- **Coordinator Rename and Type Migration (Issue #123)** — `CustomCoordinator` renamed to `Coordinator` (no longer "custom" — the ollama-rs `Coordinator` it was custom-impl-ing is gone). The generic `C: ChatHistory` bound is removed; `Coordinator` uses `Vec<LlmMessage>` directly. `SubagentRunner` and `EmbeddingClient` now use `OpenAICompatibleProvider` directly instead of `Box<dyn LlmProvider>`. `OcrError::OllamaError` renamed to `OcrError::ProviderError`. All `ollama` parameter/variable/field names renamed to `provider` across 25+ files. `get_ollama()`/`get_llm()` renamed to `get_provider()` in `tools/context.rs`. `try_format_ollama_error` renamed to `try_format_provider_error` in `tool_robustness.rs`.

- **Coordinator error types (Issue #123)** — `Coordinator` methods (`chat`, `stream_turn`, `process_response`, `process_next_stream`) now return `Result<LlmResponse, ProviderError>` instead of `ollama_rs::error::Result<ChatMessageResponse>`. `classify_ollama_error` is rewritten as `classify_provider_error` accepting `&ProviderError`. `RecoverableError::OllamaError` variant is renamed to `RecoverableError::ProviderError`.

- **Indexing Configuration Redesign (Issue #121 W2 extension)** — Restructures the embedding/indexing schema based on user feedback: the `[embedding]` section is renamed to `[indexing]` and the old `[retrieval]` section is **merged** into it (indexing and retrieval are two sides of the same concern). The embedding capability moves from the provider (`[provider.X].embedding = true`) to the **model** (`[models.X].embeddings = true` + `dimensions = N`). The `[indexing].model` field is now an **alias** from `models.toml` (not the upstream `model_id`); the provider is inferred from the alias's `provider` field. New `Settings::resolve_indexing_model()` returns `(UserModelConfig, ProviderConfig, model_id, dimensions)`. New `db::IndexingInit` struct replaces `db::EmbeddingInit` and adds a `dimensions` field. `EmbeddingClient::with_model(ollama, model_id, dimensions)` now takes 3 args. `probe_embedding()` is **adaptive**: it does NOT pass `dimensions` in the request body (some providers reject it); the response's vector dim count is compared against the alias's declared `dimensions` for **strict verify** (mismatch → fatal error with 4-cause diagnostic). `run_indexing_probe` replaces `run_embedding_probe`. New rule: **embedding-only models cannot be selected for chat** — `-m <alias>` and `/model <alias>` reject aliases with `embeddings = true` with a clear error pointing the user to `[indexing].model`. The completer in the TUI uses `list_chat_model_names()` (filters out embedding-only); `sprach --list` still shows all models with a new `[embeddings-only]` tag. New `sprach models upgrade` migration `MissingDimensions { alias }` warns when a model has `embeddings = true` but no `dimensions` (warning-only, no auto-add). The previous `MissingEmbeddingFlag` (provider-level) is removed. `[retrieval]` section is removed from `config.toml`; `keyword_weight` and `semantic_weight` are now in `[indexing]`. `provider::embedding_models` module kept as `#[cfg(test)]` only — no production call sites, reserved for future tooling. User config updates (outside this PR): `~/.config/sprachspiel/models.toml` adds `[models."nomic"]` with `embeddings = true`, `dimensions = 768`, `provider = "llama-swap"`; `embedding = true` removed from `[provider."llama-swap"]`. `~/.config/sprachspiel/config.toml` renames `[embedding]` → `[indexing]`, sets `model = "nomic"` (alias), removes `provider = "llama-swap"` (inferred from alias), adds `keyword_weight` and `semantic_weight` (moved from `[retrieval]`).

- **Decoupled Embedding Provider from Chat Provider (Issue #121 W2)** — Adds explicit `[embedding]` configuration in `config.toml` and `embedding = true` capability flag on `[provider.*]` blocks in `models.toml`. The chat provider and the embedding provider are now resolved independently: users can run llama-swap for chat and a local Ollama instance for embeddings, or any other combination. Resolution rules (in `Settings::resolve_embedding_provider`): if `[embedding].provider = "<name>"` is set, that named provider is used (must have `embedding = true`); otherwise the chat provider is used (must also have `embedding = true`); empty `[embedding].model` is a fatal error with an actionable hint. New startup probe (opt-out via `probe = false`): 1 POST `/v1/embeddings` with `dimensions: Some(256)` to verify the provider actually serves the model — fails fast with a 4-cause diagnostic. Embedding model name is now mandatory (the historical hardcoded `nomic-embed-text-v2-moe:latest` is no longer assumed); the user must declare it. New `OpenAICompatibleProvider::probe_embedding()` and `CompatOllama::probe_embedding()` for the probe. New `crate::db::EmbeddingInit` struct bundles the 3 embedding init args (provider, model name, probe flag) into a single parameter. `EmbeddingClient::new(ollama)` is removed; `with_model(ollama, model_name)` is the only constructor. New `provider::embedding_models` module holds an internal-only hardcoded list of 11 well-known embedding model fragments (Nomic, BGE, GTE, OpenAI text-embedding-3, mxbai, Snowflake Arctic, qwen3-embed); used by `sprach models upgrade` to surface a **warning** (not auto-add) when a provider serves a known embedding model but lacks `embedding = true`. The list is never exposed in error messages (per user policy: strict, no list in errors). New `sprach models upgrade` migration `MissingEmbeddingFlag` is warning-only; the apply step is a no-op. New helper methods on `Settings`: `embedding_model_name()`, `embedding_provider_name()`, `embedding_probe_enabled()`, `resolve_embedding_provider(chat_provider_name)`. `lib.rs` now declares `pub mod commands;` so `cargo test --lib` picks up the 24 unit tests in `commands::config_upgrade` and `commands::models_upgrade` (previously only visible to the binary). User config updates: `~/.config/sprachspiel/models.toml` `[provider."llama-swap"]` now declares `embedding = true`; `~/.config/sprachspiel/config.toml` adds the `[embedding]` section (`model = "nomic-embed-text-v2-moe"`, `provider = "llama-swap"`, `probe = true`).

- **Consumer Migration to LlmProvider + OpenAI-Compatible Default (Issue #121)** — Migrates all business modules from `ollama_rs` types to the agnostic `LlmProvider` trait. This is the largest sub-phase of the W2 Provider Chain. The default provider is now `OpenAICompatibleProvider` (talking `/v1/chat/completions`), not Ollama's native `/api/chat` — Ollama is now reached through its OpenAI-compatible endpoint, which keeps the codebase compatible with OpenAI, llama.cpp, vLLM, LM Studio, llama-swap, and any other OpenAI-spec server. New `OpenAICompatibleProvider` in `src/provider/openai_compat.rs` with: SSE streaming with idle timeout, OpenAI-spec tool calling (incremental `arguments` accumulation across chunks), `/v1/embeddings`, `/v1/models` for capability detection, and `Retry-After` header parsing on HTTP 429. `Retry-After` parsing wires up the previously-unused `retry_after: Option<Duration>` field on `ProviderError::RateLimit`. `num_ctx` is auto-detected from `/v1/models` (OpenAI-spec metadata) with fallback to Ollama's `/api/show` `model_info["llama.context_length"]`; users can still override via `num_ctx = N` in `models.toml`. The `OllamaProvider` introduced in #120 has been removed (replaced by `OpenAICompatibleProvider` pointing at `http://localhost:11434/v1`); the `ProviderKind::Ollama` variant is kept as a deprecated alias for backward compat in `factory.rs` (returns a clear error at runtime if used). `Settings::ollama_client()` (the deprecated regression from #120's review) has been removed. The `#[allow(dead_code)]` chain on `factory::build_provider`, `OllamaProvider::new`, and `provider_name` has been removed. **BREAKING CHANGES in `models.toml` schema:** (1) the `kind` field default changes from `"ollama"` to `"openai"`; existing configs are auto-migrated by `sprach models upgrade`. (2) `base_url` now requires the `/v1` suffix (e.g., `http://localhost:11434/v1` for Ollama); the migration adds it automatically. (3) The fields `top_k`, `repeat_penalty`, and `think` are REMOVED from `UserModelConfig` — they are not supported by the OpenAI API and not by Ollama's OpenAI-compat endpoint (issue [ollama/ollama#11325](https://github.com/ollama/ollama/issues/11325) closed as "not planned"). (4) New field `seed` added (cross-provider, optional). 35 files in business modules no longer contain `use ollama_rs::...`; only `src/provider/openai_compat.rs` (and the shim in `src/provider/ollama.rs` for type conversion tests) reference OpenAI/HTTP-specific structs.

- **OllamaProvider with Native Reqwest Client (Issue #120)** — Replaced `ollama-rs` dependency with a native `reqwest`-based `OllamaProvider` implementing the `LlmProvider` trait. Introduces named providers in `models.toml` (breaking change: `ollama_host`/`ollama_port` removed from `config.toml`). Each model now references a named provider via `provider = "name"`. Features: request timeouts (connect/read/stream idle), exponential backoff retry with jitter (±20%) and `Retry-After` header parsing, shared HTTP client with connection pooling, NDJSON streaming parser with idle timeout. Enables retry scenarios from #116 (server 500 linear backoff, network timeout exponential backoff, cancel-aware sleep).

- **Agnostic Provider Types (Issue #119)** — Foundation types for multi-provider LLM support: `LlmMessage`, `LlmResponse`, `LlmToolCall`, `LlmToolResult`, `ProviderError`, `ProviderCapabilities`, `ProviderOptions`, and the `LlmProvider` trait. Bidirectional conversions between `ollama_rs` types and agnostic types enable transparent migration. `ProviderError` carries retry classification semantics (`ServerRetry`, `NetworkRetry`, `RateLimitRetry`, `NoRetry`) consumed by the retry infrastructure from #116. This is the first step in the W2 Provider Chain — zero existing files modified, purely additive types and conversions.

- **Tool Execution Errors No Longer Kill Conversations** — When a tool fails at runtime (e.g., file not found, permission denied, network error), the error is now reported back to the LLM as a tool message. The model can see what went wrong and try a different approach within the same turn, instead of the conversation dying with an opaque error. Previously, any tool execution failure was treated as unrecoverable.
- **Startup Fails Fast When Ollama Is Unavailable** — `sprach chat` now detects an unreachable Ollama server at startup (via a 3-second `/api/tags` health check) and exits immediately with a clear error message, instead of hanging indefinitely during database initialization. If you see "Failed to reach Ollama server", start it with `ollama serve` in another terminal and retry.
- **Retry Infrastructure for Future Server-Side Errors** — Added a retry classification framework (`RetryCategory` enum: `ImmediateRetry`, `NetworkRetry`, `ServerRetry`, `RateLimitRetry`, `NoRetry`) with per-category backoff calculation. This is groundwork for OpenAI-compatible and other backends: when a server returns 500, times out, or rate-limits (HTTP 429), the retry loop applies the appropriate backoff (e.g., 5s/10s/15s for server errors, 100ms→1.6s exponential for network errors, respecting `Retry-After` for rate limits). With the current Ollama-only backend, the backoff is fully effective for tool execution errors; retry of transport-level errors will activate as more backends are supported.
- **Cancel-Aware Retry Backoff** — When a retry is in progress, pressing Ctrl+C now aborts the wait immediately instead of blocking until the backoff timer expires.

- **Tool Trait + Proc Macro `#[sprachspiel::tool]` (Issue #118)** — New `Tool` trait (`src/tools/tool_trait.rs`) and `#[sprachspiel::tool]` proc-macro crate (`sprachspiel-tool-derive/`) replace `ollama_rs::generation::tools::Tool` and `#[ollama_rs::function]` as the foundation for the W2 Provider Chain. The macro generates a `Params` struct (with `serde::Deserialize` + `schemars::JsonSchema` derives) and **two** `Tool` impls (one for our trait, one for ollama-rs's trait, so the migration is transparent to the ollama-rs `Coordinator` while introducing our own trait as the project's primary surface). All 58 tools have been migrated to `#[sprachspiel::tool]` in this PR — every tool file in `src/tools/` has been updated. The dual-impl pattern keeps ollama-rs working until #123 (Remove ollama-rs) lands.
- **Reimplemented DuckDuckGo Search (DdgSearcher)** — The web search tool no longer uses ollama-rs's built-in `DDGSearcher`. A new `DdgSearcher` struct in `src/tools/search_builtin.rs` uses `reqwest` + `scraper` to fetch and parse DuckDuckGo's HTML interface. Zero new dependencies (all required crates were already in the tree). Fixes the upstream URL-encoding bug where special characters in queries corrupted the request. Uses a realistic User-Agent string to reduce CAPTCHA risk.
- **Removed Serper.dev Search** — The `serper-tools` feature flag, the `src/tools/serper.rs` module, the `SERPER_API_URL` constant, and all related references have been removed. Web search is now exclusively via the DuckDuckGo-backed `DdgSearcher`. Users with `SERPER_API_KEY` configurations should switch to `web_search`/`web_search_news` (DDG) or wait for the MCP-based search implementation planned post-W2. This is a breaking change for users relying on Serper; migration is straightforward (no equivalent environment variable is required for DDG).
- **Message-Ordering Fix for Pre-Tool Content** — Fixed a UX regression where the model's pre-tool text was duplicated in the chat history (text appeared twice: once streamed, once as a stable message before the tool call). The `PreToolContent` view event now carries an `already_streamed: bool` flag; when true, the LLM-channel drain path skips re-emitting the content (since the user already saw it streaming). Affects the streaming TUI path only — terminal mode and `process_next()` rounds continue to emit normally. (See issue #199 for follow-up validation with additional models.)
- **Session Forget — Destructive Session Deletion with Confirmations (Issue #36)** — `/forget` removed entirely. `/session forget` is the canonical command for deleting sessions, with context-sensitive tab completion. New subcommands: `/session forget` (current session, requires `--yes`), `/session forget <name>` (preview then `--yes`), `/session forget --id <id> --yes` (delete by ID). Preview shows message count, embedding count, and todo count before confirmation. `--yes` is intentionally NOT autocompleted after session names (safety). Notes and facts survive session deletion (project-scoped, not session-scoped). `/save <name>` now rejects duplicate names within the same project. Context-sensitive autocomplete: `/session` shows subcommands with descriptions, `/session forget` shows `--id`, `--yes`, and session names, `/session forget --id` shows IDs with name descriptions.

### Known Limitations

- **DuckDuckGo may return "No results found" for some queries** — The `DdgSearcher` in `src/tools/search_builtin.rs` calls `https://html.duckduckgo.com/html/?q=...` directly. DuckDuckGo occasionally rate-limits automated traffic or shows CAPTCHA challenges, both of which can result in empty results. The tool itself works correctly; this is a third-party service limitation. Tracked in issue #200 (TBD — investigate retry/header tactics or move to MCP-based search). MCP-based search is the planned replacement (issue TBD, post-W2).

### Fixed

- **Cycle-aware message ordering in multi-round tool calls (Issue #201)** — Fixed UX regression where tool calls and their results from multiple rounds appeared in a single block at the end of the chat history, with the model's thinking/text content emitted first. In a multi-round cycle (e.g., model searches → observes results → searches again → final response), all tool indicators were batched after all text content, making it impossible to follow the model's reasoning. Two root causes fixed: (1) Wrong `round_index` assignment — `drain_and_add_tool_messages` used `current_round()` at drain time (which had already been incremented past the tools' actual round). Fix: `ChatMessage` now carries an ephemeral `round_index: usize` (not persisted to SQLite, reset per user prompt) that tracks which round of a multi-round cycle each message belongs to. `App.current_round` tracks the round counter, incremented on `ToolCallStarted` and `InterToolText`, reset on `Complete`/new prompt. `insert_at_round_boundary()` positions inter-round content after all messages of the previous round. Tool messages are now drained with the correct round index: before round increment in `InterToolText` (tools from the previous round), before round reset in `Complete`/`Error`/`Cancelled` (tools from the last round), and in the `StreamDone` handler before creating the final response message. (2) Pre-tool content lost on finalization — `finalize_streaming_zone_as_is()` only converted `AssistantStreaming` blocks within the streaming zone boundary. When tool messages were added before the zone was computed, the pre-tool `AssistantStreaming(0)` block fell outside the zone and was never converted to stable `Assistant`. Later, `finalize_stream()` replaced this unconverted block with the final response, destroying the pre-tool content (e.g., "Vou pesquisar os três temas..."). Fix: `finalize_streaming_zone_as_is()` now converts ALL `AssistantStreaming` blocks in the entire message list, not just those within the streaming zone, since `AssistantStreaming` is a transient type that must always be stabilized when streaming is interrupted by tool calls. The coordinator, core, and persistence layers are unchanged — these are purely TUI event-loop fixes.

- **100% CPU During LLM Streaming (Issue #193)** — Fix busy-wait spinlock in the TUI event loop. When the LLM was streaming tokens, the crossterm event branch of `tokio::select!` used `event::poll(Duration::from_millis(0))` which returned `Ready(None)` instantly on every iteration. Since this branch was always ready, the Tokio runtime never parked the thread — the loop spun thousands of times per second, consuming an entire CPU core. Fixed by replacing the 0ms timeout with 5ms, and skipping the redundant `view.render()` call at the end of the loop when no actual event was processed (during streaming, `stream_token()` / `stream_thinking()` already call `render()` per token). CPU usage during streaming drops from ~100% to <5%. Ctrl+C latency remains ≤5ms (imperceptible to users). No change to token streaming speed — tokens arrive via `llm_rx.recv().await`, independent of the crossterm poll timeout.

### Added

- **Config Upgrade Command (Issue #105)** — New `sprach config upgrade` subcommand merges missing default fields into the user's existing `config.toml`, preserving all existing values, user comments, and formatting. Compares the user's parsed `Settings` against `Settings::default()` to detect missing fields, then uses `toml_edit` to insert ONLY the missing fields with their default values and doc-comments extracted from the sample configuration. A backup file (`.bak`, or `.bak.YYYYMMDD-HHMMSS` if `.bak` exists) is created by default. Flags: `--dry-run` previews changes without modifying the file, `--no-backup` skips the backup. The command never modifies or removes existing values — it is purely additive. Invalid TOML is reported with the parser error and the process aborts (no destructive overwrite). The dispatcher reports `Config file not found: <path>` (with the conventional path) rather than the generic `Could not determine config directory` error when the user has no config, so the suggestion to run `sprach --init-config` is always shown. New dependency: `toml_edit = "0.22"` (Rust crate, used only by this command).

- **LaTeX Formula Rendering in Terminal (Issue #190)** — Render LaTeX math formulas as 2D Unicode character art in the terminal using the `term-maths` crate (v1.0.0, native ratatui widget). Follows the same architecture as the Mermaid feature: ` ```latex ` / ` ```math ` fenced code blocks and `$$...$$` display math blocks are detected as `ContentSegment::Latex` and rendered via `term_maths::render()` → `RenderedBlock.to_string()` → `Vec<Line>`. Feature flag `latex` (default on), `/toggle-style` toggles LaTeX rendering (fallback to raw source code block when disabled). Catppuccin Teal styling for rendered formulas. Safe panic wrapper `call_latex_safely()` preserves TUI alternate screen. `LATEX_INSTRUCTION` system prompt teaches LLMs to use ` ```latex ` blocks. Mouse selection works transparently (Unicode art is selectable/copyable). No C/JS dependencies (~1.8–8.5MB).

- **Thinking Content Preservation — Thinking Trace Transform Phase 0 (Issue #151)** — Fix the architectural bug where `strip_thinking_tags()` permanently deleted thinking content before storage. Four data loss paths are fixed: (1) streaming and non-streaming responses now use `extract_thinking()` (respects API-native `thinking` field from R1/Kimi before falling back to regex parsing) instead of `strip_thinking_tags()` in the storage path, preserving `thinking` in `SendMessageResult.thinking`; (2) `add_assistant_message()` accepts and stores thinking in a separate `thinking_content` column; (3) `add_pre_tool_message()` no longer concatenates thinking inline — thinking goes to `thinking_content` column, content stays clean; (4) `ContinuationResult` carries `thinking` and `pre_tool_thinking` fields, `handle_continuation()` accumulates them, and continuation pre-tool messages are saved using the original `previous_message_id`. Compaction summaries intentionally do NOT preserve thinking (by design — summaries are generated content, not original traces). Schema migration v13→v14 adds `thinking_content TEXT` column to `content_items`. **Embedding consistency fix:** `normalize_inline_thinking()` wraps all writes in an explicit `BEGIN`/`COMMIT` transaction (atomic batch — if interrupted, SQLite auto-rollbacks) and resets `has_embedding=0` with deletion of stale embeddings/chunks for rows whose `content` was rewritten (thinking removed) — the existing background embedding recovery pipeline regenerates them automatically. **No startup blocking:** normalization runs in the background `tokio::spawn` (before embedding recovery), not in synchronous init. **Known limitation:** thinking content from normal assistant messages before this fix is permanently unrecoverable (it was stripped before DB insertion and no raw response is stored). `[thinking_trace]` config section with `enabled = false` feature flag — when enabled, thinking traces are included in retrieval context. `ThinkingTraceSettings` struct in settings. Reference: Arabzadeh et al. 2026, arXiv:2605.03344.

- **Norm Correction in Embedding Tables (Issue #157)** — Add `+norm_correction FLOAT` auxiliary column to all vec0 embedding tables (`content_embeddings`, `chunk_embeddings_v2`, `fact_embeddings`) to correct systematic cosine similarity underestimation when effective dimensionality (d_eff) is low. Calculated on insert as `1/(|truncated_vec|²)`, the inverse squared L2 norm of the truncated dimensions. Applied at query time as multiplicative correction: `corrected_similarity = (1 - distance) * sqrt(nc_query * nc_result)`. `TruncateResult` struct carries both vector and norm_correction through the embedding pipeline. All DB insertion functions store norm_correction alongside embeddings. All semantic search functions (`search_content_semantic`, `search_facts_semantic`) read norm_correction from vec0 auxiliary columns and apply the correction. `ContentSearchParams` and `search_messages_hybrid` accept `query_norm_correction` parameter. Schema migration v12→v13 (DROP+re-CREATE, reset has_embedding flags for recovery pipeline regeneration). Prerequisite for TAP-2 (#153, thinking-aware retrieval).
- **Fact Semantic Threshold Validation (Issue #134)** — Configurable `[facts].semantic_threshold` (default: 0.70) replacing the hardcoded `SEMANTIC_SEARCH_THRESHOLD` constant. Threaded through `FactSettings` → `DedupContext` → `deduplicate_and_insert()`. Diagnostics report (`sprach diagnostics`) now includes a **Recommended configuration** section with data-driven threshold and weight suggestions based on observed d_eff, d̄, and regime classification. `ThresholdRecommendation` struct with `semantic_threshold`, `rationale`, `adjust_weights`, `suggested_keyword_weight`, and `suggested_semantic_weight`. Six new tests for threshold recommendation logic (TIGHT, SPREAD, mixed regimes, edge cases).
- **Configurable Retrieval Weights** — Add `[retrieval]` section to `config.toml` with `keyword_weight` (default: 0.4) and `semantic_weight` (default: 0.6) in `RetrievalSettings`. Hardcoded `KEYWORD_WEIGHT`/`SEMANTIC_WEIGHT` constants removed from `context_builder.rs`. All callers (context_builder, search, remember, chat/core, query) load weights from `Settings::load()`.

### Fixed

- **User message duplication in LLM prompt** — Every user message appeared twice in the prompt sent to the LLM. The root cause was that `add_user_message()` (called in `handle_user_message_stream()`) added the user message to `session.messages`, and then `build_context()` included it via `session.messages[start_idx..]` while `prepare_messages()` also explicitly added `ChatMessage::user(user_input)` at the end. Fix: `build_context()` now excludes the last user message from `session.messages[start_idx..]` since `prepare_messages()` always adds the current query at the end. This ensures the user message appears exactly once in the LLM prompt. This also saves tokens (each duplicated message cost the full message + overhead per turn) and eliminates confused responses where the LLM addressed the user message twice.

- **`/retry` used wrong user message** — `handle_retry()` called `remove_last_assistant_messages()` (which also removes the preceding user message) and then searched for the user message to retry via `get_last_user_message()`. Since the correct message was already removed, it found the previous user message (or none). Fix: capture the user content BEFORE removal with `get_last_user_message()`, then restore it to the session with `add_user_message()`, so the LLM receives the correct retry query and the session history stays intact.

- **Continuation injected empty user message** — When the continuation path called `send_message()` with `user_input=""`, `prepare_messages()` unconditionally pushed `ChatMessage::user("")` into the prompt. The actual continuation prompt was already added as an ephemeral message by `coordinator.push_ephemeral()`, making the empty user message redundant and confusing. Fix: `prepare_messages()` now skips adding `ChatMessage::user()` when `user_input` is empty. This applies only to the continuation path (normal chat never submits empty input).

- **System Prompt Clarifications (Issue #182)** — Fix behavioral bug where instruction hierarchy was missing, causing USER FACTS constraints (e.g., "rm requires confirmation") to lose to SOUL.md behavioral defaults (e.g., "confirm before rm"). Added `### INSTRUCTION HIERARCHY` section specifying priority order: USER FACTS > SOUL > TOOL DESCRIPTIONS > BASE INSTRUCTIONS. Added `### LANGUAGE` note in prompt builder (persists with `--soulless`, unlike SOUL.md). Reformulated `### TOOL USAGE` to concise behavioral instruction replacing the generic 3-step process that partially conflicted with SOUL.md "Search first" behavior. Reduced TODO and Notes tool description verbosity (token optimization).

- **Compaction prompt staleness labels** — `COMPACTION_PROMPT` now instructs the LLM to exclude staleness labels (`(stale)`, `(N days ago)`, `(unused)`) from summaries, because relative dates become inaccurate over time. Previously, summaries could preserve labels like "(62 days ago)" which would be wrong days or weeks later.

- **Instruction hierarchy example less confusing** — Changed the INSTRUCTION HIERARCHY example from `"rm is not authorized"` (prohibition) to `"rm requires confirmation"` (preference), and the SOUL example from `"confirm before destructive"` (overlapping behavior) to `"be concise"` (clearly different concern). This avoids the false impression that USER FACTS and SOUL conflict.

- **Language note for mixed-language facts** — Added a note in the `### LANGUAGE` section: "USER FACTS may contain mixed language (English subject, Portuguese object) due to automatic normalization. Interpret them semantically, not literally." This helps the LLM understand that "User prefers respostas curtas" is a valid fact, not a formatting error.

### Added

- **Embedding Diagnostics Subcommand (Issue #133)** — `sprach diagnostics` performs spectral analysis on stored embedding vectors, reporting effective dimensionality (d_eff), mean cosine distance (d̄), regime classification (SPREAD/TIGHT) at thresholds 0.70–0.85, and variance explained distribution. Supports `--source content|chunks|facts` for per-source analysis (default: all sources combined). Pure-Rust power iteration SVD (no new dependencies). Warnings for small corpora (n < 100) and low discriminative power (d_eff/25 < 2).

- **3-Layer Compaction Overflow Strategy (Issue #187)** — Progressive compaction when the conversation context exceeds the model's window during `/compact`. Previously, compaction would fail with "The prompt is too long" if the middle section exceeded the context window. Now uses a 3-layer fallback: (1) **Pre-pruning** strips long tool outputs (>500 chars) before constructing the compaction prompt, replacing them with truncated summaries; (2) **Chunked recursive summarization** splits the middle section into chunks that each fit within 60% of the context window, summarizes each independently, and combines the summaries. If combined summaries still exceed the window, recurses (up to `MAX_RECURSION_DEPTH = 3`). Adjacent chunks overlap by one message for coherence. If the model is unavailable or max recursion is reached, falls through to Layer 3; (3) **Fallback truncation** drops oldest middle messages until the prompt fits within 50% of the context window. **Defense in depth:** if `fits_in_context()` underestimates token usage and the LLM rejects the prompt as "too long", Layer 1 falls back to Layer 2, and Layer 3 provides detailed diagnostics if even truncation fails. Token estimation now applies a 20% safety margin (`ESTIMATION_SAFETY_MARGIN = 1.20`) and uses `COMPACT_MSG_OVERHEAD = 10` per message (vs. `MESSAGE_OVERHEAD = 4` used elsewhere) to account for role prefixes and formatting in compaction prompts. `COMPACTION_PROMPT_OVERHEAD` increased from 2500 to 3000 tokens. New functions: `pre_prune_messages()`, `fallback_truncate()`, `split_into_chunks()`, `fits_in_context()`, `estimate_compaction_tokens()`, `is_prompt_too_long_error()`, `max_chunk_tokens()`, `compact_recursive()`, `build_conversation_text()`, `compact_with_llm()`. New constants: `PRUNE_TOOL_RESULT_THRESHOLD`, `PRUNE_TOOL_RESULT_KEEP_CHARS`, `COMPACTION_MAX_CONTEXT_RATIO`, `MAX_RECURSION_DEPTH`, `TRUNCATION_TARGET_RATIO`, `COMPACTION_PROMPT_OVERHEAD`, `COMPACT_MSG_OVERHEAD`, `ESTIMATION_SAFETY_MARGIN`.

- **Compaction status bar update** — After `/compact`, the status bar now updates to reflect the reduced context usage (e.g., from 76% to 1%) instead of staying at the pre-compaction value. Previously, `CompactStreamDone` did not call `update_status_tokens()`, leaving the bar stale.
- **Compaction output visual separation** — Compaction summary now displays with responsive horizontal-rule separators (adapting to terminal width) before and after the summary, instead of fixed-width `────────` strings. The `MessageType::Separator` variant renders as `"─".repeat(available_width)` in dim style, filling the full chat area width. Completion message shows `"✓ Compacted N messages (preserved X first, Y last)."` followed by a closing separator.
- **Silent chunk streaming (Option A)** — During chunked recursive compaction (Layer 2), intermediate chunk summaries are no longer streamed to the TUI. Instead, `CompactInfo` progress messages show per-chunk status (`⚙ Compacting chunk 1/3...`, `⚙ Compacting chunk 2/3...`), and only the final consolidated summary streams to the TUI. This prevents confusing intermediate text from appearing and being overwritten during multi-chunk compaction. `compact_with_llm()` now takes a `stream: bool` parameter — `false` for intermediate chunks (tokens silently discarded), `true` for the final consolidation pass and single-pass compaction.
- **`LlmEvent::CompactInfo`** — New event variant for system-level information during compaction (chunk count, truncation warnings). Previously, this info was sent as `CompactStreamToken` which mixed progress text into the streaming summary content.
- **Compaction and embedding independence** — Documented that compaction and embeddings are completely independent systems. Compaction does not delete any `content_items`, `content_chunks`, or `vec0` embeddings. All original messages remain searchable via RAG after compaction. The compacted summary has no embedding of its own — it serves as LLM context only, not as a searchable document.

- **Database migration failure on upgrade from schema v8 or earlier** — `SCHEMA_SQL` contained a `CREATE INDEX IF NOT EXISTS idx_facts_embedding ON facts(has_embedding)` that referenced the `has_embedding` column before the migration that adds it (v10→v11) had run. When opening a database at schema v8, `SCHEMA_SQL` would execute this index creation and fail with `no such column: has_embedding`, preventing the database from initializing. The index creation has been moved to `migrate_v10_to_v11()` where the column is guaranteed to exist.
- **Database error details silently discarded** — `init_database_core()` logged the original SQLite/rusqlite error at `log::debug!` level, which was filtered out by the default logger (INFO level). Users saw a generic "DATABASE INITIALIZATION FAILED" message with no actionable detail. Now logs at `log::error!` level and the `DatabaseInitResult` struct returns the full error message (including the original error like `no such column: has_embedding`) so callers can display it.
- **TUI hangs on startup during embedding recovery** — After schema migration v11→v12 (which resets all `has_embedding` flags), the startup embedding recovery regenerated hundreds of embeddings synchronously before the event loop started, freezing the TUI for minutes. The `⚙ 0/1` indicator appeared but the prompt was unreachable. Now the embedding recovery pipeline (`regenerate_all_embeddings`, `recover_missing_embeddings`, `recover_missing_fact_embeddings`, `verify_and_dedup_facts`) runs as a background `tokio::spawn` task, allowing the TUI to be fully interactive from the first frame. Progress is reported via the existing `EmbeddingProgressTx` channel and cleared automatically by `poll_embedding_progress()`.
- **Application blocks on exit during embedding flush** — `/quit` and Ctrl+D ran `flush_pending_embeddings()` and `flush_pending_fact_embeddings()` synchronously before exiting, which could block for minutes when hundreds of embeddings were pending. The startup recovery pipeline already handles missing embeddings on next boot, so the exit flush was redundant. Both flush calls have been removed; exit is now instantaneous. The `recover_missing_embeddings` signature changed from owned `Arc<Database>`/`Arc<EmbeddingClient>` to borrowed `&Arc` references (the function never needed ownership, only borrowing). Removed dead code: `flush_pending_embeddings()`, `flush_pending_fact_embeddings()`, `recover_missing_embeddings_with_progress()`.
- **Embedding progress indicator showed `current > total`** — The `⚙ N/M` indicator in the TUI status bar could show `processed` exceeding `total` (e.g., `⚙ 1800/1743`). This happened because the `total` was calculated once at the start of recovery, but when long items were split into multiple chunks (or when `embed_item_with_fallback` triggered recursive chunking), each new chunk was a unit of work not counted in the `total`. Additionally, `recover_missing_fact_embeddings` and `verify_and_dedup_facts` did not report progress at all, leaving the indicator stale. Fix: (1) `total` is now dynamic and grows when items are split into chunks; (2) each chunk within a multi-chunk item increments the `processed` counter; (3) all four recovery functions now report progress via the `EmbeddingProgressTx` channel.
- **ANSI escape codes appearing as literal text in TUI error messages** — Error messages like `✗ ␛[31mError:␛[0m Internal Server Error (ref: ...)` displayed raw ANSI escape codes instead of being rendered as colors. This happened because `format_tool_error()` generated ANSI codes (e.g., `\x1B[31m` for red) when `is_plain_mode()` was false, but in TUI mode ratatui already applies red styling via `Span::styled(line, error_style())`. The ANSI codes appeared as garbled text in the TUI widget. Fix: (1) `format_error_with_status()` now uses `format_error_plain()` when `is_tui_mode()` is true, since the TUI renderer handles styling; (2) `show_error()`, `CommandOutput::Error`, `LlmEvent::Error`, and all other `ChatMessage::error()` call sites in `ratatui_view.rs` now strip ANSI codes via `strip_ansi_codes()` as defense-in-depth.
- **Empty assistant messages persisted from Ctrl+C cancellation (Issue #185)** — When the user pressed Ctrl+C during LLM streaming, `chat_stream()` in `custom_coordinator.rs` broke the streaming loop but returned `Ok(ChatMessageResponse)` with empty `full_content`. This propagated as success through `add_assistant_message("")`, persisting empty assistant messages in the database. These messages have no semantic value, confuse the LLM with empty turns, and can never receive embeddings. Fix: `add_assistant_message()` now validates that content is non-empty before persisting; `process_send_result()` skips empty assistant messages with a debug log.
- **Short content items stuck in infinite embedding recovery loop (Issue #185)** — Items with `content.len() < 10` or `content.trim().is_empty()` were skipped by recovery/regenerate code but left with `has_embedding = 0`. On every startup, recovery queries `WHERE has_embedding = 0`, found these items, skipped them, and left them as `has_embedding = 0` — forever. Fix: Recovery/reindex SQL queries now filter `AND length(content) >= 10 AND content != ''`, so short items are never selected for reindexing. The `MIN_EMBED_CONTENT_LEN` constant centralizes the 10-char threshold (previously hardcoded in 3 locations).
- **Fact embedding regeneration on every startup (Issue #185)** — `verify_and_dedup_facts()` called `generate_fact_embedding()` for ALL active facts on every startup, making N Ollama API calls even when all facts already had embeddings in the vec0 table. This caused the "indexing N facts" progress message to appear on every boot. Fix: Verification now reads existing embeddings from the DB via `get_all_fact_embedding_vectors()` and only generates new embeddings for facts with missing vec0 rows (rare edge case — recovery should catch these). Typical startup now makes 0 Ollama calls for fact deduplication.
- **vec0 re-embedding could fail with UNIQUE constraint (Issue #185)** — `update_fact_embedding()`, `update_content_item_embedding()`, and `update_content_chunk_embedding()` used bare `INSERT INTO` for vec0 tables. If called for an entity that already had an embedding (e.g., during re-embedding after content change), the INSERT would fail with `UNIQUE constraint failed` because vec0 virtual tables use the entity ID as PRIMARY KEY and do not support `INSERT OR REPLACE`. Fix: All three methods now use `DELETE + INSERT` pattern — deleting the old vec0 row before inserting the new one, making re-embedding safe.

### Added

- **`/gc` command for database garbage collection (Issue #185)** — New `/gc` command identifies and removes database artifacts: empty assistant messages from cancelled LLM calls, orphaned chunks, and orphaned vector embeddings (content, chunk, and fact embeddings whose parent record was deleted). Shows counts of each artifact type cleaned.

## [0.44.0] - 2026-05-25

### Added

- **W6-PR2: Responsive Chat Rebuild — Ratatui + CrosstermInput (Issue #146)** — Replace println+ANSI rendering with Ratatui for responsive chat at any terminal width. Replace rustyline with CrosstermInput (incompatible with ratatui raw mode). App event loop with crossterm key events (100ms poll for spinner). RatatuiView implements all 18 ChatView methods + all CommandOutput variants. TUI components: ChatMessage enum, StatusBarState with braille spinner, InputState with unicode cursor. MarkdownTheme (Dark/Light/Mono) from DisplaySettings.skin. WelcomeInfo and RecentContextInfo rendered as chat messages. Status bar with model name, token progress bar, and emoji indicators. Session save/restore on Ctrl+D and /quit. run_chat_repl() delegates all interactive display to run_chat_repl_tui() via RatatuiView. Streaming: markdown rendered incrementally during LLM response (ChatMessage::assistant_streaming), then re-rendered on completion (ChatMessage::assistant_markdown).

- **Streaming compaction** — `/compact` now streams the compaction summary in real time instead of showing a spinner and blocking the TUI. `LlmEvent::CompactStreamToken(String)` and `LlmEvent::CompactStreamDone { summary, range }` variants carry compaction streaming events through the same `llm_tx` channel as regular LLM streaming. `spawn_compact_task()` runs compaction in a background tokio task with its own channel pair. `LlmState::Compacting` disables input and shows "Compacting..." spinner label. Compaction is NOT cancellable — Ctrl+C shows "Compaction in progress, please wait..." instead of cancelling.

- **Tool message ordering** — `drain_and_add_tool_messages()` now runs immediately after key transition events (`ToolCallStarted`, `StreamBlockDone`, `InterToolText`, `Complete`, `Error`, `Cancelled`), ensuring tool messages are appended only after the LLM state has fully transitioned. Tool messages and ViewActions now arrive in the correct position relative to streaming content. Tool call indicators (🔧) appear below `🧠 Thinking` blocks instead of above them.

- **Inter-tool thinking before tool call indicators** — `LlmEvent::InterToolText` now carries a `thinking: Option<String>` field. In non-streaming tool rounds, pre-tool reasoning is inserted before tool call indicators, preventing the thinking block from appearing after tool calls.

- **Ctrl+C interrupts multi-tool execution loop** — `CancellationToken` is now threaded through `CustomCoordinator`. The coordinator checks cancellation at the start of each tool call and in `process_next()` before making the next request.

- **Vision capability check** — `sprach vision` and `sprach ocr` now validate that the selected model supports vision before processing images. `VisionError::NoVisionCapability` variant with actionable error message listing vision-capable models.

- **Mermaid diagram rendering (feature flag `mermaid`)** — ` ```mermaid ` code blocks in LLM output are rendered as Unicode box-drawing diagrams. Rich mode uses `mermaid-text::render_with_width()` for responsive terminal-width diagrams. Deferred rendering during streaming (no CPU waste). Theme-aware styling in TUI mode. Parse errors fall back to code block rendering.

- **TUI Input: ratatui-textarea integration** — Replace custom InputState with `ratatui-textarea` for full-featured text editing: multi-line input (Shift+Enter), selection, kill-ring (Ctrl+W/Ctrl+Y), undo/redo, word movement (Ctrl+Left/Right), Emacs-style navigation (Ctrl+A/E). Eliminated ~950 lines of duplicated buffer/cursor code.

- **Floating completion menu** — Tab completion overlay appears above the status bar when multiple matches are available. Common prefix highlighted in green. Navigate with arrows, confirm with Tab/Enter, dismiss with Esc.

- **Chat text selection with mouse** — Left-click+drag in the chat area selects text with visual highlight (white on blue). `ChatSelection` component tracks anchor/cursor positions in visual-line coordinates, supports text extraction for clipboard. Input/chat selection mutual exclusion.

- **Mouse wheel scroll** — ScrollUp/ScrollDown in chat area scrolls 3 lines per tick.

- **Intelligent table reflow** — Tables in the TUI adapt intelligently to terminal width. Rigid columns keep their natural width; elastic columns word-wrap. Markdown alignment hints (`:---`, `---:`, `:---:`) applied as text alignment. Box lines (`├─┼─┤`) between every data row.

- **Embedding progress indicator** — Status bar shows `⚙ current/total` when embeddings are being generated during startup indexing and `/reindex`. Channel infrastructure wired through `App::with_embedding_channel()` and `RatatuiView::embedding_tx()` for per-message progress reporting.

- **`/reindex --yes` confirmation gate + concurrent guard** — `/reindex` requires `--yes` flag. `ChatSession.is_reindexing` prevents running two reindexes concurrently. Background execution in TUI mode (no longer freezes the event loop). Resets all embedding flags before regenerating, fixing "0 of 0 embeddings" bug.

- **`/toggle-style` command** — Toggle style rendering on/off. When style is off: Mermaid blocks show as code blocks, syntect highlighting is stripped, tables use pipe-delimited format. Status bar indicator: 🎨 (style on) / 📄 (style off).

- **`tui_aware_print()` for tool indicators** — All tool visual indicators now use `tui_aware_print()` instead of raw `eprintln!`. In TUI mode, these route through the callback channel to appear as `ChatMessage::tool()` in the chat area. Fixes alternate screen corruption from tool indicators.

- **Ctrl+C context-dependent copy/cancel** — 4 priority levels: (1) chat selection active → copy and clear, (2) textarea selection active → copy and deselect, (3) textarea has text → select all, copy, then clear, (4) empty textarea → cancel LLM or exit.

- **Bracketed paste support** — Terminal paste events handled via `Event::Paste(String)` and inserted directly into the textarea.

- **Catppuccin palette for TUI code blocks** — Code blocks use colors from the [Catppuccin](https://catppuccin.com) palette (MIT License). Dark: Mocha Surface0 background with syntext preserved. Light: Latte Surface0 with Latte Text foreground. Mono: `Modifier::REVERSED` for monochrome rendering.

- **TUI logging: suppress stderr, route all output to file** — When the TUI alternate screen is active, stderr from the logger is completely suppressed. All log output goes to the `.log` file instead. File logging levels boosted in TUI mode: Normal → Info, Verbose → Debug, Trace → Trace.

- **Debug logging for vision image loading** — `VisionProcessor::load_images()` logs each image file path and base64 size at debug level.

### Changed

- **W6-PR4: Final Transition** — Ratatui is now the only chat rendering mode. `auto_compact_if_needed()` refactored into `CompactionContext<'_>` struct with `compact_if_needed()` method. `run_app_loop()` decomposed from ~1060 lines into free-function handlers in `src/chat/event_loop.rs` (~821 lines) with `repl_tui.rs` reduced to 378 lines. Provider-agnostic strings audit — "Ollama" references replaced with "LLM server" phrasing; `ERR_LLM_CONNECTION`, `ERR_LLM_NOT_RUNNING`, `ERR_LLM_ERROR`, `ERR_LLM_CLIENT_UNAVAILABLE` constants. Stale docstring cleanup — removed `TerminalView` references, "TUI Migration" framing, dead `rustyline` log filter. Flaky test fix — `#[serial_test::serial]` added to 15 tests that mutate global atomics. (Issue #148, PR #155)

- **Replace termimad with standalone monochrome markdown renderer** — Removed `termimad = "0.34"` dependency. All markdown rendering now uses a custom pipeline: `extract_content_segments()` → per-segment renderers. Rich mode (ANSI) and plain mode (`--plain`) fully functional.

- **Remove command aliases and shortcuts** — Removed ~40 single-letter and two-letter command shortcuts. Only `/quit` and `/exit` remain as synonyms. Subcommand letter aliases (`/fact a`, `/note a`, etc.) also removed.

- **Markdown rendering during streaming and for tool outputs** — `AssistantStreaming` now renders Markdown incrementally. `Tool` messages also render Markdown with dim style overlay. Feature parity with Thinking blocks.

- **Key binding overhaul — explicit key mappings** — Switched to `textarea.input_without_shortcuts()` with explicit custom handlers for every key. Ctrl+C context-dependent (copy/cancel). Ctrl+Y yanks from kill-ring. Shift+letter keys always produce uppercase.

- **TUI event loop: conditional poll timeout fixes idle CPU burn** — `poll(0ms)` during LLM streaming, `poll(120ms)` during idle. Eliminates ~5% CPU busy-wait.

- **Thinking block visual refinement** — `🧠 Thinking` header (dim cyan) + `│` left border. Content rendered as full Markdown with width-aware word-wrap. Terminal (non-TUI) synchronized to same format.

- **Streaming thinking block fragmentation fix** — `append_stream_thinking()` and `append_stream_token()` find and append to existing blocks instead of creating duplicates. `finalize_stream()` consolidates only streaming-zone Thinking blocks, preserving tool-call Thinking from earlier rounds.

- **TUI tool call display ordering fix** — Three parallel display channels (LLM streaming, ViewAction forwarding, tool_call_rx drain) unified. `insert_before_streaming_zone()` handles correct positioning. `RatatuiView::render()` no longer drains `tool_call_rx`.

- **Markdown heading underline styles** — H1/H2 render with `Modifier::UNDERLINED` in all three TUI themes. Fix `Line.style` propagation that was silently lost in all message types.

### Removed

- **TerminalView** (println-based rendering) — Removed in W6-PR2. Ratatui is the only chat rendering mode.
- **RustylineInput** and `rustyline` dependency — Removed in W6-PR2. CrosstermInput is the only input backend.
- **termimad** dependency — Replaced by standalone monochrome renderer.
- **Sub-agent output truncation** — Removed `SubagentConfig.max_output_chars` and `DEFAULT_MAX_OUTPUT_TOKENS`. Coordinator's emergency context overflow protection still handles oversized results.
- **`prompt` parameter from `spawn_ocr_agent`** — OCR extraction is mode-driven, not prompt-driven.

### Fixed

- **🧠 indicator stays in status bar after `/think` toggle off** — `update_status_model()` called after `/think` processing.
- **Multi-line input loses newlines on submit** — Textarea `submit()` preserves `\n` characters.
- **Embedding hang on exit** — Shows "Saving embeddings..." message before awaiting flush.
- **Plain mode ANSI leak** — `display_thinking()`, tool error formatting, and retry messages respect `--plain` mode. Output is pipe-safe.
- **Inter-tool message ordering** — `insert_before_streaming_zone()` detects trailing Tool messages and inserts before them.
- **Compaction summary truncation** — Removed `MAX_SUMMARY_TOKENS` constant and all truncation logic.
- **Fix history_pos not reset on Enter** — Navigation state resets on submit.
- **Fix Ctrl+D skipping embedding flush** — Ctrl+D now flushes pending embeddings.
- **Fix Shift+letter key input** — Shift+letter keys correctly produce uppercase characters.
- **Completion menu: Enter confirms and submits** — No longer leaves user stuck.
- **Fix mouse selection offset with wrapped lines** — `wrap_visual_lines()` and `source_line_map` correct alignment.
- **Filter empty tool parameter values from display** — Removes visual noise like `⚡ run_cmd(head=, command="ls")`.
- **Fix tool call detail lines leaking into TUI** — Detail lines now go to `log::debug!()` only.
- **Fix tool message ordering regression** — Append during ToolCall/Idle, insert before streaming zone only during Streaming/Thinking.
- **Flaky tests fixed** — `#[serial_test::serial]` added to 15 tests that mutate global atomics (`PLAIN_MODE`, `TUI_MODE`, `FILE_LEVEL_OVERRIDE`), eliminating ~10-20% intermittent failure rate.

## [0.43.0] - 2026-05-11

### Added

- **Visual indicators for tool actions** — Tools now show succinct one-line emoji indicators in DIM gray when they complete important actions, providing immediate visual feedback alongside the existing `🔧 name(args)` tool call display. Indicators: 📖 (`skill_view`), 📄 (`import_document`), 📝 (`note_add`/`note_edit`), 🗑️ (`note_delete`), 👍👎✎ (`feedback_submit`), 💾/⏭ (`fact_add` stored/skipped), ⚡ (`run_command` executing). All indicators use the shared `TOOL_DIM`/`RESET` ANSI constants from `debug_tools.rs` and `suspend_for_print()` for spinner compatibility. Hidden in Quiet mode.

- **Proactive skill loading** — System prompt SKILLS section now instructs the LLM to load relevant skills **before** starting complex tasks, not just "on-demand". Skill descriptions redesigned with `MUST LOAD` trigger words and domain-specific keywords (e.g., "PDFs, eBooks, documents, reports, papers" for `document-processing`). The `document-processing` skill now includes concrete detection heuristics for when to escalate to OCR/vision (garbled text, <50 chars per page, table/chart/formula references). Tool prompt sections for PDFs and agent spawning now explicitly direct the LLM to `skill_view(name="document-processing")` first.

- **Tool call display decoupled from `log` crate** — Implemented `SHOW_TOOL_CALLS` AtomicBool in `debug_tools.rs` so tool call visibility is controlled by a dedicated flag rather than `log::LevelFilter`. Compact `🔧 name(k=v)` now always shows in Normal+ mode regardless of log level. Added `show_tool_calls: bool` to `DisplaySettings` and `set_show_tool_calls()` called at startup. Quiet mode (`-q`) overrides to hide.

- **`run_command` tilde expansion with blocklist** — Added `expand_args_tilde()` in `run_cmd.rs` that expands `~` in command arguments (e.g., `pdftotext ~/doc.pdf -` → `/home/user/doc.pdf`). After expansion, paths are checked against the sensitive file blocklist (`.env`, `.ssh/`, `*.pem`, etc.) to prevent access to protected files. Environment variable expansion is intentionally NOT supported (security). 8 unit tests.

- **SF4: Logging Overhaul (Issue #110)** — Replaced `env_logger` with custom `MultiLogger` implementing `log::Log` for dual output: colored stderr + file (`~/.local/share/sprachspiel/sprachspiel.log`). Terminal default raised from `info` to `warn` — only warnings/errors shown by default. `-v` enables debug, `-vv` enables trace. File always receives `warn+` (trace mode: `info+`). Log rotation at 5 MB with 1 backup. Data sensitivity audit: added `truncate_for_log()` helper, truncated PII leakage in 3 locations (message content, fact content). Verbosity alias `"info"` removed (Normal now = warn), added `"warn"` alias.

- **SF5: Agent Spawning Tools (Issue #111)** — Replaced generic `spawn_subagent` tool with 4 dedicated spawning tools: `spawn_ocr_agent`, `spawn_vision_agent`, `spawn_translate_agent`, `spawn_summarize_agent`. Each tool has only its relevant parameters (e.g., `ocr_mode` only on OCR agent), improving LLM docstring clarity and eliminating irrelevant optional parameters. Removed `spawn_document_agent` — the LLM already has `run_command` + spawning tools and follows the `document-processing` skill, making a limited document subagent redundant. Removed direct PDF/EPUB import from `import_document` — PDFs/EPUBs must be extracted to text via `run_command("pdftotext")` first, then imported as TXT/MD/ORG. Removed `--pages` flag, PDF pipeline code, checkpoint system, and `PdfConversionError`/`PdfSupport` error types from vision tool. Updated `document-processing.md` skill to reference new tool names and LLM-orchestrated two-phase pipeline (Phase 1: `pdftotext`, Phase 2: `pdftoppm` → `spawn_ocr_agent`/`spawn_vision_agent`).

- **SF1: Colored user prompt** — User input now displays with `BOLD_CYAN` on `>>>` and `CYAN` on the text after pressing Enter, matching the User role label style in context display. The `colors` module in `view/mod.rs` was made public for cross-module reuse.

- **SF2: Clippy configuration** — Added `clippy.toml` with thresholds for `too-many-arguments` (7), `cognitive-complexity` (25), `type-complexity` (250), and project-specific `doc-valid-idents` (Sprachspiel, Ollama, SQLite, Vec0, GGUF, etc.). Added `[lints.clippy]` section in `Cargo.toml` enforcing `too_many_arguments`, `type_complexity`, `enum_variant_names`, `redundant_async_block` as warnings. `missing_transmute_annotations` set to allow (FFI requirement). Existing `#[allow]` attributes remain valid with justification comments; new violations produce warnings in CI and local dev.

- `normalize_to_storage_format()` in `src/facts/lang.rs` — Primary normalization function called before storing any fact. Applies PT→EN prefix translation and EN first-person→third-person normalization. PT noun translation (e.g., "respostas curtas" → "short responses") is deferred to LLM-mode (issue #106).

- `normalize_adverb_verb()` in `src/facts/lang.rs` — Regex-based adverb+verb expansion for storage normalization. Handles EN patterns like "I really like X" → "User really likes X" and PT patterns like "Eu sempre prefiro X" → "User always prefers X" that are not covered by the static prefix lists in `normalize_replacements()` and `translate_pt_to_en()`.

- `lemmatize_verb()` in `src/facts/lang.rs` — Verb lemmatization function for Layer 2 dedup comparison. Strips third-person inflection from verbs: "prefers" → "prefer", "likes" → "like", etc. Includes explicit lemma map and generic trailing-'s' rule with 'ss' guard.

- `VERB_LEMMAS` constant in `src/facts/lang.rs` — Known third-person verb forms and their lemmas for `normalize_for_comparison()`. Covers common preference verbs and adverb+verb phrase combinations.

- `EN_ADVERBS`, `PT_ADVERBS`, `EN_VERBS_FP_TP`, `PT_VERBS_EN_TP` constants in `src/facts/lang.rs` — Adverb and verb lookup tables for `normalize_adverb_verb()` regex expansion.

- Layer 3.5 semantic dedup in `src/facts/extract.rs` and `src/tools/fact_tools.rs` — Embedding-based similarity check for preference facts when FTS5 doesn't find conflicts. Catches "prefer dark mode" vs "prefer light mode" contradictions that keyword search misses.

- Triple-based contradiction disambiguation in `src/facts/conflict.rs` — `FactTriple` struct and `extract_fact_triple()` function for separating contradictions (same predicate, different object → Update) from duplicates (same triple → Skip) inside the semantic block. Now integrated into Layer 3.5 (after reorder) rather than Layer 2. Covers ~80% of preference/identity contradictions. Zero ML, sub-millisecond.

- `SEMANTIC_SEARCH_THRESHOLD = 0.70` in `src/facts/conflict.rs` — Insert-time semantic search threshold. Lowered from the previous hardcoded 0.90 that missed all contradictions (antonym cosine ~0.77). Measured gap: all contradictions ≥0.77, different topics ≤0.60. Separate from `SEMANTIC_DEDUP_THRESHOLD = 0.90` in verify.rs for startup O(n²) dedup.

- **Fact dedup pipeline centralized in `src/facts/dedup.rs`** — The three fact insertion callers (`/fact add` CLI command, `fact_add` LLM tool, auto-extraction `insert_fact_with_dedup`) previously duplicated ~65-75% of the dedup pipeline logic, diverging in behavior. Created `DedupResult` enum (`Inserted`, `ExactDuplicate`, `NormalizedDuplicate`, `SemanticDuplicate`, `Updated`, `Fts5Conflict`, `Error`), `DedupConfig` struct, and `deduplicate_and_insert()` function as the single source of truth. Each caller is now a thin wrapper that formats the `DedupResult` for its UI. This fixes 4 behavioral bugs in the LLM tool path: (1) threshold 0.90 → 0.70, (2) missing triple disambiguation in Layer 3.5, (3) Layer 3.5 running after Layer 3 instead of before, (4) fire-and-forget embedding instead of synchronous. Removed `Fact::for_insert()` (dead code, `deduplicate_and_insert` uses `Fact::new` internally).

- `TRIPLE_PREFERENCE_PREFIXES` and `TRIPLE_IDENTITY_PREFIXES` constants in `src/facts/lang.rs` — Source-of-truth triple extraction patterns for `extract_fact_triple()`. Preference patterns cover single verbs (prefers, likes, etc.), adverb+verb combos (usually prefers, really likes, etc.), and negation (doesn't like, etc.). Identity patterns cover name, location, work, language, and role. Includes legacy first-person entries for pre-ADR-E4-fix database data.

- `EXCLUSIVE_PREDICATES`, `POSITIVE_PREDICATES`, `NEGATIVE_PREDICATES` constants in `src/facts/lang.rs` — Predicate classification for contradiction detection. Exclusive predicates (prefers, name is, lives in) → any different object = contradiction. Positive predicates (likes, loves, enjoys, adores) → accumulative, contradiction only with word overlap > 0.3. Negative predicates (hates, dislikes, doesn't like, detesta, odeia) → accumulative, paired with positive for polarity flip detection. Enforced by `test_all_predicates_classified` unit test.

- `STOP_WORDS` constant in `src/facts/lang.rs` — EN + PT stop words for `object_word_overlap()` content word extraction. Keeps the list small to avoid false negatives from over-filtering.

- `object_word_overlap()`, `is_exclusive_predicate()`, `is_polarity_flip()`, `is_positive_predicate()`, `is_negative_predicate()` functions in `src/facts/conflict.rs` — Helper functions for the two-tier contradiction logic in `FactTriple::contradicts()`.

- `FactTriple::contradicts()` rewritten with two-tier logic — Exclusive predicates → any different object = contradiction; Accumulative predicates → only if `object_word_overlap()` > 0.3; Polarity flip → always contradiction.

- SMOKE_TEST.md sections 21.14 and 21.15 — Test procedures for `/fact add` CLI dedup parity and `/tools` toggle for Layer 3.5 testing.

- **Auto Fact Extraction (P6.1 — autoDream-lite)** - Automatic fact extraction from conversation content after each response (Issue #73)
  - Post-response heuristic extraction of preferences and facts from user messages
  - FTS5 deduplication against existing facts before insertion
  - Configurable extraction mode: `off`, `heuristic`, `llm` (default: `heuristic`)
  - Scope inference: project by default, global for cross-project patterns
  - Source attribution: auto-extracted facts marked with `Source::Llm`
  - User notification when facts are auto-extracted (configurable)
  - `[facts]` config section with `auto_extract` and `auto_extract_notify` fields

- **Feedback Infrastructure (P5)** - Complete feedback-driven memory system with active forgetting (Issue #23)
  - `/feedback good|bad|correction:<text>` command with `msg:N` targeting and `/fg`/`/fb` shortcuts
  - `feedback_signals` DB table (schema v10 migration) with CASCADE on content_items
  - `src/feedback/` module (types, db, decay, prompt)
  - Post-RRF boost/suppress multiplier in `search_content_hybrid()` with `.clamp(0.1, 3.0)`
  - LLM `feedback_submit()` tool (config.toml toggle, default on, 30% weight per ADR-004)
  - `ReplState.last_assistant_message_id` tracking for implicit feedback targeting
  - Content Decay Activation (ADR-008): `src/content/decay.rs` with Ebbinghaus formula for content items
  - Retrieval-Reinforced Retention (ADR-009): `on_content_access()` increments `access_count` + updates `last_accessed`
  - Feedback → importance adjustment: good (+0.05), bad (-0.1), creating feedback-driven forgetting loop
  - `/content prune` command + `/cp` shortcut for manual decay trigger
  - `/context` enhancement showing feedback + decay statistics
  - Soft-delete pruning (`pruned` column) preserves conversation chain integrity
  - `[feedback]` config section in config.toml with all canonical fields
  - 9 Architecture Decision Records (ADR-001 through ADR-009)

- **Specialized Agent Architecture** - One-shot subagents for OCR, Vision, Translation, Summarization (Issue #12)
  - 4 dedicated spawning tools: `spawn_ocr_agent`, `spawn_vision_agent`, `spawn_translate_agent`, `spawn_summarize_agent`
  - `/ocr`, `/vision`, `/translate`, `/summarize` chat commands - Direct user access to subagents
  - Feature flag: `subagent-tools` (default enabled)

- **Model-aware OCR prompt selection** - Vision models configured as `[model.ocr]` now use descriptive, restricted prompts instead of GLM-OCR prefixes
  - `OcrMode::into_descriptive_prompt()` returns mode-specific restricted prompts for vision models (Text/Table/Figure/Formula)
  - `is_glm_ocr_model()` utility for detecting GLM-OCR models vs. vision models
  - `parse_ocr_mode()` convenience function for parsing OCR mode from LLM string parameters
  - `ocr_mode` parameter on `spawn_ocr_agent` tool — LLMs can now specify Text/Table/Figure/Formula OCR mode
  - `/ocr` chat command now accepts an optional mode parameter (e.g., `/ocr image.png table`)
  - All 3 OCR entry points (CLI, chat `/ocr`, `spawn_ocr_agent`) use model-aware prompt selection

- **Fact Embedding & Semantic Dedup (P6.7)** - Embedding-based Layer 4 dedup for facts (Issue #73)
  - Schema v11: `has_embedding INTEGER DEFAULT 0` column on `facts` table + `fact_embeddings` vec0 virtual table (256d Matryoshka)
  - `src/facts/embedding.rs` — `generate_fact_embedding()` wrapper for fact content embedding
  - `src/facts/recovery.rs` — Startup/shutdown recovery of missing fact embeddings
  - `src/facts/verify.rs` — O(n²) semantic dedup at startup with cosine similarity ≥ 0.90 threshold
  - Eager embedding: `tokio::spawn` after fact insertion; graceful fallback when Ollama offline
  - Startup sequence: `recover_missing_embeddings()` → `recover_missing_fact_embeddings()` → `verify_and_dedup_facts()`
  - Shutdown: `flush_pending_fact_embeddings()` on `/exit`
  - Conflict resolution: duplicate → keep newer, contradiction → keep newer, global-wins-project
  - Silent by design: all operations use `log::info/debug` only

### Changed

- **Function extraction — reduce long functions (Issue #129)** — Refactor the worst `too_many_lines` violations. Three functions were genuinely extracted into smaller pieces: `run_migrations`/`apply_migrations` (484→~35 lines), `generate_all_tool_prompts`/`build_tool_context` (409→~40 lines), `dedup_new_fact`/`deduplicate_and_insert` (339→~20 lines dispatcher + extracted layer functions). Two dispatch tables (`handle_command` 304 lines, `parse_command` 278 lines) were annotated with `#[allow(clippy::too_many_lines)]` with justification: each arm is trivial routing/parsing, and reducing below 100 would require ~30 wrapper functions that add ceremony without reducing complexity. Inline handler logic was still extracted (7 new `handle_*` functions: `handle_quit`, `handle_forget_cmd`, `handle_save_cmd`, `handle_load_cmd`, `handle_debug_toggle`, `handle_skill_cmd`, `handle_skill_list_cmd`). Two parser functions (`parse_note_add`, `parse_note_subcommand`) also received `#[allow]` as state-machine and dispatch-table patterns respectively.

- **Unwrap/expect/panic triage (Issue #128)** — Systematic audit of all `unwrap()`, `expect()`, and `panic!` sites in production code. Library code now propagates errors with `?` and `map_err()` instead of panicking. CLI entry points retain justified `#[expect]` annotations with reasoning comments. `panic!` in library code replaced with `return Err(...)`. Removes ~54 crash-risk sites from non-CLI code paths.

- **Renamed from ask-ai to Sprachspiel** (Issue #126) — Complete project rename. Binary: `ask-ai` → `sprachspiel`. Config directory: `~/.config/ask-ai/` → `~/.config/sprachspiel/`. Data directory: `~/.local/share/ask-ai/` → `~/.local/share/sprachspiel/`. Database: `ask-ai.db` → `sprachspiel.db`. Project directory: `.ask-ai/` → `.sprachspiel/`. All source references, documentation, scripts, Makefile, and man page updated. Welcome banner regenerated with "SPRACHSPIEL" in gold/cyan. Internal Rust modules renamed `ask_ai::` → `sprachspiel::`. DB migration chain: `embeddings.db` → `sprachspiel.db` and `ask-ai.db` → `sprachspiel.db` (no fallback, no legacy constants).

- **Documentation cleanup (Phase 12)** — Final rename pass: fixed manpage refs (`sprach.1` not `sprachspiel.1`), updated all remaining `ask-ai`/`Ask-AI`/`ask_ai::` references in docs and IMPLEMENTATION.md, replaced `#[ask_ai::tool]` with `#[sprachspiel::tool]`, renamed proc-macro crate from `ask-ai-tool-derive` to `sprachspiel-tool-derive`, updated `book.toml` title and doc site banner, fixed `ASK_AI_DEBUG` env var to `RUST_LOG`, updated all GitHub URLs from `ask-ai-rs` to `sprachspiel`, updated GitHub Pages URL, updated launch reel, updated skill files, replaced `sprachspiel-rs` with `sprachspiel` in project naming.

- **ADR-E4 revised (again)** — PT identity facts now correctly stored in third person. "Meu nome é Ana" → "User's name is Ana" (was "My name is Ana"). "Eu moro em São Paulo" → "User lives in São Paulo" (was "I live in São Paulo"). All PT identity patterns in `translate_pt_to_en()` now output `User *` instead of `I *`/`My *`. Previously, these early-returned from Stage 1 before Stage 2 (`normalize_replacements()`) could apply the EN first→third person conversion.

- **ADR-E4 revised** — Third-person normalization is now applied at storage time (via `normalize_to_storage_format()`), not just at render time. Render-time normalization in `prompt.rs` remains as defense-in-depth.

- `extract_and_insert_facts()` is now `async` — accepts optional `embedding_client` parameter for Layer 3.5.

- `translate_pt_to_en()` now also normalizes English first-person input to third-person — `"I prefer dark mode"` → `"User prefers dark mode"`. This function is the core of `normalize_to_storage_format()`. English passthrough (`"I prefer dark mode"` → `"I prefer dark mode"`) is no longer the default behavior.

- `try_auto_extract_facts()` in `repl.rs` is now `async` — awaits `extract_and_insert_facts()` to support Layer 3.5 embedding generation.

- `handle_fact_add()` in `command_handlers.rs` is now `async` — supports Layer 3.5 embedding generation and contradiction detection.

- `normalize_for_comparison()` in `lang.rs` now lemmatizes third-person verbs after stripping subject — "prefers dark mode" → "prefer dark mode" matches "prefer dark mode" for Layer 2 dedup.

- `translate_pt_to_en()` now attempts regex-based adverb+verb expansion after static prefix lists fail — "I really like X" → "User really likes X", "Eu sempre prefiro X" → "User always prefers X".

- OCR prompts now adapt to configured model: GLM-OCR uses rigid prefixes, vision models use descriptive prompts with no-commentary restriction.

- Removed dead `OCR_SYSTEM_PROMPT` constant (was silently ignored by `/api/generate` API).

- Removed module-level `#![allow(dead_code)]` from `security.rs` and `subagent.rs`.

### Fixed

- **`/doc show` now renders markdown content in 80 columns** — Previously, `/doc show` used `println!()` to dump raw document content, meaning `.md` files showed `#`, `**`, `*` etc. as literal text instead of rendered headings, bold, and italic. It also ignored the 80-column chat terminal width, causing long lines to overflow. Now uses `print_markdown_chat()` — the same renderer as `/note show` — which formats markdown properly and wraps at the `CHAT_TERMINAL_WIDTH` (80 columns). Header also rebuilt as markdown with `## Document #N`, `**Title:**`, `**File:**` labels for consistent styling.

- **Bug ADR-E4: PT identity facts stored in first person** — `translate_pt_to_en()` generated first-person English for PT identity patterns (e.g., "Meu nome é Ana" → "My name is Ana" instead of "User's name is Ana"). This violated ADR-E4 (all facts stored in third person). Fixed by changing PT identity outputs in `translate_pt_to_en()` to third person: "Meu nome é Ana" → "User's name is Ana", "Eu moro em São Paulo" → "User lives in São Paulo", etc. Now consistent with EN identity normalization ("My name is Ana" → "User's name is Ana").

- **Bug S42.4/S43.1 (smoke test #3): Layer 3.5 semantic contradiction detection (reordered)** — "User prefers dark mode" and "User prefers light mode" coexisted because: (1) Layer 2 `find_normalized_fact()` didn't match them ("prefer dark mode" ≠ "prefer light mode"), so they passed through; (2) FTS5 BM25 tokenizes "prefers" ≠ "prefer" (no lemmatization), so low scores; (3) Layer 3.5 cosine = 0.77 < 0.90 threshold. Fixed by: **(a)** lowering `SEMANTIC_SEARCH_THRESHOLD` from 0.90 to 0.70 (measured gap: all contradictions sit ≥0.77, different topics ≤0.60), **(b)** moving Layer 3.5 BEFORE Layer 3 (FTS5 BM25) so it runs when Layer 2 misses, **(c)** adding triple-based disambiguation inside the semantic block: `extract_fact_triple()` distinguishes contradictions (same predicate, different object → Update) from duplicates (same triple → Skip) from related facts (different predicate → fall through), **(d)** keeping `is_contradiction()` as polarity fallback for like/hate pairs that triples miss, **(e)** removing dead Layer 2.5 code that was nested inside Layer 2's `if !matches.is_empty()` (by definition, contradictory facts have different normalized strings, so the block was unreachable). Identity facts ("name is Lucas" → "name is Maria", cosine 0.875) are covered automatically since they classify as `Category::Preference`.

- **Bug S42.4 race condition: async embedding missing on Layer 3.5 search** — When fact #2 was auto-extracted, fact #1's embedding might not yet exist in `fact_embeddings` because it was generated via fire-and-forget `tokio::spawn`. Layer 3.5's `search_facts_semantic()` found no results → contradictions were missed. Fixed by making embedding generation **synchronous** (await, not fire-and-forget) in both `insert_new_fact()` (auto-extraction path) and `handle_fact_add()` (`/fact add` path). After DB insert, the embedding is now generated and stored before returning, guaranteeing it's available for the next fact's Layer 3.5 search. If Ollama is offline, `has_embedding` stays 0 and recovery generates on next startup. Also changed Layer 3.5 gate from `Category::Preference` to `extract_fact_triple().is_some()` (more precise, covers both preference and identity triples).

- **Bug #3: sqlite-vec L2 vs cosine metric mismatch (ROOT CAUSE of S42.4 failure)** — `search_facts_semantic()` in `facts/db.rs` computed `similarity = 1.0 - distance`, which is only correct for cosine distance. But sqlite-vec's `vec0` virtual table uses **L2 (Euclidean) distance** by default when `distance_metric=cosine` is not specified. All 3 vec0 tables (`fact_embeddings`, `content_embeddings`, `chunk_embeddings_v2`) in `schema.rs` lacked the `distance_metric=cosine` parameter (fixed in schema v12). For L2-normalized vectors, the correct conversion is `cosine_similarity = 1.0 - (L2_distance² / 2.0)`, derived from `‖a−b‖² = 2(1 − cos(a,b))`. The broken formula caused ALL fact similarity scores to be ~0.25–0.35 too low — the effective 0.70 threshold actually required cosine > 0.955, making Layer 3.5 completely non-functional. The same bug existed in `content/db.rs` for content and chunk semantic search. Empirically verified: "prefers dark mode" vs "prefers light mode" scored **0.6304** (broken) vs **0.9317** (correct). Fixed in `facts/db.rs:446`, `content/db.rs:706`, `content/db.rs:774`. Also fixed comparison direction: `content/db.rs:790` changed from `<` to `>` (highest cosine wins, not lowest L2). *Discovered by Hermes Agent.*

- **Schema v12: `distance_metric=cosine` + ascending sort fix** — Two improvements to the semantic search pipeline: (1) Added `distance_metric=cosine` to all 3 vec0 table definitions in `schema.rs`, eliminating the application-level L2→cosine conversion (`1.0 - L2²/2` → `1.0 - distance`). Schema v11→v12 migration drops and recreates vec0 tables, resets `has_embedding` flags for startup recovery. (2) Fixed ascending sort bug in `search_content_semantic()` — results were sorted ascending by score (least similar first), then truncated. This inverted RRF ranking: the least similar semantic result received the highest RRF weight. Changed to descending sort (most similar first) to ensure rank 1 = best match.

- **Bug #4: Missing replacement fact insertion in `/fact add` contradiction paths** — In `command_handlers.rs`, after detecting a contradiction (both triple-based and `is_contradiction()` polarity paths) and deleting the old fact, `return;` exited the entire function without inserting the new replacement fact. The old fact was deleted and the new one was lost. Fixed by replacing bare `return;` with explicit `Fact::new()` + `db.insert_fact()` + synchronous embedding generation in both paths. The auto-extraction path in `extract.rs` was not affected (it calls `insert_new_fact()` which handles the insert). *Discovered by Hermes Agent.*

- **Bug #5: Accumulative predicates false positives** — `FactTriple::contradicts()` treated ALL same-predicate pairs as contradictions, so "User likes Python" vs "User likes Rust" was incorrectly flagged as a contradiction. Fixed with two-tier logic: **exclusive predicates** (`prefers`, `name is`, `lives in`) → any different object = contradiction; **accumulative predicates** (`likes`, `loves`, `hates`, `uses`) → only contradiction if objects share content words (`object_word_overlap()` > 0.3). "likes dark mode" vs "likes light mode" shares "mode" → contradiction. "likes Python" vs "likes Rust" shares nothing → coexist. Added polarity flip detection (`likes X` vs `hates X` → always contradiction). Centralized classification constants `EXCLUSIVE_PREDICATES`, `POSITIVE_PREDICATES`, `NEGATIVE_PREDICATES`, `STOP_WORDS` in `lang.rs` with enforcement test `test_all_predicates_classified`. *Discovered by Hermes Agent.*

- **`is_contradiction()` now handles third-person forms** — Added "likes ", "loves ", "enjoys ", "hates " to `contains_preference_like()`/`contains_preference_hate()`, and "doesn't "/"don't " to `has_opposite_negation()`. Previously only first-person forms ("like ", "hate ", "not ") were recognized, so stored facts in third person ("User likes X" vs "User hates X") were missed by the polarity fallback.

- **Bug #1 (smoke test #2): Adverb modifier normalization** — Added regex-based adverb+verb expansion in `normalize_to_storage_format()`. Previously, patterns like "I really like X", "I always prefer X", "I never want X" were not normalized because `normalize_replacements()` only covered the fixed list `"I usually prefer X"` etc. New `normalize_adverb_verb()` function handles EN adverbs (really, usually, always, never, generally, mostly, definitely, absolutely, personally, often, sometimes, quite, particularly, especially, strongly) with all verbs (prefer, like, love, hate, dislike, want, find, use), plus PT adverbs (sempre, nunca, geralmente, definitivamente, absolutamente, pessoalmente, frequentemente, às vezes, bastante, particularmente, especialmente) with PT verbs (prefiro, adoro, detesto, odeio, quero, gosto de). Also handles negation: "I usually don't like X" → "User usually doesn't like X". Falls through to no-change if pattern doesn't match.

- **Bug #2 (smoke test #2): Layer 2 verb lemmatization** — `normalize_for_comparison()` now lemmatizes third-person verbs to base form after stripping the subject: "prefers dark mode" → "prefer dark mode" (not "prefers dark mode"). This ensures Layer 2 dedup catches "I prefer dark mode" and "User prefers dark mode" as equivalent. Added `VERB_LEMMAS` constant and `lemmatize_verb()` function with both explicit verb lemma map (prefers→prefer, likes→like, etc.) and generic trailing-'s' stripping (works→work, speaks→speak) while avoiding over-stripping (class→clas is prevented by 'ss' guard).

- **Bug #3 (smoke test #2): `/fact add` CLI parity with LLM tool** — The `/fact add` CLI command was missing 3 features that `fact_add` LLM tool and auto-extraction had: (1) `normalize_to_storage_format()` — raw user input was stored without ADR-E4 third-person normalization, (2) Layer 1+2 dedup — only FTS5 (Layer 3) was used, (3) `generate_fact_embedding()` — facts were stored without embeddings, causing permanent `has_embedding=0` until startup recovery. Now `/fact add` calls `normalize_to_storage_format()`, checks Layer 1 (exact match) and Layer 2 (normalized match) before FTS5, performs Layer 3.5 semantic contradiction detection when embedding client is available, and eagerly generates embeddings after insertion. Function changed from synchronous `fn` to `async fn` to support embedding generation.

- **Bug #4 (smoke test #2): Layer 3.5 testability documentation** — Added SMOKE_TEST.md sections 21.14 and 21.15 documenting how to test Layer 3.5 via auto-extraction using the `/tools` toggle to disable LLM tool calls, forcing contradiction detection to occur through the auto-extraction path rather than proactive `fact_add` calls.

- **Bug #1: Third-person normalization now applied at storage time (ADR-E4 revised)** — All facts are now stored in third person ("User prefers X"), not just rendered in third person. Previously, English first-person facts like "I prefer dark mode" were stored as-is, causing inconsistency with PT→EN facts that were stored as "User prefers X". New `normalize_to_storage_format()` function in `src/facts/lang.rs` merges PT→EN translation with EN first-person→third-person normalization. `normalize_to_third_person()` in `src/facts/prompt.rs` remains as defense-in-depth for legacy data.

- **Bug #3: Contradiction detection via semantic embeddings (Layer 3.5)** — "I prefer dark mode" vs "I prefer light mode" now correctly resolves as a contradiction. Added Layer 3.5 to both `extract.rs` auto-extraction and `fact_tools.rs` `fact_add`: when FTS5 doesn't find conflicts and the candidate is a preference, generate an embedding and search `fact_embeddings` via `search_facts_semantic()` (cosine ≥ 0.90). Contradictions are resolved by replacing the old fact; duplicates are skipped. Requires embedding client availability; gracefully skips if unavailable.

- **Bug #4: Embedding serialization and timeout** — Added `Semaphore(1)` and 30-second timeout to `EmbeddingClient::embed()`. Previously, multiple concurrent `tokio::spawn` fire-and-forget tasks could overwhelm Ollama, causing silent embedding failures (`has_embedding = 0`). Now all embedding requests are serialized through the client, preventing model loading conflicts and timeouts. **Additionally**, embedding generation is now **synchronous** (await, not fire-and-forget) in both `insert_new_fact()` and `handle_fact_add()`, eliminating the race condition where a subsequent fact's Layer 3.5 search couldn't find the previous fact's embedding. Added `EmbeddingError::Timeout` variant. Also added post-recovery verification in `facts/recovery.rs` that logs a warning if facts still lack embeddings after startup recovery.

- **Embeddings fail on startup when input exceeds context window** (Issue #40)
  - Proactive context length check in `EmbeddingClient::embed()` before API call — returns `ContextExceeded` early
  - Cached `context_length` in `EmbeddingClient` via `OnceCell` to avoid repeated `show_model_info` API calls
  - Handle `EmbeddingError::ContextExceeded` variant in fallback match arms (was only matching `ApiError`)
  - Replace `panic!` on Ollama unreachable with graceful degradation in `regenerate.rs`
  - Added empty content validation to `recovery.rs` (was missing, only present in `regenerate.rs`)
  - Fixed `has_embedding=1` marking logic — only marks item when ALL chunks verified complete
  - Increased embedding safety margins: `CONTEXT_SAFETY_MARGIN` 10%→20%, `EMBEDDING_PREFIX_TOKENS` 20→30, `DEFAULT_CHUNK_PERCENT` 90%→80%, `DEFAULT_PREFIX_MARGIN` 30→40
  - Added detailed documentation explaining why token estimation is used (ollama-rs v0.3.4 ignores `prompt_eval_count`) and referencing Issue #103 for future exact token count support

- Fixed double "Error:" prefix in subagent security blocklist messages

- Fixed broken markdown tables in command documentation (ocr, vision, translate, summarize, query, chat)

- Fixed stale default feature flags in skills-system-design.md

- Fixed `led-tools` missing checkmark in README.md features table

### Deferred

- **Bug #2: PT noun translation** — Nouns after the translated prefix (e.g., "respostas curtas" → "short responses") remain in original language. This is an intentional limitation of heuristic translation. Full PT→EN noun translation will be handled by LLM-mode (issue #106, M2 milestone).

### Removed

- Removed dead code from subagent module: `uses_chat_api()`, `tool_whitelist` field, builder methods (`with_tool_whitelist`, `with_max_output_chars`, `with_model_options`), `settings` field on SubagentRunner, and `run_generate()` method (YAGNI)
- Removed `ARCHITECTURE.md` and `ask-ai-architecture.html` (obsolete draft files)
- Removed `MANUAL-TEST-SUBAGENT-SECURITY.md` (consolidated into unified test script)

## [0.40.0] - 2026-04-17

### Added

- **Logging infrastructure with `log` crate and `env_logger` backend** (Issues #60, #61, #87, #88)
  - Replace custom `log_debug()` / `AtomicBool` / `eprintln!` with industry-standard `log` crate
  - 4-level verbosity system: Quiet (`-q`), Normal (default), Verbose (`-v`), Trace (`-vv`)
  - Verbose flags available globally and in chat subcommand (`ask chat -v`)
  - `RUST_LOG` environment variable support for fine-grained control
  - `/debug` toggle in chat now syncs state and `log::set_max_level()`
  - Tool calls displayed as `🔧 name(args)` in DIM gray (matching `[Thinking]` style)
  - Tool result visibility is tiered: hidden in Normal, truncated in Verbose, full in Trace
  - Chat interactive mode ignores quiet flag (allows user input display)
  - Spinner suppressed in quiet mode
  - Rustyline debug output always suppressed
  - `debug_default` config option replaced by `verbosity` (backwards compatible)

- **Pre-tool thinking and content visible in chat**
  - Chat now shows the LLM's thinking process and text before tool calls
  - Previously only visible in query mode; now consistent across both modes
  - `ChatEvent::PreToolContent` processed via `.on_event()` callback during tool execution

- **Chat output fixed at 80 columns** (`CHAT_TERMINAL_WIDTH`)
  - All chat markdown rendering uses `print_markdown_chat()` at 80 columns
  - Thinking blocks wrap at 80 columns (uses `CHAT_TERMINAL_WIDTH` constant)
  - Recent context display truncated to 80 visual columns (ANSI-aware)
  - Query mode and other subcommands still use real terminal width

- **Memory Staleness Warnings in Facts Prompt** (Issue #70)
  - Facts in the system prompt now show age-based staleness labels when outdated
  - `(stale)` label when `decay_score < 0.3` (badly decayed)
  - `(N days ago)` label when `last_accessed` > 30 days (not recently used)
  - `(unused)` label when `access_count == 0` and age > 7 days (never retrieved)
  - Priority order: stale > days ago > unused (only one label per fact)
  - Fresh facts (recently accessed, high decay score) show no label — no noise

- **Truncation Warnings in Tool Outputs** (Issue #71)
  - `read_file(path, max_lines)` now appends `[TRUNCATED: Showing lines 1-N of M. Use read_file_segment to read more.]`
  - `search_files()` now uses standardized `[TRUNCATED: ...]` format instead of `... (stopped after N matches)`
  - `remember(query=...)` now shows `[TRUNCATED: 150 of N chars. Use remember(id="note:X") for full content.]` for notes/docs
  - `remember(query=...)` now shows `[TRUNCATED: 200 of N chars. Use remember(id="msg:X") for full content.]` for messages
  - `remember(query=...)` sub-messages now show `[+N chars]` instead of bare `...`
  - All truncation is Unicode-safe using `.chars().take()` pattern from project conventions
  - Introduced `REMEMBER_NOTE_PREVIEW_CHARS`, `REMEMBER_MESSAGE_PREVIEW_CHARS`, `REMEMBER_SUBMESSAGE_PREVIEW_CHARS` constants

- **Enhanced Todo Tools — CRUD Gaps, Priority, and Tags** (Issue #66)
  - `todo_get(id)` — Retrieve a single task by ID
  - `todo_delete(id)` — Delete a specific task by ID (previously only `clear_done` and `clear_all`)
  - `todo_edit(id, description?)` — Edit a task's description (follows `note_edit` pattern)
  - `Priority` enum: `low`, `medium` (default), `high`, `critical`
  - `tags: Vec<String>` on `Task` struct for grouping (bug, feature, refactor, etc.)
  - `todo_add(description, priority?, tags?)` — Extended creation with priority and tags
  - `todo_edit(id, description?, priority?, tags?)` — Extended editing with priority and tags
  - `todo_list(filter?)` — Filter tasks by status, tag, or priority
  - DB migration v8→v9: added `priority` and `tags` columns to `session_todos`
  - User commands: `/todo get <id>`, `/todo delete <id>`, `/todo edit <id> <description>`
  - User commands: `/todo add <desc> --priority <p> --tags <t1,t2>`

- **Session Context Resume** - Display recent conversation context when resuming a session
  - Shows last 3 exchanges (user + assistant pairs) automatically on session resume
  - Filters out System and Tool messages, showing only User and Assistant
  - Truncates each message to ~80 characters for readability
  - Uses `format_role_label()` for consistent role labels with emojis (👤 User, 🤖 Assistant)
  - Only displayed when resuming a saved session, not for new or anonymous sessions
  - Added `ChatSession::get_recent_exchanges()` method for extracting recent exchanges
  - Added `RecentContextInfo` and `RecentMessage` structs for context display formatting
  - Added `TerminalView::show_recent_context()` method and `RecentContextInfo::format_context_summary()`
  - Made `truncate_str()` pub(crate) for reuse across view modules
  - Related: Issue #67


### Changed

- **Simplified verbosity system to 4 levels** (Issue #87)
  - Levels: Quiet, Normal, Verbose, Trace (removed Debug level)
  - Normal level now shows info (was warn)
  - Verbose level now shows debug (was info)
  - Trace level now shows trace (was debug level)
  - Removed `-vvv` trace flag (replaced with second `-v`)
  - Removed `debug_default` config option

- **Removed `-d`/`--debug` CLI flag** (Issue #61)
  - `-d`/`--debug` completely removed from all subcommands (not deprecated)
  - New `-v` / `-vv` flags control verbosity (verbose / trace level)
  - `debug_default` config option replaced by `verbosity` in `[output]` section

- **CRITICAL: Removed LLM-controllable sandbox bypass from file tools.**
  The `sandbox` parameter in `read_file`, `read_file_segment`, `count_lines`,
  `list_directory`, `search_files`, `write_file`, `edit_file`, and `append_file`
  allowed the LLM to pass `sandbox=false` to escape filesystem restrictions.
  This was a security vulnerability — the entity being restricted should never
  be able to disable the restriction. Sandbox is now always enforced for all
  file operations. The `file_sandbox` config setting is also removed — sandbox
  cannot be disabled via configuration either.

- **Removed `enable_sandbox = false` option from tools.toml.** The Landlock
  sandbox for `run_command` is now always enabled on Linux (kernel 5.13+).
  There is no configuration option to disable it.

- **Added `/tmp` and `/var/tmp` as allowed directories for file operations.**
  Temporary directories are needed for tool interoperability (e.g., `pdftotext`
  output). These are the only directories outside CWD that file tools can access.
  This is consistent with the Landlock sandbox which already allows `/tmp`.

- **UX: `/forget` confirmation required** (Issue #85)
  - `/forget` now requires `--yes` flag to execute — without it, shows a warning
  - Prevents accidental data loss from typos or unintended execution
  - `/session forget` also requires `--yes` for consistency
  - No shortcuts exist for `/forget` (already enforced in PR #84)

- **UX: `/skill <name>` subcommand replaces `/<skill-name>` wildcard** (Issue #86)
  - Skills are now activated via `/skill <name>` (e.g., `/skill document-processing`)
  - `/skill` (no args) lists available skills
  - `/sk` is a shortcut for `/skill`
  - `/<skill-name>` (the wildcard match) is now an invalid command — use `/skill <name>` instead
  - Removes namespace collision risk (e.g., a skill named "forget", "new", "help")

- **Fix: FTS5 `conversation_id` column error in `delete_conversation()`**
  - Removed invalid `DELETE FROM content_fts WHERE conversation_id = ?1` query
  - The `content_fts` table (FTS5 external content mode) does not have a `conversation_id` column
  - FTS entries are cleaned automatically by the `content_items_ad` trigger when `content_items` are deleted

- **Fix: FOREIGN KEY constraint failure when saving todos**
  - `save_sqlite()` called `update_conversation_metadata()` (UPDATE) before `save_todos()` (INSERT with FK),
    but the conversation row might not exist yet (e.g., after `/forget` generates a new session ID)
  - The UPDATE silently affected 0 rows, then the INSERT into `session_todos` failed because
    `conversation_id REFERENCES conversations(id)` was violated
  - Now calls `ensure_conversation_exists()` (INSERT OR REPLACE) before the metadata update,
    matching the pattern already used in `add_user_message()` and `add_assistant_message()`

- **Code Quality: Reduce `parse_command` complexity** (Issue #35)
  - Extract `parse_fact_subcommand()`, `parse_note_subcommand()`, `parse_doc_subcommand()`, `parse_session_subcommand()` from monolithic `parse_command`
  - Consolidate 16 two-letter shortcut commands (/fa, /na, /di, etc.) as delegates to their parent parsers, eliminating ~135 lines of duplicated parsing logic
  - Eliminate `CommandResult` enum — was a 1:1 mirror of `ChatCommand` with 23+ identical variants and 30 pass-through arms in `execute_command`
  - Move execution logic from `execute_command()` into `handle_command()` in `command_handlers.rs`, using `ChatCommand` directly
  - Eliminate `SessionSubcommand` enum and `ChatCommand::Session` — `/session new|load|list|save|forget` now returns canonical `ChatCommand` variants, removing ~151 lines of duplicated handler logic
  - Net reduction: ~462 lines (1919 → ~1457 in `commands.rs`)
  - Add 77 unit tests for all extracted parsers and shortcut mappers
  - **Fix: Remove `/f` shortcut from `/forget`** — `/f` was a collision between `/forget` and `/search (find)`, causing accidental data loss. `/forget` is now only accessible by its full name; `/f` correctly maps to `/search`
  - **Add missing `/todo` shortcuts** — `/tg` (get), `/te` (edit), `/td` (delete), `/tcd` (clear-done), `/tca` (clear-all) now work alongside existing `/ta`, `/tl`, `/tu`

- **Welcome banner: "Ollama" label renamed to "Server"** - Future-proof for non-Ollama backends
  - Removed embed_model line (it's a fixed constant, not useful info)
  - Removed combined `db_stats()` function in favor of individual count methods

- **todo-tools is now built-in** - No longer requires feature flag
  - Todo tools are always available (like facts and notes)
  - `TodoState` is now always part of `ChatSession`
  - Removed `todo-tools` from feature flags in Cargo.toml
  - All `/todo` commands work without enabling features
  - Related: Issue #31, PR #62

- **Code Quality: registry.rs Refactoring** - Reduce cognitive complexity from 56/25 to <25/25
  - Extract 13 `register_*_tools()` helper functions for tool registration
  - Extract 13 `get_*_tool_names()` helper functions for tool name listing
  - Create `register_if_allowed!` and `push_if_allowed!` macros for DRY code
  - Ensure consistent tool ordering between `register_tools()` and `get_available_tool_names()`
  - Related: Issue #31, PR #62

- **Code Quality: context_builder.rs Refactoring** - Reduce cognitive complexity from 27/25 to below 25
  - Extract retrieval logic into `perform_retrieval()` helper
  - Extract message conversion into `push_messages()` helper
  - Related: Issue #30

- **Code Quality: query.rs Refactoring** - Reduce cognitive complexity from 32/25 to below 25
  - Extract initialization logic into helper functions
  - Related: Issue #29


### Fixed

- **Recent context display: multi-line messages breaking layout** - Newlines in message content are now collapsed to spaces so each message displays on a single line when resuming a session
  - Previously, messages containing `\n` would spill across multiple lines in the "Recent context" summary, making the display messy
  - `strip_thinking_tags()` output is now `.replace('\n', " ")` before truncation
  - Added test `test_recent_context_info_newlines_collapsed`

- **CRITICAL: Unicode panic on string truncation in chat resume** - Fixed crash when resuming sessions with multibyte characters
  - `truncate_str()` used byte-based slicing (`&s[..N]`) which panicked on non-ASCII characters
  - Portuguese text with `ç`, `ã`, `é` (2-3 bytes each) caused panic at byte boundaries
  - Rewrote `truncate_str()` to use `.chars().count()` and `.chars().take(N)` for Unicode-safe operations
  - Replaced 4 inline byte-slicing patterns in `command_handlers.rs` with `truncate_str()` calls
  - Added Unicode regression tests (Portuguese, CJK, mixed content)
  - Related: Issue #69

- **search_files: empty file_pattern silently filtering out all files** - LLMs often send `file_pattern=""` instead of omitting it, causing zero search results
  - Added `.filter(|s| !s.is_empty())` normalization so empty string → None (search all files)
  - Without this fix, `glob_to_regex("")` produced regex `^$` that matched no filenames
  - Also improved `log_tool_call` to display `"all"` instead of `""` when file_pattern is None
  - Related: AGENTS.md "Empty String Normalization for `Option<String>`" pattern

- **search_files: improved docstring and documentation** - Better guidance for LLM regex usage
  - Added note that only text files are searched (PDFs, binaries silently skipped)
  - Recommended grouped alternation `"^(A|B)"` over `"^A|^B"` to avoid pattern truncation
  - Documented `(?i)` for case-insensitive search and `^`/`$` line-anchor behavior
  - Clarified that `path` can be a single file, not just directories
  - Updated tools.md with expanded examples and pattern tips

- **summarize/vision subcommands ignoring config.toml model settings** - Subcommands now respect user model settings
  - `summarize` subcommand was falling back to hardcoded `qwen3.5:4b` instead of using `config.toml`
  - `vision` subcommand was ignoring user's configured default model
  - Both now properly resolve models from `resolve_model_config()`
  - Related: Issue #65

- **Model change via /model not persisted to database** - `/model` switch now saves to DB
  - `handle_model_switch` was not calling `session.set_model()`, so the model changed in memory but not in the session
  - `update_conversation_metadata` was not including the `model` column in the UPDATE query
  - Both fixed: session model is now updated and persisted on save

- **Empty string normalization for `Option<String>` tool parameters** - LLMs send `""` instead of omitting
  - `note_edit(id, title, content)` now normalizes `Some("")` → `None` for title and content
  - `note_add(content, title)` now normalizes empty title → None (falls back to "Untitled")
  - `import_document(path, scope, title)` now normalizes empty title → None (triggers auto-extraction)
  - Added AGENTS.md section documenting the pattern and checklist


## [0.39.5] - 2026-03-30

### Fixed

- **import_document Tool Missing Embedding/Chunking** - Tool now creates embeddings and chunks synchronously
  - Documents imported via LLM tool are immediately searchable
  - Large documents automatically chunked (~512 tokens per chunk)
  - Error message guides user to run '/reindex' if indexing fails
  - Warning message when no embedding model available
  - Documents stored with proper chunk metadata for navigation
  - Related: Issue #54

- **Document Size Limit Reduced to 2.5MB** - Prevents context overflow
  - Previous 5MB limit could exceed model context on retrieval
  - Documents larger than 2.5MB are rejected with helpful error message
  - Documents > 50KB without chunks flagged with re-import instructions
  - Related: Issue #54

- **remember Tool Protection** - No longer returns full content of unchunked large docs
  - Returns helpful error explaining how to re-import
  - Prevents context explosion for incorrectly imported documents
  - Clear instructions: delete + re-import with proper chunking
  - Related: Issue #54

- **run_command Error Messages** - Now shows meaningful error context
  - Replaces generic "exit code Some(1)" with actionable suggestions
  - Includes common causes for missing stderr
  - Clean exit code formatting
  - Related: Issue #54

### Added

- **Title Parameter for import_document** - LLM can provide descriptive titles
  - Recommended for .txt files without obvious titles
  - Improves search quality and helps identify duplicates
  - Fallback chain: `#+TITLE:` directive → first heading → filename
  - Prompt engineering in DOCUMENT TOOLS section guides LLM usage
  - Related: Issue #54

- **DOCUMENT TOOLS System Prompt Section** - Guides LLM on proper tool usage
  - Explains synchronous indexing behavior
  - Provides title guidelines with examples
  - Shows file limits and supported formats
  - Located in `src/prompts/tools.rs`, feature-gated by `document-tools`
  - Related: Issue #54

### Changed

- **MAX_DOCUMENT_SIZE constant** - Reduced from 5MB to 2.5MB
  - File: `src/content/document.rs`
  - Prevents documents that would exceed model context
  - Related: Issue #54

- **Default Model Changed** - From llama3.1 to qwen3.5:4b
  - `DEFAULT_MODEL`: `llama3.1` → `qwen3.5:4b`
  - Context: 4K → 128K tokens
  - Temperature: 0.8 → 1.0
  - Thinking mode: disabled → enabled by default
  - Multimodal: supports vision tasks natively

- **New Code Model Default** - Dedicated model for code mode
  - `DEFAULT_CODE_MODEL`: `qwen2.5-coder:7b`
  - Optimized for coding with function calling
  - Automatic fallback: code mode → code default → global default
  - Behavior: `sprach "query"` → qwen3.5:4b, `sprach -c "code"` → qwen2.5-coder:7b

- **Built-in Models Reduced** - From 4 to 3 models
  - Removed: `moondream` (now redundant - qwen3.5:4b is multimodal)
  - Kept: `qwen3.5:4b`, `translategemma`, `glm-ocr`

- **Vision Default Model** - Changed to qwen3.5:4b
  - Previous: `moondream:1.8b`
  - New: `qwen3.5:4b` (multimodal, same as general default)
  - Moondream remains available as alternative

## [0.39.0] - 2026-03-29

### Added

- **Document Import Tool** - Import documents for semantic search and retrieval
  - **File Formats:** TXT, MD, ORG only (PDF/EPUB: extract text via `run_command("pdftotext")` first)
  - **File Size Limit:** 2.5MB for uploaded files; larger files rejected with helpful error
  - **Commands:** `/doc import`, `/doc list`, `/doc show`, `/doc delete` (shortcuts: `/di`, `/dl`, `/ds`, `/dd`)
  - **LLM Tool:** `import_document(path, scope?)` for autonomous document import
  - **Chunking:** Uses same system as notes/messages (~512 tokens)
  - **Scope:** Project-scoped by default, optional global scope
  - **Storage:** Documents stored in `content_items` table (ContentType::Document)
  - **Retrieval:** Integrated with `remember()` tool via hybrid search (BM25 + vector)
  - **Title Extraction:** Automatic from filename or first heading
  - **Feature Flag:** `document-tools` feature (enabled by default, included in `all-tools`)
  - Related: Issue #9

- **Document Retrieval Integration** - Documents now searchable via `remember()` tool
  - `remember(id="doc:N")` retrieves full document content (or preview for large docs)
  - `remember(id="doc:N", chunk="M")` retrieves specific chunk of large documents
  - `remember(query="...")` searches across messages, notes, AND documents
  - Hybrid search (BM25 + semantic) includes documents in results
  - Large document preview shows first 3 chunks with navigation hint
  - Chunk output shows position info (e.g., "Chunk 15/87, chars 15000-16000")
  - Related: Issue #9

- **Parameter Validation for remember() Tool** - Clear error messages for invalid parameter combinations
  - Error when both `id` and `query` are specified (mutually exclusive)
  - Error when `limit` used without `query` (limit only for searches)
  - Error when `chunk` used with non-document IDs (chunk only for docs)
  - Helpful error messages explain correct usage

- **Synchronous Embedding for Document Import** - Documents indexed immediately by default
  - `/doc import <path>` - Synchronous indexing with progress indicator
  - `/doc import <path> --nowait` - Async indexing in background
  - Embeddings created before command returns (default behavior)
  - Progress message: "Indexing document..." → "Document indexed (N chunks)"
  - Related: Issue #9

- **Embedding Flush on Exit** - Pending embeddings completed before shutdown
  - `/exit` now waits for any pending embeddings to complete
  - Progress bar shows completion status
  - Ensures no data loss on graceful shutdown
  - Related: Issue #9

### Fixed

- **Tilde (~) Expansion in File Paths** - File paths with `~` now correctly expand to home directory
  - Affects: `/doc import`, `read_file`, `write_file`, `edit_file`, `append_file`, `list_directory`, `search_files`
  - Also affects: `validate_image_file`, `read_file_as_base64`, `/export` command
  - Users can now use `~/path/to/file` syntax everywhere
  - Related: Issue #9 (bug report from Hermes Agent)

- **Document ID Format Flexibility** - Multiple ID formats now accepted
  - `#N` format: `/doc show #1`, `/doc delete #5`
  - `doc:N` format: `/doc show doc:1`, `/doc delete doc:5`
  - Numeric format: `/doc show 1`, `/doc delete 5`
  - All three formats work consistently across all document commands
  - Related: Issue #9 (bug report from Hermes Agent)

- **Org-Mode Title Extraction** - `#+TITLE:` directive now correctly parsed
  - Files like `#+TITLE: My Document` extract "My Document" as title
  - Previously showed literal "+TITLE: My Document"
  - Fallback to `* heading` if no `#+TITLE:` found
  - Fallback to filename if no heading found
  - Related: Issue #9 (bug report from Hermes Agent)

### Technical Debt

- **Document Import: No Direct PDF/EPUB Support** - `import_document` only accepts TXT, MD, ORG files.
  - For PDF/EPUB content: extract text via `run_command("pdftotext")` first, then import as TXT/MD
  - Related: Issue #12, Issue #9

## [0.38.0] - 2026-03-27

### Added

- **Skills System Implementation** - Full implementation of on-demand skill loading
  - **Core Module:** `src/skills/` with types, loader, sanitize, and builtin skills
  - **Tools:** `skill_list()` for listing available skills, `skill_view(name)` for loading skill content
  - **Slash Commands:** Activate skills via `/skill-name` (e.g., `/document-processing`)
  - **Session Integration:** Active skills injected into system prompt
  - **4 Builtin Skills:** document-processing, ocr-images, code-analysis, web-scraping
  - **System Prompt Integration:** SKILLS INDEX section shows available skills with descriptions
  - **Tool Registration:** skills-tools feature (enabled by default)
  - **Security:** Injection pattern detection, invisible unicode removal, file size limits (256KB)
  - Related: Issue #8

- **Document Processing Skill** - Unified PDF and ePub processing
  - **PDF Tools:** pdftotext, pdfinfo, pdftoppm, tesseract (OCR fallback)
  - **ePub Tools:** ebook-convert (Calibre), epub2txt (lightweight fallback)
  - **Features:** Full extraction, page range, metadata, TOC, internal search
  - **Multi-distro:** Installation instructions for Arch, Debian, Void, Alpine, Fedora
  - **External Tool Defaults:** ebook-convert and epub2txt added to default tools.toml

- **Skills System Design Document Update** - Comprehensive design research and planning
  - **Hermes Agent Analysis:** Researched skills system implementation from `~/.hermes/hermes-agent`
  - **Progressive Disclosure:** INDEX in prompt + on-demand loading via `skill_view(name)`
  - **Directory-based Skills:** `SKILL.md` format with YAML frontmatter
  - **Deduplication Priority:** project > user > builtin
  - **Simplified Frontmatter:** Only `name` and `description` required
  - **Two Tools:** `skill_list()` for INDEX, `skill_view(name)` for content
  - **Implementation Phases:** 5 phases estimated at 3.5 days total
  - Related: Issue #8

- **Multilingual Prompt Injection Security Research** - Comprehensive security analysis
  - **Documented Bypasses:** Azure Content Filter bypassed using Thai/Arabic payloads (HackerNoon)
  - **Academic Research:** arXiv:2512.23684 multilingual hidden prompt injection on 500 papers
  - **ML Detection:** XLM-RoBERTa fine-tuned achieves 99.13% accuracy (arXiv:2410.21337v1)
  - **Future Consideration:** Translate-then-detect approach using existing `ask translate` infrastructure
  - **Current Mitigation:** English-only sanitization + warning on non-Latin characters
  - References added to skills-system-design.md

### Changed

- **pokemon-tools: Removed from default features** - Now opt-in
  - Build with `--features pokemon-tools` to enable Pokémon data tools
  - Reduces default binary size
  - Precedent for future Plugin System with MCP support

- **skills-system-design.md Complete Rewrite** - Updated from original design
  - Removed Phase 1 (already completed in v0.28.x)
  - Added Hermes Agent research findings
  - Changed from "inject all skills" to "INDEX + on-demand" pattern
  - Changed from `.md` files to `SKILL.md` in directory structure
  - Changed from 8-10 days estimate to 3.5 days
  - Added implementation status tracking
  - Added comprehensive security considerations (OWASP LLM Top 10)
  - Added multilingual injection defense as future consideration

- **Prompt Simplification** - Reduced PDF instruction duplication
  - FILE TOOLS prompt now references `skill_view("document-processing")` instead of detailed instructions
  - EXTERNAL TOOLS prompt simplified, moved examples to document-processing skill
  - Skills become single source of truth for domain-specific instructions

- **pdf-processing Skill → document-processing Skill** - Unified PDF and ePub processing
  - Replaced `pdf-processing` builtin skill with `document-processing`
  - Added ePub extraction via ebook-convert and epub2txt
  - Added metadata extraction, TOC parsing, page range support
  - Added OCR fallback for scanned PDFs and ePub images
  - Updated all code references and documentation

### Planned

- **PRIORITY 10: Multilingual Skill Sanitization** - Enhanced security for skill content
  - Phase 1: Language detection + warning (no dependencies)
  - Phase 2: Translate-then-detect approach (requires P4 Specialized Agents)
  - Dependencies: Skills System (P3) ✅ COMPLETED

- **PRIORITY 11: Skills Management Tool** - Allow LLM to manage skills
  - `skill_manage(action, name, ...)` tool for create/patch/delete
  - Estimated effort: 3-4 hours
  - Dependencies: Skills System (P3) ✅ COMPLETED

## [0.37.2] - 2026-03-22

### Added

- **Status Bar Above Prompt** - Dynamic status bar showing context information
  - Displays model name, context usage (XX.XK/YYYK), progress bar with percentage, and think/tools indicators
  - Fixed width of 77 visual characters to prevent overflow
  - Colored progress bar: green (< 50%), yellow (50-75%), red (> 75%)
  - Clean prompt: `>>> ` with all context moved to status bar
  - Updates dynamically on each prompt cycle
  - Status bar rendered above prompt, cleared before user input appears
  - Visual truncation handles Unicode characters correctly
  - Terminal width detection for proper ANSI clear codes
  - Fallback to single line if terminal width unavailable
  - Related: Issue #47

- **Inter-Tool Compaction with Automatic Continuation** - Automatic context compaction during multi-tool execution
  - Detects when context reaches `COMPACTION_BUFFER` during tool execution
  - Stops tool execution and triggers auto-compaction
  - Sends continuation prompt automatically after compaction
  - LLM continues from where it stopped without user intervention
  - Maximum 3 compaction cycles per message to prevent infinite loops
  - `ChatEvent::ContextNeedsCompaction` event for coordination
  - `OverflowHandleResult` enum to distinguish overflow types
  - `build_inter_tool_compaction_prompt()` for continuation messages

- **Observability Metrics for Inter-Tool Compaction** - Detailed debug logging
  - Token count before/after compaction (saved tokens)
  - Message count before/after compaction
  - Compaction duration in seconds
  - Summary length after compaction
  - Cycle tracking with remaining cycles warning
  - Tools executed before pause logged for debugging

- **Debug Logging for Inter-Tool Check** - Permanent logging for troubleshooting
  - `[INTER-TOOL-CHECK]` logs showing history/tools/system/result tokens
  - Shows remaining buffer vs COMPACTION_BUFFER comparison

- **Percentage-Based Context Thresholds** - Replaced fixed buffer constants with percentage-based thresholds
  - Scales correctly with different context window sizes (32K, 128K, 200K)
  - `MODERATE_USAGE_PERCENT = 0.75` - Warning at 75% usage
  - `CRITICAL_USAGE_PERCENT = 0.88` - Auto-compact at 88% usage
  - `INTER_TOOL_USAGE_PERCENT = 0.94` - Inter-tool warning at 94% usage
  - `EMERGENCY_USAGE_PERCENT = 0.97` - Emergency truncation at 97% usage
  - Absolute minimums ensure safety even for small contexts:
    - `PRE_TOOL_MIN = 2_000` tokens
    - `COMPACTION_MIN = 1_000` tokens
    - `INTER_TOOL_MIN = 512` tokens
    - `EMERGENCY_MIN = 256` tokens

### Changed

- **Startup Output Reorder** - Improved visual flow for chat startup
  - ASCII art banner now appears first, before any other output
  - Session resume and regeneration messages appear after banner
  - "Type /help for commands, /quit to exit" now appears at the end, after all startup messages
  - Sandbox status strings now lowercase for consistency with other status fields
  - "not compiled" sandbox status shortened to avoid exceeding column 80

- **Compaction Thresholds** - Adjusted to prevent overflow loops
  - Added `COMPACTION_BUFFER` (15,000 tokens) - reserve space before overflow
  - Added `MAX_SUMMARY_TOKENS` (3,000 tokens) - hard limit on summary size
  - Compaction now triggers when context reaches `context_window - COMPACTION_BUFFER`
  - Summary is automatically truncated if it exceeds `MAX_SUMMARY_TOKENS`

- **Compaction Summary Template** - Restructured for better context preservation
  - Old: Generic markdown with Key Topics, Decisions, Technical Details, Action Items
  - New: Structured template with Goal, Instructions, Progress (Completed/Pending), Discoveries, Relevant Files
  - Inspired by OpenCode's compaction template for better context continuation
  - Explicit token limit warning in prompt to prevent oversized summaries

### Fixed

- **Embedding Fallback for Oversized Content (Complete Rewrite)** - Fixed PRIMARY KEY constraint violation
  - **Bug Discovered:** Previous `embed_with_fallback()` returned multiple embeddings for same chunk_id, causing database constraint violations
  - **Bug Discovered:** `has_embedding` was marked as 1 even when embeddings failed, preventing recovery
  - **New Design:** Function now manages chunk creation atomically with transaction support
  - **New module:** `src/embeddings/fallback.rs` with `EmbedContext` and `EmbedItemContext` structs
  - **Two functions:** `embed_chunk_with_fallback()` for existing chunks, `embed_item_with_fallback()` for new items
  - **Atomic transactions:** Chunks are created and embeddings saved in single transaction
  - **Protection limits:** `MAX_FALLBACK_DIVISIONS=4`, `MAX_CHUNKS_PER_ITEM=64`, `MIN_CHUNK_TOKENS=32`
  - **Panics on misconfiguration:** Prevents database explosion from bad configs
  - **Removed:** Old `embed_with_fallback()` that returned `Vec<Vec<f32>>`
  - **Simplified:** `client.rs` now has simple `embed()` that returns error on context exceeded
  - **Fixed:** Recovery embeddings now visible with `println!` instead of `log_debug!`

- **CRITICAL: Multiple Token Calculation Bugs** - Fixed three separate double-counting bugs

  1. **Double-counting system + tools in `calculate_context_metrics()`**
     - Root cause: Comments said `real_history_tokens` was "history only" but it was actually "total from Ollama"
     - The function added system + tools again to get total, causing double-count
     - Fix: Recognize `real_history_tokens` as TOTAL, derive history by subtraction
  
  2. **Double-counting system_tokens in `needs_inter_tool_compaction()` and related functions**
     - Root cause: Functions received total and added system_tokens again with `.saturating_add(system_tokens)`
     - Fix: Accept single `total_tokens` parameter since Ollama already includes system + tools
  
  3. **Missing system + tools in pre-tool warning remaining tokens**
     - Root cause: `remaining = context_window - history_real_tokens()` missed system + tools
     - Fix: Use `total_tokens` from `ContextStatus` for correct remaining calculation

- **CRITICAL: Pre-Tool Warning Message False Advertising**
  - Root cause: Message said "Auto-compacting..." at 75% threshold, but auto-compact only triggers at 88%
  - Users saw "Auto-compacting..." but context wasn't actually compacted
  - Fix: Split logic - show warning at 75%, auto-compact only at 88%
  
- **Duplicate Context Warnings** - Fixed two warnings shown for same condition
  - Root cause: Both `send_message()` in core.rs and `check_and_compact_before_tool()` in continuation.rs showed warnings
  - Fix: Only show warning in core.rs when tools are disabled (continuation.rs has more informative message)

- **Token Estimation Undercounting vs Real Ollama Tokens**
  - Estimation word-based can undercount by 20-30%
  - Combined with missing tool tokens, total undercount was 25-35%
  - Context could be at 100% real capacity while check saw only 65-70%
  - Combined fixes now accurately detect overflow

- **Context Overflow Compaction Loop** - Fixed infinite compaction loop caused by oversized summaries
  - Root cause: Compaction summaries had no size limit, generating ~18K token summaries
  - Combined with late trigger (95%+), summaries caused immediate re-compaction
  - Solution: 3,000 token limit on summaries + 15,000 token buffer before overflow
  - New structured summary template inspired by OpenCode's approach
  - Template includes: Goal, Instructions, Progress, Discoveries, Relevant Files
  - Automatic truncation if LLM ignores token limit

- **Context Overflow During Multi-Tool Execution** - Added pre-tool token budget check
  - Token budget verification before each tool execution in multi-tool chains
  - Prevents context overflow when LLM calls multiple tools sequentially
  - Per-tool token budgets defined in `TOOL_TOKEN_BUDGETS`
  - Smart truncation for large tool results

- **Unicode Panic in note_add** - Fixed panic when creating notes with Unicode content
  - `note_add` tool now uses `truncate_chars()` for character-aware truncation
  - Previously used byte slicing (`&content[..200]`) which panicked on multi-byte characters
  - Box-drawing characters (`─`, `┌`, `└`) and other Unicode now work correctly

- **Clippy Warnings** - Fixed all clippy warnings across codebase
  - Used `div_ceil()` instead of manual ceiling division
  - Collapsed nested `if let` patterns
  - Changed `push_str("🧠")` to `push('🧠')` for single chars
  - Simplified `!x.is_none()` to `x.is_some()`
  - Added `#[allow(clippy::too_many_arguments)]` for functions that need many args

### Removed

- **Dead Code Cleanup** - Removed unused code from `context_overflow.rs`
  - `estimate_messages_tokens()` - replaced by `estimate_chat_messages_tokens()`
  - `MAX_TOOL_RESULT_TOKENS` constant - no longer used
  - `CHARS_PER_TOKEN` constant - no longer used
  - `truncate_tool_result()` function - no longer used
  - All were marked "no longer used" with explicit comments

## [0.36.0] - 2026-03-19

### Added

- **Welcome Banner Redesign** - New ASCII art banner with Extended Mind concept
  - Logo using `toilet` "future" font with metallic blue colors
  - ASCII art generated from custom image via `jp2a` (True Color ANSI)
  - Session info (Model, Think, Tools, Sandbox, Project, Session) aligned to ASCII art
  - Clean Unicode line separators (`─`) instead of double lines
  - Assets stored in `assets/` directory for reproducibility
  - See `assets/README.md` for regeneration instructions

- **Prompt Emojis** - Replaced `[t][T]` indicators with emojis
  - `🧠` = think mode active
  - `🔧` = tools active
  - Example: `model🧠🔧>` instead of `model[t][T]>`

- **`/session` Command Group** - Unified session management interface
  - `/session new` - Same as `/new`
  - `/session load <name>` - Same as `/load`
  - `/session list` - Same as `/list`
  - `/session save [name]` - Same as `/save`
  - `/session forget` - Same as `/forget`
  - Intended for users who prefer noun-verb command structure

- **Database Initialization Failure** - Fail fast with detailed error when database cannot be initialized
  - Previously, database errors were silently ignored, creating inconsistent state
  - Now shows detailed diagnostic message with storage path and possible causes
  - Suggests solutions (check Ollama, permissions, or use --anonymous)

- **Schema Migration v6→v7 UNIQUE Constraint Error** - Fixed embedding migration duplicate key error
  - Removed broken embedding migration that caused "UNIQUE constraint failed on content_embeddings primary key"
  - Embeddings are now regenerated from source content after migration
  - Added progress bar with ETA during regeneration (uses indicatif crate)
  - Preserves all user data (messages, notes, facts) - only embeddings are regenerated
  - Migration runs synchronously before app becomes usable

- **Remember Tool Empty Parameters** - Treat empty strings as None
  - LLM sometimes passes `id=""` instead of omitting the parameter
  - Tool now validates and filters empty strings before processing

- **SQLite-vec Parameter Mismatch** - Fixed semantic search query
  - `SEMANTIC_SEARCH_ITEMS_SQL` and `SEMANTIC_SEARCH_CHUNKS_SQL` constants were missing WHERE clause
  - sqlite-vec requires `WHERE embedding MATCH ? AND k = ?` for KNN queries
  - Fixed "Wrong number of parameters passed to query" error in `remember()` tool

- **YAGNI Code Removal** - Removed unused methods from DynamicChunkConfig
  - Removed: `with_percentages()`, `context_length()`, `prefix_margin()`, `chars_per_token()`
  - These were test-only or never used
  - Kept: `new()`, `max_chars()`, `overlap_chars()`, `min_chunk_chars()` (all production)

- **YAGNI Variable Removal** - Removed unused `chunks_failed_before` variable in regenerate.rs

- **Notes System** - Persistent notes with semantic search
  - User commands: `/note add`, `/note list`, `/note show`, `/note edit`, `/note delete`, `/note search`
  - Shortcuts: `/na` (add), `/nl` (list), `/ns` (show), `/nd` (delete)
  - Notes support optional titles and project/global scope
  - FTS5 keyword search for finding notes
  - `SourceType::Note` added to retrieval system
  - Schema v7: unified `content_items` table for messages, notes, and future documents
  - Unified search API: `search_content_keyword`, `search_content_semantic`, `search_content_hybrid`
  - Async embedding generation for notes on creation
  - Comprehensive test suite for note operations

- **Remember Tool Integration** - Notes now accessible via LLM retrieval
  - `remember(id="note:N")` retrieves specific notes
  - `remember(query="topic")` searches across messages AND notes
  - Results distinguish between content types (Messages vs Notes)
  - Prompt engineering updated to document content types
  - Unified `search_content_hybrid()` enables semantic search across all content

- **`note_add` Tool for LLMs** - LLMs can now create notes autonomously
  - New tool: `note_add(content, title)` creates persistent notes
  - Notes are project-scoped (not global) and marked as LLM-created
  - Distinguishes from `fact_add`: notes for longer documents (up to 10K chars), facts for short info (500 chars)
  - Notes are NOT in system prompt (use `remember()` to retrieve)
  - Prompt engineering guides LLM on when to use notes vs facts

- **Note List Pagination** - `/note list` now paginates results
  - Shows 8 notes per page by default
  - Use `/note list 2` to see page 2, `/note list 3` for page 3, etc.
  - Displays current page and total pages at the bottom
  - Preview shows only first line with `│` prefix for clarity
  - Validates page number and shows error for invalid pages

- **Note Show Markdown Rendering** - `/note show` now renders markdown content
  - Uses termimad for proper markdown formatting
  - Header metadata formatted as markdown with bold labels
  - Content rendered with full markdown support

- **Note Add Parsing Fixed** - `/note add` now handles complex arguments correctly
  - Multi-word titles with quotes: `/note add content --title "Title with spaces"`
  - Escaped dashes: `\-\-` is converted to `--` literal
  - Newlines in quoted content: `"Line 1\nLine 2"` expands `\n` to real newlines
  - Title validation: rejects newlines in title field
  - Quote stripping: removes surrounding quotes from content properly

- **Session Load by Name** - `/session load` now finds sessions by name or ID
  - First tries exact ID match
  - Falls back to name (title) match
  - Fixes "Query returned no rows" error after `/session save <name>`

- **Session List Current Marker** - `/session list` now shows current session
  - Current session marked with `→` arrow
  - Other sessions shown with space prefix
  - Helps identify which session is active

- **Page Number Validation** - `/note list` validates page numbers
  - Shows error for page < 1: "Page must be >= 1"
  - Shows error for page > total: "Page X does not exist. Total pages: Y."
  - Provides guidance: "Use /note list Y."

- **Embedding Regeneration System** - Post-migration embedding recovery
  - New `regenerate_all_embeddings()` function for schema migrations
  - `RegenerationStats` struct tracks processed/failed items
  - Shows progress bar during regeneration with ETA
  - Aborts gracefully on Ollama connection errors with recovery instructions

### Changed

- **`/clear` renamed to `/new`** - Command now starts a new conversation session
  - Previous behavior: Cleared in-memory messages but reloaded from database on restart
  - New behavior: Creates new session ID, clears all session state
  - Previous conversations remain searchable via `/search` and `remember()`
  - `/new` generates session ID: `session-{timestamp}`
  - Alias: `/n`

- **`/load` Auto-save** - Automatically saves current session before loading another
  - If current session has messages, it's saved before switching
  - Prevents accidental loss of conversation when switching sessions

- **Session Auto-Load** - Automatically loads the most recent session on startup
  - Sessions are ordered by `updated_at DESC` to find the most recent
  - If no sessions exist, starts a fresh session in memory

- **Query Pattern Refactoring** - Dynamic SQL WHERE clause construction
  - Created `WhereBuilder` utility for parameterized queries
  - Eliminated 4-8 SQL variants per function into single dynamic query
  - `list_notes`: 4 variants → 1 query (50 lines → 20 lines)
  - `search_notes_keyword`: 4 variants → 1 query (95 lines → 35 lines)
  - `list_facts`: 8 variants → 1 query (80 lines → 25 lines)
  - SQL constants extracted to centralized locations for maintainability
  - Removed `#[allow(unused_imports)]` - `fts5_escape` actively used in 3 modules

- **Database Module** - `get_storage_path()` made public for error diagnostics

## [0.35.0] - TBD

### Fixed

- **Context Display After Compaction** - Correct token count after session reload
  - `prompt_tokens` is now cleared in database after compaction
  - Previously, old token counts persisted causing incorrect context display (e.g., 92% instead of 1%)
  - Added `clear_conversation_prompt_tokens()` method to database operations
  - Applies to both auto-compact and manual `/compact` commands

### Changed

- **REPL Complexity Reduction** - Major refactoring of `run_chat_repl` for maintainability
  - Cognitive complexity reduced from **78/25 to eliminated** (no warning)
  - Extracted `handle_command_result()` - dispatches all command results (~100 lines)
  - Extracted `handle_model_switch()` - centralized model switching logic (~30 lines)
  - Moved `print_context_info()` from `repl.rs` to `command_handlers.rs` (~165 lines)
  - Extracted `handle_user_message()` - user input processing (~50 lines)
  - Extracted `create_session()` - session initialization (~75 lines)
  - Extracted `resolve_session_model()` - model validation (~25 lines)
  - Extracted `resolve_thinking_mode()` - thinking mode logic (~30 lines)
  - Extracted `init_database()` - database/embedding client init (~25 lines)
  - Extracted `run_startup_tasks()` - migration and decay cycle (~30 lines)
  - New module `src/chat/continuation.rs` with continuation handling functions
  - `repl.rs` reduced from ~1090 lines to ~540 lines
  - `command_handlers.rs` now includes `HandleResult` enum for dispatch

### Refactoring

- **Code Organization** - Improved module structure
  - All command handlers now use `ReplState` consistently
  - Command dispatch centralized in `handle_command_result()`
  - Removed duplicate code patterns from main REPL loop

## [0.34.0] - 2026-03-16

### Added

- **TODO System Activation** - Task tracking for LLM and users
  - LLM tools: `todo_add`, `todo_list`, `todo_update`, `todo_clear_done`, `todo_clear_all`
  - User commands: `/todo add`, `/todo list`, `/todo update`, `/todo clear-done`, `/todo clear-all`
  - Shortcuts: `/ta`, `/tl`, `/tu` for quick access
  - Task statuses: `pending`, `in_progress`, `done`
  - Session persistence: TODOs saved/restored with chat session
  - System prompt integration: Active tasks injected into LLM context
  - Global state sync: Tools and commands share same TodoState

## [0.33.0] - 2026-03-16

### Added

- **Factual Memory System** - Persistent fact storage with automatic decay and conflict resolution
  - LLM tools: `fact_add`, `fact_search`, `fact_remove` for autonomous fact management
  - User commands: `/fact add`, `/fact list`, `/fact search`, `/fact remove`, `/fact prune`
  - Auto-classification: Preferences vs facts detected by heuristics
  - Conflict resolution: Duplicate detection and contradiction handling
  - Decay: Ebbinghaus forgetting curve (180d preferences, 30d facts)
  - Scope: Project-specific vs global facts
  - FTS5: Full-text search for facts
  - Prompt injection: Facts injected into system prompt with usage instructions (max 2200 chars)

- **Chat Architecture Refactoring** - Preparing for TUI migration
  - `InputBackend` trait - abstracts input handling (rustyline/ratatui)
  - `ChatView` trait - abstracts output rendering
  - `ReplState` struct - consolidates mutable REPL state
  - `core.rs` module - extracted business logic from `repl.rs`
  - Layers: Input/View traits → Session → Implementations → State → Core → REPL
  - Moved ~600 lines from `repl.rs` to `core.rs` for maintainability

### Changed

- **Prompts Centralization** - All prompts now centralized in `src/prompts/`
  - Moved `build_continuation_prompt()` from `core.rs` to `prompts/builder.rs`
  - Added `COMPACTION_PROMPT` constant for conversation summarization
  - Added `CONTINUATION_PROMPT_TEMPLATE` for continuation after compaction
  - New functions: `build_compaction_prompt()`, `build_continuation_prompt()`
  - Eliminated ~50 lines of duplicated prompt templates from `core.rs`
  - Easier maintenance: all prompt templates in one location

### Fixed

- **Error Recovery for Tool Calls** - LLM now receives parsing errors for self-correction
  - Replaced string-based error classification with typed `OllamaError` matching
  - `JsonError` (JSON/XML parsing failures) now marked as recoverable
  - Errors from malformed tool calls are sent back to LLM as Tool messages
  - LLM can self-correct when it generates invalid tool call syntax
  - Removed unreliable heuristics (`is_error_str_recoverable`) in favor of types

- **BM25 Score Normalization for Conflict Detection** - Fixed incorrect similarity scoring
  - Previous formula `(-score).max(0.0)` didn't normalize to [0,1] range
  - New formula `(-score)/(1-score)` properly maps BM25 scores to [0,1]
  - Score -10 (strong match) → 0.91, score -1 (weak match) → 0.50
  - Adjusted CONFLICT_THRESHOLD from 0.8 to 0.85 after proper normalization
  - Added `normalize_bm25_score()` helper function

## [0.32.1] - 2026-03-13

### Fixed

- **Embedding Recovery for Long Messages** - Fixed crash when recovering embeddings
  - Recovery now checks if message needs chunking before embedding
  - Long messages are split into chunks, each chunk gets its own embedding
  - Messages that already have chunks are skipped (embeddings in chunks)
  - Prevents "input length exceeds context length" errors on startup

## [0.32.0] - 2026-03-13

### Added

- **File Write Tools** - Three new tools for creating, editing, and appending to files
  - `write_file` - Create or overwrite files with sandbox enforcement
  - `edit_file` - Surgical edits (replace text, insert lines, delete lines)
  - `append_file` - Append content to existing files
  - Security: Sandbox parameter respected, but blocked patterns ALWAYS enforced
  - Security: Blocked patterns for sensitive files (`.env`, `secrets`, SSH keys, certificates)
  - Security: Maximum 5MB per write operation
  - Security: Atomic writes using temp file + rename pattern
  - Optional `create_backup` parameter for `edit_file`

- **Blocklist Module** - Shared security module for file operations
  - `is_blocked_for_read()` - Check if path matches blocked patterns for read operations
  - `is_blocked_for_write()` - Check if path is blocked for write operations (always enforced)
  - `is_blocked_for_list()` - Check if filename should be hidden in directory listings
  - `BlocklistConfig` - Loads configuration from `tools.toml`
  - Integrated into all file operations: `read_file`, `read_file_segment`, `count_lines`, `search_files`, `list_directory`

- **File Tools Configuration** - Full TOML configuration integration
  - `[file-tools]` section in `~/.config/sprachspiel/tools.toml`
  - `max_file_size` - Maximum file size (default: 5MB)
  - `blocked_patterns` - Additional glob patterns to block
  - `block_read` - Block reading sensitive files (default: true)
  - `block_list` - Hide blocked filenames in listings (default: false)
  - `load_file_tools_config()` - Fully implemented configuration loader

- **Positive Framing in Prompts** - Updated all prompts to use positive instructions
  - `PERSONALITY_DEFAULT` converted from "**Does not:**" to "**Maintains:**" format
  - All SOUL.md example personalities updated (SPRACH, PEPE, ANGEMON)
  - Added documentation section on positive framing best practices

### Changed

- **File Operations** - Now 8 tools instead of 5 (3 new write tools added)
- **Tool Count** - Updated from 28 tools to 50 tools (8 file + 9 pokemon + 3 weather + 1 calc + 2 serper + 2 system + 3 search + 1 finance + 2 run_command + 3 facts + 1 notes + 1 documents + 5 todo + 2 skills + 1 remember + 5 LED + 1 misc + 1 tool_check)
- **Documentation** - Updated `doc/src/tools.md` with write tool documentation and security section
- **Documentation** - Added "Use Positive Framing" section to `doc/src/soul.md`
- **Tests** - `test_negative_instructions_in_prompts` now uses `with_soulless(true)` to test only built-in prompts

### Technical Debt

- **Code Cleanup** - Removed dead code and improved maintainability
  - Removed unused `ChatEvent::FinalResponse` and `ChatEvent::ContinuationNeeded` variants
  - Removed unused ephemeral methods (`take_ephemeral`, `has_ephemeral`, `clear_ephemeral`)
  - Fixed indentation issues in `send_message` function
  - Extracted helper functions from `send_message` to reduce complexity:
    - `build_session_system_prompt()` - constructs system prompts
    - `setup_coordinator()` - creates and configures coordinator
    - `prepare_messages()` - builds message context with retrieval
    - `process_chat_response()` - converts response to result

- **`run_chat_repl` function** remains large (~1100 lines) - refactoring planned for Priority 3

## [0.31.0] - 2026-03-12

### Added

- **Context Continuity with Graceful Interruption** - Full implementation of LLM pause/resume during context overflow
  - `ContextStatus` injected into prompts when approaching limits (>72% usage)
  - `CONTEXT_MANAGEMENT_INSTRUCTION` teaches LLM to emit `<continuation_needed>` tag
  - `ContinuationTag` struct for parsing pause/checkpoint information
  - `parse_continuation_tag()` function extracts and strips continuation tags from responses
  - `ephemeral_messages` in `CustomCoordinator` for non-persisted continuation prompts
  - `SendMessageResult.continuation_needed` field for continuation detection
  - `build_continuation_prompt()` creates resume instructions from checkpoint
  - Continuation loop in REPL automatically resumes after compaction
  - Supports nested continuations (up to 3) for extreme context pressure
  - Merges continuation responses with original for seamless output

- **Prompt Configuration**
  - `PromptConfig.context_status` field for injecting context usage
  - Context status section shows usage % and critical/warning indicators
  - Context management instructions when overflow is detected

### Fixed

- **Landlock Sandbox E2BIG Error** - Fixed crash when running multiple commands
  - Added thread-local tracking to prevent stacking Landlock rulesets
  - E2BIG error now treated as success (thread already sandboxed)
  - Documented 16-layer limit in Kernel Landlock API
  - Prevents "Argument list too long" errors after ~16 command executions

### Changed

- `ContextStatus::max_tokens()` - New method to get context window size
- `build_request()` in `CustomCoordinator` now prepends ephemeral messages
- `send_message()` now accepts optional `continuation_tag` parameter for resume
- REPL continuation handling merges responses and accumulates token metrics

## [0.30.0] - 2026-03-12

### Added

- **PreToolContent Persistence** - Intermediate assistant messages (generated before tool calls) are now saved for semantic search
  - `SavedMessage.message_type` field distinguishes `"normal"` vs `"pre_tool_content"` messages
  - `previous_message_id` links pre-tool content back to the user question
  - `subsequent_messages` in search results shows follow-up messages contextually
  - Navigation hints in `remember` tool output

- **Database Schema v5** - New columns for message metadata
  - `message_type TEXT DEFAULT 'normal'` - Distinguishes normal vs intermediate messages
  - `previous_message_id INTEGER` - Links assistant messages to preceding user message

- **Session Methods**
  - `add_pre_tool_message()` - Stores pre-tool content with `previous_message_id` linkage
  - `add_user_message()` now returns `Option<i64>` (message ID) for linking

- **Database Methods**
  - `update_message_previous_id()` - Sets previous_message_id for navigation
  - `get_conversation_messages()` now includes `message_type` column

- **MEMORY TOOLS Navigation** - Enhanced prompt section with navigation instructions
  - Explains `previous_message_id` and `subsequent_messages` fields
  - Guides LLM on contextual message navigation

### Changed

- **`remember` Tool Output** - Shows `message_type` indicator for intermediate messages
  - `[Intermediate]` prefix for `pre_tool_content` messages
  - Subsequent messages displayed with proper indentation

- **`get_conversation_messages()`** - Now retrieves `message_type` column from database

- **Retrieval Enrichment** - `subsequent_messages` includes `message_type` for each message

## [0.29.0] - 2026-03-11

### Breaking Changes

- **SOUL.md Personality System** - User-configurable agent personality replaces hardcoded Pepe personality
  - `~/.config/sprachspiel/SOUL.md` defines agent identity, behavior, and limits
  - Falls back to `PERSONALITY_DEFAULT` when no SOUL.md exists
  - Use `--soulless` flag to skip personality entirely
  - **Removed:** Pepe personality (`PERSONALITY_PEPE`) - users should create their own SOUL.md

### Added

- **SOUL.md Module** (`src/soul.rs`)
  - Loads personality from `~/.config/sprachspiel/SOUL.md` or `XDG_CONFIG_HOME/sprachspiel/SOUL.md`
  - Removes HTML comments (`<!-- ... -->`) for developer notes
  - Normalizes whitespace
  - Validates structure (requires at least one `## ` section)

- **PERSONALITY_DEFAULT** - Fallback personality when SOUL.md is missing

- **`--soulless` CLI Flag** - Skip personality layer for neutral responses
  - Available for `chat` and `query` commands
  - Useful for debugging or when personality is not desired

- **Multiple Personalities** - Documentation for switching between personality files
  - See `doc/src/soul.md` for example personalities (PEPE, SPRACH, ANGEMON)
  - Symlink or copy approach for switching

- **Example Personalities** - Three complete example personalities in documentation:
  - **SPRACH** - Cognitive companion for research and Zettelkasten work
  - **PEPE** - Sarcastic senior developer (replaces hardcoded Pepe)
  - **ANGEMON** - Guardian administrator for system protection

- **Documentation** - New `doc/src/soul.md` with examples and best practices
  - Updated `doc/src/commands/chat.md` and `doc/src/commands/query.md`

### Changed

- **Prompt Assembly** - New layered architecture:
  1. SOUL LAYER (SOUL.md or PERSONALITY_DEFAULT or empty if --soulless)
  2. OPERATION LAYER (Role + Behavior + Tool Usage)
  3. CONTEXT LAYER (Platform + System + AGENTS.md)
  4. CAPABILITY LAYER (Tools + Memory + Examples)
  5. FINAL INSTRUCTION

- **Removed `src/prompts/personality.rs`** - Pepe personality code deleted

- **Updated `src/prompts/mod.rs`** - Removed personality exports, added PERSONALITY_DEFAULT export

### Migration Guide

If you used Pepe personality before, create `~/.config/sprachspiel/SOUL.md` with your desired personality.

Example personalities are available in `doc/src/soul.md`:
- **SPRACH** - Thoughtful research companion
- **PEPE** - Sarcastic senior developer
- **ANGEMON** - Guardian administrator

See the [SOUL.md documentation](./soul.md) for complete examples and best practices.

## [0.28.0] - 2026-03-11

### Fixed

- **CRITICAL: run_command Parameter Types** - Fixed crash when LLM sends strings for numeric parameters
  - Changed `head`, `tail`, `timeout_seconds` from `Option<usize>`/`Option<u32>` to `Option<String>`
  - LLMs frequently send `"null"` (string) instead of `null` (JSON), causing deserialization failures
  - Internal parsing with `.parse().ok()` handles all variations (`"5"`, `5`, `"null"`, `null`, `""`)
  - Added `"CRITICAL: Parameter Types for LLM Tools"` section to AGENTS.md
  - Updated doc/src/tools.md with guidance on parameter types

- **run_command Timeout Implementation** - Processes now properly killed on timeout
  - Replaced `std::process::Command` with `tokio::process::Command`
  - Added `.kill_on_drop(true)` to ensure process termination
  - Implemented `tokio::time::timeout` wrapper
  - Timeout error messages include actionable suggestions
  - Added unit tests for timeout functionality and string parameter parsing

- **Landlock API Deprecation** - Updated to new Ruleset API
  - Changed `Ruleset::new()` to `Ruleset::default()` (deprecated warning fixed)

### Changed

- **Code Cleanup** - Removed unused code and fixed all clippy warnings
  - Deleted `src/external/executor.rs` (CommandExecutor never used)
  - Deleted `src/external/registry.rs` (only used by executor.rs)
  - Removed `CommandError` and `ToolAvailability` unused types from types.rs
  - Fixed 15+ clippy warnings: `collapsible_if`, `needless_question_mark`, `redundant_locals`, `map_clone`, `let_and_return`, `io_other_error`, `needless_borrow`, `manual_clamp`, `redundant_async_block`

- **API Refactoring** - Improved function signatures for maintainability
  - Created `SearchParams` struct for `search_hybrid()` parameters (9 args → 1 struct)
  - Created `ConversationMetadataParams` struct for `update_conversation_metadata()` (10 args → 1 struct)
  - Both structs exported from `src/db/mod.rs` for external use

### Added

- **Documentation** - Added LLM tool parameter type guidelines
  - `AGENTS.md`: New section "CRITICAL: Parameter Types for LLM Tools" explaining why `Option<String>` is required
  - `doc/src/tools.md`: Updated Tool Error Handling section with guidance
  - Reference tables showing dangerous vs correct parameter types

- **Unit Tests** - Test coverage for timeout and parameter parsing
  - `test_timeout_kills_long_running_command` - Verifies process killed on timeout
  - `test_timeout_allows_fast_command` - Verifies normal execution within timeout
  - `test_timeout_error_message_format` - Verifies error message structure
  - `test_string_parameter_parsing` - Verifies string-to-number conversion

- **Code Organization** - SQLite cleanup
  - Created `src/project.rs` with `get_project_id()` and `normalize_git_url()`
  - Updated `history.rs` to be purely a migration module (deprecated)
  - Clear separation: project identification vs. legacy storage
  - `history.rs` still contains `ConversationStorage` for `/restore` command

- **User Documentation** - Updated storage model
  - `doc/src/commands/chat.md`: Updated session storage documentation
  - Clarified SQLite as primary storage, JSON for backup/restore only
  - Added `/restore` command documentation
  - Added database tables explanation

## [0.27.3] - 2026-03-09

### Added

- **Compaction Visual Indicator** - Clear feedback during context compaction
  - Shows yellow "⏳ Compacting context (X% full)..." before starting
  - Shows dimmed result: "[auto/urgent-compacted: N messages summarized]"
  - `/compact` command now shows checkmark "✓ Compacted" on success
  - Error messages show "✗ Compaction failed" in red

### Fixed

- **Context Not Cleared After /compact** - Token count now correctly reflects reduced context
  - `prompt_tokens` are cleared from messages after compaction
  - Next interaction will have fresh token count
  - `/context` now shows accurate reduced token usage after `/compact`

- **Markdown in Compaction Summary** - Summary now renders in markdown format
  - Compaction prompt requests structured markdown output
  - Uses `print_markdown()` for proper rendering in terminal
  - Sections include: Key Topics, Decisions Made, Technical Details, Action Items

- **Web Scraping Content Quality** - Improved HTML-to-markdown conversion
  - Extracts main content area (`<main>`, `<article>`, etc.) when available
  - Prioritizes semantic content over navigation/sidebars
  - Limits content to 50,000 characters to prevent memory issues
  - Safe UTF-8 boundary handling for Unicode content
  - Shows "(truncated)" indicator when content is limited

### Changed

- **Roadmap Reorganization** - Memory Enhancement Phases 2-3 moved to Blocked
  - Phase 2 (Query Routing) blocked by Document Import Tool + Notes System
  - Phase 3 (Timestamp Filtering) blocked by Phase 2
  - New priorities: Document Import Tool and Notes System first
  - Chat Module Integration renamed to Specialized Agent Architecture (P4)

## [0.27.1] - 2026-03-09

### Added

- **Automatic JSON Migration** - One-time automatic migration on startup
  - Detects all JSON sessions in `~/.local/share/sprachspiel/conversations/`
  - Migrates sessions not yet in SQLite (with embeddings)
  - Archives ALL JSON files to `~/.local/share/sprachspiel/archived/`
  - Removes empty project directories
  - Does NOT touch `OLD/` directory

### Changed

- **SQLite-Only Storage** - Removed dual-write to JSON files
  - `/save` and `/load` now use SQLite exclusively
  - Removed `/migrate` command (automatic migration replaces it)
  - `/restore <id>` imported from JSON as disaster recovery option
  - **`should_force_retrieve()` logic rewritten**
    - Old: Only triggered when session.messages.is_empty()
    - New: Triggers when DB count > session count (after /clear with new messages)
    - Also triggers when session is empty AND has compacted_summary

### Fixed

- **Token Count Bug** - Fixed incorrect token calculation in `/context` display
  - `history_real_tokens()` now uses the LAST cumulative `prompt_tokens` value (Ollama's `prompt_eval_count`)
  - Previous code incorrectly SUMMED all `prompt_tokens` values, causing ~184K tokens when actual was ~22K
  - `check_context_overflow()` now correctly handles fallback path (includes tools estimate)
  - Context status simplified to "OK", "MODERATE", "CRITICAL" (removed confusing "auto-compact triggered")

- **Token Persistence** - Added `prompt_tokens` column to messages table
  - Messages now store `prompt_eval_count` from Ollama responses
  - Token counts persist across sessions
  - `/context` shows accurate token usage on startup

- **CRITICAL: Retrieval after /clear now works!**
  - Bug: `should_force_retrieve()` checked if session was empty, but user already added 1+ messages
  - Fix: Compare DB message count vs session message count
  - If DB has more messages than session, retrieval is forced
  - This correctly handles: `/clear` → user asks question → retrieval happens

### Removed

- **Dead Code Cleanup**
  - Removed `migrate_project()` function (replaced by automatic migration)
  - Deprecated `Session.save()` (JSON) in favor of `Session.save_sqlite()`

## [0.26.8] - 2026-03-09

### Fixed

- **Context Utilization After Compaction** - Fixed token count calculation after `/compact`
  - `history_real_tokens()` now skips compacted messages
  - `check_context_overflow()` now respects `messages_sent_to_llm`
  - `/context` display now shows correct active messages and summary tokens
  - Context utilization bar reflects post-compaction state

### Details

Before this fix, `/context` showed incorrect token counts after compaction:
- Counted ALL messages (including compacted ones)
- Showed 100%+ utilization even after successful compaction
- Displayed wrong message count

Now correctly calculates:
- Tokens from summary + active messages only
- Skips messages before `messages_sent_to_llm`
- Shows summary token estimate in output

## [0.27.0] - PLANNED

### Changed

**SQLite as Single Storage** - Major architecture change

This release consolidates session storage from dual (JSON + SQLite) to SQLite-only, improving reliability and eliminating data synchronization issues.

#### Architecture Changes

- **Session storage** - Moved from JSON files to SQLite database
  - Session metadata (model, think, tools, system_prompt) now in `conversations` table
  - Todo list state now in `session_todos` table
  - Compaction metadata (summary, range) persisted in database
  - Messages remain in SQLite (no change)

- **Commands updated** - All session commands now use SQLite
  - `/save` - Saves to SQLite only (no more JSON)
  - `/load` - Loads from SQLite only
  - `/list` - Queries SQLite for sessions
  - `/forget` - Deletes from SQLite only

#### New Features

- **`/restore <file>`** - Restore session from JSON backup
  - Imports backup files (from `/export json`)
  - Deletes JSON after successful import
  - Useful for disaster recovery

- **Legacy session detection** - Automatic notification on startup
  - Detects uncommitted JSON sessions
  - Prompts to use `/restore` command

#### Removed

- **`/migrate` command** - Replaced by `/restore` and auto-detection
  - Legacy JSONs now imported via `/restore <file>`
  - Automatic detection on startup replaces manual migration

#### Schema Changes

- Added `conversations` table columns:
  - `system_prompt TEXT`
  - `compacted_summary TEXT`
  - `compacted_range_start INTEGER`
  - `compacted_range_end INTEGER`
  - `think INTEGER DEFAULT 0`
  - `tools INTEGER DEFAULT 1`
  - `tool_output_level TEXT DEFAULT 'compact'`

- Added `messages` table column:
  - `prompt_tokens INTEGER`

- Added `session_todos` table for task tracking

#### Benefits

- **Reliability** - ACID transactions prevent data corruption
- **Consistency** - Single source of truth, no sync issues
- **Performance** - SQLite faster than filesystem writes
- **RAG Access** - Compacted messages remain searchable

#### Migration

Users with existing JSON sessions will see a notification:
```
[!] Found 3 uncommitted session(s): session1, session2, default
[!] Use /restore <file> to import them.
```

---

## [0.26.7] - 2026-03-09

### Changed

- **Dead Code Cleanup** - Removed unused code for better maintainability
  - Removed `MIN_PRESERVE_LAST` constant from `context_overflow.rs` (only used in tests, now local)
  - Removed unused `count_embedded_messages()` from `db/operations.rs`
  - Removed unused `get_message_chunks()` and `ChunkRow` struct from `db/operations.rs`
  - Removed legacy `set_compacted_summary()` from `chat/session.rs` (replaced by `set_compacted_summary_with_range()`)
  - Removed unused `clear_compacted_summary()` from `chat/session.rs`
  - Removed duplicate `as_chat_messages()` from `chat/session.rs` (same as `get_messages_for_llm()`)
  - Removed unused `set_todo_state()` and `get_todo_copy()` from `tools/todo.rs`
  - Converted test-only methods to `#[cfg(test)]` in `chat/todo_state.rs` (`get()`, `all()`, `count()`, `is_empty()`)
  - Removed `#[allow(dead_code)]` from `to_info()` in `chat/session.rs` (actually used in `history.rs`)

### Notes

- Functions and structs with roadmap justifications were kept:
  - `list_conversations()` - planned for `/reindex all`
  - `embed_batch()`, `with_model()`, `model()`, `embedding_dimension()` - planned for future use
  - `normalize()`, `cosine_similarity()` - planned for MMR/reranking

## [0.26.6] - 2026-03-08

### Added

- **Integration Tests for Context Overflow** - Comprehensive test coverage for overflow protection
  - `tests/context_recovery_flow.rs` - 9 integration tests
  - `tests/context_tool_overflow.rs` - 13 integration tests
  - Tests for threshold hierarchy, Unicode truncation, recovery cycles
  - Tests for message removal, turn preservation, multiple recovery cycles

### Fixed

- **Context Builder Panic After /compact + /clear** - Session crash fixed
  - `clear_messages()` now resets `compacted_range` to prevent stale indices
  - Added bounds checking in `context_builder.rs` with `.min(session.messages.len())`
  - Prevents "range end index X out of range for slice of length Y" panic

## [0.26.5] - 2026-03-08

### Added

- **Error Recovery During Tool Execution** - Automatic recovery from context overflow
  - Detects "Context overflow during tool execution" error from coordinator
  - Removes failed assistant messages from session
  - Auto-compacts immediately after error
  - Saves session after recovery
  - Prompts user to retry with clear message

- **Pre-Tool Context Check** - Proactive compaction before tool execution
  - Checks context at 75% threshold before creating coordinator
  - Auto-compacts if needed to prevent overflow during tools
  - User message preserved during compaction (already saved)
  - Prevents context exhaustion during multi-tool turns

- **Turn Preservation in Compaction** - Current turn never compacted
  - `MIN_PRESERVE_LAST` constant ensures at least 1 message preserved
  - User message saved before pre-tool check runs
  - Compaction preserves `DEFAULT_KEEP_LAST = 5` recent messages

### Fixed

- **/undo Incomplete Cleanup** - Embeddings now deleted from database
  - Added `delete_last_messages()` function in Database
  - `/undo` calls database cleanup for both messages and embeddings
  - Prevents orphaned embeddings in SQLite

- **User Prompt Included in Hybrid Search** - Current prompt excluded
  - Added `exclude_ids` parameter to `search_hybrid()`
  - Prepared for future use (not yet wired - current message not in DB at search time)

- **Code Mode (-c Flag) Not Working in Chat** - Now functional
  - `cli_code` parameter passed through to `run_chat_repl()`
  - Code mode now correctly disables retrieval and uses code prompts

## [0.26.4] - 2026-03-08

### Added

- **Token Estimation in Coordinator** - Context overflow detection during tool execution
  - `estimate_messages_tokens()` for SavedMessage (session history)
  - `estimate_chat_messages_tokens()` for ChatMessage (coordinator history)
  - `context_window` and `system_prompt` fields added to CustomCoordinator
  - Context check in `process_next()` at 90% threshold
  - Returns clear error when overflow detected during tools

- **Unicode-Safe Tool Result Truncation** - Prevents unbounded context growth
  - `truncate_tool_result()` with `.chars().take()` for Unicode safety
  - `MAX_TOOL_RESULT_TOKENS = 4000` limit for tool results
  - `CHARS_PER_TOKEN = 4` conservative ratio
  - Truncation notice includes original token count
  - Debug logging when truncation occurs

- **Unit Tests** - 7 new tests in `src/context_overflow.rs`
  - Token estimation accuracy tests
  - Unicode truncation tests (Japanese, Chinese, Arabic, Emoji)
  - Threshold hierarchy tests
  - Context status percentage tests

## [0.26.3] - 2026-03-08

### Fixed

- **Multiple Bug Fixes**:
  - `/undo` now deletes embeddings from database (not just memory)
  - Fix crash after `/compact` + `/clear` (reset compacted_range on clear)
  - Add bounds checking in context_builder to prevent panics
  - Code mode (-c flag) now works in chat mode
  - Hybrid search supports `exclude_ids` parameter (prepared for future use)

### Added

- **delete_last_messages()** in Database for proper cleanup

## [0.26.2] - 2026-03-05

### Fixed

- **remember() tool ID display** - Fixed missing source type prefix in search results
  - IDs now properly display as `msg:N` instead of just `N`
  - Affects query results, specific message retrieval, and error messages
  - Prevents confusion when AI tries to use returned IDs in subsequent calls

## [0.26.1] - 2026-03-05

### Added

- **Centralized String Constants Module** - `src/consts/`
  - `roles.rs` - Message role constants (`ROLE_USER`, `ROLE_ASSISTANT`, `ROLE_SYSTEM`, `ROLE_TOOL`)
  - `api.rs` - API URL constants (`OPEN_METEO_BASE`, `OPEN_METEO_GEOCODING`, `SERPER_API_URL`)
  - Helper functions: `format_role_label()`, `format_role_label_md()`

- **AGENTS.md Guidelines** - New "Constants and String Management" section
  - Rules for preventing string duplication
  - Categories of regulated strings (roles, source prefixes, API URLs)
  - Checklist for adding new string literals
  - Policy for rejecting `#[allow(dead_code)]` on constants

### Changed

- **Refactored 11 files to use centralized constants**
  - `retrieval/context_builder.rs` - Use `SourceType::prefix()` dynamically
  - `tools/remember.rs` - Use `format_role_label()` and role constants
  - `retrieval/search.rs` - Use `format_role_label_md()`
  - `tools/weather.rs` - Use `OPEN_METEO_BASE/GEOCODING` constants
  - `tools/serper.rs` - Use `SERPER_API_URL` constant
  - `db/operations.rs` - Use `ROLE_USER/ASSISTANT` constants
  - `db/migration.rs` - Use role constants
  - `chat/session.rs` - Use `ROLE_USER/ASSISTANT` constants
  - Test files updated accordingly

## [0.26.0] - 2026-03-04

### Added

- **Improved Distribution System** - Easier installation for Linux and Termux
  - One-liner installation via curl|bash
  - Intelligent install script with automatic platform detection
  - Manpage installation support for Termux
  - Detailed installation instructions in README-TERMUX.txt

- **Installation Scripts**
  - `scripts/install.sh` - Portable installer with --prefix, --bin, --man options
  - `scripts/uninstall.sh` - Clean uninstallation
  - `scripts/install-sprach.sh` - Remote installer for curl|bash one-liner

- **New Makefile Targets**
  - `tarball-linux` - Linux tarball with install scripts
  - `tarball-termux` - Termux tarball with README-TERMUX.txt
  - `all-tarballs` - Create all distribution tarballs

- **Documentation Improvements**
  - Consolidated version history into `implementation-history.md`
  - Integrated `/search` and `/context` commands into `chat.md`
  - Updated installation documentation with one-liner instructions
  - Added architecture diagrams to `architecture.md`
  - New `retrieval-design.md` explaining hybrid search

### Changed

- **Tarball Structure** - Now includes install/uninstall scripts inside
  - `sprach.1` manpage renamed from `man/ask-ai.1` to top-level
  - Added `README-TERMUX.txt` for Termux tarballs
  - Install script detects Termux and adjusts default paths

- **Documentation Cleanup**
  - Removed obsolete version plan files (v0.22.x, v0.23.0, v0.24.0, v0.25.0)
  - Removed `context_v2_plan.md` (superseded by `context_composition_design.md`)
  - Removed `markdown_skin_plan.md` (completed, documented elsewhere)
  - Removed separate `search.md` and `context.md` (integrated into `chat.md`)

### Fixed

- **Translate Model Configuration** - Fixed "Translate model configuration not found"
  - Added `translate: SubcommandModelConfig` to settings
  - Falls back to "translategemma" builtin model
  - Sample config now includes `[model.translate]` section

- **Code Cleanup**
  - Removed duplicate `is_led_configured()` function in led.rs
  - Removed unused `SummarizeArgs::get_text()` method
  - Added `normalize_input()` utility for unicode-safe lowercase+trim
  - Fixed stdin duplication by consolidating to `utils::read_stdin()`

### Technical Details

**Install Script Features:**
- Platform detection: Linux, Termux, macOS
- Default paths: `~/.local/bin` (Linux/macOS), `~/bin` (Termux)
- Manpage installation: `~/.local/share/man/man1/sprach.1`
- PATH/MANPATH detection and instructions
- Manpage access verification

**Tarball Contents:**
```
sprachspiel-VERSION-linux-x86_64.tar.gz
├── sprach
├── sprach.1
├── install.sh
├── uninstall.sh
├── README.md
└── LICENSE.txt
```

**One-liner Installation:**
```bash
curl -sL https://raw.githubusercontent.com/luksamuk/sprachspiel/main/scripts/install-sprach.sh | bash
curl -sL ... | bash -s -- --version 0.26.0
curl -sL ... | bash -s -- --tools all
curl -sL ... | bash -s -- --prefix /usr
```

### Files Modified

- `Cargo.toml` - Version bump to 0.26.0
- `Makefile` - New tarball targets with install scripts
- `README.md` - Reorganized installation section
- `doc/src/installation.md` - New installation methods
- `doc/src/commands/chat.md` - Integrated /search and /context
- `doc/src/SUMMARY.md` - Updated structure
- `doc/src/development/implementation-history.md` - NEW: Consolidated decisions
- `doc/src/development/architecture.md` - Major rewrite with diagrams
- `doc/src/development/retrieval-design.md` - NEW: Retrieval system design
- `scripts/install.sh` - NEW: Portable installer
- `scripts/uninstall.sh` - NEW: Uninstaller
- `scripts/install-sprach.sh` - NEW: One-liner installer
- `README-TERMUX.txt` - NEW: Termux-specific instructions
- `src/settings.rs` - Added translate model config
- `src/main.rs` - Translate model fallback
- `src/utils.rs` - Added normalize_input()
- `src/summarize/cli.rs` - Removed dead code
- `src/tools/led.rs` - Removed duplicate, use normalize_input()
- `src/tools/calc.rs` - Use normalize_input()
- `src/translate/style.rs` - Use normalize_input()

## [0.25.0] - 2026-03-03

### Added

- **Project-Aware Query Mode** - Query mode now retrieves context from project history
  - Access to all conversations in the project (read-only)
  - Same RAG retrieval as chat mode
  - Same 5-message context limit via RRF
  - Same enrichment with assistant responses

### Changed

- `query` and `legacy` modes initialize DB + EmbeddingClient
- `search_hybrid()` now accepts `project_id` parameter for project-wide search
- Prompt includes MEMORY section when retrieval is available
- `--code` continues without DB/history (unchanged)

### Technical Details

**Problem:** Query mode had no access to conversation history, making it less useful
for quick questions that benefit from project context.

**Solution:** Enable retrieval from project's conversation history using the same
RAG system as chat mode, but without persisting new messages.

**Implementation:**
1. `project_id` determined same way as chat (git remote or folder name)
2. DB + EmbeddingClient initialized for query (except --code)
3. `build_query_context()` retrieves from all sessions in project
4. Task-local context enables remember tool in query
5. Graceful degradation if DB unavailable

**Example:**
```
Query before: [system_prompt] + [user_query]
Query after:  [system_prompt] + [retrieved_context] + [user_query]
                            ↑ from project history (read-only)
```

### Files Modified

- `src/db/operations.rs` - Add `project_id` to `search_hybrid()`
- `src/retrieval/context_builder.rs` - New `build_query_context()` function
- `src/query.rs` - Initialize DB, use context, task-local for remember tool

## [0.24.0] - 2026-03-03

### Added

- **Conversation-Aware Retrieval** - Enrich retrieved user messages with assistant responses
  - `next_message` field in SearchResult for user messages
  - `get_next_message_by_role()` database method
  - `enrich_with_context()` to attach assistant responses to user questions
  - Both auto-context and remember tool use the same enrichment
  - `/search` command shows question-answer pairs together

### Changed

- `SearchResult` struct now has optional `next_message` field
- Context builder formats question-answer pairs together
- Remember tool shows assistant response when retrieving user message

### Technical Details

**Problem:** Short user questions have high semantic similarity (concentrated) while
long assistant responses have low similarity (dispersed). Retrieval returned only
questions, not the answers that contain the actual information.

**Solution:** Post-retrieval enrichment:
1. Retrieve messages as before (semantic + keyword hybrid)
2. For each user message, query DB for next assistant message
3. Include both in context for complete question-answer pairs

**Example:**
```
Before: Retrieved "What about Wittgenstein?" (question only)
After:  Retrieved "What about Wittgenstein?" + Assistant response (complete info)
```

**Token Overhead:** +5 assistant responses (acceptable within 198K context)

### Files Modified

- `src/db/operations.rs` - Add `next_message` field, `get_next_message_by_role()`, `enrich_with_context()`
- `src/retrieval/context_builder.rs` - Enrich results, format question-answer pairs
- `src/tools/remember.rs` - Show assistant response for user messages
- `src/retrieval/search.rs` - Enrich results in search command

## [0.23.0] - 2026-03-03

### Added

- **Remember Tool** - LLM can actively retrieve conversation history
  - `remember(id="42")` - Get full message by ID
  - `remember(query="topic")` - Search by topic
  - Default 5 results, max 10
  - Task-local storage for async-safe DB access (via `tokio::task_local!`)

- **Context Enhancement** - Retrieved messages now show database IDs
  - Messages include `id="N"` attribute
  - Clear framing explains tool usage
  - MEMORY TOOLS section in system prompt

- **Retrieval Enabled by Default** - `retrieval_enabled: true` in new sessions

### Changed

- Retrieved context uses `message_id` instead of enumeration index
- Anonymous sessions don't register the remember tool (no database available)

### Technical Details

The GLM-5:cloud model (198K context) was still responding "I have no memory" after v0.22.9 because:
1. LLM couldn't request MORE context (only received 5 messages)
2. LLM couldn't reference specific messages (no IDs)
3. LLM couldn't search for topics (no tool)

**Solution:** Give the LLM both IDs and an explicit tool to request more context.

**Token Overhead:** ~130 tokens (0.06% of 198K context)

### Implementation Status

**Phase 1: Database**
- [x] Add `get_message_by_id()` to `src/db/operations.rs`

**Phase 2: Task-Local Storage**
- [x] Create `src/tools/context.rs` with `tokio::task_local!`

**Phase 3: Remember Tool**
- [x] Create `src/tools/remember.rs`
- [x] Implement `remember(id)` function
- [x] Implement `remember(query)` function

**Phase 4: Update Retrieved Context**
- [x] Change `context_builder.rs` to use `message_id`
- [x] Update framing text with ID explanation
- [x] Add remember tool usage instructions

**Phase 5: Default Retrieval Enabled**
- [x] Change `retrieval_enabled: false` to `true` in `session.rs`

**Phase 6: Conditional Tool Registration**
- [x] Update `src/tools/mod.rs` to export new modules
- [x] Update `src/tools/registry.rs` for conditional registration
- [x] Add context wrapper in `src/chat/repl.rs`

**Phase 7: MEMORY TOOLS Section**
- [x] Add MEMORY TOOLS section to `src/prompts/builder.rs`

**Phase 8: Testing**
- [x] Update test for `retrieval_enabled: true` default
- [x] All tests pass

**Phase 9: Finalization**
- [x] Update CHANGELOG.md
- [x] Update version in Cargo.toml
- [x] Update version in man page
- [x] Build release binary (pending)

### Files Modified

- `src/db/operations.rs` - Add `get_message_by_id()`
- `src/tools/context.rs` - NEW: Task-local storage
- `src/tools/remember.rs` - NEW: Remember tool
- `src/tools/mod.rs` - Export new modules
- `src/tools/registry.rs` - Register remember tool
- `src/retrieval/context_builder.rs` - ID format + new framing
- `src/prompts/builder.rs` - MEMORY TOOLS section
- `src/chat/session.rs` - Default `retrieval_enabled: true`
- `src/chat/repl.rs` - Context wrapper for remember tool

## [0.22.9] - 2026-03-03

### Fixed

- **Context Framing for Semantic Retrieval** - LLM now understands retrieved context
  - Added explicit framing text in `<retrieved_context>` explaining the context is from the conversation history
  - Added MEMORY section to system prompt explaining the retrieval mechanism
  - Models now correctly reference past conversations after `/clear`

### Technical Details

After v0.22.7, semantic retrieval was working correctly (session ID stable, messages preserved in SQLite, proper detection of post-clear state). However, the LLM still said "I have no memory of previous conversations" because it didn't understand what `<retrieved_context>` represented.

**Solution:**
1. Added framing text (~50 tokens):
   ```
   The following messages are from YOUR conversation history with this user.
   They represent topics you have discussed together earlier.
   Reference these when the user asks about previous topics.
   ```

2. Added MEMORY section in system prompt (~30 tokens):
   ```
   ### MEMORY
   When <retrieved_context> appears in our conversation, it contains 
   messages from our prior conversation. Reference them when the user 
   asks about topics we discussed earlier.
   ```

**Token overhead:** ~80 tokens (0.04% of 198K context for glm-5:cloud)

### Files Modified

- `src/retrieval/context_builder.rs` - Added framing text to retrieved context
- `src/prompts/builder.rs` - Added MEMORY section, `retrieval_enabled` flag
- `src/chat/repl.rs` - Pass retrieval flag to prompt builder
- `src/query.rs` - Pass retrieval flag to prompt builder
- `src/summarize/processor.rs` - Pass retrieval flag (false) for summarize

## [0.22.8] - 2026-03-03

### Added

- **Markdown Skin Configuration** - Theme support for markdown rendering
  - New `src/markdown.rs` module with global skin initialization
  - Supports `dark`, `light`, and `mono` themes from `config.toml`
  - `dark`: Transparent background, optimized for dark terminals
  - `light`: Transparent background, optimized for light terminals
  - `mono`: Monochrome with gray bold/italic, no colors
  - Config: `display.skin = "mono"` in `~/.config/sprachspiel/config.toml`

### Changed

- **All markdown output now respects skin setting**
  - `main.rs`: translate, summarize, vision commands
  - `query.rs`: query output
  - `chat/repl.rs`: chat responses
  - `retrieval/search.rs`: search results
  - `thinking.rs`: Keeps its own skin (unaffected by global skin)
  - **`should_force_retrieve()` logic rewritten**
    - Old: Only triggered when session.messages.is_empty()
    - New: Triggers when DB count > session count (after /clear with new messages)
    - Also triggers when session is empty AND has compacted_summary

### Fixed

- **CRITICAL: Retrieval after /clear now works!**
  - Bug: `should_force_retrieve()` checked if session was empty, but user already added 1+ messages
  - Fix: Compare DB message count vs session message count
  - If DB has more messages than session, retrieval is forced
  - This correctly handles: `/clear` → user asks question → retrieval happens

## [0.22.6] - 2026-03-03

### Fixed

- **Retrieval After /clear Debug Logging** - Added comprehensive debug logs
  - Logs show: `enabled`, `should_retrieve`, `force_retrieve` values
  - Logs show: session ID, anonymous status, message count, summary status
  - Logs show: DB and embedding client availability
  - Logs show: search results count
  - Use `/debug` to see detailed retrieval diagnostics

### Changed

- **build_context() parameter** - Added `use_debug: bool` parameter
  - Required for debug logging in context builder
  - Updated call sites in `repl.rs`

## [0.22.5] - 2026-03-03

### Fixed

- **Retrieval After /clear** - Critical bug fixed
  - Model lost all memory after `/clear` even though SQLite data persisted
  - Root causes: retrieval disabled by default, minimum threshold too high (20)
  
### Changed

- **Forced Retrieval After Clear** - Automatic context recovery
  - Added `should_force_retrieve()` function
  - Retrieval activates when session empty but DB has 2+ messages
  - Ignores `retrieval_enabled` flag and `MIN_MESSAGES` threshold
  - Gray system message: `[i] You may ask about previous topics.`

- **Lowered Retrieval Threshold**
  - `MIN_MESSAGES_FOR_RETRIEVAL`: 20 → 5 (more useful for short conversations)
  - `MIN_RETRIEVAL_FORCE_COUNT`: 2 (minimum for forced retrieval)

### Technical

- Modified `build_context()` to check both normal and forced retrieval conditions
- Added `MIN_RETRIEVAL_FORCE_COUNT` constant for post-clear threshold
- Updated `/clear` command to check DB for messages and show appropriate messages

## [0.22.4] - 2026-03-03

### Added

- **Persistent Memory** - Context survives `/clear` command
  - `/clear` and `/new` preserve compacted summary
  - SQLite history intact for RAG retrieval
  - Users can ask about previous topics after clearing

- **/forget Command** - Complete session reset
  - Clears all messages and summary
  - Deletes conversation from SQLite synchronously
  - Generates new session ID
  - Truly fresh start

### Changed

- **/clear Command** - Now preserves context instead of destroying it
  - Messages cleared from session memory
  - Compacted summary preserved for retrieval
  - SQLite conversation history preserved
  
- **should_retrieve()** - Now checks database message count
  - Works even when session.messages is empty (after /clear)
  - Considers both session and database for retrieval decisions

## [0.22.3] - 2026-03-03

### Added

- **Auto-Compaction** - Silent automatic context compaction
  - Triggers at 72% warning threshold
  - Triggers at 80% overflow threshold
  - Runs after assistant response
  - No user confirmation required
  - Shows `[auto-compacted context at 72%]` message in gray

- **Visual Metrics in /context** - Bar chart with colors
  - Green for <72% usage
  - Yellow for 72-80% usage  
  - Red for ≥80% usage
  - Shows token counts and percentage
  - Status text indicates current state

### Changed

- **ContextStatus** - New methods `is_warning()`, `is_overflow()`
- **needs_compaction()** - Now returns true for Warning OR Overflow
- **SendMessageResult** - Now includes `system_prompt` and `context_window` for auto-compact

## [0.22.2] - 2026-03-03

### Fixed

- **Synchronous Chunking** - Chunks are now saved synchronously, guaranteeing persistence
  - Previously: fire-and-forget async could lose chunks if process terminated
  - Now: chunks always saved, embeddings generated asynchronously
  - Addresses stress test finding: 2 of 6 long messages had incomplete chunking

- **Embedding Recovery** - Automatic recovery of missing embeddings on startup
  - New `get_chunks_without_embedding()` database function
  - Recovery runs silently on REPL startup, reports count if recovered
  - Database now has `has_embedding` flag for chunks

- **Middle Compaction** - `/compact` now preserves first N + last N messages
  - Previously: summarized ALL messages, losing important context
  - Now: preserves first 5 and last 5 messages, summarizes middle
  - Based on "Lost in the Middle" research for optimal LLM performance

### Changed

- **Context Builder** - New context order for middle compaction
  - Order: System → Retrieved → First N → Summary → Recent → Query
  - Uses `compacted_range` for middle compaction context
  - Falls back to `messages_sent_to_llm` for legacy sessions

- **Database Schema** - Version bumped to 3
  - Added `has_embedding` column to `message_chunks` table
  - Added index for finding chunks without embeddings

### Technical

- **Recovery Module** - New `src/embeddings/recovery.rs`
  - `recover_missing_embeddings()` function for startup recovery
  - Handles both messages and chunks without embeddings

- **Session Field** - Added `compacted_range: Option<(usize, usize)>`
  - Tuple format: `(first_preserved, last_preserved_start)`
  - Backward compatible: defaults to None, uses `messages_sent_to_llm` fallback

- **Compaction Function** - `compact_conversation()` now returns `(summary, range)`
  - Uses `get_compaction_range_default()` for middle compaction
  - Falls back to full compaction for small message counts

## [0.22.1] - 2026-03-03

### Fixed

- **UTF-8 Chunking Crash** - Fixed panic when splitting strings at multi-byte character boundaries
  - Chunker now correctly handles UTF-8 multi-byte characters (accents, emojis, CJK)
  - Added `find_char_boundary()` and `find_char_boundary_forward()` helpers
  - Fixed `find_sentence_boundary()` to use `.chars()` instead of byte indexing
  - Bug: "byte index 1024 is not a char boundary; it is inside 'ó'"
  - Reported when assistant response with Portuguese accents triggered chunking

### Technical

- **Tests**: Added 4 UTF-8 boundary tests (Portuguese accents, emojis, Chinese, boundary edge case)

## [0.22.0] - 2026-03-03

### Added

- **Message Chunking** - Automatic splitting of long messages for better semantic search
  - Messages > 1024 characters are split into overlapping chunks (20% overlap)
  - Each chunk gets its own embedding for precise matching
  - All message roles (user/assistant/system/tool) now get embeddings
  - Search results show matched chunk with context ellipsis

- **Chunk Storage** - New `message_chunks` table
  - Stores chunk content, offsets, and links to parent message
  - Enables reconstructing full message from chunk matches
  - Automatic cleanup when parent message is deleted (CASCADE)

### Changed

- **Embedding Generation** - Now applies to ALL roles, not just user messages
  - Fixes issue where assistant responses about Wittgenstein weren't searchable
  - System and tool messages also benefit from semantic search

- **Search Results** - Improved display for chunked messages
  - Shows matched chunk content with `...` ellipsis for boundary context
  - Full message content available for viewing
  - Better relevance scoring with chunk-level precision

- **Database Schema** - Version bumped to 2
  - Added `message_chunks` table
  - Added `chunk_embeddings` virtual table (sqlite-vec)
  - Separate embedding tables for messages and chunks

### Technical

- **New module**: `src/embeddings/chunker.rs` - Text chunking with overlap
  - `chunk_text()` - Split text into overlapping chunks
  - `needs_chunking()` - Check if message needs chunking
  - Sentence boundary detection for clean splits
  
- **Constants**:
  - `DEFAULT_CHUNK_SIZE`: 1024 characters
  - `DEFAULT_CHUNK_OVERLAP`: 200 characters
  - `DEFAULT_CHUNK_MIN_SIZE`: 256 characters

- **Database operations**:
  - `insert_chunk()` - Insert a message chunk
  - `update_chunk_embedding()` - Store chunk embedding
  - `get_message_chunks()` - Retrieve all chunks for a message
  
- **Search operations**:
  - `search_semantic()` now queries both `message_embeddings` and `chunk_embeddings`
  - Result deduplication by `message_id` (keep best score)
  - `SearchResult` now includes `chunk_content`, `chunk_start`, `chunk_end` fields

## [0.21.0] - 2026-03-03

### Added

- **ChatSession SQLite Integration** - Automatic message persistence
  - Messages saved to SQLite immediately when added
  - Embeddings generated asynchronously in background
  - Database attached via `attach_db()` method
  - Fields: `db`, `embedding_client`, `retrieval_enabled`, `last_retrieval_time`

- **Context Overflow Detection** - Automatic warning when context fills
  - `check_context_overflow()` function monitors token usage
  - Warning at 80% of context window (72% = early warning)
  - Constants: `DEFAULT_OVERFLOW_THRESHOLD`, `DEFAULT_KEEP_FIRST`, `DEFAULT_KEEP_LAST`
  - Suggests `/compact` when approaching limits
  - Future: Auto-compaction will use `get_compaction_range()` and `estimate_compaction_savings()`

- **Context Builder** - Optimal message ordering for LLM
  - `build_context()` implements "lost in the middle" research
  - Order: System → Retrieved → Summary → Recent → Query
  - Research shows up to 30% better performance with this ordering
  - Constants: `MIN_MESSAGES_FOR_RETRIEVAL`, `RELEVANT_MESSAGES_COUNT`, `RECENT_MESSAGES_COUNT`

- **Retrieval Configuration** - Configurable context retrieval
  - `RetrievalConfig` with sensible defaults
  - Min 20 messages before activation
  - 5 relevant messages retrieved + 10 recent messages
  - 5-second throttle between retrievals
  - `/retrieval` command to toggle on/off

- **Migration Commands** - JSON to SQLite migration
  - `/migrate` - Migrate all project sessions or specific session
  - `/reindex` - Rebuild embeddings for current conversation
  - Progress reporting for long migrations

### Changed

- **`send_message()`** - Now uses `build_context()` instead of `get_messages_for_llm()`
  - Integrated overflow detection with warning display
  - Integrated retrieval context building
  - Added `db` and `embedding_client` parameters
  - Returns `ContextResult` with retrieval status

- **Embeddings** - Documented future-use functions
  - `embed_batch()` for bulk embedding (future: `/migrate` performance)
  - `embedding_dimension()` for validation (test use)
  - `normalize()` and `cosine_similarity()` for future diversity filtering

- **Database operations** - Documented future-use functions
  - `list_conversations()` for `/reindex all` command
  - `get_messages_for_reindex()` for bulk reindexing
  - `delete_conversation()` for conversation management
  - `count_embedded_messages()` for statistics
  - `with_connection_mut()` for DDL operations

### Technical

- **New modules:**
  - `src/db/migration.rs` - Session migration logic
  - `src/context_overflow.rs` - Overflow detection and compaction planning
  - `src/retrieval/context_builder.rs` - Context composition with optimal ordering

- **Database operations:**
  - `get_messages_for_reindex()` - Fetch messages needing embeddings
  - `list_conversations()` - List all conversation IDs
  - `insert_message()` with embedding support

- **Context constants:**
  - `DEFAULT_OVERFLOW_THRESHOLD`: 0.8 (80%)
  - `DEFAULT_KEEP_FIRST`: 5 messages
  - `DEFAULT_KEEP_LAST`: 5 messages
  - `MIN_MESSAGES_FOR_RETRIEVAL`: 20 messages
  - `RELEVANT_MESSAGES_COUNT`: 5 messages
  - `RECENT_MESSAGES_COUNT`: 10 messages

- **Future-use functions (documented with `#[allow(dead_code)]`):**
  - `get_compaction_range_default()` - Auto-compaction planning
  - `estimate_compaction_savings()` - Compaction benefit calculation
  - `should_position_summary_after_system()` - Summary placement
  - CompactionSuggestion struct fields: `keep_first`, `keep_last`, `middle_count`

## [0.20.0] - 2026-03-03

### Added

- **Semantic Search** - New `/search` (alias `/find`, `/f`) command for chat
  - Hybrid search combining BM25 (keyword) and semantic (vector similarity)
  - Reciprocal Rank Fusion (RRF) for result ranking
  - Search across all conversation history
  - Usage: `/search <query>` or `/search <query> <limit>`
  - Requires `nomic-embed-text-v2-moe` model from Ollama

- **Database Module** - New SQLite storage with sqlite-vec extension
  - `src/db/mod.rs` - Database initialization and exports
  - `src/db/schema.rs` - SQL schema (conversations, messages, embeddings, FTS5)
  - `src/db/connection.rs` - sqlite-vec global initialization
  - `src/db/operations.rs` - CRUD operations and hybrid search
  - Storage location: `~/.local/share/sprachspiel/embeddings.db`

- **Embeddings Module** - New embedding generation for semantic search
  - `src/embeddings/client.rs` - Ollama embedding client
  - `src/embeddings/truncate.rs` - Matryoshka truncation (768d → 256d)
  - Validates embedding dimensions before truncation
  - L2 normalization for cosine similarity

- **Retrieval Module** - New search module
  - `src/retrieval/search.rs` - Hybrid search implementation
  - Formatted results with icons and metadata
  - Integration with `/search` command

- **FTS5 Query Sanitization** - SQL injection protection
  - `fts5_escape()` function for safe FTS5 queries
  - Wraps queries in double quotes, escapes embedded quotes
  - Prevents FTS5 syntax errors and injection attacks

### Dependencies

- `rusqlite` 0.32 (bundled) - SQLite database
- `sqlite-vec` 0.1 - Vector similarity extension
- `zerocopy` 0.8 - Safe byte casting for embeddings

### Technical

- Embedding dimensions: 768 (full) → 256 (truncated, Matryoshka)
- RRF weights: Keyword 0.4, Semantic 0.6
- sqlite-vec KNN syntax: `WHERE embedding MATCH ? AND k = ?`
- Database initialized on startup via `db::init()`

## [0.19.0] - 2026-03-02

### Added

- **Context Metrics Command** - New `/context` (alias `/ctx`) command for chat
  - Displays estimated token usage breakdown (system, tools, conversation)
  - Shows context window utilization percentage
  - Helps users understand context pressure and plan compaction

- **Token Counting Module** - New `src/tokens.rs` module
  - Word-based token estimation (~0.75 words/token for English)
  - Message overhead calculation (~4 tokens per message)
  - `ContextMetrics` struct for context usage tracking
  - `calculate_context_metrics()` for full context analysis

- **Todo List Tools** - New tool category for task tracking
  - `todo_add(description)` - Add a new task to the list
  - `todo_update(id, status)` - Update task status (pending/in_progress/done)
  - `todo_list()` - List all tasks with current status
  - `todo_clear_done()` - Remove completed tasks
  - `todo_clear_all()` - Clear all tasks
  - Reduces need to search conversation history for task tracking
  - Enabled via `todo-tools` feature flag (enabled by default)

- **Todo State Persistence** - Todo list persists with chat session
  - New `todos` field in `ChatSession`
  - `TodoState` struct with `Task` and `TaskStatus` enums
  - Automatically saved/restored with session

- **HTTP Helpers** - New utilities for tool implementations
  - `fetch_json<T>()` for GET requests with JSON parsing
  - `fetch_json_with_headers<T>()` for requests with custom headers
  - `post_json_with_headers<T>()` for POST requests
  - All helpers include automatic error logging

- **Logging Macros** - Boilerplate reduction for tools
  - `log_tool_call!` macro for tool call logging
  - `log_tool_result!` macro for result logging
  - `tool_wrapper!` macro for automatic logging wrapper

### Fixed

- **Code Quality** - Clippy warnings and dead code cleanup
  - Fixed collapsible if statements in platform.rs and personality.rs
  - Fixed `.map(|x| x.clone())` to `.cloned()` in builder.rs
  - Fixed manual RangeInclusive::contains patterns in tests
  - Removed duplicate ContextMetrics struct
  - Fixed MESSAGE_OVERHEAD constant visibility

### Changed

- **Feature Flags** - `todo-tools` added to default features
  - `all-tools` now includes `todo-tools`

- **Refactoring** - Unified HTTP handling across tools (~370 lines reduced)
  - pokemon.rs: 9 functions refactored using shared helper
  - weather.rs: 3 functions + helper refactored
  - serper.rs: 2 functions refactored with POST helper
  - finance.rs: collapsible_if fix

## [0.18.2] - 2026-03-01

### Added

- **Undo Command** - New `/undo` (alias `/u`) command for chat
  - Removes last assistant response(s) from conversation
  - Displays last user message for easy re-editing
  - Use arrow up (↑) to retrieve from history and edit
  - Workflow: `/undo` → see message → ↑ to edit → resend

## [0.18.1] - 2026-03-01

### Fixed

- **Chat Model Configuration** - `[model.chat].model` is now properly respected
  - Chat subcommand now uses `model.chat.model` from config.toml as default
  - Previously fell back directly to global `model.default`, ignoring chat-specific model
  - Affects anonymous mode (`-a`), failed session loads, and new sessions

## [0.18.0] - 2026-03-01

### Added

- **LED Control Tools** - New optional tool category for NeoPixel LED control
  - 5 tools: `led_get_status`, `led_set_power`, `led_set_program`, `led_set_brightness`, `led_set_color`
  - Control LED strips via Raspberry Pi Pico W HTTP server
  - Requires `led-tools` feature flag and `[led]` configuration in config.toml
  - Color manipulation with hex or RGB values (LLM-friendly)
  - Configuration: `ip` (required) and `port` (optional, default: 80)

- **LED Tools Documentation** - Comprehensive docs in `doc/src/tools.md`
  - Tool reference with examples
  - Configuration guide
  - Color manipulation tips for LLMs
  - Example workflows for natural language control

- **Chat Configuration** - New `[model.chat]` section in config.toml
  - Configure default model, thinking, and tools for chat subcommand
  - Falls back to global `[model]` settings if not specified

- **Thinking Mode Priority** - Improved thinking mode resolution
  - Priority: Model capability → CLI flags → Subcommand config → Global config → Model default
  - Configurable via `model.thinking` (global) and `model.chat.thinking` (subcommand)
  - Warning shown if thinking enabled but model doesn't support it

### Configuration

New config.toml options:
```toml
[model]
# Global default for thinking mode (optional)
thinking = false

[model.chat]
# Chat-specific model and settings
# model = "llama3.1"
# thinking = false
# tools = true
```

New `[led]` section in config.toml:
```toml
[led]
ip = "192.168.1.100"  # Required for LED tools
port = 80             # Optional, default: 80
```

### Feature Flags

- `led-tools` - Enable LED control tools (disabled by default)
- `all-tools` now includes `led-tools`

### Few-shot Examples

Added 3 new LED tool examples to demonstrate:
- Basic power and color control
- Color adjustment workflow (get status → modify RGB → set color)
- Brightness and power control

## [0.17.0] - 2026-02-28

### Added

- **Prompt Refactoring** - Complete system prompt overhaul based on prompt engineering best practices
  - Created modular prompt system in `src/prompts/` with hierarchical structure
  - Added 13 ReAct-style few-shot examples (replaced arrow notation)
  - Removed all negative instructions (DO NOT, NEVER, etc.)
  - Created benchmark tests (10 passing) for prompt validation
  - Token count reduced from ~1700 to ~890 tokens

- **Platform Detection** - Dynamic OS/distro detection in `src/platform.rs`
  - Detects Linux distros (Arch, Ubuntu, Debian, Fedora, etc.)
  - Detects Termux on Android
  - Detects macOS and Windows
  - Platform info added to system context

- **Retry Command** - New `/retry` (alias: `/r`) command
  - Removes assistant messages since last user message
  - Regenerates response with same context
  - Useful for getting different answers

### Fixed

- **Anonymous Chat Mode** - Now truly anonymous
  - Fixed bug where anonymous mode was loading sessions from "anonymous" directory
  - Anonymous sessions now start completely fresh, no history persistence

- **Immediate Message Saving** - User messages saved immediately after sending
  - Previous behavior: saved only after receiving response
  - New behavior: saved immediately, preventing message loss on crash/interrupt

### Changed

- **Chat Session API** - Added new methods:
  - `remove_last_assistant_messages()` - removes messages since last user message
  - `get_last_user_message()` - retrieves last user message for retry

### Technical

- Created `src/lib.rs` for library module exports (test infrastructure)
- Added `PromptConfig` builder pattern for flexible prompt generation
- Deprecated old prompt functions (`get_prompt`, `get_prompt_with_blacklist`)

## [0.16.2] - 2026-02-25

### Fixed

- **CLI Model Override in Chat** - CLI model parameter now takes precedence over saved session model
  - Fixed bug where `-m <model>` was ignored when resuming a saved session
  - Added validation: if CLI model doesn't exist, show error and exit gracefully
  - Added fallback: if saved session model no longer exists, use default with warning

## [0.16.1] - 2026-02-25

### Fixed

- **Model Switching in Chat** - Centralized model switching logic to prevent state inconsistencies
  - Created `src/chat/model_switch.rs` as single point for all model switching
  - Fixed bug where `session.tools` and `session.think` could diverge from internal state
  - All model validation, capability detection, and state updates now happen in one place
  - Removed duplicate model switching code from `commands.rs`

- **Man Page Updated** - Updated to v0.16.1 with:
  - New `vision` command documentation
  - New `completion` command documentation
  - Updated model list with default vision model (moondream)
  - Correction: translategemma default changed to 4b model

## [0.16.0] - 2026-02-24

### Added

- **Vision Command** - New `ask vision` subcommand for image analysis
  - Default model: moondream:1.8b (lightweight, 1.7GB)
  - Multi-image support via Ollama API `images` array
  - Three modes: default (brief), --detailed (comprehensive), custom prompt
  - JSON output with --json flag
  - Markdown rendering with --plain global flag for plain text
  - Configurable via `[model.vision]` in config.toml
  - Documentation in `doc/src/commands/vision.md`

- **Shared Image Utilities** in `src/utils.rs`
  - `validate_image_file()` - validates file existence and extension
  - `read_file_as_base64()` - async file reading with base64 encoding
  - Used by both OCR and Vision modules

### Changed

- **Translation Model Updated** - Changed default from translategemma:12b to translategemma:4b
  - Smaller, faster model with same translation quality
  - Updated all documentation and config defaults

- **Vision Models Tested** - Updated documentation with verified working models:
  - moondream:1.8b - Default, lightweight
  - llava:13b - Better quality (llava:7b doesn't work)
  - llama3.2-vision:11b - Large context, good interpretation
  - ministral-3:14b - Multi-image support

- **Code Deduplication** - Shared utilities between OCR and Vision modules

## [0.15.0] - 2026-02-23

### Added

- **Custom Coordinator** - New `CustomCoordinator` implementation
  - Pre-tool content forwarding - model's thinking/intro text before tool calls is now displayed
  - Event callbacks for `PreToolContent`, `ToolCall`, `ToolResult`
  - Replaces ollama-rs Coordinator for full control over tool execution flow

- **Thinking Display Improvements**
  - Lighter gray color (`\x1B[37m`) for better readability
  - Markdown rendering support with `termimad::MadSkin`
  - Proper word wrapping respecting terminal width
  - No more words cut in half on narrow terminals

- **Retry Logic for Query Mode**
  - Query and legacy query now have same retry logic as REPL
  - Recoverable errors (unknown tool, invalid args, network) trigger retry
  - Model receives error message and can correct tool calls
  - Up to 3 retry attempts

### Changed

- **Tool Output Display**
  - Tool calls show function name with parameters (from debug_tools.rs)
  - Tool results show abbreviated preview in normal mode
  - Debug mode shows full detailed output
  - No duplicate logging

- **Refactored `display_thinking()`**
  - New `render_markdown` parameter for markdown rendering
  - Automatically detects terminal width for proper wrapping
  - Word-wrap algorithm respects word boundaries

### Fixed

- **Thinking Text Wrapping** - Now properly wraps at word boundaries
  - Uses terminal width detection via `termimad::terminal_size()`
  - Accounts for 2-character indentation
  - Supports both markdown and plain text modes
  - Created `src/query.rs` module with shared query logic:
    - `run_query()` - unified function for query, legacy query, and chat message handling
    - `ChatContext` - builder for coordinator with event callbacks
    - `OutputFlags` - resolved debug/plain flags from CLI and config
    - `handle_chat_event()` - centralized event handling for tool execution
  - Consolidated `handle_query()` and `handle_legacy_query()` in `main.rs`
  - `main.rs` reduced from 1175 lines to 572 lines (51% reduction)
  - Chat REPL now uses `ChatContext` builder pattern
  - Created `src/tools/registry.rs` with centralized tool registration
  - Created `src/utils.rs` with shared utility functions
  - Moved `build_model_options()` to `ModelConfig` as instance method
  - Added `detect_or_default()` to `ModelCapabilities`
  - Added `display_thinking()` helper to `chat/thinking.rs`
  - Added `resolve_model_config()` and `resolve_think_mode()` to `user_models.rs`
  - Added `SpinnerGuard` RAII pattern to `spinner.rs`

- **Chat Mode CLI Flags** - Model and flags from CLI now work correctly
  - `ask chat -m <model>` now properly sets the initial model
  - `ask chat -t` now enables think mode from CLI
  - `ask chat --tools` now enables tools from CLI
  - `ask chat --ignore-agents` now ignores AGENTS.md from CLI

### Removed

- **Dead Code** - Removed unused code and false-positive `#[allow(dead_code)]`
  - Removed `OutputFormat` enum and unused methods from `ocr/cli.rs`
  - Removed false `#[allow(dead_code)]` from `NamedApiResource.url` and `Settings::blacklist_set()`

## [0.14.2] - 2026-02-22

### Fixed

- **Tool Error Handling** - Tools now return errors as `Ok(String)` instead of `Err()`
  - Model sees tool errors and can react/retry
  - Previously, `Err()` would immediately fail the entire request
  - Now the model receives the error message and decides how to proceed

- **test_tool** - Debug tool now returns error message as success
  - Allows testing tool error recovery scenarios
  - Model can see error and retry with different parameters

### Added

- **Error Recovery Helpers** - New utilities in `chat/coordinator.rs`
  - `RecoverableError` enum for classifying errors
  - `classify_error_str()` for string-based error classification
  - `format_recovery_message()` for model-friendly error messages
  - Prepared for future network/Ollama-level error recovery

## [0.14.1] - 2026-02-20

### Fixed

- **Thinking Output** - Now uses API-provided `thinking` field from Ollama
  - Previously only extracted thinking from content via regex
  - Now checks `response.message.thinking` first, then falls back to regex
  - Works correctly with cloud models that support thinking

### Changed

- **Model Parameters** - `top_k`, `top_p`, `repeat_penalty` now optional
  - If not specified in config, uses Ollama's defaults
  - Updated defaults from docs.ollama.com: `temperature=0.8`, `repeat_penalty=1.1`
  - Previous defaults were too low (`temperature=0.2`)

- **Cloud Model Thinking** - Enable thinking via config
  - Add `thinking = true` in `models.toml` for cloud models
  - Model's `thinking` field checked alongside capability detection
  - Priority: CLI flag > model config > subcommand config

## [0.14.0] - 2026-02-19

### Added

- **Custom Models** - Define your own models or override built-in presets
  - Create `~/.config/sprachspiel/models.toml` to add custom models
  - Override parameters for built-in models (partial override)
  - Custom models shown with `[user]` marker in `--list` output
  - See [Configuration - Custom Models](./configuration.md#custom-models)

- **Thinking Output** - Thinking content now visible in chat mode
  - When think mode is enabled, thinking content shown in gray/dim text
  - Supports multiple thinking tag formats: unicode, `<tool_call>`, `<thinking>`
  - Handles malformed tags (orphan `</thinking>`)

- **Token Metrics** - Response now shows token usage
  - Displays: `[Tokens: X prompt + Y response = Z total]`
  - Helps track context window usage

- **Error Formatting** - Improved error messages
  - JSON errors from Ollama formatted with red status codes
  - Clear guidance for common error scenarios

### Changed

- **Tool Output Control**:
  - New `/tools-output <level>` command: compact, full, or hidden
  - New `--tools-output` startup flag
  - Compact summary shown after `/compact` command

- **Built-in Models Simplified** - Reduced to essential models only
  - Built-in: `llama3.1:8b` (default), `translategemma:4b` (translation), `glm-ocr:bf16` (OCR), `moondream:1.8b` (vision)
  - All other models moved to `~/.config/sprachspiel/models.toml`
  - Cloud models have no hardcoded parameters (let Ollama decide)

- **Default Context Size** - User models now default to 32K context
  - Previous: 4K default for user-defined models
  - Now: 32K default for better compatibility with large context models
  - Omit `num_ctx` to let Ollama auto-detect based on available memory

- **Model Naming Convention** - Removed context size suffixes from model IDs
  - Previous: `lfm2.5-thinking:1.2b-32k`, `llama3.2:3b-32k`
  - Now: `lfm2.5-thinking:1.2b`, `llama3.2:3b`
  - Context size configured via `num_ctx` in models.toml, not model tag

- **Default Model Changed** - From `lfm` to `llama3.1`
  - `llama3.1:8b` is more capable and widely available
  - `lfm` still available as user-defined model

- **GPT-OSS Removed** - Model removed due to persistent tool calling issues
  - The model output special tokens after JSON, breaking tool parsing
  - Alternative models: `qwen3-coder`, `mistral-small`, `llama3.1`

### Migration Notes

If upgrading from v0.13.0:
1. Run `sprach --list` to see the new model organization
2. Default model is now `llama3.1` (update config if you used `lfm`)
3. Check `~/.config/sprachspiel/models.toml` for all available model presets
4. Cloud models no longer have hardcoded parameters - configure as needed

## [0.13.0] - 2026-02-19

### Added

- **Chat Mode Enhancements**:
  - `/think` command - Toggle think mode on/off
  - `/tools` command - Toggle tools on/off
  - `/compact` command - Summarize conversation history to reduce context
  - Tab completion for commands and model names
  - Mode indicators in prompt: `[t]` for think, `[T]` for tools
  - Warning when tools enabled but model doesn't support them

### Changed

- **Session Format** (Breaking Change):
  - Added `compacted_summary` field for conversation summarization
  - Added `messages_sent_to_llm` field to track compacted portion
  - Old session files may need to be deleted (`~/.local/share/sprachspiel/conversations/`)

- **UI Improvements**:
  - Welcome message only shows available features (think/tools hidden if unsupported)
  - Prompt shows active modes with emojis: `lfm🧠🔧>` when think and tools enabled
  - `/info` shows compacted message count if applicable

### Removed

- `uuid` dependency (session IDs are now simple strings)

## [0.12.0] - 2026-02-19

### Added

- **Interactive Chat Mode** - New `chat` subcommand for multi-turn conversations
  - Persistent conversation history per project (identified by git remote URL or folder name)
  - Anonymous sessions with `--anonymous` flag (no persistence)
  - Session management: `/save`, `/load`, `/list` commands
  - Model switching mid-conversation: `/model <name>`
  - Export conversations: `/export md` or `/export json`
  - Rich REPL with command history and line editing (rustyline)
  - Auto-saves after each message to `~/.local/share/sprachspiel/conversations/`
  - Commands: `/quit`, `/clear`, `/help`, `/model`, `/system`, `/save`, `/load`, `/export`, `/list`, `/info`

### Changed

- **Dependencies**:
  - Added `rustyline` for REPL with history
  - Made `chrono` non-optional (used for session timestamps)

## [0.11.1] - 2026-02-18

### Fixed

- **Config file ignored by subcommands** - translate, ocr, summarize now respect ollama_host/ollama_port
  - Previously: These subcommands used `Ollama::default()` (localhost:11434) ignoring config
  - Now: All subcommands use `Settings::ollama_client()` for consistent config handling
  - Fixes "Reqwest error" when connecting to remote Ollama server from Termux/Android
- **CLI parameter precedence** - Fixed bug where CLI flags were not properly respected
  - Changed `model`, `plain`, `debug` fields from `String`/`bool` to `Option<String>`/`Option<bool>`
  - Precedence now correctly: CLI arguments > config file > built-in defaults

### Changed

- **CLI flag architecture** - Centralized shared flags at global level
  - Flags like `-m`, `-d`, `--plain`, `-t`, `--tools`, `-c`, `--ignore-agents` now only exist at global level
  - Usage: `ask -d query "text"` (flags BEFORE subcommand)
  - Subcommands retain their specific flags: `translate --list`, `summarize --format bullets`, `ocr --mode table`
  - Updated documentation and manpage to reflect this change

## [0.11.0] - 2026-02-18

### Added

- **Termux/Android support** - Cross-compilation for Android devices
  - New Makefile targets: `termux`, `termux-all-tools`, `tarball-termux`, `tarball-termux-all-tools`
  - Cross.toml configuration for `aarch64-linux-android` target
  - Documentation for Termux installation
- **rustls TLS backend** - Replaced OpenSSL with rustls for better cross-platform support
  - Enables cross-compilation without OpenSSL dependencies
  - Binary size optimized (12MB default, 16MB all tools)

### Fixed

- **ollama_host configuration** - Now accepts IP addresses without `http://` prefix
  - Previously: `ollama_host = "192.168.1.100"` would panic
  - Now: Automatically prepends `http://` if scheme is missing
  - Works with: `"192.168.1.100"`, `"http://192.168.1.100"`, `"https://myserver.local"`

### Changed

- **Dependency optimization**:
  - Aligned `reqwest` version with `ollama-rs` (v0.12) to avoid duplication
  - Removed redundant explicit dependencies (`html2md`, `scraper`) - already provided by `ollama-rs`
- **Documentation updates**:
  - Fixed incorrect web search documentation (serper-tools is working, search-tools is optional)
  - Updated feature flags table in README and AGENTS.md

## [0.10.0] - 2026-02-18

### Added

- **System context injection** - Minimal context (~20 tokens) injected into every prompt
  - Current date (day of week, date)
  - Current working directory
  - Git branch (if in repo)
- **New tool: `get_current_datetime`** - Current date, time, and timezone
  - Feature flag: `system-tools` (enabled by default)
  - Returns: date, time, timezone, day of week, week of year, ISO 8601, Unix timestamp
- **New tool: `get_project_context`** - Project state (languages, git, stack)
  - Feature flag: `system-tools` (enabled by default)
  - Provides: directory, git branch/remote, language detection, stack detection, key files
  - **Relationship with AGENTS.md**: AGENTS.md contains conventions (HOW), get_project_context provides state (WHAT)

### Changed

- **Default feature flags reorganized**:
  - `system-tools` is now enabled by default
  - All tools enabled by default (empty blacklist)
- **Code cleanup** - Fixed all clippy warnings

### Removed

- **Legacy web-search-tools removed** - Replaced by serper-tools

## [0.9.0] - 2026-02-18

### Added

- **New tool: `fetch_pokemon_by_type`** - List all Pokémon of a specific type (limit 100)
- **New tool: `calculate`** - Mathematical expression evaluation
  - Basic arithmetic: +, -, *, /
  - Exponents: ** or ^
  - Percentages: "15% of 850"
  - Functions: sqrt(), sin(), cos(), tan(), log(), etc.
  - Feature flag: `calc-tools` (enabled by default)
- **New tool: `get_stock_quote`** - Stock quotes from Google Finance
  - Feature flag: `finance-tools` (disabled by default)
  - Usage: `get_stock_quote(exchange: "NASDAQ", ticker: "AAPL")`
- **New tool: Web search via Serper** - Google Search results via Serper.dev API
  - Feature flag: `serper-tools` (enabled by default)
  - Requires `SERPER_API_KEY` environment variable
  - Tools: `web_search`, `web_search_news`
  - Automatic fallback to DuckDuckGo if API key not set
  - Debug mode shows: "🔑 [Serper] API key found - enabling Google Search via Serper"
- **All tools now output in English** - Consistent English output across all tools
- **Pokémon tools enabled by default** - No longer need `--features pokemon-tools`
- **Makefile targets for feature builds**:
  - `make build-pokemon` - Build with Pokémon tools
  - `make build-all-tools` - Build with all tools
  - `make install-local-pokemon` - Local install with Pokémon tools
  - `make install-local-all-tools` - Local install with all tools
  - `make test-all` - Run tests with all features

### Changed

- **BREAKING: Tool output language changed to English** - All tool responses now in English
- **Web search now prefers Serper over DuckDuckGo** - Serper uses Google Search with no CAPTCHA issues
- **Feature flag reorganization**:
  - `serper-tools` - Google Search via Serper API (enabled by default)
  - `system-tools` - Date/time and project context (enabled by default)
  - `search-tools` - DuckDuckGo + Web scraper (disabled, used as fallback)
  - `finance-tools` - Stock quotes via Google Finance (disabled by default)
  - `all-tools` now includes all tool categories
- **All numeric/optional tool parameters now accept strings** - LLMs frequently pass `"5"` instead of `5`, tools now handle this gracefully

### Fixed

- Weather tools fixed - API response structs now use optional fields
- All tools have proper error handling - No more crashes from network/API errors
- Raw errors shown with pretty printing in debug mode - Use `{:#?}` for readable output
- Pokémon tools fixed - All 9 tools have proper logging and error handling
- **Web search CAPTCHA issue resolved** - Using Serper for Google Search (no CAPTCHA)
- **Tool parameter parsing fixed** - All tools now accept strings for numeric parameters

### Documentation

- Updated tools.md with Serper configuration, system tools, and usage examples
- Updated CHANGELOG.md with all changes
- Updated README.md with AGENTS.md context and build features
- Updated contributing.md with feature flags and Makefile commands
- Updated man page with --ignore-agents flag
- Updated roadmap.md with ollama-rs integration status

## [0.1.0] - 2026-02-17

### Added

- Initial release
- Core CLI with 4 subcommands:
  - `query` - General LLM queries
  - `translate` - Language translation (50+ languages)
  - `ocr` - Image text extraction
  - `summarize` - Text summarization
- 14 tools:
  - 8 Pokémon tools via PokéAPI
  - 3 Weather tools via Open-Meteo
  - 3 Web Search tools via DuckDuckGo (currently blocked)
- 14+ model presets with capability detection
- Markdown rendering via termimad
- Tool integration with auto-detection
- Pipe support for all commands
- Debug mode
- Think mode for reasoning models
- Code mode for code-focused responses
- Model capability detection
- Spinner for UX feedback
- Stdin support
- Plain text output option
- Pepe Easter Egg personality

### Known Issues

- DuckDuckGo web search blocked by CAPTCHA

## Categories

- `Added` - New features
- `Changed` - Changes to existing functionality
- `Deprecated` - Soon-to-be removed features
- `Removed` - Removed features
- `Fixed` - Bug fixes
- `Security` - Security fixes

## Versioning

We follow [Semantic Versioning](https://semver.org/):

- MAJOR: Incompatible API changes
- MINOR: Backward-compatible functionality
- PATCH: Backward-compatible bug fixes

## Format

Based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
