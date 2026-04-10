---
name: opencli-research
description: Use OpenCLI for low-token, structured external retrieval when an existing site adapter matches the task. Prefer for repeatable research queries such as YouTube transcript, NotebookLM source access, Wikipedia summary, Hacker News search, and similar structured data retrieval tasks.
cliDependencies:
  required:
    - opencli
mcpDependencies:
  optional:
    - fetch
    - imagefetch
    - puppeteer
---

# OpenCLI Research

Use OpenCLI as an optional research-side plugin for structured retrieval.

OpenCLI is a CLI runtime, not an MCP server.
Use it when the task maps cleanly to an existing OpenCLI adapter and the goal is to reduce model context size by retrieving stable structured output.

## Preferred Tool Order

1. Use OpenCLI when an existing adapter already matches the source and fields you need.
2. Use MCP `fetch` or `imagefetch` when the task is still web retrieval but OpenCLI is unavailable or not the right fit.
3. Use MCP browser tools when the task depends on interaction, DOM inspection, screenshots, or adapter debugging.
4. Use workspace file and symbol tools for repo-local code understanding instead of forcing OpenCLI into the wrong lane.

## Availability Check

Before using OpenCLI, verify it is installed:

```bash
opencli --version
```

- Exit code `0`: OpenCLI is available.
- Non-zero or command-not-found: OpenCLI is unavailable. Use the fallback strategy below.

## Fallback Strategy

If OpenCLI is unavailable or the required adapter cannot reach the needed data:

1. If the target is a normal web page, fall back to MCP `fetch` or `imagefetch`.
2. If the target requires page interaction, DOM inspection, screenshots, or network debugging, fall back to MCP browser tools.
3. If the task is repo-local code understanding, use workspace file/symbol tools instead.

Do not silently pretend OpenCLI succeeded when it did not.

## No-Tool Behavior

If neither OpenCLI nor the documented MCP or workspace fallback can satisfy the task, stop and say which capability is missing.

- Ask the user for a URL, source artifact, or permission to switch to a manual path when that would still move the task forward.
- Do not fabricate adapter output.
- Do not describe the task as completed when the retrieval path was unavailable.

## Preferred Usage Pattern

- Prefer site-specific adapters over generic page reads.
- Prefer `-f json` whenever the output will be consumed by an agent.
- Always constrain result size with `--limit` when supported.
- Prefer public adapters before browser-backed adapters when both are viable.
- Upgrade from shortlist commands to detailed commands only when necessary.

## Good Fits

- `opencli youtube transcript <url> -f json`
- `opencli notebooklm summary -f json`
- `opencli notebooklm source-guide "<source>" -f json`
- `opencli wikipedia summary "<topic>" -f json`
- `opencli hackernews search "<query>" --limit 5 -f json`
- `opencli google news "<topic>" --limit 5 -f json`

## When Not To Use OpenCLI First

- The page structure is unknown and must be explored.
- You need comments, live chat, or other interactions outside an existing adapter surface.
- You need to debug why an adapter is failing.
- The task is repo-local code or git analysis.

## YouTube Example

- Use OpenCLI first for `search`, `video`, and `transcript`.
- Switch to MCP browser tools if you need comments, live interaction, DOM inspection, or adapter debugging.

## Operator Note

For the full GAL routing guidance, see [docs/opencli-routing.md](../../docs/opencli-routing.md).