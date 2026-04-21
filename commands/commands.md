# Commands

GAL now exposes a deliberately small public command surface: 9 commands total.

- Control plane: `/gal`, `/gal-init`, `/gal-status`, `/gal-whats-next`, `/gal-wrap-up`, `/gal-pipeline`
- Planning: `/planning`, `/deep-planning`, `/plan-to-prompt`

Everything else that used to live behind execution-stage slash commands is now owned by golem agents.

## Architecture

Command skills are installed into Copilot and Codex skill directories, while Gemini native slash commands are generated into `~/.gemini/commands/` via `Setup-Machine`. Shared reusable non-command skills remain in `.agents/skills`.

Codex note: installed GAL skills are available as Codex skills, but explicit invocation uses `$skill-name`, not custom slash-command syntax. Example: use `$gal status`, not `/gal status`.

**Four runtime layers:**

| Layer | Path | Content |
| --- | --- | --- |
| Installed dispatcher entry | `~/.copilot/skills/gal/SKILL.md`, `~/.gemini/commands/gal.toml`, `~/.codex/skills/gal/SKILL.md` | Main entry point — generated from `commands/gal/SKILL.template.md` |
| Discoverability aliases | `~/.copilot/skills/gal-*/SKILL.md`, `~/.gemini/commands/gal-*.toml`, `~/.codex/skills/gal-*/SKILL.md` | Alias entries — generated from `commands/gal-*/SKILL.template.md` |
| GAL references | `~/.copilot/gal/`, `~/.gemini/gal/` -> repo symlink | templates/, workflows/, conventions/, agent/ |
| Workspace context | `.dev/project.md`, `.dev/state.md` | Per-repo state |

**Key principles:**

- `/gal` owns the control plane
- planning remains a native command family
- execution-stage specialist work is agent-owned, not command-owned
- `Setup-Machine` still discovers installed commands from `commands/*/SKILL.template.md`
- cross-runtime behavior must stay consistent across Copilot, Gemini, and Codex

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
| `/gal <golem-name>` | Route to a specialist agent | Invoke that golem via script |

### `gal-*` — Discoverability Aliases

| Alias | Status | Purpose |
| --- | --- | --- |
| `/gal-init` | Active | Bootstrap `.dev/` for a repo |
| `/gal-status` | Active | Full state projection |
| `/gal-whats-next` | Active | Next-action recommendation |
| `/gal-wrap-up` | Active | Session close-out |
| `/gal-pipeline` | Active | Task execution through implementer -> tester -> reviewer -> verifier |

For Codex CLI, use the equivalent skill names with `$` invocation.

## Planning Commands

| Command | Purpose | Primary write-back |
| --- | --- | --- |
| `/planning` | Create a source plan from a request | `docs/plans/<plan-slug>.md` |
| `/deep-planning` | Refine a source plan until it is implementation-ready | `docs/plans/<plan-slug>.md` |
| `/plan-to-prompt` | Materialize the execution prompt from the reviewed source plan | `.dev/plans/<plan-slug>.prompt.md` |

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

### Golem Classification

| Golem | Class | Default Mode |
| --- | --- | --- |
| golem-implementer | Pipeline | consult |
| golem-tester | Pipeline | consult |
| golem-reviewer | Pipeline | consult |
| golem-verifier | Pipeline | consult |
| golem-architect | Domain | consult |
| golem-analyst | Domain | consult |
| golem-designer | Domain | consult |
| golem-researcher | Domain | consult |
| golem-security | Domain | consult |
| golem-releaser | Domain | consult |
| golem-debugger | Utility | utility |
| golem-notewriter | Utility | utility |

## Planning-Stage Review Lanes

GAL keeps planning as a native command family and routes planning-stage review by lane, not by legacy command name.

| Lane | Preferred Provider | Fallback Invocation | Write-Back Target |
| --- | --- | --- | --- |
| Business / Scope review | Upstream gstack business review provider when installed | `/gal golem-analyst` | `## Review Results` + `## Open Questions` |
| Design review | Upstream gstack design review provider when installed | `/gal golem-designer` | `## Review Results` + `## Open Questions` |
| Engineering review | Upstream gstack engineering review provider when installed | `/gal golem-architect` | `## Review Results` + `## Open Questions` + `## Test Plan` + `## Tasks` + `<!-- ENG_REVIEW: CLEAR -->` |

Legacy gstack planning command names are not part of GAL's public command catalog. They survive only as provider semantics in integration contracts.

## Dispatch Output Protocol

For script-dispatched subcommands (`init`, `research`, `deep-research`, golem names), the CLI emits a bounded block that the AI reads and executes:

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
| `commands/plan-to-prompt/SKILL.template.md` | Prompt materialization source template |

## Installation

Managed by `Setup-Machine.ps1` and `setup-machine.sh`. The scripts:

1. Create `~/.copilot/gal/` and `~/.gemini/gal/` -> GAL repo root symlinks.
2. Scan all remaining `commands/*/SKILL.template.md` files, replacing `{{GAL_ROOT}}` with the absolute path.
3. Append any gitignored `commands/*/SKILL.local.md` overlay to the baked content.
4. Write baked `SKILL.md` files into each remaining `commands/*/` directory.
5. Symlink command directories into `~/.copilot/skills/` and `~/.codex/skills/`.
6. Generate Gemini native command files in `~/.gemini/commands/` from the baked `SKILL.md` content.

If you need a machine-local customization that should survive setup reruns, put it in `commands/<command>/SKILL.local.md`. Do not edit `commands/<command>/SKILL.md` directly.
