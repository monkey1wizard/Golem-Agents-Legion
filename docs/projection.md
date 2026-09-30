---
type: Reference
title: Projection and Runtime Surfaces
description: Explains how GAL renders source contracts into the canonical root, projects them across runtime surfaces, and generates repository adapters, projection registries, and MCP manifests.
tags:
  - projection
  - runtime
  - adapters
  - render
status: stable
---

# Projection and Runtime Surfaces

## The Projection Lifecycle

GAL skills, commands, agent definitions, and templates are version-controlled Markdown source files located under `plugins/gal-core/`. AI runtimes do not read these files directly. They take effect only after rendering and projection.

GAL delivers contracts through two channels:

1. **Machine-level projection**: Copies or links assembled contracts into the configuration directories of each supported runtime.
2. **Repository-level generation**: Compiles `.dev/project.md` into repo-local adapter files, such as `AGENTS.md` and `CLAUDE.md`.

The projection pipeline is divided into three distinct modules with strict boundaries:

- **Canonical root rendering** (`crates/gal-engine/src/render/`): Compiles `plugins/gal-core/` and optional user customizations from `~/.gal/local/` into the canonical root at `~/.gal/plugins/gal/`.
- **Runtime surface projection** (`crates/projection/`): Projects canonical root assets into each target runtime's configuration directory using the formats that tool supports.
- **Repository adapter generation** (`crates/cli/src/gal/render.rs`): Compiles a repository's `.dev/project.md` into `AGENTS.md` and `CLAUDE.md`. This routine runs independently without calling the previous two modules.

```text
plugins/gal-core/ (Source contracts, version-controlled)
        │
        ├── ~/.gal/local/ (Personal overlay layer, merged when present)
        ▼
   Canonical Root Render
   (Core-Wins collision resolution)
        ▼
~/.gal/plugins/gal/
(Canonical root following Agent Plugins specification)
        │
        │  Per-runtime projection
        │  (Junction, symlink, or file copy depending on target)
        │
        ├──▶ Claude Code
        │      Plugin commands and subagents
        │
        ├──▶ GitHub Copilot
        │      ~/.copilot/skills/ + ~/.copilot/agents/
        │
        ├──▶ OpenAI Codex
        │      ~/.agents/skills/ (file copies) + ~/.codex/agents/
        │
        ├──▶ Google Antigravity
        │      ~/.gemini/antigravity-cli/skills/ + symlink to canonical root
        │
        └──▶ opencode
               ~/.config/opencode/commands/ + ~/.config/opencode/agents/
```

Command and skill content delivered to runtimes must be byte-identical to the pre-rendered canonical root. Delivery mechanisms may differ across environments. Sandbox permissions and invocation trigger syntax may also differ. Shared prose may differ when a runtime requires host-specific instructions. These differences do not change the shared command behavior.

## Canonical Root Architecture

Each machine maintains exactly one canonical root located at `~/.gal/plugins/gal/`. Files consumed by AI runtimes are copies or symlinks projected from this directory, and never independent sources of truth.

Projection operates as a one-way synchronization push from the canonical root to target runtimes. Runtimes do not write back modifications, ensuring runtime files can be deterministically rebuilt from the canonical root at any time.

## Multi-Source Merging

The canonical root merges version-controlled core contracts (`plugins/gal-core/`) with machine-local personal overlays (`~/.gal/local/`). If paths or identifiers collide, the core contract takes precedence and the personal entry is discarded (the Core-Wins rule).

Users may add supplementary skills or configurations, but overriding core contracts is prohibited. Personal overlays are activated purely by file presence without requiring configuration flags. Deleting a personal file removes its assets on the next `gal refresh`. GAL modifies no files outside the `_galProjection` namespace and never writes back to personal overlay directories.

## Command Projection per Runtime

GAL commands (such as `/gal status`) are defined canonically under `commands/`. Runtimes handle commands differently: Claude Code and opencode provide native command systems, whereas Copilot, Codex, and Antigravity load commands as skills. The projection logic in `crates/projection/src/lib.rs::update_commands` translates canonical specifications into runtime-specific structures:

| Runtime | Command System | Projection Path | Trigger Syntax |
| --- | --- | --- | --- |
| Claude Code | Native plugin command | Canonical root `commands/` | `/gal status` |
| GitHub Copilot | Agent skill | `~/.copilot/skills/<name>/SKILL.md` | `/gal-status` |
| OpenAI Codex | Agent skill | `~/.agents/skills/<name>/SKILL.md` | `$gal-status` |
| Google Antigravity | Agent skill | `~/.gemini/antigravity-cli/skills/<name>/SKILL.md` | `/gal-status` |
| opencode | Native Markdown command | `~/.config/opencode/commands/<name>.md` | `/gal-status` |

Runtimes with native command systems (Claude Code and opencode) do not receive redundant command-skill projections, preventing duplicate command listings. While `~/.agents/skills/` is shared across Codex, opencode, and Copilot, command skills are projected there only when Codex is enabled. If only opencode is enabled, only native command files are created.

Before writing files, `write_command_skill` validates ownership markers (`is_managed`) to prevent overwriting user directories. When a runtime is disabled in configuration, `remove_gal_command_skill` cleans up previously projected command files.

## Codex Plugin Hook Projection

GAL keeps Codex pipeline hooks in the Codex-specific source fragment `plugins/gal-core/codex/hooks.json`. During rendering, GAL merges this fragment into `.codex-plugin/plugin.json#hooks`. The fragment defines `UserPromptSubmit`, `PreToolUse`, and `Stop` handlers. Each handler invokes GAL's internal `gal pipeline-host-hook` command.

Hooks are manifest-only assets. GAL does not create root `hooks/hooks.json` and does not add hooks to root `plugin.json` or `.claude-plugin/plugin.json`. GAL does not project these hooks to Claude Code or OpenCode. Claude Code and OpenCode keep their existing host-owned pipeline continuation behavior.

```text
plugins/gal-core/codex/hooks.json
          │
          ▼
.codex-plugin/plugin.json#hooks
          ├──▶ Codex plugin hook handlers
          ├──▶ no root hooks/hooks.json
          ├──▶ no root plugin.json hooks
          ├──▶ no .claude-plugin/plugin.json hooks
          └──▶ no OpenCode hook projection
```

The Codex hook branch activates guarded continuation only after a trusted, live hook handshake. `gal doctor` can report static packaging and command readiness, but it cannot prove that Codex trusts or ran the handlers. The same-session canary supplies that live proof: the Stop handler acknowledges the bootstrap and returns the exact grant-bearing continuation, then `UserPromptSubmit` binds that continuation to the same session before the guarded CLI entry can consume the grant. Grant consumption alone does not authorize implementation work. The guarded entry must pass its fresh entry latch and semantic task checkpoint first.

The bootstrap `PreToolUse` handler denies every supported tool call while bootstrap is pending. After the canary, it permits only the exact grant-bearing first action in that session. Specialized and hosted tool paths are outside this guarantee and cannot be used for the bootstrap continuation. Once guarded entry consumes the grant and creates coordinator revision 0, the bootstrap restriction ends. For the matching active profile, the hook returns neutral/pass for normal tool use. This result does not authorize a checkpoint or phase transition. Guarded GAL actions still enforce coordinator revisions, receipts, and task-quality checks. The Stop handler blocks premature final responses until the coordinator is terminal or reports a typed human-required state.

If guarded continuation stops, keep the workspace and session identity unchanged while diagnosing it. Run `gal doctor` for static readiness, then inspect coordinator and attempt evidence under `.dev/pipeline/<plan-scope>/`. Fix trust, manifest, or handler problems and run `gal refresh` when projected assets are stale. Start a new trusted same-session handshake for a fresh grant. Do not copy, hand-author, or reuse grants. Preserve conflicting identity or evidence state until the conflict is resolved. Use ordinary `gal pipeline <prompt>` to continue through the existing host-owned flow without guarded Codex behavior.

An ordinary `gal pipeline <prompt>` invocation without a valid grant remains on `LegacyInteractive` and preserves the existing v1 receipt, marker, exit, and mutation behavior. A host-neutral ordinary invocation cannot infer that Codex hooks are absent, disabled, or untrusted when those hooks never run. An explicit grant-required guarded entry fails before implementation when the grant is missing or invalid. Codex hooks are projected only to the Codex plugin. Claude Code and OpenCode retain their existing host-owned continuation behavior.

GAL's shared skills and command prose may include Codex-specific instructions. Such prose differences are allowed where a host requires them. They do not permit projecting Codex hooks to another host or changing that host's invocation and continuation contract.

## Agent Projection Matrix

GAL transforms role definitions from `*.agent.md` into runtime-native agent or skill formats, rather than performing simple file copies:

| Runtime | Target Format | Projection Target | Mechanism |
| --- | --- | --- | --- |
| Claude Code | `*.agent.md` with GAL headers | `~/.gal/plugins/gal/agents/` | Mapped directly to Claude subagents. |
| OpenAI Codex | Flat `*.toml` without nested tables | `~/.codex/agents/<name>.toml` | Codex TOML subagents (`crates/projection/src/codex_agent.rs`). |
| Google Antigravity | Projected skills | `~/.gemini/antigravity-cli/skills/` | Mounted through skills directory, with native agent files unverified. |
| opencode | `<name>.md` with `mode: subagent` front matter | `~/.config/opencode/agents/<name>.md` | Rendered via `render_opencode_agent`. |
| GitHub Copilot | `*.agent.md` mapped to Claude tool names | `~/.copilot/agents/<name>.agent.md` | Fallback files generated when `pluginMode.copilot` is `false`. When `true`, Copilot uses native plugin registration and fallback files are removed. |

The `pluginMode.copilot` setting in `~/.gal/config/config.json` determines Copilot fallback generation. Setting it to `true` indicates native plugin registration is complete, prompting GAL to remove fallback agent files and retain only command skills.

Codex agent files use a flat TOML structure. The serializer `serialize_codex_toml` defines six fields (`name`, `description`, `developer_instructions`, `model`, `model_reasoning_effort`, `sandbox_mode`) at the document root without subtables. Because Codex rejects files containing unrecognized or legacy nested keys, strict flat schemas surface schema divergence immediately during `codex doctor` checks.

### Role Execution Modes

GAL agent personas support two operational modes:

- **Isolated Mode** (`/gal architect`): Launches a dedicated subagent to execute the task, returning only a summary verdict to the main conversation.
- **In-Context Mode** (`/gal discuss architect`): Injects role rules and developer instructions directly into the active session, enabling multi-turn interactive consultation.

Because Codex lacks native dynamic role injection, `gal refresh` generates supplementary `discuss-<role>/SKILL.md` skills in `~/.agents/skills/` for the four planning review roles (`architect`, `analyst`, `designer`, `releaser`). Invoking these skills loads role instructions directly into the active thread without launching subagents. These directories are registered on the reserved directory list to prevent accidental removal during cleanup.

## Exclusion of Orchestrator-Driven Roles

For the four pipeline roles (`implementer`, `tester`, `auditor`, and `researcher`), `update_agents` in `crates/projection/src/lib.rs` is **strictly forbidden** from generating projection files (`*.agent.md` or `*.toml`). While these roles exist in the internal `KNOWN_GOLEMS` registry for pipeline resolution, they have no user-facing agent files. This prevents unmonitored execution: running `/gal tester` directly returns an unknown-command error, ensuring execution occurs only under orchestrator oversight.

Role definitions declare abstract permission scopes (such as `read`, `edit`, `execute`, `search`, and `web`). The module `crates/projection/src/tool_map.rs` translates these permissions into runtime-specific tool identifiers. In Codex, sandbox isolation levels map accordingly: planning review roles run in `read-only` mode, whereas pipeline implementation runs in `workspace-write` mode.

## Repository Adapter Generation

Commands `gal init` and `gal render-adapters` generate two adapter roots (`AGENTS.md` and `CLAUDE.md`) and two conditional Rust rule layers (`.claude/rules/gal-rust.md` and `.github/instructions/gal-rust.instructions.md`). These files are generated by `crates/cli/src/gal/render.rs`:

```text
.dev/project.md
   │
   ▼
Byte count validation (30,720 B maximum, LF-normalized UTF-8)
   │  fail → Abort without modifying files (see ADR 06)
   ▼
Validate 8 required H2 section headings (each must appear once)
   │  missing or duplicated → Reject and report offending heading
   ▼
Generate 2 adapter roots and 2 conditional Rust layers (if Rust enabled)
   │
   ▼
Execute pre-write validation:
   ├─ Per-root file size validation (32,768 B maximum)
   ├─ Ownership marker validation and collision check
   │  fail → Abort immediately without partial file writes
   ▼
Remove retired root files and prune empty parent directories
   │
   ▼
Write files and report status (Written, Unchanged, or Removed)
```

Target paths are defined centrally in `render.rs` across two lists: `REPO_ADAPTER_ROOTS` (for `AGENTS.md` and `CLAUDE.md`) and `REPO_ADAPTER_CONDITIONAL_LAYERS` (for conditional rule files). Adapter rendering routines, CLI status reports, and finalize idempotency checks (`finalize_check.rs::check_sync_idempotency`) all reference these definitions. The idempotency check evaluates renders in memory without writing to disk.

The `render_and_apply_repo_adapters` function validates candidates before writing. Adapter roots are written before conditional layers in a single operation. Cross-file rollback mechanisms are omitted because all validation occurs in preflight checks.

The sync routine returns a `ProjectionReport` tracking modified and removed paths. Reporting modules wrap this into formatted CLI summaries (`Written`, `Unchanged`, `Removed`).

## Adapter Ownership Validation

The function `classify_root_ownership` inspects the initial line of adapter files. If the header matches the generation marker (`SlimRuntime::generated_marker()`), the file is marked `ManagedByGal` and updated in place. If the marker is missing, the file is classified as user-maintained (`HandOwned`), and GAL leaves it unmodified. For conditional Rust rule files, ownership is established by the presence of `<!-- GAL-generated: gal init -->`.

If an existing file lacks GAL ownership markers, generation aborts before modifying any files to prevent data loss. To update configuration, modify the version-controlled source files (`.dev/project.md` or `plugins/gal-core/conventions/rust.md`) and rerun `gal render-adapters`.

## Conditional Rust Rule Layers

Adapter roots do not embed full convention texts. For Rust projects, GAL distributes rules using two approaches depending on runtime capabilities:

1. **Conditional Rule Files**: Runtimes supporting directory-based rule loading (such as Claude Code and GitHub Copilot) receive dedicated rule files (`.claude/rules/gal-rust.md` and `.github/instructions/gal-rust.instructions.md`).
2. **Directive Pointers**: For Codex, Antigravity, and opencode, a single instruction line is prepended to `AGENTS.md` requiring models to read `plugins/gal-core/conventions/rust.md` prior to Rust tasks.

## The Projection Registry (`plugins.lock.json`)

The registry file `~/.gal/state/plugins.lock.json` tracks projected assets. GAL manages only the `_galProjection` top-level namespace, while other top-level keys remain untouched. Each write saves an atomic snapshot of projected assets and their source mappings:

| Subfield of `_galProjection` | Type | Description |
| --- | --- | --- |
| `schemaVersion` | Number | Registry schema version (e.g., `2`). |
| `agentProjectionPaths` | Array | Managed agent file paths. |
| `commandProjectionPaths` | Array | Managed command file paths. |
| `skillProjectionPaths` | Array | Managed skill file paths. |
| `discussSkillProjectionPaths` | Array | Managed Codex discuss-skill paths. |
| `codexAgentProjectionPaths` | Array | Managed Codex agent paths. |
| `legacyProjectionPaths` | Array | Managed legacy adapter paths. |
| `sourceAttribution` | Object | Map of file paths to source identifiers for multi-source tracking. |
| `pluginOwned` | Object | Map of runtime identifiers (`claude`, `codex`, `copilot`, `agy`) to plugin classification keys. |

GAL restricts operations exclusively to the `_galProjection` namespace. External keys are neither parsed nor modified.

## Canonical MCP Manifest (`.mcp.json`)

The manifest at `~/.gal/plugins/gal/.mcp.json` is generated during `gal refresh` by combining `plugins/gal-core/mcp.json` with personal servers in `~/.gal/local/mcp.json` using the Core-Wins rule:

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `mcpServers` | Yes | Object | Server definitions matching the schema of `plugins/gal-core/mcp.json`. |
| `inputs` | No | Array | Prompt-driven input definitions passed through verbatim. |

By default, `plugins/gal-core/mcp.json` contains an empty servers map. On machines without personal servers, generation emits an empty manifest. If core capabilities require an MCP server in the future, declaring it in the source file automatically populates the manifest.

Environment variable syntax (`${VAR}`) passes through without modification. Variables are resolved at runtime by the MCP host process. GAL does not store credentials or secrets within MCP manifests.

During generation, the source `"servers"` key is renamed to `"mcpServers"` to comply with protocol specifications. The package root also includes an `mcp.json` conforming to Agent Plugins 1.0.0 specifications. Note that `gal update` displays version info only. Manifest regeneration occurs exclusively via `gal refresh`.

## MCP Configuration Boundaries

GAL manages its own plugin manifests exclusively. It never edits host-managed configuration files, including `claude_desktop_config.json`, `~/.copilot/mcp-config.json`, `~/.codex/config.toml`, `opencode.json`, or `mcp_config.json`. Host tool configurations remain user-managed.

## Regenerating After Changes

Changes made to `plugins/gal-core/` do not hot-reload automatically. Apply changes based on the modified layer:

- **Edits to `plugins/gal-core/`** (skills, agents, commands, conventions): Run `gal refresh --source ./plugins/gal-core`.
- **Rust code changes in `crates/`**: Rebuild and install the binary onto `PATH` (`cargo build --release -p gal-cli`), then run `gal refresh --source ./plugins/gal-core`.

Updating the binary before running `gal refresh` is required. Reversing the order can cause subcommands to fail due to missing binary support. Under directory-junction mode, Claude Code loads changes on the next turn. Marketplace installations require restarting the application.

| Modified Component | Required Command | Verification Step |
| --- | --- | --- |
| Skill, agent, command, or convention in `plugins/gal-core/` | `gal refresh --source ./plugins/gal-core` | Execute the updated command in your target runtime. |
| Core or personal MCP configuration (`mcp.json`) | `gal refresh --source ./plugins/gal-core` | Reload servers in your MCP host. |
| Rust source code in `crates/` | Rebuild and install binary on `PATH` | Run `gal refresh --source ./plugins/gal-core`. |
| `.dev/project.md` or Rust conventions | Save source file | Run `gal render-adapters`. |
| `~/.gal/config/config.json` | Save configuration file | Takes effect immediately without projection. |

When executing `gal refresh` in development environments, specify `--source ./plugins/gal-core`. Without this flag, `gal refresh` expects assets adjacent to the executable, which can fail when running from `~/.cargo/bin`.

Note the distinction between `gal render-adapters` (which updates repo-local adapter files) and `gal refresh` (which updates machine-level agent projections). To reset an initialized repository, delete `.dev/project.md` and `.dev/state.md` manually before running `gal init`.

### Diagnostic Detection (`gal doctor`)

The `gal doctor` command detects four forms of configuration drift:

1. **Skill projection divergence**: Flags projected skills that differ from canonical sources.
2. **opencode projection drift**: In `--dry-run` mode, highlights files in `~/.config/opencode/` diverging from canonical renders. Run `gal refresh` to resolve.
3. **Binary skew**: Compares embedded `GAL_GIT_STAMP` against workspace `git rev-parse --short HEAD`. Mismatches indicate `crates/` was modified without recompiling the binary on `PATH`.
4. **Plugin mode mismatch**: Compares each runtime's `pluginMode` setting with observed GAL registration entry evidence; it does not verify that a runtime loaded the entry. A registered runtime with `pluginMode=false` receives a warning and exits with code 0; confirm the registration before enabling plugin mode, or keep `pluginMode=false` and run `gal refresh` to retain fallback projection. A runtime with `pluginMode=true` but no observed registration entry also receives a warning and exits with code 0; complete registration, or set `pluginMode=false` and run `gal refresh` to restore fallback projection.

When evaluating integrations for new AI runtimes, follow four design criteria:

1. Identify whether the runtime supports a machine-level configuration directory.
2. Determine whether repository instructions can consume `AGENTS.md` directly or require dedicated adapters.
3. If native commands are supported, render them from shared templates. Otherwise, project command skills into supported skill paths.
4. Implement configuration bridges only if the runtime provides non-destructive configuration extension mechanisms.

Following agent renames or routing changes, maintainers must run `gal refresh` to project updated definitions and run `gal render-adapters` across local repositories.

## Verification Checklist

When updating rendering or projection logic, verify:

- All supported runtimes have valid projection target directories.
- Rendered files in `commands/*/SKILL.md` contain no unexpanded template variables (e.g., `{{GAL_ROOT}}`).
- Antigravity command skills regenerate cleanly at `~/.gemini/antigravity-cli/skills/<name>/SKILL.md`.
- Shared skill directories contain general skills without duplicate aliases.
- Host-level MCP configuration files were not modified.
- Path resolution functions in `gal doctor` resolve system paths once through `DoctorPathContext::from_standard_paths` rather than invoking `user_home()` independently.

Automated test suites must verify byte-level equivalence between projected files and canonical build contracts across all entries in `VALID_RUNTIMES`. Never edit `AGENTS.md`, `CLAUDE.md`, or projected skills manually. All updates must flow from source contracts via `gal render-adapters` or `gal refresh`.
