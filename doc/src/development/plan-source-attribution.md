# Implementation Plan: Source Attribution

**Status:** Ready for implementation
**Priority:** HIGH (Memory Enhancement Phase 1)
**Estimated effort:** 1-2 days
**Created:** 2026-03-04
**Updated:** 2026-03-04

## Overview

Enable the LLM to cite sources in responses using message IDs. Prepare infrastructure for future source types (documents, notes) with backwards compatibility.

## Design Decisions

### ID Format and Prefixes

| Source Type | ID Format | Citation Example | remember() Call |
|-------------|-----------|------------------|------------------|
| Conversation | `msg:N` or just `N` | `[msg:42]` or `[42]` | `remember(id="msg:42")` or `remember(id="42")` |
| Document | `doc:N` | `[doc:13]` | `remember(id="doc:13")` |
| Note | `note:N` | `[note:7]` | `remember(id="note:7")` |
| Web | `web:N` | `[web:5]` | `remember(id="web:5")` |

**Key decisions:**

1. **Original IDs, no renumbering** - IDs come directly from database (42, 103, etc.), not sequential (1, 2, 3)
2. **Prefix format for future-proofing** - `msg:42` for conversations, `doc:13` for documents
3. **Backwards compatibility** - `remember(id="42")` works, assumes `msg:42`
4. **Citations use prefixes** - `[msg:42]` explicitly shows source type
5. **English in prompts** - System prompts in English, LLM responds in user's language

### Context Format

```xml
<retrieved_context>
MESSAGES FROM YOUR PAST CONVERSATION with this user.

Each message has an ID. Use remember(id="N") for full content or remember(query="topic") to search.

CITATIONS: When referencing retrieved content, include the source ID after the statement.
- Conversations: [msg:N] or just [N]
- Documents: [doc:N]
- Notes: [note:N]

Example: "As we discussed [msg:42], the project uses Rust."

<message id="42">
<role>user</role>
<content>What about Wittgenstein?</content>
<timestamp>2024-01-15 14:30</timestamp>
</message>
<message id="43">
<role>assistant</role>
<content>Wittgenstein had two philosophical phases...</content>
</message>
</retrieved_context>
```

## Implementation

### Step 1: Add SourceType Enum

**File:** `src/db/operations.rs`

Add the `SourceType` enum with `Conversation` as the default:

```rust
/// Source type for retrieved content
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SourceType {
    Conversation,
    Document,
    Note,
    Web,
}

impl Default for SourceType {
    fn default() -> Self {
        SourceType::Conversation
    }
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::Conversation => write!(f, "conversation"),
            SourceType::Document => write!(f, "document"),
            SourceType::Note => write!(f, "note"),
            SourceType::Web => write!(f, "web"),
        }
    }
}

impl SourceType {
    /// Get the prefix for this source type (e.g., "msg" for Conversation)
    pub fn prefix(&self) -> &'static str {
        match self {
            SourceType::Conversation => "msg",
            SourceType::Document => "doc",
            SourceType::Note => "note",
            SourceType::Web => "web",
        }
    }
    
    /// Parse a prefix string to SourceType
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
```

### Step 2: Update SearchResult Struct

**File:** `src/db/operations.rs`

Add `source_type` and `timestamp` to `SearchResult`:

```rust
pub struct SearchResult {
    pub message_id: i64,
    pub role: String,
    pub content: String,
    pub source_type: SourceType,      // NEW - default: Conversation
    pub timestamp: i64,                 // NEW - Unix timestamp
    pub next_message: Option<Box<SearchResult>>,
}

impl SearchResult {
    /// Create a new SearchResult from database row (conversation)
    pub fn from_conversation(message_id: i64, role: String, content: String, timestamp: i64) -> Self {
        Self {
            message_id,
            role,
            content,
            source_type: SourceType::Conversation,
            timestamp,
            next_message: None,
        }
    }
}
```

### Step 3: Implement ID Parsing for remember Tool

**File:** `src/tools/remember.rs`

Add parsing for prefixed IDs:

```rust
/// Parse a source ID into (SourceType, numeric_id)
/// Supports both "42" and "msg:42" formats
fn parse_source_id(id: &str) -> Result<(SourceType, i64), String> {
    if id.contains(':') {
        // Prefixed format: "msg:42", "doc:13", etc.
        let parts: Vec<&str> = id.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid ID format: {}", id));
        }
        
        let source_type = SourceType::from_prefix(parts[0])
            .ok_or_else(|| format!("Unknown source type: {}", parts[0]))?;
        
        let numeric_id = parts[1].parse::<i64>()
            .map_err(|e| format!("Invalid numeric ID '{}': {}", parts[1], e))?;
        
        Ok((source_type, numeric_id))
    } else {
        // Plain numeric ID - assume conversation for backwards compatibility
        let numeric_id = id.parse::<i64>()
            .map_err(|e| format!("Invalid ID '{}': {}", id, e))?;
        Ok((SourceType::Conversation, numeric_id))
    }
}

/// Fetch a message by ID (with source type routing)
async fn fetch_by_source(
    source_type: SourceType,
    numeric_id: i64,
    db: &Arc<Database>,
) -> Result<String, String> {
    match source_type {
        SourceType::Conversation => {
            fetch_message_by_id(numeric_id, db).await
        }
        SourceType::Document => {
            // Phase 5: Document ingestion not yet implemented
            Err("Document retrieval not yet implemented. Only conversations are supported.".to_string())
        }
        SourceType::Note => {
            // Future: Note support
            Err("Note retrieval not yet implemented. Only conversations are supported.".to_string())
        }
        SourceType::Web => {
            // Future: Web source support
            Err("Web retrieval not yet implemented. Only conversations are supported.".to_string())
        }
    }
}
```

### Step 4: Update RetrievedChunk for Context Builder

**File:** `src/retrieval/context_builder.rs`

Add source_type and timestamp fields:

```rust
pub struct RetrievedChunk {
    pub message_id: i64,
    pub role: String,
    pub content: String,
    pub source_type: SourceType,
    pub timestamp: i64,
    pub next_message: Option<Box<RetrievedChunk>>,
}
```

### Step 5: Format Timestamp Human-Readably

**File:** `src/retrieval/context_builder.rs`

```rust
/// Format timestamp for human-readable display
fn format_timestamp(timestamp: i64) -> String {
    use chrono::{TimeZone, Utc};
    
    let dt = Utc.timestamp_opt(timestamp, 0).single()
        .unwrap_or_else(|| Utc::now());
    let now = Utc::now();
    let diff = now.signed_duration_since(dt);
    
    if diff.num_hours() < 24 {
        // Today - show time only
        dt.format("%H:%M").to_string()
    } else if diff.num_days() < 7 {
        // This week - show day and time
        dt.format("%A %H:%M").to_string()
    } else if dt.year() == now.year() {
        // Same year - show month and day
        dt.format("%b %d %H:%M").to_string()
    } else {
        // Different year - show full date
        dt.format("%Y-%m-%d %H:%M").to_string()
    }
}
```

### Step 6: Update Context Formatting

**File:** `src/retrieval/context_builder.rs`

Update `build_context()` and `build_query_context()` to include citations:

```rust
/// Build the retrieved context section with citation instructions
fn format_retrieved_context(chunks: &[RetrievedChunk]) -> String {
    let mut context = String::from("<retrieved_context>\n");
    context.push_str("MESSAGES FROM YOUR PAST CONVERSATION with this user.\n\n");
    context.push_str("Each message has an ID. Use remember(id=\"N\") for full content or remember(query=\"topic\") to search.\n\n");
    context.push_str("CITATIONS: When referencing retrieved content, include the source ID after the statement.\n");
    context.push_str("- Conversations: [msg:N] or just [N]\n");
    context.push_str("- Documents: [doc:N]\n");
    context.push_str("- Notes: [note:N]\n\n");
    context.push_str("Example: \"As we discussed [msg:42], the project uses Rust.\"\n\n");
    
    for msg in chunks {
        let timestamp = format_timestamp(msg.timestamp);
        context.push_str(&format!(
            "<message id=\"{}\">\n<role>{}</role>\n<content>{}</content>\n<timestamp>{}</timestamp>\n</message>\n",
            msg.message_id,
            msg.role,
            msg.content,
            timestamp
        ));
        
        if let Some(ref next) = msg.next_message {
            let next_timestamp = format_timestamp(next.timestamp);
            context.push_str(&format!(
                "<message id=\"{}\">\n<role>{}</role>\n<content>{}</content>\n<timestamp>{}</timestamp>\n</message>\n",
                next.message_id,
                next.role,
                next.content,
                next_timestamp
            ));
        }
    }
    
    context.push_str("</retrieved_context>");
    context
}
```

### Step 7: Update Database Operations

**File:** `src/db/operations.rs`

Ensure `search_hybrid()` and `enrich_with_context()` include timestamp:

```rust
// In search_semantic() and search_keyword()
// Ensure timestamp is selected and returned

// In enrich_with_context()
// Set source_type: SourceType::Conversation for all results
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/db/operations.rs` | Add `SourceType` enum, update `SearchResult` with `source_type` and `timestamp` |
| `src/retrieval/context_builder.rs` | Add `format_timestamp()`, update context format with citations |
| `src/tools/remember.rs` | Add `parse_source_id()` for prefixed IDs |

---

## Backwards Compatibility

### For Users

- `remember(id="42")` continues to work (assumes conversation)
- `remember(id="msg:42")` also works (new format)
- Citations `[42]` and `[msg:42]` both valid

### For Database

- No schema changes required (Phase 1)
- `source_type` defaults to `Conversation`
- `timestamp` already exists in messages table

---

## Testing

### Test Cases

1. **Basic citation**
   ```
   User: What did we discuss about Rust?
   Expected: Response includes [msg:N] or [N] citations
   ```

2. **Multiple citations**
   ```
   User: Tell me about what we discussed, citing sources.
   Expected: Multiple [N] citations for different sources
   ```

3. **remember with old format**
   ```
   User: Use remember(id="42") to get that message.
   Expected: Works (backwards compatible)
   ```

4. **remember with new format**
   ```
   User: Use remember(id="msg:42") to get that message.
   Expected: Works (new prefix format)
   ```

5. **Timestamp in context**
   ```
   User: When did we talk about X?
   Expected: Context shows timestamp, LLM can reference "on Monday..."
   ```

### Manual Testing

```bash
# Start chat
ask chat

# Test citation behavior
> Remember that my favorite programming language is Rust.
> What's my favorite programming language?

# Expected: "Your favorite programming language is Rust [msg:N]." 
# or similar with citation

# Test remember tool
> Use remember(id="N") to see the full message.

# Test timestamp awareness
> When did we discuss Rust?
```

---

## Success Criteria

- [ ] LLM includes `[msg:N]` or `[N]` citations when referencing retrieved context
- [ ] Citations use actual message IDs from database
- [ ] `remember(id="42")` works (backwards compatible)
- [ ] `remember(id="msg:42")` works (new prefix format)
- [ ] Context shows human-readable timestamps
- [ ] `SourceType` enum ready for future document/note support
- [ ] Error messages clear for unimplemented source types (doc, note)

---

## Future Phases

This implementation prepares for:

- **Phase 2:** Query routing by intent
- **Phase 3:** Timestamp filtering
- **Phase 4:** Schema for documents (add `documents` table)
- **Phase 5:** Document ingestion and `doc:N` citations

When documents are implemented:
1. Add `fetch_document_by_id()` function
2. Update `fetch_by_source()` to handle `SourceType::Document`
3. Document citations `[doc:13]` will work automatically

---

## Rollback

If issues arise:
1. Remove citation instructions from context format
2. Revert `format_retrieved_context()` to previous version
3. Keep `SourceType` enum (harmless, useful for future)

No database changes to rollback.