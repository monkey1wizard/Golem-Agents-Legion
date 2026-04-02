# Installation Topology

This document explains how GAL is installed into tool environments and how the same methodology stays synchronized across machines.

## Runtime Layers

GAL uses four practical runtime layers.

| Layer | Location | Purpose |
| --- | --- | --- |
| Layer 1 | `~/golem-agents-legion/` | Canonical methodology source |
| Layer 1.5 | `~/.copilot/` and `~/.gemini/` | Tool-facing symlinks and baked command skills |
| Layer 2 | `<target-repo>/.dev/` | Per-repo working context and state |
| Layer 3 | Generated adapter files in target repos | `AGENTS.md` (shared cross-CLI) + tool-specific shims |

README keeps the high-level three-layer model. This document expands the runtime installation details that sit between the repo and the tools.

## Cross-CLI Support Matrix

This matrix is the canonical reference for how each CLI runtime integrates with GAL. Use it when adding a new CLI or auditing an existing one.

### Machine-Layer Install

| CLI | Machine Install | Notes |
| --- | --- | --- |
| Copilot CLI | `~/.copilot/agents/` + `~/.copilot/skills/` | Symlinks + baked `SKILL.md` via Setup-Machine |
| Gemini CLI | `~/.agents/skills/` + `settings.json` bridge | Discovers GAL skills via `.agents`; bridge writes `AGENTS.md` to `context.fileName` |
| Codex CLI | `~/.agents/skills/` | Needed for `$gal` and other Codex skills; `AGENTS.md` guidance stays native |
| Claude Code CLI | `~/.claude/` *(future)* | Deferred until confirmed usage |

### Repo-Layer Adapter Outputs

| Output File | CLI(s) | Content Strategy |
| --- | --- | --- |
| `AGENTS.md` | Copilot + Codex + Gemini (via bridge) + Claude Code | Canonical shared contract. Inline skills, conventions, workflows. |
| `.github/copilot-instructions.md` | Copilot only | Additive Copilot layer. No inline skills (machine install provides them). |
| `GEMINI.md` | Gemini *(transitional shim)* | Kept during migration to bridge. Inline skills. |

### Slash-Command / Skill Availability

| CLI | Status | Location |
| --- | --- | --- |
| Copilot CLI | Available | `~/.copilot/skills/<command>/` |
| Gemini CLI | No native slash-command runtime | — |
| Codex CLI | Available via `/skills` or `$skill` mention, not custom `/slash-command` syntax | `.agents/skills/` |
| Claude Code CLI | Deferred | MCP tools |

### Adding a New CLI

To add a new CLI runtime to GAL:

1. **Machine layer**: determine if the CLI has a global config dir; if yes, add a symlink or config step to both Setup-Machine scripts.
2. **Repo layer**: determine the CLI's canonical instruction file. If it is `AGENTS.md`, no code change needed — the repo adapter generator already writes it. Otherwise add a `Build-AdapterContent` / `build_adapter` call to both Sync-DevContext scripts.
3. **Settings bridge pattern**: if the CLI reads a configurable filename list (like Gemini's `context.fileName`), add a settings-write/merge step to Setup-Machine.
4. **Slash-command layer**: research the CLI's plugin or skill packaging model separately — do not block the other layers on it.

## Layer 1.5: Tool Installation Surface

Setup-Machine creates this effective topology.

| Source | Copilot Target | Gemini Target | Codex Target | Notes |
| --- | --- | --- | --- | --- |
| `agent/*.agent.md` | `~/.copilot/agents/` | — | — | Copilot-only custom agents |
| `skills/*/` | `~/.copilot/skills/` | — | `~/.agents/skills/` | Shared portable skill set (Gemini + Codex both discover via `.agents`) |
| `commands/gal/` | `~/.copilot/skills/gal/` | — | `~/.agents/skills/gal/` | `/gal` (Copilot/Gemini) · `$gal` (Codex) |
| `commands/gal-init/` | `~/.copilot/skills/gal-init/` | — | `~/.agents/skills/gal-init/` | `/gal-init` · `$gal-init` |
| `commands/gal-status/` | `~/.copilot/skills/gal-status/` | — | `~/.agents/skills/gal-status/` | `/gal-status` · `$gal-status` |
| `commands/gal-whats-next/` | `~/.copilot/skills/gal-whats-next/` | — | `~/.agents/skills/gal-whats-next/` | `/gal-whats-next` · `$gal-whats-next` |
| `commands/gal-wrap-up/` | `~/.copilot/skills/gal-wrap-up/` | — | `~/.agents/skills/gal-wrap-up/` | `/gal-wrap-up` · `$gal-wrap-up` |
| `commands/<specialist>/` | `~/.copilot/skills/<specialist>/` | — | `~/.agents/skills/<specialist>/` | All specialist skills |
| `<repo root>` | `~/.copilot/gal/` | `~/.gemini/gal/` | — | Stable GAL_ROOT symlink (not needed for Codex) |

## Generated Files

Setup-Machine also generates a small set of runtime files:

| Generated File | Purpose |
| --- | --- |
| `commands/*/SKILL.md` | Baked command skill with absolute GAL_ROOT (generated from each `SKILL.template.md`) |
| `~/.gemini/gal-context.md` | Aggregated `@file` imports so Gemini can load GAL consistently |

These files are generated because the tools need runtime-specific absolute paths, while the templates in the repo remain portable.

## Why GAL_ROOT Exists

`~/.copilot/gal/` and `~/.gemini/gal/` give installed command skills one stable way to reference the canonical repo.

That keeps runtime command prompts small and deterministic:

- skills do not need to infer repo location
- setup can bake paths once
- commands keep reading the same workflows, conventions, templates, and agents regardless of tool

## Cross-Machine Model

Each machine clones the same GAL repo and runs the same setup script for its platform.

```text
machine A                      machine B
-----------                    -----------
~/golem-agents-legion/         ~/golem-agents-legion/
~/.copilot/...                 ~/.copilot/...
~/.gemini/...                  ~/.gemini/...
        \                        /
         \---- git push/pull ----/
```

The methodology stays synchronized through Git. Machine-specific differences live in local tool configuration and model routing, not in duplicated copies of the methodology.

## Verification Checklist

After running Setup-Machine, verify:

1. `~/.copilot/gal/` and `~/.gemini/gal/` point to the GAL repo root.
2. `~/.copilot/skills/gal/` and `~/.agents/skills/gal/` are symlinked command skill directories.
3. All `commands/*/` skill directories exist in both tool skill trees.
4. Generated `SKILL.md` files no longer contain `{{GAL_ROOT}}`.
5. `~/.gemini/gal-context.md` exists and all import paths reference `.agents/skills`.

## Related Operational Sources

- [scripts/scripts.md](../scripts/scripts.md)
- [commands/commands.md](../commands/commands.md)
- [README.md](../README.md)
