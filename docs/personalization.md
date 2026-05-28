# Personalization

This document holds the machine-local details that do not belong on the README front page: placeholders, runtime selection, model routing, MCP overrides, Obsidian routing, working-hours settings, and when to rerun setup.

## Runtime Selection

The machine installer now persists runtime selection in `~/.gal/install-state.json`.

- `selectedRuntimes` records which machine-layer targets GAL should manage.
- `primaryRuntime` records which runtime should be treated as your default entry point.
- The GAL repo remains the single source of truth for `agent/`, `skills/`, and `commands/`. Primary runtime affects defaults and summaries, not the underlying source content.

Antigravity CLI (AGY) is the primary Google terminal runtime for GAL. GAL installs into AGY as a provider plugin at `~/.gemini/antigravity-cli/plugins/gal/`, which carries skills, agents, rules, and MCP config as a self-contained plugin tree. The plugin is a generated artifact rendered by `Build-AgyPlugin` from a provider-neutral common package model; AGY is renderer 1, not the architecture itself. `Update-Personalization` keeps the integration conservative, does not mutate user-owned global Antigravity rule files, and does not create repo-local `.agents` content.

Use setup again with `-Reconfigure` on Windows or `--reconfigure` on macOS/Linux if you want to change the selected runtimes or primary runtime.

The machine setup surface is now split by concern on both Windows and macOS/Linux:

- `scripts/Setup-Machine.ps1` runs the full sequence
- `scripts/Update-Personalization.ps1` refreshes install-state, legacy Gemini settings bridges and `gal-context.md`, Antigravity plugin integration (renders `rules/gal.md` via `Build-AgyPlugin`), and local config seeding
- `scripts/Update-Skills.ps1` refreshes agents, Antigravity plugin skills (via `Build-AgyPlugin`), remaining shared skill links, and GAL root links
- `scripts/Update-Commands.ps1` refreshes baked command skills, Antigravity plugin command skills (via `Build-AgyPlugin`), remaining Gemini native command files, and Claude legacy cleanup state
- `scripts/Update-Mcp.ps1` refreshes runtime MCP config from the tracked manifest, including Antigravity plugin-root `mcp_config.json`
- `scripts/setup-machine.sh` runs the full sequence
- `scripts/update-personalization.sh` refreshes install-state, legacy Gemini settings bridges and `gal-context.md`, Antigravity plugin integration (renders `rules/gal.md` via `Build-AgyPlugin`), and local config seeding
- `scripts/update-skills.sh` refreshes agents, Antigravity plugin skills (via `Build-AgyPlugin`), remaining shared skill links, and GAL root links
- `scripts/update-commands.sh` refreshes baked command skills, Antigravity plugin command skills (via `Build-AgyPlugin`), remaining Gemini native command files, and Claude legacy cleanup state
- `scripts/update-mcp.sh` refreshes runtime MCP config from the tracked manifest, including Antigravity plugin-root `mcp_config.json`

## Install Mode vs Source Mode

GAL supports two operational modes controlled by `~/.gal/config/config.json`:

- **Install mode** — for end users who just want to use GAL. You don't need to clone the repo. Install via `winget` (Windows) or `homebrew` (macOS/Linux), and GAL manages its own `~/.gal/` runtime home. AGY already supports the full provider-native lifecycle without a source checkout. Claude now supports plugin artifact rendering, strict validation when the local CLI exposes it, lifecycle-state tracking, and session-load smoke without a source checkout, but direct provider-native install is still capability-dependent and not yet a verified default lane. Broader install-mode defaults remain gated on the remaining provider smoke guards.

- **Source mode** — for GAL contributors. Keep a local clone of the GAL repo, set `galRoot` in `~/.gal/config/config.json` to that path, and enable `devMode`. This gives you live local overrides, direct repo-skill mounting, and the ability to test changes without packaging.

To switch modes:

- Set `installMode` to `install` or `source` in `~/.gal/config/config.json`.
- In source mode, also set `galRoot` to your local GAL repo path and optionally enable `devMode`.
- Rerun `Setup-Machine` after switching.

### Bootstrap First Launch

For package-managed installs, first launch is intentionally install-mode-first:

- If `~/.gal/config/config.json` does not exist yet, the bootstrap path seeds `installMode=install`, keeps `devMode=false`, and does not require `galRoot`.
- That first launch creates or reuses `~/.gal/`, resolves the default profile into `~/.gal/state/plugins.lock.json`, and refreshes provider projections plus any `~/.gal/active/<provider>/` targets needed by install mode.
- Source mode is an explicit opt-in for contributors. Switch only after you set `installMode=source`, set `galRoot`, and rerun setup.

This means the installed `gal` package path can stay install-mode-first for end users, while the repo-owned `Setup-Machine` scripts remain a contributor and packaging harness that mirrors the same branching rules. Contributors still have a clear path back to repo-root development mode.

### Upgrade Boundaries

When GAL is refreshed through a future package-manager upgrade, a provider-native direct-update lane, or a manual archive replacement, the machine-local rule stays the same:

- the payload may be replaced and GAL-managed generated state may be refreshed
- `~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, explicit local overrides, and secret sources remain user-owned and must be preserved
- an upgrade must not silently switch `installMode`, clear `galRoot`, or turn `devMode` on or off unless you edit the config yourself

In other words, upgrades may refresh GAL-managed runtime outputs, but they must preserve your machine intent.

### Uninstall And Purge Boundaries

Default uninstall is not a reset button.

- package-manager uninstall removes the packaged `gal` binary only
- GAL-managed uninstall removes rebuildable GAL-owned runtime outputs such as provider-native plugin installs owned by GAL, `~/.gal/store/plugins`, and generated projections under `~/.gal/generated/`
- `~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, `~/.gal/state/plugins.lock.json`, explicit local overrides, and secret sources remain user-owned and must be preserved

If you want a true reset, use the explicit purge/reset lane such as `Uninstall-Machine -Purge -ConfirmPurge` or `uninstall-machine.sh --purge --confirm-purge`. Default uninstall must never silently delete the preserved surfaces above.

### Backup And Migration

When you move GAL to a new machine, preserve the machine-local intent rather than the rebuildable payload:

- back up `~/.gal/config/config.json`
- back up `~/.gal/config/xmachine.json`
- back up `~/.gal/state/plugins.lock.json`
- back up explicit local overrides and any secret sources your local setup depends on

You do not need to carry forward package-managed `gal` binaries, provider plugin install trees, `~/.gal/store/plugins`, or `~/.gal/generated/` projections. Reinstall GAL first, restore the backed-up machine-intent files, then rerun setup or bootstrap refresh so GAL can rebuild the managed runtime outputs.

### Channel Lag And Non-Guarantees

Machine-local settings do not change GAL's release-channel contract. Follow `README.md` and `docs/release-matrix.md` for the user-facing lag, fallback, and non-guarantee policy. The machine-local implication is narrower: preserve your machine-intent files so you can reinstall or refresh the managed payload later without losing local state.

### Distribution Architecture and Ownership Boundaries

GAL explicitly separates how the CLI is installed from how the provider plugins are distributed and managed:

- **Bootstrap Installer Distribution**: `winget` (Windows), `homebrew` (macOS/Linux), and GitHub Releases handle the installation of the `gal` executable binary. They own the **package-managed payload**.
- **Install-Mode Plugin Distribution**: GAL owns the plugin catalog (`plugins/catalog.json`) and resolves it into `~/.gal/state/plugins.lock.json` to manage provider-native plugin installations (e.g., Claude, AGY, Copilot).
- **GAL-Managed Runtime and Generated State**: GAL manages content under `~/.gal/store/` and `~/.gal/generated/`. These are safe to rebuild or reinstall.
- **User-Owned Config and State**: You own `~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, `~/.gal/state/plugins.lock.json`, explicit local overrides, and secrets. Package managers must **never** delete these during uninstalls. A full destructive cleanup requires an explicit purge flow.

## Companion Plugins and Support Tiers

GAL keeps `gal-core` small: the control plane, golem agents, core workflows, essential conventions, and a small set of GAL-owned skills. Everything else is an external companion plugin you opt into.

**Support tiers** tell you who maintains the content:

| Tier | Maintained by | Auto-update | Example |
| --- | --- | --- | --- |
| `official-gal` | GAL repo / release artifacts | yes, via GAL releases | `gal-core` |
| `curated-upstream` | external upstream repo; GAL locks the version | controlled, per lockfile pin | `dart-lang/skills` |
| `mirrored` | external upstream, managed mirror by GAL | no unversioned copies | upstream that needs a managed cache |
| `forked` | fork owner (GAL or user) | manual, with fork base tracking | a patched fork of an upstream skill |
| `local` | you, for source-mode overrides only | never shared | `file://` local path |

**Default profile**: the initial `default` profile only installs `gal-core`. All companion plugins are opt-in. Enable them through named profiles (e.g., `dart`, `flutter`, `dotnet`) or explicit plugin selection in `~/.gal/config/config.json`.

**Known companion candidates** (all `curated-upstream`, all opt-in):

- `dart-lang/skills` — Dart
- `flutter/skills` — Flutter
- `dotnet/skills` — .NET / C#
- `anthropics/skills` — Claude ecosystem
- `samber/cc-skills-golang` — Go
- `twostraws/swift-agent-skills` — Swift
- `kepano/obsidian-skills` — Obsidian
- `actionbook/rust-skills` — Rust

Game asset, Godot, and GStack framework skills remain in `gal-core` (GAL-owned, not external companion) unless confirmed otherwise.

Your plugin selections, profiles, and resolver output live in:

- `~/.gal/config/config.json` — what you want to install
- `~/.gal/state/plugins.lock.json` — what is actually resolved and locked

Back up `~/.gal/config/config.json` and `~/.gal/state/plugins.lock.json` when migrating machines. Package-managed payloads, provider plugin install trees, and `~/.gal/generated/` content can be rebuilt by reinstalling.

## Placeholders You May Need To Fill

| Placeholder | Meaning | Common use |
| --- | --- | --- |
| `<OBSIDIAN_VAULT>` | absolute path to the Obsidian vault | Obsidian agents and skills |
| `<OBSIDIAN_VAULT_NAME>` | display name of the vault | Obsidian skills |
| `<OBSIDIAN_GUIDE_PATH>` | vault-relative path to your personal Obsidian guide | notewriter and Obsidian knowledge workflows |
| `<OBSIDIAN_GUIDE_MODE>` | `auto`, `guide`, or `generic` | optional Guide loading for Obsidian writes |
| `<OBSIDIAN_PRIVATE_RESEARCH_DIR>` | vault-relative directory for private research captures | `/gal research` private-note routing |
| `<OBSIDIAN_DIARY_DIR>` | vault-relative directory for work diaries | notewriter diary mode |
| `<OBSIDIAN_SCRATCH_DIR>` | vault-relative directory for quick scratch logs | notewriter diary mode |
| `<OBSIDIAN_ARCHIVE_DIR>` | vault-relative directory for diary archives | notewriter diary mode |
| `<RESEARCH_DEFAULT_DEST>` | default durable destination for research output | `repo`, `private`, `knowledge`, or `none` |
| `<WORKING_HOURS_ENABLED>` | whether working-hours enforcement is active on this machine | opt-in wrap-up and hard-stop enforcement |
| `<WORKDAY_START>` | start of the preferred workday in `HH:MM` | Working Hours schedule |
| `<WORKDAY_END>` | end of the preferred workday in `HH:MM` | After Hours boundary |
| `<WRAP_UP_TIME>` | Wrap-up Time in `HH:MM` | shutdown-window behavior |
| `<HARD_STOP_TIME>` | Hard Stop in `HH:MM` | stop-work behavior |
| `<LOCAL_SEARCH_PROJECT>` | clone path for the local search project | local-first and knowledge-management skills |
| `<GAL_ROOT>` | path to local GAL repo clone (source mode only) | source mode contributor workflow |
| `<TEMP_DIR>` | temp output directory | PDF and file-processing workflows |
| `<MCP_FILESYSTEM_PATHS>` | allowed root paths for the filesystem MCP server | MCP manifest merge |
| `<CONTEXT7_API_KEY>` | Context7 API key for runtimes that require it | MCP manifest merge (materialized into `~/.gal/generated/mcp/managed.json`) |

## Common Personalization Steps

### 1. Model routing

- Copy `model-roles.example.md` into `~/.gal/config/model-roles.local.md`.
- Change provider and model mappings only in `~/.gal/config/model-roles.local.md`.

### 2. Local secrets and paths

- Put secrets, absolute paths, and machine-specific values in `~/.gal/config/config.local.env`.
- Do not write local values into tracked docs, command templates, or source files.

### 2a. Command skill local overlays

If you want a machine-local customization for a specific command skill that should survive `Setup-Machine`, create `commands/<command>/SKILL.local.md`.

- `SKILL.local.md` is gitignored and treated as user-owned machine-local input.
- `Setup-Machine` bakes `SKILL.template.md`, then appends `SKILL.local.md` into the generated `SKILL.md` before regenerating the baked command outputs that still feed Gemini compatibility and Claude plugin packaging inputs.
- Do not edit `commands/<command>/SKILL.md` directly. It remains a generated file and will be replaced on the next setup run.
- Keep `SKILL.local.md` to additional instruction content only. Do not add a second frontmatter block.

### 2b. Obsidian routing

Obsidian support is machine-local and optional. GAL separates repo-owned state from user-owned notes:

- GAL's Obsidian automation now uses the built-in `obsidian` CLI, not the old Local REST API MCP bridge.
- Repo-owned research stays in `docs/research/` by default.
- Private captures and reusable knowledge can route into your Obsidian vault when `OBSIDIAN_VAULT` is configured.
- If you want GAL to follow your own library rules, set `OBSIDIAN_GUIDE_PATH` and leave `OBSIDIAN_GUIDE_MODE=auto` or force `guide`.
- If you do not keep a personal guide, leave `OBSIDIAN_GUIDE_PATH` empty or set `OBSIDIAN_GUIDE_MODE=generic`.

Vault-relative paths should not include the vault root and should not end with a trailing slash.

Recommended defaults:

| Setting | Typical value |
| --- | --- |
| `OBSIDIAN_GUIDE_PATH` | `Guide.md` |
| `OBSIDIAN_PRIVATE_RESEARCH_DIR` | `/Projects/Research_Private` |
| `OBSIDIAN_DIARY_DIR` | `/Projects/Work_Journal` |
| `OBSIDIAN_SCRATCH_DIR` | `/Projects/Work_Journal` |
| `OBSIDIAN_ARCHIVE_DIR` | `/Archives/Work_Journal` |
| `RESEARCH_DEFAULT_DEST` | `repo` |

### 2c. Working Hours

Working-hours enforcement is disabled by default. If you want GAL to respect your own workday boundary, configure it in `~/.gal/config/config.local.env`:

- `WORKING_HOURS_ENABLED=false` keeps all working-hours logic off.
- `WORKDAY_START` and `WORKDAY_END` describe your preferred work window.
- `WRAP_UP_TIME` starts reminders and shutdown-window behavior.
- `HARD_STOP_TIME` defines the point where agents refuse further work.

These values are machine-local preferences, not tracked repo policy.

### 2d. xmachine node config

xmachine node definitions are machine-local and live in `~/.gal/config/xmachine.json`.

- Copy `xmachine.config.example.json` into `~/.gal/config/xmachine.json`.
- Define each work node under the top-level `nodes` object.
- Use the node alias as the key and set at least `target` and `repoPath`.
- Add `runtimeRepoPath` when the remote GAL runtime checkout lives in a different path from the target repo checkout.
- Add `repoMappings` when one work node hosts multiple target repositories and you want GAL to resolve the remote repo automatically from the current local repo name.
- Keep SSH targets and repo paths in `~/.gal/config/xmachine.json`, not in `~/.gal/config/config.local.env`.

Example:

```json
{
  "nodes": {
    "mac-mini": {
      "target": "username@username-mac-mini.local",
      "repoPath": "/Users/username/Golem-Agents-Legion",
      "runtimeRepoPath": "/Users/username/Golem-Agents-Legion",
      "repoMappings": {
        "local-ai-tools": {
          "repoPath": "/Users/username/Code/zawip/local-ai-tools",
          "runtimeRepoPath": "/Users/username/Golem-Agents-Legion"
        }
      }
    }
  }
}
```

In this example, GAL can keep using the same `mac-mini` node alias while routing `Golem-Agents-Legion` and `local-ai-tools` to different remote checkouts.

`scripts/Test-Xmachine.ps1` reads `~/.gal/config/xmachine.json` directly, with repo-root fallback kept only as a warned migration path, so editing the canonical file does not require rerunning setup.

### 3. MCP overrides

- Keep the tracked GAL source in `../mcp.json`.
- Put machine-specific MCP differences in `~/.gal/config/mcp.local.json`.

Project-specific or database-specific MCP servers should usually live in `~/.gal/config/mcp.local.json`, not the tracked `mcp.json`. This matters for Postgres because one machine may work across many repos, and one repo may talk to multiple databases.

For Playwright MCP, keep the tracked `mcp.json` entry conservative and machine-agnostic. Put local-only browser behavior in `~/.gal/config/mcp.local.json`: headed mode, viewport or device emulation, storage-state paths, output directories, optional capability flags, persistent profile paths, extension or CDP connections, and other stateful browser settings.

Example local-only Playwright override:

```json
{
  "servers": {
    "playwright": {
      "args": [
        "-y",
        "@playwright/mcp@latest",
        "--isolated",
        "--headless",
        "--storage-state",
        "${PLAYWRIGHT_MCP_STORAGE_STATE}",
        "--output-dir",
        "${PLAYWRIGHT_MCP_OUTPUT_DIR}"
      ]
    }
  }
}
```

This override replaces the full `args` list for `playwright`, so keep the inherited safe defaults you still want, such as `--isolated` and `--headless`. `command` and `type` continue to come from the tracked entry through the normal deep-merge behavior.

Keep those env vars in `~/.gal/config/config.local.env`, and keep the referenced files/directories outside tracked repo paths. Do not commit storage-state files, persistent browser profiles, browser output artifacts, or any secret-like local files.

Example for two Postgres databases on one machine:

```json
{
  "servers": {
    "postgres-app": {
      "type": "stdio",
      "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": {
        "DATABASE_URI": "${POSTGRES_MCP_APP_URI}"
      }
    },
    "postgres-analytics": {
      "type": "stdio",
      "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": {
        "DATABASE_URI": "${POSTGRES_MCP_ANALYTICS_URI}"
      }
    }
  }
}
```

Then add matching variables to `~/.gal/config/config.local.env` with any names you want. `Update-Mcp.ps1` and `update-mcp.sh` already merge all local server names and resolve arbitrary `${ENV_VAR}` placeholders from `~/.gal/config/config.local.env`.

### 4. Runtime-owned config

The installed runtime configs remain user-owned even when GAL refreshes GAL-managed entries.

| Runtime | Typical MCP config location |
| --- | --- | |
| VS Code | user `mcp.json` |
| Antigravity CLI | `~/.gemini/antigravity-cli/plugins/gal/mcp_config.json` (plugin-root) |
| Codex CLI | `config.toml` under `[mcp_servers.*]` |
| Claude Code | user-scope MCP entries managed through `claude mcp` |

GAL now treats `mcp.json` plus `~/.gal/config/mcp.local.json` as the MCP source of truth. Rerunning `Update-Mcp.ps1` or `update-mcp.sh` overwrites only GAL-managed server names for supported runtimes and preserves unrelated user-defined entries. For AGY, GAL-managed MCP lands in the plugin-root `mcp_config.json` at `~/.gemini/antigravity-cli/plugins/gal/mcp_config.json`; the global `~/.gemini/antigravity-cli/mcp_config.json` is only touched for legacy cleanup of old GAL-managed entries. Gemini legacy compatibility remains installed for commands or context, and reruns remove the GAL-managed Gemini MCP entries previously written into `settings.json`.

## When To Rerun Setup

Run setup again when any of these change:

- `~/.gal/config/config.local.env`
- `mcp.json`
- `~/.gal/config/mcp.local.json`
- any `commands/*/SKILL.local.md`
- `~/.gal/install-state.json`
- Obsidian routing paths or Guide mode
- working-hours settings
- model routing or runtime install locations
- GAL command or skill installation

`~/.gal/config/xmachine.json` is read directly by the xmachine scripts and does not require a setup rerun.

Use the narrower concern script when only one concern changed:

- `scripts/Update-Personalization.ps1` after editing runtime bridges, `~/.gal/config/config.local.env`, or `~/.gal/config/model-roles.local.md`
- `scripts/Update-Skills.ps1` after changing `agent/` or `skills/`
- `scripts/Update-Commands.ps1` after changing `commands/*/SKILL.template.md` or `commands/*/SKILL.local.md`
- `scripts/Update-Mcp.ps1` after changing `mcp.json`, `~/.gal/config/mcp.local.json`, or MCP-related values in `~/.gal/config/config.local.env`
- `scripts/update-personalization.sh` after editing runtime bridges, `~/.gal/config/config.local.env`, or `~/.gal/config/model-roles.local.md`
- `scripts/update-skills.sh` after changing `agent/` or `skills/`
- `scripts/update-commands.sh` after changing `commands/*/SKILL.template.md` or `commands/*/SKILL.local.md`
- `scripts/update-mcp.sh` after changing `mcp.json`, `~/.gal/config/mcp.local.json`, or MCP-related values in `~/.gal/config/config.local.env`

If you changed source-of-truth content that feeds repo-local generated adapters such as `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, or `GEMINI.md`, rerun `scripts/Sync-DevContext.ps1` or `scripts/sync-dev-context.sh`. `Update-Mcp` does not regenerate those adapter files.

Windows:

```powershell
./scripts/Setup-Machine.ps1
./scripts/Setup-Machine.ps1 -Reconfigure
./scripts/Update-Personalization.ps1
./scripts/Update-Skills.ps1
./scripts/Update-Commands.ps1
./scripts/Update-Mcp.ps1
```

macOS/Linux:

```bash
./scripts/setup-machine.sh
./scripts/setup-machine.sh --reconfigure
./scripts/update-personalization.sh
./scripts/update-skills.sh
./scripts/update-commands.sh
./scripts/update-mcp.sh
```

## Provider Plugin Packaging

GAL uses a provider-neutral plugin package model. Source contracts in the repo are the single source of truth; each provider plugin is a generated artifact rendered by a provider-specific renderer.

### Common Base

The common package model carries metadata, reusable skills, command skills (as skill bundles), a canonical MCP spec, an instruction corpus, and optional agents with capability flags. It explicitly excludes:

- Provider-specific output paths
- Resolved machine-local secrets or paths
- `runtimeScripts` or plugin-root `scripts/`
- `gal-results/`
- Hooks (deferred from v1)

### AGY as Renderer 1

AGY is the first renderer, not the architecture. `Build-AgyPlugin` renders the common package into `~/.gal/dist/provider-plugins/agy/gal/` and installs to `~/.gemini/antigravity-cli/plugins/gal/`. The AGY plugin carries:

- `plugin.json` — manifest with stable `name: gal`
- `skills/` — reusable skills and command skills
- `agents/` — agent definitions
- `rules/gal.md` — combined instruction corpus
- `mcp_config.json` — MCP server configuration (plugin-root)

Setup/reinstall removes all prior GAL-managed AGY content (legacy skills directory, `GAL_ROOT` symlink, global MCP entries, prior plugin installs) before installing the clean plugin tree.

### Gemini Migration Lane

Gemini CLI is not a fifth renderer. It is an AGY migration/compatibility lane. Existing Gemini-specific cleanup and bridge logic stays in the AGY renderer concern; it does not enter the provider-neutral substrate.

### Future Renderer Sequence

After AGY and Claude renderer validation, the remaining planned renderer sequence is: Copilot CLI → Codex. Claude still has open direct-install lifecycle verification, but the renderer itself is no longer pending. No future renderer should copy the AGY layout.

## Responsibility Boundary

- Methodology and durable contracts stay in tracked repo files.
- Machine-local values stay in `*.local.*` files or runtime-owned config.
- Collaborative-tool availability belongs to machine-local setup and personalization. Collaborative-tool readiness belongs to workflow preflight through [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md).
- If a setting would create cross-machine drift, first ask whether it belongs in a source file instead of a local override.

## Read Next

- [../README.md](../README.md) for the main user entry point.
- [devguide.md](devguide.md) for maintainer-facing setup and runtime topology.
- [../scripts/scripts.md](../scripts/scripts.md) for the script inventory.
