# Security Validation Implementation Learnings

## Date: 2025-04-17

## What was done
Added `validate_subagent_path()` and `validate_subagent_paths()` security checks to all 6 subagent file-reading paths.

## Files Modified
- `src/chat/subagent.rs` — Added validation to `run_generate()`, `run_vision()`, `run_ocr()`, `run_document()`
- `src/chat/command_handlers.rs` — Added validation to `handle_subagent_ocr()`, `handle_subagent_vision()`
- `src/lib.rs` — Added `pub mod security;`
- `src/main.rs` — Added `mod security;`

## Key Patterns

### In SubagentRunner methods (returns Result<...>)
- Use `.map_err(|e| format!("Error: {}", e))?` to convert `Result<PathBuf, String>` to the subagent error format
- For single paths: `validate_subagent_path(Path::new(&path))` → `Result<PathBuf, String>`
- For multiple paths: `validate_subagent_paths(paths)` → `Result<Vec<PathBuf>, String>`
- Use the canonical path returned by validation for actual file reads

### In Command handlers (pub async fn that prints to stdout/stderr)
- Use `if let Err(e) = validate_subagent_path(&file_path)` pattern
- Print errors with red ANSI codes: `eprintln!("\x1B[31mError: {}\x1B[0m", e)`
- Return early on validation failure

### Interface Changes (Prerequisite)
- `OcrProcessor::process_file()` changed from `(path, mode, settings)` to `(path, mode, model, model_options, ollama)`
- `VisionProcessor::process()` changed from `(args, model, settings)` to `(args, model, model_options, settings)`
- `SubagentRunner::run_ocr()` no longer takes `settings: &Settings` parameter

## Gotchas
- The `security.rs` module must be declared in BOTH `lib.rs` AND `main.rs` for both lib and bin targets
- `validate_subagent_path()` returns `Result<PathBuf, String>` — the canonical path is usable for reads
- `validate_subagent_paths()` takes `&[PathBuf]` and returns `Result<Vec<PathBuf>, String>`
- Validation must happen BEFORE file existence checks (validate_subagent_path already checks existence internally, but the CLI handlers still have their own `.exists()` checks as a fallback)