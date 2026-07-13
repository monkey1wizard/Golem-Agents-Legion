# structural-retrieval Collaborative Capability

structural-retrieval is GAL's capability lane for locating structural targets faster and more accurately without turning any retrieval aid into a required dependency.

## What This Module Covers

This module is the routing surface for the two current structural-retrieval aids:

- [graphify](graphify.md) for static structural context from a generated report; it is not an MCP lane.
- [codebase-memory-mcp](codebase-memory-mcp.md) for live structural and symbol lookup through MCP; for doc-sync, native `git diff` remains the mandatory baseline.

Use this file to answer one question quickly: for the current lane, which aid should GAL consult, what should it expect back, and what is the non-tool fallback.

## Capability Definition

structural-retrieval means using optional structural aids to:

- find module, community, route, or symbol targets faster
- improve target precision before deeper reading or review
- refine affected-document or affected-code guesses with extra structural evidence

This capability is always bounded the same way:

- advisory-only
- degrade silently when a tool is not ready
- never required for normal GAL planning, review, or doc-sync execution

## Shared Preflight

All structural-retrieval routing follows the shared preflight model in [checking-contract.md](checking-contract.md).
This file intentionally does not restate the shared five-state table.

## Decision Table

| Agent goal / lane | Preferred aid | What it returns | Fallback |
| --- | --- | --- | --- |
| planning: judge scope, module boundaries, or surprising cross-area connections before drafting or refining a plan | [graphify](graphify.md) | advisory report-level structure such as communities, god nodes, and notable cross-module links | continue with native codebase reading and normal planning flow |
| architect: reason about abstractions, coupling, ownership boundaries, or structural blast radius | [graphify](graphify.md) | advisory structural context that helps challenge a plan or branch against the repo's coarse architecture | continue with direct file reads, call-path inspection, and normal architecture review |
| auditor: check whether a change appears to create unexpected cross-community or cross-boundary effects | [graphify](graphify.md) | advisory cross-community context for whole-change review; not a replacement for diff-based audit | continue with standard diff review and direct code inspection |
| doc-sync: tighten affected-doc targeting after native file detection, especially when `codeRefs` include symbols | [codebase-memory-mcp](codebase-memory-mcp.md) | advisory symbol-aware lookup and structural hints after readiness is confirmed; may refine doc-section targeting after `git diff` | keep the native `git diff` plus direct file-read baseline only |
| complement: use both broad structure and exact symbol targeting in the same task | start with [graphify](graphify.md), then refine with [codebase-memory-mcp](codebase-memory-mcp.md) when the second lane is ready | graphify gives coarse structure first; codebase-memory-mcp then narrows to exact files, symbols, or impacted sections | keep the coarse result only when available, otherwise fall back fully to native reading |

## Complement Rule

The two aids complement each other rather than compete:

- graphify is the better first pass for coarse structural orientation
- codebase-memory-mcp is the better second pass for precise symbol-aware targeting
- if either aid is absent, GAL continues without surfacing setup work as part of normal execution

## Codex Capability Gating (configured ≠ exposed)

In Codex, the codebase-memory MCP lane is **capability-gated on the tools actually exposed in the session, not on config presence**. A `[mcp_servers.*]` entry in `~/.codex/config.toml` (which `gal doctor` may report as an advisory) means the server is *configured* — it does **not** guarantee that the codebase-memory graph tools (`search_graph`, `trace_path`, `get_code_snippet`, …) are actually exposed to the model in this session.

Therefore, before routing into the codebase-memory-mcp lane in Codex:

- Resolve tool availability through the [checking-contract.md](checking-contract.md) preflight against the **exposed** tool set, never against the configured-server list.
- If the graph tools are **not exposed** in the session, treat the lane as `unavailable` and **fall back to repo-native discovery** (`rg` / direct file reads / `git diff`) — the same non-tool baseline the Decision Table already prescribes.
- Never block on the missing MCP tools, and never claim the graph tools were used when they were not exposed. A configured-but-unexposed server is a silent-degrade case, not an error and not a success.

This gate applies to any runtime where configured MCP servers can diverge from the tools exposed to the model; it is called out for Codex because that divergence is common there.
