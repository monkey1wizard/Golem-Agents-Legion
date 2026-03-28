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
AI slash commands map onto the same subcommands, with `/gal` as the canonical entry and `gal-*` aliases for autocomplete discoverability.

| Command | Purpose |
| --- | --- |
| `/gal init` | Initialize `.dev/project.md`, `.dev/state.md`, and `docs/plans/` |
| `/gal plan [-Type <type>] <name>` | Create a draft plan scaffold (type defaults to `feat`) |
| `/gal status` | Read current workflow state |
| `/gal next` | Show the next recorded step |
| `/gal pause` | Commit `.dev/` and `docs/plans/` for worktree context handoff |
| `/gal sync` | Generate `.github/copilot-instructions.md` and `GEMINI.md` from `.dev/project.md` |
| `/gal dispatch [golem\|subcommand]` | Resolve and invoke the correct golem for current state, or explicitly target a golem |

Alias slash commands:

| Command | Purpose |
| --- | --- |
| `/gal-init` | Alias for `/gal init` |
| `/gal-plan` | Alias for `/gal plan` |
| `/gal-status` | Alias for `/gal status` |
| `/gal-next` | Alias for `/gal next` |
| `/gal-pause` | Alias for `/gal pause` |

Shell usage examples:

```powershell
.\scripts\gal.ps1 init
.\scripts\gal.ps1 plan "refactor order pipeline"
.\scripts\gal.ps1 plan -Type fix "null ref in parser"
.\scripts\gal.ps1 status
.\scripts\gal.ps1 pause
.\scripts\gal.ps1 sync
.\scripts\gal.ps1 dispatch
.\scripts\gal.ps1 dispatch golem-planner
```

```bash
./scripts/gal.sh init
./scripts/gal.sh plan "refactor order pipeline"
./scripts/gal.sh plan -t fix "null ref in parser"
./scripts/gal.sh status
./scripts/gal.sh pause
./scripts/gal.sh sync
./scripts/gal.sh dispatch
./scripts/gal.sh dispatch golem-planner
```

### Init-Repo: Adopt-Existing Mode

By default, `gal init` scans the target repo for existing documentation:

1. Finds README variants, `docs/` files, ADR directories
2. Detects tech stack from config files (`.csproj`, `package.json`, `go.mod`, etc.)
3. Pre-populates `.dev/project.md` Source Documents table and Tech Stack field
4. User/AI completes the summary by reviewing discovered docs
5. User curates `## Active Skills`, then runs `/gal sync` to generate repo-local adapters

Use `--Blank` (PowerShell) or `--blank` (bash) to skip scanning and use a blank template.

## Setup-Machine Symlinks

The setup script creates these symlinks:

| Source (repo) | Copilot Target | Gemini Target |
| --- | --- | --- |
| `agent/*.agent.md` | `~/.copilot/agents/` | — |
| `skills/*/` | `~/.copilot/skills/` | `~/.gemini/skills/` |
| `commands/gal/` | `~/.copilot/skills/gal/` | `~/.gemini/skills/gal/` |
| `commands/gal-init/` | `~/.copilot/skills/gal-init/` | `~/.gemini/skills/gal-init/` |
| `commands/gal-plan/` | `~/.copilot/skills/gal-plan/` | `~/.gemini/skills/gal-plan/` |
| `commands/gal-status/` | `~/.copilot/skills/gal-status/` | `~/.gemini/skills/gal-status/` |
| `commands/gal-next/` | `~/.copilot/skills/gal-next/` | `~/.gemini/skills/gal-next/` |
| `commands/gal-pause/` | `~/.copilot/skills/gal-pause/` | `~/.gemini/skills/gal-pause/` |
| `<repo root>` | `~/.copilot/gal/` | `~/.gemini/gal/` |

Additionally **generates** `commands/gal/SKILL.md` and `commands/gal-*/SKILL.md` by baking each `SKILL.template.md` (replacing `{{GAL_ROOT}}` with the absolute repo path), then symlinks those command directories into both skill targets so Gemini CLI discovers them as proper `ReparsePoint` directories.

Generates `~/.gemini/gal-context.md` with all GAL command skills first, then sorted `@file` skill imports.

See [docs/installation-topology.md](../docs/installation-topology.md) for the architecture-level explanation behind this runtime layout.

## Sync-DevContext Flow

`gal sync` is the manual-first adapter generation step. It validates `## Active Skills` in `.dev/project.md` and fails fast if the list is missing, empty, duplicated, or references unknown skills.

```text
.dev/project.md (Active Skills field)
       │
       ├──→ Read conventions/*.md
       ├──→ Read workflows/coding.md
       ├──→ Read model-roles.md
       ├──→ Validate listed skills exist under skills/*/SKILL.md
       │
       └──→ Generate:
            ├── .github/copilot-instructions.md (project context + shared GAL docs; no skill bodies)
            └── GEMINI.md (same shared base + listed skill bodies inlined)
```

`gal init` does not infer skills. The target repo owner or AI session must curate `## Active Skills` manually before Sync can succeed.

See [docs/per-repo-context.md](../docs/per-repo-context.md) for the working-memory model behind this flow.
