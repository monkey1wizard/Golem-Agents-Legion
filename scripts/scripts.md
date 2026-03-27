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
| `Setup-Machine.ps1` | Windows | Symlink agents/ + skills/ → ~/.copilot/ + ~/.gemini/, generate gal-context.md |
| `setup-machine.sh` | macOS | Same for Mac |
| `Uninstall-Machine.ps1` | Windows | Remove all GAL symlinks + gal-context.md (wrapper for `Setup-Machine.ps1 -Uninstall`) |
| `uninstall-machine.sh` | macOS | Same for Mac |
| `Sync-DevContext.ps1` | Windows | Generate adapter files from .dev/project.md |
| `sync-dev-context.sh` | macOS | Same for Mac |

## Command Surface

The portable shell entrypoint is `gal <subcommand>`.
AI-specific slash commands should map onto the same subcommands.

| Command | Purpose |
| --- | --- |
| `/gal init` | Initialize `.dev/project.md`, `.dev/state.md`, and `docs/plans/` (adopt-existing by default) |
| `/gal plan [-Type <type>] <name>` | Create a draft plan scaffold (type defaults to `feat`) |
| `/gal status` | Read current workflow state |
| `/gal next` | Show the next recorded step |
| `/gal pause` | Commit `.dev/` and `docs/plans/` for worktree context handoff |
| `/gal sync` | Generate tool-specific adapter files |
| `/gal dispatch [golem\|subcommand]` | Resolve and invoke the correct golem for current state, or explicitly target a golem |

Shell usage examples:

```powershell
.\scripts\gal.ps1 init
.\scripts\gal.ps1 plan "refactor order pipeline"
.\scripts\gal.ps1 plan -Type fix "null ref in parser"
.\scripts\gal.ps1 status
.\scripts\gal.ps1 pause
.\scripts\gal.ps1 dispatch              # auto-detect correct golem from state.md
.\scripts\gal.ps1 dispatch golem-planner # explicitly target a golem
```

```bash
./scripts/gal.sh init
./scripts/gal.sh plan "refactor order pipeline"
./scripts/gal.sh plan -t fix "null ref in parser"
./scripts/gal.sh status
./scripts/gal.sh pause
./scripts/gal.sh dispatch              # auto-detect correct golem from state.md
./scripts/gal.sh dispatch golem-planner # explicitly target a golem
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
| `<repo root>` | `~/.copilot/gal/` | `~/.gemini/gal/` |

Additionally **generates** `commands/gal/SKILL.md` by baking `SKILL.template.md` (replaces `{{GAL_ROOT}}` with the absolute repo path), then symlinks `commands/gal/` into both skill targets so Gemini CLI discovers it as a proper `ReparsePoint` directory.

Generates `~/.gemini/gal-context.md` with `/gal` dispatcher first, then sorted `@file` skill imports.

## Sync-DevContext Flow

```text
.dev/project.md (Active Skills field)
       │
       ├──→ Read selected skill content from conventions/
       ├──→ Read workflows/ state machines
       ├──→ Read model-roles.md routing table
       │
       └──→ Generate:
            ├── .github/copilot-instructions.md (for Copilot)
            └── GEMINI.md (for Gemini CLI, skills inlined)
```
