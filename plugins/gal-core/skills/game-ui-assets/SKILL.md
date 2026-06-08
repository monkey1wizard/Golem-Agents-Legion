---
name: game-ui-assets
description: AI-first workflow for UI, icon, and HUD assets in games. Use when generating visual direction in ComfyUI, building component and layout states in Figma, and performing SVG cleanup or export work in Inkscape.
license: Complete terms in LICENSE.txt
---

# Game UI Assets

Use this skill for HUD, icons, menus, and interface asset work.

## Preferred Tool Order

1. Use ComfyUI for direction finding and style exploration.
2. Use Figma for layout, states, and component structure.
3. Use Inkscape for deterministic SVG cleanup, path operations, or export.

## Common Tasks

| Task | Preferred Path |
| --- | --- |
| HUD direction board | ComfyUI -> Figma |
| Icon family | ComfyUI -> Figma -> Inkscape |
| State variants for UI controls | Figma |
| SVG cleanup or export conversion | Inkscape |

## Decision Tree

```text
Need UI or HUD asset work?
    -> ComfyUI for visual direction
    -> Figma for layout and components

Need deterministic SVG cleanup or export?
    -> Inkscape
```

## Output Requirements

When using this skill:

1. Export state variants with stable names.
2. Provide SVG and PNG sizes when the game pipeline needs both.
3. Keep Figma responsible for layout semantics.
4. Keep Inkscape responsible for deterministic vector cleanup and export.