# codebase-memory-mcp Collaborative Tool Contract

codebase-memory-mcp is an optional MCP-based structural lookup lane for GAL. It is advisory-only and never becomes a required dependency for doc-sync or the wider control plane.

## What This Module Covers

This module defines GAL's consumption contract for a reachable and indexed codebase-memory-mcp server.

When the lane is ready, GAL may use it to:

- refine file-to-symbol impact mapping after native `git diff`
- tighten doc-section hits for `codeRefs` that include `#symbol`
- gather high-level structural hints during doc drift reconcile

This module does not document installation, indexing commands, MCP server bootstrapping, or upstream query syntax beyond GAL's readiness needs.

## What Does Not Change When codebase-memory-mcp Is Present

- `git diff` plus direct file reads remain the mandatory detection baseline.
- Repo-local Markdown and NDJSON files remain the authoritative state.
- GAL does not install, bootstrap, or require codebase-memory-mcp.
- Missing MCP readiness must not produce setup prompts, blocking errors, or workflow detours.

## Preflight - Shared Checking Model

This tool follows the shared preflight model in [checking-contract.md](checking-contract.md).

| Shared state | codebase-memory-mcp meaning | GAL behavior |
| --- | --- | --- |
| `not-applicable` | The current lane does not need symbol-aware graph lookup. | Continue without MCP usage. |
| `unavailable` | No reachable codebase-memory-mcp server is available to the current runtime. | Continue with file-level detection only. |
| `available-but-needs-init` | The MCP server is reachable, but this repo is not indexed yet. | Do not trigger indexing automatically. Continue without MCP usage. |
| `available-but-not-ready` | The server responds, but `index_status` cannot prove this repo is indexed and queryable. | Degrade silently to file-level detection. |
| `ready` | The MCP server is reachable and `index_status` confirms this repo is indexed. | Use MCP results as advisory signals after `git diff`. |

Readiness is defined in MCP terms, not by local cache presence alone.

## GAL-Consumed Signals

| Input | Required | GAL use |
| --- | --- | --- |
| `index_status` | yes for readiness | Confirms repo indexing state before any advisory query |
| symbol lookup / graph query results | no | Refine impacted doc sections when `#symbol` references exist |
| architecture summary results | no | Add structural hints during reconcile or doc drift review |

## Integration Rules

- doc-sync starts from `git diff` file paths and only then consults MCP results.
- MCP output may narrow or extend the affected doc-section set, but it may not override the file-level baseline.
- When MCP results are missing, partial, or stale, GAL falls back silently to file-level behavior.

## Non-Goals

- Installing codebase-memory-mcp for the user
- Auto-indexing the repo during pipeline execution
- Replacing file-system memory with the MCP graph
- Blocking doc-sync because MCP is absent

## Upstream Handoff

Use the upstream codebase-memory-mcp project for installation, indexing, and query semantics. In GAL, consume only readiness plus advisory query output.
