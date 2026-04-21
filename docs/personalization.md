# Personalization

This document holds the machine-local details that do not belong on the README front page: placeholders, runtime selection, model routing, MCP overrides, Obsidian routing, working-hours settings, and when to rerun setup.

## Runtime Selection

The machine installer now persists runtime selection in `~/.gal/install-state.json`.

- `selectedRuntimes` records which machine-layer targets GAL should manage.
- `primaryRuntime` records which runtime should be treated as your default entry point.
- The GAL repo remains the single source of truth for `agent/`, `skills/`, and `commands/`. Primary runtime affects defaults and summaries, not the underlying source content.

Use setup again with `-Reconfigure` on Windows or `--reconfigure` on macOS/Linux if you want to change the selected runtimes or primary runtime.

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
| `<GAL_SKILLS>` | skills install path | helper skills that need a stable local path |
| `<TEMP_DIR>` | temp output directory | PDF and file-processing workflows |
| `<MCP_FILESYSTEM_PATHS>` | allowed root paths for the filesystem MCP server | MCP manifest merge |
| `<MCP_MEMORY_FILE_PATH>` | path to the persistent MCP memory JSON file | MCP manifest merge |
| `<CONTEXT7_API_KEY>` | Context7 API key for runtimes that require it | MCP manifest merge |

## Common Personalization Steps

### 1. Model routing

- Copy `../model-roles.example.md` to `../model-roles.local.md`.
- Change provider and model mappings only in `model-roles.local.md`.

### 2. Local secrets and paths

- Put secrets, absolute paths, and machine-specific values in `../config.local.env`.
- Do not write local values into tracked docs, command templates, or source files.

### 2a. Command skill local overlays

If you want a machine-local customization for a specific command skill that should survive `Setup-Machine`, create `commands/<command>/SKILL.local.md`.

- `SKILL.local.md` is gitignored and treated as user-owned machine-local input.
- `Setup-Machine` bakes `SKILL.template.md`, then appends `SKILL.local.md` into the generated `SKILL.md` before regenerating Gemini and Claude command files.
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
| `OBSIDIAN_GUIDE_PATH` | `99_System/Guide.md` |
| `OBSIDIAN_PRIVATE_RESEARCH_DIR` | `10_Projects/Research_Private` |
| `OBSIDIAN_DIARY_DIR` | `10_Projects/Work_Journal` |
| `OBSIDIAN_SCRATCH_DIR` | `10_Projects/Work_Journal` |
| `OBSIDIAN_ARCHIVE_DIR` | `30_Archives/Work_Journal` |
| `RESEARCH_DEFAULT_DEST` | `repo` |

### 2c. Working Hours

Working-hours enforcement is disabled by default. If you want GAL to respect your own workday boundary, configure it in `config.local.env`:

- `WORKING_HOURS_ENABLED=false` keeps all working-hours logic off.
- `WORKDAY_START` and `WORKDAY_END` describe your preferred work window.
- `WRAP_UP_TIME` starts reminders and shutdown-window behavior.
- `HARD_STOP_TIME` defines the point where agents refuse further work.

These values are machine-local preferences, not tracked repo policy.

### 3. MCP overrides

- Put machine-specific MCP differences in `../mcp-servers.local.json`.
- Keep the tracked baseline in `../mcp-servers.example.json`.

### 4. Runtime-owned config

The installed runtime configs remain user-owned even when GAL merges missing entries.

| Runtime | Typical MCP config location |
| --- | --- |
| VS Code | user `mcp.json` |
| Gemini CLI | `settings.json` under `mcpServers` |
| Codex CLI | `config.toml` under `[mcp_servers.*]` |

Claude Code is now part of the installer runtime surface for skills and commands, but its MCP merge remains deferred.

## When To Rerun Setup

Run setup again when any of these change:

- `config.local.env`
- `mcp-servers.local.json`
- any `commands/*/SKILL.local.md`
- `~/.gal/install-state.json`
- Obsidian routing paths or Guide mode
- working-hours settings
- model routing or runtime install locations
- GAL command or skill installation

Windows:

```powershell
./scripts/Setup-Machine.ps1
./scripts/Setup-Machine.ps1 -Reconfigure
```

macOS/Linux:

```bash
./scripts/setup-machine.sh
./scripts/setup-machine.sh --reconfigure
```

## Responsibility Boundary

- Methodology and durable contracts stay in tracked repo files.
- Machine-local values stay in `*.local.*` files or runtime-owned config.
- Collaborative-tool availability belongs to machine-local setup and personalization. Collaborative-tool readiness belongs to workflow preflight through [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md).
- If a setting would create cross-machine drift, first ask whether it belongs in a source file instead of a local override.

## Read Next

- [../README.md](../README.md) for the main user entry point.
- [devguide.md](devguide.md) for maintainer-facing setup and runtime topology.
- [../scripts/scripts.md](../scripts/scripts.md) for the script inventory.
