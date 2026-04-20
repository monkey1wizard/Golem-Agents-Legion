# Command Index

This is the index for GAL's current command surface. Start here when you need to decide what to run next.

## Three Rules

- `/gal` owns the control plane
- `/planning`, `/deep-planning`, and `/plan-to-prompt` own planning
- execution-stage work is routed to golem agents, not separate specialist slash commands

## Quick Routing

| If you need to... | Start with | Read next |
| --- | --- | --- |
| bootstrap a repo into GAL | `/gal init` | [../commands/commands.md](../commands/commands.md) |
| know current progress or blockers | `/gal status` | [../commands/commands.md](../commands/commands.md) |
| get a single next action | `/gal whats-next` | [../commands/commands.md](../commands/commands.md) |
| close a session cleanly | `/gal wrap-up` | [../commands/commands.md](../commands/commands.md) |
| create or deepen a plan | `/planning` or `/deep-planning` | [collaborative-tools/gstack.md](collaborative-tools/gstack.md) |
| materialize an execution work file | `/plan-to-prompt` | [collaborative-tools/gstack.md](collaborative-tools/gstack.md) |
| implement tasks automatically | `/gal pipeline` | [../workflows/coding.md](../workflows/coding.md) |
| run QA, design, review, debug, security, or release work | the relevant golem agent | [../agent/agents.md](../agent/agents.md) |

## Public Commands

The control-plane contract, dispatch block schema, alias behavior, and runtime surface are defined in [../commands/commands.md](../commands/commands.md).

| Command | What it answers | Primary files |
| --- | --- | --- |
| `/gal init` | How does this repo enter GAL management? | creates `.dev/project.md` and `.dev/state.md` |
| `/gal status` | Where are we right now? | reads `.dev/state.md` and the active plan |
| `/gal whats-next` | What should happen next? | reads state plus review, test, and release sections |
| `/gal wrap-up` | How do I stop cleanly? | updates `### Handoff Notes` and session continuity |
| `/gal research` | How do I enter structured research mode? | routes into the research workflow |
| `/gal deep-research` | How do I enter multi-source research mode? | routes into the deep-research workflow |
| `/gal pipeline` | How do tasks move through implementation, test, review, and verification? | reads active prompt, `## Tasks`, `## Test Plan`, and model routing |
| `/planning` | How do I create a source plan? | writes `docs/plans/<plan-slug>.md` |
| `/deep-planning` | How do I harden a source plan for implementation? | updates `docs/plans/<plan-slug>.md` |
| `/plan-to-prompt` | How do I materialize execution work? | writes `.dev/plans/<plan-slug>.prompt.md` |

## Agent Routing

Exact agent prompts live in `agent/*.agent.md`. This index stays intentionally high level.

| If you need... | Route to | Primary write-back |
| --- | --- | --- |
| design system creation, variants, build, or live UI audit | `golem-designer` | `DESIGN.md`, `docs/designs/`, plan `## Review Results` |
| spec-driven tests or browser QA | `golem-tester` | plan `## Test Results` |
| staff review and drift analysis | `golem-reviewer` | plan `## Review Results`, `## Analyze` |
| root-cause-first debugging | `golem-debugger` | debug notes or active plan |
| security audit | `golem-security` | plan `## Review Results` |
| release prep, deploy, or doc sync | `golem-releaser` | plan `## Release`, repo docs |

Planning-stage review lanes are still capabilities, not public GAL command names. If gstack is installed they can be routed. If not, GAL still has native planning and fallback golems. See [collaborative-tools/gstack.md](collaborative-tools/gstack.md).

All collaborative-tool-routed lanes should resolve tool state through [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md) before attempting tool-specific capabilities.

## Plan Write-Back Map

| Section or file | Primary owner |
| --- | --- |
| `docs/plans/<plan-slug>.md` | planning and deep-planning work |
| source plan `## Open Questions` | planning-stage review lanes |
| source plan `## Review Results` | planning-stage review lanes |
| source plan `## Test Plan` | engineering review lane |
| source plan `## Tasks` | engineering review lane |
| `.dev/plans/<plan-slug>.prompt.md` | `/plan-to-prompt` |
| execution prompt `## Review Results` | `golem-reviewer`, `golem-designer`, `golem-security` |
| execution prompt `## Analyze` | `golem-reviewer` |
| execution prompt `## Test Results` | `golem-tester` |
| execution prompt `## Release` | `golem-releaser` |
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

- [../commands/commands.md](../commands/commands.md) for the control-plane contract, alias rules, and runtime surface
- [../agent/agents.md](../agent/agents.md) for the specialist routing map
- [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md) for shared collaborative-tool preflight behavior
- [collaborative-tools/gstack.md](collaborative-tools/gstack.md) for optional planning-stage review integration
- [../workflows/coding.md](../workflows/coding.md) for the execution lifecycle
