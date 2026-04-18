# Draft: Subagent File Reading Security

## Requirements (confirmed)
- Vision subagent can accept multiple images in file_path (only vision, not others)
- All subagent file reading MUST go through security checks:
  1. `is_blocked_path()` — block sensitive files (env, secrets, SSH keys, etc.)
  2. CWD/sandbox check — file must be in current directory or subdirectories
  3. Landlock — second layer of defense against path traversal

## Problems Identified
1. Vision multi-image: Need to verify if vision subagent properly handles comma-separated paths
2. Security bypass: Subagents may read files without going through `is_blocked_path()` and CWD checks
3. Need to verify Landlock is applied during subagent execution

## Technical Decisions
- (pending research results)

## Research Findings
- (pending explore agent results)

## Scope Boundaries
- INCLUDE: All subagent types (OCR, Vision, Translate, Summarize, Document)
- INCLUDE: Both tool path (`spawn_subagent`) and command path (`/ocr`, `/vision`, etc.)
- INCLUDE: Manual test scenarios update
- EXCLUDE: Changes to existing file tool security logic (only applying it to subagents)