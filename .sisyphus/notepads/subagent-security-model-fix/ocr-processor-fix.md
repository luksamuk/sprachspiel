# OCR Processor Fix - Remove Hardcoded Model/Options

## Changes Made

### src/ocr/processor.rs
- Changed `OcrProcessor::process_file()` signature:
  - Removed `settings: &Settings` parameter
  - Added `model: &str`, `model_options: ModelOptions`, `ollama: &Ollama` parameters
  - Removed hardcoded `"glm-ocr:bf16"` model name
  - Removed hardcoded `ModelOptions::default().temperature(0.0)`
  - Removed `use crate::settings::Settings` import
  - Added `use ollama_rs::Ollama` import
- Changed `process_batch()` similarly:
  - Removed `settings: &Settings`
  - Added `model: &str`, `model_options: ModelOptions`, `ollama: &Ollama`
  - Forwards params to `process_file()` with `model_options.clone()` for each file

### src/chat/subagent.rs
- `run_ocr()`: Removed `settings: &Settings` parameter (uses `&self.config.model`, `self.config.model_options.clone()`, `&self.ollama` from struct)
- `run_vision()`: Added `let model_options = self.config.model_options.clone()` and passed to `processor.process()`

### src/main.rs
- `handle_ocr()`: Resolves model key from `settings.get_subcommand_config("ocr")`, then resolves to model_id and model_options via `user_models::get_model_config()`. Creates Ollama client via `settings.ollama_client()`. Passes all to `process_batch()`.
- `handle_vision()`: Added `model_options` from `model_config.build_model_options()` and passes to `processor.process()`
- Added `mod security` (needed by command_handlers)

### src/lib.rs
- Added `pub mod security` (needed for crate::security import)

### src/chat/command_handlers.rs
- `/ocr` handler: Removed `&state.settings` from `run_ocr()` call (now uses 2 args: path, mode)

### src/vision/processor.rs
- `process()`: Added `model_options: ModelOptions` parameter
- Uses passed `model_options` instead of hardcoded `ModelOptions::default().temperature(0.1)`
- Still layers `.num_predict(args.max_tokens as i32)` on top of passed options

## Key Design Decisions
- Model and options are resolved at call sites from SubagentConfig/user_models, not hardcoded
- CLI path falls back to `ModelOptions::default().temperature(0.0)` if config key not found
- Vision processor was also fixed (same pattern as OCR)
- The `mod security` addition was required because `command_handlers.rs` was already importing `crate::security`
