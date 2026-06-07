# Manual Test Script — Cycle-Aware Message Ordering (Issue #201)

**Issue:** #201 - Cycle-aware message ordering in TUI
**Branch:** `fix/201-cycle-aware-message-ordering`
**PR:** #202

---

## Prerequisites

```bash
cd /path/to/sprachspiel
cargo build --release --features all-tools
ollama serve  # In another terminal

# Backup and reset the database for clean state
cp ~/.local/share/sprachspiel/sprachspiel.db ~/.local/share/sprachspiel/sprachspiel.db.backup 2>/dev/null || true
rm -f ~/.local/share/sprachspiel/sprachspiel.db
```

## Test Model

```bash
# Use a model that supports tool calling and multi-round tool cycles
MODEL=${SMOKE_MODEL:-qwen3.5:4b}
ollama list | grep -q "$MODEL" || ollama pull "$MODEL"
echo "Test model: $MODEL"
```

## LLM Tool Refusal Policy

Some tests require the LLM to call tools multiple times in sequence. If the model **refuses** to call a tool:

1. **Retry with an explicit instruction** — rephrase the prompt to make the tool call unavoidable.
2. **If the model still refuses persistently** — **FAIL the test** and note which model refused which tool.
3. **Switch to an abliterated model** — check `models.toml` for an abliterated variant.

---

## 1. Multi-Round Web Search Cycle

**Objective:** Verify that in a multi-round tool call cycle (model searches → observes results → searches again → final response), tool calls and their results from each round appear in the correct temporal order, interleaved with the model's thinking/content for that round. Tool indicators should NOT be batched at the end.

```bash
./target/release/sprachspiel --model $MODEL
```

In chat with LLM (model with tools enabled, web search available):

> **LLM Refusal:** If the model refuses to call tools, rephrase with "You MUST use the search tool to find information. Do not just describe what you would do."

- [ ] Type: "Search for the population of São Paulo, then search for its area, and give me both."
- [ ] Verify: The first search tool call (🔧 search) appears BEFORE the second search, not batched at the end.
- [ ] Verify: If the model shows thinking between tool rounds, the thinking appears AFTER the previous round's tool results, not before all tool results.
- [ ] Verify: The final response appears after ALL tool results, in the correct position.
- [ ] Verify: No duplicate content appears anywhere in the chat.

---

## 2. File Operations Cycle

**Objective:** Verify that file operations (write → read → analyze) appear in correct round order.

```bash
./target/release/sprachspiel --model $MODEL
```

- [ ] Type: "Write 'Hello World' to /tmp/test_201.txt, then read it back to verify, then tell me how many characters it has."
- [ ] Verify: The 🔧 write tool call appears first.
- [ ] Verify: The 📤 write result appears after the write call.
- [ ] Verify: The 🔧 read tool call appears after the write result, not batched with the write call.
- [ ] Verify: The final answer appears after ALL tool results.

**Cleanup:**
```bash
rm -f /tmp/test_201.txt
```

---

## 3. Calculator + Explanation Cycle

**Objective:** Verify that a multi-round math calculation (calc → interpretation → calc again) shows each round in correct temporal order.

```bash
./target/release/sprachspiel --model $MODEL
```

- [ ] Type: "Calculate 42 * 17, then tell me if the result is prime, and if not, calculate its largest prime factor."
- [ ] Verify: The first calc tool call appears first.
- [ ] Verify: The model's interpretation (if any) appears after the first calc result, not before all tool results.
- [ ] Verify: The second calc call appears after the first result + interpretation.
- [ ] Verify: The final answer appears at the end, after all tool results.

---

## 4. No-Tool Cycle (Regression)

**Objective:** Verify that a single-round response (no tool calls) works identically to before. This is a regression test — the round_index mechanism should be invisible for simple queries.

```bash
./target/release/sprachspiel --model $MODEL
```

- [ ] Type: "What is the capital of France?"
- [ ] Verify: The response appears normally, no tool indicators, no thinking block (unless model has thinking enabled).
- [ ] Verify: The response renders correctly as markdown.
- [ ] Verify: Status bar shows "Idle" after response completes.

---

## 5. Error-During-Tool Cycle

**Objective:** Verify that when a tool errors out and the model retries or provides a fallback, the round structure is still correct and the round counter resets properly.

```bash
./target/release/sprachspiel --model $MODEL
```

- [ ] Type: "Search for 'xyzzy12345nonexistent' and if you get no results, search for 'weather in Tokyo' instead."
- [ ] Verify: The first search result (empty or error) appears before the second search call.
- [ ] Verify: The second search result appears after the second search call, in correct position.
- [ ] Verify: The round counter resets to 0 after the response completes (no residual state).
- [ ] Verify: A subsequent simple query (like "What time is it?") works normally without tool ordering issues.

---

## 6. Round Counter Reset Verification

**Objective:** Verify that the round counter resets to 0 after each user prompt, ensuring no cross-contamination between independent queries.

```bash
./target/release/sprachspiel --model $MODEL
```

- [ ] Type: "Search for the elevation of Mount Everest." (multi-round tool call expected)
- [ ] Wait for the response to complete fully.
- [ ] Type: "What is 2+2?" (no tool call expected)
- [ ] Verify: The second response (no tools) appears normally, with no tool indicators from the first query.
- [ ] Type: "Search for the capital of Japan." (another multi-round tool call)
- [ ] Verify: The tool calls in the third query appear in correct temporal order, starting fresh.

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
- [ ] `cargo fmt` passed
- [ ] Documentation reviewed (CHANGELOG updated)
- [ ] PR reviewed and approved