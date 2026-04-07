---
name: game-2d-assets
description: AI-first workflow for 2D game art and illustration assets. Use when generating character art, prop art, card art, painted icons, portraits, or other raster 2D assets that start in ComfyUI and finish in GIMP.
license: Complete terms in LICENSE.txt
---

# Game 2D Assets

Use this skill for non-pixel raster game art.

## Preferred Tool Order

1. Use ComfyUI for ideation, style consistency, and candidate generation.
2. Use GIMP for cleanup, compositing, alpha fixes, and final export.

## Common Tasks

| Task | Preferred Path |
| --- | --- |
| Character portrait variants | ComfyUI -> GIMP |
| Item illustration with transparent background | ComfyUI -> GIMP |
| Card or key art direction | ComfyUI -> GIMP |
| Batch variation for a single style family | ComfyUI defaults plus GIMP cleanup |

## Decision Tree

```text
Need non-pixel 2D art?
    -> ComfyUI for generation

Need cleanup, compositing, alpha fixes, or export sizing?
    -> GIMP
```

## Output Requirements

When using this skill:

1. Produce named candidate variants before cleanup.
2. Export final PNG or WebP in game-ready dimensions.
3. Preserve prompt, seed, or style references where practical.
4. Do not route this lane through Krita in the first release.