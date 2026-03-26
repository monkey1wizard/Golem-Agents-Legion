# Scripts

Machine setup and adapter sync scripts.

| Script | Platform | Purpose |
|--------|----------|---------|
| `gal.ps1` | Windows | `gal <subcommand>` dispatcher |
| `gal.sh` | macOS | `gal <subcommand>` dispatcher |
| `Init-Repo.ps1` | Windows | Initialize `<repo>/.dev/` + `docs/plans/` |
| `init-repo.sh` | macOS | Same for Mac |
| `Setup-Machine.ps1` | Windows | Symlink agents/ + skills/ → ~/.copilot/ |
| `setup-machine.sh` | macOS | Same for Mac |
| `Sync-DevContext.ps1` | Windows | Generate adapter files from .dev/project.md |
| `sync-dev-context.sh` | macOS | Same for Mac |

## Command Surface

The portable shell entrypoint is `gal <subcommand>`.
AI-specific slash commands should map onto the same subcommands.

| Command | Purpose |
|--------|---------|
| `/gal init` | Initialize `.dev/project.md`, `.dev/state.md`, and `docs/plans/` |
| `/gal plan <name>` | Create a draft plan scaffold |
| `/gal status` | Read current workflow state |
| `/gal next` | Show the next recorded step |
| `/gal sync` | Generate tool-specific adapter files |
| `/gal ask <agent>` | Direct consult with a golem without changing workflow state |
| `/gal run <agent>` | Invoke utility-style golems |

Shell usage examples:

```powershell
.\scripts\gal.ps1 init
.\scripts\gal.ps1 plan "refactor order pipeline"
.\scripts\gal.ps1 status
```

```bash
./scripts/gal.sh init
./scripts/gal.sh plan "refactor order pipeline"
./scripts/gal.sh status
```

## Setup-Machine Symlinks

The setup script creates these symlinks:

| Source (repo) | Target (runtime) |
|--------------|------------------|
| `agent/*.agent.md` | `~/.copilot/agents/` |
| `skills/*/` | `~/.copilot/skills/` |

## Sync-DevContext Flow

```text
.dev/project.md (Active Skills field)
       │
       ├──→ Read selected skill content from conventions/
       ├──→ Read workflow.md state machine
       ├──→ Read model-roles.md routing table
       │
       └──→ Generate:
            ├── .github/copilot-instructions.md (for Copilot)
            ├── GEMINI.md (for Gemini CLI, skills inlined)
            ├── CLAUDE.md (for Claude Code, skills inlined)
            ├── AGENTS.md (for OmO/OpenCode)
            └── .cursorrules (for Cursor)
```
