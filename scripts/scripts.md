# Scripts

Machine setup and adapter sync scripts.

| Script | Platform | Purpose |
| --- | --- | --- |
| `gal.ps1` | Windows | `gal <subcommand>` dispatcher |
| `gal.sh` | macOS | `gal <subcommand>` dispatcher |
| `Update-Mcp.ps1` | Windows | Resolve `plugins/gal-core/mcp.json` + `~/.gal/config/mcp.local.json` + `~/.gal/config/config.local.env`, then update VS Code Copilot, Copilot CLI, Antigravity, Codex, and Claude MCP runtime config from the tracked manifest; Google-side MCP install now lands in Antigravity's `mcp_config.json` and GAL-managed Gemini MCP entries are removed from `settings.json` |
| `update-mcp.sh` | macOS | Resolve `plugins/gal-core/mcp.json` + `~/.gal/config/mcp.local.json` + `~/.gal/config/config.local.env`, then update VS Code Copilot, Copilot CLI, Antigravity, Codex, and Claude MCP runtime config from the tracked manifest; Google-side MCP install now lands in Antigravity's `mcp_config.json` and GAL-managed Gemini MCP entries are removed from `settings.json` |
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

## gal setup Flow

`gal setup` (Rust, `crates/setup/`) is the machine-setup orchestrator:

1. resolve or reconfigure runtime selection once (persisted to `~/.gal/install-state.json`)
2. run the machine-surface refresh (library call into `adapters`)
3. run the MCP refresh (library call into `mcp`; skipped on uninstall)
4. register the `gal-config` git filter
5. run the Rust install orchestration (`gal install` / `gal uninstall`) through the setup crate

`gal setup --tools` checks and optionally installs the collaborative tools (gstack / graphify / OpenCLI / xmachine status).

Use `gal update --machine-only` when you only need machine-surface refreshes and `gal sync` when you only need repo-local adapter regeneration.

`gal setup --check` bypasses the normal concern chain and runs the in-process provider doctor, keeping the provider check path read-only.

## Current Boundary

The current script surface is split across two adjacent concerns:

- install-mode plugin orchestration: `gal install`, canonical-root render, `~/.gal/` ownership, and provider-specific lifecycle work
- bootstrap installer and official distribution channels: versioned bootstrap payloads, package managers, release archives, and marketplace/discoverability work

Today the AGY, Claude, Copilot, and Codex provider-native lifecycle slices are implemented in the install-mode scripts, including the read-only provider doctor/check surface. Bootstrap packaging and official install-channel wording remain handled by the separate bootstrap-installer planning track.

## gal setup Symlinks

The setup script creates these symlinks:

| Source (repo) | Copilot Target | Gemini / Shared Target | Antigravity Target | Codex Target | OpenCode Target |
| --- | --- | --- | --- | --- | --- |
| `plugins/gal-core/agents/*.agent.md` | — (Copilot reads them from the installed plugin payload) | — | `~/.gemini/antigravity-cli/plugins/gal/agents/` | — | `~/.config/opencode/agents/*.md` |
| `plugins/gal-core/skills/*/` | — (Copilot reads them from the installed plugin payload) | imported from repo paths via `~/.gemini/gal-context.md` | `~/.gemini/antigravity-cli/plugins/gal/skills/` | `~/.agents/skills/` | `~/.config/opencode/skills/` |
| `plugins/gal-core/commands/gal/` | — (Copilot reads it from the installed plugin payload) | `~/.gemini/commands/gal.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/gal/` | `~/.codex/skills/gal/` | `~/.config/opencode/commands/gal.md` |
| `commands/gal-init/` | — (Copilot reads it from the installed plugin payload) | `~/.gemini/commands/gal-init.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/gal-init/` | `~/.codex/skills/gal-init/` | `~/.config/opencode/commands/gal-init.md` |
| `commands/gal-status/` | — (Copilot reads it from the installed plugin payload) | `~/.gemini/commands/gal-status.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/gal-status/` | `~/.codex/skills/gal-status/` | `~/.config/opencode/commands/gal-status.md` |
| `commands/gal-whats-next/` | — (Copilot reads it from the installed plugin payload) | `~/.gemini/commands/gal-whats-next.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/gal-whats-next/` | `~/.codex/skills/gal-whats-next/` | `~/.config/opencode/commands/gal-whats-next.md` |
| `commands/gal-wrap-up/` | — (Copilot reads it from the installed plugin payload) | `~/.gemini/commands/gal-wrap-up.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/gal-wrap-up` | `~/.codex/skills/gal-wrap-up/` | `~/.config/opencode/commands/gal-wrap-up.md` |
| `commands/<specialist>/` | — (Copilot reads it from the installed plugin payload) | `~/.gemini/commands/<specialist>.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/<specialist>/` | `~/.codex/skills/<specialist>/` | `~/.config/opencode/commands/<specialist>.md` |
| `~/.gal/plugins/gal/` | `~/.copilot/installed-plugins/<marketplace>/gal/` | — | `~/.gemini/antigravity-cli/plugins/gal/` | marketplace/plugin projection | — |
| `~/.gal/` | `~/.copilot/gal/` | — | — | — | — |
| `~/.gal/source/` | — | `~/.gemini/gal/` (GAL_ROOT only) | `~/.gemini/antigravity-cli/gal/` (legacy GAL_ROOT only) | — | — |

All `plugins/gal-core/commands/` subdirectories are picked up dynamically — adding a new command folder is sufficient.

Additionally **generates** each `plugins/gal-core/commands/*/SKILL.md` by baking `SKILL.template.md` (replacing `{{GAL_ROOT}}` with the absolute repo path) and appending any gitignored `SKILL.local.md` override from the same command directory. `gal setup` then symlinks those command directories into Copilot and Codex skill targets while generating legacy Gemini native command files plus Claude and OpenCode markdown command files from the same baked content.

Antigravity installs as a provider plugin projected from the superset canonical root `~/.gal/plugins/gal/`. The canonical root carries all provider entry-point markers and is rendered by the Rust install/render path (`gal install`, `crates/gal-engine/src/render.rs`). Setup removes all prior GAL-managed AGY content (legacy skills directory, `GAL_ROOT` symlink, global MCP entries, prior plugin installs) before installing the clean plugin tree. Legacy GAL-managed links under `~/.gemini/skills/` are still cleaned up. AGY is projected to three surfaces: CLI junction (`~/.gemini/antigravity-cli/plugins/gal`), IDE junction (`~/.gemini/antigravity-ide/plugins/gal`), and GUI-config (`~/.gemini/config/plugins/gal` via `agy plugin install`).

`gal sync` generates `.agents/rules/gal.md`, which references the repo-local `AGENTS.md` through Antigravity's documented `@filename` rule syntax instead of introducing a custom Antigravity-only adapter file.

For legacy Gemini CLI compatibility, `gal setup` writes GAL-managed `~/.gemini/commands/*.toml` files so Gemini exposes native slash commands without colliding with Agent Skills.

Generates the legacy compatibility file `~/.gemini/gal-context.md` with sorted non-command `@file` skill imports. All import paths reference the current repo's `.agents/skills/` workspace directory.

`Update-Mcp.ps1` and `update-mcp.sh` merge the tracked GAL MCP source from `plugins/gal-core/mcp.json` plus optional local overrides from `~/.gal/config/mcp.local.json` into:

- VS Code `mcp.json`
- Copilot CLI `~/.copilot/mcp-config.json`
- Antigravity `~/.gemini/antigravity-cli/plugins/gal/mcp_config.json` `mcpServers` (plugin-root); global `~/.gemini/antigravity-cli/mcp_config.json` only touched for legacy cleanup
- Codex `config.toml` `[mcp_servers.*]`
- Claude Code user-scope MCP config via `claude mcp add/remove`

Copilot Chat and Copilot CLI continue to share the same `.copilot` skills and agents surface. Gemini CLI and Antigravity CLI share the `.gemini` root, but Google-side MCP ownership for GAL now lives in the AGY plugin-root `mcp_config.json`, and reruns remove the GAL-managed Gemini MCP entries previously written into `settings.json` as well as legacy GAL-managed entries from the global AGY `mcp_config.json`.

The merged manifest owns `servers` and can also carry optional top-level `inputs` for runtimes that accept prompt-backed MCP values.

The merge strategy is manifest-owned for GAL-managed server names: existing provider-owned entries with unrelated names are preserved, while tracked GAL server entries are overwritten in place on rerun so config updates propagate correctly. When a local override declares `servers.github`, GAL treats that local key as the active GitHub MCP entry and cleans up the older `github-mcp-server` name during bridge sync so the PAT-backed remote server replaces the tracked OAuth entry instead of duplicating it.

For Playwright MCP, keep the tracked `plugins/gal-core/mcp.json` entry limited to safe core startup and place headed mode, storage-state paths, output directories, optional capability flags, persistent profile paths, extension/CDP wiring, and similar machine-local behavior in `~/.gal/config/mcp.local.json` plus `~/.gal/config/config.local.env`.

Rerun guidance:

- `gal update --machine-only`: refresh runtime bridges, command projections, skill projections, and local personalization after changing `plugins/gal-core/agents/`, `plugins/gal-core/skills/`, `plugins/gal-core/commands/*/SKILL.*`, `~/.gal/config/config.local.env`, or `~/.gal/config/executor-routing.json`.
- `gal mcp update`: refresh runtime MCP config after changing `plugins/gal-core/mcp.json`, `~/.gal/config/mcp.local.json`, or MCP-related values in `~/.gal/config/config.local.env`.
- `gal sync`: regenerate repo-local adapters such as `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, and `GEMINI.md` after changing their source-of-truth inputs.
- `gal setup`: rerun the full machine refresh + install orchestration stack when you want one top-level refresh.

## Core Plugin Renderer

The Rust install/render path now owns canonical plugin rendering from the GAL source contracts. `gal install` renders the superset canonical root at `~/.gal/plugins/gal/`, emits the provider entry-point markers for Claude, Copilot, Codex, and AGY, writes provider lifecycle state under `~/.gal/dist/providers/`, and projects provider-visible surfaces such as `~/.claude/skills/gal/`, `~/.copilot/installed-plugins/gal-copilot/gal/`, and the AGY plugin roots.

See [docs/devguide.md](../docs/devguide.md) for the architecture-level explanation behind this runtime layout.
