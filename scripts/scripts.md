# Scripts

Machine setup and adapter sync scripts.

| Script | Platform | Purpose |
| --- | --- | --- |
| `gal.ps1` | Windows | `gal <subcommand>` dispatcher |
| `gal.sh` | macOS | `gal <subcommand>` dispatcher |
| `gal-smudge.sh` | cross-platform | Git smudge filter — replaces `<PLACEHOLDER>` with values from `config.local.env` |
| `gal-clean.sh` | cross-platform | Git clean filter — restores `<PLACEHOLDER>` tokens on commit |
| `Init-Repo.ps1` | Windows | Initialize `<repo>/.dev/` + `docs/plans/`, generate `.github/copilot-instructions.md`, `GEMINI.md`, `CLAUDE.md`, and `AGENTS.md`, then auto-run graphify when the CLI is already available and stamp the generated report with the graphify version |
| `init-repo.sh` | macOS | Same for Mac |
| `Sync-DevContext.ps1` | Windows | Generate `.github/copilot-instructions.md`, `GEMINI.md`, `CLAUDE.md`, and `AGENTS.md` from `.dev/project.md`; auto-discovers all skills in `skills/` |
| `sync-dev-context.sh` | macOS | Same for Mac |
| `Setup-Machine.ps1` | Windows | Prompt for selected runtimes + primary runtime on first run, persist `~/.gal/install-state.json`, symlink runtime targets, bake command skills with optional `SKILL.local.md` overlays, generate Gemini and Claude command files, clean stale runtime installs, generate `gal-context.md`, merge MCP config into VS Code / Gemini / Codex |
| `Setup-Tools.ps1` | Windows | Check optional collaborative tool status, ask which missing tools to install, install gstack / graphify / OpenCLI with official upstream methods, then verify GAL collaboration readiness |
| `setup-machine.sh` | macOS | Same for macOS/Linux |
| `setup-tools.sh` | macOS | Same for Mac/Linux |
| `Uninstall-Machine.ps1` | Windows | Remove all GAL symlinks + baked command skills + `gal-context.md` |
| `uninstall-machine.sh` | macOS | Same for Mac |
| `Invoke-XmachineRemoteTask.ps1` | Windows | Dispatch a task to the remote Windows xmachine lane over SSH |
| `Start-xMachine.ps1` | Windows | Run a task on the remote Windows machine in the `remote-windows` lane via Gemini CLI |
| `Get-XmachineRemoteResult.ps1` | Windows | Retrieve results from a completed remote xmachine task and clean up the worktree |
| `Test-Xmachine.ps1` | Windows | Smoke-test wrapper for the `remote-windows` xmachine lane |
| `Test-Xmachine.sh` | macOS/Linux | Smoke-test wrapper for the `local-async` xmachine lane |
| `Invoke-XmachineLocalTask.sh` | macOS/Linux | Dispatch a task into the `local-async` xmachine lane on a controlled machine |
| `Start-xMachine.sh` | macOS/Linux | Run a task on the local machine in the `local-async` lane via Gemini CLI |
| `Get-XmachineLocalResult.sh` | macOS/Linux | Retrieve results from a completed `local-async` xmachine task and optionally clean up |

## Command Surface

The portable shell entrypoint is `gal <subcommand>`.
AI slash commands map onto the same subcommands, with `/gal` as the primary entry.

| Command | Purpose |
| --- | --- |
| `/gal init` | Initialize `.dev/project.md`, `.dev/state.md`, and `docs/plans/`; if graphify is already installed, also generate `graphify-out/` and stamp the generated report with the current graphify version |
| `/gal status` | Show current plan progress, active plan, review and test status, blockers, and specialist readiness |
| `/gal whats-next` | Determine the next step from current plan status, review results, and QA readiness |
| `/gal wrap-up` | Close the session cleanly — converge handoff updates, update `.dev/state.md`, prompt for commit |
| `/gal research` | Enter structured investigation mode |
| `/gal deep-research` | Enter multi-source investigation mode with cross-review and reference verification |

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
.\scripts\gal.ps1 dispatch
.\scripts\gal.ps1 dispatch init
.\scripts\gal.ps1 dispatch golem-architect
```

```bash
./scripts/gal.sh init
./scripts/gal.sh dispatch
./scripts/gal.sh dispatch init
./scripts/gal.sh dispatch golem-architect
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

| Source (repo) | Copilot Target | Gemini / Shared Target | Codex Target |
| --- | --- | --- | --- |
| `agent/*.agent.md` | `~/.copilot/agents/` | — | — |
| `skills/*/` | `~/.copilot/skills/` | `~/.agents/skills/` | `~/.agents/skills/` |
| `commands/gal/` | `~/.copilot/skills/gal/` | `~/.gemini/commands/gal.toml` | `~/.codex/skills/gal/` |
| `commands/gal-init/` | `~/.copilot/skills/gal-init/` | `~/.gemini/commands/gal-init.toml` | `~/.codex/skills/gal-init/` |
| `commands/gal-status/` | `~/.copilot/skills/gal-status/` | `~/.gemini/commands/gal-status.toml` | `~/.codex/skills/gal-status/` |
| `commands/gal-whats-next/` | `~/.copilot/skills/gal-whats-next/` | `~/.gemini/commands/gal-whats-next.toml` | `~/.codex/skills/gal-whats-next/` |
| `commands/gal-wrap-up/` | `~/.copilot/skills/gal-wrap-up/` | `~/.gemini/commands/gal-wrap-up.toml` | `~/.codex/skills/gal-wrap-up/` |
| `commands/<specialist>/` | `~/.copilot/skills/<specialist>/` | `~/.gemini/commands/<specialist>.toml` | `~/.codex/skills/<specialist>/` |
| `<repo root>` | `~/.copilot/gal/` | `~/.gemini/gal/` (GAL_ROOT only) | — |

All `commands/` subdirectories are picked up dynamically — adding a new command folder is sufficient.

Additionally **generates** each `commands/*/SKILL.md` by baking `SKILL.template.md` (replacing `{{GAL_ROOT}}` with the absolute repo path) and appending any gitignored `SKILL.local.md` override from the same command directory. Setup-Machine then symlinks those command directories into Copilot and Codex skill targets while generating Gemini native command files from the same baked content.

For Gemini CLI, Setup-Machine writes GAL-managed `~/.gemini/commands/*.toml` files so Gemini exposes native slash commands without colliding with Agent Skills.

Generates `~/.gemini/gal-context.md` with sorted non-command `@file` skill imports. All import paths reference `~/.agents/skills/`.

Setup-Machine also merges the tracked MCP catalog from `mcp-servers.example.json` plus optional local overrides from `mcp-servers.local.json` into:

- VS Code `mcp.json`
- Gemini `settings.json` `mcpServers`
- Codex `config.toml` `[mcp_servers.*]`

The merge strategy is additive: existing provider-owned entries are preserved, and only missing servers are added.

See [docs/installation-topology.md](../docs/installation-topology.md) for the architecture-level explanation behind this runtime layout.
