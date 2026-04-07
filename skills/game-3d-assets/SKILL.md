---
name: game-3d-assets
description: AI-first workflow for 3D game assets. Use when generating concept sheets, turnarounds, texture references, or material direction in ComfyUI, then using Blender for general 3D work, preview renders, and export.
license: Complete terms in LICENSE.txt
---

# Game 3D Assets

Use this skill for 3D game asset production.

## Preferred Tool Order

1. Use ComfyUI for concept sheets, turnarounds, texture references, and material direction.
2. Use Blender by default for game-asset modeling, scene setup, baking, preview renders, and export.
3. Keep the 3D lane focused on Blender unless a new free tool is explicitly added later.

## Common Tasks

| Task | Preferred Path |
| --- | --- |
| General 3D prop | ComfyUI -> Blender |
| Stylized environment prop | ComfyUI -> Blender |
| Mechanical or product-like prop | ComfyUI -> Blender |
| Clean-silhouette hard-surface base | ComfyUI -> Blender |
| Bake and preview render | Blender |

## Decision Tree

```text
Need a 3D game asset?
    -> ComfyUI for concept and references

Need modeling, baking, rendering, or export?
    -> Blender
```

## Output Requirements

When using this skill:

1. Produce a concept or reference artifact before modeling.
2. Use Blender as the documented 3D production tool.
3. Keep the lane reproducible with source files and preview renders.
4. End with preview renders plus an export-ready source package.