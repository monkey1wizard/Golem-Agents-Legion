# Commands

GAL now exposes a deliberately small public command surface: 11 commands total.

- Control plane: `/gal`, `/gal-init`, `/gal-status`, `/gal-whats-next`, `/gal-wrap-up`, `/gal-finalize`, `/gal-pipeline`
- Planning: `/planning`, `/deep-planning`, `/plan-to-prompt`, `/refining-plan`

Everything else that used to live behind execution-stage slash commands is now owned by golem agents.

## Architecture

GAL keeps one shared public command contract and packages it into each supported runtime's native command or skill surface via `gal init`.

Runtime-specific install paths, generated files, rule shims, and MCP details are documented in `docs/devguide.md`, not in this file.

Codex note: installed GAL skills are available as Codex skills, but explicit invocation uses `$skill-name`, not custom slash-command syntax. Example: use `$gal status`, not `/gal status`.

**Key principles:**

- `/gal` owns the control plane
- planning remains a native command family
- execution-stage specialist work is agent-owned, not command-owned
- `gal init` discovers commands from `commands/*/SKILL.template.md` and bakes `SKILL.md` outputs
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
| `/gal wrap-up` | How do I close this session? | Converge handoff updates and update per-plan session continuity |
| `/gal finalize` | How do I land and close a completed plan? | Gated completion landing — holistic review, (worktree) merge-to-main + teardown, release/doc-sync, delegated lifecycle close |
| `/gal research` | I need structured investigation | Invoke research golem via script |
| `/gal deep-research` | I need multi-source investigation with cross-review | Invoke deep-research workflow via script |
| `/gal <golem-name>` | Route to a supported specialist agent | Invoke a dispatcher-supported golem such as `architect`, `analyst`, `designer`, `researcher`, `debugger`, `implementer`, `tester`, or `auditor` |

### `gal-*` — Discoverability Aliases

| Alias | Status | Purpose |
| --- | --- | --- |
| `/gal-init` | Active | Bootstrap `.dev/` for a repo |
| `/gal-status` | Active | Full state projection |
| `/gal-whats-next` | Active | Next-action recommendation |
| `/gal-wrap-up` | Active | Session close-out |
| `/gal-finalize` | Active | Plan completion landing + lifecycle close |
| `/gal-pipeline` | Active | Task execution through implementer -> correctness gate -> tester -> auditor, source/prompt/state task-closeout convergence, then orchestrator goal-backward verification |

For Codex CLI, use the equivalent skill names with `$` invocation.

## Planning Commands

| Command | Purpose | Primary write-back |
| --- | --- | --- |
| `/planning` | Create a source plan from a request | `.dev/plans/<plan-slug>.md` |
| `/deep-planning` | Refine a source plan until it is implementation-ready | `.dev/plans/<plan-slug>.md` |
| `/plan-to-prompt` | Generate the execution prompt from the reviewed, human-approved source plan | `.dev/plans/<plan-slug>.prompt.md` |
| `/refining-plan` | Populate source-plan `## Tasks`, `## Test Plan`, and `## Review Results > ### Engineering Review`; emit `<!-- ENG_REVIEW: CLEAR -->` when the plan passes | `.dev/plans/<plan-slug>.md` |

## Agent-Owned Execution Surface

The following work no longer has a public slash command and should be routed to the named golem instead.

| Execution work | Owning agent | Primary write-back |
| --- | --- | --- |
| Pipeline task closeout | `/gal-pipeline` orchestrator | source plan `## Tasks`, execution prompt `## Status` / `## Tasks`, matching `.dev/state.md` session continuity row |
| Spec-driven tests and real-browser QA | `golem-tester` | execution prompt `## Test Results` |
| Staff audit and drift analysis | `golem-auditor` | execution prompt `## Review Results`, `## Analyze` |
| Root-cause-first debugging | `golem-debugger` | plan debug log or `.dev/state.md` |
| Design system, variants, build, and live audit | `golem-designer` | `DESIGN.md`, `docs/designs/`, plan review sections |
| Security and deep-performance audit | `golem-auditor` | plan `## Review Results` |
| Release-flow design advice (planning stage) | `golem-releaser` | design advice → `/planning` generates a `release-<slug>` plan |

Ownership does not imply that every specialist currently has a dispatcher entry through `/gal <golem-name>`. The dispatcher scripts are the source of truth for which golems can be invoked directly.

### Golem Classification

| Golem | Class | Default Mode |
| --- | --- | --- |
| golem-implementer | Pipeline | consult when invoked directly; `bound` when dispatcher emits `DISPATCH_KIND: pipeline-phase` |
| golem-tester | Pipeline | consult when invoked directly; `bound` when dispatcher emits `DISPATCH_KIND: pipeline-phase` |
| golem-auditor | Pipeline | consult when invoked directly; `bound` when dispatcher emits `DISPATCH_KIND: pipeline-phase` |
| golem-architect | Domain | consult |
| golem-analyst | Domain | consult |
| golem-designer | Domain | consult |
| golem-researcher | Domain | consult |
| golem-releaser | Consult | consult (planning designer) |
| golem-steward | Domain | consult (`/gal steward`) |
| golem-debugger | Utility | utility |

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
TASK_SCOPE: <T-NN>
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
| `commands/gal-finalize/SKILL.template.md` | Alias template for `/gal-finalize` |
| `commands/gal-pipeline/SKILL.template.md` | Alias template for `/gal-pipeline` |
| `commands/planning/SKILL.template.md` | Planning source template |
| `commands/deep-planning/SKILL.template.md` | Deep-planning source template |
| `commands/plan-to-prompt/SKILL.template.md` | Prompt generation source template |
| `commands/refining-plan/SKILL.template.md` | Engineering-review contract source template |

## Installation

The `gal` binary is delivered by a package manager (`cargo install --git` / winget / Homebrew / curl); see `docs/manual.md`. The repo-local command surface is generated by `gal init`, which discovers commands from `commands/*/SKILL.template.md` and renders the repo-local adapter files. The bake step replaces `{{GAL_ROOT}}` with the absolute path and appends any gitignored `commands/*/SKILL.local.md` overlay before writing the baked `SKILL.md` output.

Machine-level projection of commands and skills into the other coding-agent runtime surfaces is no longer part of the `gal` workflow product.

For the current runtime topology, see `docs/devguide.md`.

If you need a machine-local customization that should survive `gal init` reruns, put it in `commands/<command>/SKILL.local.md`. Do not edit `commands/<command>/SKILL.md` directly.
