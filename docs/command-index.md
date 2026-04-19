# Command Index

This is the index for GAL commands. Start here when you need to decide what to run next. For exact runtime behavior, alias rules, dispatch details, and install surface, follow the source links in this file.

## Three Rules

- `/gal` owns the control plane. It answers status, next-step, wrap-up, and bounded orchestration questions.
- Specialist commands do the work directly. They must write back to the plan sections and repo files that `/gal` reads.
- Repo-local Markdown files are the ownership boundary. GAL does not depend on upstream gstack global storage as its core state model.

## Quick Routing

| If you need to... | Start with | Read next |
| --- | --- | --- |
| bootstrap a repo into GAL | `/gal init` | [../commands/commands.md](../commands/commands.md) |
| know current progress or blockers | `/gal status` | [../commands/commands.md](../commands/commands.md) |
| get a single next action | `/gal whats-next` | [../commands/commands.md](../commands/commands.md) |
| close a session cleanly | `/gal wrap-up` | [../commands/commands.md](../commands/commands.md) |
| create or deepen a plan | `/planning` or `/deep-planning` | [collaborative-tools/gstack.md](collaborative-tools/gstack.md) |
| materialize an execution work file | `/plan-to-prompt` | [collaborative-tools/gstack.md](collaborative-tools/gstack.md) |
| run execution, QA, review, or release work | the relevant specialist command | `commands/<command>/SKILL.template.md` |

## Control Plane

The control-plane contract, dispatch block schema, alias behavior, and runtime surface are defined in [../commands/commands.md](../commands/commands.md).

| Command | What it answers | Primary files |
| --- | --- | --- |
| `/gal init` | How does this repo enter GAL management? | creates `.dev/project.md` and `.dev/state.md` |
| `/gal status` | Where are we right now? | reads `.dev/state.md` and the active plan |
| `/gal whats-next` | What should happen next? | reads state plus review, test, ship, and deploy sections |
| `/gal wrap-up` | How do I stop cleanly? | updates `### Handoff Notes` and session continuity |
| `/gal research` | How do I enter structured research mode? | routes into the research workflow |
| `/gal pipeline` | How do tasks move through implementation, test, review, and verification? | reads active prompt, `## Tasks`, `## Test Plan`, and model routing |

## Specialist Families

Exact per-command prompts live in `commands/<command>/SKILL.template.md`. This index stays intentionally high level.

### Planning And Review Before Execution

| Command or lane | Purpose | Primary write-back |
| --- | --- | --- |
| `/planning` | create a source plan from a request | `docs/plans/<plan-slug>.md` |
| `/deep-planning` | refine a source plan until it is implementation-ready | `docs/plans/<plan-slug>.md` |
| `/plan-to-prompt` | convert a reviewed source plan into an execution work file | `.dev/plans/<plan-slug>.prompt.md` |
| business or scope review lane | challenge ambition, scope, and value ordering | source plan `## Review Results` and `## Open Questions` |
| design review lane | close UX, state, accessibility, and design-system gaps | source plan `## Review Results` and `## Open Questions` |
| engineering review lane | close architecture, tasking, and test readiness gaps | source plan `## Review Results`, `## Test Plan`, and `## Tasks` |
| `/cso` | add security findings when the change touches trust boundaries | plan `## Review Results` |

Planning-stage review lanes are capabilities, not public GAL command names. If gstack is installed they can be routed. If not, GAL still has native planning and fallback golems. See [collaborative-tools/gstack.md](collaborative-tools/gstack.md).

All collaborative-tool-routed lanes should resolve tool state through [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md) before attempting tool-specific capabilities.

### Design Work

| Command | Purpose | Primary write-back |
| --- | --- | --- |
| `/design-consultation` | establish or update the repo-level design system | `DESIGN.md` and `CLAUDE.md` |
| `/design-shotgun` | explore multiple directions and approve one | `docs/designs/<plan-slug>/variant-approved.json` |
| `/design-html` | turn an approved mockup into executable HTML or a component | `docs/designs/<plan-slug>/handoff-final.html` |
| `/design-review` | audit a running UI against `DESIGN.md` | plan `## Review Results` and `docs/design-reports/` |

### Build, Debug, Review, And QA

| Command | Purpose | Primary write-back |
| --- | --- | --- |
| `/investigate` | root-cause-first debugging with a bounded edit surface | debug notes or the active plan |
| `/review` | staff-level diff review and drift detection | plan `## Review Results` and `## Analyze` |
| `/browse` | browser primitive used by higher-level workflows | session only |
| `/connect-chrome` | switch to a headed Chrome session | session only |
| `/setup-browser-cookies` | import authentication into the browser session | session only |
| `/qa` | execute the test plan and fix discovered bugs | plan `## Test Results` and `docs/qa-reports/` |
| `/qa-only` | execute QA and report findings without code changes | plan `## Test Results (Report Only)` and `docs/qa-reports/` |

### Release, Safety, And Memory

| Command | Purpose | Primary write-back |
| --- | --- | --- |
| `/ship` | final release gate before merge | plan `## Ship` |
| `/land-and-deploy` | merge, deploy, and verify the deployment | plan `## Deploy` |
| `/setup-deploy` | establish deploy configuration baseline | `CLAUDE.md` |
| `/document-release` | sync released behavior back into docs | repo docs and PR-facing release notes |
| `/learn` | maintain repo-local institutional memory | `.dev/learnings.jsonl` |
| `/careful` | warn before destructive commands | session only |
| `/freeze` | lock edits to a bounded directory | session only |
| `/guard` | enable `careful` and `freeze` together | session only |
| `/unfreeze` | remove the current freeze boundary | session only |
| `/gstack-upgrade` | delegate to the upstream gstack compatibility shim | local machine maintenance |

## Dispatch Protocol Summary

For script-dispatched control-plane actions, the CLI emits a bounded block that the model executes:

```text
--- GAL DISPATCH ---
COMMAND: <init|error|suggest>
ROLE: <golem-name>
MODE: <bound|consult|utility>
READ: <file-path>
ACTION: <instruction text>
ON_COMPLETE: <next-step hint>
--- END DISPATCH ---
```

The authoritative version lives in [../commands/commands.md](../commands/commands.md).

## Plan Write-Back Map

This is the minimal map humans usually need when checking whether a command wrote to the right place.

| Section or file | Primary owner |
| --- | --- |
| `docs/plans/<plan-slug>.md` | planning and deep-planning work |
| source plan `## Open Questions` | planning-stage review lanes |
| source plan `## Review Results` | planning-stage review lanes |
| source plan `## Test Plan` | engineering review lane |
| source plan `## Tasks` | engineering review lane |
| `.dev/plans/<plan-slug>.prompt.md` | `/plan-to-prompt` |
| execution prompt `## Review Results` | `/review`, `/design-review`, `/cso` |
| execution prompt `## Analyze` | `/review` |
| execution prompt `## Test Results` | `/qa`, `/qa-only` |
| execution prompt `## Ship` | `/ship` |
| execution prompt `## Deploy` | `/land-and-deploy` |
| `### Handoff Notes` and `.dev/state.md` | `/gal wrap-up` |

## Required Gate

The only required pre-execution planning gate is engineering review readiness. Business and design review lanes are optional enhancements, not universal blockers.

## Runtime Notes

| Runtime | Role |
| --- | --- |
| Copilot | primary interactive control plane |
| Gemini CLI | bounded worker runtime |
| Codex CLI | shares the same write-back contract, but uses `$skill` invocation instead of slash commands |

## Read Next

- [../commands/commands.md](../commands/commands.md) for the control-plane contract, alias rules, and runtime surface.
- [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md) for shared collaborative-tool preflight behavior.
- [collaborative-tools/gstack.md](collaborative-tools/gstack.md) for optional collaborative-tool semantics and planning-stage review integration.
- [../workflows/coding.md](../workflows/coding.md) for the execution state machine.
- `commands/<command>/SKILL.template.md` for the exact prompt and write-back behavior of a specific specialist command.
