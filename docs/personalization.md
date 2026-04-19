# Personalization

This document holds the machine-local details that do not belong on the README front page: placeholders, model routing, MCP overrides, and when to rerun setup.

## Placeholders You May Need To Fill

| Placeholder | Meaning | Common use |
| --- | --- | --- |
| `<OBSIDIAN_VAULT>` | absolute path to the Obsidian vault | Obsidian agents and skills |
| `<OBSIDIAN_VAULT_NAME>` | display name of the vault | Obsidian skills |
| `<LOCAL_SEARCH_PROJECT>` | clone path for the local search project | local-first and knowledge-management skills |
| `<GAL_SKILLS>` | skills install path | helper skills that need a stable local path |
| `<TEMP_DIR>` | temp output directory | PDF and file-processing workflows |
| `<MCP_FILESYSTEM_PATHS>` | allowed root paths for the filesystem MCP server | MCP manifest merge |
| `<MCP_MEMORY_FILE_PATH>` | path to the persistent MCP memory JSON file | MCP manifest merge |
| `<CONTEXT7_API_KEY>` | Context7 API key for runtimes that require it | MCP manifest merge |
| `<OBSIDIAN_API_KEY>` | Obsidian Local REST API key | Obsidian MCP |
| `<OBSIDIAN_BASE_URL>` | Obsidian Local REST API base URL | Obsidian MCP |

## Common Personalization Steps

### 1. Model routing

- Copy `../model-roles.example.md` to `../model-roles.local.md`.
- Change provider and model mappings only in `model-roles.local.md`.

### 2. Local secrets and paths

- Put secrets, absolute paths, and machine-specific values in `../config.local.env`.
- Do not write local values into tracked docs, command templates, or source files.

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

## When To Rerun Setup

Run setup again when any of these change:

- `config.local.env`
- `mcp-servers.local.json`
- model routing or runtime install locations
- GAL command or skill installation

Windows:

```powershell
./scripts/Setup-Machine.ps1
```

macOS:

```bash
./scripts/setup-machine.sh
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
