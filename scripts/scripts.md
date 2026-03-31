# Scripts

Machine setup and adapter sync scripts.

| Script | Platform | Purpose |
| --- | --- | --- |
| `gal.ps1` | Windows | `gal <subcommand>` dispatcher |
| `gal.sh` | macOS | `gal <subcommand>` dispatcher |
| `gal-smudge.sh` | cross-platform | Git smudge filter — replaces `<PLACEHOLDER>` with values from `config.local.env` |
| `gal-clean.sh` | cross-platform | Git clean filter — restores `<PLACEHOLDER>` tokens on commit |
| `Init-Repo.ps1` | Windows | Initialize `<repo>/.dev/` + `docs/plans/` |
| `init-repo.sh` | macOS | Same for Mac |
| `Sync-DevContext.ps1` | Windows | Generate `.github/copilot-instructions.md` + `GEMINI.md` from `.dev/project.md` |
| `sync-dev-context.sh` | macOS | Same for Mac |
| `Setup-Machine.ps1` | Windows | Symlink agents/ + skills/ → ~/.copilot/ + ~/.gemini/, bake command skills, generate `gal-context.md` |
| `setup-machine.sh` | macOS | Same for Mac |
| `Uninstall-Machine.ps1` | Windows | Remove all GAL symlinks + baked command skills + `gal-context.md` |
| `uninstall-machine.sh` | macOS | Same for Mac |

## Command Surface

The portable shell entrypoint is `gal <subcommand>`.
AI slash commands map onto the same subcommands, with `/gal` as the canonical entry.

| Command | Purpose |
| --- | --- |
| `/gal init` | Initialize `.dev/project.md`, `.dev/state.md`, and `docs/plans/` |
| `/gal status` | Show current workflow state, active plan, review and test status, blockers, and specialist readiness |
| `/gal whats-next` | Determine the next step from current plan status, review results, and QA readiness |
| `/gal wrap-up` | Close the session cleanly — converge handoff artifacts, update `.dev/state.md`, prompt for commit |
| `/gal research` | Enter structured investigation mode |

Alias slash commands:

| Command | Purpose |
| --- | --- |
| `/gal-init` | Alias for `/gal init` |
| `/gal-status` | Alias for `/gal status` |
| `/gal-whats-next` | Alias for `/gal whats-next` |
| `/gal-wrap-up` | Alias for `/gal wrap-up` |

Shell usage examples:

```powershell
.\scripts\gal.ps1 init
.\scripts\gal.ps1 status
.\scripts\gal.ps1 dispatch
.\scripts\gal.ps1 dispatch golem-planner
```

```bash
./scripts/gal.sh init
./scripts/gal.sh status
./scripts/gal.sh dispatch
./scripts/gal.sh dispatch golem-planner
```

### Init-Repo: Adopt-Existing Mode

By default, `gal init` scans the target repo for existing documentation:

1. Finds README variants, `docs/` files, ADR directories
2. Detects tech stack from config files (`.csproj`, `package.json`, `go.mod`, etc.)
3. Pre-populates `.dev/project.md` Source Documents table and Tech Stack field
4. User/AI completes the summary by reviewing discovered docs

Use `--Blank` (PowerShell) or `--blank` (bash) to skip scanning and use a blank template.

## Setup-Machine Symlinks

The setup script creates these symlinks:

| Source (repo) | Copilot Target | Gemini Target |
| --- | --- | --- |
| `agent/*.agent.md` | `~/.copilot/agents/` | — |
| `skills/*/` | `~/.copilot/skills/` | `~/.gemini/skills/` |
| `commands/gal/` | `~/.copilot/skills/gal/` | `~/.gemini/skills/gal/` |
| `commands/gal-init/` | `~/.copilot/skills/gal-init/` | `~/.gemini/skills/gal-init/` |
| `commands/gal-status/` | `~/.copilot/skills/gal-status/` | `~/.gemini/skills/gal-status/` |
| `commands/gal-whats-next/` | `~/.copilot/skills/gal-whats-next/` | `~/.gemini/skills/gal-whats-next/` |
| `commands/gal-wrap-up/` | `~/.copilot/skills/gal-wrap-up/` | `~/.gemini/skills/gal-wrap-up/` |
| `commands/<specialist>/` | `~/.copilot/skills/<specialist>/` | `~/.gemini/skills/<specialist>/` |
| `<repo root>` | `~/.copilot/gal/` | `~/.gemini/gal/` |

All `commands/` subdirectories are picked up dynamically — adding a new command folder is sufficient.

Additionally **generates** each `commands/*/SKILL.md` by baking `SKILL.template.md` (replacing `{{GAL_ROOT}}` with the absolute repo path), then symlinks those command directories into both skill targets.

Generates `~/.gemini/gal-context.md` with all GAL command skills first, then sorted `@file` skill imports.

See [docs/installation-topology.md](../docs/installation-topology.md) for the architecture-level explanation behind this runtime layout.
