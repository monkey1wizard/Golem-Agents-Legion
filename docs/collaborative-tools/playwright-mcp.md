# Playwright MCP Collaborative Tool Guide

This document defines when GAL should use Playwright MCP, how it should preflight
the capability, and how browser-backed work should degrade when the tool or task is
not ready.

## Positioning

Playwright MCP is an optional browser automation capability exposed through GAL's
managed MCP manifest and runtime bridges. It is not a new `/gal` command, not a
replacement for native Playwright scripts, and not a substitute for local-first
research or structured retrieval.

Use it when a lane needs live browser interaction, accessibility snapshots,
screenshots, responsive inspection, storage/session checks, or browser-visible MCP
evaluation. Do not use it when a cheaper deterministic path already proves the task.

## Preflight - Shared Checking Model

Playwright MCP follows the shared preflight model in [checking-contract.md](checking-contract.md).

| Shared state | Playwright MCP meaning | GAL behavior |
| --- | --- | --- |
| `not-applicable` | The task does not need observable browser behavior. | Use the non-browser path. |
| `unavailable` | The current machine or runtime cannot launch or reach Playwright MCP at all. | Fall back to the documented non-Playwright route and say browser execution did not happen. |
| `available-but-needs-init` | Playwright MCP is wired, but required first-run browser/server initialization is incomplete. | Do not auto-initialize during normal testing, review, design audit, or research. |
| `available-but-not-ready` | Playwright MCP exists, but the current task lacks the browser preconditions needed for this lane. | Degrade to the documented alternative route for the task shape. |
| `ready` | Playwright MCP is applicable and the current lane has the required browser preconditions. | Route into Playwright MCP. |

## Readiness Boundaries

Use these boundaries when deciding between `available-but-needs-init` and
`available-but-not-ready`.

- Availability: the runtime can resolve the managed Playwright MCP server and launch the tool.
- Initialization: Node/npm or the Playwright browser install state is incomplete for this machine/runtime.
- Readiness: the task has a browser-appropriate target and does not require local-only state that is absent.

Common `available-but-not-ready` examples:

- the task only needs static docs or repo-local inspection
- the task requires a running UI or reachable page but none exists
- the task needs headed mode, persistent profile, storage state, extension, CDP, or output paths that were not explicitly supplied through local overrides
- the task needs a reusable regression flow better served by native Playwright scripts

## Quick Decision Table

| Task shape | Default route | Prefer instead when |
| --- | --- | --- |
| Live browser QA, forms, dialogs, tabs, upload/drop, interaction-heavy checks | Playwright MCP | The flow should become a reusable scripted regression |
| Accessibility snapshot or browser-backed assertion | Playwright MCP | Static markup or lower-cost tests already prove the requirement |
| Live design audit, responsive checks, screenshots, highlight/annotation evidence | Playwright MCP | There is no running UI, or the work is still planning-stage only |
| Deep protocol, network, or performance debugging | Chrome DevTools MCP | Playwright MCP lacks the required diagnostic depth |
| Dynamic-page research with interactive rendering | Playwright MCP after local-first and structured retrieval checks | OpenCLI, fetch, or workspace tools already answer the question |
| Browser-visible MCP or Web UI evaluation | Playwright MCP | The server is API-only and can be evaluated with Inspector, CLI, or tests |
| Deterministic reusable automation | Native Playwright scripts or future Playwright CLI/SKILL | Persistent browser context or exploratory state is the primary need |

## Safe Defaults And Local-Only State

- Tracked GAL defaults should remain isolated, headless, and non-persistent.
- Local-only overrides own headed mode, viewport or device settings, storage state,
  persistent profiles, output directories, optional capability flags, extension
  connection, CDP or remote endpoints, and any secret-like local paths.
- Playwright MCP is not a security boundary. Do not treat origin filters, file-access
  guardrails, or `--secrets` as a substitute for client-side trust controls.

## Degrade Path

If Playwright MCP is unavailable, needs initialization, or is not ready for the
current task:

- fall back to Chrome DevTools MCP for deep browser diagnostics when that lane still applies
- fall back to native Playwright scripts or future Playwright CLI/SKILL routing for reusable automation
- fall back to OpenCLI, fetch, Defuddle, or workspace tools when the task is research or documentation retrieval
- stop with an explicit browser-capability message when no documented non-browser route can satisfy the task

Do not claim browser validation succeeded when Playwright MCP did not run.

## Operating Rules

- Route by task shape, not by a blanket "Playwright first" rule.
- Keep the normal GAL write-back targets unchanged.
- Preserve evidence when Playwright MCP is used for testing, design audit, verification, or research.
- Prefer the cheapest tool that can prove the requirement.
- Treat first-run browser installation or server bootstrap as initialization work, not normal lane execution.

## Related Files

- [checking-contract.md](checking-contract.md) for the shared state model.
- [opencli.md](opencli.md) for structured retrieval and research-side routing.
- [../devguide.md](../devguide.md) for setup and runtime topology.