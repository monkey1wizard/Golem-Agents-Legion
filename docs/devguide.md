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

- If a change makes Copilot, Gemini, and Codex diverge in contract or file flow, it is usually the wrong change.
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
| Layer 1.5 | tool config directories such as `~/.copilot/`, `~/.gemini/`, `~/.gemini/antigravity/`, `~/.codex/`, `~/.claude/`, plus `~/.gal/install-state.json` | installed skills, generated commands, runtime-facing symlinks, and machine-local runtime selection |
| Layer 2 | `<target-repo>/.dev/` | per-repo working context and state |
| Layer 3 | generated adapter files in the target repo | shared instructions and runtime-specific shims |

### Cross-Runtime Surface

| Runtime | Machine-layer install | Command surface | Notes |
| --- | --- | --- | --- |
| Copilot | `~/.copilot/agents/` and `~/.copilot/skills/` | installed command skills | supports custom agents and slash-command discovery |
| Gemini CLI | `~/.gemini/commands/`, `~/.gemini/gal/`, and shared `~/.agents/skills/` | generated native `.toml` commands | also merges `mcpServers` into `settings.json` |
| Antigravity | `~/.gemini/antigravity/skills/`, `~/.gemini/antigravity/gal/`, and `~/.gemini/antigravity/mcp_config.json` | installed named skills plus repo-local workspace rule | uses generated `.agents/rules/gal.md` to reference `AGENTS.md` via Antigravity's documented `@filename` rule syntax; phase 1 does not generate native slash workflows |
| Codex CLI | `~/.codex/skills/` and shared `~/.agents/skills/` | installed named skills | uses `$skill` invocation, not custom slash commands |
| Claude Code | `~/.claude/skills/`, `~/.claude/commands/`, and user-scope `claude mcp` config | generated command markdown plus repo-local `CLAUDE.md` | MCP install is managed through the Claude CLI |

### Layer 1.5 Install Topology

| Source in repo | Copilot target | Gemini target | Antigravity target | Codex target | Claude target |
| --- | --- | --- | --- | --- | --- |
| `agent/*.agent.md` | `~/.copilot/agents/` | not installed | not installed | not installed | not installed |
| `skills/*/` | `~/.copilot/skills/` | `~/.agents/skills/` | `~/.gemini/antigravity/skills/` | `~/.agents/skills/` | `~/.claude/skills/` |
| `commands/*/` | `~/.copilot/skills/<command>/` | `~/.gemini/commands/<command>.toml` | `~/.gemini/antigravity/skills/<command>/` | `~/.codex/skills/<command>/` | `~/.claude/commands/<command>.md` |
| repo root | `~/.copilot/gal/` | `~/.gemini/gal/` | `~/.gemini/antigravity/gal/` | not required | not required |

### Generated Runtime Files

| Generated file | Why it exists |
| --- | --- |
| `commands/*/SKILL.md` | baked command prompt with absolute `GAL_ROOT` plus any gitignored `SKILL.local.md` overlay |
| `~/.gemini/commands/*.toml` | Gemini-native command surface generated from the baked command skill |
| `.agents/rules/gal.md` | thin Antigravity workspace rule shim that references `AGENTS.md` |
| `~/.claude/commands/*.md` | Claude-native command surface generated from the baked command skill |
| `~/.gemini/gal-context.md` | reusable shared skill imports for Gemini |

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
- Gemini CLI: overwrite tracked server entries inside `settings.json` under `mcpServers`
- Antigravity: overwrite tracked server entries inside `~/.gemini/antigravity/mcp_config.json` under `mcpServers`
- JSON-based runtime bridges also preserve managed top-level `inputs` entries by input `id` when the merged manifest includes them.
- Codex CLI: regenerate tracked `[mcp_servers.*]` sections inside `config.toml`
- Claude Code: remove and re-add tracked user-scope servers through the `claude mcp` CLI

Provider-owned config still stays user-owned. GAL only takes ownership of the server names declared in the tracked manifest and preserves unrelated user-defined entries.

### Why `GAL_ROOT` Exists

`~/.copilot/gal/`, `~/.gemini/gal/`, and `~/.gemini/antigravity/gal/` give installed command skills one stable path back to the source repo. That keeps generated command prompts small and deterministic.

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
- Antigravity generated `.agents/rules/gal.md`, its `@` reference resolves to `AGENTS.md`, and installed command skills resolve through `~/.gemini/antigravity/gal/`
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

Do not read generated adapters (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`, `.github/copilot-instructions.md`, `ANTIGRAVITY.md`) or build outputs (`bin/`, `obj/`) unless the current task is explicitly about auditing those generated files. They are large, frequently regenerated, and contain no information not already in their source templates.

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
