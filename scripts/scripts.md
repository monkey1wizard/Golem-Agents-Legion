# Scripts

Machine setup and adapter sync scripts.

| Script | Platform | Purpose |
| --- | --- | --- |
| `gal.ps1` | Windows | `gal <subcommand>` dispatcher |
| `gal.sh` | macOS | `gal <subcommand>` dispatcher |
| `gal-smudge.sh` | cross-platform | Git smudge filter — replaces `<PLACEHOLDER>` with values from `config.local.env` |
| `gal-clean.sh` | cross-platform | Git clean filter — restores `<PLACEHOLDER>` tokens on commit |
| `Init-Repo.ps1` | Windows | Initialize `<repo>/.dev/` + `docs/plans/`, generate `.github/copilot-instructions.md`, `GEMINI.md`, `CLAUDE.md`, `AGENTS.md`, and `.agents/rules/gal.md`, then inspect any existing graphify artifacts without generating new ones |
| `init-repo.sh` | macOS | Same for Mac |
| `Sync-DevContext.ps1` | Windows | Generate `.github/copilot-instructions.md`, `GEMINI.md`, `CLAUDE.md`, `AGENTS.md`, and `.agents/rules/gal.md` from `.dev/project.md`; auto-discovers all skills in `skills/` |
| `sync-dev-context.sh` | macOS | Same for Mac |
| `Setup-Machine.ps1` | Windows | Prompt for selected runtimes + primary runtime on first run, persist `~/.gal/install-state.json`, then orchestrate `Update-Personalization.ps1`, `Update-Skills.ps1`, `Update-Commands.ps1`, and `Update-Mcp.ps1` |
| `Update-Personalization.ps1` | Windows | Manage install-state, Gemini `gal-context.md`, Gemini and VS Code settings bridges, Antigravity workspace-rule strategy, local config seeding, and git smudge/clean personalization |
| `Update-Skills.ps1` | Windows | Manage GAL root links, agent links, shared skill links, Antigravity skill links, Claude skill links, and legacy runtime skill cleanup |
| `Update-Commands.ps1` | Windows | Bake `commands/*/SKILL.md`, install Copilot/Codex/Antigravity command skill links, generate Gemini `.toml` commands, generate Claude `.md` commands, and remove stale command artifacts |
| `Update-Mcp.ps1` | Windows | Resolve `mcp.json` + `mcp.local.json` + `config.local.env`, then update VS Code Copilot, Copilot CLI, Gemini, Antigravity, Codex, and Claude MCP runtime config from the tracked manifest |
| `Setup-Tools.ps1` | Windows | Check optional collaborative tool status, ask which missing tools to install, install gstack / graphify / OpenCLI with official upstream methods, then verify GAL collaboration readiness |
| `setup-machine.sh` | macOS | Prompt for selected runtimes + primary runtime on first run, persist `~/.gal/install-state.json`, then orchestrate `update-personalization.sh`, `update-skills.sh`, `update-commands.sh`, and `update-mcp.sh` |
| `update-personalization.sh` | macOS | Manage install-state, Gemini `gal-context.md`, Gemini and VS Code settings bridges, Antigravity workspace-rule strategy, local config seeding, and git smudge/clean personalization |
| `update-skills.sh` | macOS | Manage GAL root links, agent links, shared skill links, Antigravity skill links, Claude skill links, and legacy runtime skill cleanup |
| `update-commands.sh` | macOS | Bake `commands/*/SKILL.md`, install Copilot/Codex/Antigravity command skill links, generate Gemini `.toml` commands, generate Claude `.md` commands, and remove stale command artifacts |
| `update-mcp.sh` | macOS | Resolve `mcp.json` + `mcp.local.json` + `config.local.env`, then update VS Code Copilot, Copilot CLI, Gemini, Antigravity, Codex, and Claude MCP runtime config from the tracked manifest |
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
| `/gal init` | Initialize `.dev/project.md`, `.dev/state.md`, and `docs/plans/`; if graphify artifacts already exist, inspect them and report freshness without generating new graphify output |
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

If the target repo does not contain local `scripts\gal.ps1` yet, stay in the target repo root and run the GAL runtime checkout entrypoint instead, for example `C:\path\to\gal\scripts\gal.ps1 init` or `C:\path\to\gal\scripts\gal.ps1 dispatch init`.

```bash
./scripts/gal.sh init
./scripts/gal.sh dispatch
./scripts/gal.sh dispatch init
./scripts/gal.sh dispatch golem-architect
```

If the target repo does not contain local `scripts/gal.sh` yet, stay in the target repo root and run the GAL runtime checkout entrypoint instead, for example `/path/to/gal/scripts/gal.sh init` or `/path/to/gal/scripts/gal.sh dispatch init`.

### Init-Repo: Adopt-Existing Mode

By default, `gal init` scans the target repo for existing documentation:

1. Finds README variants, `docs/` files, ADR directories
2. Detects tech stack from config files (`.csproj`, `package.json`, `go.mod`, etc.)
3. Pre-populates `.dev/project.md` Source Documents table and Tech Stack field
4. User/AI completes the summary by reviewing discovered docs

Use `--Blank` (PowerShell) or `--blank` (bash) to skip scanning and use a blank template.

## Setup-Machine Flow

`Setup-Machine.ps1` and `setup-machine.sh` are now concern orchestrators:

1. resolve or reconfigure runtime selection once
2. run `Update-Personalization.ps1` or `update-personalization.sh`
3. run `Update-Skills.ps1` or `update-skills.sh`
4. run `Update-Commands.ps1` or `update-commands.sh`
5. run `Update-Mcp.ps1` or `update-mcp.sh`

Each concern script can also run standalone when you only need one concern refreshed.

## Setup-Machine Symlinks

The setup script creates these symlinks:

| Source (repo) | Copilot Target | Gemini / Shared Target | Antigravity Target | Codex Target |
| --- | --- | --- | --- | --- |
| `agent/*.agent.md` | `~/.copilot/agents/` | — | — | — |
| `skills/*/` | `~/.copilot/skills/` | `~/.agents/skills/` | `~/.gemini/antigravity/skills/` | `~/.agents/skills/` |
| `commands/gal/` | `~/.copilot/skills/gal/` | `~/.gemini/commands/gal.toml` | `~/.gemini/antigravity/skills/gal/` | `~/.codex/skills/gal/` |
| `commands/gal-init/` | `~/.copilot/skills/gal-init/` | `~/.gemini/commands/gal-init.toml` | `~/.gemini/antigravity/skills/gal-init/` | `~/.codex/skills/gal-init/` |
| `commands/gal-status/` | `~/.copilot/skills/gal-status/` | `~/.gemini/commands/gal-status.toml` | `~/.gemini/antigravity/skills/gal-status/` | `~/.codex/skills/gal-status/` |
| `commands/gal-whats-next/` | `~/.copilot/skills/gal-whats-next/` | `~/.gemini/commands/gal-whats-next.toml` | `~/.gemini/antigravity/skills/gal-whats-next/` | `~/.codex/skills/gal-whats-next/` |
| `commands/gal-wrap-up/` | `~/.copilot/skills/gal-wrap-up/` | `~/.gemini/commands/gal-wrap-up.toml` | `~/.gemini/antigravity/skills/gal-wrap-up/` | `~/.codex/skills/gal-wrap-up/` |
| `commands/<specialist>/` | `~/.copilot/skills/<specialist>/` | `~/.gemini/commands/<specialist>.toml` | `~/.gemini/antigravity/skills/<specialist>/` | `~/.codex/skills/<specialist>/` |
| `<repo root>` | `~/.copilot/gal/` | `~/.gemini/gal/` (GAL_ROOT only) | `~/.gemini/antigravity/gal/` | — |

All `commands/` subdirectories are picked up dynamically — adding a new command folder is sufficient.

Additionally **generates** each `commands/*/SKILL.md` by baking `SKILL.template.md` (replacing `{{GAL_ROOT}}` with the absolute repo path) and appending any gitignored `SKILL.local.md` override from the same command directory. Setup-Machine then symlinks those command directories into Copilot and Codex skill targets while generating Gemini native command files from the same baked content.

Antigravity installs reusable skills and baked command skill directories under `~/.gemini/antigravity/skills/` and creates `~/.gemini/antigravity/gal/` -> repo-root symlinks for stable `GAL_ROOT` resolution.

`Sync-DevContext` generates `.agents/rules/gal.md`, which references the repo-local `AGENTS.md` through Antigravity's documented `@filename` rule syntax instead of introducing a custom Antigravity-only adapter file.

For Gemini CLI, Setup-Machine writes GAL-managed `~/.gemini/commands/*.toml` files so Gemini exposes native slash commands without colliding with Agent Skills.

Generates `~/.gemini/gal-context.md` with sorted non-command `@file` skill imports. All import paths reference `~/.agents/skills/`.

`Update-Mcp.ps1` and `update-mcp.sh` merge the tracked GAL MCP source from `mcp.json` plus optional local overrides from `mcp.local.json` into:

- VS Code `mcp.json`
- Copilot CLI `~/.copilot/mcp-config.json`
- Gemini `settings.json` `mcpServers`
- Antigravity `~/.gemini/antigravity/mcp_config.json` `mcpServers`
- Codex `config.toml` `[mcp_servers.*]`
- Claude Code user-scope MCP config via `claude mcp add/remove`

Copilot Chat and Copilot CLI continue to share the same `.copilot` skills and agents surface. Gemini CLI and Antigravity share the `.gemini` root but use separate GAL-managed subtrees. Only MCP ownership is split per client.

The merged manifest owns `servers` and can also carry optional top-level `inputs` for runtimes that accept prompt-backed MCP values.

The merge strategy is manifest-owned for GAL-managed server names: existing provider-owned entries with unrelated names are preserved, while tracked GAL server entries are overwritten in place on rerun so config updates propagate correctly. When a local override declares `servers.github`, GAL treats that local key as the active GitHub MCP entry and cleans up the older `github-mcp-server` name during bridge sync so the PAT-backed remote server replaces the tracked OAuth entry instead of duplicating it.

For Playwright MCP, keep the tracked `mcp.json` entry limited to safe core startup and place headed mode, storage-state paths, output directories, optional capability flags, persistent profile paths, extension/CDP wiring, and similar machine-local behavior in `mcp.local.json` plus `config.local.env`.

Rerun guidance:

- `Update-Mcp.ps1` / `update-mcp.sh`: refresh runtime MCP config after changing `mcp.json`, `mcp.local.json`, or MCP-related values in `config.local.env`.
- `Sync-DevContext.ps1` / `sync-dev-context.sh`: regenerate repo-local adapters such as `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, and `GEMINI.md` after changing their source-of-truth inputs.
- `Setup-Machine.ps1` / `setup-machine.sh`: rerun the full concern stack when you want one top-level refresh.

See [docs/installation-topology.md](../docs/installation-topology.md) for the architecture-level explanation behind this runtime layout.
