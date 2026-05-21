# Developer Guide

This document is maintainer navigation, not a second specification. Use it to decide which layer you are changing, which source files own that layer, and which rules you must not break.

## Start By Finding The Right Layer

| If you are changing... | Ask first... | Read these source files |
| --- | --- | --- |
| `/gal` command surface, aliases, or dispatch | is this control-plane behavior or runtime plumbing? | [../commands/commands.md](../commands/commands.md), [../scripts/scripts.md](../scripts/scripts.md) |
| planning flow or optional collaborative-tool semantics | is this GAL-native planning, optional gstack behavior, or workflow teaching? | [../commands/commands.md](../commands/commands.md), [collaborative-tools/gstack.md](collaborative-tools/gstack.md), [../workflows/coding.md](../workflows/coding.md) |
| setup, install topology, baked command files, or MCP merge | is this machine-layer install or repo-layer adapter generation? | [../scripts/scripts.md](../scripts/scripts.md), `scripts/Setup-Machine.ps1`, `scripts/Update-*.ps1`, `scripts/setup-machine.sh`, `scripts/update-*.sh` |
| templates and plan lifecycle | which file should own this information? | [../templates/templates.md](../templates/templates.md), [../workflows/coding.md](../workflows/coding.md) |
| xmachine execution behavior | is this part of the main workflow or an execution-plane extension? | [collaborative-tools/xmachine.md](collaborative-tools/xmachine.md), xmachine scripts under `scripts/` |
| Godot or graphics workflows | is this repo-wide methodology or a module-specific lane? | [collaborative-tools/godot.md](collaborative-tools/godot.md), [collaborative-tools/graphworkflow.md](collaborative-tools/graphworkflow.md) |

If you cannot tell which layer you are touching, stop and resolve that first. Most broken refactors in GAL come from mixing README, docs, templates, scripts, and command contracts in one change.

## Non-Negotiable Rules

### 1. Markdown owns the durable contract

- Methodology, rules, and contracts live in tracked Markdown and source files.
- Generated adapters, baked command files, and runtime configs are outputs, not source inputs.

### 2. `/gal` only solves control-plane problems

- `/gal` should not wrap a second copy of tester, reviewer, designer, security, debugger, or releaser work.
- Execution-stage specialist behavior belongs in agents.
- Planning commands can run directly because they are still part of the public command surface.

### 3. Repo-local state is the ownership boundary

- `.dev/`, `docs/plans/`, `docs/designs/`, and similar repo-local files are the shared working state.
- Do not move GAL's core state back into user-global storage.

### 4. Missing tools must not look like success

- GAL uses skill-level routing, not one repo-wide CLI-first or MCP-first rule.
- Each external-tool skill should define a preferred path, a fallback path, and a no-tool behavior.

### 5. Do not optimize one runtime by breaking portability

- If a change makes Copilot, Antigravity, and Codex diverge in contract or file flow, it is usually the wrong change.
- README, docs, templates, and setup scripts should preserve cross-runtime parity first.

### 6. Navigation docs must not become a second spec

- If a document exists to help humans find the real source, keep it short and directional.
- Summaries are useful. Duplicate contracts are not.

### 7. Collaborative-tool routing belongs to the workflow layer

- Do not make an agent silently switch personas or contracts because a collaborative tool was detected.
- The workflow decides the collaborative tool first, then the tool writes back into the same repo-owned files.

The shared preflight model lives in [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md).

## Runtime Topology For Setup Work

This section absorbs the setup topology that maintainers need when changing `Setup-Machine`, the `Update-*` scripts, command installation, or MCP wiring.

### Four Runtime Layers

| Layer | Location | Purpose |
| --- | --- | --- |
| Layer 1 | the GAL repo | main methodology source |
| Layer 1.5 | tool config directories such as `~/.copilot/`, `~/.gemini/`, `~/.gemini/antigravity-cli/`, `~/.codex/`, `~/.claude/`, plus `~/.gal/install-state.json` | installed skills, generated commands, runtime-facing symlinks, and machine-local runtime selection |
| Layer 2 | `<target-repo>/.dev/` | per-repo working context and state |
| Layer 3 | generated adapter files in the target repo | shared instructions and runtime-specific shims |

Naming note: upstream docs still use the full product name `Antigravity CLI` and the path segment `antigravity-cli`, but Google also exposes `AGY CLI` as the short name. In GAL-owned helper and function names, prefer `Agy` or `agy` for internal identifiers; keep `Antigravity CLI` and `antigravity-cli` for user-facing labels, runtime keys, and upstream-owned paths.

### Cross-Runtime Surface

| Runtime | Machine-layer install | Command surface | Notes |
| --- | --- | --- | --- |
| Copilot | `~/.copilot/agents/` and `~/.copilot/skills/` | installed command skills | supports custom agents and slash-command discovery |
| Antigravity CLI | `~/.gemini/antigravity-cli/plugins/gal/` (plugin-root) | installed named skills via plugin | primary Google CLI runtime; installs as a provider plugin at `~/.gemini/antigravity-cli/plugins/gal/` carrying skills, agents, rules, and MCP config as a self-contained tree; AGY is renderer 1 on the common package model, not the architecture itself |
| Gemini CLI | `~/.gemini/commands/`, `~/.gemini/gal-context.md`, `~/.gemini/settings.json`, and `~/.gemini/gal/` | generated native command files plus compatibility bridges | archived compatibility runtime; keep only the remaining surfaces listed below until AGY fully replaces them |
| Codex CLI | `~/.codex/skills/` and shared `~/.agents/skills/` | installed named skills | uses `$skill` invocation, not custom slash commands |
| Claude Code | `~/.claude/skills/`, `~/.claude/commands/`, and user-scope `claude mcp` config | generated command markdown plus repo-local `CLAUDE.md` | MCP install is managed through the Claude CLI |

### Layer 1.5 Install Topology

| Source in repo | Copilot target | Gemini target | Antigravity target | Codex target | Claude target |
| --- | --- | --- | --- | --- | --- |
| `agent/*.agent.md` | `~/.copilot/agents/` | not installed | `~/.gemini/antigravity-cli/plugins/gal/agents/` | not installed | not installed |
| `skills/*/` | `~/.copilot/skills/` | imported from repo paths via `~/.gemini/gal-context.md` | `~/.gemini/antigravity-cli/plugins/gal/skills/` | `~/.agents/skills/` | `~/.claude/skills/` |
| `commands/*/` | `~/.copilot/skills/<command>/` | `~/.gemini/commands/<command>.toml` | `~/.gemini/antigravity-cli/plugins/gal/skills/<command>/` | `~/.codex/skills/<command>/` | `~/.claude/commands/<command>.md` |
| repo root | `~/.copilot/gal/` | `~/.gemini/gal/` | `~/.gemini/antigravity-cli/plugins/gal/` (plugin tree) | not required | not required |

### Generated Runtime Files

| Generated file | Why it exists |
| --- | --- |
| `commands/*/SKILL.md` | baked command prompt with absolute `GAL_ROOT` plus any gitignored `SKILL.local.md` overlay |
| `~/.gemini/commands/*.toml` | Gemini-native command surface generated from the baked command skill |
| `~/.claude/commands/*.md` | Claude-native command surface generated from the baked command skill |
| `~/.gemini/gal-context.md` | reusable shared skill imports for Gemini |

### Archived Gemini CLI Surfaces

These are the remaining Gemini CLI compatibility surfaces that still exist on purpose. Treat them as archived bridges to be retired gradually as AGY reaches parity. Do not expand them unless the change is explicitly about keeping Gemini compatibility working during that transition.

| Archived surface | Owning files | Why it still exists | Expected retirement path |
| --- | --- | --- | --- |
| Gemini runtime selection, path constants, and install-state detection | `scripts/common/Common.ps1`, `scripts/common/common.sh` | Setup still needs to detect and manage Gemini-specific compatibility outputs such as `~/.gemini/commands/`, `~/.gemini/settings.json`, `~/.gemini/gal-context.md`, and `~/.gemini/gal/`. | Remove once no GAL-managed Gemini install target remains. |
| Gemini native command generation | `scripts/Update-Commands.ps1`, `scripts/update-commands.sh` | GAL still bakes `commands/*/SKILL.md` into `~/.gemini/commands/*.toml` for the legacy Gemini native slash-command surface. | Replace when AGY skill or plugin surfaces are the only Google command entry point GAL supports. |
| Gemini shared-skill context bridge | `scripts/Update-Personalization.ps1`, `scripts/update-personalization.sh` | `~/.gemini/gal-context.md` still imports repo skills for Gemini compatibility. | Remove when Gemini no longer needs repo-skill imports for GAL. |
| Gemini settings.json bridge | `scripts/Update-Personalization.ps1`, `scripts/update-personalization.sh` | `~/.gemini/settings.json` still gets `AGENTS.md` and `GEMINI.md` in `context.fileName` for legacy Google-runtime loading. | Remove when Google-side loading is fully owned by AGY runtime surfaces instead of Gemini settings. |
| Gemini `GAL_ROOT` link and legacy skill cleanup | `scripts/Update-Skills.ps1`, `scripts/update-skills.sh` | GAL still manages `~/.gemini/gal/` and cleans old GAL-managed `~/.gemini/skills/*` remnants during migration. | Remove when no Gemini runtime path needs a stable repo link and no legacy cleanup is needed. |
| Legacy Gemini MCP cleanup | `scripts/Update-Mcp.ps1`, `scripts/update-mcp.sh` | Gemini is no longer the MCP owner, but GAL still removes old Gemini MCP entries from `~/.gemini/settings.json` so AGY MCP ownership stays clean. | Remove after legacy Gemini MCP residue no longer exists in supported installs. |
| `GEMINI.md` generated adapter filename | `scripts/Sync-DevContext.ps1`, `scripts/sync-dev-context.sh`, `scripts/Init-Repo.ps1`, `scripts/init-repo.sh` | The repo still emits `GEMINI.md` as a Google-runtime compatibility adapter filename even though AGY is the primary Google CLI runtime. | Rename or remove only when Google-runtime consumers no longer depend on the `GEMINI.md` carrier. |
| xmachine Gemini headless execution lane | `scripts/Start-xMachine.ps1`, `scripts/Start-xMachine.sh` | xmachine remote execution still invokes Gemini CLI headlessly and maps Gemini exit codes. | Replace when xmachine is migrated to AGY or another runtime end-to-end. |

If you are removing one of these archived surfaces, also audit the matching maintainer guidance in [../scripts/scripts.md](../scripts/scripts.md), [personalization.md](personalization.md), and [personalization.zh-Hant.md](personalization.zh-Hant.md) so the docs stop describing a retired bridge.

### Install-State

The installer persists machine-local runtime selection in `~/.gal/install-state.json`.

- `selectedRuntimes` controls which machine-layer targets GAL should manage.
- `primaryRuntime` controls defaults and summaries only.
- The tracked GAL repo remains the primary source for agents, skills, and commands.

### MCP Management

The MCP manifest is a separate install concern from skills.

| File | Scope | Role |
| --- | --- | --- |
| `mcp.json` | tracked | single GAL MCP source of truth |
| `mcp.local.json` | local only | machine-specific overrides and enablement |
| `config.local.env` | local only | secrets and local values referenced by the manifest |
| `xmachine.config.json` | local only | machine-local xmachine node definitions keyed by work-node alias |

The merged MCP manifest is centered on `servers` and may also include optional top-level `inputs` when a runtime supports prompt-backed values such as a PAT entry.

For Playwright MCP specifically:

- Keep `mcp.json` limited to the tracked safe startup contract: canonical `playwright` key plus conservative core flags such as `--isolated` and `--headless`.
- Put headed mode, viewport or device emulation, storage-state paths, output directories, optional capability flags, persistent profile paths, extension or CDP wiring, and similar machine-local behavior in `mcp.local.json`.
- Put secret-like paths or environment-backed local values referenced by those overrides in `config.local.env`.
- Do not track browser artifacts, storage-state files, persistent profile directories, or secret files in the repo.

`Update-Mcp.ps1` and `update-mcp.sh` use the tracked manifest as the source of truth for GAL-managed server names:

- VS Code: overwrite tracked server entries inside user `mcp.json`
- Antigravity CLI: write GAL-managed MCP to plugin-root `~/.gemini/antigravity-cli/plugins/gal/mcp_config.json` under `mcpServers`; the global `~/.gemini/antigravity-cli/mcp_config.json` is only touched for legacy cleanup of old GAL-managed entries
- JSON-based runtime bridges also preserve managed top-level `inputs` entries by input `id` when the merged manifest includes them.
- Codex CLI: regenerate tracked `[mcp_servers.*]` sections inside `config.toml`
- Claude Code: remove and re-add tracked user-scope servers through the `claude mcp` CLI

Provider-owned config still stays user-owned. GAL only takes ownership of the server names declared in the tracked manifest, preserves unrelated user-defined entries, and removes the GAL-managed legacy Gemini MCP names previously written into `settings.json`.

### Why `GAL_ROOT` Exists

`~/.copilot/gal/` and `~/.gemini/gal/` give installed command skills one stable path back to the source repo. That keeps generated command prompts small and deterministic.

For AGY, the plugin tree at `~/.gemini/antigravity-cli/plugins/gal/` replaces the old `~/.gemini/antigravity-cli/gal/` symlink as the managed install surface. The plugin is self-contained and does not require an external `GAL_ROOT` symlink; setup removes the legacy `GAL_ROOT` symlink during pre-cleanup.

## Provider Plugin Packaging

GAL uses a provider-neutral plugin package model. Source contracts in the repo are the single source of truth; each provider plugin is a generated artifact rendered by a provider-specific renderer.

### Common Package Model

The common package (`scripts/common/ProviderPlugin.ps1`, `scripts/common/provider-plugin.sh`) carries:

| Field | Source | Shared across all four providers? |
| --- | --- | --- |
| `metadata` | repo name, display name, version diagnostics, generation timestamp | conceptually yes |
| `skills` | `skills/<name>/SKILL.md` | yes |
| `commandSkills` | `commands/*/SKILL.md` | yes, as skill bundles |
| `mcpSpec` | `mcp.json` plus `mcp.local.json` boundary info | conceptually yes, but resolved local values stay out |
| `instructionCorpus` | `.dev/project.md`, required conventions, workflows, `model-roles.md`, generated indexes | content yes, path no |
| `agents` | `agent/*.agent.md` | optional; projected to three of four providers |

The common model explicitly excludes: provider-specific output paths, resolved machine-local secrets or paths, `runtimeScripts`, plugin-root `scripts/`, `gal-results/`, and hooks (deferred from v1).

### AGY Renderer

AGY is renderer 1, not the architecture. `Build-AgyPlugin` renders the common package into `dist/provider-plugins/agy/gal/` and installs to `~/.gemini/antigravity-cli/plugins/gal/`. The AGY plugin carries `plugin.json`, `skills/`, `agents/`, `rules/gal.md`, and `mcp_config.json`. It does not generate `hooks.json`, `scripts/`, marketplace metadata, provider stubs, or `gal-results/`.

Setup/reinstall removes all prior GAL-managed AGY content (legacy skills directory, `GAL_ROOT` symlink, global MCP entries, prior plugin installs) before installing the clean plugin tree.

### Gemini Migration Lane

Gemini CLI is not a fifth renderer. It is an AGY migration/compatibility lane. Existing Gemini-specific cleanup and bridge logic stays in the AGY renderer concern; it does not enter the provider-neutral substrate.

### Future Renderer Sequence

After AGY validation, the planned renderer sequence is: Copilot CLI → Codex → Claude Code. Each will reuse the common base with its own layout and install lifecycle. No future renderer should copy the AGY layout.

## Common Change Entry Points

### Changing `/gal` or alias behavior

1. Read [../commands/commands.md](../commands/commands.md).
2. Check whether the change is contract-level behavior or only install/runtime presentation.
3. If it affects generated command files, inspect the setup scripts and the relevant `commands/*/SKILL.template.md`.

### Adding or changing a planning command

1. Place it in the right family via [../commands/commands.md](../commands/commands.md).
2. Update the owning prompt in `commands/<command>/SKILL.template.md`.
3. Confirm the write-back target fits the existing plan sections and workflow state machine.
4. If it changes optional collaborative-tool semantics, also update [collaborative-tools/gstack.md](collaborative-tools/gstack.md) and [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md) when shared preflight behavior changes.

### Adding or changing an execution specialist

1. Update the owning prompt in `agent/<golem>.agent.md`.
2. Confirm the write-back target fits the existing plan sections and workflow lifecycle.
3. Update [../agent/agents.md](../agent/agents.md), [../commands/commands.md](../commands/commands.md), and any README sections that route users to that specialist.
4. Do not reintroduce the behavior as a standalone public command unless it is truly control-plane or planning work.

### Changing setup, installation, or MCP merge

1. Read [../scripts/scripts.md](../scripts/scripts.md).
2. Decide which concern owns the change first: `Update-Personalization`, `Update-Skills`, `Update-Commands`, `Update-Mcp`, or the top-level orchestrator.
3. Windows and macOS/Linux both use the split Setup-Machine plus concern-script stack. Keep the two entrypoint families aligned unless the change is intentionally platform-specific.
4. Check whether `commands/commands.md` should also change because the user-visible runtime surface changed.
5. Keep README focused on entry points, keep setup plumbing here and in the source scripts.

### Refreshing MCP vs. Regenerating Adapters

- Run `Update-Mcp.ps1` or `update-mcp.sh` after changing `mcp.json`, `mcp.local.json`, or MCP-related values in `config.local.env`. This refreshes runtime MCP config only.
- Run `Sync-DevContext.ps1` or `sync-dev-context.sh` after changing source-of-truth content that should regenerate repo-local adapters such as `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, or `GEMINI.md`.
- Run `Setup-Machine.ps1` or `setup-machine.sh` when you need the full concern stack refreshed in one pass.

### Refactoring docs themselves

1. Make sure each doc has one clear job.
2. If another source file already owns the contract, summarize it and link out instead of copying it.
3. If you remove content from one reader entry point, give it a clear new landing page.

## Adding A New CLI Runtime

1. Decide whether the CLI has a machine-layer config directory that GAL can target.
2. Decide whether its repo-facing instruction file can reuse `AGENTS.md` or needs another generated adapter.
3. If the runtime supports native commands, generate them from the same shared command templates instead of building a second workflow source. If it does not, install the same baked command skills into the runtime's supported skill surface.
4. Add any config-merge bridge only if the runtime has a stable, user-owned config file that can safely accept additive changes.

## Verify Setup Changes

After changing install or setup logic, verify at least these points:

- the stable repo symlink exists for each supported runtime that needs one
- generated `commands/*/SKILL.md` files no longer contain `{{GAL_ROOT}}`
- Gemini native command files were regenerated from the baked command content
- Antigravity installed skills and agents resolve through `~/.gemini/antigravity-cli/plugins/gal/`
- AGY plugin-root `mcp_config.json` is the sole GAL-managed MCP source for AGY; no GAL-managed MCP entries remain in the global `mcp_config.json` or `settings.json`
- shared skill directories contain reusable skills only, not duplicated command aliases
- MCP reruns update tracked server entries correctly without clobbering unrelated provider-owned config

## Where Information Belongs

| Information type | Right home |
| --- | --- |
| durable methodology and contracts | tracked source docs and source files |
| repo working context | `.dev/project.md` and `.dev/state.md` in the target repo |
| human-readable feature plan | `docs/plans/<plan-slug>.md` |
| machine-readable execution work file | `.dev/plans/<plan-slug>.prompt.md` |
| temporary session continuity | `### Handoff Notes` plus `.dev/state.md` |
| machine-local xmachine node config | `xmachine.config.json` |

If a completed plan contains knowledge that should survive, extract it back into a durable source file instead of leaving the plan as hidden long-term documentation.

## Self-Check

Before you finish a maintainer change, ask:

- Did I create a second source of truth?
- Does `/gal` still only solve control-plane problems?
- Do specialist commands still write back to repo-owned files?
- Did I accidentally move state back into a user-global path?
- Can a missing tool still fail loudly instead of pretending to succeed?
- Did I keep collaborative-tool routing at the workflow layer?
- Do README, `commands/commands.md`, and this guide still have distinct jobs?

## Suggested Reading Order

| Reader | Suggested order |
| --- | --- |
| first-time GAL maintainer | this guide → [../commands/commands.md](../commands/commands.md) → [../scripts/scripts.md](../scripts/scripts.md) |
| maintainer changing command behavior | [../commands/commands.md](../commands/commands.md) → [../scripts/scripts.md](../scripts/scripts.md) |
| maintainer changing setup | this guide → [../scripts/scripts.md](../scripts/scripts.md) |
| maintainer changing workflow semantics | [../commands/commands.md](../commands/commands.md) → [collaborative-tools/gstack.md](collaborative-tools/gstack.md) → [../workflows/coding.md](../workflows/coding.md) |

## Related Files

- [../README.md](../README.md) for the primary user entry point.
- [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md) for shared collaborative-tool preflight behavior.
- [collaborative-tools/gstack.md](collaborative-tools/gstack.md) for optional collaborative-tool behavior.
- [../commands/commands.md](../commands/commands.md) for the control-plane contract and runtime surface.
- [../scripts/scripts.md](../scripts/scripts.md) for the script inventory and setup behavior.
- [../templates/templates.md](../templates/templates.md) for template ownership.

## Token Discipline

These rules apply to all maintainer and agent work in this repo. The full policy lives in [../conventions/token-budget.md](../conventions/token-budget.md). The developer-facing summary is here.

### Generated-Artifact Exclusion

Do not read generated adapters (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`, `.github/copilot-instructions.md`) or build outputs (`bin/`, `obj/`) unless the current task is explicitly about auditing those generated files. They are large, frequently regenerated, and contain no information not already in their source templates.

### Directed Exploration

Before reading any file, confirm it is named in the current task or is a direct dependency of a task-named file. Stop reading when you have the information needed. Do not load the full codebase as a cold-start step.

### Failure-Focused Output

When running builds or tests, emit:

- Build: first error with file and line reference. On success, one summary line only.
- Tests: failing test names and assertion messages only. Do not echo passing test names.
- Lint: files and rule violations only. On a clean pass, one summary line only.

Store full logs on disk when needed; retrieve specific lines selectively rather than piping entire logs into context.

### Context-Pressure Recovery

When context is near the limit during an active task:

1. Write the current task name, last completed step, and any key decisions to `### Handoff Notes` in the active plan's `## Status` section.
2. Write `Stopped at:` and `Next step:` to `.dev/state.md` Session Continuity.
3. Do **not** create a separate `CONTEXT.md` file — the plan and state files are the only durable session state stores.

This ensures the next session can resume without re-deriving context.
