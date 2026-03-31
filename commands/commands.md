# Commands

GAL slash commands use a canonical `/gal` control-plane entry point plus `gal-*` aliases for autocomplete discoverability.

See [docs/command-dispatch-architecture.md](../docs/command-dispatch-architecture.md) for the architectural rationale and dispatch contract.
See [docs/gal-control-plane-contracts.md](../docs/gal-control-plane-contracts.md) for the canonical command definitions, read/write contracts, and migration rules.

## Architecture

Command skills are installed into Copilot, Gemini, and Codex skill directories via `Setup-Machine`.

Codex note: installed GAL skills are available as Codex skills, but Codex explicit invocation uses `/skills` or `$skill-name`, not custom `/slash-command` syntax. Example: use `$gal status`, not `/gal status`.

**Four runtime layers:**

| Layer | Path | Content |
| --- | --- | --- |
| Canonical dispatcher | `~/.copilot/skills/gal/SKILL.md`, `~/.gemini/skills/gal/SKILL.md`, `~/.agents/skills/gal/SKILL.md` | Main entry point — generated from `commands/gal/SKILL.template.md` |
| Discoverability aliases | `~/.copilot/skills/gal-*/SKILL.md`, `~/.gemini/skills/gal-*/SKILL.md`, `~/.agents/skills/gal-*/SKILL.md` | Alias entries — generated from `commands/gal-*/SKILL.template.md` |
| GAL references | `~/.copilot/gal/`, `~/.gemini/gal/` → repo symlink | templates/, workflows/, conventions/, agent/ |
| Workspace context | `.dev/project.md`, `.dev/state.md` | Per-repo state |

**Key principles:**

- **Canonical control plane** — `/gal` is the main interface. It routes subcommands, consults golems, and handles state-aware dispatch.
- **Substantive alias skills** — `/gal-status`, `/gal-whats-next`, and `/gal-wrap-up` contain full procedures and do not dispatch through the script.
- **Script-dispatched subcommands** — `init` and `research` still route through `gal.ps1 dispatch`.
- **Discoverability aliases** — `/gal-init`, `/gal-status`, `/gal-whats-next`, `/gal-wrap-up` exist so typing `/gal-` exposes controls in slash-command autocomplete.
- **Baked absolute paths** — `Setup-Machine.ps1` and `setup-machine.sh` replace `{{GAL_ROOT}}` with the absolute repo path before installation.
- **Runtime-agnostic procedures** — shared command templates refer to installed skill names rather than a Copilot-only path.
- **One repo symlink** — `~/.copilot/gal/` and `~/.gemini/gal/` point to the GAL repo root.

## Command Surface

### `/gal` — Control Plane Entry Point

```sh
/gal [subcommand | golem-name | free text]
```

| Invocation | What It Answers | Behaviour |
| --- | --- | --- |
| `/gal` | What should I do? | Auto-detect from `.dev/state.md` — follow `/gal-whats-next` procedure |
| `/gal init` | How do I bootstrap this repo? | Scaffold `.dev/project.md` + `.dev/state.md` via script |
| `/gal status` | Where are we right now? | Full state projection — active plans, review/test/blockers/continuity |
| `/gal whats-next` | What do I do next? | Read state and recommend single next action or command |
| `/gal wrap-up` | How do I close this session? | Converge handoff artifacts and update session continuity |
| `/gal research` | I need structured investigation | Invoke research golem via script |
| `/gal <golem-name>` | Consult a specific golem | Invoke that golem via script |

### `gal-*` — Discoverability Aliases

These aliases exist for slash-command autocomplete discoverability in command surfaces that support custom slash commands.

| Alias | Status | Purpose |
| --- | --- | --- |
| `/gal-init` | Active | Bootstrap `.dev/` for a repo |
| `/gal-status` | Active | Full state projection (substantive skill, no script) |
| `/gal-whats-next` | Active | Next-action recommendation (substantive skill, no script) |
| `/gal-wrap-up` | Active | Session close-out (substantive skill, no script) |

For Codex CLI, use the equivalent skill names with `$` invocation:

| Codex Skill | Equivalent GAL Command |
| --- | --- |
| `$gal` | `/gal ...` |
| `$gal-init` | `/gal init` |
| `$gal-status` | `/gal status` |
| `$gal-whats-next` | `/gal whats-next` |
| `$gal-wrap-up` | `/gal wrap-up` |

### Dispatch Output Protocol

For script-dispatched subcommands (`init`, `research`, golem names), the CLI emits a block that the AI reads and executes:

```text
--- GAL DISPATCH ---
COMMAND: <init|error|suggest>
ROLE: <golem-name>           # mutually exclusive with COMMAND
MODE: <bound|consult|utility>  # required when ROLE present
READ: <file-path>            # optional, may appear multiple times
ACTION: <instruction text>
ON_COMPLETE: <next-step hint>
--- END DISPATCH ---
```

### Golem Classification

| Golem | Class | Default Mode |
| --- | --- | --- |
| golem-planner | Workflow | bound / consult |
| golem-implementer | Workflow | bound / consult |
| golem-tester | Workflow | bound / consult |
| golem-reviewer | Workflow | bound / consult |
| golem-verifier | Workflow | bound / consult |
| golem-architect | Domain | consult |
| golem-analyst | Domain | consult |
| golem-designer | Domain | consult |
| golem-researcher | Domain | consult |
| golem-librarian | Domain | consult |
| golem-debugger | Utility | utility |
| golem-scribe | Utility | utility |

## Source Files

| File | Purpose |
| --- | --- |
| `commands/gal/SKILL.template.md` | Canonical dispatcher template |
| `commands/gal-init/SKILL.template.md` | Alias template for `/gal-init` |
| `commands/gal-status/SKILL.template.md` | Alias template for `/gal-status` |
| `commands/gal-whats-next/SKILL.template.md` | Alias template for `/gal-whats-next` |
| `commands/gal-wrap-up/SKILL.template.md` | Alias template for `/gal-wrap-up` |
| `commands/<specialist>/SKILL.template.md` | Specialist command templates (one per command) |

## Installation

Managed by `Setup-Machine.ps1` / `setup-machine.sh`. The scripts:

1. Create `~/.copilot/gal/` and `~/.gemini/gal/` → GAL repo root symlinks.
2. Scan all `commands/*/SKILL.template.md` files, replacing `{{GAL_ROOT}}` with the absolute path.
3. Write baked `SKILL.md` files into each `commands/*/` directory.
4. Symlink all command directories into `~/.copilot/skills/`, `~/.gemini/skills/`, and `~/.agents/skills/`.
