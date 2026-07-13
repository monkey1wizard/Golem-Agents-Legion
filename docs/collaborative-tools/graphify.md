# graphify Collaborative Tool Contract

Member of [structural-retrieval](structural-retrieval.md); routing lives at that L1 capability surface.

graphify is an optional CLI-driven structural-context collaborative tool for GAL, not a core runtime dependency. If a target repo already contains graphify outputs, GAL may consume them to improve planning and review quality. If not, GAL behaves exactly as before.

## What This Module Covers

This module defines GAL's consumption contract for prebuilt graphify CLI artifacts.

When `graphify-out/GRAPH_REPORT.md` exists for this repo, GAL may read it in these workflows:

- `/planning` to judge whether scope crosses module boundaries
- `/deep-planning` to add structure-aware context before converging a source plan
- `golem-architect` to reason about core abstractions, coupling, and module boundaries
- `golem-auditor` to cross-check whether code changes create unexpected cross-community edges

This module does not document graphify installation, graph generation, rebuild commands, or full query usage. Those remain upstream responsibilities.

## What Does Not Change When graphify Is Present

- `/gal` still owns the control plane.
- Repo-local Markdown files still own plan, review, and execution state.
- GAL does not install or require graphify.
- GAL does not wire graphify as an MCP server.
- Normal planning, review, and research lanes do not trigger graph generation on their own.
- Workflow routing does not silently switch collaborative tools because a report exists.
- Missing graphify outputs must not cause errors, setup prompts, or mandatory fallback steps.

## Shared Preflight

graphify follows the shared preflight model in [checking-contract.md](checking-contract.md); readiness for GAL's report-based lane means `graphify-out/GRAPH_REPORT.md` is present.

## GAL-Consumed Files And Signals

| Input | Required | GAL use |
| --- | --- | --- |
| `graphify-out/GRAPH_REPORT.md` | yes for report-based integration | Read god nodes, communities, and surprising connections as advisory structural context |
| `graphify-out/GAL_GRAPHIFY_VERSION.txt` | no | Optional version stamp used only for tool-version freshness checks |
| `graphify-out/graph.json` | no | Upstream CLI artifact only; GAL does not query it live or wire it through MCP |

For report-based integration, GAL reads the report and keeps its normal write-back targets:

- source plans in `.dev/plans/`
- execution prompts in `.dev/plans/`
- review results in plan `## Review Results`
- analyze verdicts in prompt `## Analyze`

graphify does not become a new state owner. It only provides extra structure evidence when present.

When `GAL_GRAPHIFY_VERSION.txt` exists, GAL may compare its stamped graphify version to the current `graphify --version` output:

- If the versions match, the report stays usable as normal advisory context.
- If the versions differ and `GRAPH_REPORT.md` is not newer than the stamp file, GAL treats the report as stale-by-tool-version and continues without graphify context unless the user explicitly wants refreshed graph artifacts.
- If `GRAPH_REPORT.md` is newer than the stamp file, GAL assumes the graph may have been manually refreshed after the last GAL stamp and keeps the report advisory instead of blocking on the mismatch.
- If no version stamp exists, GAL does not try to infer freshness from codebase drift; it keeps the report usable as advisory context, treats it as fresh unless another stale signal is provable, and may note that automatic version verification is unavailable.

## Integration Level

Level 1: report-based context. Trigger: `graphify-out/GRAPH_REPORT.md` exists. GAL behavior: read the report before planning or review work and use it as advisory structural evidence.

GAL's graphify integration is command-based and report-based only. Treat `INFERRED` edges as advisory signals rather than hard facts. graphify remains a no-MCP lane.

If graphify is unavailable or not ready, degrade to native codebase reading and standard GAL planning or review behavior. Do not prompt for installation or graph regeneration unless the user explicitly asked for graphify-specific capability.

## Non-Goals

- Installing graphify for the user
- Rebuilding stale graph outputs automatically
- Detecting whether graph outputs match the latest codebase contents
- Copying graphify's full CLI manual into this repo
- Making graphify a requirement for planning, architect review, or staff review
- Changing the normal GAL write-back sections or output files

## Upstream Handoff

For installation, graph generation, freshness management, and query semantics, use the upstream graphify project: [safishamsi/graphify](https://github.com/safishamsi/graphify). In GAL, prefer the `graphify` command and existing generated artifacts over MCP wiring.
