# Research: Playwright MCP Integration

Date checked: 2026-05-19

## Question

What upstream Playwright MCP behavior should GAL assume when planning managed browser automation support?

## Sources

- [microsoft/playwright-mcp README](https://github.com/microsoft/playwright-mcp)
- [Raw README](https://raw.githubusercontent.com/microsoft/playwright-mcp/main/README.md)
- [MCP Security Best Practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices)

## Upstream sections checked

- `### Playwright` — MCP vs Playwright CLI trade-off.
- `### Requirements` — runtime prerequisite baseline.
- `### Getting started` — standard manifest shape and common client examples.
- `### Configuration` — current CLI/config options and opt-in capability surface.
- `### User profile` — persistent profile default, isolated mode, extension-backed connection.
- `### Initial state` — storage state, init-page, init-script, and page/context setup.
- `### Standalone MCP server` — HTTP/SSE transport for headed or worker-hosted execution.
- `## Security` — explicit warning that Playwright MCP is not a security boundary.
- `### Tools` — current core tools and opt-in capability groups.

## Verified upstream contract

### Standard managed entry

- Upstream examples consistently use the server key `playwright`.
- Standard stdio config is `command: npx` with `args: ["@playwright/mcp@latest"]`.
- Several upstream client examples also show `-y` in front of `@playwright/mcp@latest`; this is the safer GAL default for non-interactive startup.

### Runtime and transport

- Upstream requires Node.js 18 or newer.
- Normal MCP client setup is stdio-based through a manifest entry.
- Upstream also supports standalone HTTP transport by starting the server with `--port`, for example `npx @playwright/mcp@latest --port 8931`, then pointing the client at `http://localhost:8931/mcp`.
- The README explicitly positions standalone transport as useful when running headed browsers on systems without a display attached to the calling process, or from IDE worker processes.

### Profile and state behavior

- Upstream default is a persistent profile. Logged-in information is stored under machine-local Playwright cache paths keyed by browser channel and workspace hash.
- `--isolated` keeps the browser profile in memory and discards storage when the browser closes.
- Persistent user state can also be introduced through `--user-data-dir`, `--storage-state`, extension connection, CDP endpoints, remote endpoints, configuration files, `--init-page`, and `--init-script`.
- Isolated mode can still be seeded with `--storage-state`, which means stateful setup is compatible with isolation but should stay local-only in GAL.

### Security and trust boundary

- Upstream explicitly states that Playwright MCP is not a security boundary.
- Upstream also documents that `--secrets` is a convenience for redacting tool responses, not a security feature.
- `--allow-unrestricted-file-access` is framed as a convenience guardrail override, not a secure sandbox boundary.
- Origin allow/block settings do not serve as a full security boundary and do not affect redirects.

### Current option surface relevant to GAL

- Conservative startup flags available now include `--isolated` and `--headless`.
- Stateful or environment-sensitive options include `--user-data-dir`, `--storage-state`, `--output-dir`, `--save-session`, `--shared-browser-context`, `--extension`, `--cdp-endpoint`, `--endpoint`, `--device`, `--viewport-size`, `--grant-permissions`, `--proxy-server`, and `--proxy-bypass`.
- Network and file-scope options include `--allowed-hosts`, `--allowed-origins`, `--blocked-origins`, `--block-service-workers`, and `--allow-unrestricted-file-access`.
- Output and diagnostics options include `--output-mode`, `--output-dir`, console-level settings, tracing/video tools, screenshot/PDF output, and snapshot controls.

### Tool surface relevant to GAL routing

- Core automation tools cover navigation, click, hover, drag, drop, file upload, form fill, typing, keyboard input, dialog handling, evaluate, wait, resize, tabs, snapshots, screenshots, console messages, and network inspection.
- Opt-in capability groups currently documented upstream include `config`, `network`, `storage`, `devtools`, `vision`, `pdf`, and `testing`.
- The exact capability names and defaults remain version-sensitive and must be revalidated before T-002 through T-010 lock in tracked behavior.

### MCP vs CLI trade-off

- Upstream explicitly distinguishes Playwright MCP from Playwright CLI + SKILL workflows.
- CLI/skill routing is presented as more token-efficient for coding agents and better for high-throughput reusable automation.
- MCP is presented as the better fit for exploratory, stateful, iterative browser work and long-running agentic loops where persistent context or richer introspection matter.

## Safe tracked-default assumptions for GAL

- Canonical managed key should be `playwright`, matching upstream examples and avoiding runtime alias drift.
- GAL tracked defaults should prefer `npx -y @playwright/mcp@latest --isolated --headless`.
- GAL tracked defaults should not set persistent profiles, storage-state paths, output directories, optional capabilities, extension connection, CDP endpoints, remote endpoints, unrestricted file access, or secret-file paths.
- Local overrides should own headed mode, viewport/device emulation, storage state, persistent profile paths, output directories, optional caps, extension/CDP/remote connections, and any machine-local or secret-like values.

## Version-sensitive assumptions to revalidate during implementation

- `-y` remains accepted and appropriate for non-interactive `npx` startup.
- `--isolated` still forces in-memory browser profile behavior.
- `--headless` remains the correct explicit flag for conservative non-interactive GAL defaults.
- Capability groups and their names remain `config`, `network`, `storage`, `devtools`, `vision`, `pdf`, and `testing`.
- No newly documented security caveat changes the recommendation to keep optional capabilities and stateful browser setup local-only.
- The standalone HTTP transport contract remains `--port` plus MCP client `url` configuration.

## Planning implications

- GAL should treat Playwright MCP as a managed browser capability only when the tracked config forces conservative defaults.
- Local overrides should own persistent profiles, storage state, browser extensions, CDP endpoints, viewport/device settings, output directories, optional capabilities, and any secret-like paths.
- Playwright MCP should not replace native Playwright scripts or future Playwright CLI/SKILL usage for reusable regression automation.
- The plan correctly needs to cover browser QA, design audit, dynamic-page research, MCP/browser-visible evaluation, storage/session handling, PDF/artifact capture, and remote/headless transport routing.
