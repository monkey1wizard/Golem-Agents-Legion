# Scripts

Machine setup and adapter sync scripts.

| Script | Platform | Purpose |
| --- | --- | --- |
| `gal.ps1` | Windows | `gal <subcommand>` dispatcher |
| `gal.sh` | macOS | `gal <subcommand>` dispatcher |
| `Build-CorePlugin.ps1` | Windows | Build and validate the provider-neutral common package, then render the superset canonical plugin root at `~/.gal/plugins/gal/` with all provider entry-point markers (Claude, AGY, Codex, Copilot); when `-Install` is specified, projects to all AGY surfaces (CLI junction, IDE junction, GUI-config) and removes GAL-owned vestigial artifacts |
| `build-core-plugin.sh` | macOS/Linux | Same for Mac/Linux |
| `gal-smudge.sh` | cross-platform | Git smudge filter — replaces `<PLACEHOLDER>` with values from `~/.gal/config/config.local.env` |
| `gal-clean.sh` | cross-platform | Git clean filter — restores `<PLACEHOLDER>` tokens on commit |
| `Init-Repo.ps1` | Windows | Initialize `<repo>/.dev/` + `docs/plans/`, generate `.github/copilot-instructions.md`, `GEMINI.md`, `CLAUDE.md`, and `AGENTS.md`, then inspect any existing graphify artifacts without generating new ones |
| `init-repo.sh` | macOS | Same for Mac |
| `Sync-DevContext.ps1` | Windows | Generate `.github/copilot-instructions.md`, `GEMINI.md`, `CLAUDE.md`, and `AGENTS.md` from `.dev/project.md`; auto-discovers all skills in `skills/` |
| `sync-dev-context.sh` | macOS | Same for Mac |
| `Setup-Machine.ps1` | Windows | Prompt for selected runtimes + primary runtime on first run, persist `~/.gal/install-state.json`, then orchestrate `Update-Personalization.ps1`, `Update-Skills.ps1`, `Update-Commands.ps1`, and `Update-Mcp.ps1` |
| `Install-GalPlugins.ps1` | Windows | Install-mode orchestration for resolver-driven provider lifecycle work; owns `~/.gal/` runtime-state setup, provider build dispatch, and install/uninstall plus explicit purge dry-run visibility for ownership boundaries |
| `Build-ProviderPlugins.ps1` | Windows | Build provider-specific install-mode package output and canonical-root metadata from resolver output; AGY and Claude renderers are wired, while Copilot/Codex remain not-yet-implemented native-install lanes |
| `Update-Personalization.ps1` | Windows | Manage install-state, legacy Gemini `gal-context.md` and settings bridges, Antigravity runtime integration, local config seeding, and git smudge/clean personalization |
| `Update-Skills.ps1` | Windows | Manage GAL root links, agent links, Antigravity global skill links, remaining shared skill links, Claude skill links, and legacy runtime skill cleanup |
| `Update-Commands.ps1` | Windows | Bake `commands/*/SKILL.md`, install Copilot/Codex/Antigravity command skill links, generate legacy Gemini `.toml` commands, generate Claude `.md` commands, generate OpenCode `.md` commands, and remove stale command artifacts |
| `Update-Mcp.ps1` | Windows | Resolve `mcp.json` + `~/.gal/config/mcp.local.json` + `~/.gal/config/config.local.env`, then update VS Code Copilot, Copilot CLI, Antigravity, Codex, and Claude MCP runtime config from the tracked manifest; Google-side MCP install now lands in Antigravity's `mcp_config.json` and GAL-managed Gemini MCP entries are removed from `settings.json` |
| `Setup-Tools.ps1` | Windows | Check optional collaborative tool status, ask which missing tools to install, install gstack / graphify / OpenCLI with official upstream methods, then verify GAL collaboration readiness |
| `setup-machine.sh` | macOS | Prompt for selected runtimes + primary runtime on first run, persist `~/.gal/install-state.json`, then orchestrate `update-personalization.sh`, `update-skills.sh`, `update-commands.sh`, and `update-mcp.sh` |
| `install-gal-plugins.sh` | macOS/Linux | Install-mode orchestration for resolver-driven provider lifecycle work; owns `~/.gal/` runtime-state setup, provider build dispatch, and install/uninstall plus explicit purge dry-run visibility for ownership boundaries |
| `build-provider-plugins.sh` | macOS/Linux | Build provider-specific install-mode package output and canonical-root metadata from resolver output; AGY and Claude renderers are wired, while Copilot/Codex remain not-yet-implemented native-install lanes |
| `update-personalization.sh` | macOS | Manage install-state, legacy Gemini `gal-context.md` and settings bridges, Antigravity runtime integration, local config seeding, and git smudge/clean personalization |
| `update-skills.sh` | macOS | Manage GAL root links, agent links, Antigravity global skill links, remaining shared skill links, Claude skill links, and legacy runtime skill cleanup |
| `update-commands.sh` | macOS | Bake `commands/*/SKILL.md`, install Copilot/Codex/Antigravity command skill links, generate legacy Gemini `.toml` commands, generate Claude `.md` commands, generate OpenCode `.md` commands, and remove stale command artifacts |
| `update-mcp.sh` | macOS | Resolve `mcp.json` + `~/.gal/config/mcp.local.json` + `~/.gal/config/config.local.env`, then update VS Code Copilot, Copilot CLI, Antigravity, Codex, and Claude MCP runtime config from the tracked manifest; Google-side MCP install now lands in Antigravity's `mcp_config.json` and GAL-managed Gemini MCP entries are removed from `settings.json` |
| `setup-tools.sh` | macOS | Same for Mac/Linux |
| `Uninstall-Machine.ps1` | Windows | Remove GAL-managed machine artifacts while preserving user-owned config, lockfile, xmachine bindings, local overrides, and secrets by default; `-Purge -ConfirmPurge` makes destructive reset explicit |
| `uninstall-machine.sh` | macOS | Same for Mac, using `--purge --confirm-purge` for explicit destructive reset |
| `Invoke-XmachineRemoteTask.ps1` | Windows | Dispatch a task to the remote Windows xmachine lane over SSH |
| `Start-xMachine.ps1` | Windows | Run a task on the remote Windows machine in the `remote-windows` lane via Gemini CLI (legacy Google headless lane pending Antigravity CLI parity) |
| `Get-XmachineRemoteResult.ps1` | Windows | Retrieve results from a completed remote xmachine task and clean up the worktree |
| `Test-Xmachine.ps1` | Windows | Smoke-test wrapper for the `remote-windows` xmachine lane |
| `Test-Xmachine.sh` | macOS/Linux | Smoke-test wrapper for the `local-async` xmachine lane |
| `Invoke-XmachineLocalTask.sh` | macOS/Linux | Dispatch a task into the `local-async` xmachine lane on a controlled machine |
| `Start-xMachine.sh` | macOS/Linux | Run a task on the local machine in the `local-async` lane via Gemini CLI (legacy Google headless lane pending Antigravity CLI parity) |
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

## Current Boundary

The current script surface is split across two adjacent concerns:

- install-mode plugin orchestration: `Install-GalPlugins.*`, `Build-ProviderPlugins.*`, resolver output, `~/.gal/` ownership, and provider-specific lifecycle work
- bootstrap installer and official distribution channels: versioned bootstrap payloads, package managers, release archives, and marketplace/discoverability work

Today the AGY provider-native lifecycle slice is implemented end to end in the install-mode scripts, and Claude also has canonical-root rendering, lifecycle-state tracking, validation, and session-load projection coverage. Copilot CLI and Codex remain planned native-install lanes, while bootstrap packaging and official install-channel wording are handled by the separate bootstrap-installer planning track.

## Setup-Machine Symlinks

The setup script creates these symlinks:

| Source (repo) | Copilot Target | Gemini / Shared Target | Antigravity Target | Codex Target | OpenCode Target |
| --- | --- | --- | --- | --- | --- |
| `agent/*.agent.md` | `~/.copilot/agents/` | — | `~/.gemini/antigravity-cli/plugins/gal/agents/` | — | `~/.config/opencode/agents/*.md` |
| `skills/*/` | `~/.copilot/skills/` | imported from repo paths via `~/.gemini/gal-context.md` | `~/.gemini/antigravity-cli/plugins/gal/skills/` | `~/.agents/skills/` | `~/.config/opencode/skills/` |
| `commands/gal/` | `~/.copilot/skills/gal/` | `~/.gemini/commands/gal.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/gal/` | `~/.codex/skills/gal/` | `~/.config/opencode/commands/gal.md` |
| `commands/gal-init/` | `~/.copilot/skills/gal-init/` | `~/.gemini/commands/gal-init.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/gal-init/` | `~/.codex/skills/gal-init/` | `~/.config/opencode/commands/gal-init.md` |
| `commands/gal-status/` | `~/.copilot/skills/gal-status/` | `~/.gemini/commands/gal-status.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/gal-status/` | `~/.codex/skills/gal-status/` | `~/.config/opencode/commands/gal-status.md` |
| `commands/gal-whats-next/` | `~/.copilot/skills/gal-whats-next/` | `~/.gemini/commands/gal-whats-next.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/gal-whats-next/` | `~/.codex/skills/gal-whats-next/` | `~/.config/opencode/commands/gal-whats-next.md` |
| `commands/gal-wrap-up/` | `~/.copilot/skills/gal-wrap-up/` | `~/.gemini/commands/gal-wrap-up.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/gal-wrap-up/` | `~/.codex/skills/gal-wrap-up/` | `~/.config/opencode/commands/gal-wrap-up.md` |
| `commands/<specialist>/` | `~/.copilot/skills/<specialist>/` | `~/.gemini/commands/<specialist>.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/<specialist>/` | `~/.codex/skills/<specialist>/` | `~/.config/opencode/commands/<specialist>.md` |
| `~/.gal/source/` | `~/.copilot/gal/` | `~/.gemini/gal/` (GAL_ROOT only) | `~/.gemini/antigravity-cli/plugins/gal/` (plugin tree) | — | — |

All `commands/` subdirectories are picked up dynamically — adding a new command folder is sufficient.

Additionally **generates** each `commands/*/SKILL.md` by baking `SKILL.template.md` (replacing `{{GAL_ROOT}}` with the absolute repo path) and appending any gitignored `SKILL.local.md` override from the same command directory. Setup-Machine then symlinks those command directories into Copilot and Codex skill targets while generating legacy Gemini native command files plus Claude and OpenCode markdown command files from the same baked content.

Antigravity installs as a provider plugin projected from the superset canonical root `~/.gal/plugins/gal/`. The canonical root carries all provider entry-point markers and is rendered by `Build-CorePlugin` from the provider-neutral common package model. Setup removes all prior GAL-managed AGY content (legacy skills directory, `GAL_ROOT` symlink, global MCP entries, prior plugin installs) before installing the clean plugin tree. Legacy GAL-managed links under `~/.gemini/skills/` are still cleaned up. AGY is projected to three surfaces: CLI junction (`~/.gemini/antigravity-cli/plugins/gal`), IDE junction (`~/.gemini/antigravity-ide/plugins/gal`), and GUI-config (`~/.gemini/config/plugins/gal` via `agy plugin install`).

`Sync-DevContext` generates `.agents/rules/gal.md`, which references the repo-local `AGENTS.md` through Antigravity's documented `@filename` rule syntax instead of introducing a custom Antigravity-only adapter file.

For legacy Gemini CLI compatibility, Setup-Machine writes GAL-managed `~/.gemini/commands/*.toml` files so Gemini exposes native slash commands without colliding with Agent Skills.

Generates the legacy compatibility file `~/.gemini/gal-context.md` with sorted non-command `@file` skill imports. All import paths reference the current repo's `.agents/skills/` workspace directory.

`Update-Mcp.ps1` and `update-mcp.sh` merge the tracked GAL MCP source from `mcp.json` plus optional local overrides from `~/.gal/config/mcp.local.json` into:

- VS Code `mcp.json`
- Copilot CLI `~/.copilot/mcp-config.json`
- Antigravity `~/.gemini/antigravity-cli/plugins/gal/mcp_config.json` `mcpServers` (plugin-root); global `~/.gemini/antigravity-cli/mcp_config.json` only touched for legacy cleanup
- Codex `config.toml` `[mcp_servers.*]`
- Claude Code user-scope MCP config via `claude mcp add/remove`

Copilot Chat and Copilot CLI continue to share the same `.copilot` skills and agents surface. Gemini CLI and Antigravity CLI share the `.gemini` root, but Google-side MCP ownership for GAL now lives in the AGY plugin-root `mcp_config.json`, and reruns remove the GAL-managed Gemini MCP entries previously written into `settings.json` as well as legacy GAL-managed entries from the global AGY `mcp_config.json`.

The merged manifest owns `servers` and can also carry optional top-level `inputs` for runtimes that accept prompt-backed MCP values.

The merge strategy is manifest-owned for GAL-managed server names: existing provider-owned entries with unrelated names are preserved, while tracked GAL server entries are overwritten in place on rerun so config updates propagate correctly. When a local override declares `servers.github`, GAL treats that local key as the active GitHub MCP entry and cleans up the older `github-mcp-server` name during bridge sync so the PAT-backed remote server replaces the tracked OAuth entry instead of duplicating it.

For Playwright MCP, keep the tracked `mcp.json` entry limited to safe core startup and place headed mode, storage-state paths, output directories, optional capability flags, persistent profile paths, extension/CDP wiring, and similar machine-local behavior in `~/.gal/config/mcp.local.json` plus `~/.gal/config/config.local.env`.

Rerun guidance:

- `Update-Mcp.ps1` / `update-mcp.sh`: refresh runtime MCP config after changing `mcp.json`, `~/.gal/config/mcp.local.json`, or MCP-related values in `~/.gal/config/config.local.env`.
- `Sync-DevContext.ps1` / `sync-dev-context.sh`: regenerate repo-local adapters such as `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, and `GEMINI.md` after changing their source-of-truth inputs.
- `Setup-Machine.ps1` / `setup-machine.sh`: rerun the full concern stack when you want one top-level refresh.

## Core Plugin Renderer

`Build-CorePlugin.ps1` and `build-core-plugin.sh` render the superset canonical plugin root from the GAL repo source contracts.

### What It Does

1. Builds a provider-neutral common package using `New-ProviderPluginPackage` / `build_provider_plugin_package`
2. Validates the common package using `Test-ProviderPluginPackage` / `validate_provider_plugin_package`
3. Renders the superset canonical root at `~/.gal/plugins/gal/` with all provider entry-point markers:
   - `.claude-plugin/plugin.json` — Claude Code plugin manifest
   - `skills/` — reusable skills (shared by all providers)
   - `commands/` — flat command markdown files (Claude / Copilot)
   - `agents/<name>.md` — Claude-compatible filtered agent definitions
   - `agents/<name>.agent.md` — AGY-compatible unfiltered agent definitions
   - `.mcp.json` — portable non-secret MCP server configuration (Claude / Copilot)
   - `plugin.json` — AGY root manifest
   - `mcp_config.json` — AGY MCP configuration
   - `rules/gal.md` — AGY instruction corpus
4. When `-Install` / `--install` is specified, projects to all AGY surfaces (link-first):
   - CLI junction: `~/.gemini/antigravity-cli/plugins/gal` → canonical root
   - IDE junction: `~/.gemini/antigravity-ide/plugins/gal` → canonical root
   - GUI-config: `agy plugin install <canonical root>` (host-managed copy)
   - Removes GAL-owned vestigial artifacts (whitelist-guarded)

### What It Does NOT Output

- `hooks.json`
- `scripts/`
- Marketplace metadata
- Provider stubs moved out of the shared root
- `gal-results/`

### Usage

```powershell
# Render superset canonical root (all provider markers)
.\scripts\Build-CorePlugin.ps1

# Force overwrite existing artifacts
.\scripts\Build-CorePlugin.ps1 -Force

# Render and install to all AGY surfaces (CLI + IDE + GUI-config)
.\scripts\Build-CorePlugin.ps1 -Install -Force
```

```bash
# Render superset canonical root (all provider markers)
./scripts/build-core-plugin.sh

# Force overwrite existing artifacts
./scripts/build-core-plugin.sh --force

# Render and install to all AGY surfaces (CLI + IDE + GUI-config)
./scripts/build-core-plugin.sh --install --force
```

### Common Package Model

The renderer relies on `scripts/common/ProviderPlugin.ps1` and `scripts/common/provider-plugin.sh` for the provider-neutral substrate. The common model contains no provider-specific paths, no resolved local secrets, and no runtime scripts. It explicitly records skipped components (`hooks`, `runtimeScripts`) so unsupported features are documented rather than silently omitted.

See [docs/installation-topology.md](../docs/installation-topology.md) for the architecture-level explanation behind this runtime layout.
