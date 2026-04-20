# gstack Collaborative Tool Contract

gstack is an optional planning-stage collaborative tool for GAL, not a core runtime dependency. GAL keeps its own planning entry points, agent-owned execution model, and repo-local state whether gstack is installed or not.

## What This Module Covers

When gstack is installed, GAL can use it to enhance planning-stage review capabilities such as discovery-style planning, business review, design review, and engineering review.

Without gstack, GAL still works through native planning commands and fallback golems.

## What Does Not Change When gstack Is Installed

- `/gal` still owns the control plane.
- Repo-local Markdown files still own state.
- Specialist work is still written back to the same plan sections and output directories.
- Collaborative-tool routing still happens at the workflow layer, not by silently changing an agent persona.

## Preflight - Shared Checking Model

This tool follows the shared preflight model in [checking-contract.md](checking-contract.md).

| Shared state | gstack meaning | GAL behavior |
| --- | --- | --- |
| `not-applicable` | The current lane does not use planning-stage review enhancement or specialist collaboration. | Continue with GAL-native planning or execution. |
| `unavailable` | This machine does not have a supported gstack install. | Use fallback golems or GAL-native flow. |
| `available-but-needs-init` | gstack is installed, but required machine or repo bootstrap is incomplete. | Do not auto-bootstrap during normal planning or review. |
| `available-but-not-ready` | gstack is installed and initialized, but the current repo or branch lacks the workflow artifacts needed for the chosen lane. | Degrade to the corresponding GAL-native lane or fallback golem. |
| `ready` | The chosen lane has the required install, bootstrap, and workflow artifacts. | Route into gstack-backed collaboration. |

GAL should not treat a single `.gstack` directory or one detected file as proof that the full collaborative-tool contract is ready.

## Planning Integration

GAL-native planning starts with `/planning`, `/deep-planning`, and `/plan-to-prompt`. gstack adds optional quality lanes before execution, not a replacement for the whole planning surface.

### Core Assumption

gstack-style planning works best when one plan describes one feature, not an entire product roadmap.

- a roadmap still needs human feature selection
- each executable plan should map to one deliverable feature
- engineering review breaks one feature into tasks, it does not split one giant roadmap into multiple plans

### Recommended Flow

```text
product idea or roadmap
    -> choose one feature
    -> /planning or a discovery-style collaborative tool
    -> optional business, design, and engineering review lanes
    -> /plan-to-prompt
    -> implementation, review, QA, ship
```

### Planning-Stage Review Lanes

These are capabilities, not public GAL command names.

| Lane | Purpose | Primary write-back |
| --- | --- | --- |
| business or scope review | challenge ambition, scope, and value order | source plan `## Review Results` and `## Open Questions` |
| design review | close UX, accessibility, and design-system gaps | source plan `## Review Results` and `## Open Questions` |
| engineering review | close architecture, task, and test-readiness gaps | source plan `## Review Results`, `## Test Plan`, and `## Tasks` |

These lanes happen after the source plan draft exists and before `/plan-to-prompt` materializes the execution work file.

If gstack is unavailable or not ready, degrade to GAL-native planning commands and fallback golems. Do not surface gstack setup as a normal prerequisite unless the user explicitly asked for gstack-specific capability.

## Execution Surface Boundary

gstack does not own GAL's execution-stage public surface.

- design execution and audit are owned by `golem-designer`
- QA is owned by `golem-tester`
- code review is owned by `golem-reviewer`
- security review is owned by `golem-security`
- release work is owned by `golem-releaser`

These are GAL-native agent contracts, whether or not gstack is installed.

For the current public command surface, execution ownership map, and runtime surface, use [../../commands/commands.md](../../commands/commands.md). For the exact agent prompts, use [../../agent/agents.md](../../agent/agents.md).

## Upstream Semantics Mapped Into GAL

GAL does not claim full equivalence with upstream gstack. The goal is narrower: preserve useful specialist semantics while keeping GAL's own state ownership and runtime layout.

| Upstream-style capability | GAL interpretation |
| --- | --- |
| discovery-style feature planning | creates a source plan in `docs/plans/` |
| engineering review lane | writes `## Test Plan` and `## Tasks` into the source plan before execution |
| planning-stage design review | writes review feedback into the source plan before execution |
| business or scope review | challenges ambition, scope, and value order before execution |

## Read Next

- [../../README.md](../../README.md) for the main repo entry point.
- [../../commands/commands.md](../../commands/commands.md) for the public command surface, execution ownership, aliases, and runtime surface.
- [../../agent/agents.md](../../agent/agents.md) for the specialist routing map and agent responsibilities.
- [../../workflows/coding.md](../../workflows/coding.md) for the execution lifecycle.
- `../../commands/<command>/SKILL.template.md` for the exact prompt and write-back behavior of a specific command.
