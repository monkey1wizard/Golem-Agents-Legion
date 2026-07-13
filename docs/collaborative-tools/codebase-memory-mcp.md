# codebase-memory-mcp Collaborative Tool Contract

Member of [structural-retrieval](structural-retrieval.md); routing lives at that L1 capability surface.

codebase-memory-mcp is an optional MCP-based structural-retrieval aid for GAL. It stays advisory-only and never becomes a required dependency for doc-sync or the wider control plane.

## What This Module Covers

This module defines GAL's consumption contract for a reachable and indexed codebase-memory-mcp server.

When the lane is ready, GAL may use it to:

- refine file-to-symbol impact mapping after native `git diff`
- tighten doc-section hits when `codeRefs` include `#symbol`
- add structural hints during doc drift reconcile or related review work

This module does not document installation, indexing commands, MCP server bootstrapping, or full upstream query syntax.

## What Does Not Change When codebase-memory-mcp Is Present

- Native `git diff` plus direct file reads remain the mandatory detection baseline.
- Repo-local Markdown and NDJSON files remain the authoritative state.
- GAL does not install, bootstrap, or require codebase-memory-mcp.
- Missing MCP readiness must not produce setup prompts, blocking errors, or workflow detours.

## Shared Preflight

codebase-memory-mcp follows the shared preflight model in [checking-contract.md](checking-contract.md); this file intentionally does not restate the shared five-state table.

For GAL's MCP lane, `ready` means the server is reachable and `index_status` proves the current repo is indexed and queryable. If that proof is missing, degrade silently to the native baseline.

## GAL-Consumed Signals

| Input | Required | GAL use |
| --- | --- | --- |
| `index_status` | yes for readiness | Confirm repo indexing state before any advisory query |
| symbol lookup or graph query results | no | Refine impacted doc sections when `#symbol` references exist |
| architecture summary results | no | Add structural hints during reconcile or doc drift review |

## Integration Level

Level 1: live MCP-assisted symbol targeting. Trigger: the structural-retrieval lane selects codebase-memory-mcp and `index_status` confirms the repo is indexed. GAL behavior: start from native `git diff`, then use advisory symbol-aware lookup to narrow or extend affected doc-section targeting.

codebase-memory-mcp may refine the affected-section set, but it may not override the file-level baseline. When MCP results are missing, partial, or stale, GAL falls back silently to file-level behavior.

## Non-Goals

- Installing codebase-memory-mcp for the user
- Auto-indexing the repo during pipeline execution
- Replacing file-system memory with the MCP graph
- Blocking doc-sync because MCP is absent

## Upstream Handoff

Use the upstream codebase-memory-mcp project for installation, indexing, and query semantics. In GAL, consume only readiness proof plus advisory symbol and structural query output.
