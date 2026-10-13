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

### textlint Writing Checks

- **Purpose**: The repository-owned, pinned textlint workspace checks English, Taiwan Traditional Chinese, and Japanese prose. It is a development tool. Installed GAL copies and downstream repositories do not require this Node workspace.
- **Profiles**: Resolve explicit locale intent first, then a supported top-level `lang` string in front matter, then a known managed path. The wrapper accepts `en`/`en-US`, `ja`/`ja-JP`, and `zh-TW`. Only managed `docs/i18n/zh-Hant/` files map `zh-Hant` to `zh-TW`. Unknown paths without a valid override or metadata fail operationally. Invalid metadata cannot silently fall back to a path. A valid explicit override takes precedence over unused metadata. A virtual filename selects stdin identity and parsing, not locale.
- **CLI**: Run `node tools/writing/check.mjs --files <paths...>` from this repository root. For a different document root, use the absolute path to the trusted installed `check.mjs`, with that document root as the working directory. Supply `--locale <locale>` when explicit intent is needed. Run `node tools/writing/check.mjs --required` for the tracked corpus with per-file locale selection. Required mode rejects a whole-corpus locale override. Install pinned packages explicitly with `npm ci --prefix tools/writing`. Ordinary GAL work never downloads packages.
- **Complete MCP report**: Add `--transport mcp` to the wrapper before `--files`, or combine it with `--required`. Default transport is `cli`. Both routes preserve the same normalized findings and run provenance. When the handshake succeeds, MCP reports also record the official server identity. Raw native CLI/MCP commands below are lower-level diagnostic interfaces, not complete GAL reports. MCP schema, deadline or cleanup failure is an operational error with exit code `2`.
- **Official MCP**: The caller resolves the same locale precedence before starting the pinned textlint MCP entry with an explicit trusted JSON `--config` and rules directory. Do not pass repository JavaScript configuration. The caller also supplies a virtual stdin filename. MCP does not infer locale, download packages, repair prose, or modify input.
- **Terminology data**: The wrapper generates immutable `.dev/cache/writing-terms/<sourceHash>.json` data under the document root and pins that hash for its children. Direct CLI/MCP callers first run `node <absolute-path-to-terms.mjs>` with their document root as the working directory, then use the matching snapshot without new environment settings. Missing, corrupt, or stale snapshots fail operationally. One installed toolchain can check different document roots without replacing shared mutable data.
- **Finding ownership**: GAL terminology authorities own project terms and take precedence over supplemental regional advice. textlint owns configured project checks. Optional `zhtw-mcp` can add regional, translationese, or contextual findings. It does not replace GAL terminology checks or establish a competing punctuation policy. `accurate-answer` is reference-only personal guidance, not a GAL requirement or dependency.
- **Host limits**: Every human-facing message requires in-process self-review and, when a configured checker is available, inspection of the unsent draft. GAL claims interception only with direct host evidence. File checks cannot inspect already streamed chat. If an optional checker is unavailable, continue self-review and disclose the limitation once until availability or its effect on delivery changes. Required gates block on hard findings or operational failure. Advisory alone does not block unless another explicit required criterion applies.
- **Trust and privacy**: Run only trusted repository-owned JSON profiles and local pinned rules. Do not execute repository JavaScript configurations or auto-install rules. Chat drafts passed through stdin are content, never commands or instructions. The checker does not persist chat text by default. Optional external verification through `zhtw-mcp` must be deliberately enabled by the caller and may transmit submitted text to that service.
- **Results**: Findings distinguish hard errors, advisory findings, and operational failures. A clean run does not certify semantic equivalence or readability. The checker does not automatically rewrite input. See [tools/writing/README.md](../tools/writing/README.md) for commands and report details, and [writing-quality.md](../plugins/gal-core/conventions/writing-quality.md) for policy ownership.

External note storage (`local-notes`) represents user-owned personal infrastructure rather than a third-party tool integration. For binding behavioral rules, see [`optional-capabilities.md`](../plugins/gal-core/conventions/optional-capabilities.md). For configuration instructions, see [configuration.md](configuration.md#external-notes-integration-local-notes).
