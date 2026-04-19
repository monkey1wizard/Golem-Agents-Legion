# graphify Collaborative Tool Contract

graphify is an optional structural-context collaborative tool for GAL, not a core runtime dependency. If a target repo already contains graphify outputs, GAL may consume them to improve planning and review quality. If not, GAL behaves exactly as before.

## What This Module Covers

This module defines GAL's consumption contract for prebuilt graphify artifacts.

When `graphify-out/GRAPH_REPORT.md` exists at the repo root, GAL may read it in these workflows:

- `/planning` to judge whether scope crosses module boundaries
- `/deep-planning` to add structure-aware context before converging a source plan
- `golem-architect` to reason about core abstractions, coupling, and module boundaries
- `/review` to cross-check whether code changes create unexpected cross-community edges

This module does not document graphify installation, graph generation, rebuild commands, or full query usage. Those remain upstream responsibilities.

## What Does Not Change When graphify Is Present

- `/gal` still owns the control plane.
- Repo-local Markdown files still own plan, review, and execution state.
- GAL does not install or require graphify.
- Workflow routing does not silently switch collaborative tools because a report exists.
- Missing graphify outputs must not cause errors, setup prompts, or mandatory fallback steps.

## Preflight - Shared Checking Model

This tool follows the shared preflight model in [checking-contract.md](checking-contract.md).

| Shared state | graphify meaning | GAL behavior |
| --- | --- | --- |
| `not-applicable` | The current lane does not use structural graph context. | Continue without graphify. |
| `unavailable` | The machine or runtime does not have graphify installed and no live-query wiring exists. | Continue with native codebase reading. |
| `available-but-needs-init` | graphify is installed, but the repo has not been generated into `graphify-out/` for the expected collaboration mode. | Do not auto-generate graph outputs during planning or review. |
| `available-but-not-ready` | graphify is installed, but the current lane is missing the required artifact such as `graphify-out/GRAPH_REPORT.md` for report mode or `graphify-out/graph.json` for live-query follow-up. | Degrade to the normal non-graph workflow path. |
| `ready` | The applicable graphify artifact exists for the current integration level. | Use graphify as advisory structural context. |

Report-based integration uses repo readiness, not machine availability alone. A machine with graphify installed but no repo outputs is not graph-ready for GAL.

## GAL-Consumed Files And Signals

| Input | Required | GAL use |
| --- | --- | --- |
| `graphify-out/GRAPH_REPORT.md` | yes for report-based integration | Read god nodes, communities, and surprising connections as advisory structural context |
| `graphify-out/graph.json` | no | Reserved for optional live-query follow-up, not required for the base contract |

For report-based integration, GAL reads the report and keeps its normal write-back targets:

- source plans in `docs/plans/`
- execution prompts in `.dev/plans/`
- review results in plan `## Review Results`
- analyze verdicts in prompt `## Analyze`

graphify does not become a new state owner. It only provides extra structure evidence when present.

## Integration Levels

| Level | Trigger | GAL behavior |
| --- | --- | --- |
| Level 1: report-based context | `graphify-out/GRAPH_REPORT.md` exists | Read the report before planning or review work and use it as advisory structural evidence |
| Level 2: optional live-query follow-up | a runtime explicitly wires graphify query tools | Use targeted graph queries for follow-up exploration without changing GAL's state ownership |

At Level 1, treat `INFERRED` edges as advisory signals rather than hard facts.

If graphify is unavailable or not ready, degrade to native codebase reading and standard GAL planning or review behavior. Do not prompt for installation or graph regeneration unless the user explicitly asked for graphify-specific capability.

## Non-Goals

- Installing graphify for the user
- Rebuilding stale graph outputs
- Managing graph freshness in GAL workflows
- Copying graphify's full CLI or MCP manual into this repo
- Making graphify a requirement for planning, architect review, or staff review
- Changing the normal GAL write-back sections or output files

## Upstream Handoff

For installation, graph generation, freshness management, and query semantics, use the upstream graphify project: [safishamsi/graphify](https://github.com/safishamsi/graphify).
