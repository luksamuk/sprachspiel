# Manual Test Scenarios: Subagent Security + Model Resolution

This document covers manual QA scenarios for the 4 interconnected subagent fixes:
1. **Security Validation** — Blocklist + CWD sandbox for all subagent file reads
2. **Vision Multi-Image** — Comma-separated path parsing for vision subagent
3. **Model Resolution** — All subagents use config key → ModelConfig flow consistently
4. **OCR Prompt Restriction** — glm-ocr gets standard prompts; other models keep custom prompts

---

## Security Validation Tests

### Test 1: Blocklist blocks sensitive files within CWD

**Purpose:** Verify that blocked patterns (.env, secrets, SSH keys) are rejected even when inside the current working directory.

**Steps:**
1. Create a test file in the current directory:
   ```bash
   echo "DUMMY_SECRET=test123" > .env
   ```
2. Run the OCR subagent via tool call:
   ```
   spawn_subagent("ocr", "Extract text from this image", Some(".env"))
   ```
3. Alternatively, test via CLI:
   ```bash
   ask-ai ocr .env
   ```

**Expected Result:**
- Error message containing: "BLOCKED" and "protected file pattern"
- File is NOT read
- No sensitive content exposed in error message

**Cleanup:**
```bash
rm .env
```

---

### Test 2: CWD sandbox blocks paths outside allowed directory

**Purpose:** Verify that paths outside CWD and /tmp are rejected.

**Steps:**
1. Run vision subagent with an absolute path outside CWD:
   ```
   spawn_subagent("vision", "Describe this file", Some("/etc/passwd"))
   ```
2. Test with SSH key path:
   ```
   spawn_subagent("ocr", "Extract text", Some("~/.ssh/id_rsa"))
   ```
3. Test via CLI:
   ```bash
   ask-ai vision /etc/passwd "What is this?"
   ```

**Expected Result:**
- Error message containing: "outside the allowed directory"
- Mentions "current working directory" and "temporary directories"
- File is NOT accessed

**Cleanup:** None required (no files created)

---

### Test 3: Valid CWD path accepted

**Purpose:** Verify that legitimate files within CWD are processed successfully.

**Steps:**
1. Place a test image in the current directory:
   ```bash
   # Use any small test image, or create a simple one
   cp /path/to/test_image.png .
   ```
2. Run OCR subagent:
   ```
   spawn_subagent("ocr", "Extract all text from this image", Some("test_image.png"))
   ```
3. Run via CLI:
   ```bash
   ask-ai ocr test_image.png
   ```

**Expected Result:**
- OCR processes successfully
- Returns extracted text (or description if no text in image)
- No security errors

**Cleanup:**
```bash
rm test_image.png
```

---

### Test 4: /tmp directory allowed for tool interop

**Purpose:** Verify that /tmp and /var/tmp are allowed (needed for tool interoperability like pdftotext).

**Steps:**
1. Create a test file in /tmp:
   ```bash
   echo "test content" > /tmp/test_ocr_input.txt
   ```
2. Run subagent with /tmp path:
   ```
   spawn_subagent("document", "Read this file", Some("/tmp/test_ocr_input.txt"))
   ```

**Expected Result:**
- File is processed successfully
- No "outside allowed directory" error

**Cleanup:**
```bash
rm /tmp/test_ocr_input.txt
```

---

### Test 5: Tilde expansion works correctly

**Purpose:** Verify that `~` in paths is expanded to home directory before validation.

**Steps:**
1. Place a test image in home directory:
   ```bash
   cp /path/to/test_image.png ~/test_image.png
   ```
2. Run subagent with tilde path:
   ```
   spawn_subagent("vision", "Describe this image", Some("~/test_image.png"))
   ```

**Expected Result:**
- Path is expanded correctly
- Image is processed successfully (if within CWD tree)
- OR rejected with "outside allowed directory" if home is not under CWD (expected behavior)

**Cleanup:**
```bash
rm ~/test_image.png
```

---

## Vision Multi-Image Tests

### Test 6: Multi-image vision via comma-separated paths

**Purpose:** Verify that vision subagent can process multiple images passed as comma-separated paths.

**Steps:**
1. Place two test images in current directory:
   ```bash
   cp /path/to/img1.png .
   cp /path/to/img2.png .
   ```
2. Run vision subagent with comma-separated paths:
   ```
   spawn_subagent("vision", "Describe both images and compare them", Some("img1.png,img2.png"))
   ```

**Expected Result:**
- Both images are processed
- Response includes analysis of both images
- No parsing errors

**Cleanup:**
```bash
rm img1.png img2.png
```

---

### Test 7: Single-image vision backward compatible

**Purpose:** Verify that single-image vision calls still work (no regression).

**Steps:**
1. Place a single test image:
   ```bash
   cp /path/to/single.png .
   ```
2. Run vision subagent with single path:
   ```
   spawn_subagent("vision", "Describe this image", Some("single.png"))
   ```

**Expected Result:**
- Single image is processed successfully
- Behavior identical to pre-fix version
- No "multiple paths" errors

**Cleanup:**
```bash
rm single.png
```

---

### Test 8: OCR rejects comma-separated paths (single file only)

**Purpose:** Verify that OCR subagent only accepts single image paths (not comma-separated).

**Steps:**
1. Place two test images:
   ```bash
   cp /path/to/img1.png .
   cp /path/to/img2.png .
   ```
2. Run OCR subagent with comma-separated paths:
   ```
   spawn_subagent("ocr", "Extract text", Some("img1.png,img2.png"))
   ```

**Expected Result:**
- Either: Error explaining OCR only accepts single files
- Or: Only the first image is processed (implementation-dependent)
- Should NOT crash or behave unpredictably

**Cleanup:**
```bash
rm img1.png img2.png
```

---

## Model Resolution Tests

### Test 9: OCR uses resolved model from config

**Purpose:** Verify that OCR subagent uses the model configured in `[model.ocr]` section, not hardcoded "glm-ocr:bf16".

**Steps:**
1. Check current config in `~/.config/ask-ai/config.toml`:
   ```toml
   [model.ocr]
   model = "glm-ocr:bf16"  # or custom model
   ```
2. Optionally, temporarily change to a different model:
   ```toml
   [model.ocr]
   model = "moondream:1.8b"
   ```
3. Run OCR subagent:
   ```
   spawn_subagent("ocr", "Extract text", Some("test_image.png"))
   ```
4. Check logs or debug output for model name used

**Expected Result:**
- Uses configured model from `[model.ocr]` section
- If changed to moondream, uses moondream (not glm-ocr)
- No hardcoded model fallback

**Cleanup:** Restore original config if changed

---

### Test 10: Vision uses self.config (no re-read from settings)

**Purpose:** Verify that vision subagent uses the model from SubagentConfig, not re-reading from settings.

**Steps:**
1. Configure vision model in config.toml:
   ```toml
   [model.vision]
   model = "kimi-k2.5:cloud"
   ```
2. Run vision subagent:
   ```
   spawn_subagent("vision", "Describe this image", Some("test_image.png"))
   ```
3. Verify in code (development test): Check `src/chat/subagent.rs:run_vision()` uses `self.config.model`, not `self.settings.get_subcommand_config("vision")`

**Expected Result:**
- Uses configured vision model
- No redundant settings re-read
- Consistent with other subagents

**Cleanup:** None

---

### Test 11: Translate uses translategemma fallback

**Purpose:** Verify that translate subagent uses the correct fallback model.

**Steps:**
1. Ensure no custom translate model configured
2. Run translate subagent:
   ```
   spawn_subagent("translate", "Translate to Portuguese: Hello world", None)
   ```
3. Check which model is used

**Expected Result:**
- Falls back to "translategemma:4b"
- Matches the fallback in `get_subcommand_config()`

**Cleanup:** None

---

### Test 12: Summarize and Document use global default

**Purpose:** Verify that summarize and document subagents fall back to global default model when not configured.

**Steps:**
1. Ensure no custom model configured for summarize/document
2. Run summarize subagent:
   ```
   spawn_subagent("summarize", "Summarize: [long text]", None)
   ```
3. Run document subagent:
   ```
   spawn_subagent("document", "Extract key points", Some("document.txt"))
   ```

**Expected Result:**
- Both use `[model] default` from config
- Consistent fallback behavior

**Cleanup:** None

---

## OCR Prompt Restriction Tests

### Test 13: glm-ocr gets standard prompt prefix

**Purpose:** Verify that when using glm-ocr model, custom prompts are overridden with standard OcrMode prefixes.

**Steps:**
1. Ensure OCR model is set to glm-ocr:
   ```toml
   [model.ocr]
   model = "glm-ocr:bf16"
   ```
2. Run OCR with a custom prompt:
   ```
   spawn_subagent("ocr", "extract all text carefully", Some("test_image.png"))
   ```
3. Check what prompt is actually sent to the model (via debug logs)

**Expected Result:**
- Custom prompt is REPLACED with "Text Recognition:" prefix (from `OcrMode::Text.into_prompt()`)
- Model receives standard prompt, not user's custom text
- Ensures glm-ocr compatibility

**Cleanup:** None

---

### Test 14: Non-glm-ocr model keeps custom prompt

**Purpose:** Verify that non-glm-ocr models preserve user's custom prompts.

**Steps:**
1. Configure a custom OCR model:
   ```toml
   [model.ocr]
   model = "moondream:1.8b"
   ```
2. Run OCR with custom prompt:
   ```
   spawn_subagent("ocr", "describe this image in detail", Some("test_image.png"))
   ```
3. Check what prompt is sent to the model

**Expected Result:**
- Custom prompt is PRESERVED as-is
- Model receives "describe this image in detail"
- No automatic prefix added

**Cleanup:** None

---

### Test 15: CLI /ocr command with explicit mode still works

**Purpose:** Verify that CLI OCR command with explicit mode flag is not affected by prompt restriction.

**Steps:**
1. Run CLI OCR with explicit mode:
   ```bash
   ask-ai ocr --detailed test_image.png
   ```
2. Run CLI OCR with default mode:
   ```bash
   ask-ai ocr test_image.png
   ```

**Expected Result:**
- `--detailed` flag uses `OcrMode::Detailed` prompt
- Default uses `OcrMode::Text` prompt
- CLI paths work independently of subagent prompt restriction

**Cleanup:** None

---

## Integration Tests

### Test 16: All subcommand CLI commands still work

**Purpose:** Verify that CLI commands (`/ocr`, `/vision`, `/translate`, `/summarize`) continue working after the fixes.

**Steps:**
1. Test OCR:
   ```bash
   ask-ai ocr test_image.png
   ```
2. Test Vision:
   ```bash
   ask-ai vision test_image.png "What's in this image?"
   ```
3. Test Translate:
   ```bash
   ask-ai translate :pt "Hello world"
   ```
4. Test Summarize:
   ```bash
   echo "Long text here..." | ask-ai summarize
   ```

**Expected Result:**
- All commands execute successfully
- No regressions from subagent fixes
- Consistent behavior with tool-based calls

**Cleanup:** Remove test files

---

### Test 17: Security validation on CLI paths

**Purpose:** Verify that CLI command handlers also validate paths (not just tool paths).

**Steps:**
1. Test CLI OCR with blocked file:
   ```bash
   echo "secret" > .env
   ask-ai ocr .env
   ```
2. Test CLI vision with outside path:
   ```bash
   ask-ai vision /etc/passwd "Describe"
   ```

**Expected Result:**
- Same security errors as tool-based calls
- CLI paths also go through `validate_subagent_path()`
- Consistent security across all entry points

**Cleanup:**
```bash
rm .env
```

---

### Test 18: Error message quality

**Purpose:** Verify that error messages are helpful and actionable (per AGENTS.md philosophy).

**Steps:**
1. Trigger each error type:
   - File not found
   - Path outside CWD
   - Blocked file pattern
   - Invalid subagent type
2. Review error message content

**Expected Result:**
- Messages explain WHAT went wrong
- Messages suggest HOW to fix (when applicable)
- No cryptic errors or stack traces
- Messages in English (per AGENTS.md)

**Example good error:**
```
Error: BLOCKED - '.env' matches a protected file pattern. This file may contain 
sensitive information (credentials, secrets, keys). Reading such files is 
restricted for security.
```

**Cleanup:** None

---

## Test Evidence Collection

For each test executed, save evidence to:

```
.sisyphus/evidence/manual-test-{test-number}-{slug}.txt
```

Example:
```
.sisyphus/evidence/manual-test-1-blocklist.txt
.sisyphus/evidence/manual-test-6-multi-image.txt
.sisyphus/evidence/manual-test-13-glm-ocr-prompt.txt
```

Evidence should include:
- Command/tool call executed
- Output received
- Pass/fail status
- Timestamp

---

## Test Coverage Summary

| Category | Test # | Description | Status |
|----------|--------|-------------|--------|
| Security | 1 | Blocklist blocks .env within CWD | ☐ |
| Security | 2 | CWD sandbox blocks /etc/passwd | ☐ |
| Security | 3 | Valid CWD path accepted | ☐ |
| Security | 4 | /tmp directory allowed | ☐ |
| Security | 5 | Tilde expansion works | ☐ |
| Multi-Image | 6 | Comma-separated vision paths | ☐ |
| Multi-Image | 7 | Single-image backward compat | ☐ |
| Multi-Image | 8 | OCR rejects multi-path | ☐ |
| Model Resolution | 9 | OCR uses config model | ☐ |
| Model Resolution | 10 | Vision uses self.config | ☐ |
| Model Resolution | 11 | Translate uses translategemma | ☐ |
| Model Resolution | 12 | Summarize/Document use default | ☐ |
| OCR Prompt | 13 | glm-ocr gets standard prefix | ☐ |
| OCR Prompt | 14 | Non-glm-ocr keeps custom prompt | ☐ |
| OCR Prompt | 15 | CLI /ocr with mode flag works | ☐ |
| Integration | 16 | All CLI commands work | ☐ |
| Integration | 17 | CLI paths validate security | ☐ |
| Integration | 18 | Error messages are helpful | ☐ |

**Total:** 18 test scenarios

---

## Notes

- Tests cover both tool path (`spawn_subagent`) and CLI path (`/ocr`, `/vision`)
- Each test has clear Steps, Expected Result, and Cleanup when applicable
- Security tests verify both blocklist and CWD sandbox layers
- Model resolution tests verify config consistency across all subagent types
- OCR prompt tests verify glm-ocr-specific behavior vs. other models
