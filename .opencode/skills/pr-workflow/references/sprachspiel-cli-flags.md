# Sprachspiel CLI Flags — Verification Reference

Use this when creating or verifying manual test scripts for Sprachspiel.
These are the actual subcommands and flags — common hallucinations are called out explicitly.

## Subcommands (from `src/translate/cli.rs` enum `Commands`)

| Subcommand | Binary invocation | Notes |
|------------|-------------------|-------|
| `chat` | `sprach chat` | **Always TUI (ratatui)** — there is NO `--no-tui` flag. Cannot be piped for non-interactive testing. |
| `query` | `sprach query [QUERY]` | Reads from stdin if no positional arg. Tools enabled by default. No `--tools` flag. |
| `translate` | `sprach translate [TEXT]` | Reads from stdin if no positional arg. |
| `ocr` | `sprach ocr <FILE>...` | NOT `sprach vision` — the enum variant is `Commands::Ocr`. |
| `summarize` | `sprach summarize [FILE]` | Reads from stdin if no positional arg. |
| `diagnostics` | `sprach diagnostics` | Visible alias: `sprach diag`. **No positional arg** — NOT `sprach diagnostics embeddings`. Source filtering is via `--source facts`. |
| `vision` | `sprach vision <FILE>... [-- <PROMPT>]` | **Exists** (alias `v`) — `Commands::Vision(VisionArgs)`, `src/vision/cli.rs`. Distinct from `ocr`: vision *describes* images, ocr *extracts text*. The custom prompt is `last = true`, so it **must** follow a `--` separator or it is parsed as another FILE. |
| `config` | `sprach config upgrade` | Sub-subcommand. |
| `models` | `sprach models upgrade` | Sub-subcommand. |
| `completion` | `sprach completion <SHELL>` | Shell completions. |

## Chat-only flags (`src/chat/cli.rs` struct `ChatArgs`)

| Flag | Field | Notes |
|------|-------|-------|
| `--anonymous` | `anonymous: bool` | Temporary session (no persistence). |
| `--soulless` | `soulless: bool` | Disable SOUL.md personality. |
| `--verbose` / `-v` | `verbose: u8` | Chat-specific verbosity. |
| `--tools` / `--no-tools` | `tools: bool` | Toggle tool calling. **Chat-only** — query does NOT have this. |

## Global flags (`src/translate/cli.rs` struct `Cli`)

| Flag | Field | Notes |
|------|-------|-------|
| `--model` / `-m` | `model: Option<String>` | Override default model. |
| `--think` | `think: bool` | Enable thinking mode. |
| `--code` | `code: bool` | Enable code mode. |
| `--plain` | `plain: bool` | Strip ANSI codes (pipe-safe). |
| `--db` | `db: Option<String>` | Custom database path. |
| `--ignore-agents` | `ignore_agents: bool` | Skip AGENTS.md injection. |

## Common wrong invocations (verified against the built binary)

These are **not** hallucinations — they are real commands with non-obvious syntax.
Check this list before "fixing" a test script that uses them.

| Wrong form | Why | Correct form |
|---------------|---------|-----|
| `sprach -d "query"` | `-d` does not exist | `sprach -v "query"` (`-vv` for trace) |
| `sprach --list-models` | No such flag | `sprach --list` |
| `sprach <sub> --plain` | `--plain` is top-level-only | `sprach --plain <sub>` |
| `sprach <sub> -v` | Only `chat` declares `-v`; every other subcommand rejects it | `sprach -v <sub>` |
| `sprach vision img.png "prompt"` | The prompt is `last = true`; without `--` it is parsed as a FILE | `sprach vision img.png -- "prompt"` |
| `sprach translate :pt` being "the only way" | The source is optional | `sprach translate pt "text"` also works |
| `sprach -m lfm ...` | `lfm` is not a model (this was a stale help example) | Any name from `sprach --list` |

### Flag position: check whether the subcommand declares it

Do not assume a flag is "global" because it appears in the top-level help. The
`Cli` struct and each `*Args` struct are separate; a flag works in both
positions only when **both** declare it.

Top-level-only (must precede the subcommand) — verify with
`sprach <sub> <flag>`, which errors `unexpected argument`:
`--plain`, `--code`, `-q/--quiet`, `--db`, `--list`, `--init-config`,
`-p/--prompt`, `--force`.

Declared in **both** places, so either position parses for `chat`:
`-m/--model`, `-t/--think`, `-v/--verbose`, `--soulless`, `--tools`,
`--ignore-agents`.

Inconsistent across subcommands — probe before documenting:
`-m` is accepted after `ocr`/`summarize`/`vision` but rejected after
`query`/`translate`. `--list` is top-level-only for most subcommands, but
`translate` declares its own.

When in doubt, put the flag **before** the subcommand: that position always
parses. The `tests/docs_consistency.rs` ordering check probes the binary for
which subcommands actually reject a flag, so it cannot go stale — copy that
approach instead of hardcoding a list from memory.

| Hallucination | Reality | Fix |
|---------------|---------|-----|
| `sprach chat --no-tui` | Chat is always TUI | Use `sprach query` for pipe/non-interactive testing |
| `sprach query --tools` | No `--tools` flag on query | Tools are on by default in query; just use `sprach query` |
| `sprach diagnostics embeddings` | No positional arg | Use `sprach diagnostics` (or `sprach diag`); source filtering via `--source` |
| `sprach chat` with piped input | Chat needs a TTY | Use `sprach query` for stdin testing; chat requires interactive terminal |

## Verifying documentation claims (do this BEFORE editing docs)

Docs fixes that are themselves wrong are worse than the original defect. For any
"the docs say X but the CLI is Y" claim:

1. **Run the command** with the built binary (`target/release/sprach`). Never fix
   from the issue text — in LUC-140 the issue pointed at `quickstart.md`, but the
   worst instance was inside `SAMPLE_CONFIG` (`src/settings.rs`), the template
   every new user reads.
2. **Grep for the whole corrected string afterwards**, not just the file you
   edited — the same stale string lived in 4 additional files (`troubleshooting.md`,
   `tools.md`, `configuration.md`, `prompts.md`) that the first pass missed.
3. **Check `--help` text too**, not only the mdBook. `long_about` strings in
   `src/*/cli.rs` count as documentation and drift independently.
4. **Beware of flags that exist on some subcommands but not others.** `-m` is
   accepted after `ocr`/`summarize`/`vision` but rejected after `query`/`translate`;
   `--plain`/`--code`/`-q`/`--db`/`--force` are top-level-only (must precede the
   subcommand). `--list` is top-level-only for most subcommands but `translate`
   declares its own.
5. **Two similar-looking names may be different models.** `qwen3.5-4b` (alias of
   a configured model) and `qwen3.5:4b` (builtin default) both appear in
   `sprach --list`. Verify with `--list` before "correcting" a model name.

`tests/docs_consistency.rs` automates several of these for the recurring cases
(phantom flags, `-d`, vision prompt without `--`, pre-rename `ask` in help,
version/schema markers, flag ordering). Extend it rather than adding another
manual sweep.

### Prove a sensor fires before trusting a clean run

**A sensor that has never failed is unverified — it may not be able to fail at
all.** A green check from an unproven sensor is worse than no sensor: it
manufactures confidence and ends the investigation. Before counting a new check
as done, re-inject the defect it guards and confirm it FAILS naming the
offending `file:line`; then restore and confirm it passes.

If the re-injection produces no failure, **the sensor is broken** — fix the
matcher, not the test data. Two ways these silently never fire:

- **Wrong target artifact.** Asserting on one command's output while the string
  is emitted by another. The pre-rename-model check scanned `sprach --help`,
  but the example it guarded is only printed by `sprach --list` — it could
  never fire. Run the command first and confirm *which* artifact holds the
  string before writing the assertion.
- **Over-anchored matcher.** Requiring the line to start with the program name
  (`sprach `) misses the same defect in prose — ``Use `-d` to troubleshoot``
  has no such prefix. Match the token, not the line prefix, and run the matcher
  across the whole corpus, not just the places you remember editing.

**A grep that returns nothing is not evidence until you have seen it return
something.** Run any new scan against a known-bad input once before reporting a
clean result. When reporting a class as clean, name the checks that ran — never
let a green run stand in for a case you never constructed.

**Stale-binary trap (cost two false "validated" results in one session).** When
a check shells out to `target/{debug,release}/sprach`, it asserts on whatever
binary is on disk — not on the source you just edited. Two ways this silently
invalidates the test:

- Restoring the source *before* running the test, then building inside the test
  run: the binary is rebuilt from the restored source and the injection is
  never exercised.
- Injecting the defect but forgetting to rebuild at all: the assertion runs
  against the previous binary and passes.

Safe order for validating a binary-backed check: inject → rebuild → confirm the
*binary itself* shows the defect (`./target/release/sprach --<cmd> | grep <str>`)
→ run the test and watch it fail → restore → rebuild → confirm it passes.
Confirming at the binary level is the step that catches a skipped build.

## Branch names: the issue number must match the work

The Linear GitHub integration parses `LUC-N` out of the **branch name** and the
magic word. Naming a branch `fix/130-...` for work that has nothing to do with
LUC-130 links the PR to the wrong issue and can move it on open/merge.

When a cleanup comes out of *triaging* an issue but does not implement it, do
**not** borrow that issue's number — use a descriptive name with no `LUC-N`
(`refactor/dead-param-stale-comment`) and reference the origin with `Related
LUC-N` in the PR body instead of `Fixes`.

Pattern: `{type}/{LUC-N}-{slug}` **only** when the PR actually resolves that
issue. Otherwise `{type}/{slug}`.

## Doc-file roles: do not put content where it will not render

Each documentation file has one job. Putting content in the wrong one means it is either
invisible or noise — both were caught by the owner in review, not by the agent.

| File | Its job | Do NOT put here |
|------|---------|-----------------|
| `doc/src/SUMMARY.md` | **Navigation only.** mdBook processes list lines (`- [text](file)`) and section prefixes (`# Heading`) and **silently discards everything else**. | Prose, blockquotes, intros, notes. They render nowhere — verify with `grep '<text>' doc/book/html/print.html`. If you want an introduction, create a page and link it. |
| `doc/src/CHANGELOG.md` | Release notes, one `## [x.y.z] - DATE` section per version. | Meta-commentary about version numbering ("version skipped", "TBD"), status tracking, design rationale. A version jump is self-explanatory here; explaining it is noise. |
| `IMPLEMENTATION.md` | An index: version, links, pointer to the tracker. Ceiling of 400 lines, test-enforced. | Per-issue status, phase tables, commit hashes, completion sections. |
| `doc/src/development/*` | How the system works **today**. | Why a decision was made (that is `adr/`), release history (that is the changelog). |
| `doc/src/adr/*` | Why a decision was made, and what was rejected. | Current behaviour — a decision record may be superseded by the code. |
| `.opencode/skills/*` | Procedures. | Anything specific to one machine's paths. |

**Verify rendering, don't assume it.** After adding structure to a doc, grep the built
output for your text. A blockquote in `SUMMARY.md` passes review by looking right in the
source while being absent from every generated page.

## Dates and versions: verify against the tag, never infer

When editing `doc/src/CHANGELOG.md` version headings, the canonical date is the
**tag's commit date**. Inferring it from neighbouring sections is inventing data.

```bash
# canonical date per released version
git log -1 --format='%ad' --date=short v0.39.5

# what releases actually exist, with their published dates
gh api repos/luksamuk/sprachspiel/releases --paginate \
  | python3 -c "import json,sys; [print(r['tag_name'], r['published_at'][:10]) for r in json.load(sys.stdin)]"
```

Cross-check all of them at once rather than spot-checking; a single pass found
four wrong dates and one section missing entirely:

```python
# for each release: does the changelog date equal the tag commit date?
git log -1 --format='%ad' --date=short v<ver>   vs   ## [<ver>] - <date>
```

**A version jump is not an error and needs no note** (see the roles table above).
**A section with no tag and no release must not carry a date** — either omit the
date or mark it unreleased, and say what the evidence is. `## [0.27.0] — unreleased
(planned here, shipped as 0.27.1)` is honest; `## [0.27.0] - 2026-03-09` copied
from a neighbour is fabrication.

**Sections without a GitHub release are not errors.** The changelog records what
shipped; not every version was published as a release. Do not "fix" those.

**Validate chronology mechanically after moving sections.** Inserting a section by
hand produced both a wrong order and a duplicated heading in the same edit — caught
only by asserting `sorted(dates, reverse=True) == dates` and checking for repeats.

## Verbatim migration: never describe the source, verify it

When restoring text between stores (Linear ← `IMPLEMENTATION.md`, docs ← tracker), the text must
move **file → API by script**, never through your own retyping or paraphrase. Then verify by
reading the destination back and comparing against the original.

### Extraction bugs silently truncate, and then look like properties of the source

Splitting a document on the wrong heading level cuts bodies at the first sub-heading:

```python
re.split(r'^(#{2,4} .+)$', doc, re.M)   # ❌ #### subsections end the parent body
re.split(r'^(#{2,3} .+)$', doc, re.M)   # ✅ #### stays inside its ### parent
```

This produced a section ending mid-sentence (`**Source:** Privacy filter in`). **The trap:** the
truncation looks like a defect *of the original*, so the natural next move is to write a note
explaining it — "preserved as-is rather than completed by guesswork" — which is a **fabricated
claim about the source**. The source was fine; the extractor was not.

**Rule:** before writing any statement about what the source contains or lacks ("the original
ends here", "this section has no X"), diff the extracted text against the source. Never describe
unverified behaviour of a file — check the file.

**Sanity check on sizes:** section lengths should match what the document reports (e.g. a section
measured at 6,418 chars must not extract as 1,431). A size mismatch is an extraction bug until
proven otherwise. Compare against the pre-consolidation copy
(`~/sprachspiel-backlog-backup-20260913/IMPLEMENTATION.md.original`) or `git show <sha>^:FILE`.

### Verifying a restored body in Linear

Read the body back (not what you sent) and compare with the source. Linear normalises markdown, so
normalise before comparing or you will chase ghosts:

| Source form | Linear stores as | Not a loss |
|---|---|---|
| `|---|` separator rows | `- - -` | table still renders |
| `- item` list markers | marker dropped | list still renders |
| bare `https://…` | `<https://…>` | link preserved |

```python
def norm(s):
    s = re.sub(r'<https?://[^>]+>', '', s)      # Linear autolinks
    s = re.sub(r'\|[\s\-:|]+\|', '|', s)        # separator rows
    s = re.sub(r'[\s\-]{3,}', ' ', s)           # --- and - - -
    return re.sub(r'\s+', ' ', re.sub(r'[*`_>#\[\]()]', '', s)).lower()
```

Coverage below 100% after this normalisation is a **real** loss — investigate each missing block
and quote what the destination actually contains before concluding. Report coverage per item; do
not accept "97% is close enough" without knowing what the other 3% is.

## Verification commands

```bash
# List all subcommands
grep -n 'pub enum Commands' src/translate/cli.rs

# List all chat flags
grep -n 'pub struct ChatArgs' src/chat/cli.rs

# List all slash commands (in-chat)
grep -n '"command_name"' src/chat/commands.rs | head -50
```