---
name: graphics-workflow
description: Route AI-first game asset work into the correct production lane. Use when generating or refining 2D art, sprites, UI assets, icons, HUD assets, or 3D game assets, and when choosing between ComfyUI, Blender, GIMP, Figma, Inkscape, or Aseprite workflows.
license: Complete terms in LICENSE.txt
---

# Graphics Workflow

Use this skill to select the correct asset lane.

## Preferred Tool Order

1. Start with ComfyUI for generation, variation, and direction finding.
2. Route to the lane-specific finishing tool.
3. Package and export only after the lane output contract is satisfied.

## Common Tasks

| Task | Preferred Path |
| --- | --- |
| Character or prop concept art | ComfyUI -> GIMP |
| Sprite or pixel enemy | ComfyUI -> Aseprite |
| UI icon set or HUD state work | ComfyUI -> Figma -> Inkscape |
| General 3D prop or scene asset | ComfyUI -> Blender |

## Decision Tree

```text
Need a game asset?
    -> Start in ComfyUI

Need painted or raster 2D output?
    -> GIMP lane

Need sprite, tile, or pixel animation output?
    -> Aseprite lane

Need UI, icon, or HUD output?
    -> Figma and Inkscape lane

Need 3D output?
    -> Blender lane
```

## Output Requirements

When using this skill:

1. Choose the lane before choosing the app.
2. Keep ComfyUI as the default starting point.
3. Keep Blender as the default 3D lane tool.
4. Ensure the chosen lane ends with a usable exported asset package.