# Developer Guide

This document is maintainer navigation, not a second specification. Use it to decide which layer you are changing, which source files own that layer, and which rules you must not break.

## Start By Finding The Right Layer

| If you are changing... | Ask first... | Read these source files |
| --- | --- | --- |
| `/gal` command surface, aliases, or dispatch | is this control-plane behavior or runtime plumbing? | [../commands/commands.md](../commands/commands.md), [../scripts/scripts.md](../scripts/scripts.md) |
| planning flow or optional collaborative-tool semantics | is this GAL-native planning, optional gstack behavior, or workflow teaching? | [command-index.md](command-index.md), [collaborative-tools/gstack.md](collaborative-tools/gstack.md), [../workflows/coding.md](../workflows/coding.md) |
| setup, install topology, baked command files, or MCP merge | is this machine-layer install or repo-layer adapter generation? | [../scripts/scripts.md](../scripts/scripts.md), `scripts/Setup-Machine.ps1`, `scripts/setup-machine.sh` |
| templates and plan lifecycle | which file should own this information? | [../templates/templates.md](../templates/templates.md), [../workflows/coding.md](../workflows/coding.md) |
| remote worker behavior | is this part of the main workflow or an execution-plane extension? | [collaborative-tools/remote-worker.md](collaborative-tools/remote-worker.md), remote worker scripts under `scripts/` |
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

This section absorbs the setup topology that maintainers need when changing `Setup-Machine`, command installation, or MCP wiring.

### Four Runtime Layers

| Layer | Location | Purpose |
| --- | --- | --- |
| Layer 1 | the GAL repo | main methodology source |
| Layer 1.5 | tool config directories such as `~/.copilot/`, `~/.gemini/`, `~/.codex/`, `~/.claude/`, plus `~/.gal/install-state.json` | installed skills, generated commands, runtime-facing symlinks, and machine-local runtime selection |
| Layer 2 | `<target-repo>/.dev/` | per-repo working context and state |
| Layer 3 | generated adapter files in the target repo | shared instructions and runtime-specific shims |

### Cross-Runtime Surface

| Runtime | Machine-layer install | Command surface | Notes |
| --- | --- | --- | --- |
| Copilot | `~/.copilot/agents/` and `~/.copilot/skills/` | installed command skills | supports custom agents and slash-command discovery |
| Gemini CLI | `~/.gemini/commands/`, `~/.gemini/gal/`, and shared `~/.agents/skills/` | generated native `.toml` commands | also merges `mcpServers` into `settings.json` |
| Codex CLI | `~/.codex/skills/` and shared `~/.agents/skills/` | installed named skills | uses `$skill` invocation, not custom slash commands |
| Claude Code | `~/.claude/skills/` and `~/.claude/commands/` | generated command markdown plus repo-local `CLAUDE.md` | Claude MCP merge remains deferred |

### Layer 1.5 Install Topology

| Source in repo | Copilot target | Gemini target | Codex target | Claude target |
| --- | --- | --- | --- | --- |
| `agent/*.agent.md` | `~/.copilot/agents/` | not installed | not installed | not installed |
| `skills/*/` | `~/.copilot/skills/` | `~/.agents/skills/` | `~/.agents/skills/` | `~/.claude/skills/` |
| `commands/*/` | `~/.copilot/skills/<command>/` | `~/.gemini/commands/<command>.toml` | `~/.codex/skills/<command>/` | `~/.claude/commands/<command>.md` |
| repo root | `~/.copilot/gal/` | `~/.gemini/gal/` | not required | not required |

### Generated Runtime Files

| Generated file | Why it exists |
| --- | --- |
| `commands/*/SKILL.md` | baked command prompt with absolute `GAL_ROOT` |
| `~/.gemini/commands/*.toml` | Gemini-native command surface generated from the baked command skill |
| `~/.claude/commands/*.md` | Claude-native command surface generated from the baked command skill |
| `~/.gemini/gal-context.md` | reusable shared skill imports for Gemini |

### Install-State

The installer persists machine-local runtime selection in `~/.gal/install-state.json`.

- `selectedRuntimes` controls which machine-layer targets GAL should manage.
- `primaryRuntime` controls defaults and summaries only.
- The tracked GAL repo remains the canonical source for agents, skills, and commands.

### MCP Management

The MCP manifest is a separate install concern from skills.

| File | Scope | Role |
| --- | --- | --- |
| `mcp-servers.example.json` | tracked | baseline server catalog |
| `mcp-servers.local.json` | local only | machine-specific overrides and enablement |
| `config.local.env` | local only | secrets and local values referenced by the manifest |

Setup uses merge, not overwrite:

- VS Code: merge missing servers into user `mcp.json`
- Gemini CLI: merge missing servers into `settings.json` under `mcpServers`
- Codex CLI: append missing `[mcp_servers.*]` sections into `config.toml`
- Claude Code: deferred; no installer-managed MCP merge yet

Provider-owned config stays user-owned. GAL fills gaps from the tracked manifest, it does not take full ownership of those files.

### Why `GAL_ROOT` Exists

`~/.copilot/gal/` and `~/.gemini/gal/` give installed command skills one stable path back to the source repo. That keeps generated command prompts small and deterministic.

## Common Change Entry Points

### Changing `/gal` or alias behavior

1. Read [../commands/commands.md](../commands/commands.md).
2. Check whether the change is contract-level behavior or only install/runtime presentation.
3. If it affects generated command files, inspect the setup scripts and the relevant `commands/*/SKILL.template.md`.

### Adding or changing a planning command

1. Place it in the right family via [command-index.md](command-index.md).
2. Update the owning prompt in `commands/<command>/SKILL.template.md`.
3. Confirm the write-back target fits the existing plan sections and workflow state machine.
4. If it changes optional collaborative-tool semantics, also update [collaborative-tools/gstack.md](collaborative-tools/gstack.md) and [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md) when shared preflight behavior changes.

### Adding or changing an execution specialist

1. Update the owning prompt in `agent/<golem>.agent.md`.
2. Confirm the write-back target fits the existing plan sections and workflow lifecycle.
3. Update [../agent/agents.md](../agent/agents.md), [command-index.md](command-index.md), and any README sections that route users to that specialist.
4. Do not reintroduce the behavior as a standalone public command unless it is truly control-plane or planning work.

### Changing setup, installation, or MCP merge

1. Read [../scripts/scripts.md](../scripts/scripts.md).
2. Update both setup scripts unless the change is intentionally platform-specific.
3. Check whether `commands/commands.md` should also change because the user-visible runtime surface changed.
4. Keep README focused on entry points, keep setup plumbing here and in the source scripts.

### Refactoring docs themselves

1. Make sure each doc has one clear job.
2. If another source file already owns the contract, summarize it and link out instead of copying it.
3. If you remove content from one reader entry point, give it a clear new landing page.

## Adding A New CLI Runtime

1. Decide whether the CLI has a machine-layer config directory that GAL can target.
2. Decide whether its repo-facing instruction file can reuse `AGENTS.md` or needs another generated adapter.
3. If the runtime supports native commands, generate them from the same shared command templates instead of building a second workflow source.
4. Add any config-merge bridge only if the runtime has a stable, user-owned config file that can safely accept additive changes.

## Verify Setup Changes

After changing install or setup logic, verify at least these points:

- the stable repo symlink exists for each supported runtime that needs one
- generated `commands/*/SKILL.md` files no longer contain `{{GAL_ROOT}}`
- Gemini native command files were regenerated from the baked command content
- shared skill directories contain reusable skills only, not duplicated command aliases
- MCP merge only added missing servers and did not clobber existing provider-owned config

## Where Information Belongs

| Information type | Right home |
| --- | --- |
| durable methodology and contracts | tracked source docs and source files |
| repo working context | `.dev/project.md` and `.dev/state.md` in the target repo |
| human-readable feature plan | `docs/plans/<plan-slug>.md` |
| machine-readable execution work file | `.dev/plans/<plan-slug>.prompt.md` |
| temporary session continuity | `### Handoff Notes` plus `.dev/state.md` |

If a completed plan contains knowledge that should survive, extract it back into a durable source file instead of leaving the plan as hidden long-term documentation.

## Self-Check

Before you finish a maintainer change, ask:

- Did I create a second source of truth?
- Does `/gal` still only solve control-plane problems?
- Do specialist commands still write back to repo-owned files?
- Did I accidentally move state back into a user-global path?
- Can a missing tool still fail loudly instead of pretending to succeed?
- Did I keep collaborative-tool routing at the workflow layer?
- Do README, `command-index.md`, and this guide still have distinct jobs?

## Suggested Reading Order

| Reader | Suggested order |
| --- | --- |
| first-time GAL maintainer | this guide → [../commands/commands.md](../commands/commands.md) → [../scripts/scripts.md](../scripts/scripts.md) |
| maintainer changing command behavior | [command-index.md](command-index.md) → [../commands/commands.md](../commands/commands.md) |
| maintainer changing setup | this guide → [../scripts/scripts.md](../scripts/scripts.md) |
| maintainer changing workflow semantics | [command-index.md](command-index.md) → [collaborative-tools/gstack.md](collaborative-tools/gstack.md) → [../workflows/coding.md](../workflows/coding.md) |

## Related Files

- [../README.md](../README.md) for the primary user entry point.
- [command-index.md](command-index.md) for the human-facing command map.
- [collaborative-tools/checking-contract.md](collaborative-tools/checking-contract.md) for shared collaborative-tool preflight behavior.
- [collaborative-tools/gstack.md](collaborative-tools/gstack.md) for optional collaborative-tool behavior.
- [../commands/commands.md](../commands/commands.md) for the control-plane contract and runtime surface.
- [../scripts/scripts.md](../scripts/scripts.md) for the script inventory and setup behavior.
- [../templates/templates.md](../templates/templates.md) for template ownership.
