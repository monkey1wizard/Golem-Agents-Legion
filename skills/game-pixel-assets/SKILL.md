---
name: game-pixel-assets
description: AI-first workflow for pixel-art game assets. Use when generating pixel sprites, enemies, items, tiles, simple animations, or spritesheets by starting with ComfyUI direction work and finishing in Aseprite through pixel-mcp.
license: Complete terms in LICENSE.txt
---

# Game Pixel Assets

Use this skill for pixel-art production.

## Preferred Tool Order

1. Use ComfyUI for silhouette exploration, direction, or variation.
2. Use Aseprite through `pixel-mcp` for final production work.

## Common Tasks

| Task | Preferred Path |
| --- | --- |
| Enemy sprite ideation | ComfyUI -> Aseprite |
| Item icon sprite | ComfyUI -> Aseprite |
| Animation cleanup | Aseprite |
| Spritesheet export | Aseprite |

## Decision Tree

```text
Need pixel-art output?
    -> ComfyUI for direction if needed
    -> Aseprite for production cleanup and export
```

## Output Requirements

When using this skill:

1. Keep a source `.aseprite` or `.ase` file when the asset continues evolving.
2. Define palette and frame size before final export.
3. Export spritesheets with explicit layout and dimension choices.
4. Treat ComfyUI outputs as reference material unless they already match the lane's pixel constraints.