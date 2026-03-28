# Commands

GAL slash commands use a canonical `/gal` dispatcher plus lightweight `gal-*` aliases for autocomplete discoverability.

See [docs/command-dispatch-architecture.md](../docs/command-dispatch-architecture.md) for the architectural rationale and dispatch contract.

## Architecture

A small command-skill set is baked from templates under `commands/` and installed into both Copilot and Gemini skill directories.

**Four runtime layers:**

| Layer | Path | Content |
| --- | --- | --- |
| Canonical dispatcher | `~/.copilot/skills/gal/SKILL.md`, `~/.gemini/skills/gal/SKILL.md` | Main entry point — generated from `commands/gal/SKILL.template.md` |
| Discoverability aliases | `~/.copilot/skills/gal-*/SKILL.md`, `~/.gemini/skills/gal-*/SKILL.md` | Alias entries — generated from `commands/gal-*/SKILL.template.md` |
| GAL references | `~/.copilot/gal/`, `~/.gemini/gal/` → repo symlink | templates/, workflows/, conventions/, agent/ |
| Workspace context | `.dev/project.md`, `.dev/state.md` | Per-repo state |

**Key principles:**

- **Canonical entry point** — `/gal` is the main interface. It handles subcommands, golem routing, and state-aware dispatch.
- **Discoverability aliases** — `/gal-init`, `/gal-plan`, `/gal-status`, `/gal-next`, `/gal-pause` exist so typing `/gal-` exposes common actions in slash-command autocomplete.
- **Script-driven dispatch** — `gal dispatch` emits a structured `--- GAL DISPATCH ---` block. The AI follows that output deterministically.
- **Baked absolute paths** — `Setup-Machine.ps1` and `setup-machine.sh` replace `{{GAL_ROOT}}` with the absolute repo path before installation.
- **One repo symlink** — `~/.copilot/gal/` and `~/.gemini/gal/` point to the GAL repo root, so all conventions, workflows, and templates remain available.

## Command Surface

### `/gal` — Canonical Dispatcher

```sh
/gal [subcommand | golem-name | free text]
```

| Invocation | Behaviour |
| --- | --- |
| `/gal` | Auto-detect from `.dev/state.md` and route to the bound golem |
| `/gal init` | Initialize `.dev/` for a repo — scaffold `project.md` + `state.md` |
| `/gal plan` | Create a plan scaffold from template |
| `/gal status` | Show current workflow state |
| `/gal next` | Show next step for session resumption |
| `/gal pause` | Commit context for session handoff |
| `/gal sync` | Generate repo-local Copilot and Gemini adapters from `.dev/project.md` |
| `/gal golem-architect` | Consult architect in domain/consult mode |
| `/gal golem-debugger` | Invoke debugger in utility mode |
| `/gal <any golem name>` | Invoke that golem directly |

### `gal-*` — Discoverability Aliases

These aliases exist only for slash-command discoverability. They all route back through `gal dispatch`.

| Alias | Equivalent canonical call |
| --- | --- |
| `/gal-init` | `/gal init` |
| `/gal-plan` | `/gal plan` |
| `/gal-status` | `/gal status` |
| `/gal-next` | `/gal next` |
| `/gal-pause` | `/gal pause` |

### Dispatch Output Protocol

The CLI emits a block that the AI reads and executes:

```text
--- GAL DISPATCH ---
COMMAND: <init|plan|status|next|pause|error|suggest>
ROLE: <golem-name>           # mutually exclusive with COMMAND
MODE: <bound|consult|utility>  # required when ROLE present
READ: <file-path>            # optional, may appear multiple times
ACTION: <instruction text>
ON_COMPLETE: <next-step hint>
--- END DISPATCH ---
```

### Golem Classification

| Golem | Class | Bound State | Default Mode |
| --- | --- | --- | --- |
| golem-planner | Workflow | PLAN | bound / consult |
| golem-implementer | Workflow | IMPLEMENT | bound / consult |
| golem-tester | Workflow | TEST | bound / consult |
| golem-reviewer | Workflow | REVIEW | bound / consult |
| golem-verifier | Workflow | VERIFY | bound / consult |
| golem-architect | Domain | — | consult |
| golem-analyst | Domain | — | consult |
| golem-librarian | Domain | — | consult |
| golem-debugger | Utility | any | utility |
| golem-scribe | Utility | any | utility |

Workflow golems use `bound` mode when the current state matches their bound state, `consult` otherwise. The dispatcher also accepts legacy `CROSS_REVIEW` state values for reviewer compatibility.

## Source Files

| File | Purpose |
| --- | --- |
| `commands/gal/SKILL.template.md` | Canonical dispatcher template |
| `commands/gal-init/SKILL.template.md` | Alias template for `/gal-init` |
| `commands/gal-plan/SKILL.template.md` | Alias template for `/gal-plan` |
| `commands/gal-status/SKILL.template.md` | Alias template for `/gal-status` |
| `commands/gal-next/SKILL.template.md` | Alias template for `/gal-next` |
| `commands/gal-pause/SKILL.template.md` | Alias template for `/gal-pause` |

## Installation

Managed by `Setup-Machine.ps1` / `setup-machine.sh`. The scripts:

1. Create `~/.copilot/gal/` and `~/.gemini/gal/` → GAL repo root symlinks.
2. Read `commands/gal/SKILL.template.md` and `commands/gal-*/SKILL.template.md`, replacing `{{GAL_ROOT}}` with the absolute path.
3. Write baked `SKILL.md` files into each `commands/gal*/` directory.
4. Symlink those command directories into `~/.copilot/skills/` and `~/.gemini/skills/`.
5. Remove only stale legacy `gal-*` skill directories that are not part of the active alias set.
