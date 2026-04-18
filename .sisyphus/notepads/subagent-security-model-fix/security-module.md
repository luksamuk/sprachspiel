# Security Module Implementation Notes

## Pattern Reference

The security module was implemented following the existing `validate_path()` pattern in `src/tools/files.rs`:

1. **CWD sandbox enforcement**: Files must be within current working directory
2. **Temporary directory exception**: `/tmp` and `/var/tmp` are allowed for tool interop
3. **Blocklist check**: Sensitive files (.env, secrets, SSH keys, etc.) are always rejected
4. **Cached BlocklistConfig**: Loaded once using `once_cell::sync::Lazy`

## Implementation Decisions

### Why Allow CWD Sandbox + /tmp?

- CWD: Most file operations should be project-local for security
- /tmp, /var/tmp: Required for tool interoperability (e.g., `pdftotext` output)

### Why Cache BlocklistConfig?

The `BlocklistConfig::load()` function reads from `tools.toml` on every call. Since:
1. The config is loaded at application startup
2. The blocklist is loaded once from TOML
3. Subagent calls may happen frequently for vision/OCR

Caching with `Lazy` ensures we don't reload the config on every path validation.

### Why Use `#[allow(dead_code)]`?

The module provides functions that will be used by subagent tools (which don't exist yet).
Until those tools are implemented:
- The functions appear "dead"
- Clippy with `-D warnings` would fail

This follows the pattern in AGENTS.md: unused public functions for future features should be marked with `#[allow(dead_code)]`.

### Error Handling

Following AGENTS.md tool error philosophy:
- All errors return `Ok(String)` format messages for the LLM
- No `?` operator or `Err(Box<dyn Error>)` returns
- Error messages are user-friendly and actionable

## Files Created/Modified

- **Created**: `src/security.rs` - New security module
- **Modified**: `src/main.rs` - Added `pub mod security;`

## Verification

```bash
# Tests pass
cargo test --all-features security

# Clippy clean
cargo clippy --all-features -- -D warnings

# Build succeeds
cargo build --all-features
```
