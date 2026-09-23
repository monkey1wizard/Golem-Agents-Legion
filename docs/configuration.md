---
type: Guide
title: Configuration
description: Manage machine-local settings in `~/.gal/config/config.json`, including executor routing, working hours, plan language, MCP sources, the personal layer, and local-notes routing.
tags:
  - configuration
  - routing
  - mcp
  - personal-layer
status: stable
---

# Configuration

## Machine-Local Settings (`config.json`)

GAL stores machine-local settings in `~/.gal/config/config.json`. Values in this file apply exclusively to the current machine. Never commit machine-local values into version-controlled files, command templates, or source code. Recognized keys include `galSkills`, `workingHours`, `planLanguage`, `memoryHarvest`, `executorRouting`, and `pluginMode`. Note that `schemaVersion` belongs in `plugins.lock.json` rather than `config.json`. Similarly, `enabledPlugins` is a setting in GitHub Copilot's own `settings.json`, which `gal doctor` reads only to confirm plugin registration.

Deprecated keys (`devMode`, `galRoot`, and `secrets`) are ignored. If `gal doctor` detects `devMode` or `galRoot`, it emits a non-blocking warning. The `secrets` key is retired. It served a legacy MCP host writer, and GAL reads no credential data from this file.

### Primary Fields

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `executorRouting` | optional | object | Role-based executor routing grouped by consumer. Serves as the system's sole routing authority. |
| `galSkills` | optional | string | Absolute path to the local skills directory. This is the only remaining machine-local path substitution key. |
| `workingHours` | optional | object | Working-hours policy containing `enabled`, `workdayStart`, `workdayEnd`, `wrapUpTime`, and `hardStopTime`. Times use `HH:MM` format, and `enabled` is boolean. |
| `planLanguage` | optional | string | Default output language for `.dev/plans/*.md` and `.dev/research/*.md` when unspecified. Defaults to `en`. |
| `memoryHarvest` | optional | object | Provider memory harvesting configuration shaped as `{ enabled: boolean }`. Disabled by default. |
| `pluginMode` | optional | object | Per-runtime plugin registration flags with keys `claude`, `codex`, `copilot`, and `agy`. Values are booleans defaulting to `false`. When enabled, projection skips fallback artifacts for that runtime, and `gal doctor` verifies active registration. |

Core configuration does not manage external notes backends. For external notes specifications, see `plugins/gal-core/conventions/optional-capabilities.md`. Repository research outputs default to `.dev/research/`, and temporary working files reside in `.dev/tmp`.

## Executor Routing (`executorRouting`)

The `executorRouting` object organizes agent roles into three consumer groups:

- **`planning`**: Review group containing `ARCHITECT`, `ANALYST`, `DESIGNER`, and `RELEASER`.
- **`pipeline`**: Dispatch group containing `CODER`, `TESTER`, and `AUDITOR`.
- **`research`**: Research group routing across up to two of three parallel research workers.

All three groups inherit defaults from a shared `executors` block, which functions as the single routing authority. Legacy flat routing schemas are retired, are no longer parsed, and produce warnings. Standalone routing configuration files are no longer supported.

### The `combinations` Registry

The `executorRouting.combinations` registry defines reusable executor configurations. Each entry specifies an `executor` along with optional overrides for `model`, `effort`, `timeoutSecs`, `sshTarget`, and `remoteWorkdir`. Roles can reference a combination by name in their `executor` property, overriding combination values field by field. Note that `sshTarget` and `remoteWorkdir` must always be provided together.

### The `executors` Defaults Block

Entries in `executorRouting.executors` accept either string shorthands or full configuration objects:

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `model` | required | string | Default model identifier for the executor. |
| `effort` | optional | string | Default reasoning effort setting. |
| `timeoutSecs` | optional | positive integer | Execution timeout in seconds. Setting `0` is rejected during configuration loading. |

When resolving settings for a role, fallback order is deterministic:
**Inline role setting → Combination base → `executors[final.executor]` → `None`**.
The model resolution chain defaults to an empty string if unconfigured. If a role explicitly sets `timeoutSecs: 0`, timeout resolution yields `None` rather than falling back to defaults.

### Research Routing

The `research` group uses headless dispatch similarly to `pipeline`. In contrast, the `planning` group is verification-only and never invokes headless dispatch. Research workflows utilize three parallel workers: `RESEARCHER#0`, `RESEARCHER#1`, and `RESEARCHER#2`. These workers run in isolation without mutual visibility. Once all workers finish, the orchestrator synthesizes findings directly. This process does not use an intermediate verification dispatch step, does not use a `verify` key in `executorRouting`, and routes only across the selected worker instances.

### Remote Execution

Remote execution parameters are configured per role inside `executorRouting` without auxiliary files. Roles specify `sshTarget` and `remoteWorkdir` concurrently to designate SSH endpoints and repository working directories. Because these settings represent sensitive machine-local infrastructure details, never share them across repositories.

Credentials and secrets must never be placed in `config.json`. GAL does not extract credentials from configuration files.

## Machine-Local Path Placeholders

Git filter operations (`gal clean` and `gal smudge`, detailed in [workflows.md](workflows.md)) strip machine-local paths using placeholders:

| Placeholder | Definition | Application |
| --- | --- | --- |
| `<GAL_SKILLS>` | Absolute path to the local skills directory. | Applied in git filters and local skill projections. |
| `<GAL_SKILLS_BIN>` | The `bin` directory located under `<GAL_SKILLS>`. | Substituted before `<GAL_SKILLS>` to preserve path specificity. |

Path substitution applies exclusively to `galSkills`. Other configuration keys (such as working hours) do not undergo placeholder replacement.

## Model Context Protocol (MCP) Servers

GAL aggregates MCP servers from two distinct sources, merging them into the canonical root at `~/.gal/plugins/gal/` during `gal refresh`:

| Source | Location | Description |
| --- | --- | --- |
| **Core** | `plugins/gal-core/mcp.json` | Version-controlled servers required by GAL core. Kept empty by default. Entries are added only when core functionality requires them. |
| **Personal** | `~/.gal/local/mcp.json` | User-defined servers local to the host machine. The presence of this file acts as the enablement trigger without extra configuration flags. |

Both files use the standard `"servers"` dictionary schema.

### Merge and Generation Flow

Running `gal refresh` processes MCP servers through the following sequence:

1. Translates the core `mcp.json` `"servers"` dictionary into `~/.gal/plugins/gal/.mcp.json` under the `"mcpServers"` key.
2. If `~/.gal/local/mcp.json` exists, merges its server definitions into the `"mcpServers"` dictionary. If a personal server name conflicts with a core server, the personal definition is ignored and the core configuration takes precedence. If the personal file is missing or invalid JSON, GAL preserves core servers and continues execution cleanly.
3. Writes the combined server list to `~/.gal/plugins/gal/mcp.json` conforming to the Agent Plugins standard format.

Server configurations pass through verbatim without path placeholder substitution. Environment variables in server configs (`${ENV_VAR}`) are resolved dynamically by MCP host runtimes during process launch. Define these variables within the host shell environment rather than in `config.json`.

Configuration overrides are not supported via auxiliary files. To modify server parameters, edit the corresponding source file and rerun `gal refresh`. Runtimes like Claude Code require a restart or plugin reload to register configuration updates.

The personal MCP layer is merged exclusively during local `gal refresh` operations. Build outputs for public marketplace snapshots omit personal configurations entirely.

### Runtime Support Matrix

GAL does not inject MCP server configurations into external application directories, nor does it edit third-party settings files. Runtimes discover servers when loading GAL as a plugin:

| Runtime | Configuration Target | Integration Status |
| --- | --- | --- |
| Claude Code | `.mcp.json` | Supported natively. |
| OpenAI Codex | `.mcp.json` | Supported natively. |
| GitHub Copilot | `mcp.json` | Supported natively. |
| Google Antigravity | None | Not officially verified within plugin containers. |
| opencode | None | Not supported. opencode does not load Markdown plugins, and file projection does not include MCP configurations. |

"Supported natively" indicates GAL produces the configuration file expected by the runtime plugin loader. Actual loading depends on runtime support and requires completing registration steps in [setup.md](setup.md).

GAL never modifies host-managed configuration files such as `claude_desktop_config.json`, `~/.copilot/mcp-config.json`, `~/.codex/config.toml`, `opencode.json`, or `mcp_config.json`. These files remain user-managed. Configure application-wide tools directly in the respective runtime configuration file.

### Example: Custom MCP Server

The example below illustrates registering custom MCP servers via `~/.gal/local/mcp.json`. Multiple instances of a single tool can be registered with different environment variables. The `postgres-mcp` package runs via `uvx`:

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

Save this content to `~/.gal/local/mcp.json`, export `POSTGRES_MCP_APP_URI` and `POSTGRES_MCP_ANALYTICS_URI` in your shell, and run `gal refresh`. Both database servers will appear in generated plugin manifests.

## Working Hours Policy (`workingHours`)

Working hours enforcement is disabled by default. Configure settings under `workingHours` in `~/.gal/config/config.json` to enforce schedule boundaries. Set `enabled: false` to disable checks. Properties `workdayStart` and `workdayEnd` establish active working hours, `wrapUpTime` triggers session closing notifications, and `hardStopTime` blocks new task dispatches. These boundaries represent personal developer preferences rather than shared repository policies.

## Plan Language Configuration (`planLanguage`)

The `planLanguage` key specifies the default language for generated `.dev/plans/*.md` and `.dev/research/*.md` files when unprompted. The resolution hierarchy is:
**Explicit user instruction → `planLanguage` setting → Prompt language detection → Default `en`**.
Project-level documentation policy is controlled separately via `PROJECT_LANGUAGE` in repository metadata. Execution prompts (`.dev/plans/*.prompt.md`) remain strictly in English to maintain deterministic multi-model performance.

Setting `planLanguage` to a non-English language enables a three-layer plan workflow:

1. **English technical draft** (`.dev/plans/<slug>.en.md`): Defines canonical technical specifications.
2. **Localized working plan** (`.dev/plans/<slug>.md`): Editable localized file for human review. Section headers, file paths, task identifiers, and status verdicts remain in English, while descriptive text is localized.
3. **Execution prompt** (`.dev/plans/<slug>.prompt.md`): Final English compilation for model execution.

Manual edits made to localized plans are preserved. When subsequent planning commands execute, they pause at a read-only reconciliation step to backport manual revisions into the English draft. Manual edits are never overwritten. When `/plan-to-prompt` compiles the final execution prompt, the intermediate draft is deleted, leaving two persistent plan files. Setting `planLanguage: "en"` bypasses translation layers entirely.

## Memory Harvesting (`memoryHarvest`)

Memory harvesting captures operational lessons learned by models during active sessions, persisting them into repository files to retain context across session restarts:

- **Disabled by default**: Enable by setting `config.json#memoryHarvest.enabled: true`. If omitted or set to `false`, the feature remains inactive.
- **Bounded exploration**: GAL never parses, indexes, or scans global chat histories or local session dumps for candidates.
- **Context-bound and de-identified**: Candidate lessons must reference concrete repository files. Candidates are sanitized to strip raw conversation logs, secrets, machine-local paths, and personal notes.
- **Human approval required**: Harvested candidates can only be reviewed and accepted during `/gal wrap-up`. Unapproved candidates are discarded without touching disk. Approved entries are recorded in the active plan's handoff notes as advisory task context.
- **Promotion to documentation**: Memory harvest approval does not automatically alter documentation. Approved notes require independent verification during `/gal finalize` before promotion into `docs/`.

## Local Customizations

### Personal Directory Root (`~/.gal/local/`)

User-specific assets reside in `~/.gal/local/`. These assets are rendered into the canonical root and projected across all active runtimes. GAL reads from `local/skills/`, `local/mcp.json`, and `local/conventions/`, but **never modifies, creates, or deletes files inside user-managed directories**. Multi-machine file synchronization is managed by the user.

#### Directory Layout

```text
~/.gal/local/
  skills/
    <skill-name>/
      SKILL.md          ← Personal skill definition (read-only for gal)
  mcp.json              ← Personal MCP servers (read-only for gal)
  conventions/
    <lang>.md           ← Personal language coding convention (read-only for gal)
```

#### Activation

Activation is determined strictly by file existence without requiring configuration flags. Placing files within `~/.gal/local/` activates personal overlays. If directories or files are missing, output generation matches default core behavior exactly.

#### Precedence and Collisions

- Personal skills are evaluated after core skills. If a personal skill shares an identifier with a core skill, the personal version is skipped and the core skill takes precedence.
- Personal MCP servers are merged into the canonical `.mcp.json`. Name collisions resolve in favor of core definitions.
- Running `gal doctor` reports personal skill counts, active convention files, and any definitions skipped due to name collisions.

### Coding Style Conventions

Core GAL templates do not impose personal coding styles. Custom styling can be applied to target repositories through three mechanisms:

#### Source 1: Personal Convention Files (Repository-Wide)

Copy `plugins/gal-core/templates/csharp-convention.example.md` to `~/.gal/local/conventions/csharp.md` and customize rules. Accepted file stems are strictly limited to `csharp`, `typescript`, or `go`. The `typescript` file applies to projects listing either TypeScript or JavaScript in their `Language` setting, but the file stem must remain `typescript`. If you previously relied on generic naming, rename files to match approved stems. When `gal render-adapters` runs in a repository with a matching `Language` value in `.dev/project.md`, rules are injected into adapter instructions similarly to built-in conventions.

#### Source 2: Installed Plugin Discovery (Zero Configuration)

If your coding runtime contains official language plugins, run `gal render-adapters` in the target repository. GAL matches plugin metadata against the repository's `Language` field, rendering a **Detected Language Skills** instruction block that references the installed plugin. GAL never copies external plugin source code, nor does it install, update, or remove plugins. Run `gal render-adapters` after installing new runtime extensions to update detection references.

#### Source 3: Personal Skills (On-Demand Loading)

Create a custom skill under `~/.gal/local/skills/<name>/SKILL.md`. Claude Code requires a restart to index new skill directories. Other runtimes detect files on their subsequent read cycle. These skills load only when explicitly invoked by name, avoiding global context pollution.

#### Selecting a Customization Source

For standard setups with official plugins installed, choose Source 2 for zero-maintenance integration. Choose Source 1 when you need consistent, enforced team conventions across projects. Use Source 3 for specialized, ad-hoc agent workflows.

#### Fallback Behavior

If a repository's `Language` metadata matches no convention file or plugin, generated adapters omit language convention blocks. This represents normal operational fallback rather than an error condition.

#### Disabling Automatic Conventions

To disable automatic convention injection from Sources 1 and 2, add the row `| Personal Conventions | off |` to the `Tech Stack` table in `.dev/project.md`. Public repositories should enable this setting to prevent machine-local configurations from leaking into committed adapters. Source 3 skills remain available for on-demand use.

### Local Command Overlays (`SKILL.local.md`)

To customize command skills locally, create `plugins/gal-core/commands/<command>/SKILL.local.md`:

- The file is gitignored and treated as user-owned configuration.
- During build routines, contents of `SKILL.local.md` are appended to the generated `SKILL.md`.
- Never edit `plugins/gal-core/commands/<command>/SKILL.md` directly. That file is regenerated automatically during projection.
- `SKILL.local.md` supplies supplementary prompt instructions only. Do not include front matter blocks.

### External Notes Integration (`local-notes`)

Connecting external personal notes is entirely optional. GAL maintains strict separation between repository state and private note vaults. Core functionality does not depend on external note stores. The portable specification is detailed in [`optional-capabilities.md`](../plugins/gal-core/conventions/optional-capabilities.md). Notes integration is disabled by default and engages only after local backend availability is verified.

If a notes backend is unavailable, unconfigured, or offline, the system degrades gracefully to standard repository workflows without throwing errors. Only explicitly connected backends can perform reads or writes at authorized workflow checkpoints. Notes integrations are never prerequisites for core features. Repositories without note backends function identically to connected repositories, minus access to private vault context.

Supported backend implementations include Obsidian (`coddingtonbear/obsidian-local-rest-api`), Logseq (`ergut/mcp-logseq`), Joplin (`joplin-mcp`), generic Markdown directories (`vault-mcp`), and `SeekLink` for CJK-focused retrieval. Feature parity varies across implementations.

Repository research documents are stored in `.dev/research/` by default.
