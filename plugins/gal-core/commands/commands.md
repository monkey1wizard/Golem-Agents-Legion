# Commands

GAL now exposes a deliberately small public command surface.

- Control plane: `/gal`, `/gal-status`, `/gal-whats-next`, `/gal-wrap-up`, `/gal-finalize`, `/gal-pipeline`
- Planning: `/planning`, `/deep-planning`, `/plan-to-prompt`, `/refining-plan`

Everything else that used to live behind execution-stage slash commands is now owned by golem agents.

Two of the 12 on-disk command directories sit outside this control-plane enumeration: `git-commit-msg` (a skill-command) and `steward` (a `/gal <golem-name>` directly callable route that happens to own a directory — already counted once as `/gal`, same class as `/gal research`). `gal` itself is part of the enumeration above, not excluded from it.

## Architecture

GAL keeps one shared public command contract and packages it into each supported runtime's native command or skill surface via `gal refresh`.

Runtime-specific install paths, generated files, rule shims, and MCP details are documented in `docs/devguide.md`, not in this file.

Codex note: installed GAL skills are available as Codex skills, but explicit invocation uses `$skill-name`, not custom slash-command syntax. Example: use `$gal status`, not `/gal status`.

**Key principles:**

- `/gal` owns the control plane
- planning remains a native command family
- execution-stage specialist work is agent-owned, not command-owned
- `gal refresh` discovers commands from `commands/*/SKILL.template.md` and bakes `SKILL.md` outputs
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
| `/gal <golem-name>` | Route to a supported specialist agent | Invoke a directly callable golem: `architect`, `analyst`, `designer`, `releaser`, `debugger`, or `steward` (`MODE: direct`). Only architect, analyst, designer, and releaser also support `/gal discuss <golem-name>`. Orchestrated-only golems (`implementer`, `tester`, `auditor`, `researcher`) are not directly routable here — a bare call returns `COMMAND: error`; they run only via `/gal pipeline`, `/gal finalize`, `/gal research`, or `/gal deep-research` |

### `gal-*` — Discoverability Aliases

| Alias | Status | Purpose |
| --- | --- | --- |
| `/gal-status` | Active | Full state projection |
| `/gal-whats-next` | Active | Next-action recommendation |
| `/gal-wrap-up` | Active | Session close-out |
| `/gal-finalize` | Active | Plan completion landing + lifecycle close |
| `/gal-pipeline` | Active | Task execution under `test-first-v1` (optional behavior-free `scaffold` → test authoring with `red` evidence → `implement` to `green` → orchestrator correctness gate → `audit`), source/prompt/state task-closeout convergence, then orchestrator-owned goal-backward `verify` pass |

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

### Pipeline Execution Modes (`test-first-v1`)

When an execution prompt activates `test-first-v1`, task execution follows a strict phase sequence:
1. **Scaffold Routing**: If `Scaffold: required`, CODER constructs a behavior-free stub on production paths without committing.
2. **Test Authoring (Red Evidence)**: TESTER authors acceptance probes targeting the locked public seam and records expected `red` evidence. Test paths are frozen.
3. **Implement to Green**: CODER implements behavior to satisfy exact `green` probe pass (same-command rerun).
4. **Gate & Audit**: The orchestrator correctness gate runs, followed by AUDITOR review.
5. **Convergence**: The orchestrator performs the implementation commit at HEAD and synchronizes task closeout across source plan, prompt, and `.dev/state.md`.

The goal-backward `verify` pass is orchestrator-owned, runs in-process at pipeline completion, and has no public or dispatched command surface.

### Golem Classification

Two user-facing invocability classes: **Directly callable** and **Orchestrated-only**. Within directly callable, four roles additionally support `/gal discuss <role>` (consult dual-mode); the other two are isolated-only.

| Golem | Class | Discuss? | Default Mode |
| --- | --- | --- | --- |
| golem-implementer | Orchestrated-only | no | bare call → `COMMAND: error`; `bound` only when dispatcher emits `DISPATCH_KIND: pipeline-phase` |
| golem-tester | Orchestrated-only | no | bare call → `COMMAND: error`; `bound` only when dispatcher emits `DISPATCH_KIND: pipeline-phase` |
| golem-auditor | Orchestrated-only | no | bare call → `COMMAND: error`; `bound` only when dispatcher emits `DISPATCH_KIND: pipeline-phase` (task audit) or the finalize branch-audit context (whole-branch audit) |
| golem-researcher | Orchestrated-only | no | bare call → `COMMAND: error`; only reachable via `/gal research` or `/gal deep-research` |
| golem-architect | Directly callable | yes | `direct` |
| golem-analyst | Directly callable | yes | `direct` |
| golem-designer | Directly callable | yes | `direct` |
| golem-releaser | Directly callable | yes | `direct` (planning designer) |
| golem-steward | Directly callable | no | `direct` (`/gal steward`) |
| golem-debugger | Directly callable | no | `direct` |

## Planning-Stage Review Lanes

GAL keeps planning as a native command family and routes planning-stage review by lane, not by legacy command name.

| Lane | Invocation | Write-Back Target |
| --- | --- | --- |
| Business / Scope review | `/gal golem-analyst` | `## Review Results` + `## Open Questions` |
| Design review | `/gal golem-designer` | `## Review Results` + `## Open Questions` |
| Engineering review | `/gal golem-architect` | Architect feedback feeds the source plan; `/refining-plan` remains the command that writes `## Tasks`, `## Test Plan`, and `<!-- ENG_REVIEW: CLEAR -->` |

## Dispatch Output Protocol

For script-dispatched subcommands (`init`, `research`, `deep-research`, golem names), the CLI emits a bounded block that the AI reads and executes:

```text
--- GAL DISPATCH ---
COMMAND: <init|error|suggest>
ROLE: <golem-name>
MODE: <bound|direct>
DISPATCH_KIND: <pipeline-phase>
PIPELINE_PHASE: <scaffold|implement|test|audit>
TASK_SCOPE: <T-NN>
FIX_MODE: <true>
READ: <file-path>
ACTION: <instruction text>
ON_COMPLETE: <next-step hint>
--- END DISPATCH ---
```

Accepted `PIPELINE_PHASE` dispatch values are `scaffold`, `implement`, `test`, and `audit`. The goal-backward `verify` pass is orchestrator-owned, runs in-process at pipeline completion, and has no public or dispatched command surface.

## Source Files

| File | Purpose |
| --- | --- |
| `commands/gal/SKILL.template.md` | Dispatcher source template |
| `commands/gal-status/SKILL.template.md` | Alias template for `/gal-status` |
| `commands/gal-whats-next/SKILL.template.md` | Alias template for `/gal-whats-next` |
| `commands/gal-wrap-up/SKILL.template.md` | Alias template for `/gal-wrap-up` |
| `commands/gal-finalize/SKILL.template.md` | Alias template for `/gal-finalize` |
| `commands/gal-pipeline/SKILL.template.md` | Alias template for `/gal-pipeline` |
| `commands/planning/SKILL.template.md` | Planning source template |
| `commands/deep-planning/SKILL.template.md` | Deep-planning source template |
| `commands/plan-to-prompt/SKILL.template.md` | Prompt generation source template |
| `commands/refining-plan/SKILL.template.md` | Engineering-review contract source template |
| `commands/git-commit-msg/SKILL.template.md` | Skill-command source template; outside the control-plane enumeration |
| `commands/steward/SKILL.template.md` | `/gal steward` directly callable route source template; outside the control-plane enumeration |

## Installation

The `gal` binary is delivered by a package manager (`cargo install --git` / winget / Homebrew / curl); see `docs/manual.md`. The repo-local command surface is generated by `gal refresh`, which discovers commands from `commands/*/SKILL.template.md` and bakes the repo-local command surface. The bake step replaces `{{GAL_ROOT}}` with the absolute path and appends any gitignored `commands/*/SKILL.local.md` overlay before writing the baked `SKILL.md` output.

Machine-level projection of commands and skills into the other coding-agent runtime surfaces is no longer part of the `gal` workflow product.

For the current runtime topology, see `docs/devguide.md`.

If you need a machine-local customization that should survive `gal refresh` reruns, put it in `commands/<command>/SKILL.local.md`. Do not edit `commands/<command>/SKILL.md` directly.
