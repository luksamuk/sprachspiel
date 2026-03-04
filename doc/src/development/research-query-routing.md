# Research: Query Routing

**Status:** Research in progress
**Created:** 2026-03-04
**Priority:** HIGH (Memory Enhancement Phase 1)

## Problem Statement

We need to route queries to appropriate search targets based on intent:

- "lembra o que eu falei ontem?" → Memory (conversations)
- "o que está no PDF?" → Documents
- "como está o tempo?" → None (skip search)

**Challenge:** How to do this across multiple languages without relying on LLM calls?

## Languages to Support

| Language | Code | Priority |
|----------|------|----------|
| English | en | High |
| Portuguese | pt-BR | High |
| Spanish | es | Medium |
| French | fr | Medium |
| German | de | Medium |

## Approaches

### 1. Rule-Based (Regex + Keywords)

**Pros:**
- Zero cost (no LLM call)
- Fast (<1ms)
- Deterministic
- Easy to debug

**Cons:**
- Requires patterns for each language
- May miss edge cases
- Needs maintenance for new patterns

**Implementation:**
```rust
enum SearchTarget {
    Memory,     // conversations
    Documents,  // docs/PDFs
    Notes,      // user notes
    All,        // default
    None,       // skip search
}

fn route_query(query: &str, lang: &str) -> SearchTarget {
    let query_lower = query.to_lowercase();
    
    // Small talk patterns (per language)
    let small_talk = match lang {
        "pt-BR" => vec!["tempo", "horas", "bom dia", "oi", "olá", "tudo bem"],
        "en" => vec!["weather", "time", "good morning", "hi", "hello", "how are you"],
        _ => vec![],
    };
    
    if small_talk.iter().any(|p| query_lower.contains(p)) {
        return SearchTarget::None;
    }
    
    // Memory patterns
    let memory_patterns = match lang {
        "pt-BR" => vec!["lembra", "conversa", "falamos", "sobre isso", "quando eu disse"],
        "en" => vec!["remember", "conversation", "we talked", "about that", "when i said"],
        _ => vec![],
    };
    
    if memory_patterns.iter().any(|p| query_lower.contains(p)) {
        return SearchTarget::Memory;
    }
    
    // Document patterns
    // ...
    
    SearchTarget::All
}
```

### 2. Intent Classification (Lightweight Model)

**Options:**

| Model | Size | Latency | Languages |
|-------|------|---------|-----------|
| FastText classifier | ~10MB | ~5ms | Multilingual |
| DistilBERT NLI | ~250MB | ~50ms | Multilingual |
| Local embedding similarity | ~300MB | ~20ms | Multilingual |

**Pros:**
- More robust than regex
- Better generalization

**Cons:**
- Additional model loading
- Latency overhead
- Maintenance of classification model

### 3. Embedding-Based Intent Matching

Pre-compute embeddings for intent examples:

```
Intent: Memory
Examples: ["remember what I said", "lembra o que eu falei", ...]
Embedding: average of examples

Intent: Documents
Examples: ["what's in the PDF", "o que está no documento", ...]
Embedding: average of examples
```

At query time:
1. Embed query
2. Compare to intent centroids
3. Route to closest intent (if above threshold)

**Pros:**
- Language agnostic (same embedding model)
- No regex patterns to maintain
- Works with paraphrases

**Cons:**
- Requires embedding call (already using nomic-embed-text)
- Threshold tuning needed
- Overhead: ~20-50ms

### 4. Hybrid (Regex + Fallback Embedding)

1. Try regex patterns first (fast path)
2. If no match, use embedding similarity (fallback)

**Pros:**
- Best of both worlds
- Common cases handled fast
- Edge cases handled by embedding

**Cons:**
- More complex implementation
- Two code paths

## Language Detection

If we go with regex approach, we need language detection:

### Option A: Fast Language Detection

```rust
use whatlang::detect;

fn detect_language(text: &str) -> Option<&'static str> {
    detect(text).map(|info| match info.lang() {
        Lang::Eng => "en",
        Lang::Por => "pt-BR",
        Lang::Spa => "es",
        Lang::Fra => "fr",
        Lang::Deu => "de",
        _ => "en", // default
    })
}
```

**Crate:** `whatlang` (~1MB, no dependencies)

### Option B: Accept User-Configured Language

Add to `config.toml`:
```toml
[general]
language = "pt-BR"  # or "en", "es", etc.
```

**Pros:**
- Zero runtime cost
- User control

**Cons:**
- Requires configuration
- Doesn't auto-detect

### Option C: First Message Language Detection

Detect language from first few messages in conversation, cache result.

## Research Questions

1. **What's the latency budget for routing?**
   - Regex: <1ms ✅
   - Embedding: ~20-50ms (acceptable?)
   - Light model: ~50-100ms (too slow?)

2. **Do we need multilingual support on day 1?**
   - Start with pt-BR + en only?
   - Expand later?

3. **What patterns are most common?**
   - Need to analyze real query logs
   - Create pattern library

4. **Can we use existing embedding infrastructure?**
   - Already using nomic-embed-text for RAG
   - Intent embeddings could reuse same model
   - Overhead: just cosine similarity calculation

## Next Steps

- [ ] Collect real query patterns from usage
- [ ] Prototype regex routing (pt-BR + en)
- [ ] Benchmark embedding-based routing
- [ ] Test `whatlang` for language detection
- [ ] Compare accuracy vs latency trade-offs

## Decision Criteria

| Factor | Weight | Regex | Embedding | Hybrid |
|--------|--------|--------|-----------|--------|
| Latency | High | ✅ <1ms | ⚠️ ~30ms | ✅ <5ms avg |
| Accuracy | High | ⚠️ 85% | ✅ 95% | ✅ 92% |
| Multilingual | Medium | ⚠️ Manual | ✅ Automatic | ✅ Automatic |
| Maintenance | Medium | ⚠️ High | ✅ Low | ⚠️ Medium |
| Implementation | Medium | ✅ Simple | ⚠️ Medium | ⚠️ Complex |

## Recommendation (Tentative)

**Phase 1:** Start with regex (pt-BR + en only)
- Fast, simple, good enough for common cases
- Add language detection via `whatlang`

**Phase 2:** Add embedding fallback for edge cases
- Reuse nomic-embed-text model
- Pre-compute intent centroids
- Threshold at 0.75 cosine similarity

**Phase 3:** Expand pattern library based on usage data

---

## Implementation Sketch (Phase 1 - Regex)

```rust
// src/retrieval/query_router.rs

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchTarget {
    Memory,     // conversations only
    Documents,  // documents/files only
    Notes,      // user notes only
    All,        // all sources (default)
    None,       // skip search entirely
}

pub struct QueryRouter {
    patterns: HashMap<&'static str, Vec<&'static str>>,
}

impl QueryRouter {
    pub fn new() -> Self {
        let mut patterns = HashMap::new();
        
        // Small talk (skip search)
        patterns.insert("pt-BR", vec![
            "tempo", "horas", "bom dia", "boa tarde", "boa noite",
            "oi", "olá", "tudo bem", "como vai", "obrigad", "valeu",
        ]);
        patterns.insert("en", vec![
            "weather", "time", "good morning", "good afternoon", "good evening",
            "hi", "hello", "how are you", "thanks", "thank you",
        ]);
        
        // Memory patterns (search conversations)
        patterns.insert("pt-BR", vec![
            "lembra", "conversa", "falamos", "sobre isso", 
            "quando eu disse", "o que eu falei", "você disse",
        ]);
        patterns.insert("en", vec![
            "remember", "conversation", "we talked", "about that",
            "when i said", "what i said", "you said",
        ]);
        
        // Document patterns (search documents)
        patterns.insert("pt-BR", vec![
            "no pdf", "no documento", "no arquivo", "no texto",
            "está escrito", "o que diz",
        ]);
        patterns.insert("en", vec![
            "in the pdf", "in the document", "in the file", "in the text",
            "what does it say", "is written",
        ]);
        
        Self { patterns }
    }
    
    pub fn route(&self, query: &str, lang: &str) -> SearchTarget {
        let query_lower = query.to_lowercase();
        let lang_patterns = self.patterns.get(lang)
            .or_else(|| self.patterns.get("en"))
            .expect("default patterns");
        
        // Check small talk (skip search)
        if lang_patterns.iter().any(|p| query_lower.contains(p)) {
            // This is a simplified version - need separate pattern lists
            // for small_talk, memory, documents
        }
        
        // TODO: Implement full logic
        SearchTarget::All
    }
}
```

**Status:** Sketch only, needs refinement