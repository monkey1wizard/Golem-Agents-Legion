# Plan: Collaborative Tools Unified Checking

## Goal

Define one shared checking model for collaborative tools so GAL can consistently decide:

- whether a tool is applicable to the current lane
- whether the tool is installed and reachable
- whether the tool still needs first-time initialization
- whether the current repo or task is ready to collaborate with it
- how the workflow should degrade when the tool is not ready

This follow-up plan excludes the terminology rename already completed in the current change.

## Scope

- Add one shared collaborative-tool checking contract
- Standardize install, init, readiness, and fallback language
- Align graphify, OpenCLI, and gstack to the same state model
- Clarify the boundary between workflow semantics and machine-local personalization

## Non-Goals

- Do not auto-install tools
- Do not auto-run first-time tool init during normal planning, review, or research
- Do not auto-rebuild graphify outputs as a hidden side effect
- Do not change GAL state ownership or write-back targets
- Do not rename file paths in this phase unless a separate migration is approved

## Shared Checking Model

Every workflow lane that may use a collaborative tool should resolve this sequence:

```text
applicability -> availability -> initialization status -> readiness -> route -> degrade
```

### Standard States

- `not-applicable`
- `unavailable`
- `available-but-needs-init`
- `available-but-not-ready`
- `ready`

## Tool-Specific Targets

### graphify

- Applicability: planning, deep-planning, architect, review
- Availability: graphify command exists or live-query wiring exists
- Initialization status: repo-level graph generation has been done at least once when graphify collaboration is expected
- Readiness:
  - report mode: `graphify-out/GRAPH_REPORT.md` exists
  - live-query mode: `graphify-out/graph.json` exists and MCP wiring is enabled
- Degrade path: continue with native codebase reading and normal GAL planning or review behavior

### OpenCLI

- Applicability: research and external retrieval lanes
- Availability: `opencli --version` succeeds
- Initialization status: any required local OpenCLI setup, adapter install, or browser/session bridge is complete
- Readiness: the current task maps to a supported adapter that can return the required fields
- Degrade path: fall back to MCP fetch, imagefetch, browser tools, or other documented research paths

### gstack

- Applicability: planning-stage review lanes and explicitly routed specialist lanes
- Availability: machine has a supported gstack install
- Initialization status: required upstream bootstrap for this machine or repo context is complete
- Readiness: repo or branch has the formal workflow artifacts required for the chosen lane
- Degrade path: continue with GAL-native planning or fallback golems

## Documentation Changes

- Add one repo-level collaborative-tool checking contract
- Update graphify, OpenCLI, and gstack docs to reference the shared model
- Update developer guidance so workflow-level checking is explicit
- Update personalization guidance so local config only affects machine-local availability and preferences

## Workflow Changes

- Planning-family workflows should use the shared checking model before attempting graphify or gstack collaboration
- Research workflows should use the shared checking model before attempting OpenCLI collaboration
- Missing tools must not look like success
- Optional enhancement lanes should degrade cleanly without prompting for setup unless the user explicitly asked for the tool-specific capability

## Deliverables

- A shared collaborative-tool checking contract document
- Updated graphify, OpenCLI, and gstack docs aligned to the shared model
- Updated workflow guidance for planning, review, and research lanes
- Clear wording for install vs init vs ready vs fallback

## Verification

1. Verify graphify, OpenCLI, and gstack can all be described with the same state model.
2. Verify normal planning, review, and research do not silently install or initialize tools.
3. Verify personalization remains machine-local and does not redefine workflow semantics.
4. Verify fallback behavior is explicit for every collaborative tool.

## Open Questions

- Should the shared checking contract live under `docs/mod/` first, or wait for a later folder migration?
- Should `available-but-needs-init` be surfaced to users by default, or only when they explicitly request that tool capability?
- Should GAL status report collaborative-tool readiness, or should that remain a workflow-internal concern?
