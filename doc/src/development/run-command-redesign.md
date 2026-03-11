# run_command Redesign - Security & Usability

**Date:** 2026-03-10  
**Status:** Implementation Ready  
**Related:** CLI Tools Infrastructure (Phase 1)

## Executive Summary

Redesign `run_command` tool with:
- **No shell features** (pipes, redirects blocked for security)
- **Mandatory whitelist** (single command validation)
- **head/tail parameters** (LLM controls truncation)
- **Landlock sandbox** (enabled by default on Linux)
- **No automatic truncation** (full output returned)

## Design Decisions

### 1. Security Model

| Feature | Decision | Rationale |
|---------|----------|-----------|
| Shell features (pipes, redirects) | ❌ Blocked | Prevent command injection and bypass attacks |
| Whitelist | ✅ Mandatory | Only whitelisted tools can execute |
| Command parsing | Single command only | Block `;`, `&&`, `||`, `$()`, backticks |
| Sandbox | ✅ Landlock by default (Linux) | Filesystem isolation |
| Sandbox on Termux | ❌ Not available | Android provides app-level isolation |
| Sandbox on macOS | ❌ Not yet supported | Future: `sandbox-exec` |

### 2. Output Control

**Previous behavior:** Automatic truncation at 4000 tokens

**New behavior:** Full output + LLM-controlled truncation via parameters

```rust
pub async fn run_command(
    command_line: String,
    head: Option<usize>,      // First N lines
    tail: Option<usize>,      // Last N lines
    timeout_seconds: Option<u32>,
) -> Result<String, ...>
```

**head/tail combinations:**
- `head=100, tail=null` → First 100 lines
- `head=null, tail=50` → Last 50 lines
- `head=50, tail=50` → First 50 + `[truncated]` + last 50
- `head=null, tail=null` → Full output

### 3. Platform Support

| Platform | Sandbox | Behavior |
|----------|---------|----------|
| Linux (kernel 5.13+) | ✅ Landlock | Filesystem isolation enforced |
| Linux (older) | ⚠️ Graceful | Warning, continues without sandbox |
| Termux | ❌ None | Warning if `enable_sandbox=true` |
| macOS | ❌ None | Warning if `enable_sandbox=true` |
| Windows | ❌ Not supported | Use WSL |

## Blocked Patterns

These patterns are **rejected** before execution:

```
|        (pipe)
;        (command separator)
&&       (AND operator)
||       (OR operator)
$(       (command substitution)
`        (backtick substitution)
>        (redirect output)
<        (redirect input)
>>       (append output)
<<       (here-document)
```

**Error message:** `Error: Shell feature '|' is not allowed. Use tool-specific flags instead.`

## Whitelist Validation

Only the **first command** is validated against whitelist:

```rust
// OK: pdftotext is whitelisted
"pdftotext -f 1 -l 5 document.pdf -"

// BLOCKED: head is not whitelisted (but would be blocked by pipe anyway)
"pdftotext file.pdf - | head"
```

## Landlock Sandbox

### Configuration

```toml
[external]
enable_sandbox = true  # Default: true on Linux
default_timeout = 30
```

### Allowed Paths (Default)

| Path | Access | Purpose |
|------|--------|---------|
| CWD | Read/Write | User's working directory |
| `/usr` | Read-only | System binaries |
| `/lib`, `/lib64` | Read-only | Shared libraries |
| `/etc` | Read-only | System configuration |
| `/tmp` | Read/Write | Temporary files |

### Implementation

```rust
#[cfg(all(feature = "sandbox", target_os = "linux"))]
fn apply_sandbox_if_enabled(config: &ExternalToolsConfig) -> Result<(), String> {
    if !config.enable_sandbox {
        return Ok(());
    }
    
    use landlock::*;
    
    let status = Ruleset::new()
        .handle_access(AccessFs::from_all(ABI::V1))?
        .create()?
        .add_rule(PathBeneath::new(CWD, AccessFs::ReadFile | AccessFs::WriteFile | AccessFs::ReadDir))?
        .add_rule(PathBeneath::new("/usr", AccessFs::ReadFile | AccessFs::ReadDir))?
        .restrict_self()?;
    
    if !status.ruleset.is_fully_enforced() {
        eprintln!("Warning: Landlock not fully enforced");
    }
    
    Ok(())
}
```

## Threat Model

### What Landlock Prevents

✅ Reading sensitive files:
- `~/.ssh/id_rsa`
- `/etc/shadow`
- Other users' home directories

✅ Writing to system locations:
- `/usr/bin`
- `/etc`
- Other users' files

### What Landlock Does NOT Prevent

❌ Network access (unless kernel 6.7+ with ABI v4)

❌ Resource exhaustion:
- Fork bombs
- Infinite loops
- Memory exhaustion

❌ Process execution:
- Running whitelisted tools
- Spawning child processes

### Mitigations

| Threat | Mitigation |
|--------|------------|
| Network exfiltration | Whitelist limits tools, Landlock limits file access |
| Resource exhaustion | Timeout (configurable per tool) |
| Malicious commands | Whitelist + pattern blocking + sandbox |

## Migration Guide

### For LLMs

**Before (with pipes):**
```
run_command("pdftotext file.pdf - | head -100")
```

**After (with parameters):**
```
run_command("pdftotext file.pdf -", 100, null, null)
```

**Before (with truncation):**
```
// Automatic truncation at 4000 tokens
run_command("pdftotext huge.pdf -")
```

**After (explicit control):**
```
// LLM decides what to request
run_command("pdftotext huge.pdf -", 100, null, null)  // First 100 lines
run_command("pdftotext -f 1 -l 10 huge.pdf -", null, null, null)  // Pages 1-10
run_command("pdftotext huge.pdf -", null, null, null)  // Full (be careful!)
```

### For Users

**Enable sandbox (default on Linux):**
```toml
[external]
enable_sandbox = true
```

**Disable sandbox (not recommended):**
```toml
[external]
enable_sandbox = false
```

**Compile with sandbox support:**
```bash
cargo build --features sandbox
```

**Compile without sandbox support:**
```bash
cargo build
```

## Examples

### PDF Extraction

```
// Preview first page
run_command("pdftotext -f 1 -l 1 document.pdf -", 50, null, null)

// Extract conclusion (last 50 lines)
run_command("pdftotext document.pdf -", null, 50, null)

// Extract specific pages
run_command("pdftotext -f 5 -l 10 document.pdf -", null, null, null)

// Full document (careful with size!)
run_command("pdftotext document.pdf -", null, null, null)
```

### OCR

```
// OCR with language
run_command("tesseract image.png stdout -l jpn", null, null, 120)

// Preview OCR result
run_command("tesseract image.png stdout", 20, null, 60)
```

### Metadata

```
// Image metadata
run_command("exiftool photo.jpg", null, null, null)

// PDF metadata
run_command("pdfinfo document.pdf", null, null, null)
```

## Implementation Checklist

- [x] Document design decisions
- [x] Add `landlock` dependency to Cargo.toml (feature flag `sandbox`)
- [x] Add `enable_sandbox` to ExternalToolsConfig
- [x] Parse `enable_sandbox` in config.rs
- [x] Implement new run_command signature
- [x] Implement pattern validation
- [x] Implement head/tail logic
- [x] Implement Landlock sandbox (Linux)
- [x] Implement sandbox warnings (non-Linux)
- [x] Remove automatic truncation from custom_coordinator.rs
- [x] Update EXTERNAL TOOLS prompt
- [x] Update FILE TOOLS prompt
- [x] Add unit tests (14 tests)
- [x] Update tools.toml template with security documentation
- [ ] Manual testing with --debug
- [x] Update IMPLEMENTATION.md

## References

- [Landlock Documentation](https://landlock.io/)
- [Landlock Rust Crate](https://docs.rs/landlock/)
- [OpenCode Security Model](https://github.com/opencode-ai/opencode/blob/main/SECURITY.md)
- [Claude Code Sandbox Runtime](https://www.npmjs.com/package/@anthropic-ai/sandbox-runtime)
