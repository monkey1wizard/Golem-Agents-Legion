---
name: game-asset-export
description: Finalize and package AI-first game assets for downstream use. Use when exporting final PNG, WebP, SVG, spritesheets, preview renders, transparent backgrounds, or structured output packages after a lane-specific asset workflow is complete.
license: Complete terms in LICENSE.txt
---

# Game Asset Export

Use this skill to package lane outputs into usable deliverables.

## Preferred Tool Order

1. Finish the lane-specific source asset first.
2. Export in the tool that owns the final format semantics.
3. Normalize names, dimensions, and variant structure before handoff.

## Common Tasks

| Task | Preferred Path |
| --- | --- |
| Final raster export | GIMP |
| Spritesheet export | Aseprite |
| SVG or icon export | Inkscape |
| 3D preview render and mesh export | Blender |

## Decision Tree

```text
Need final raster image outputs?
    -> GIMP

Need spritesheet or animation sheet?
    -> Aseprite

Need vector outputs?
    -> Inkscape

Need 3D preview renders or mesh exports?
    -> Blender
```

## Output Requirements

When using this skill:

1. Export the final asset in the owning lane tool, not from an earlier draft stage.
2. Use deterministic names for variants and states.
3. Capture target dimensions, sheet layout, or export format in the handoff notes.
4. Ensure the exported package is directly usable by a downstream game project or content pipeline.