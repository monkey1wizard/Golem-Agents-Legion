# OpenCLI Collaborative Tool Guide

This document defines when GAL should use OpenCLI, when it should use MCP browser tools, and where OpenCLI fits in the architecture.

## Positioning

OpenCLI is an **optional** external CLI collaborative tool. GAL treats it as a set of pluggable site adapters and commands, not as a single research tool. Research and structured retrieval are its most common high-frequency use cases, but they are not its only role. It is not an MCP server, not part of the `/gal` control plane, and not a required dependency for GAL workflows.

This is a lane-specific routing guide, not a repo-wide rule for every tool choice. Other GAL skills may be MCP-first, local-first, or hybrid.

## Where It Fits In The Architecture

| Layer | Function | OpenCLI fit |
| --- | --- | --- |
| Control plane | Routes control-plane questions and specialist commands | Not applicable |
| MCP layer | Makes general tools visible across runtimes | Not applicable |
| Skill layer | Teaches the agent to use optional tools and adapter commands safely | **Primary fit** |
| External CLI adapter layer | Provides source-specific commands and schemas | **Primary fit** |
| Research and retrieval | Returns structured external information with low token cost | Common high-frequency use |

## Preflight - Shared Checking Model

OpenCLI follows the shared preflight model in [checking-contract.md](checking-contract.md).

| Shared state | OpenCLI meaning | GAL behavior |
| --- | --- | --- |
| `not-applicable` | The task does not involve external retrieval or does not map to an OpenCLI-capable source. | Use the normal non-OpenCLI path. |
| `unavailable` | `opencli --version` fails or the command is not installed. | Fall back to MCP retrieval or workspace tools as appropriate. |
| `available-but-needs-init` | OpenCLI is installed, but required local setup such as adapter install, browser bridge, or session wiring is incomplete. | Do not auto-initialize during normal research. |
| `available-but-not-ready` | OpenCLI is installed, but the current task does not map cleanly to a supported adapter or the adapter cannot return the required fields. | Fall back to the documented alternative tool path. |
| `ready` | OpenCLI is installed and a supported adapter can satisfy the requested retrieval. | Route into OpenCLI. |

## Quick Decision Table

| Task shape | Default tool | Upgrade when |
| --- | --- | --- |
| Known site, known schema, stable structured output needed | OpenCLI | The adapter is missing fields or fails |
| Unknown page, unknown DOM, clicking or scrolling required | Playwright MCP or other MCP browser tools after local-first checks | The interaction becomes stable enough to justify an adapter |
| Read a normal article page once | MCP `fetch` or `imagefetch` | A site adapter yields cleaner structured output |
| Batch retrieval or repeatable shell workflow | OpenCLI | The task depends on one-off manual exploration |
| Logged-in browser-backed data with an adapter available | OpenCLI | The browser bridge or session path is unavailable |
| UI debugging, network inspection, or page-state validation | MCP browser tools | The goal narrows into deterministic data retrieval |
| Repo-local code understanding | Workspace tools | Never route to OpenCLI |

## Common Source Routing

| Source | Default tool | Common tasks |
| --- | --- | --- |
| YouTube | `opencli youtube ...` | Search, video metadata, transcript |
| NotebookLM | `opencli notebooklm ...` | Notebook metadata, source list, summary |
| Wikipedia | `opencli wikipedia ...` | Search and summary |
| Hacker News | `opencli hackernews ...` | Top stories, search, user profile |
| Google News | `opencli google news ...` | Topic headlines |
| General article pages | MCP `fetch` or `imagefetch` | One-off article reading |

## Operating Rules

- Prefer OpenCLI when an existing site adapter already returns the fields you need.
- Prefer `-f json` and an explicit `--limit` whenever possible.
- Prefer public adapters over browser-backed adapters.
- Use Playwright MCP or other MCP browser tools only when the page requires rendering, interaction, or adapter debugging that structured retrieval cannot provide.
- When browser tooling informs research output, preserve reverse-checkable evidence such as the final URL, key interaction, and screenshot/snapshot or rendered quote.
- Do not route repo-local code or git tasks into OpenCLI.
- Do not make OpenCLI a required dependency for `/gal`, `/gal research`, or any other control-plane command.
- Do not generalize OpenCLI-first behavior into a repo-wide rule.
- If OpenCLI and its fallback paths both fail, stop with an explicit no-tool message instead of pretending retrieval succeeded.

## Recommended Query Pattern

```text
question
-> opencli shortlist (with --limit and -f json)
-> opencli detail (optional: transcript, fulltext, guide)
-> MCP inspection or fallback retrieval (optional)
-> model synthesis
```

## Degrade Path

If OpenCLI is unavailable, needs initialization, or is not ready for the current task:

- fall back to MCP `fetch` or `imagefetch` for standard page retrieval
- fall back to Playwright MCP or other MCP browser tools for interaction, DOM inspection, screenshots, or adapter debugging
- fall back to workspace tools for repo-local code understanding
- stop with a clear missing-capability message if no documented path can satisfy the task

Do not auto-install adapters or silently switch the task into a fake-success state.

## What OpenCLI Is In GAL

Treat OpenCLI as:

- an optional external CLI runtime
- a set of pluggable site adapters and commands
- a capability consumed through skill-layer routing
- a frequent tool in research and retrieval workflows

Do not treat OpenCLI as:

- an MCP manifest entry
- a control-plane dependency
- a replacement for MCP browser tools

## Related Files

- [checking-contract.md](checking-contract.md) for the shared collaborative-tool state model.
- [../devguide.md](../devguide.md) for setup and runtime topology.
