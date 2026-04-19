# gstack Collaborative Tool Contract

gstack is an optional specialist collaborative tool for GAL, not a core runtime dependency. GAL keeps its own planning entry points, execution commands, and repo-local state model whether gstack is installed or not.

## What This Module Covers

When gstack is installed, GAL can use it for two kinds of enhancement:

1. planning-stage review capabilities such as discovery-style planning, business review, design review, and engineering review
2. specialist execution commands whose outputs still write back into GAL-owned files

Without gstack, GAL still works through native planning commands and fallback golems.

## What Does Not Change When gstack Is Installed

- `/gal` still owns the control plane.
- Repo-local Markdown files still own state.
- Specialist commands still write back to the same plan sections and output directories.
- Collaborative-tool routing still happens at the workflow layer, not by silently changing an agent persona.

## Two Separate Checks

Keep these checks separate in implementation and documentation:

- tool availability: does this machine have a supported gstack install?
- tool readiness: does this repo or branch have the files needed for the chosen formal workflow?

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

## Execution Specialists

The execution-stage commands do not depend on gstack planning being present. Their value is that they keep a disciplined write-back model.

| Command family | Examples | Primary write-back |
| --- | --- | --- |
| design execution | `/design-consultation`, `/design-shotgun`, `/design-html`, `/design-review` | `DESIGN.md`, `docs/designs/`, `docs/design-reports/`, plan review sections |
| review and QA | `/review`, `/qa`, `/qa-only`, `/cso` | plan `## Review Results`, `## Analyze`, `## Test Results`, `docs/qa-reports/` |
| release | `/ship`, `/land-and-deploy`, `/document-release` | plan `## Ship`, `## Deploy`, and repo docs |
| safety and memory | `/learn`, `/careful`, `/freeze`, `/guard`, `/unfreeze` | `.dev/learnings.jsonl` or session-only state |

For the full command map, use [../command-index.md](../command-index.md). For the control-plane contract and runtime surface, use [../../commands/commands.md](../../commands/commands.md).

## Upstream Semantics Mapped Into GAL

GAL does not claim full equivalence with upstream gstack. The goal is narrower: preserve useful specialist semantics while keeping GAL's own state ownership and runtime layout.

| Upstream-style capability | GAL interpretation |
| --- | --- |
| discovery-style feature planning | creates a source plan in `docs/plans/` |
| engineering review lane | writes `## Test Plan` and `## Tasks` into the source plan before execution |
| staff-style code review | writes a drift-readable verdict into `## Analyze` |
| QA run with persistent report | writes `## Test Results` and stores reports in `docs/qa-reports/` |
| design variant exploration | stores files in `docs/designs/` |
| sprint learnings | stores repo-local learnings in `.dev/learnings.jsonl` |

## Runtime Notes

| Runtime | Role |
| --- | --- |
| Copilot | primary interactive control plane |
| Gemini CLI | bounded worker runtime |
| Codex CLI | shares the same write-back contract and uses `$skill` invocation |

## Read Next

- [../../README.md](../../README.md) for the main repo entry point.
- [../command-index.md](../command-index.md) for the human-facing command map.
- [../../commands/commands.md](../../commands/commands.md) for control-plane behavior, aliases, and runtime surface.
- [../../workflows/coding.md](../../workflows/coding.md) for the execution lifecycle.
- `../../commands/<command>/SKILL.template.md` for the exact prompt and write-back behavior of a specific command.
