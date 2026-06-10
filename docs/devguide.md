# Developer Guide

This document is maintainer navigation, not a second specification. Use it to decide which layer you are changing, which source files own that layer, and which rules you must not break.

## Overview & Dev Setup

To set up GAL for working on GAL itself (clone + `gal setup`), see the **[Dev Mode section in the README](../README.md#dev-mode)**. This guide does not repeat the dev-mode enablement steps; it assumes you already have a source-mode checkout.

If you cannot tell which layer you are touching, stop and resolve that first. Most broken refactors in GAL come from mixing README, docs, templates, scripts, and command contracts in one change. The `Codebase & Runtime Structure` section below is the map; the `Making Changes` section is the procedure.

## Install Mode vs Source Mode

GAL uses two distinct operational modes.

| Mode | Audience | `gal-engine` source | External plugin source | Shortcut policy |
| --- | --- | --- | --- | --- |
| Install mode | general users | Claude-compatible canonical package + provider-native install source | `~/.gal/config/config.json` + `~/.gal/state/plugins.lock.json` resolved upstream packages | provider-specific shortcuts only to `~/.gal/active/<provider>/`; repo-root shortcut forbidden |
| Source mode | GAL contributors | `~/.gal/config/config.json.galRoot` pointing to local GAL repo | `~/.gal/state/plugins.lock.json` resolved cache, or `~/.gal/config/xmachine.json` explicit local override | repo link allowed but must be marked as source mode |
| Migration cleanup | existing GAL users | existing managed surfaces | existing managed external skills | only removes GAL-managed legacy links/cache, never deletes user-owned config |
| Bridge/degraded lane | OpenCode or runtimes lacking primary install parity | GAL-managed cache/artifact | resolved package subset | only capability-level links; not treated as primary install success |

Key rules:

- `galRoot` and `devMode` are controlled by `~/.gal/config/config.json`.
- Xmachine routing is controlled by `~/.gal/config/xmachine.json`.
- `GAL_SKILLS` is no longer part of the config surface; existing values are only migration input.
- `context7ApiKey`, once rendered into `~/.gal/generated/mcp/managed.json`, is machine-local secret-bearing state — never tracked or shared.

## Codebase & Runtime Structure

This section is the single map of where everything lives. The two annotated trees below replace the old separate "find the right layer", "where information belongs", and "owning surfaces" sections — each node carries its purpose, layer, and owner.

### Codebase Data Structure

```text
Golem-Agents-Legion/
├── plugins/
│   └── gal-core/                canonical source-contract root
│       ├── commands/            public /gal command surface · source contract · changing it affects every runtime   [protected]
│       ├── conventions/         portable rules all golems follow · source contract                                   [protected]
│       ├── workflows/           workflow contracts (coding, doc-sync, research) · source contract                   [protected]
│       ├── templates/           durable repo-state templates · changing them reshapes every initialized repo        [protected]
│       ├── agents/              golem agent contracts (*.agent.md) + agents.md index · source contract
│       ├── skills/              skill bodies (skills/<name>/SKILL.md) · source contract
│       ├── mcp.json             tracked MCP manifest · source contract
│       └── opencode.json        tracked OpenCode runtime config shipped with gal-core
├── scripts/         PowerShell + Bash runtime: setup, install, build, sync · some protected*
├── docs/            documentation
│   ├── manual.md            user operations manual (canonical EN)
│   ├── devguide.md          this file — maintainer guide
│   ├── collaborative-tools/ one capability/tool contract per file (+ examples/)
│   ├── i18n/<lang>/         all translations (mirror layout, <name>.<lang>.md)
│   ├── plans/               source plans (human-readable, transient)
│   ├── research/            research outputs
│   └── structure/           dockeeper NDJSON structure map + schema
└── .dev/            repo working state
    ├── project.md           compressed project summary + Source Documents index
    ├── state.md             active plans index + session continuity
    └── plans/*.prompt.md     execution work files (mutable task memory)
```

\* Protected scripts and callers: `crates/setup/`, `Init-Repo.*`, and the adapter/setup orchestration surfaces that feed them. Touching any protected path requires an architect-reviewed plan before implementation.

### Where Information Belongs

| Information type | Right home |
| --- | --- |
| durable methodology and contracts | tracked source docs and source files |
| repo working context | `.dev/project.md` and `.dev/state.md` in the target repo |
| human-readable feature plan | `docs/plans/<plan-slug>.md` |
| machine-readable execution work file | `.dev/plans/<plan-slug>.prompt.md` |
| temporary session continuity | `### Handoff Notes` plus `.dev/state.md` |
| machine-local xmachine node config | `~/.gal/config/xmachine.json` |

If a completed plan contains knowledge that should survive, extract it back into a durable source file instead of leaving the plan as hidden long-term documentation.

### Owning Surfaces (which source files own what)

| If you are changing... | Ask first... | Read these source files |
| --- | --- | --- |
| `/gal` command surface, aliases, or dispatch | is this control-plane behavior or runtime plumbing? | [../plugins/gal-core/commands/commands.md](../plugins/gal-core/commands/commands.md), [../scripts/scripts.md](../scripts/scripts.md) |
| planning flow or optional collaborative-tool semantics | is this GAL-native planning, optional gstack behavior, or workflow teaching? | [../plugins/gal-core/commands/commands.md](../plugins/gal-core/commands/commands.md), [collaborative-tools/gstack.md](collaborative-tools/gstack.md), [../plugins/gal-core/workflows/coding.md](../plugins/gal-core/workflows/coding.md) |
| setup, install topology, baked command files, or MCP merge | is this machine-layer install or repo-layer adapter generation? | `Runtime / Setup Flow` below, [../scripts/scripts.md](../scripts/scripts.md), `crates/setup/src/lib.rs`, `crates/adapters/src/lib.rs`, `crates/cli/src/main.rs` |
| templates and plan lifecycle | which file should own this information? | [../plugins/gal-core/templates/templates.md](../plugins/gal-core/templates/templates.md), [../plugins/gal-core/workflows/coding.md](../plugins/gal-core/workflows/coding.md) |
| user-facing install copy | who owns the words users read? | [../README.md](../README.md), [manual.md](manual.md), `Release Artifact Matrix` below |
| machine-local restore, mode switching, backup | which doc owns the user's machine intent? | [manual.md](manual.md), this guide |
| release channel lineage | where is the canonical release contract? | `Release Artifact Matrix` below, [../README.md](../README.md) |
| xmachine execution behavior | is this part of the main workflow or an execution-plane extension? | [collaborative-tools/xmachine.md](collaborative-tools/xmachine.md), xmachine scripts under `scripts/` |
| Godot or graphics workflows | is this repo-wide methodology or a module-specific lane? | [collaborative-tools/godot.md](collaborative-tools/godot.md), [collaborative-tools/graphics-workflow.md](collaborative-tools/graphics-workflow.md) |

### .gal Data Structure

```text
~/.gal/                          machine-local runtime root (never the source of truth)
├── config/                      USER-OWNED machine intent — preserved across upgrade/uninstall
│   ├── config.json              machine config: personalization, profiles, galRoot, devMode, installMode
│   ├── mcp.local.json           machine-specific MCP overrides and enablement
│   ├── config.local.env         secrets and local values referenced by the manifest
│   └── xmachine.json            xmachine node aliases / machine profiles / local overrides
├── state/
│   └── plugins.lock.json        USER-OWNED resolved lockfile (deterministic, backup-safe)
├── plugins/gal/                 GAL-MANAGED canonical plugin root — the runtime content owner
├── generated/                   GAL-PRODUCED projections, rebuildable
│   ├── mcp/managed.json         resolved MCP (SECRET-BEARING, machine-local, never shared)
│   ├── xmachine/managed.json    resolved xmachine projection (non-secret)
│   └── providers/               provider projections
├── dist/                        package/conversion output, provider ledgers, dev-mode isolation
│   └── providers/<provider>/managed.json   provider lifecycle ledger
└── active/<provider>/           stable shortcut targets for AI tools (consumers never point at store paths)
```

Provider-visible projections live outside `~/.gal/` (e.g. `~/.claude/skills/gal`, `~/.copilot/installed-plugins/gal-copilot/gal`, `~/.gemini/antigravity-cli/plugins/gal`) and are aliases of the canonical root, not second sources of truth.

The four authoritative runtime files have explicit schemas below.

### Runtime File Schemas

This subsection defines the structure, required fields, optional fields, secret boundaries, precedence rules, and drift metadata for the four authoritative `~/.gal/` runtime files.

#### `~/.gal/config/config.json` — User-Managed Machine Config

Concentrates personalization, plugin/profile/provider selections, install/source mode, and `galRoot`/`devMode` in one user-owned file. This file is designed to be backed up and transferred between machines.

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `schemaVersion` | yes | integer | schema version for migration |
| `galRoot` | source-mode only | string | absolute path to local GAL repo clone |
| `devMode` | no | boolean | enables contributor tooling; defaults false |
| `obsidianVault` | no | string | absolute path to Obsidian vault |
| `obsidianVaultName` | no | string | display name of the vault |
| `obsidianGuidePath` | no | string | vault-relative path to personal Guide |
| `obsidianGuideMode` | no | string | `auto`, `guide`, or `generic` |
| `obsidianPrivateResearchDir` | no | string | vault-relative private research directory |
| `obsidianDiaryDir` | no | string | vault-relative work diary directory |
| `obsidianScratchDir` | no | string | vault-relative scratch log directory |
| `obsidianArchiveDir` | no | string | vault-relative diary archive directory |
| `researchDefaultDest` | no | string | `repo`, `private`, `knowledge`, or `none` |
| `localSearchProject` | no | string | clone path for local search project |
| `tempDir` | no | string | temp output directory |
| `mcpFilesystemPaths` | no | array | optional compatibility field; only output when filesystem MCP is present |
| `context7ApiKey` | no | string | **SECRET-BEARING** — do not share; materialized into `mcp/managed.json` |
| `workingHoursEnabled` | no | boolean | enables working-hours enforcement |
| `workdayStart` | no | string | `HH:MM` format |
| `workdayEnd` | no | string | `HH:MM` format |
| `wrapUpTime` | no | string | `HH:MM` format |
| `hardStopTime` | no | string | `HH:MM` format |
| `defaultProfile` | no | string | default profile name; defaults to `default` |
| `profiles` | no | object | map of profile name → plugin list |
| `enabledPlugins` | no | array | explicit plugin selections |
| `disabledPlugins` | no | array | explicit plugin exclusions |
| `providerSelections` | no | object | per-provider enablement |
| `installMode` | no | string | `install` or `source` |
| `updateChannel` | no | string | update policy |
| `allowAutoUpdate` | no | boolean | per-plugin auto-update override |
| `preferredProviders` | no | array | ordered provider preference |
| `userSettings` | no | object | free-form user extensions |

**Secret boundary**: `context7ApiKey` is the only secret-bearing field. Once materialized into `~/.gal/generated/mcp/managed.json`, that generated file becomes machine-local secret-bearing state — never commit or share it.

**Precedence**: `config.json` is the user-facing input. Profile selections, explicit enabled/disabled lists, and provider selections are resolved together. Explicit `enabledPlugins`/`disabledPlugins` override profile-level settings.

**Excluded fields**: `GAL_SKILLS` is intentionally removed. Existing values are migration input only and must not appear in the final schema.

#### `~/.gal/state/plugins.lock.json` — Resolved Lockfile

Deterministic machine-local resolution produced by the resolver from `plugins/catalog.json` and `~/.gal/config/config.json`. Designed to be backed up, transferred, and re-resolved.

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `schemaVersion` | yes | integer | lockfile schema version |
| `resolverVersion` | yes | string | resolver tool version that produced this lock |
| `lockTimestamp` | yes | string | ISO 8601 timestamp |
| `plugins` | yes | array | resolved plugin entries |

Each resolved plugin entry carries:

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `pluginId` | yes | string | matches catalog `pluginId` |
| `resolvedSource` | yes | object | `{type, repo, ref, path}` |
| `resolvedVersion` | yes | string | release tag, version, or commit SHA |
| `resolvedChecksum` | yes | string | actual checksum of resolved content |
| `resolvedLicense` | yes | string | confirmed license identifier |
| `resolvedComponentMap` | yes | object | confirmed available components |
| `selectedProviders` | yes | array | providers this plugin is active for |
| `selectedProfiles` | yes | array | profiles that selected this plugin |
| `installTimestamp` | yes | string | ISO 8601 when resolved and installed |

**Drift detection metadata**: each plugin entry includes `resolvedChecksum` compared against catalog `checksumPolicy`. When the lockfile checksum differs from the catalog policy's expected value, the resolver must flag drift. Local overrides (source mode only) are recorded in `~/.gal/config/xmachine.json`, not in the lockfile.

#### `~/.gal/config/xmachine.json` — Machine-Local Xmachine Binding

Machine-local binding file for xmachine routing. Not a team-shared configuration.

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `schemaVersion` | yes | integer | binding schema version |
| `defaultXmachineNode` | no | string | default work node alias |
| `xmachineNodeAliases` | no | object | map of alias → `{target, repoPath, runtimeRepoPath}` |
| `machineProfiles` | no | object | machine-specific profile overrides |
| `localPluginPaths` | no | array | source-mode local plugin path overrides |
| `providerPathOverrides` | no | object | per-provider path mapping |
| `additionalBindings` | no | object | free-form extension bindings |

**Secret boundary**: `xmachine.json` may contain SSH targets and repo paths. Treat as machine-local and do not share.

#### `~/.gal/generated/mcp/managed.json` — GAL-Produced MCP Projection

Generated file owned by GAL that replaces repo-root `mcp.local.json` in install mode. Rendered from `plugins/gal-core/mcp.json` + `~/.gal/config/mcp.local.json` boundary info with machine-local values resolved.

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `schemaVersion` | yes | integer | generated schema version |
| `generatedAt` | yes | string | ISO 8601 generation timestamp |
| `generatedBy` | yes | string | tool version that produced this file |
| `mcpServers` | yes | object | resolved MCP server configurations |
| `inputs` | no | array | resolved prompt-backed inputs |
| `_metadata` | yes | object | generation metadata |

**Server entry shape** (per server):

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `command` | conditional | string | CLI command (absent when `serverUrl` present) |
| `args` | conditional | array | CLI args (absent when `serverUrl` present) |
| `serverUrl` | conditional | string | HTTP MCP endpoint (absent when `command` present) |
| `env` | no | object | environment variables (resolved, no placeholders) |
| `headers` | no | object | HTTP headers (resolved, no placeholders) |

**Secret boundary**: `context7ApiKey` is materialized directly into `headers.CONTEXT7_API_KEY` — no `${CONTEXT7_API_KEY}` placeholder remains. This file is **secret-bearing machine-local state**. It must never enter a tracked repo, shared lockfile, or team configuration. Mark it in `.gitignore`.

**Resolved values rules**:

- `mcpFilesystemPaths` appears only when filesystem MCP is present in the resolved set; otherwise omitted.
- No runtime placeholder resolution is required — all values are fully materialized.

#### Schema Precedence Chain

```text
plugins/catalog.json (repo-tracked, authoritative catalog source)
  ↓ resolved with
~/.gal/config/config.json (user-owned, machine-local preferences)
  ↓ produces
~/.gal/state/plugins.lock.json (machine-local, deterministic, backup-safe)
  ↓ renders into
~/.gal/generated/mcp/managed.json (machine-local, secret-bearing, never shared)
~/.gal/generated/xmachine/managed.json (machine-local, non-secret)
~/.gal/generated/providers/ (machine-local provider projections)
```

`~/.gal/config/xmachine.json` is an independent leaf — it does not feed into the resolver chain but controls xmachine routing and local overrides for source mode.

### Catalog and Lockfile Architecture

GAL is a catalog + lockfile orchestrator, not a universal plugin runtime.

```text
plugins/catalog.json
  → ~/.gal/state/plugins.lock.json
  → resolved plugin set
  → provider-native install spec
  → Claude-compatible canonical package
  → provider-specific installer / renderer / shortcut mapping
```

Core layout under `~/.gal/`:

| Path | Purpose |
| --- | --- |
| `~/.gal/config/config.json` | user-managed machine config: personalization, plugin/profile/provider selections, `galRoot`, `devMode` |
| `~/.gal/config/xmachine.json` | machine-local xmachine binding: node aliases, machine profiles, local overrides |
| `~/.gal/state/plugins.lock.json` | resolved lockfile: installed sources, versions, checksums, component maps |
| `~/.gal/plugins/` | GAL-managed canonical plugin roots and companion plugin content |
| `~/.gal/generated/mcp/managed.json` | GAL-produced MCP projection, replaces repo-root `mcp.local.json` in install mode |
| `~/.gal/generated/xmachine/managed.json` | GAL-produced xmachine projection |
| `~/.gal/dist/providers/` | GAL-produced provider lifecycle ledgers and managed metadata |
| `~/.gal/active/<provider>/` | stable shortcut targets for AI tools; consumers do not point directly at store paths |

The `plugins/catalog.json` in the repo is the authoritative catalog source and metadata registry. The `~/.gal/state/plugins.lock.json` is the deterministic machine-local resolution that can be backed up, transferred, and re-resolved.

Initial external companion candidates: `dart-lang/skills`, `flutter/skills`, `dotnet/skills`, `anthropics/skills`, `samber/cc-skills-golang`, `twostraws/swift-agent-skills`, `kepano/obsidian-skills`, `actionbook/rust-skills` — all `curated-upstream`, all opt-in.

## Distribution & Release Architecture

### Runtime / Setup Flow

This subsection absorbs the setup topology that maintainers need when changing `gal setup`, `gal update --machine-only`, command installation, or MCP wiring. The high-level flows:

```text
Contributor/source path:
GAL checkout
  → gal setup
  → gal update --machine-only / gal mcp update
  → gal install
  → ~/.gal/plugins/gal/ plus provider projections

Target repo bootstrap:
target repo cwd
  → gal init
  → Init-Repo.*
  → gal sync
  → .dev/ plus generated adapters

Future package-managed path:
winget / Homebrew / GitHub Release payload
  → gal binary first launch
  → install-mode runtime contract
  → ~/.gal/ runtime roots and provider projections
```

#### Four Runtime Layers

| Layer | Location | Purpose |
| --- | --- | --- |
| Layer 1 | the GAL repo | main methodology source |
| Layer 1.5 | tool config directories such as `~/.copilot/`, `~/.gemini/`, `~/.gemini/antigravity-cli/`, `~/.codex/`, `~/.claude/`, plus `~/.gal/install-state.json` | installed skills, generated commands, runtime-facing symlinks, and machine-local runtime selection |
| Layer 2 | `<target-repo>/.dev/` | per-repo working context and state |
| Layer 3 | generated adapter files in the target repo | shared instructions and runtime-specific shims |

Naming note: upstream docs still use the full product name `Antigravity CLI` and the path segment `antigravity-cli`, but Google also exposes `AGY CLI` as the short name. In GAL-owned helper and function names, prefer `Agy` or `agy` for internal identifiers; keep `Antigravity CLI` and `antigravity-cli` for user-facing labels, runtime keys, and upstream-owned paths.

#### Cross-Runtime Surface

| Runtime | Machine-layer install | Command surface | Notes |
| --- | --- | --- | --- |
| Copilot | `~/.copilot/installed-plugins/gal-copilot/gal/` (link-first projection when possible, refreshed host copy otherwise) | plugin-provided agents and skills from the installed GAL package | plugin-only runtime; GAL should not project legacy source-mode links into `~/.copilot/agents/` or `~/.copilot/skills/`; `~/.copilot/gal/` is legacy source-mode only |
| Antigravity CLI | `~/.gemini/antigravity-cli/plugins/gal/` (plugin-root) | installed named skills via plugin | primary Google CLI runtime; installs as a provider plugin at `~/.gemini/antigravity-cli/plugins/gal/` carrying skills, agents, rules, and MCP config as a self-contained tree; AGY is renderer 1 on the common package model, not the architecture itself |
| Gemini CLI | `~/.gemini/commands/`, `~/.gemini/gal-context.md`, `~/.gemini/settings.json`, and `~/.gemini/gal/` | generated native command files plus compatibility bridges | archived compatibility runtime; keep only the remaining surfaces listed below until AGY fully replaces them |
| Codex CLI | provider-managed marketplace registration plus `gal@gal-marketplace` install target | installed plugin content via provider-native lifecycle | uses marketplace registration instead of repo-root skill links; GAL records lifecycle state even when the provider-managed read surface stays opaque |
| Claude Code | `~/.gal/plugins/gal/` canonical plugin root with provider-visible projection at `~/.claude/skills/gal/` | namespaced plugin skills and commands from plugin root | only `.claude-plugin/plugin.json` belongs inside `.claude-plugin/`; `skills/`, `commands/`, `agents/`, and `.mcp.json` stay at plugin root; the local read surface is the skills-dir projection, not `~/.claude/plugins/gal/` |

#### Layer 1.5 Install Topology

| Source in repo | Copilot target | Gemini target | Antigravity target | Codex target | Claude target |
| --- | --- | --- | --- | --- | --- |
| `agent/*.agent.md` | `~/.copilot/installed-plugins/<marketplace>/gal/agents/<name>.md` (filtered projection) | not installed | `~/.gemini/antigravity-cli/plugins/gal/agy-agents/<name>.agent.md` | not installed | `~/.gal/plugins/gal/agents/<name>.md` |
| `skills/*/` | `~/.copilot/installed-plugins/<marketplace>/gal/skills/` | imported from repo paths via `~/.gemini/gal-context.md` | `~/.gemini/antigravity-cli/plugins/gal/skills/` | `~/.agents/skills/` | `~/.gal/plugins/gal/skills/` |
| `commands/*/` | `~/.copilot/installed-plugins/<marketplace>/gal/commands/<command>.md` | `~/.gemini/commands/<command>.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/<command>/` | `~/.codex/skills/<command>/` | `~/.gal/plugins/gal/commands/<command>.md` |
| `~/.gal/` | legacy source-mode shortcut only (`~/.copilot/gal/`) | not required | not required | not required | not required |
| `~/.gal/source/` | not required | `~/.gemini/gal/` | `~/.gemini/antigravity-cli/gal/` (legacy GAL_ROOT only) | not required | not required |
| `~/.gal/plugins/gal/` | `~/.copilot/installed-plugins/gal-copilot/gal/` (plugin lifecycle projection) | not required | `~/.gemini/antigravity-cli/plugins/gal/` (plugin tree) | provider-managed marketplace delivery from the canonical artifact | `~/.claude/skills/gal/` provider-visible projection |

#### Generated Runtime Files

| Generated file | Why it exists |
| --- | --- |
| `commands/*/SKILL.md` | baked command prompt with absolute `GAL_ROOT` plus any gitignored `SKILL.local.md` overlay |
| `~/.gemini/commands/*.toml` | Gemini-native command surface generated from the baked command skill |
| `~/.gal/plugins/gal/.claude-plugin/plugin.json` | Claude plugin manifest living under the canonical plugin root for validation, install, and plugin manager metadata |
| `~/.gal/plugins/gal/.mcp.json` | Claude plugin MCP configuration containing only portable GAL-managed entries |
| `~/.gal/dist/providers/claude/managed.json` | Claude lifecycle metadata recording `canonicalRoot`, `projectionRoot`, `installTarget`, and `packageOutputRoot` |
| `~/.gemini/gal-context.md` | reusable shared skill imports for Gemini |

#### Archived Gemini CLI Surfaces

These are the remaining Gemini CLI compatibility surfaces that still exist on purpose. Treat them as archived bridges to be retired gradually as AGY reaches parity. Do not expand them unless the change is explicitly about keeping Gemini compatibility working during that transition.

| Archived surface | Owning files | Why it still exists | Expected retirement path |
| --- | --- | --- | --- |
| Gemini runtime selection, path constants, and install-state detection | `scripts/common/Common.ps1`, `scripts/common/common.sh` | Setup still needs to detect and manage Gemini-specific compatibility outputs such as `~/.gemini/commands/`, `~/.gemini/settings.json`, `~/.gemini/gal-context.md`, and `~/.gemini/gal/`. | Remove once no GAL-managed Gemini install target remains. |
| Gemini native command generation | `crates/adapters/src/lib.rs` via `gal update --machine-only` | GAL still bakes `plugins/gal-core/commands/*/SKILL.md` into `~/.gemini/commands/*.toml` for the legacy Gemini native slash-command surface. | Replace when AGY skill or plugin surfaces are the only Google command entry point GAL supports. |
| Gemini shared-skill context bridge | `crates/adapters/src/lib.rs` via `gal update --machine-only` | `~/.gemini/gal-context.md` still imports repo skills for Gemini compatibility. | Remove when Gemini no longer needs repo-skill imports for GAL. |
| Gemini settings.json bridge | `crates/adapters/src/lib.rs` via `gal update --machine-only` | `~/.gemini/settings.json` still gets `AGENTS.md` and `GEMINI.md` in `context.fileName` for legacy Google-runtime loading. | Remove when Google-side loading is fully owned by AGY runtime surfaces instead of Gemini settings. |
| Gemini `GAL_ROOT` link and legacy skill cleanup | `crates/adapters/src/lib.rs` via `gal update --machine-only` | GAL still manages `~/.gemini/gal/` and cleans old GAL-managed `~/.gemini/skills/*` remnants during migration. | Remove when no Gemini runtime path needs a stable repo link and no legacy cleanup is needed. |
| Legacy Gemini MCP cleanup | `scripts/Update-Mcp.ps1`, `scripts/update-mcp.sh` | Gemini is no longer the MCP owner, but GAL still removes old Gemini MCP entries from `~/.gemini/settings.json` so AGY MCP ownership stays clean. | Remove after legacy Gemini MCP residue no longer exists in supported installs. |
| `GEMINI.md` generated adapter filename | `crates/adapters/src/lib.rs` via `gal sync` / `gal init-repo` | The repo still emits `GEMINI.md` as a Google-runtime compatibility adapter filename even though AGY is the primary Google CLI runtime. | Rename or remove only when Google-runtime consumers no longer depend on the `GEMINI.md` carrier. |
| xmachine Gemini headless execution lane | `scripts/Start-xMachine.ps1`, `scripts/Start-xMachine.sh` | xmachine remote execution still invokes Gemini CLI headlessly and maps Gemini exit codes. | Replace when xmachine is migrated to AGY or another runtime end-to-end. |

If you are removing one of these archived surfaces, also audit the matching maintainer guidance in [../scripts/scripts.md](../scripts/scripts.md) and [manual.md](manual.md) so the docs stop describing a retired bridge.

#### Install-State

The installer persists machine-local runtime selection in `~/.gal/install-state.json`.

- `selectedRuntimes` controls which machine-layer targets GAL should manage.
- `primaryRuntime` controls defaults and summaries only.
- The tracked GAL repo remains the primary source for agents, skills, and commands.

#### MCP Management

The MCP manifest is a separate install concern from skills.

| File | Scope | Role |
| --- | --- | --- |
| `plugins/gal-core/mcp.json` | tracked | single GAL MCP source of truth |
| `~/.gal/config/mcp.local.json` | local only | machine-specific overrides and enablement |
| `~/.gal/config/config.local.env` | local only | secrets and local values referenced by the manifest |
| `~/.gal/config/xmachine.json` | local only | machine-local xmachine node definitions keyed by work-node alias |

The merged MCP manifest is centered on `servers` and may also include optional top-level `inputs` when a runtime supports prompt-backed values such as a PAT entry.

For Playwright MCP specifically:

- Keep `plugins/gal-core/mcp.json` limited to the tracked safe startup contract: canonical `playwright` key plus conservative core flags such as `--isolated` and `--headless`.
- Put headed mode, viewport or device emulation, storage-state paths, output directories, optional capability flags, persistent profile paths, extension or CDP wiring, and similar machine-local behavior in `~/.gal/config/mcp.local.json`.
- Put secret-like paths or environment-backed local values referenced by those overrides in `~/.gal/config/config.local.env`.
- Do not track browser artifacts, storage-state files, persistent profile directories, or secret files in the repo.

`Update-Mcp.ps1` and `update-mcp.sh` use the tracked manifest as the source of truth for GAL-managed server names:

- VS Code: overwrite tracked server entries inside user `mcp.json`
- Antigravity CLI: write GAL-managed MCP to plugin-root `~/.gemini/antigravity-cli/plugins/gal/mcp_config.json` under `mcpServers`; the global `~/.gemini/antigravity-cli/mcp_config.json` is only touched for legacy cleanup of old GAL-managed entries
- JSON-based runtime bridges also preserve managed top-level `inputs` entries by input `id` when the merged manifest includes them.
- Codex CLI: regenerate tracked `[mcp_servers.*]` sections inside `config.toml`
- Claude Code: remove and re-add tracked user-scope servers through the `claude mcp` CLI

Provider-owned config still stays user-owned. GAL only takes ownership of the server names declared in the tracked manifest, preserves unrelated user-defined entries, and removes the GAL-managed legacy Gemini MCP names previously written into `settings.json`.

#### Why `GAL_ROOT` Exists

`~/.copilot/gal/` is now a legacy source-mode shortcut that points at `~/.gal/` and is classified by the provider doctor as a cleanup candidate in install mode. `~/.gemini/gal/` remains a legacy Gemini compatibility link back to `~/.gal/source/` until the Gemini bridge is retired.

For AGY, the plugin tree at `~/.gemini/antigravity-cli/plugins/gal/` replaces the old `~/.gemini/antigravity-cli/gal/` symlink as the managed install surface. The plugin is self-contained and does not require an external `GAL_ROOT` symlink; setup removes the legacy `GAL_ROOT` symlink during pre-cleanup.

### Ownership Boundaries

GAL explicitly separates how the CLI is installed (Bootstrap Installer) from how provider plugins are managed (Install-Mode Plugin Distribution). This boundary ensures that package managers do not overwrite user data and that GAL plugins can update independently of the CLI payload.

- `gal setup` is the development and packaging harness entry. It should mirror the install-mode rules, but it is not the final end-user package payload.
- Package managers own only the package-managed `gal` binary payload.
- GAL owns rebuildable runtime outputs such as provider projections, generated MCP state, generated xmachine state, and the canonical plugin root.
- GAL also owns the per-provider ledgers under `~/.gal/dist/providers/<provider>/managed.json`; provider directories outside provable GAL-managed projections remain host-owned or user-owned.
- Users own `~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, `~/.gal/state/plugins.lock.json`, explicit local overrides, and secret sources.
- `~/.gal/dist/` is package output, conversion output, managed metadata, or dev-mode isolation. It is not the runtime source of truth.
- Install mode must not depend on repo-root links, baked local checkout paths, or hidden `GAL_ROOT` assumptions.

#### Bootstrap Installer Distribution

- **What it is**: How the GAL CLI (`gal` executable) gets onto the user's machine.
- **Channels**: `winget` (Windows), `homebrew` (macOS/Linux), and GitHub Releases `.zip` / `.tar.gz` (manual fallbacks). See the [Release Artifact Matrix](#release-artifact-matrix) below for the exact artifact lineage.
- **Ownership**: The package manager owns the **package-managed payload** (the single executable binary). It handles upgrades and removals of the CLI itself, but it must **never** manage or delete `~/.gal/` contents.

#### Bootstrap Runtime Contract

After the package-managed `gal` binary is on disk, the first launch contract is:

1. Expose a runnable `gal` CLI from the package-managed install location.
2. Detect whether `~/.gal/config/config.json` already exists.
3. If no machine config exists, seed a minimal install-mode config and create the managed runtime roots under `~/.gal/`.
4. Resolve the default profile into `~/.gal/state/plugins.lock.json`.
5. Render provider-native projections under `~/.gal/generated/` and any required stable targets under `~/.gal/active/<provider>/`.
6. Hand off to install mode by default for bootstrap installs; source mode only begins after the user explicitly sets `installMode=source` plus `galRoot` and `devMode`.

The canonical end-user bootstrap contract must work without a cloned repo. Repo-local workflow state such as `.dev/`, `docs/plans/`, or repo-root skill links is never a first-launch requirement for the installed package payload. The Rust `gal setup` orchestrator is the development and packaging harness that should mirror the same install-mode-first branching and `~/.gal/` ownership rules, but they are not themselves the final end-user package payload.

#### First-Launch Branching Rules

The install-mode/source-mode split happens only after GAL has a machine config to read:

- **Bootstrap install with no existing machine config**: seed `installMode=install`, leave contributor-only repo bindings disabled, and build provider-native projections from `~/.gal/`.
- **Existing machine config with `installMode=install`**: reuse `~/.gal/` and refresh lockfile plus projections without creating repo-root links.
- **Existing machine config with `installMode=source`**: reuse the explicit `galRoot` and `devMode` settings, then allow contributor-only source links and local overrides.

This keeps package-managed first launch safe for end users while preserving an explicit contributor path.

#### Upgrade Contract

Bootstrap upgrades must rerun the same install-mode-first contract without widening ownership.

- **`winget upgrade` / `brew upgrade`**: replace only the package-managed `gal` binary, then let the refreshed CLI re-enter the bootstrap runtime contract. The refresh may regenerate `~/.gal/generated/`, refresh provider projections, and update `~/.gal/state/plugins.lock.json` when the resolved profile changes, but it must not overwrite `~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, explicit local overrides, or secret sources.
- **Provider-native direct-install or direct-update lanes**: if a marketplace lane is later verified to support canonical install and update, that lane still behaves like a bootstrap payload upgrade rather than a runtime reset. It may replace the packaged GAL payload and re-run GAL-managed projection refresh, but it must preserve user-owned config and keep install mode versus source mode unchanged unless the user edits machine config explicitly.
- **Manual archive refresh**: replacing the extracted `gal` binary from a GitHub Releases `.zip` or `.tar.gz` is allowed only as a payload swap. Users may rerun the bootstrap entrypoint afterward to refresh lockfile and generated projections, but manual archive updates must not delete or reset existing `~/.gal/config/*`, xmachine bindings, local overrides, or secrets.
- **Schema or runtime migrations**: when an upgrade needs to evolve GAL-managed runtime state, migrations must be additive or explicitly reversible. They may rewrite GAL-owned generated or cached state, but they must not silently migrate user-owned config into a new location or clear values that the user would need to reconstruct manually.

The decisive rule is simple: upgrades may refresh the payload, the lockfile, and GAL-managed generated state, but they must preserve the user's machine intent.

#### Uninstall Contract

Default uninstall must stay narrower than a machine reset.

- **Package-manager uninstall**: removes only the package-managed `gal` payload. It must not delete `~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, `~/.gal/state/plugins.lock.json`, explicit local overrides, or secret-bearing sources.
- **GAL-managed uninstall**: removes GAL-owned runtime outputs that can be rebuilt, including provider-native plugin install targets that GAL owns, the canonical plugin roots under `~/.gal/plugins/`, and generated projections or ledgers under `~/.gal/generated/` and `~/.gal/dist/providers/`.
- **Preserved surfaces**: uninstall keeps the user's machine intent intact. That includes install/source mode choice, `galRoot`, `devMode`, xmachine bindings, lockfile state, explicit local overrides, and secret sources.
- **No implicit second runtime**: uninstall must not leave behind a second GAL-managed runtime tree that the next install would treat as authoritative. Rebuildable GAL-owned runtime outputs are removed; preserved user-owned config remains as input for the next install.

#### Purge And Reset Boundary

Purge or reset is a separate destructive lane, not part of default uninstall.

- The explicit purge entrypoint is the Rust uninstall flow, and it must remain opt-in, visible, and dry-runnable before destructive execution.
- Purge/reset may remove preserved machine-local intent such as `config.json`, `xmachine.json`, lockfile state, local overrides, install-state metadata, and secret-bearing generated surfaces, but only after an explicit destructive confirmation step.
- Neither package-manager uninstall nor default GAL-managed uninstall may simulate purge/reset by deleting preserved surfaces automatically.
- The practical rule is simple: uninstall removes what GAL can safely rebuild; purge/reset removes what the user would otherwise need to carry forward.

#### Install-Mode Plugin Distribution

- **What it is**: How provider-native plugins (like Claude, AGY, Copilot plugins) are resolved, installed, and updated.
- **Channels**: Provider-native marketplaces, driven by the `plugins/catalog.json` and resolved into `~/.gal/state/plugins.lock.json`.
- **Ownership**: GAL owns the plugin catalog and resolution. It manages provider-specific installation paths and the `~/.gal/` plugin store.

#### State Ownership Boundaries in `~/.gal/`

When a package manager uninstalls or upgrades the GAL CLI, it must respect these boundaries:

- **Package-Managed Payload**: The `gal` binary. Owned by `winget` / `homebrew`. Can be safely deleted during uninstall.
- **GAL-Managed Runtime and Generated State**: canonical plugin roots under `~/.gal/plugins/`, generated MCP and xmachine state, and per-provider ledgers under `~/.gal/dist/providers/`, plus provider-native plugin installations that GAL explicitly owns. These can be safely regenerated or reinstalled if the CLI is rebuilt on a new machine. They are removed during a GAL-managed uninstall.
- **User-Owned Config and State**: `~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, `~/.gal/state/plugins.lock.json`, explicit user-authored local overrides, and secrets. Owned by the user. Must be preserved during any package-manager uninstall or default GAL-managed uninstall. Full deletion requires an explicit, destructive purge flow.

During upgrade, treat `~/.gal/config/config.json`, `~/.gal/config/xmachine.json`, explicit local overrides, and secret-bearing sources as read-preserve surfaces. The installer may read them to determine install mode, provider selection, or migration steps, but it must not replace them with defaults merely because a newer bootstrap payload was installed.

### Provider Plugin Packaging

GAL uses a provider-neutral plugin package model. Source contracts in the repo are the single source of truth; each provider plugin is a generated artifact rendered by a provider-specific renderer.

The runtime content owner for GAL's plugin-shaped providers is `~/.gal/plugins/gal/`. `~/.gal/dist/` is intentionally downgraded to package output, conversion output, managed provider metadata, and dev-mode `~/.gal/dist/commits/` isolation. Provider-visible targets and active shortcuts may point at the canonical root or a documented projection, but they do not become a second source of truth.

#### Common Package Model

The provider-neutral package model carried by the Rust install/render path (`crates/gal-engine/src/render.rs` and `crates/gal-engine/src/install.rs`) carries:

| Field | Source | Shared across all four providers? |
| --- | --- | --- |
| `metadata` | repo name, display name, version diagnostics, generation timestamp | conceptually yes |
| `skills` | `skills/<name>/SKILL.md` | yes |
| `commandSkills` | `commands/*/SKILL.md` | yes, as skill bundles |
| `mcpSpec` | `mcp.json` plus `~/.gal/config/mcp.local.json` boundary info | conceptually yes, but resolved local values stay out |
| `instructionCorpus` | `.dev/project.md`, required conventions, workflows, generated indexes | content yes, path no |
| `agents` | `agent/*.agent.md` | optional; projected to three of four providers |

The common model explicitly excludes: provider-specific output paths, resolved machine-local secrets or paths, `runtimeScripts`, plugin-root `scripts/`, `.tmp/gal-results/`, and hooks (deferred from v1).

#### Provider Lane Policy

GAL targets are classified by provider-native install capability, not by a uniform renderer model.

| Lane | Platform | Role | Success criteria |
| --- | --- | --- | --- |
| Primary install target (canonical) | Claude Code | canonical schema / canonical renderer / capability-sensitive lifecycle target | strict artifact validation plus session-load smoke are implemented; direct install, update, and uninstall remain contingent on documented provider-native CLI support |
| Primary install target (near-parity) | Copilot CLI | Claude-compatible structure; `agents/`, `skills/`, `hooks.json`, `.mcp.json`, `lsp.json` share the same directory conventions as Claude | `/plugin install`, marketplace, GitHub/Git URL/local path; directory layout already aligns with Claude, no shortcut needed |
| Primary install target | Codex | Claude baseline mapped renderer / marketplace target | `codex plugin install`, documented marketplace / cache install path, some components readable natively |
| Primary install target (shortcut) | AGY CLI / Antigravity CLI | Claude baseline mapped renderer + catalog-aware install target | provider-native plugin staging / install, or capability shortcut pointing to `~/.gal/active/agy/`; no repo-root shortcut dependency |
| Migration lane | Gemini CLI | legacy cleanup and compatibility only | not a fifth renderer; only cleanup or migration of existing GAL-managed Gemini surfaces |
| Bridge lane | OpenCode | deferred independent bridge plan | can read catalog/lockfile, mount `~/.gal/active/opencode/` capability shortcut, but does not promise primary install parity |
| Deferred / unsupported | other runtimes | out of scope | no documented install or skill discovery pathway |

Shortcut policy:

- Claude-compatible canonical package is the single source of truth.
- Copilot CLI can consume Claude-compatible structure natively and does not need a shortcut.
- AGY CLI and OpenCode, lacking native `plugin install` CLI, may use `~/.gal/active/<provider>/` as GAL-managed stable targets for capability-level shortcut redirection.
- Install mode forbids repo-root shortcuts, baked source paths, and hidden `GAL_ROOT` dependencies.
- Capability-level links in install mode must only point to `~/.gal/active/<provider>/` or its GAL-managed projection; source mode may point to user-specified local overrides.
- Bridge lane must not imply primary provider-native install parity to users.

#### AGY Surface Projection (Implementation Status)

AGY is a consumer of the canonical root, not a renderer. The provider-neutral Rust renderer emits AGY's entry-point markers directly into `~/.gal/plugins/gal/` (the canonical content owner), and install orchestration projects that root to all three AGY surfaces: CLI junction (`~/.gemini/antigravity-cli/plugins/gal`), IDE junction (`~/.gemini/antigravity-ide/plugins/gal`), and the Antigravity 2.0 GUI host-managed copy (`~/.gemini/config/plugins/gal` via `agy plugin install`). `~/.gal/active/agy/` is used only as a stable alias when a capability shortcut is needed. The AGY-facing markers are `plugin.json`, `skills/`, `agy-agents/`, `rules/gal.md`, and `mcp_config.json`. The Copilot/Claude-facing `agents/` directory now contains only filtered `.md` agent files so Copilot `setagent` does not surface duplicate names. The core renderer does not generate `hooks.json`, `scripts/`, marketplace metadata, provider stubs, or `.tmp/gal-results/`.

Setup/reinstall removes all prior GAL-managed AGY content (legacy skills directory, `GAL_ROOT` symlink, global MCP entries, prior plugin installs) before re-projecting the clean canonical root. The earlier per-provider AGY dist tree (`~/.gal/dist/provider-plugins/agy/gal/`) has been retired now that all three surfaces resolve to the canonical root.

Current implementation status:

- AGY, Claude, Copilot, and Codex all have concrete install-mode lifecycle slices plus per-provider `managed.json` ledgers under `~/.gal/dist/providers/`.
- Claude Code now projects the canonical plugin root through `~/.claude/skills/gal/`, keeps lifecycle-state tracking plus legacy projection cleanup, and uses CLI validation when the local `claude` binary exposes it.
- Copilot prefers a link-first projection at `~/.copilot/installed-plugins/gal-copilot/gal/` and falls back to `refreshed-copy2-host` with a manifest-version bump when host-copy refresh is required.
- Codex now owns its marketplace registration/install lifecycle independently of AGY, while still recording `unprojected-artifact` when the provider-managed read surface remains opaque.
- Bootstrap packaging, official install channels, and marketplace discoverability do not belong to this install-mode surface; they are handled by the separate bootstrap-installer planning track.

#### Future Renderer Sequence

The core renderer sequence is complete for AGY, Claude, Copilot, and Codex. Remaining follow-up work lives in bootstrap packaging, binary delivery, and documentation alignment rather than adding another core-provider renderer.

### Plugin Support Tiers

| Tier | Content source | GAL responsibility | Update strategy |
| --- | --- | --- | --- |
| `official-gal` | GAL repo / GAL release artifact | GAL maintains content, testing, installation, and regression | updated directly by GAL releases |
| `curated-upstream` | external upstream (e.g. `dart-lang/skills`) | GAL verifies metadata, provider compatibility, default profile, and lockfile; content maintained by upstream | updated per lockfile pin; manual or controlled updates allowed |
| `mirrored` | managed mirror of external upstream | GAL responsible for provenance, license, checksum, and mirror drift | no unversioned copies; updates require drift check |
| `forked` | fork maintained by GAL or user | fork owner responsible for divergence and fixes | must record fork base, diff policy, and update strategy |
| `local` | `file://` or local path override | source mode / contributor override only | does not enter shareable lockfile; recorded as machine-local override |

Default profile: initial `default` profile installs only `gal-core`. All companion plugins are opt-in through named profiles or explicit plugin selection.

### Bootstrap Distribution Documentation Ownership

When you change bootstrap installer behavior or release policy, keep the document roles separate:

- `README.md` explains the user-facing install paths (the compressed map): plugin/install path, dev mode, install status, and where to go for detail.
- `manual.md` owns the machine-local restore boundary and detailed user operations: what the user carries forward, what GAL regenerates, backup/migration/uninstall steps, and how install mode versus source mode affect migration.
- The [Release Artifact Matrix](#release-artifact-matrix) section below owns the detailed canonical release lineage, marketplace matrix, submission rules, lag windows, and fallback copy.

Do not duplicate the full release matrix elsewhere. Point readers to the owning home, then keep each file focused on its single job.

Bootstrap-distribution guardrails:

- GitHub Releases is the canonical version source.
- `winget`, `homebrew`, and provider marketplace entries must all map back to that same release lineage.
- Package-manager uninstall and GAL-managed uninstall must preserve user-owned machine intent.
- Provider marketplaces remain discoverability-first until provider-native direct install, update, and uninstall are verified end to end; renderer-only or validation-only progress is not enough.
- Lag between downstream channels is expected; the user-facing fallback remains GitHub Releases.

### Release Artifact Matrix

This section defines the canonical release lineage for the `gal` CLI package-managed payload and the downstream publication contract for `winget`, Homebrew, and manual fallback archives.

GitHub Releases is the single source of truth. Every supported install channel must consume the same versioned single-binary lineage, and every fallback archive must wrap the exact canonical binary for that platform plus `LICENSE` only.

#### Canonical Asset Naming

Release tag placeholder: `<version>` means the exact GitHub Release tag, for example `v1.2.3`.

For every release, publish these canonical binary assets directly on GitHub Releases:

| OS | Architecture | Canonical binary asset | Supported install command | Downstream consumer |
| --- | --- | --- | --- | --- |
| Windows | x64 | `gal-<version>-windows-x64.exe` | `winget install Monkey1Wizard.GAL` | `winget`, manual download |
| Windows | arm64 | `gal-<version>-windows-arm64.exe` | `winget install Monkey1Wizard.GAL` | `winget`, manual download |
| macOS | x64 | `gal-<version>-darwin-x64` | `brew install monkey1wizard/tap/gal` | Homebrew, manual download |
| macOS | arm64 | `gal-<version>-darwin-arm64` | `brew install monkey1wizard/tap/gal` | Homebrew, manual download |
| Linux | x64 | `gal-<version>-linux-x64` | `brew install monkey1wizard/tap/gal` | Homebrew, manual download |
| Linux | arm64 | `gal-<version>-linux-arm64` | `brew install monkey1wizard/tap/gal` | Homebrew, manual download |

Rules:

1. The canonical binary asset name must include the exact release tag and target platform/architecture.
2. Manual fallback archives must contain the same canonical binary filename shown above, not a renamed generic `gal` placeholder.
3. Package managers may rename the installed file to `gal` in the target `bin/` directory during installation, but they may not invent a second binary lineage.

#### Fallback Archive Contract

Fallback archives exist for manual install and locked-down environments that cannot use `winget` or Homebrew.

Archive contents are strictly limited to the platform's canonical binary asset plus `LICENSE`.

| Target | Archive asset | Required contents |
| --- | --- | --- |
| Windows x64 | `gal-<version>-windows-x64.zip` | `gal-<version>-windows-x64.exe`, `LICENSE` |
| Windows arm64 | `gal-<version>-windows-arm64.zip` | `gal-<version>-windows-arm64.exe`, `LICENSE` |
| macOS x64 | `gal-<version>-darwin-x64.tar.gz` | `gal-<version>-darwin-x64`, `LICENSE` |
| macOS arm64 | `gal-<version>-darwin-arm64.tar.gz` | `gal-<version>-darwin-arm64`, `LICENSE` |
| Linux x64 | `gal-<version>-linux-x64.tar.gz` | `gal-<version>-linux-x64`, `LICENSE` |
| Linux arm64 | `gal-<version>-linux-arm64.tar.gz` | `gal-<version>-linux-arm64`, `LICENSE` |

Excluded from all archives:

- `~/.gal/` directory structures or presets
- default `config.json` files
- pre-resolved plugin lockfiles
- repository-local state such as `.dev/`, `docs/`, `commands/`, or `skills/`

The bootstrap binary owns first-run initialization of `~/.gal/`; release artifacts do not pre-seed that runtime state.

Release packaging also does not define GAL's runtime source of truth. The machine-local canonical plugin root remains `~/.gal/plugins/gal/`, while `~/.gal/dist/` is reserved for package output, managed metadata, conversion output, and dev-mode `~/.gal/dist/commits/` isolation.

#### Required Release Metadata and Provenance

Every canonical release must publish the following companion files:

| File | Requirement | Purpose |
| --- | --- | --- |
| `checksums.txt` | required | SHA-256 hashes for every canonical binary and fallback archive asset |
| `checksums.txt.sig` | required | Signature over `checksums.txt` using the project's chosen signing system (GPG or sigstore) |
| `artifact-manifest.json` | required | Machine-readable mapping of release tag, asset names, sha256 values, platform, architecture, and source build provenance |
| `LICENSE` | required | MIT license text included alongside artifacts and inside fallback archives |

`artifact-manifest.json` must record, at minimum: `version`, `publishedAt`, `assets[]`, `assets[].name`, `assets[].platform`, `assets[].architecture`, `assets[].sha256`, `assets[].kind` with `binary` or `archive`, and `assets[].contains` for archive assets.

#### Package Manager Publication Spec

Package managers are downstream consumers of GitHub Releases. They do not build from source and they do not own `~/.gal/`.

**Winget publication spec** — canonical publication target:

| Field | Required value or rule |
| --- | --- |
| `PackageIdentifier` | `Monkey1Wizard.GAL` |
| `PackageName` | `GAL` |
| `Moniker` | `gal` |
| `Publisher` | `monkey1wizard` |
| `License` | `MIT` |
| `PackageVersion` | exact GitHub Release tag without rewriting semantics |
| `Commands` | must expose `gal` |
| `Installers` | one row per supported Windows architecture |
| `InstallerUrl` | must point to the matching GitHub Release archive asset |
| `InstallerSha256` | must match `checksums.txt` for that archive asset |

Submission rules: (1) `winget` must consume `gal-<version>-windows-x64.zip` and `gal-<version>-windows-arm64.zip` from GitHub Releases. (2) The manifest must not trigger a source build, download a repo snapshot, or inject bootstrap state into `~/.gal/`. (3) If `winget` review lags the GitHub Release, the manifest description must keep GitHub Releases as the canonical fallback.

**Homebrew publication spec** — canonical publication target:

| Field | Required value or rule |
| --- | --- |
| Tap | `monkey1wizard/tap` |
| Formula name | `gal` |
| `desc` | must describe GAL as the bootstrap CLI |
| `homepage` | GitHub repository URL |
| `version` | exact GitHub Release tag |
| `license` | `MIT` |
| `url` | platform-specific GitHub Release archive asset |
| `sha256` | must match `checksums.txt` for that archive asset |
| `def install` | must install the canonical archived binary as `bin/gal` |
| `test do` | must execute `gal --version` or equivalent lightweight version check |

Submission rules: (1) Homebrew must consume the `.tar.gz` archive matching the current platform and architecture. (2) The formula must rename the archived canonical binary to `gal` only at install time via `bin.install`. (3) The formula must not synthesize extra files under `~/.gal/`, and it must not treat generated runtime state as part of the package payload.

#### Release Governance and Drift Policy

**Canonical version source**: The GitHub Releases page for `monkey1wizard/golem-agents-legion` is the sole canonical version source. All package-manager manifests and provider-marketplace entries are downstream wrappers over that same lineage.

**Publish order**: (1) Build the canonical binaries and fallback archives. (2) Publish the GitHub Release with binaries, archives, `checksums.txt`, `checksums.txt.sig`, and `artifact-manifest.json`. (3) Update `winget` and Homebrew manifests to the new release assets and hashes. (4) Update provider-marketplace metadata or wrappers after package-manager publication is queued.

**Lag tolerance** — accepted downstream lag windows:

| Channel | Target window from GitHub Release publication | Escalation point |
| --- | --- | --- |
| `winget` | within 1 business day | more than 3 business days behind canonical release |
| Homebrew | within 1 business day | more than 3 business days behind canonical release |
| Provider marketplaces | best effort within 3 business days | more than 5 business days behind canonical release |

GAL does not guarantee same-day parity across every downstream channel. If a downstream channel is outside its target window, GitHub Releases remains the official fallback and the lag must be called out in channel copy.

**Verification commands** — run these checks for every release:

| Check | Command | Expected result |
| --- | --- | --- |
| Windows package metadata | `winget show --id Monkey1Wizard.GAL --exact` | reported version matches the current canonical release tag |
| Homebrew metadata | `brew info monkey1wizard/tap/gal` | reported version matches the current canonical release tag |
| Release packaging dry run | `gal release --dry-run --version <version> --output-dir <dir>` | emits `checksums.txt` and `artifact-manifest.json` for the canonical release asset matrix |
| Release hash audit | compare `checksums.txt` with `artifact-manifest.json` | every published asset appears once with the same SHA-256 |

**Fallback messaging**: every downstream channel that may lag must carry this guidance verbatim or with only minor style edits:

> If this channel is behind the latest canonical GAL release because of review or publish latency, install the newest version directly from GitHub Releases.

#### Claude Marketplace Baseline

> **SUPERSEDED (2026-06-09):** The official `claude plugin marketplace add` + `claude plugin install` flow is no longer the primary install mechanism for Claude skills. GAL now uses the **GAL-owned skill surface** `~/.claude/skills/gal` (symlink → canonical root) as the live read surface. Run `gal install` to converge; verify with `gal doctor` and `scripts/test-install-acceptance.sh`. See [GAL-Owned Live Read Surfaces](#gal-owned-live-read-surfaces-per-provider) for the governing table.
>
> The prior local marketplace install is noted for history: `claude plugin marketplace add --scope user ~/.gal/plugins` + `claude plugin install gal --scope user` installed `gal@gal` v1.0.0 into the versioned cache at `~/.claude/plugins/cache/gal/gal/1.0.0/`. This cache is now **stale** (lacks `doc-sync`, `golem-dockeeper`, and all content added since 2026-05-30). It is superseded by the `~/.claude/skills/gal` projection. Do not remove the stale cache manually; it is orphaned once the skills surface is authoritative.
>
> The public Claude marketplace / community submission path remains deferred. It is a future follow-on after the current GAL-owned skill surface is stable.

Provider marketplaces remain downstream wrappers over the canonical GitHub Release lineage. They are official discoverability surfaces by default. Marketplace submission and direct-install promotion gates are preserved below for when that work is scheduled.

**Marketplace promotion criteria (future):** reclassify from discoverability-only to direct-install only after: (1) Claude renderer and strict validation lane verified against the canonical package lineage; (2) install, update, and uninstall behavior verified end-to-end via provider-native lifecycle; (3) the same GitHub Release tag is visible through both GitHub Releases and the Claude marketplace entry.

#### Codex and Copilot Marketplace Publication Matrix

After the Claude baseline is established, downstream provider marketplaces must still map back to the same canonical package lineage and GitHub Release tag. Codex and Copilot entries are allowed to use provider-specific wrappers or submission metadata, but they must not introduce provider-only package shapes, provider-only version streams, or direct-install claims ahead of lifecycle verification.

| Provider | Current classification | Direct-install eligibility |
| --- | --- | --- |
| Claude Code | discoverability-only (GAL-owned skill surface is the current live install path) | deferred pending GAL-owned surface stability + marketplace verification |
| Codex | discoverability-only | not yet eligible; renderer and end-to-end lifecycle verification still missing |
| Copilot | discoverability-only | not yet eligible; renderer and end-to-end lifecycle verification still missing |

#### AI Tool Integration Status

This subsection records the verified integration status across Claude Code, Claude Desktop, and Antigravity CLI. Updated: 2026-06-09.

| Target | GAL provider | Install mechanism | Loadable components | Status |
| --- | --- | --- | --- | --- |
| **Claude Code (CLI) — skills** | GAL-owned skill surface | `gal install` → `~/.claude/skills/gal` → `~/.gal/plugins/gal` (symlink/junction) | skills / commands (auto-scanned from `~/.claude/skills/`) | ⚠ **skill surface not yet created on this machine** — requires `gal install` in a new session after T-003 binary is PATH-available |
| **Claude Code (CLI) — stale cache** | official versioned cache (LEGACY) | `claude plugin marketplace add` + `claude plugin install` (OLD FLOW — superseded) | stale v1.0.0 (2026-05-30; lacks doc-sync, golem-dockeeper) | ⚠ **loaded but stale** — orphaned by `~/.claude/skills/gal` projection once active |
| **Claude Desktop (GUI)** | MCP-only special target (not a plugin provider) | Safe-merge into `claude_desktop_config.json` via `Update-Mcp.ps1` → `Update-ClaudeDesktopMcpConfig`; Phase 2: `.mcpb` desktop extension | **MCP servers only** — agents/skills/commands are NOT supported | ✓ **verified** (4 Phase-1 servers injected, 2026-05-30) |
| **Antigravity CLI** | `agy` | `gal install` → managed junction `~/.gemini/antigravity-cli/plugins/gal → ~/.gal/plugins/gal` | plugin.json / mcp_config.json / skills / agents / rules (complete) | ✓ **junction confirmed** (2026-06-09, T-001 probe) |
| **Copilot CLI** | `copilot` | `gal install` → managed symlink `~/.copilot/installed-plugins/gal-copilot/gal → ~/.gal/plugins/gal` | manifest / host copy | ✓ **symlink confirmed** (2026-06-01) |

Claude Desktop is an MCP-only host. It **cannot** load GAL agents, skills, or commands. Phase 1 (verified 2026-05-30) injects only no-auth stdio servers: `chrome-devtools`, `firebase-mcp-server`, `markitdown`, `playwright`. Phase 1 excludes auth-required or HTTP-remote servers (`github`, `microsoftdocs`, `context7`) — these may be added via Connectors/Integrations UI or a future `.mcpb` extension (Phase 2). Safety properties: idempotent safe-merge (user-owned servers never modified), timestamped backup before every write, `~/.gal/dist/providers/claude-desktop/managed.json` ledger records GAL-written keys (uninstall only removes ledger entries), no secret placeholders ever written.

Antigravity CLI uses a managed junction (Windows) or symlink (Unix) from the AGY plugin path to the canonical root `~/.gal/plugins/gal`, so canonical-root updates (via `gal install` or `gal update`) reflect without reinstall. Antigravity's MCP config uses `serverUrl` (not `url`) for HTTP servers; the Rust MCP serializer handles this correctly.

**Current install path (GAL-owned, not official marketplace):** `gal install` → `~/.claude/skills/gal` (symlink/junction → canonical root). Verify with `gal doctor`. See [Clean-Install Acceptance Bar](#clean-install-acceptance-bar).

**Future public marketplace listing gates (deferred):** Claude Code — GitHub Release tag published + community marketplace submission verified; Claude Desktop — `.mcpb` packaging verified and submitted; Antigravity — GAL plugin installed and verified via `agy inspect`.

#### GAL-Owned Live Read Surfaces (per Provider)

This table documents the **authoritative live read surface** — the path from which each provider **actually reads** GAL content at runtime after `gal install`. It is the governing source for T-001 go/no-go and the install convergence acceptance bar. Updated: 2026-06-09 (T-001 real-machine probe, Windows dev machine).

**Governing principle**: install success = live read surface matches source. A correct render that never converges into the surface the provider actually loads = not installed. `gal doctor` uses this table as its verification baseline.

| Provider | GAL-owned live read surface | Surface type | Stale-proof? | This machine (2026-06-09) |
| --- | --- | --- | --- | --- |
| **Claude Code (skills)** | `~/.claude/skills/gal` → `~/.gal/plugins/gal` | symlink / junction | **YES** — skills update immediately when canonical root changes; no cache layer | ⚠ **not yet created** — created by T-003 / `gal install`; final skill-load verification requires a new Claude Code session after T-003 |
| **Claude Code (legacy, AVOID)** | `~/.claude/plugins/gal` | symlink | YES (but LEGACY) | EXISTS but treated as legacy; oracle removes it on every refresh (`common.sh:76`) |
| **Claude Code (stale actual load, AVOID)** | `~/.claude/plugins/cache/gal/gal/1.0.0` | versioned cache | **NO** — frozen at install date; does NOT include `doc-sync` or `golem-dockeeper` | LOADED by `enabledPlugins: {gal@gal: true}` but stale (2026-05-30, pre-dockeeper/doc-sync); superseded by `~/.claude/skills/gal` after T-003 |
| **Copilot CLI** | `~/.copilot/installed-plugins/gal-copilot/gal` → `~/.gal/plugins/gal` | symlink | YES | ✓ EXISTS (symlink confirmed 2026-06-01) |
| **AGY CLI** | `~/.gemini/antigravity-cli/plugins/gal` → `~/.gal/plugins/gal` | symlink / junction | YES | ✓ EXISTS (symlink confirmed 2026-06-09) |
| **OpenCode** | `~/.config/opencode/skills/`, `~/.config/opencode/agents/`, `~/.config/opencode/commands/` | host copy | NO — requires `gal update` to refresh | not verified on this machine |
| **Codex** | `~/.codex/skills/` | host copy | NO — requires `gal update` to refresh | not verified on this machine |

**Claude Code skill surface notes (T-001 real-machine probe):**

- **`~/.claude/skills/` is auto-scanned** by Claude Code without any `enabledPlugins` entry (confirmed: `graphify` and `codebase-memory` load from `~/.claude/skills/` without settings.json registration).
- **Oracle designation**: `CLAUDE_PLUGIN_INSTALL_TARGET = ~/.claude/skills/gal` (`common.sh:73`). `CLAUDE_LEGACY_PLUGIN_INSTALL_TARGET = ~/.claude/plugins/gal` (`common.sh:76`) is removed every refresh by `safe_unlink`.
- **`~/.claude/skills/gal` is NOT yet created** on this machine — it is created by `gal install` (T-003). Creation was blocked from this session because modifying `~/.claude/skills/` requires explicit user permission (correctly blocked as self-modification).
- **Multi-skill bundle loading**: the canonical root contains `.claude-plugin/plugin.json` listing all 29 skills. Whether Claude Code's `skills/` auto-scan respects this bundle format (vs flat single-skill dirs like `graphify`) is **confirmed architecturally** by oracle design but requires a new-session smoke test after T-003 creates the symlink.
- **Supersession**: after T-003, the `enabledPlugins: {gal@gal: true}` / versioned-cache mechanism should be superseded by the `~/.claude/skills/gal` symlink. The stale cache at `~/.claude/plugins/cache/gal/gal/1.0.0` should not be removed manually; it is orphaned once the skills surface is authoritative.

**How GAL-managed symlink updates are re-read by Claude Code:**

When `~/.claude/skills/gal` → `~/.gal/plugins/gal` exists, any change to the canonical root (from `gal install` or `gal update`) is **immediately reflected** in the next Claude Code session — no cache layer, no version bump required. This is the key advantage of the symlink surface over the `enabledPlugins` versioned-cache mechanism.

#### Clean-Install Acceptance Bar

A GAL install is **complete** when all of the following hold simultaneously. Running `gal doctor` is the primary programmatic check; the acceptance script below verifies the full set.

| Check | Command / Path | Acceptance criterion |
| --- | --- | --- |
| `gal` on PATH | `gal --version` | exits 0, prints version |
| `gal doctor` green | `gal doctor` | exits 0, no errors |
| Canonical root present | `~/.gal/plugins/gal/` | directory exists |
| Agent completeness | `~/.gal/plugins/gal/agents/golem-dockeeper.agent.md` | file present |
| Skill completeness | `~/.gal/plugins/gal/skills/doc-sync/` | directory present |
| Claude skill surface aligned | `~/.claude/skills/gal` → `~/.gal/plugins/gal` | symlink (Unix) or junction (Windows) pointing to canonical root |
| No orphan render temps | `~/.gal/plugins/.gal-render-*` | none present |
| Plugin bin exposed | `~/.gal/plugins/gal/bin/gal` (`.exe` on Windows) | file present and executable (`+x` on Unix) |

"Rendered a correct artifact" is **not** completion — the artifact must also be convergent into the live read surface. The canonical root must be built **and** the Claude skill surface must point to it.

**E2E acceptance script (Unix / mac-mini TP-15):**

```bash
bash scripts/test-install-acceptance.sh
```

`scripts/test-install-acceptance.sh` runs the full eight-check set above and exits 0 on full pass, 1 on any failure. Run it on the target machine after `gal install` completes, or pipe it over SSH:

```bash
ssh mac-mini 'bash -s' < scripts/test-install-acceptance.sh
```

**Windows acceptance (TP-16):** the same eight checks apply; run them manually until a PowerShell acceptance script exists (post T-011):

1. `gal --version` in a new terminal (PATH check)
2. `gal doctor` (exits 0)
3. `Test-Path "$env:USERPROFILE\.gal\plugins\gal"` → True
4. `Test-Path "$env:USERPROFILE\.gal\plugins\gal\agents\golem-dockeeper.agent.md"` → True
5. `Test-Path "$env:USERPROFILE\.gal\plugins\gal\skills\doc-sync"` → True
6. `Test-Path "$env:USERPROFILE\.claude\skills\gal"` → True (junction)
7. `(Get-ChildItem "$env:USERPROFILE\.gal\plugins" -Filter ".gal-render-*").Count` → 0
8. `Test-Path "$env:USERPROFILE\.gal\plugins\gal\bin\gal.exe"` → True

#### Raw PowerShell and Shell Convenience Installer Policy

Raw PowerShell or shell installers such as `irm ... | iex` and `curl ... | sh` are convenience entrypoints only. They are not canonical install channels, they do not define an independent package shape, and they must not become the only supported way to get GAL onto a machine.

Policy rules: (1) the canonical version source remains GitHub Releases — a raw installer must resolve or point to the exact canonical release tag and per-platform asset instead of an ad hoc payload from a branch, repo checkout, or alternate host; (2) it may fetch or redirect only to the exact GitHub Releases fallback archive or binary for the current platform, and must not wrap a second payload layout or install extra unmanaged runtime content; (3) it must describe itself as a convenience or bootstrap fallback, not the primary or canonical distribution model; (4) it must preserve the same ownership boundaries as every other bootstrap lane (install or replace the bootstrap payload only, then let GAL manage `~/.gal/`); (5) it must not bypass release governance by pulling unpublished assets, mutable branch heads, or provider-specific payload variants.

User-facing copy rules — every raw installer entrypoint must make clear: it is a convenience wrapper over the canonical release lineage; `winget`, `homebrew`, and GitHub Releases remain the official install channels; it may lag or be redirected as release policy changes; it does not redefine uninstall, purge, or migration behavior.

Acceptance gate — do not present a raw command as an official install surface until: (1) it resolves only to canonical GitHub Release metadata and the exact GitHub Releases payload for the current platform; (2) the fetched/delegated payload matches the documented per-platform single-binary lineage; (3) it does not create a second update, uninstall, or support policy; (4) README and release docs describe it as a convenience fallback rather than a primary distribution lane.

## Making Changes

### Rules & Fences

#### 1. Markdown owns the durable contract

- Methodology, rules, and contracts live in tracked Markdown and source files.
- Generated adapters, baked command files, and runtime configs are outputs, not source inputs.

#### 2. `/gal` only solves control-plane problems

- `/gal` should not wrap a second copy of tester, reviewer, designer, security, debugger, or releaser work.
- Execution-stage specialist behavior belongs in agents.
- Planning commands can run directly because they are still part of the public command surface.

#### 3. Repo-local state is the ownership boundary

- `.dev/`, `docs/plans/`, `docs/designs/`, and similar repo-local files are the shared working state.
- Do not move GAL's core state back into user-global storage.

#### 4. Missing tools must not look like success

- GAL uses skill-level routing, not one repo-wide CLI-first or MCP-first rule.
- Each external-tool skill should define a preferred path, a fallback path, and a no-tool behavior.

#### 5. Do not optimize one runtime by breaking portability

- If a change makes Copilot, Antigravity, and Codex diverge in contract or file flow, it is usually the wrong change.
- README, docs, templates, and setup scripts should preserve cross-runtime parity first.

#### 6. Navigation docs must not become a second spec

- If a document exists to help humans find the real source, keep it short and directional.
- Summaries are useful. Duplicate contracts are not.

#### 7. Collaborative-tool routing belongs to the workflow layer

- Do not make an agent silently switch personas or contracts because a collaborative tool was detected.
- The workflow decides the collaborative tool first, then the tool writes back into the same repo-owned files.

The shared preflight model lives in [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md).

### Before Editing Install

1. Decide which concern is changing: bootstrap payload, machine setup, provider plugin lifecycle, repo init, generated adapters, or documentation copy.
2. Read the primary owner from the `Owning Surfaces` table above, then read only the directly called scripts or docs for that concern.
3. Check source mode and install mode separately. A fix for contributor setup may be wrong for package-managed first launch.
4. Preserve user-owned machine intent during upgrade and default uninstall.
5. If the change affects public install wording, update English and the translated copies under `docs/i18n/<lang>/` together, or explicitly mark the translation drift.
6. Run the narrowest install tests that cover the changed lane. For provider packaging, start with `Test-BuildProviderPlugins.ps1` and `Test-InstallGalPlugins.ps1`.

### Common Change Entry Points

#### Changing `/gal` or alias behavior

1. Read [../commands/commands.md](../commands/commands.md).
2. Check whether the change is contract-level behavior or only install/runtime presentation.
3. If it affects generated command files, inspect the setup scripts and the relevant `commands/*/SKILL.template.md`.

#### Adding or changing a planning command

1. Place it in the right family via [../commands/commands.md](../commands/commands.md).
2. Update the owning prompt in `commands/<command>/SKILL.template.md`.
3. Confirm the write-back target fits the existing plan sections and workflow state machine.
4. If it changes optional collaborative-tool semantics, also update [collaborative-tools/gstack.md](collaborative-tools/gstack.md) and [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md) when shared preflight behavior changes.

#### Adding or changing an execution specialist

1. Update the owning prompt in `agent/<golem>.agent.md`.
2. Confirm the write-back target fits the existing plan sections and workflow lifecycle.
3. Update [../agent/agents.md](../agent/agents.md), [../commands/commands.md](../commands/commands.md), and any README sections that route users to that specialist.
4. Do not reintroduce the behavior as a standalone public command unless it is truly control-plane or planning work.

#### Changing setup, installation, or MCP merge

1. Read [../scripts/scripts.md](../scripts/scripts.md).
2. Decide which concern owns the change first: `gal update --machine-only` (`adapters` backend), `gal mcp update`, `gal sync`, or the top-level `gal setup` orchestrator.
3. Windows and macOS/Linux share the single Rust `gal setup` orchestrator and Rust install/render path; keep behavior aligned unless the change is intentionally platform-specific.
4. Check whether `commands/commands.md` should also change because the user-visible runtime surface changed.
5. Keep README focused on entry points, keep setup plumbing here and in the source scripts.

#### Refreshing MCP vs. Regenerating Adapters

- Run `gal mcp update` after changing `mcp.json`, `~/.gal/config/mcp.local.json`, or MCP-related values in `~/.gal/config/config.local.env`. This refreshes runtime MCP config only.
- Run `gal sync` after changing source-of-truth content that should regenerate repo-local adapters such as `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, or `GEMINI.md`.
- Run `gal setup` when you need the full concern stack refreshed in one pass.

#### Refactoring docs themselves

1. Make sure each doc has one clear job.
2. If another source file already owns the contract, summarize it and link out instead of copying it.
3. If you remove content from one reader entry point, give it a clear new landing page.

### Adding A New CLI Runtime

1. Decide whether the CLI has a machine-layer config directory that GAL can target.
2. Decide whether its repo-facing instruction file can reuse `AGENTS.md` or needs another generated adapter.
3. If the runtime supports native commands, generate them from the same shared command templates instead of building a second workflow source. If it does not, install the same baked command skills into the runtime's supported skill surface.
4. Add any config-merge bridge only if the runtime has a stable, user-owned config file that can safely accept additive changes.

### Verify & Self-Check

After changing install or setup logic, verify at least these points:

- the stable repo symlink exists for each supported runtime that needs one
- generated `commands/*/SKILL.md` files no longer contain `{{GAL_ROOT}}`
- Gemini native command files were regenerated from the baked command content
- Antigravity installed skills and agents resolve through `~/.gemini/antigravity-cli/plugins/gal/`
- AGY plugin-root `mcp_config.json` is the sole GAL-managed MCP source for AGY; no GAL-managed MCP entries remain in the global `mcp_config.json` or `settings.json`
- shared skill directories contain reusable skills only, not duplicated command aliases
- MCP reruns update tracked server entries correctly without clobbering unrelated provider-owned config

Before you finish a maintainer change, also ask:

- Did I create a second source of truth?
- Does `/gal` still only solve control-plane problems?
- Do specialist commands still write back to repo-owned files?
- Did I accidentally move state back into a user-global path?
- Can a missing tool still fail loudly instead of pretending to succeed?
- Did I keep collaborative-tool routing at the workflow layer?
- Do README, `commands/commands.md`, and this guide still have distinct jobs?

### Drift Queue

Known navigation/ownership drift to watch when editing install or release docs:

- Provider lifecycle status claims are split across docs and code. Before changing public claims for Claude, Copilot, or Codex, compare [../README.md](../README.md), this guide, [../scripts/scripts.md](../scripts/scripts.md), [../crates/gal-engine/src/install.rs](../crates/gal-engine/src/install.rs), [../crates/gal-engine/src/render.rs](../crates/gal-engine/src/render.rs), and the latest verified plan [plans/feat-plugin-arch-migration.md](plans/feat-plugin-arch-migration.md).
- When a translated copy under `docs/i18n/<lang>/` falls behind its canonical source, the translation freshness check flags it; resync before changing public install status claims.

## Conventions

### Documentation Conventions

This section is the authoritative naming and translation policy for `docs/`. The on-location translator signpost is [i18n/guide.md](i18n/guide.md); it points back here and must not duplicate this policy.

#### File Naming

- **`README.md` is reserved for the single repo-root README.** No other file in the repo may be named `README.md`. A sub-area entry/index doc uses `guide.md` (a curated signpost) or `index.md` (a generated or listing index) instead.
- **Tool/capability docs** under `docs/collaborative-tools/` use the `-mcp` suffix only when the doc is specifically about an MCP server (`blender-mcp.md`, `playwright-mcp.md`, `codebase-memory-mcp.md`). A workflow or methodology doc does not take the suffix (`graphics-workflow.md`, `godot.md`).
- **Skill-aligned docs** match the skill name they support: `docs/collaborative-tools/graphics-workflow.md` supports the `graphics-workflow` skill.
- No two unrelated docs may share the same basename.

#### Multi-Language Translation Policy

The policy is designed so both the set of translated docs and the set of languages can grow without churn.

- **Canonical = English**, at the main filename in its normal location (`README.md`, `docs/manual.md`). Canonical docs are never moved for translation.
- **Translations live under `docs/i18n/<lang>/`** and are named `<name>.<lang>.md`. The folder *and* the filename both carry the language — a deliberate redundancy so a translation file is self-describing out of context, and so the basename is never bare `README.md` (honoring the single-README rule).
- **Mirror-path rule**: root `README.md` → `docs/i18n/<lang>/README.<lang>.md`; `docs/X.md` → `docs/i18n/<lang>/X.<lang>.md` (sub-paths such as `collaborative-tools/` are preserved). The canonical doc's top carries a language-switch link to each translation, and each translation links back.
- **Translatable allowlist** (not "every doc"): currently `README` and `docs/manual.md`, plus a small number of tool docs added on demand. Adding a doc or a language is one entry in `docs/i18n/` — coverage grows visibly and stays bounded. Known near-term language: `ja`.
- **EN-only by default**: maintainer/contract docs — this guide, `collaborative-tools/*` contracts, `research/`, and `plans/` — are not in the allowlist and are not translated.
- `<lang>` tags follow the existing convention in use (e.g. `zh-Hant`, `ja`).

#### Translation Freshness

Every translation under `docs/i18n/` carries YAML front-matter so drift against its English source is visible rather than silent:

```yaml
---
source: README.md          # repo-relative path to the canonical source
lang: zh-Hant
source_commit: <hash>      # the source commit this translation is in sync with; PENDING until stamped
translated_at: 2026-06-02
status: current | stale     # human hint; the check recomputes from source_commit
---
```

`gal translation-freshness` scans `docs/i18n/`, compares `source_commit` against `git log -1 --format=%H -- <source>`, and reports each `(doc, lang)` as `current`, `stale` (hash differs, missing, or a non-hash placeholder like `PENDING`), or `missing` (an allowlisted pair with no translation). It is report-only and scales linearly with languages and docs — it does not extend the structure-map schema.

Operational notes:

- Because GAL commits are authored by a human after review, a freshly synced translation is stamped `PENDING` until the maintainer records the real source commit after committing; the check surfaces those as `stale` so they are not forgotten.
- A translation flagged `status: stale` is knowingly behind its English source. Such files are excluded from the hard repo-wide broken-link gate — the freshness check owns them — until they are re-translated and re-stamped.

#### Planning-Doc OQ Lifecycle

Open Questions in `docs/plans/*.md` are scaffolding, not a permanent record:

- When an OQ is resolved, bake the ruling into the section that owns it (typically a `## Decisions` table with the ruling, date, and who decided), then **delete the OQ entry** — do not keep `- [x]` OQ corpses.
- When all of a plan's OQs are resolved, do a **full rewrite pass** of the plan instead of incremental patching. Incremental patches leave "see OQ-xxx" cross-references pointing at deleted or moved content, which makes plans progressively unreadable.
- A plan body must never require the reader to reconstruct decision history from OQ archaeology; history belongs to git, the plan states only the current ruling.

### Token Discipline

These rules apply to all maintainer and agent work in this repo. The full policy lives in [../conventions/token-budget.md](../conventions/token-budget.md). The developer-facing summary is here.

#### Generated-Artifact Exclusion

Do not read generated adapters (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`, `.github/copilot-instructions.md`) or build outputs (`bin/`, `obj/`) unless the current task is explicitly about auditing those generated files. They are large, frequently regenerated, and contain no information not already in their source templates.

#### Directed Exploration

Before reading any file, confirm it is named in the current task or is a direct dependency of a task-named file. Stop reading when you have the information needed. Do not load the full codebase as a cold-start step.

#### Failure-Focused Output

When running builds or tests, emit:

- Build: first error with file and line reference. On success, one summary line only.
- Tests: failing test names and assertion messages only. Do not echo passing test names.
- Lint: files and rule violations only. On a clean pass, one summary line only.

Store full logs on disk when needed; retrieve specific lines selectively rather than piping entire logs into context.

#### Context-Pressure Recovery

When context is near the limit during an active task:

1. Write the current task name, last completed step, and any key decisions to `### Handoff Notes` in the active plan's `## Status` section.
2. Write or update the matching plan row in `.dev/state.md` `## Session Continuity` with `Stopped At` and `Next Step`.
3. Do **not** create a separate `CONTEXT.md` file — the plan and state files are the only durable session state stores.

This ensures the next session can resume without re-deriving context.
