# Commands

GAL now exposes a deliberately small public command surface: 10 commands total.

- Control plane: `/gal`, `/gal-init`, `/gal-status`, `/gal-whats-next`, `/gal-wrap-up`, `/gal-pipeline`
- Planning: `/planning`, `/deep-planning`, `/plan-to-prompt`, `/refining-plan`

Everything else that used to live behind execution-stage slash commands is now owned by golem agents.

## Architecture

GAL keeps one shared public command contract and packages it into each supported runtime's native command or skill surface via `Setup-Machine`.

Runtime-specific install paths, generated files, rule shims, and MCP details are documented in `scripts/scripts.md` and `docs/devguide.md`, not in this file.

Codex note: installed GAL skills are available as Codex skills, but explicit invocation uses `$skill-name`, not custom slash-command syntax. Example: use `$gal status`, not `/gal status`.

**Key principles:**

- `/gal` owns the control plane
- planning remains a native command family
- execution-stage specialist work is agent-owned, not command-owned
- `Setup-Machine` still discovers installed commands from `commands/*/SKILL.template.md`
- cross-runtime behavior must stay consistent across supported runtimes

## Public Command Surface

### `/gal` — Control Plane Entry Point

```sh
/gal [subcommand | golem-name | free text]
```

| Invocation | What It Answers | Behavior |
| --- | --- | --- |
| `/gal` | What should I do? | Auto-detect from `.dev/state.md` — follow `/gal-whats-next` procedure |
| `/gal init` | How do I bootstrap this repo? | Scaffold `.dev/project.md` + `.dev/state.md` via script |
| `/gal status` | Where are we right now? | Full state projection — active plans, review/test/blockers/continuity |
| `/gal whats-next` | What do I do next? | Read state and recommend one next action |
| `/gal wrap-up` | How do I close this session? | Converge handoff updates and update session continuity |
| `/gal research` | I need structured investigation | Invoke research golem via script |
| `/gal deep-research` | I need multi-source investigation with cross-review | Invoke deep-research workflow via script |
| `/gal xmachine <node> to do <task-ref>` | Run one active-plan task on a readied work node | Normalize to a bounded single-task xmachine pipeline dispatch |
| `/gal <golem-name>` | Route to a supported specialist agent | Invoke a dispatcher-supported golem such as `architect`, `analyst`, `designer`, `researcher`, `debugger`, `notewriter`, `implementer`, `tester`, `reviewer`, or `verifier` |

### `gal-*` — Discoverability Aliases

| Alias | Status | Purpose |
| --- | --- | --- |
| `/gal-init` | Active | Bootstrap `.dev/` for a repo |
| `/gal-status` | Active | Full state projection |
| `/gal-whats-next` | Active | Next-action recommendation |
| `/gal-wrap-up` | Active | Session close-out |
| `/gal-pipeline` | Active | Task execution through implementer -> tester -> reviewer, with conditional security audit for security-sensitive changes, then verifier |

For Codex CLI, use the equivalent skill names with `$` invocation.

## Planning Commands

| Command | Purpose | Primary write-back |
| --- | --- | --- |
| `/planning` | Create a source plan from a request | `docs/plans/<plan-slug>.md` |
| `/deep-planning` | Refine a source plan until it is implementation-ready | `docs/plans/<plan-slug>.md` |
| `/plan-to-prompt` | Generate the execution prompt from the reviewed source plan | `.dev/plans/<plan-slug>.prompt.md` |
| `/refining-plan` | Populate source-plan `## Tasks`, `## Test Plan`, and `## Review Results > ### Engineering Review`; emit `<!-- ENG_REVIEW: CLEAR -->` when the plan passes | `docs/plans/<plan-slug>.md` |

## Agent-Owned Execution Surface

The following work no longer has a public slash command and should be routed to the named golem instead.

| Execution work | Owning agent | Primary write-back |
| --- | --- | --- |
| Spec-driven tests and real-browser QA | `golem-tester` | plan `## Test Results` |
| Staff review and drift analysis | `golem-reviewer` | plan `## Review Results`, `## Analyze` |
| Root-cause-first debugging | `golem-debugger` | plan debug log or `.dev/state.md` |
| Obsidian writes, private captures, diary, and knowledge extraction | `golem-notewriter` | user-owned vault paths or shutdown diary |
| Design system, variants, build, and live audit | `golem-designer` | `DESIGN.md`, `docs/designs/`, plan review sections |
| Security audit | `golem-security` | plan `## Review Results` |
| Release prep, deploy, and doc sync | `golem-releaser` | plan `## Release`, repo docs |

Ownership does not imply that every specialist currently has a dispatcher entry through `/gal <golem-name>`. The dispatcher scripts are the source of truth for which golems can be invoked directly.

### Golem Classification

| Golem | Class | Default Mode |
| --- | --- | --- |
| golem-implementer | Pipeline | consult when invoked directly; `bound` when dispatcher emits `DISPATCH_KIND: pipeline-phase` |
| golem-tester | Pipeline | consult when invoked directly; `bound` when dispatcher emits `DISPATCH_KIND: pipeline-phase` |
| golem-reviewer | Pipeline | consult when invoked directly; `bound` when dispatcher emits `DISPATCH_KIND: pipeline-phase` |
| golem-verifier | Pipeline | consult when invoked directly; `bound` when dispatcher emits `DISPATCH_KIND: pipeline-phase` |
| golem-architect | Domain | consult |
| golem-analyst | Domain | consult |
| golem-designer | Domain | consult |
| golem-researcher | Domain | consult |
| golem-security | Domain | direct specialist |
| golem-releaser | Domain | direct specialist |
| golem-debugger | Utility | utility |
| golem-notewriter | Utility | utility |

## Planning-Stage Review Lanes

GAL keeps planning as a native command family and routes planning-stage review by lane, not by legacy command name.

| Lane | Preferred Provider | Fallback Invocation | Write-Back Target |
| --- | --- | --- | --- |
| Business / Scope review | Upstream gstack business review provider when installed | `/gal golem-analyst` | `## Review Results` + `## Open Questions` |
| Design review | Upstream gstack design review provider when installed | `/gal golem-designer` | `## Review Results` + `## Open Questions` |
| Engineering review | Upstream gstack engineering review provider when installed | `/gal golem-architect` | Architect feedback feeds the source plan; `/refining-plan` remains the command that writes `## Tasks`, `## Test Plan`, and `<!-- ENG_REVIEW: CLEAR -->` |

Legacy gstack planning command names are not part of GAL's public command catalog. They survive only as provider semantics in integration contracts.

## Dispatch Output Protocol

For script-dispatched subcommands (`init`, `research`, `deep-research`, golem names), the CLI emits a bounded block that the AI reads and executes:

```text
--- GAL DISPATCH ---
COMMAND: <init|error|suggest>
ROLE: <golem-name>
MODE: <bound|consult|utility>
DISPATCH_KIND: <pipeline-phase>
PIPELINE_PHASE: <implement|test|review|verify|security>
TASK_SCOPE: <T-NNN>
FIX_MODE: <true>
READ: <file-path>
ACTION: <instruction text>
ON_COMPLETE: <next-step hint>
--- END DISPATCH ---
```

## Source Files

| File | Purpose |
| --- | --- |
| `commands/gal/SKILL.template.md` | Dispatcher source template |
| `commands/gal-init/SKILL.template.md` | Alias template for `/gal-init` |
| `commands/gal-status/SKILL.template.md` | Alias template for `/gal-status` |
| `commands/gal-whats-next/SKILL.template.md` | Alias template for `/gal-whats-next` |
| `commands/gal-wrap-up/SKILL.template.md` | Alias template for `/gal-wrap-up` |
| `commands/gal-pipeline/SKILL.template.md` | Alias template for `/gal-pipeline` |
| `commands/planning/SKILL.template.md` | Planning source template |
| `commands/deep-planning/SKILL.template.md` | Deep-planning source template |
| `commands/plan-to-prompt/SKILL.template.md` | Prompt generation source template |
| `commands/refining-plan/SKILL.template.md` | Engineering-review contract source template |

## Installation

Managed by `Setup-Machine.ps1` and `setup-machine.sh`. The scripts:

1. Create the runtime-facing GAL root links or references each supported runtime needs.
2. Scan all remaining `commands/*/SKILL.template.md` files, replacing `{{GAL_ROOT}}` with the absolute path.
3. Append any gitignored `commands/*/SKILL.local.md` overlay to the baked content.
4. Write baked `SKILL.md` files into each remaining `commands/*/` directory.
5. Install or generate runtime-specific command entries from the baked command content.
6. Install shared reusable skills into the runtime skill surfaces that support them.

For the current runtime topology, see `scripts/scripts.md` and `docs/devguide.md`.

If you need a machine-local customization that should survive setup reruns, put it in `commands/<command>/SKILL.local.md`. Do not edit `commands/<command>/SKILL.md` directly.
