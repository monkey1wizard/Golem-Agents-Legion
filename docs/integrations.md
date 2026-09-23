---
type: Reference
title: Integrations
description: Reference guide for optional external integrations, covering operational purpose, readiness checks, configuration boundaries, and graceful degradation.
tags:
  - integrations
  - optional
  - mcp
  - fallback
status: stable
---

# Integrations

**English** · [日本語](i18n/ja/integrations.ja.md) · [繁體中文](i18n/zh-Hant/integrations.zh-Hant.md)

This document provides reference specifications for optional third-party tool integrations. Every tool listed here is optional, and none are required for core GAL operations. GAL functions completely without any of these tools installed. The governing operational rules (including the five-state preflight check, structural retrieval routing, and graceful degradation standards) are defined in [`plugins/gal-core/conventions/optional-capabilities.md`](../plugins/gal-core/conventions/optional-capabilities.md). This document provides informational setup context and operational boundaries.

Each entry below covers operational purpose, active GAL consumers, readiness verification, setup boundaries, fallback behavior, and integration ownership.

## Optional Integration Matrix

| Integration | Purpose | Fallback Behavior When Absent |
| --- | --- | --- |
| `codebase-memory-mcp` | Code symbol and structural graph lookup | Falls back to native `git diff` and targeted file reads. |
| `graphify` | Consumes pre-computed codebase knowledge graphs | Falls back to direct source file reading. |
| `OpenCLI` | Low-token structured external data retrieval | Falls back to standard web retrieval tools. |
| `Playwright MCP` | Interactive browser testing and visual inspection | Explicitly reports missing capability without simulating success. |

### codebase-memory-mcp

- **Purpose**: Optional Model Context Protocol (MCP) server providing structural symbol lookup and graph traversal. It refines symbol-level impact mapping after initial `git diff` inspections.
- **Active GAL Consumers**: Used by `doc-sync` to identify impacted documentation when `codeRefs` specify `#symbol` anchors, and serves as a second-pass structural lookup tool after `graphify`.
- **Readiness Check**: The MCP server must be reachable, and its `index_status` must confirm that the active repository is indexed and queryable. In Codex, readiness is determined by tools actively exposed in the current session rather than configured server lists. Configured but unexposed tools are treated as `unavailable`.
- **Setup Boundary**: GAL does not install, configure, or auto-index repositories for `codebase-memory-mcp`. Tool installation, indexing schedules, and query semantics remain upstream responsibilities.
- **Safe Fallback**: Native `git diff` and direct file inspection remain the foundational baseline. MCP queries refine file-level findings but never replace basic file inspection.
- **Integration Ownership**: Consumed only. Under project policy, `plugins/gal-core/mcp.json` contains an empty server manifest by default. To enable this integration, register it in `~/.gal/local/mcp.json` using the standard `"servers"` schema, or configure it directly in your host runtime's MCP settings.

### graphify

- **Purpose**: CLI-driven architectural context engine. Consumes a pre-generated graph report (`graphify-out/GRAPH_REPORT.md`) to provide structural context during planning and review, highlighting god nodes, module communities, and unexpected coupling.
- **Active GAL Consumers**: Consulted during `/planning` and `/deep-planning` for boundary analysis, by `golem-architect` for blast radius evaluations, and by `golem-auditor` for cross-boundary change analysis.
- **Readiness Check**: Requires `graphify-out/GRAPH_REPORT.md` in the repository root. If `graphify-out/GAL_GRAPHIFY_VERSION.txt` is present, it is compared against `graphify --version` to detect stale artifacts. If versions differ and the report timestamp is unchanged, the report is treated as stale.
- **Setup Boundary**: GAL does not install graphify, execute graph builds, or regenerate stale reports automatically. Graph creation and tool maintenance are external responsibilities. See [safishamsi/graphify](https://github.com/safishamsi/graphify).
- **Safe Fallback**: Falls back to direct source inspection and standard planning workflows. The system never raises errors or prompts for setup when graph files are missing.
- **Integration Ownership**: Consumed file artifact only. Not configured as an MCP server. GAL parses the generated report file and never invokes graphify interactively.

### OpenCLI

- **Purpose**: External CLI runtime offering pluggable site adapters for structured, low-token data retrieval from sources like YouTube, NotebookLM, Wikipedia, Hacker News, and Google News.
- **Active GAL Consumers**: Invoked during research workflows (`/gal research`, `/gal deep-research`) and by the `opencli-research` skill.
- **Readiness Check**: The command `opencli --version` must execute cleanly, and the active task must correspond to an available adapter that supports the requested data fields.
- **Setup Boundary**: GAL does not install OpenCLI or its site adapters. Installation and adapter configuration are managed independently by the user.
- **Safe Fallback**: For standard web pages, agents use MCP `fetch` or `imagefetch` tools. For rich rendering or interactive workflows, agents fall back to Playwright MCP or Chrome DevTools MCP. For local files, workspace tools are used. When tools are missing, agents report the missing capability explicitly and halt rather than simulating success.
- **Integration Ownership**: Consumed CLI executable only. Not registered in MCP manifests.

### Playwright MCP

- **Purpose**: Browser automation server providing live web navigation, accessibility tree snapshots, responsive layout verification, and DOM inspection.
- **Active GAL Consumers**: Used by `golem-tester` for automated UI regression tests, by `golem-designer` for visual audits, and during research when dynamic JavaScript execution is required.
- **Readiness Check**: The host runtime must resolve and launch the Playwright MCP server cleanly, complete browser initialization, and receive a browser-compatible target URL.
- **Setup Boundary**: GAL never initiates browser downloads or runtime installations during active testing, review, or research tasks.
- **Safe Fallback**: For deep network or performance diagnostics, fall back to Chrome DevTools MCP. For repeatable test scripts, execute standalone Playwright test runners. For basic text retrieval, use OpenCLI, Defuddle, or workspace tools. Agents must never report browser verification as passing unless real browser assertions executed successfully.
- **Integration Ownership**: User-wired integration. Playwright MCP is not bundled in core plugin manifests (`plugins/gal-core/mcp.json`). Users configure it directly within their host runtime environments.

External note storage (`local-notes`) represents user-owned personal infrastructure rather than a third-party tool integration. For binding behavioral rules, see [`optional-capabilities.md`](../plugins/gal-core/conventions/optional-capabilities.md). For configuration instructions, see [configuration.md](configuration.md#external-notes-integration-local-notes).
