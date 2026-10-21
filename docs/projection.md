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
2. **Repository-level generation**: Compiles `.dev/project.md` into repo-local adapter files, such as `AGENTS.md`.

The projection pipeline is divided into three distinct modules with strict boundaries:

- **Canonical root rendering** (`crates/gal-engine/src/render/`): Compiles `plugins/gal-core/` and optional user customizations from `~/.gal/local/` into the canonical root at `~/.gal/plugins/gal/`.
- **Runtime surface projection** (`crates/projection/`): Projects canonical root assets into each target runtime's configuration directory using the formats that tool supports.
- **Repository adapter generation** (`crates/cli/src/gal/render.rs`): Compiles a repository's `.dev/project.md` into `AGENTS.md`. This routine runs independently without calling the previous two modules.

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
        │      ~/.gemini/antigravity-cli/skills/ + ~/.gemini/antigravity-cli/gal link to ~/.gal/source
        │
        └──▶ opencode
               ~/.config/opencode/commands/ + ~/.config/opencode/agents/
```

Command and skill content delivered to runtimes must be byte-identical to the pre-rendered canonical root. Delivery mechanisms may differ across environments. Sandbox permissions and invocation trigger syntax may also differ. Shared prose may differ when a runtime requires host-specific instructions. These differences do not change the shared command behavior.

## Canonical Root Architecture

Each machine maintains exactly one canonical root located at `~/.gal/plugins/gal/`. Files consumed by AI runtimes are copies or symlinks projected from this directory, and never independent sources of truth.

Projection operates as a one-way synchronization push from the canonical root to target runtimes. Runtimes do not write back modifications, ensuring runtime files can be deterministically rebuilt from the canonical root at any time.

The projection source is the GAL source root, the directory that holds `skills/`, `agents/`, and `commands/`. In a checkout, this is `plugins/gal-core/`. Skill and agent projection reads from that root. The lookup order is described under Source Lookup Order below.

The canonical root contains these paths. Each runtime projection copies or links from them.

| Path | Contents |
| --- | --- |
| `skills/<name>/SKILL.md` | Skills from the source root. |
| `commands/<name>.md` | Command files. Claude Code loads them through the plugin. |
| `agents/<name>.md` | Agent files for Claude Code, with frontmatter limited to Claude keys. The orchestrated-only list does not apply here. |
| `agy-agents/<name>.agent.md` | Verbatim agent copies. The OpenCode drift check reads them. |
| `com.github.copilot/agents/<name>.agent.md` | Copilot agent files, with tool names mapped and orchestrated-only roles removed. |
| `com.github.copilot/rules/` | Copilot copies of the files in `rules/`. |
| `rules/gal.md` | The GAL instruction corpus. |
| `.claude-plugin/plugin.json` | The Claude Code plugin manifest. |
| `.codex-plugin/plugin.json` | The Codex plugin manifest, including the hook handlers. |
| `.mcp.json` and `mcp.json` | MCP manifests in the Claude and Agent Plugins layouts. |
| `bin/` | The GAL executable. Written only for machine-local renders. |

Outside the canonical root, GAL writes `~/.gal/plugins/.claude-plugin/marketplace.json` and `~/.gal/plugins/.agents/plugins/marketplace.json`. When Antigravity is selected, GAL also creates `~/.gal/source`, a link to the source root. A materialized embedded source is stored in `~/.gal/embedded-src`.

GAL replaces the canonical root by renaming the old root to a backup and moving the new render into place. When the old root cannot be renamed, GAL falls back to a non-atomic in-place sync that overwrites changed files and removes stale entries. This happens when another process holds the root open, or on Windows when a process has its working directory at the root. The same rule applies to `~/.gal/embedded-src`. A process stopped during the in-place sync can leave a partly updated root, and the next `gal refresh` repairs it.

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

Runtimes with native command systems (Claude Code and opencode) do not receive redundant command-skill projections, preventing duplicate command listings. While `~/.agents/skills/` is shared across Codex, opencode, and Copilot, command skills are projected there only when Codex is enabled. If only opencode is enabled, only native command files are created. Claude Code uses a fallback when its GAL plugin is not registered. GAL then writes each command to `~/.claude/commands/<name>.md`, and it leaves a user-owned file at that path in place.

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
| Claude Code | `agents/<name>.md` in the canonical root, with frontmatter limited to Claude keys and `tools` mapped to Claude names | `~/.gal/plugins/gal/agents/` | Loaded as plugin subagents. The canonical render does not apply the orchestrated-only list to this tree. |
| OpenAI Codex | Flat `*.toml` without nested tables | `~/.codex/agents/<name>.toml` | Codex TOML subagents (`crates/projection/src/codex_agent.rs`). Orchestrated-only roles are skipped. |
| Google Antigravity | Projected skills | `~/.gemini/antigravity-cli/skills/` | Skill directories only. Agents stay in the canonical root as `agy-agents/<name>.agent.md`, and no projection writes agent files here. Native agent support is unverified. |
| opencode | `<name>.md` with `mode: subagent` front matter | `~/.config/opencode/agents/<name>.md` | Rendered via `render_opencode_agent`. |
| GitHub Copilot | `*.agent.md` mapped to Claude tool names | `~/.copilot/agents/<name>.agent.md` | Fallback files are projected unless GAL confirms plugin registration. Orchestrated-only roles are skipped. Command skills remain projected. |

GAL detects registration from Copilot's `enabledPlugins` setting. Confirmed registration makes the plugin own agents. Missing or uncertain registration keeps fallback agents available. Uncertain evidence produces a warning. Registration changes take effect on the next refresh, and GAL only prunes or restores GAL-owned fallback files. User-owned collisions are preserved.

Codex agent files use a flat TOML structure. The serializer `serialize_codex_toml` defines six fields (`name`, `description`, `developer_instructions`, `model`, `model_reasoning_effort`, `sandbox_mode`) at the document root without subtables. Because Codex rejects files containing unrecognized or legacy nested keys, strict flat schemas surface schema divergence immediately during `codex doctor` checks.

### Role Execution Modes

GAL agent personas support two operational modes:

- **Isolated Mode** (`/gal architect`): Launches a dedicated subagent to execute the task, returning only a summary verdict to the main conversation.
- **In-Context Mode** (`/architect discuss`): Injects role rules and developer instructions directly into the active session, enabling multi-turn interactive consultation.

The four standalone role commands come from `plugins/gal-core/commands/{architect,analyst,designer,releaser}/SKILL.template.md`. The existing command scanner and renderer handle them like any other command, with no new projection path. Each command defaults to `gal dispatch-script golem-<role>` and, when its first argument is `discuss`, runs the internal `gal consult-script golem-<role>`.

The rendered products and the runtime install modes are separate facts:

- **Canonical products**: The renderer writes one command and one matching baked skill per role under the canonical root. Repeat renders are identical.
- **Install modes**: Claude Code and opencode use the native command. In Claude Code plugin mode, the plugin supplies the command as `/gal:<role>`, and no extra fallback file is written. Copilot, Codex, and Antigravity receive command skills through the existing fallback projection.

Projection no longer creates `discuss-<role>` skills, and `discussSkillProjectionPaths` is retired. `gal refresh` removes the four retired GAL-managed directories and preserves unrecorded user directories. No redirect replaces the removed `$discuss-<role>` skills. Use the replacement for each runtime:

| Runtime | Replacement |
| --- | --- |
| Claude Code | `/<role> discuss` |
| Claude Code plugin mode | `/gal:<role> discuss` |
| Codex | `$<role> discuss` |

Codex native-agent definitions (`~/.codex/agents/<name>.toml`) and their `model` and `effort` projection are unchanged.

## Exclusion of Orchestrator-Driven Roles

For the four pipeline roles (`implementer`, `tester`, `auditor`, and `researcher`), the `ORCHESTRATED_ONLY_GOLEM_NAMES` list in `crates/projection/src/lib.rs` names the roles that native subagent projection skips. `update_skills` in the same file checks this list before it writes Copilot fallback agents to `~/.copilot/agents/` and Codex TOML agents to `~/.codex/agents/`. The canonical render applies the same list to `com.github.copilot/agents/`. It does not apply the list to the canonical `agents/` tree, so those four roles remain in that tree. These roles stay in `KNOWN_GOLEMS` so the pipeline can resolve and dispatch them under orchestrator control.

Role definitions declare abstract permission scopes (such as `read`, `edit`, `execute`, `search`, and `web`). The module `crates/projection/src/tool_map.rs` translates these permissions into runtime-specific tool identifiers. In Codex, sandbox isolation levels map accordingly: planning review roles run in `read-only` mode, whereas pipeline implementation runs in `workspace-write` mode.

## Repository Adapter Generation

Commands `gal init` and `gal render-adapters` generate one adapter root (`AGENTS.md`) and two conditional Rust rule layers (`.claude/rules/gal-rust.md` and `.github/instructions/gal-rust.instructions.md`). These files are generated by `crates/cli/src/gal/render.rs`:

For adapters that include the skill index, the generated `AGENTS.md` lists repository content in two separate sections. `Repo Skills` lists discovered skills. `Repo Commands` lists commands found as real `SKILL.template.md` files under the repository's command directories, each with its directory name and repository-relative path, sorted by name. A skill and a command with the same name appear in their own sections. GAL does not list a command whose root or template is absent, and it does not invent paths for commands from external sources.

Generation does not guarantee that your coding agent loads `AGENTS.md`. An existing `CLAUDE.md` or similar instruction file may cause the agent to skip it, depending on the agent's loading design. Check your coding agent's documentation for supported filenames, file discovery, and loading precedence. See [setup guidance](setup.md#initializing-a-repository-gal-init).

```text
.dev/project.md
   │
   ▼
Byte count check (30,720 B advisory budget, LF-normalized UTF-8)
   │  over budget → print one warning and continue (see conventions/token-budget.md)
   ▼
Validate 8 required H2 section headings (each must appear once)
   │  missing or duplicated → Reject and report offending heading
   ▼
Generate 1 adapter root and 2 conditional Rust layers (if Rust enabled)
   │
   ▼
Execute pre-write validation:
   ├─ Per-root file size validation (32,768 B maximum)
   ├─ Ownership marker validation and collision check
   │  fail → Abort immediately without partial file writes
   ▼
Remove GAL-owned retired root files, including CLAUDE.md, and prune empty parent directories
   │
   ▼
Write files and report status (Written, Unchanged, or Removed)
```

Target paths are defined centrally in `render.rs` across two lists: `REPO_ADAPTER_ROOTS` (for `AGENTS.md`) and `REPO_ADAPTER_CONDITIONAL_LAYERS` (for conditional rule files). Adapter rendering routines, CLI status reports, and finalize idempotency checks (`finalize_check.rs::check_sync_idempotency`) all reference these definitions. The idempotency check evaluates renders in memory without writing to disk.

The `render_and_apply_repo_adapters` function validates candidates before writing. Adapter roots are written before conditional layers in a single operation. Cross-file rollback mechanisms are omitted because all validation occurs in preflight checks.

The sync routine returns a `ProjectionReport` tracking modified and removed paths. Reporting modules wrap this into formatted CLI summaries (`Written`, `Unchanged`, `Removed`).

### Projection ownership for installed and source builds

The package manager owns the installed GAL executable. `gal refresh` updates the shared projection from the installed GAL, while `gal dev-apply` deploys a source checkout into it and must only be run by a maintainer. A source-worktree acceptance run uses `gal dev-apply --source <path> --root <path>` and projects only beneath that root. GAL applies `reject-before-write` to missing, relative, profile, and shared installation roots before writing any files. Environment variables such as `HOME` and `USERPROFILE` do not authorize a projection root. Keep source-build output inside the worktree's private runtime lane.

### Source Lookup Order

`gal refresh` searches for the source root in this order:

1. The directory that contains the GAL binary, then its ancestors.
2. The embedded source, materialized to `~/.gal/embedded-src`.

If neither location has a source root, `gal refresh` exits with an error and writes nothing. It does not search the current directory. `gal dev-apply` takes the source from `--source`, or from the current checkout when `--source` is absent. `--root` requires `--source`.

`gal render-adapters` searches differently. It walks up from the current directory to the first ancestor that contains `.git`. It then checks the binary's directory and its ancestors, and finally the embedded source.

## Adapter Ownership Validation

Before any write, GAL classifies each root adapter and conditional layer into one of three states:

| State | Root adapter (`AGENTS.md`) | Conditional layer |
| --- | --- | --- |
| `Absent` | The file does not exist. GAL creates it. | The file does not exist. GAL creates it when the Rust convention is selected. |
| `GalOwned` | The first line is the exact generated marker. GAL updates the file in place. | The file contains `<!-- GAL-generated: gal init -->`. GAL updates it, or removes it when the Rust convention is no longer selected. |
| `Collision` | The file exists without the marker. GAL aborts the whole call before any write. | The file exists without the marker. GAL aborts the whole call before any write. |

`classify_root_ownership` and `classify_layer_ownership` in `crates/cli/src/gal/render.rs` perform these checks. A collision on either conditional layer aborts the whole call, so one layer is never written while the other is refused.

Before the new root is written, GAL removes the retired root adapters `.agents/rules/gal.md`, `.github/copilot-instructions.md`, `CLAUDE.md`, and `GEMINI.md`. It removes one only when its first line matches the marker recorded for that path. A file without that marker is kept, and GAL reports it as hand-owned.

If an existing file lacks GAL ownership markers, generation aborts before modifying any files to prevent data loss. To update configuration, modify the version-controlled source files (`.dev/project.md` or `plugins/gal-core/conventions/rust.md`) and rerun `gal render-adapters`.

## Conditional Rust Rule Layers

Adapter roots do not embed full convention texts. For Rust projects, GAL distributes rules using two approaches depending on runtime capabilities:

1. **Conditional Rule Files**: Runtimes supporting directory-based rule loading (such as Claude Code and GitHub Copilot) receive dedicated rule files (`.claude/rules/gal-rust.md` and `.github/instructions/gal-rust.instructions.md`).
2. **Rust Convention Trigger**: The shared `AGENTS.md` root, which GAL renders for Codex CLI, Copilot CLI, Antigravity, and opencode, has no native rule layer. When the Rust convention is selected, GAL adds a `## Rust Convention Trigger` section after the convention list. The section tells the model to read `plugins/gal-core/conventions/rust.md` before any Rust or Cargo work.

## The Projection Registry (`plugins.lock.json`)

The registry file `~/.gal/state/plugins.lock.json` tracks projected assets. GAL manages only the `_galProjection` top-level namespace, while other top-level keys remain untouched. At the end of a projection pass, GAL rewrites the file only when the projected state changed. The rewrite keeps every top-level key outside `_galProjection`, and a dry run writes nothing. The `_galProjection` object records the projected assets and their source mappings:

| Subfield of `_galProjection` | Type | Description |
| --- | --- | --- |
| `schemaVersion` | Number | Registry schema version (e.g., `2`). |
| `agentProjectionPaths` | Array | Managed agent file paths. |
| `commandProjectionPaths` | Array | Managed command file paths. |
| `skillProjectionPaths` | Array | Managed skill file paths. |
| `discussSkillProjectionPaths` | Array | Retired metadata read during cleanup of old GAL-managed consult skills. |
| `codexAgentProjectionPaths` | Array | Managed Codex agent paths. |
| `legacyProjectionPaths` | Array | Managed legacy adapter paths. |
| `sourceAttribution` | Object | Map of file paths to source identifiers for multi-source tracking. |
| `pluginOwned` | Object | Map of runtime identifiers (`claude`, `codex`, `copilot`, `agy`) to plugin classification keys. |

`pluginOwned` is derived from the registration snapshot for the current refresh. It records only categories omitted because registration was confirmed. `Unknown` never records plugin ownership. Claude owns commands, Codex owns core skills, Copilot owns agents, and Antigravity owns command skills. Codex TOML agents and commands, Copilot command skills, and the Antigravity canonical plugin link remain projected. OpenCode always receives native commands and agents plus shared `~/.agents/skills`, so selecting OpenCode keeps shared skills projected even when Codex registration is confirmed.

GAL observes registration once before projection and reuses that immutable snapshot for the skill and command passes. Runtime registration is the routing authority rather than a separate `pluginMode` preference, which prevents one refresh from combining contradictory states. Missing or unreadable evidence selects projection fallback because retaining a GAL-managed duplicate is safer than deleting a fallback without confirmed plugin ownership.

GAL restricts operations exclusively to the `_galProjection` namespace. External keys are neither parsed nor modified.

## Canonical MCP Manifest (`.mcp.json`)

The manifest at `~/.gal/plugins/gal/.mcp.json` is generated during `gal refresh` by combining `plugins/gal-core/mcp.json` with personal servers in `~/.gal/local/mcp.json` using the Core-Wins rule:

| Field | Required | Type | Description |
| --- | --- | --- | --- |
| `mcpServers` | Yes | Object | Server definitions matching the schema of `plugins/gal-core/mcp.json`. |
| `inputs` | No | Array | Prompt-driven input definitions passed through verbatim. |

By default, `plugins/gal-core/mcp.json` contains an empty servers map. On machines without personal servers, generation emits an empty manifest. If core capabilities require an MCP server in the future, declaring it in the source file automatically populates the manifest.

Environment variable syntax (`${VAR}`) passes through without modification. Variables are resolved at runtime by the MCP host process. GAL does not store credentials or secrets within MCP manifests.

During generation, the source `"servers"` key is renamed to `"mcpServers"` to comply with protocol specifications. The package root also includes an `mcp.json` conforming to Agent Plugins 1.0.0 specifications. Note that `gal update` displays version info only. Manifest regeneration occurs via `gal refresh` or `gal dev-apply`.

## MCP Configuration Boundaries

GAL manages its own plugin manifests exclusively. It never edits host-managed configuration files, including `claude_desktop_config.json`, `~/.copilot/mcp-config.json`, `~/.codex/config.toml`, `opencode.json`, or `mcp_config.json`. Host tool configurations remain user-managed.

## Regenerating After Changes

Changes made to `plugins/gal-core/` do not hot-reload automatically. Apply changes based on the modified layer:

- **Edits to `plugins/gal-core/`** (skills, agents, commands, conventions): Run `gal dev-apply`.
- **Rust code changes in `crates/`**: Rebuild and install the binary onto `PATH` (`cargo build --release -p gal-cli`), then run `gal dev-apply`.

Update the binary before deploying with `gal dev-apply`. Reversing the order can cause subcommands to fail due to missing binary support. Under directory-junction mode, Claude Code loads changes on the next turn. Marketplace installations require restarting the application.

| Modified Component | Required Command | Verification Step |
| --- | --- | --- |
| Skill, agent, command, or convention in `plugins/gal-core/` | `gal dev-apply` | Execute the updated command in your target runtime. |
| Core MCP configuration (`plugins/gal-core/mcp.json`) | `gal dev-apply` | Reload servers in your MCP host. |
| Personal MCP configuration (`~/.gal/local/mcp.json`) | `gal refresh` | Reload servers in your MCP host. |
| Rust source code in `crates/` | Rebuild and install binary on `PATH` | Run `gal dev-apply`. |
| `.dev/project.md` or Rust conventions | Save source file | Run `gal render-adapters`. |
| `~/.gal/config/config.json` | Save configuration file | Takes effect immediately without projection. |

Run `gal dev-apply` from the checkout to deploy it, because `gal refresh` renders the installed GAL and ignores the checkout.

Note the distinction between `gal render-adapters` (which updates repo-local adapter files) and `gal refresh` (which updates machine-level agent projections). To reset an initialized repository, delete `.dev/project.md` and `.dev/state.md` manually before running `gal init`.

### Diagnostic Detection (`gal doctor`)

The `gal doctor` command detects four forms of configuration drift:

1. **Skill projection divergence**: Flags projected skills that differ from canonical sources.
2. **opencode projection drift**: Flags files in `~/.config/opencode/` that diverge from canonical renders. Run `gal refresh` to resolve.
3. **Binary skew**: Compares embedded `GAL_GIT_STAMP` against workspace `git rev-parse --short HEAD`. Mismatches indicate `crates/` was modified without recompiling the binary on `PATH`.
4. **Plugin registration evidence**: `gal doctor` uses the same registration observer as projection. Uncertain evidence produces a warning with the source and reason. Registration evidence describes local configuration and does not prove that a running runtime loaded the plugin. Existing `pluginMode` values are reported by the retired-key check. `gal doctor --dry-run` is accepted, but every check is read-only, so the flag does not change the findings.

When evaluating integrations for new AI runtimes, follow four design criteria:

1. Identify whether the runtime supports a machine-level configuration directory.
2. Determine whether repository instructions can consume `AGENTS.md` directly or require dedicated adapters.
3. If native commands are supported, render them from shared templates. Otherwise, project command skills into supported skill paths.
4. Implement configuration bridges only if the runtime provides non-destructive configuration extension mechanisms.

Following agent renames or routing changes, maintainers must run `gal dev-apply` to project updated definitions and run `gal render-adapters` across local repositories.

## Verification Checklist

When updating rendering or projection logic, verify:

- All supported runtimes have valid projection target directories.
- Rendered files in `commands/*/SKILL.md` contain no unexpanded template variables (e.g., `{{GAL_ROOT}}`).
- Antigravity command skills regenerate cleanly at `~/.gemini/antigravity-cli/skills/<name>/SKILL.md`.
- Shared skill directories contain general skills without duplicate aliases.
- Host-level MCP configuration files were not modified.
- Path resolution functions in `gal doctor` resolve system paths once through `DoctorPathContext::from_standard_paths` rather than invoking `user_home()` independently.

Automated test suites must verify byte-level equivalence between projected files and canonical build contracts across all entries in `VALID_RUNTIMES`. Never edit `AGENTS.md`, or projected skills manually. All updates must flow from source contracts via `gal render-adapters`, `gal refresh`, or `gal dev-apply`.
