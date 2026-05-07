---
name: tool-guidelines
description: Complete guidelines for developing LLM tools in sprachspiel: error handling philosophy, parameter types for LLM compatibility, empty string normalization, docstrings, API response structs, file write security, and output conventions.
license: MIT
compatibility: opencode
metadata:
  audience: developers
  workflow: tool-development
---

## What I do

I provide comprehensive guidelines for creating and modifying LLM tools in the sprachspiel project. I cover error handling, parameter types, security, documentation, and output conventions.

## When to use me

Load this skill when:
- Creating new LLM tools (`#[ollama_rs::function]`)
- Modifying existing tools
- Reviewing tool code for correctness
- Debugging tool crashes or deserialization failures

---

# Rule 1: Tools Must NEVER Crash

**The `?` operator and `Err()` returns will propagate errors and crash the entire tool execution. This must NEVER happen.**

```rust
// ❌ NEVER - Will crash on error
let metadata = std::fs::metadata(&path)?;
let content = std::fs::read_to_string(&path)?;
let parsed = some_str.parse::<u32>()?;

// ✅ ALWAYS - Returns helpful error to LLM
let metadata = match std::fs::metadata(&path) {
    Ok(m) => m,
    Err(e) => {
        let err_msg = format!("Error: Cannot read file metadata: {}", e);
        log_tool_result("my_tool", &err_msg);
        return Ok(err_msg);
    }
};
```

**When can errors crash?** Only for catastrophic errors that should stop the ENTIRE APPLICATION:
- Application startup failures
- Configuration loading errors
- Database connection failures

Tools should ALWAYS return `Ok(String)` with either success or error message.

# Rule 2: Use String for Numeric Parameters

**LLMs often send parameters as strings instead of proper JSON types.** This causes deserialization failures that crash tools.

## Never Use These Types

| Type | Why It's Dangerous | Use Instead |
|------|-------------------|-------------|
| `Option<usize>` | Fails on `"100"` or `"null"` | `Option<String>` |
| `Option<u32>` | Fails on `"30"` or `"null"` | `Option<String>` |
| `Option<i32>` | Fails on `"-1"` or `"null"` | `Option<String>` |
| `usize` (required) | Fails on `"5"` or empty string | `String` + validate |
| `bool` (optional) | May work, but `Option<String>` is safer | `Option<String>` → `parse_bool()` |

## Correct Pattern

```rust
// ✅ ALWAYS USE String/Option<String>, parse internally
#[ollama_rs::function]
pub async fn my_tool(
    path: String,
    max_lines: Option<String>,      // ✅ Accepts "100", 100, "null", null
    timeout: Option<String>,       // ✅ Accepts "30", 30, "null", null
) -> Result<String, ...> {
    let max_lines_val: Option<usize> = max_lines.as_deref().and_then(|m| m.parse().ok());
    let timeout_val: Option<u32> = timeout.as_deref().and_then(|t| t.parse().ok());
    let lines = max_lines_val.unwrap_or(usize::MAX);
}
```

## Required Numeric Parameters

Still use `String` but validate early:

```rust
let start: usize = start_line.parse()
    .map_err(|_| format!("Error: Invalid start_line '{}'. Must be a positive number.", start_line))?;

if start == 0 {
    let err_msg = "Error: start_line must be 1 or greater. Line numbers start at 1.".to_string();
    log_tool_result("my_tool", &err_msg);
    return Ok(err_msg);
}
```

# Rule 3: Normalize Empty Strings

**LLMs frequently send `""` (empty string) instead of omitting optional parameters.**

When `Some("")` should be treated the same as `None`, normalize at the start:

```rust
// ✅ CORRECT - Normalize empty strings to None
let title = title.filter(|s| !s.is_empty());
let content = content.filter(|s| !s.is_empty());

// Now is_none() works correctly for both None and Some("")
if title.is_none() && content.is_none() {
    return Ok("Error: Provide at least one field".to_string());
}
```

**When NOT to normalize:**
- Empty string is a valid value (e.g., `replace: Option<String>` in edit_file)
- The parameter is boolean/numeric (use `parse_bool()`/`parse_u32()` instead)
- The parameter already validates content

# Rule 4: API Response Structs

**ALWAYS make fields optional with `#[serde(default)]`:**

```rust
// ❌ BAD - Will crash if API doesn't return this field
#[derive(Deserialize)]
struct ApiResponse {
    data: Vec<Item>,
}

// ✅ GOOD - Handles missing fields gracefully
#[derive(Deserialize, Default)]
struct ApiResponse {
    #[serde(default)]
    data: Vec<Item>,
    #[serde(default)]
    metadata: Metadata,
}
```

# Rule 5: Network Requests

**Always wrap with proper error handling:**

```rust
let response = match client.get(&url).send().await {
    Ok(r) => r,
    Err(e) => {
        let err = format!("Network error: {}. Please try again later.", e);
        log_tool_result("my_tool", &err);
        return Ok(err);
    }
};

let data: MyStruct = match response.json().await {
    Ok(d) => d,
    Err(e) => {
        let err = format!("Error parsing response: {}. Please try again later.", e);
        log_tool_result("my_tool", &err);
        return Ok(err);
    }
};
```

# Rule 6: Logging Debug Output

**Always log tool calls and results:**

```rust
use crate::debug_tools::{log_tool_call, log_tool_result};

pub async fn my_tool(param: String) -> Result<String, ...> {
    log_tool_call("my_tool", &[("param".to_string(), param.clone())]);
    // ... do work ...
    let result = "...";
    log_tool_result("my_tool", &result);
    Ok(result)
}
```

For optional parameters, show defaults in debug output, not empty strings:

```rust
log_tool_call(
    "read_file",
    &[
        ("path".to_string(), path.clone()),
        ("max_lines".to_string(), max_lines.map(|l| l.to_string()).unwrap_or_else(|| "all".to_string())),
    ],
);
```

# Rule 7: Tool Output Language

**All tool output must be in English.** Error messages, result formatting, and descriptions in English only.

# Rule 8: File Write Security

File write operations have additional security requirements:

1. **Always sandboxed** — Sandbox always enforced, cannot be disabled
2. **Blocked patterns** — Sensitive files always blocked (`.env`, `id_rsa`, `.pem`, etc.)
3. **Size limits** — Maximum 5MB per write
4. **UTF-8 only** — Binary content rejected
5. **Atomic writes** — Use temp file + rename

```rust
// MUST call validate_write_path() before ANY write operation
let canonical_path = match validate_write_path(&path, &config) {
    Ok(p) => p,
    Err(e) => {
        log_tool_result("my_tool", &e);
        return Ok(e);
    }
};
```

# Rule 9: File Size Output

Always show human-readable format:

```rust
let kb = metadata.len() as f64 / 1024.0;
let size_info = if kb >= 1024.0 {
    format!(" ({:.1} MB)", kb / 1024.0)
} else {
    format!(" ({:.0} KB)", kb)
};
```

# Rule 10: Docstrings

Every tool function MUST have a docstring:

1. **One-line summary** — What it does (imperative mood)
2. **Extended description** — When to use, returns
3. **Parameter documentation** — Name, description, default, example
4. **Returns** — Format of successful output
5. **Errors** — Common error conditions
6. **Examples** — Realistic function call examples

```rust
/// Search the web using Google via Serper.dev API.
///
/// Returns search results with title, URL, and snippet for each result.
/// Use this tool when you need to find current information on the internet.
///
/// # Arguments
/// * `query` - The search query. Be specific for better results.
///   - Example: "Rust async programming best practices"
/// * `num_results` - Number of results (default: 5, max: 10). Optional.
///
/// # Returns
/// Formatted search results. Error message if API fails.
#[ollama_rs::function]
pub async fn web_search(query: String, num_results: Option<String>) -> Result<String, ...>
```

# Common Tool Bugs Checklist

When reviewing or creating tools:

1. ❌ Missing `log_tool_call` at start
2. ❌ Missing `log_tool_result` before every return
3. ❌ Using `?` operator instead of match
4. ❌ Using `Err()` returns instead of `Ok(error_message)`
5. ❌ Non-optional struct fields for API responses
6. ❌ Missing error handling for network requests
7. ❌ Missing error handling for JSON parsing
8. ❌ Missing error handling for file operations
9. ❌ Using `Option<usize>/Option<u32>` instead of `Option<String>`
10. ❌ Missing `.filter(|s| !s.is_empty())` for truly optional text parameters