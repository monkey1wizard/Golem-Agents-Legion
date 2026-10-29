---
name: defuddle
description: Extract clean markdown content from web pages using Defuddle CLI, removing clutter and navigation to save tokens. Use instead of WebFetch when the user provides a URL to read or analyze, for online documentation, articles, blog posts, or any standard web page.
cliDependencies:
  required:
    - defuddle
mcpDependencies:
  optional:
    - fetch
    - imagefetch
---

# Defuddle

Use Defuddle CLI to extract clean readable content from web pages. Prefer over WebFetch for standard web pages — it removes navigation, ads, and clutter, reducing token usage.

## Preferred Tool Order

1. Use Defuddle for standard article-style pages where clean markdown extraction is the main goal.
2. Use `fetch` when Defuddle is unavailable, the page is not article-like, or raw retrieval is preferable.
3. Use `imagefetch` when the task depends on understanding page images as well as text.

## Availability Check

Before using Defuddle, verify the CLI is installed:

```bash
defuddle --help
```

- Exit code `0`: Defuddle is available.
- Non-zero or command-not-found: Defuddle is unavailable. Use the fallback strategy below.

If Defuddle is unavailable, the page is not a standard article page, or you need raw page retrieval rather than article extraction, fall back to the `fetch` MCP when it is available. If the task depends on understanding page images, prefer `imagefetch` over plain fetch.

If not installed: `npm install -g defuddle`

## Fallback Strategy

- Use `fetch` for raw or general page retrieval when Defuddle is unavailable.
- Use `imagefetch` instead of plain `fetch` when page imagery is part of the task.
- If the page is highly interactive rather than article-like, switch to a browser-oriented tool instead of forcing Defuddle.

## No-Tool Behavior

If neither Defuddle nor the documented MCP fallback is available, say that clean page extraction is unavailable in the current runtime.

- Ask the user for the source URL or page content if a manual path is still viable.
- Do not describe the extraction as complete when the tool path was missing.

## Usage

Always use `--md` for markdown output:

```bash
defuddle parse <url> --md
```

Save to file:

```bash
defuddle parse <url> --md -o content.md
```

Extract specific metadata:

```bash
defuddle parse <url> -p title
defuddle parse <url> -p description
defuddle parse <url> -p domain
```

## Output formats

| Flag | Format |
| --- | --- |
| `--md` | Markdown (default choice) |
| `--json` | JSON with both HTML and markdown |
| (none) | HTML |
| `-p <name>` | Specific metadata property |
