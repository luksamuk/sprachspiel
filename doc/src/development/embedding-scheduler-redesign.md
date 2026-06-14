# W2 #121 — Embedding Scheduler Redesign (draft)

## Status

Draft — awaiting user approval.

## Context

Today, embeddings are generated at five different points in the
lifecycle, two of which are **synchronous** and stall the chat cycle:

| Site | When | Sync? | Purpose |
|------|------|-------|---------|
| `run_indexing_probe` | startup (init_chat_database) | yes | probe embedding dim (REMOVING) |
| `recover_missing_embeddings` | startup (repl_tui) | async fire-and-forget | backfill missing items |
| `build_context` (RAG) | every user turn | **yes** | embed query for retrieval |
| `remember` tool | mid ReAct cycle | **yes** | embed query for note search |
| `/note add`, `/doc import` | post user message | async fire-and-forget | embed new content |
| `save_sqlite` chunks | session save | async fire-and-forget | embed message chunks |
| `extract_and_insert_facts` | post user message | async fire-and-forget | embed facts |

The two **sync** sites — RAG and `remember` — both call
`/v1/embeddings` on llama-swap, which is slow. They cause:

- Visible stalls in the TUI spinner during tool calls.
- A HTTP 400 race with the LLM context window (already fixed in #207
  commits 75c38eb / 6538467).
- A general feeling that the model is "thinking" but actually
  waiting on HTTP.

Per user decision (this conversation):

1. Embed only at end of each ReAct cycle (plus startup recovery).
2. Remove probe completely.
3. RAG uses embedding from **previous** turn (1-turn latency, but
   invisible to the user).
4. `remember` keeps embedding but with 1-turn latency (queued).

## Proposal

### Data flow

```
Turn N
  user → build_context (uses embedding from turn N-1, if any)
       → send to LLM
       → ReAct cycle (tools run, no embedding)
       → LLM final answer
  end-of-cycle hook
       → process EmbeddingQueue
            - pending user message embedding (1 item)
            - pending remember queries (0..N)
            - pending /note add or /doc import items (0..N)
            - pending chunking from save_sqlite (0..N)
            - pending fact extractions (0..N)
       → batched /v1/embeddings request
       → DB writes happen synchronously
  turn N+1
  build_context sees embedding from turn N
```

### Components

1. **EmbeddingQueue** (`src/embeddings/queue.rs`, new)
   - Thread-safe queue (`Mutex<Vec<PendingEmbedding>>` or
     `tokio::sync::mpsc::Sender<PendingEmbedding>`).
   - `PendingEmbedding` is an enum with variants for each kind of
     pending work:
     ```rust
     enum PendingEmbedding {
         Message { item_id, content, conv_id, project_id },
         Remember { query, caller_session_id, return_to_tool: bool },
         Note { note_id, content, project_id },
         Doc { doc_id, ... },
         Chunk { chunk_id, content, ... },
         Fact { fact_id, content, ... },
     }
     ```
   - `enqueue(p: PendingEmbedding)` — non-blocking, push to queue.
   - `drain_all() -> Vec<PendingEmbedding>` — atomic drain.
   - `len() -> usize` — for status bar display.

2. **EmbeddingScheduler** (`src/embeddings/scheduler.rs`, new)
   - Owns the queue + DB + client.
   - `enqueue_user_message(...)`, `enqueue_remember_query(...)`, etc.
   - `flush()` — async, drains queue, batches embeddings, writes
     to DB. Reports progress via the existing `EmbeddingProgress`
     channel so the TUI status bar can show "Indexing N items..."

3. **Embed on cycle end** (modify `event_loop.rs`)
   - In the `LlmEvent::Complete` handler (around line 368), after
     `view.app_mut().reset_round()`, call
     `state.embedding_scheduler.flush().await`.
   - In the `LlmEvent::Error` handler (line 391), call
     `state.embedding_scheduler.flush().await` too — partial
     state is still useful.

4. **RAG with previous-turn embedding** (modify
   `retrieval/context_builder.rs`)
   - Currently, `build_context` does
     `client.embed(query).await` at line 190 to embed the user's
     current message. Replace with a lookup in
     `EmbeddingQueue` or session storage for the **previous**
     message's embedding.
   - On the first turn of a session, no previous embedding
     exists — fall back to keyword-only retrieval (BM25).
   - This gives 1-turn semantic latency: the LLM in turn N sees
     messages whose embeddings were generated at the end of
     turn N-1.

5. **`remember` tool becomes async** (modify
   `tools/remember.rs:662`)
   - When called mid-cycle, enqueue the query in
     `EmbeddingQueue` and return a "queued" message immediately.
   - When the cycle ends, the scheduler flushes the queued
     remember query: generates the embedding, runs the search,
     **stores the result for the next `remember` call or future
     RAG**.
   - For the current turn's `remember` call, the tool returns
     a deferred result: "Queued for end-of-cycle search. Results
     will appear in the next turn's RAG."

6. **Remove probe** (modify `chat/repl.rs:91`,
   `db/init.rs:174`)
   - Delete `run_indexing_probe` entirely.
   - Delete the `[indexing].probe` config key from `Settings`
     and the SAMPLE_CONFIG docs.
   - Update `IndexingInit` so it doesn't accept `probe_enabled`.
   - If the user really wants a probe, they can curl the
     endpoint manually. The strict-verify logic is preserved in
     `probe_embedding()` but no longer called from startup.

7. **Auto fact extraction stays async**
   - It already runs via `tokio::spawn` (fire-and-forget), and
     the embeddings are best-effort. We keep this behavior.

### Edge cases

- **Cycle aborted by Ctrl+C**: queue items survive. On next
  startup, `recover_missing_embeddings` handles them (existing
  code).
- **Embedding server down**: `flush()` retries 3x, then logs
  warnings and continues. Items stay in the queue for next
  attempt. No user-visible error; the TUI just shows "Indexing
  failed for N items".
- **Multiple users / concurrent chat**: not supported today
  (single-session REPL). If we add it later, the queue becomes
  per-session.
- **`remember` queueing**: a 1-turn latency means a turn-N
  `remember("X")` whose result feeds turn N+1's response.
  Acceptable because the LLM already trained on this pattern
  (instructed in the system prompt).

### Migration

- Backward-compat: `[indexing].probe = false` in user
  `config.toml` is silently ignored after the change. We add a
  warning at startup that says "this key is no longer used;
  remove it."
- The `embed_item_with_fallback`, `embed_chunk_with_fallback`,
  and `embed_query_with_fallback` functions stay — the
  scheduler calls them, but the call sites that previously
  invoked them synchronously now go through the queue.
- New `Settings::indexing` keeps `model`, `dimensions`,
  `keyword_weight`, `semantic_weight` (no `probe`).

### Phasing

- Phase 1 (this PR): EmbeddingQueue + scheduler + flush at
  LlmEvent::Complete. RAG with previous-turn. Remember queued.
  Probe removed. (atomic, single PR)
- Phase 2 (later): Batching of queue items per flush (1 HTTP
  request for N items instead of N requests). Currently
  `EmbeddingClient::embed_batch` already exists; just need to
  call it from the scheduler.
- Phase 3 (later, if desired): Optional embed-on-tool-call
  opt-in for low-latency cases (e.g. when user asks "remember
  this" and expects immediate confirmation). Add a flag
  `[indexing].sync_tools = true` to disable queuing for
  `remember` specifically.

## Open questions

- **Q1**: Should the `remember` tool block the LLM from
  re-asking the same question in the same turn? E.g. if the LLM
  calls `remember("Hamming")` and the result is queued, can the
  LLM still call `remember("Hamming")` again before cycle end?
  Proposal: yes, no change; the LLM is already trained to avoid
  duplicate calls. Queued `remember` results are stored
  in-memory for the next turn's RAG context; they don't
  back-propagate into the same turn.

- **Q2**: Does RAG with previous-turn embedding lose any
  functionality? The RAG context is computed once per turn. If
  the user asks "what did I say about Hamming?" in turn N, the
  RAG context for turn N was built with the embedding from
  turn N-1's user message — which doesn't yet know the user
  asked about Hamming. The semantic search will return turns
  *prior* to N where Hamming was discussed.
  Proposal: this is acceptable. The user can always ask
  "search for Hamming" again in turn N+1 and RAG will pick up
  the new turn-N query.

- **Q3**: How does the user see "Indexing pending" in the
  status bar? Existing `EmbeddingProgress` channel is used.
  The scheduler sends `EmbeddingProgress::new(EmbeddingPhase::Pending,
  0, queue.len(), 0, queue.len())` to indicate pending items,
  and progress through the phases for the active flush.

- **Q4**: Should the `tool_infos` for `remember` describe the
  new "queued" semantics? Yes — update the docstring in
  `tools/remember.rs:655` and `tools/remember.rs:1-50` (file
  header) to explain the 1-turn latency.

## Quality gates

- All existing 1543 lib tests must still pass.
- New unit tests for `EmbeddingQueue` (concurrent enqueue,
  drain, len).
- New integration test: simulate a 3-turn cycle, verify RAG
  uses turn-1 embedding in turn 2, turn-2 embedding in turn 3.
- New unit test for `flush()`: enqueue 3 items, verify 1
  `embed_batch` call and 3 DB writes.
- New unit test for remember queueing: enqueue remember,
  verify tool returns "queued" and search result appears in
  next turn's RAG.
- Manual smoke test: same flow as the user's reported case
  (gemma4-e2b + PDF + pdftotext + run_command). Verify no
  stalls, errors visible in TUI, embeddings generated at
  cycle end.

## Estimate

- EmbeddingQueue + scheduler: ~150 lines
- event_loop.rs hook: ~10 lines
- context_builder.rs RAG lookup: ~30 lines
- remember.rs async path: ~50 lines
- remove probe: ~30 lines (deletions + Settings)
- tests: ~200 lines
- docs: ~100 lines

Total: ~570 lines, 1 PR, 1 commit (or 3-4 granular).
