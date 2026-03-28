# Installation Topology

This document explains how GAL is installed into tool environments and how the same methodology stays synchronized across machines.

## Runtime Layers

GAL uses four practical runtime layers.

| Layer | Location | Purpose |
| --- | --- | --- |
| Layer 1 | `~/golem-agents-legion/` | Canonical methodology source |
| Layer 1.5 | `~/.copilot/` and `~/.gemini/` | Tool-facing symlinks and baked command skills |
| Layer 2 | `<target-repo>/.dev/` | Per-repo working context and state |
| Layer 3 | Generated adapter files in target repos | Tool-specific repo instructions |

README keeps the high-level three-layer model. This document expands the runtime installation details that sit between the repo and the tools.

## Layer 1.5: Tool Installation Surface

Setup-Machine creates this effective topology.

| Source | Copilot Target | Gemini Target | Notes |
| --- | --- | --- | --- |
| `agent/*.agent.md` | `~/.copilot/agents/` | — | Copilot-only custom agents |
| `skills/*/` | `~/.copilot/skills/` | `~/.gemini/skills/` | Shared portable skill set |
| `commands/gal/` | `~/.copilot/skills/gal/` | `~/.gemini/skills/gal/` | Canonical slash-command entry |
| `commands/gal-init/` | `~/.copilot/skills/gal-init/` | `~/.gemini/skills/gal-init/` | Discoverability alias |
| `commands/gal-plan/` | `~/.copilot/skills/gal-plan/` | `~/.gemini/skills/gal-plan/` | Discoverability alias |
| `commands/gal-status/` | `~/.copilot/skills/gal-status/` | `~/.gemini/skills/gal-status/` | Discoverability alias |
| `commands/gal-next/` | `~/.copilot/skills/gal-next/` | `~/.gemini/skills/gal-next/` | Discoverability alias |
| `commands/gal-pause/` | `~/.copilot/skills/gal-pause/` | `~/.gemini/skills/gal-pause/` | Discoverability alias |
| `<repo root>` | `~/.copilot/gal/` | `~/.gemini/gal/` | Stable GAL_ROOT symlink |

## Generated Files

Setup-Machine also generates a small set of runtime files:

| Generated File | Purpose |
| --- | --- |
| `commands/gal/SKILL.md` | Baked canonical command skill with absolute GAL_ROOT |
| `commands/gal-*/SKILL.md` | Baked alias command skills |
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
2. `~/.copilot/skills/gal/` and `~/.gemini/skills/gal/` are symlinked command skill directories.
3. The five `gal-*` alias directories exist in both tool skill trees.
4. Generated `SKILL.md` files no longer contain `{{GAL_ROOT}}`.
5. `~/.gemini/gal-context.md` exists and imports GAL command skills first.

## Related Operational Sources

- [scripts/scripts.md](../scripts/scripts.md)
- [commands/commands.md](../commands/commands.md)
- [README.md](../README.md)
