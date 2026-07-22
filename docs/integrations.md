# Integrations

[繁體中文](i18n/zh-Hant/integrations.zh-Hant.md) · **English**

GAL's public reference for optional external tools. Each is an optional capability, never a core runtime dependency — GAL works normally with none of them installed. Binding behavior (the five-state preflight, structural-retrieval routing, and honest-degradation rules every golem must obey) lives in the shipped convention: [`plugins/gal-core/conventions/optional-capabilities.md`](../plugins/gal-core/conventions/optional-capabilities.md). This file is human-facing setup and reference only — it is advisory, not load-bearing.

Every section below follows one schema: purpose, real GAL consumer, readiness check, setup boundary, safe fallback, managed-vs-consumed.

## codebase-memory-mcp

- **Purpose** — optional MCP-based structural-retrieval aid for live symbol and structural lookup, refining file-to-symbol impact mapping after native `git diff`.
- **Real GAL consumer** — `doc-sync` (tightening affected-doc targeting when `codeRefs` include `#symbol`), general structural-retrieval lanes as the second pass after graphify.
- **Readiness check** — the MCP server is reachable and `index_status` proves the current repo is indexed and queryable. In Codex specifically, readiness must be checked against the tools actually **exposed** in the session, not the configured-server list — a configured-but-unexposed server is `unavailable`, not `ready`.
- **Setup boundary** — GAL does not install, bootstrap, or auto-index the repo. Installation, indexing, and query semantics are upstream to the `codebase-memory-mcp` project.
- **Safe fallback** — native `git diff` plus direct file reads remain the mandatory baseline. MCP results only refine, never replace, file-level detection.
- **Managed-vs-consumed** — **managed**. Listed in GAL's managed MCP manifest (`plugins/gal-core/mcp.json`, `DeusData/codebase-memory-mcp`).

## graphify

- **Purpose** — optional CLI-driven structural-context capability. Consumes prebuilt graph artifacts (`graphify-out/GRAPH_REPORT.md`) to give planning and review lanes advisory structural context: god nodes, communities, and surprising cross-module connections.
- **Real GAL consumer** — `/planning` and `/deep-planning` (scope and module-boundary judgment), `golem-architect` (abstractions, coupling, blast radius), `golem-auditor` (unexpected cross-community effects).
- **Readiness check** — `graphify-out/GRAPH_REPORT.md` exists for the repo. Optional `graphify-out/GAL_GRAPHIFY_VERSION.txt` is compared against the installed `graphify --version` for staleness. If versions differ and the report is not newer than the stamp, treat the report as stale-by-tool-version.
- **Setup boundary** — GAL does not install graphify, generate graphs, or rebuild stale outputs. Installation, graph generation, and query semantics are upstream: [safishamsi/graphify](https://github.com/safishamsi/graphify).
- **Safe fallback** — degrade to native codebase reading and standard planning/review behavior. Missing outputs never produce errors or setup prompts.
- **Managed-vs-consumed** — consumed only. Not wired as an MCP server — GAL reads generated report files, never queries graphify live.

## OpenCLI

- **Purpose** — optional external CLI runtime providing pluggable site adapters for structured, low-token external retrieval (YouTube, NotebookLM, Wikipedia, Hacker News, Google News, and similar sources).
- **Real GAL consumer** — research workflows (`/gal research`, `/gal deep-research`), the `opencli-research` skill.
- **Readiness check** — `opencli --version` succeeds, and the current task maps to an existing adapter that can return the required fields.
- **Setup boundary** — GAL does not auto-install OpenCLI or its adapters. Installation and adapter authoring are upstream responsibilities.
- **Safe fallback** — fall back to MCP `fetch`/`imagefetch` for standard pages, Playwright MCP or Chrome DevTools MCP for interaction/rendering, or workspace tools for repo-local code. Stop with an explicit missing-capability message rather than faking success.
- **Managed-vs-consumed** — consumed only. Not an MCP manifest entry, but a CLI runtime invoked directly.

## Playwright MCP

- **Purpose** — optional browser automation capability for live browser interaction, accessibility snapshots, screenshots, responsive inspection, and browser-visible evaluation.
- **Real GAL consumer** — `golem-tester` (QA and regression), `golem-designer` (live UI audit), research workflows needing dynamic-page rendering after local-first and structured-retrieval checks fail.
- **Readiness check** — the runtime can resolve and launch a Playwright MCP server, browser/server initialization is complete, and the current task has a browser-appropriate target.
- **Setup boundary** — GAL does not auto-initialize first-run browser/server setup during normal testing, review, design audit, or research.
- **Safe fallback** — fall back to Chrome DevTools MCP for deep diagnostics, native Playwright scripts for reusable automation, or OpenCLI/fetch/Defuddle/workspace tools for retrieval. Never claim browser validation succeeded when it did not run.
- **Managed-vs-consumed** — **user-wired**. Playwright MCP is not present in GAL's managed MCP manifest (`plugins/gal-core/mcp.json`), and is instead wired by the user's own runtime configuration when they want the capability.

local-notes (a user-owned external note-store capability) is not a tool integration and has no section here — its binding semantics live in [`optional-capabilities.md`](../plugins/gal-core/conventions/optional-capabilities.md) and its operational setup lives in [`manual.md`](manual.md).
