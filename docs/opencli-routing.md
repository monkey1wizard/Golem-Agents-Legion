# OpenCLI Routing

This document defines when GAL should use OpenCLI, when it should use MCP tools instead, and where OpenCLI fits in the architecture.

OpenCLI is an optional research-side plugin for structured retrieval.
It is not an MCP server, not part of the `/gal` control plane, and not a required dependency for GAL-managed work.

This is a lane-specific routing guide, not a repo-wide universal tool rule. Other GAL skills may be MCP-first, local-first, or mixed by design.

## Positioning

| Layer | What it does | OpenCLI fit |
| --- | --- | --- |
| Control plane | Routes workflow state and specialist commands | Not here |
| MCP layer | Makes general-purpose tools visible across runtimes | Not here |
| Skill layer | Teaches agents how to use optional tools safely | Primary fit |
| Research/data retrieval | Fetches structured external information with low token overhead | Primary fit |

## Fast Decision Table

| Task shape | Default tool | Use when | Escalate when |
| --- | --- | --- | --- |
| Known site, known schema, stable structured output needed | OpenCLI | Existing adapter already matches the task | The adapter is missing fields or failing |
| Unknown page, unknown DOM, or unknown network behavior | MCP browser tools | Need to inspect, click, scroll, debug, or reverse engineer | The interaction becomes stable enough to justify an adapter |
| Read a normal article page once | MCP `fetch` or `imagefetch` | Need quick page retrieval or image-aware extraction | A site-specific OpenCLI adapter exists and gives cleaner structured output |
| Batch retrieval or repeatable shell workflows | OpenCLI | Need `--limit`, `-f json`, repeatability, or pipelines | The task depends on one-off manual exploration |
| Logged-in browser data that already has an adapter | OpenCLI | Need to reuse Chrome session and get a stable schema | Browser bridge/session is unavailable or the page flow changed |
| UI debugging, network inspection, or page-state verification | MCP browser tools | Need screenshots, DOM/network visibility, or live interaction | The goal reduces to deterministic data retrieval |
| Repo-local code understanding | Workspace tools | Need to read files, symbols, or git changes | Never route to OpenCLI by default |

## Common Source Routing

| Source | Common tasks | Default tool | Escalate to MCP when |
| --- | --- | --- | --- |
| YouTube | Search, video metadata, transcript | `opencli youtube search`, `opencli youtube video`, `opencli youtube transcript` | Need comments, live chat, page interaction, or adapter debugging |
| NotebookLM | Notebook metadata, source list, source guide, summary, source fulltext | `opencli notebooklm ...` | Need page-state debugging or the adapter stops resolving notebook state |
| Wikipedia | Search and summary | `opencli wikipedia ...` | Need arbitrary page scraping outside the adapter surface |
| Hacker News | Top stories, search, user profile | `opencli hackernews ...` | Need cross-page exploration rather than structured result rows |
| Google News | Topic headlines | `opencli google news ...` | Need raw page interaction or custom extraction from Google UI |
| General article pages | One-off article reading | MCP `fetch` or `imagefetch` | A site-specific adapter exists and is more structured |
| Reddit or other logged-in social sources | Search, thread read, user/activity data | Site-specific OpenCLI adapter when available | Need dynamic interaction or selectors/network debugging |

## Operating Rules

- Prefer OpenCLI when there is already a site adapter that returns the fields you need.
- Prefer `-f json` and an explicit `--limit` whenever possible.
- Prefer public adapters before browser-backed adapters when both are viable.
- Use MCP browser tools first when exploring a new site or debugging a broken adapter.
- Do not route repo-local code or git tasks to OpenCLI.
- Do not treat OpenCLI as a required dependency for `/gal`, `/gal research`, or any control-plane command.
- Do not generalize OpenCLI-first into a repo-wide rule for unrelated skills.
- If neither OpenCLI nor its documented fallback can satisfy the task, stop with an explicit no-tool message instead of pretending the retrieval succeeded.

## Recommended Query Pattern

Use this progression for research tasks:

1. Try a site-specific OpenCLI command with `--limit` and `-f json`.
2. If the shortlist is sufficient, synthesize from that output directly.
3. If more depth is needed, upgrade to a more detailed OpenCLI command such as transcript, fulltext, or guide.
4. If the adapter cannot reach the needed data, switch to MCP browser tools for inspection or fallback extraction.

```text
question
-> opencli shortlist
-> opencli detail (optional)
-> MCP inspection or fallback (optional)
-> model synthesis
```

## OpenCLI Plugin Shape In GAL

Within GAL, OpenCLI should be treated as:

- an optional external CLI runtime
- consumed through skills in the execution layer
- especially useful for research/data retrieval workflows

It should not be treated as:

- an MCP manifest entry
- a control-plane dependency
- a replacement for MCP browser tools

## Adapter Coverage Reference

OpenCLI maintains a large upstream adapter registry covering browser-backed sources, public API sources, and desktop adapters.

Use these sources in order:

- Local snapshot in this repo: [opencli-coverage.md](opencli-coverage.md)
- Upstream registry: [OpenCLI adapters index](https://github.com/jackwener/OpenCLI/blob/main/docs/adapters/index.md)
- Local live registry: `opencli list`

`opencli-coverage.md` is the fast local lookup layer for GAL users and agents.
The upstream project remains the source of truth, and `opencli list` remains the refresh path for the currently installed local registry.

## Maintenance

- If GAL guidance conflicts with upstream OpenCLI behavior, verify with `opencli list` and the upstream adapters index.
- If a new source becomes common in GAL research workflows, add it to the Common Source Routing table above.
- If OpenCLI becomes a frequent dependency across repos, promote the associated skill guidance, not the control plane.
- Keep the exception model explicit: some skills are intentionally MCP-first or local-first and should stay that way.

## Related Docs

- [installation-topology.md](installation-topology.md)
- [ai-agent-onboarding.md](ai-agent-onboarding.md)
- [opencli-coverage.md](opencli-coverage.md)
- [research/20260410-gal-opencli-token-feasibility.md](research/20260410-gal-opencli-token-feasibility.md)
