> 🌐 **English** · [繁體中文](i18n/zh-Hant/manual.zh-Hant.md)

# GAL User Manual

Everything for running GAL on your machine: runtime selection, configuration and placeholders, model routing, MCP overrides, Obsidian routing, working hours, headless executor routing, and the backup / uninstall boundaries.

This manual owns **how you operate GAL**. The architecture behind it (codebase + `~/.gal/` structure, distribution, release lineage, provider packaging) lives once in the [developer guide](devguide.md) and is not repeated here.

## Overview

GAL runs in one of two modes, controlled by `~/.gal/config/config.json`:

- **Install mode** — for end users. No repo clone. The `gal` CLI manages its own `~/.gal/` runtime home.
- **Source mode** — for GAL contributors. A local clone plus `galRoot` + `devMode` for live overrides and repo-skill mounting.

In both modes `~/.gal/plugins/gal/` is the canonical plugin root; provider-visible targets and `~/.gal/active/<provider>/` are projections, not content owners. For the full mode contract, `~/.gal/` layout, and ownership boundaries see [developer guide → Install Mode vs Source Mode](devguide.md#install-mode-vs-source-mode) and [→ .gal Data Structure](devguide.md#gal-data-structure).

To switch modes: set `installMode` to `install` or `source` in `~/.gal/config/config.json` (in source mode also set `galRoot` and optionally `devMode`), then rerun `gal setup`.

## First-Time Setup

### Runtime Selection

The machine installer persists runtime selection in `~/.gal/install-state.json`:

- `selectedRuntimes` — which machine-layer targets GAL should manage.
- `primaryRuntime` — your default entry point (affects defaults and summaries only).

The GAL repo remains the single source of truth for `plugins/gal-core/agents/`, `plugins/gal-core/skills/`, and `plugins/gal-core/commands/`. Antigravity CLI (AGY) is the primary Google terminal runtime; GAL links each AGY surface to the canonical root at `~/.gal/plugins/gal/`. Use setup again with `-Reconfigure` (Windows) or `--reconfigure` (macOS/Linux) to change selected or primary runtimes.

The machine setup surface is split by concern; the full-sequence entry point is `gal setup`, `gal update --machine-only` refreshes machine projections, `gal sync` regenerates repo-local adapters, and `gal mcp update` is the direct MCP concern entrypoint. See [When To Rerun Setup](#when-to-rerun-setup).

### Configuration & Placeholders

Machine-local values live in `~/.gal/config/config.local.env` (secrets, absolute paths, machine-specific values). Never write local values into tracked docs, command templates, or source files.

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
| `<WORKING_HOURS_ENABLED>` | whether working-hours enforcement is active | opt-in wrap-up and hard-stop enforcement |
| `<WORKDAY_START>` / `<WORKDAY_END>` | preferred workday in `HH:MM` | Working Hours schedule / After Hours boundary |
| `<WRAP_UP_TIME>` / `<HARD_STOP_TIME>` | wrap-up and hard-stop in `HH:MM` | shutdown-window / stop-work behavior |
| `<LOCAL_SEARCH_PROJECT>` | clone path for the local search project | local-first and knowledge-management skills |
| `<GAL_ROOT>` | path to local GAL repo clone (source mode only) | source-mode contributor workflow |
| `<TEMP_DIR>` | temp output directory | PDF and file-processing workflows |
| `<MCP_FILESYSTEM_PATHS>` | allowed root paths for the filesystem MCP server | MCP manifest merge |
| `<CONTEXT7_API_KEY>` | Context7 API key for runtimes that require it | MCP merge (materialized into `~/.gal/generated/mcp/managed.json`) |

### Companion Plugins

GAL keeps `gal-core` small (control plane, golem agents, core workflows, essential conventions, a small set of GAL-owned skills). Everything else is an opt-in external companion plugin. The `default` profile installs only `gal-core`; enable companions through named profiles or explicit plugin selection in `~/.gal/config/config.json`.

| Tier | Maintained by | Auto-update | Example |
| --- | --- | --- | --- |
| `official-gal` | GAL repo / release artifacts | yes, via GAL releases | `gal-core` |
| `curated-upstream` | external upstream; GAL locks the version | controlled, per lockfile pin | `dart-lang/skills` |
| `mirrored` | external upstream, managed mirror by GAL | no unversioned copies | upstream needing a managed cache |
| `forked` | fork owner (GAL or user) | manual, with fork base tracking | a patched fork of an upstream skill |
| `local` | you, source-mode overrides only | never shared | `file://` local path |

Known companion candidates (all `curated-upstream`, opt-in): `dart-lang/skills`, `flutter/skills`, `dotnet/skills`, `anthropics/skills`, `samber/cc-skills-golang`, `twostraws/swift-agent-skills`, `kepano/obsidian-skills`, `actionbook/rust-skills`. Your selections live in `~/.gal/config/config.json`; the resolved set is in `~/.gal/state/plugins.lock.json`.

## Personalization

### Model Routing

- Copy `executor-routing.example.json` into `~/.gal/config/executor-routing.json`.
- Change role-to-executor mappings in `~/.gal/config/executor-routing.json`. Role definitions and cross-model policy are in `plugins/gal-core/workflows/coding.md`.

### Local Secrets, Paths, and Doc Language

Put secrets, absolute paths, and machine-specific values in `~/.gal/config/config.local.env`. Documentation language follows the same ownership split:

- `PLAN_LANGUAGE` (in `config.local.env`) is machine-local and optional; it sets the default output language for `docs/plans/*.md` and `docs/research/*.md` when no explicit directive is given. Resolution: explicit directive → `PLAN_LANGUAGE` → prompt-language auto-detect → fallback `en`.
- `PROJECT_LANGUAGE` (tracked project metadata) controls the canonical language for main docs.
- Translations live under `docs/i18n/<lang>/` as `<name>.<lang>.md` (see [developer guide → Documentation Conventions](devguide.md#documentation-conventions)).
- `.dev/plans/*.prompt.md` stays English-only for cross-model stability.

### Command Skill Local Overlays

For a machine-local customization of a command skill that should survive `gal setup`, create `plugins/gal-core/commands/<command>/SKILL.local.md`:

- It is gitignored and treated as user-owned machine-local input.
- `gal setup` bakes `SKILL.template.md`, then appends `SKILL.local.md` into the generated `SKILL.md`.
- Do not edit `plugins/gal-core/commands/<command>/SKILL.md` directly — it is generated and will be replaced.
- Keep `SKILL.local.md` to additional instruction content only; no second frontmatter block.

### Obsidian Routing

Obsidian support is machine-local and optional. GAL separates repo-owned state from user-owned notes; its Obsidian automation uses the built-in `obsidian` CLI. Repo-owned research stays in `docs/research/` by default; private captures and reusable knowledge route into your vault when `OBSIDIAN_VAULT` is configured. Set `OBSIDIAN_GUIDE_PATH` (with `OBSIDIAN_GUIDE_MODE=auto` or `guide`) to follow your own library rules; leave it empty or use `generic` otherwise. Vault-relative paths exclude the vault root and have no trailing slash.

| Setting | Typical value |
| --- | --- |
| `OBSIDIAN_GUIDE_PATH` | `Guide.md` |
| `OBSIDIAN_PRIVATE_RESEARCH_DIR` | `/Projects/Research_Private` |
| `OBSIDIAN_DIARY_DIR` | `/Projects/Work_Journal` |
| `OBSIDIAN_SCRATCH_DIR` | `/Projects/Work_Journal` |
| `OBSIDIAN_ARCHIVE_DIR` | `/Archives/Work_Journal` |
| `RESEARCH_DEFAULT_DEST` | `repo` |

### Working Hours

Working-hours enforcement is disabled by default. To respect your own workday boundary, configure in `~/.gal/config/config.local.env`: `WORKING_HOURS_ENABLED=false` keeps it off; `WORKDAY_START`/`WORKDAY_END` describe your window; `WRAP_UP_TIME` starts reminders and the shutdown window; `HARD_STOP_TIME` is where agents refuse further work. These are machine-local preferences, not tracked repo policy.

### xmachine Node Config

xmachine node definitions are machine-local in `~/.gal/config/xmachine.json`:

- Copy `plugins/gal-core/templates/xmachine.config.example.json` into `~/.gal/config/xmachine.json`.
- Define each work node under the top-level `nodes` object, keyed by node alias, with at least `target` and `repoPath`.
- Add `runtimeRepoPath` when the remote GAL runtime checkout differs from the target repo checkout.
- Add `repoMappings` when one node hosts multiple target repositories.
- Keep SSH targets and repo paths here, not in `config.local.env`.

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

`scripts/Test-Xmachine.ps1` reads `~/.gal/config/xmachine.json` directly, so editing the canonical file does not require rerunning setup.

### MCP Overrides

Keep the tracked GAL source in `plugins/gal-core/mcp.json`; put machine-specific MCP differences in `~/.gal/config/mcp.local.json`. Project- or database-specific MCP servers usually belong in `mcp.local.json`, not the tracked manifest.

For Playwright MCP, keep the tracked entry conservative and machine-agnostic; put local-only browser behavior (headed mode, viewport/device emulation, storage-state paths, output dirs, persistent profiles, extension/CDP wiring) in `mcp.local.json`. A local override replaces the full `args` list, so keep the safe defaults you still want (`--isolated`, `--headless`):

```json
{
  "servers": {
    "playwright": {
      "args": ["-y", "@playwright/mcp@latest", "--isolated", "--headless",
        "--storage-state", "${PLAYWRIGHT_MCP_STORAGE_STATE}",
        "--output-dir", "${PLAYWRIGHT_MCP_OUTPUT_DIR}"]
    }
  }
}
```

Example for two Postgres databases on one machine:

```json
{
  "servers": {
    "postgres-app": {
      "type": "stdio", "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": { "DATABASE_URI": "${POSTGRES_MCP_APP_URI}" }
    },
    "postgres-analytics": {
      "type": "stdio", "command": "uvx",
      "args": ["postgres-mcp", "--access-mode=restricted"],
      "env": { "DATABASE_URI": "${POSTGRES_MCP_ANALYTICS_URI}" }
    }
  }
}
```

Add matching variables to `~/.gal/config/config.local.env`; `Update-Mcp.*` merges all local server names and resolves arbitrary `${ENV_VAR}` placeholders. Installed runtime MCP configs stay user-owned: VS Code user `mcp.json`, AGY plugin-root `~/.gemini/antigravity-cli/plugins/gal/mcp_config.json`, Codex `config.toml` `[mcp_servers.*]`, Claude user-scope via `claude mcp`. Reruns overwrite only GAL-managed server names and preserve unrelated user entries. Do not commit storage-state files, persistent profiles, browser artifacts, or secret-like local files.

### Headless Executor Routing

GAL can offload pipeline phases (implement / test / review / verify) to a secondary headless CLI instead of the conversation loop, via `~/.gal/config/executor-routing.json` (read by the `gal-dispatch` bin). Run `gal update --machine-only` once to seed `executor-routing.example.json` into the local copy, then configure routing:

```json
{
  "executors": {
    "claude":   "claude-haiku-4-5-20251001",
    "codex":    "gpt-5.4-mini",
    "opencode": "opencode/minimax-m3-free",
    "copilot":  "claude-haiku-4-5-20251001",
    "agy":      "gemini-2.5-flash"
  },
  "CODER":    { "executor": "codex" },
  "TESTER":   { "executor": "opencode" },
  "REVIEWER": { "executor": "claude", "model": "claude-sonnet-4-6" },
  "VERIFIER": { "executor": "opencode" }
}
```

**`executors` block** — defines the default model for each tool. A role entry that omits `model` inherits the default from `executors[executor]`. An explicit `model` on a role always takes precedence. The bin warns in the executor log if a role has neither.

Valid executors: `claude`, `codex`, `opencode`, `copilot`, `agy`. Omit a role to keep it in the conversation loop; delete the file to disable routing (the bin then emits a `--- GAL DISPATCH ---` text fallback). Role definitions and cross-model policy (CODER≠TESTER, etc.) live in `plugins/gal-core/workflows/coding.md`.

> ⚠️ **SECURITY WARNING — bypass-permission.** Headless executor adapters invoke secondary CLIs with `--dangerously-skip-permissions` (Claude Code, OpenCode), `--allow-all` (Copilot), or `-s workspace-write` (Codex), granting **full trust** over the local filesystem and terminal — equivalent to no sandbox. Enable executor routing only on machines and in environments you fully trust, and never when the repo or agent contracts come from untrusted sources. The spec forbids the secondary CLI from running `git commit`/`git push`, but that is an instruction, not a technical enforcement.

#### Inspecting A Dispatch — Receiving And Reviewing Results

Every dispatch leaves **two** durable traces. Use the right one for the job:

**Tier 1 — GAL executor log (primary; uniform across all tools).** Each dispatch writes `<repo>/.dev/executor-logs/<epoch>-<task>-<phase>-<executor>.log`. The header records terminal state, exit code, actual model, git branch/HEAD, and the captured provider `session_id`; the `---STDOUT---` section captures the **full** provider event stream (every agent message, command execution, file change, and token usage). This is the canonical audit trail — to review *what the executor did*, read this log. It is identical in shape for all five tools, so one location serves every provider.

This aligns with the GAL memory contract: **provider-local chat history is advisory, not authoritative.** The executor log is the repo-owned, reviewable record.

**Tier 2 — provider-native session resume (secondary; for continuing/branching).** The `Dispatch:` marker and the log header record a resumable `session_id`. Use it when you want to **continue or branch** the conversation inside the provider's own UI. Each tool stores and surfaces headless sessions differently — the commands are **not** uniform:

| Executor | Native-view / resume command | Default visibility of headless session |
| --- | --- | --- |
| **claude** | `claude --resume <session_id>` | listed |
| **codex** | `codex resume <uuid>` (UUID bypasses the filter) | **hidden** — `codex exec` creates a *non-interactive* session that the `codex resume` picker hides by default; use `codex resume --include-non-interactive` (add `--all` to disable cwd filtering) to see it in the picker. Rollout file: `~/.codex/sessions/YYYY/MM/DD/rollout-…-<uuid>.jsonl` |
| **opencode** | `opencode run -s <session_id>` (continue) · `opencode export <session_id>` (dump JSON) · `opencode session list` (browse) | listed — `run` sessions appear in `opencode session list` and the TUI; cannot be hidden |
| **copilot** | `copilot --resume=<session_id>` | stored in `~/.copilot/session-store.db`; resume by id (no public list command) |
| **agy** | `agy --conversation <uuid>` | stored under `~/.gemini/antigravity-cli/brain/<uuid>/` (Windows); no list subcommand — browse by id |

**Rule of thumb:** to *audit* a dispatch, read the Tier-1 executor log (uniform, always present). To *resume* a dispatch in its native tool, use the Tier-2 command for that executor. The "hidden but resumable" behaviour you may notice with `codex` is that tool's own default, not a GAL setting; only `codex` hides headless sessions by default, and all five remain resumable by id regardless.

## Machine Operations

### When To Rerun Setup

Rerun setup when any of these change: `~/.gal/config/config.local.env`, `plugins/gal-core/mcp.json`, `~/.gal/config/mcp.local.json`, any `plugins/gal-core/commands/*/SKILL.local.md`, `~/.gal/install-state.json`, Obsidian routing/Guide mode, working-hours settings, model routing, or runtime install locations. (`~/.gal/config/xmachine.json` is read directly and needs no rerun.)

Use the narrower concern entrypoint when only one concern changed — `gal update --machine-only` (runtime bridges, `config.local.env`, `executor-routing.json`, `plugins/gal-core/agents/`, `plugins/gal-core/skills/`, `plugins/gal-core/commands/*/SKILL.*`), `gal mcp update` (`plugins/gal-core/mcp.json`, `mcp.local.json`, MCP env), or `gal sync` for repo-local adapter regeneration. `gal update --machine-only` does not regenerate repo-local adapters; `gal sync` does not touch machine MCP config.

```bash
# All platforms
gal setup
gal setup --reconfigure
gal update --machine-only
gal mcp update
gal sync
```

### Backup & Migration

When moving GAL to a new machine, preserve machine intent, not the rebuildable payload. Back up:

- `~/.gal/config/config.json`
- `~/.gal/config/xmachine.json`
- `~/.gal/state/plugins.lock.json`
- explicit local overrides and any secret sources your setup depends on

You do not need to carry forward package-managed `gal` binaries, provider plugin install trees, or `~/.gal/generated/` projections. Reinstall GAL, restore the backed-up files, then rerun setup / bootstrap refresh so GAL rebuilds the managed runtime outputs. Upgrades may refresh the payload and GAL-managed generated state but must preserve your machine intent — they must not silently switch `installMode`, clear `galRoot`, or toggle `devMode`.

### Uninstall & Purge

Default uninstall is not a reset button:

- **Package-manager uninstall** removes the packaged `gal` binary only.
- **GAL-managed uninstall** removes rebuildable GAL-owned outputs (GAL-owned provider plugin installs, canonical plugin roots under `~/.gal/plugins/`, generated projections under `~/.gal/generated/`).
- `~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, `~/.gal/state/plugins.lock.json`, explicit local overrides, and secret sources stay user-owned and are preserved.

For a true reset, use the explicit purge lane — `Uninstall-Machine -Purge -ConfirmPurge` or `uninstall-machine.sh --purge --confirm-purge`. Default uninstall must never silently delete the preserved surfaces.

## Install Status & Channels

Machine-local settings do not change GAL's release-channel contract. Which runtimes support install today, the canonical release lineage, marketplace matrix, lag windows, and fallback policy are owned by [README → Install Status](../README.md#install-status) and [developer guide → Release Artifact Matrix](devguide.md#release-artifact-matrix). The machine-local implication is narrow: preserve your machine-intent files so you can reinstall or refresh the managed payload later without losing local state.

## Responsibility Boundary

- Methodology and durable contracts stay in tracked repo files.
- Machine-local values stay in `*.local.*` files or runtime-owned config.
- Collaborative-tool availability is machine-local setup; readiness is workflow preflight via [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md).
- If a setting would create cross-machine drift, first ask whether it belongs in a source file instead of a local override.

## Read Next

- [../README.md](../README.md) — the map and what GAL is.
- [devguide.md](devguide.md) — maintainer-facing architecture, distribution, and runtime topology.
- [../scripts/scripts.md](../scripts/scripts.md) — the script inventory.
