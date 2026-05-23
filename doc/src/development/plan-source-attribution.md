# Implementation Plan: Source Attribution

**Status:** Completed
**Priority:** HIGH (Memory Enhancement Phase 1)
**Estimated effort:** 1-2 days
**Created:** 2026-03-04
**Updated:** 2026-03-04
**Completed:** 2026-03-04

## Overview

Enable the LLM to cite sources in responses using message IDs. Prepare infrastructure for future source types (documents, notes) with prefixed IDs.

## Design Decisions

### ID Format and Prefixes

| Source Type | ID Format | Citation Example | remember() Call |
|-------------|-----------|------------------|------------------|
| Conversation | `msg:N` | `[msg:42]` | `remember(id="msg:42")` |
| Document | `doc:N` | `[doc:13]` | `remember(id="doc:13")` |
| Note | `note:N` | `[note:7]` | `remember(id="note:7")` |
| Web | `web:N` | `[web:5]` | `remember(id="web:5")` |

**Key decisions:**

1. **IDs always have prefix** - `msg:42`, not just `42`
2. **No backwards compatibility** - `id="42"` returns error, must use `id="msg:42"`
3. **Consistent format everywhere** - context, citations, and remember tool all use same format
4. **English in prompts** - System prompts in English, LLM responds in user's language

### Context Format

```xml
<retrieved_context>
MESSAGES FROM YOUR PAST CONVERSATION with this user.

Each message has an ID. Use remember(id="msg:N") for full content or remember(query="topic") to search.

CITATIONS: When referencing retrieved content, include the source ID after the statement.
- Conversations: [msg:N]
- Documents: [doc:N]
- Notes: [note:N]

Example: "As we discussed [msg:42], the project uses Rust."

<message id="msg:42">
<role>user</role>
<content>What about Wittgenstein?</content>
<timestamp>2024-01-15 14:30</timestamp>
</message>
</retrieved_context>
```

### Error Message for Invalid IDs

When user/LLM provides `id="42"` (without prefix):

```
Error: Invalid ID format: '42'. Must include source type prefix.
Use: remember(id="msg:42") for conversations
Use: remember(id="doc:13") for documents
Use: remember(id="note:7") for notes
```

## Implementation

### Step 1: SourceType Enum and SearchResult

**File:** `src/db/operations.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Conversation,
    Document,
    Note,
    Web,
}

impl SourceType {
    pub fn prefix(&self) -> &'static str {
        match self {
            SourceType::Conversation => "msg",
            SourceType::Document => "doc",
            SourceType::Note => "note",
            SourceType::Web => "web",
        }
    }
    
    pub fn from_prefix(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "msg" | "conversation" => Some(SourceType::Conversation),
            "doc" | "document" => Some(SourceType::Document),
            "note" => Some(SourceType::Note),
            "web" => Some(SourceType::Web),
            _ => None,
        }
    }
}

pub struct SearchResult {
    // ... other fields
    pub source_type: SourceType,  // NEW
    // ...
}
```

### Step 2: Context Format with Prefixed IDs

**File:** `src/retrieval/context_builder.rs`

```rust
fn format_retrieved_context(results: &[SearchResult]) -> String {
    let mut text = String::from("<retrieved_context>\n");
    text.push_str("MESSAGES FROM YOUR PAST CONVERSATION with this user.\n\n");
    text.push_str("Each message has an ID. Use remember(id=\"msg:N\") for full content or remember(query=\"topic\") to search.\n\n");
    text.push_str("CITATIONS: When referencing retrieved content, include the source ID after the statement.\n");
    text.push_str("- Conversations: [msg:N]\n");
    text.push_str("- Documents: [doc:N]\n");
    text.push_str("- Notes: [note:N]\n\n");
    text.push_str("Example: \"As we discussed [msg:42], the project uses Rust.\"\n\n");
    
    for msg in results {
        text.push_str(&format!(
            "<message id=\"msg:{}\">\n<role>{}</role>\n<content>{}</content>\n<timestamp>{}</timestamp>\n</message>\n",
            msg.message_id,  // prefixed with "msg:"
            msg.role,
            msg.content,
            format_timestamp(msg.timestamp)
        ));
        
        if let Some(ref answer) = msg.next_message {
            text.push_str(&format!(
                "<message id=\"msg:{}\">\n<role>{}</role>\n<content>{}</content>\n<timestamp>{}</timestamp>\n</message>\n",
                answer.message_id,
                answer.role,
                answer.content,
                format_timestamp(answer.timestamp)
            ));
        }
    }
    
    text.push_str("</retrieved_context>");
    text
}
```

### Step 3: ID Parsing (No Backwards Compatibility)

**File:** `src/tools/remember.rs`

```rust
fn parse_source_id(id: &str) -> Result<(SourceType, i64), String> {
    if let Some(pos) = id.find(':') {
        let prefix = &id[..pos];
        let num_str = &id[pos+1..];
        
        let source_type = SourceType::from_prefix(prefix)
            .ok_or_else(|| format!(
                "Unknown source type: '{}'. Valid types: msg, doc, note, web",
                prefix
            ))?;
        
        let num = num_str.parse::<i64>()
            .map_err(|e| format!("Invalid ID number: {}", e))?;
        
        Ok((source_type, num))
    } else {
        Err(format!(
            "Invalid ID format: '{}'. Must include source type prefix.\n\
             Use: remember(id=\"msg:42\") for conversations\n\
             Use: remember(id=\"doc:13\") for documents\n\
             Use: remember(id=\"note:7\") for notes",
            id
        ))
    }
}
```

### Step 4: remember Tool Updates

**File:** `src/tools/remember.rs`

Docstring with examples:

```rust
/// Recall messages from your conversation history.
///
/// Use this tool to:
/// 1. Get the full content of a specific message (by ID from retrieved context)
/// 2. Search for topics not in the current context (by query)
///
/// # Arguments
/// * `id` - ID of message to retrieve (MUST include prefix). Optional.
///   - Example: "msg:42" for conversation message
///   - Example: "doc:13" for document (when implemented)
/// * `query` - Search query for semantic search. Optional.
///   - Example: "Wittgenstein" to find messages about that topic
/// * `limit` - Max results for query (default: 5, max: 10). Optional.
///
/// # Returns
/// - For id: Full message content with metadata
/// - For query: List of matching messages with IDs and excerpts
///
/// # Examples
/// ```ignore
/// remember(id="msg:42")              // Get conversation message 42
/// remember(query="Wittgenstein")     // Search by topic
/// remember(query="philosophy", limit="10")
/// ```
```

---

## Files Modified

| File | Changes |
|------|---------|
| `src/db/operations.rs` | Add `SourceType` enum, update `SearchResult.source_type` |
| `src/db/mod.rs` | Export `SourceType` |
| `src/retrieval/context_builder.rs` | `format_timestamp()`, `format_retrieved_context()` with prefixed IDs |
| `src/tools/remember.rs` | `parse_source_id()`, updated docstring, remove backwards compat |

---

## Backwards Compatibility

**No backwards compatibility.** 

Users must use prefixed IDs:
- ✅ `remember(id="msg:42")` - Works
- ❌ `remember(id="42")` - Returns error with instructions

This ensures consistency and prevents confusion.

---

## Testing

```bash
sprach chat

# Test prefixed ID
> Remember that my favorite programming language is Rust.
> Use remember(id="msg:1") to see that message.

# Expected: Message content displayed

# Test invalid ID (no prefix)
> Use remember(id="42")

# Expected: Error with instructions on correct format

# Test citation
> What's my favorite programming language?

# Expected: Response includes [msg:N] citation
```

---

## Success Criteria

- [x] LLM includes `[msg:N]` citations when referencing retrieved context
- [x] Citations use prefixed IDs (`msg:42`, not just `42`)
- [x] Context shows `<message id="msg:N">` with prefix
- [x] `remember(id="msg:42")` works correctly
- [x] `remember(id="42")` returns clear error message
- [x] Context shows human-readable timestamps
- [x] `SourceType` enum ready for future document/note support