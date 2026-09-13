# Architecture Decision Records & Validated Decisions

> **Documentos decisórios.** Registram *por que* algo foi decidido, não *como está hoje*.
> Consulte para entender a racionalidade; o comportamento atual está no código e nos
> docs de arquitetura em `doc/src/development/`.

> **Status:** documentação técnica viva, extraída de `IMPLEMENTATION.md` durante a
> consolidação do backlog (LUC-140 / `refactor/backlog-consolidation`). O conteúdo é
> **verbatim** da fonte; apenas os cabeçalhos foram normalizados (eram marcados como
> "PRIORITY N … COMPLETED", resíduo de tracker).

## Sprach 2.0: Validated Decisions (DEC-001 to DEC-007)

The following architectural decisions from the Sprach 2.0 article have been validated by state-of-the-art research:

| Decision | Ruling | Validation |
|----------|--------|------------|
| **DEC-001** Cache incremental for `content_relations` | On-demand, not pre-computed | GraphSeek 2026, Graph RAG 2026 |
| **DEC-002** Reflection triggers over periodic | Specific triggers, not time-based | ICML 2025, MeCo arXiv 2025 |
| **DEC-003** Curation with human approval | Drafts, not auto-publish | Rewire.it, "Human-in-the-loop" |
| **DEC-004** WASM sandbox by capabilities | Allowed/denied, not total isolation. **CRITICAL (2026-04-19):** DEC-007 extends this — `process_spawn` deny is meaningless when MCP STDIO transport itself *is* process spawning. STDIO MCP servers require explicit allowlist + sandbox. | The New Stack 2026, MCP-SandboxScan, OX Security 2026 |
| **DEC-005** Semantic versioning for plugins | Major equal, minor ≥ | OpenFang, "Semver + manifest signing" |
| **DEC-006** SOUL.md patches with human approval | Suggestions, not automatic | MetaMind NeurIPS 2025, "Human oversight" |
| **DEC-007** MCP STDIO security: no untrusted command execution | Explicit approval + allowlist + sandbox for STDIO | OX Security 2026, CVE-2025-65720 et al., Anthropic MCP SDK vulnerability |

**Competitors identified:**
- Joplin GSoC 2026: Note graphs with AI (similar to S2.1 + S2.2)
- OpenClaw: WASM sandbox for community skills (similar to S2.4)

---

---

## ADR-007: MCP STDIO Transport Security

**Date:** 2026-04-19  
**Status:** Accepted  
**Severity:** CRITICAL

#### Context

The Anthropic MCP SDK has a by-design Remote Code Execution (RCE) vulnerability in its STDIO transport. `StdioServerParameters` executes arbitrary OS commands with the parent application's privileges **before any validation or connection attempt occurs**. This means that simply configuring an MCP server connection can execute malicious commands on the host system, even if the connection fails.

**Affected CVEs:** CVE-2025-65720, CVE-2026-30623, CVE-2026-30624, CVE-2026-30618, CVE-2026-33224, CVE-2026-30625, CVE-2026-30615, CVE-2026-26015, CVE-2026-40933, CVE-2025-49596, CVE-2026-22252, CVE-2026-22688, CVE-2025-54994, CVE-2025-54136

**Scope:** 7000+ MCP servers, 150M+ downloads affected. Anthropic declined to fix ("expected behavior").

**Impact on sprachspiel:** Currently zero — sprachspiel has no MCP code. However, P6 (Phase 1) includes MCP Client Integration (P15/Plugin System), making this a future-critical concern.

#### Decision

1. **Never use `StdioServerParameters` directly.** If STDIO transport is supported, it will be through a sandboxed wrapper that validates commands against an explicit allowlist before execution.
2. **Mandatory human confirmation for MCP server installation.** Users must explicitly approve each MCP server, with clear warning about the security implications.
3. **`config.toml` command allowlist.** STDIO MCP server configurations must declare an explicit `allowed_commands` list. Any command not on the list is rejected.
4. **HTTP transport preference.** Prefer HTTP/SSE transport over STDIO wherever possible. STDIO should require explicit opt-in with security acknowledgment.
5. **Extend DEC-004 WASM sandbox to MCP processes.** STDIO MCP servers run inside the same WASM sandbox that plugins use, with `process_spawn` capability denied by default.

#### Consequences

- **Positive:** sprachspiel users are protected from the RCE vulnerability by design. The allowlist + sandbox approach means even a malicious MCP server config cannot execute arbitrary commands.
- **Negative:** STDIO MCP servers with complex startup commands may not work out-of-the-box. Users will need to review and approve each server's command list. This is intentional — security over convenience.
- **Relation to DEC-004:** `denied = ["process_spawn"]` is **meaningless** when MCP STDIO transport itself *is* process spawning. DEC-007 fixes this gap by requiring an explicit allowlist and sandbox for STDIO transport, making the DEC-004 capability model effective even with MCP.

#### References

- OX Security: "MCP Vulnerabilities Could Expose AI Apps to RCE, Data Theft and Other Attacks" (2026)
- CVE-2025-65720 et al.
- Anthropic MCP SDK `StdioServerParameters` source code
- DEC-004: WASM Sandbox by Capabilities

---

---
