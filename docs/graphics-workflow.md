# AI-First Game Asset Workflow

This workflow adds a game-asset production lane to GAL.

The goal is not broad graphics-tool coverage. The goal is to let GAL generate usable 2D and 3D game assets with AI first, then route them through the smallest useful set of finishing tools.

## Core Principle

- Start with ComfyUI for ideation, style consistency, variation, and reference generation.
- Use downstream tools to structure, clean up, package, and export assets.
- Choose tools by asset lane, not by personal preference or tool completeness.

## Supported Asset Lanes

| Lane | Default Flow | Purpose |
| --- | --- | --- |
| 2D concept and illustration | ComfyUI -> GIMP | Character art, props, cards, splash art, marketing-style concept frames |
| Sprite and pixel assets | ComfyUI -> Aseprite | Sprite ideation, simplification, palette cleanup, spritesheet export |
| UI, icon, and HUD assets | ComfyUI -> Figma -> Inkscape | UI direction, icon systems, HUD states, layout-ready export |
| 3D game assets | ComfyUI -> Blender | Turnarounds, texture references, prop modeling, preview renders, exports |

## Tool Roles

| Tool | Role in Workflow |
| --- | --- |
| ComfyUI | Generation and orchestration layer |
| GIMP | Raster cleanup, compositing, alpha fixes, batch export |
| Aseprite via `pixel-mcp` | Pixel cleanup, palette work, animation, spritesheets |
| Figma | UI layout, component states, HUD composition |
| Inkscape | SVG cleanup, path operations, icon export |
| Blender | Default 3D asset tool for general game asset work |

## Decision Tree

```text
Need a game asset?
    -> Start in ComfyUI

Need 2D illustration, concept art, or painted prop direction?
    -> ComfyUI
    -> GIMP for cleanup and export

Need a sprite, tileset, or pixel-art enemy/item?
    -> ComfyUI for ideation or variation
    -> Aseprite for production cleanup and spritesheet export

Need UI, icons, or HUD states?
    -> ComfyUI for visual direction
    -> Figma for layout and component states
    -> Inkscape for SVG cleanup or deterministic icon export

Need a 3D asset?
    -> ComfyUI for reference sheet, material references, and turnaround boards
    -> Blender for modeling, preview renders, and export
```

## ComfyUI Responsibilities

ComfyUI is the primary generation engine.

Use it for:

- text-to-image concept generation
- img2img refinement
- ControlNet pose, silhouette, or composition control
- LoRA-driven style consistency
- batch variation for candidate exploration
- upscale and cleanup pre-pass
- texture and material reference generation
- turnaround or orthographic reference sheet preparation

Do not treat downstream tools as alternate generation centers unless the lane explicitly requires it.

## 2D Concept Lane

### 2D Default Path

1. Generate 3 to 8 candidate directions in ComfyUI.
2. Select one candidate or merge direction from multiple candidates.
3. Clean edges, alpha, layers, and color balance in GIMP.
4. Export final game-ready images.

### 2D Output Contract

- Final PNG or WebP
- Transparent or solid background as required
- Variant naming for approved directions
- Prompt and seed metadata preserved somewhere in the workflow notes

## Sprite And Pixel Lane

### Sprite Default Path

1. Generate concept or silhouette direction in ComfyUI.
2. Reduce visual complexity before production use.
3. Rebuild and clean in Aseprite.
4. Export sprite, animation, or spritesheet package.

### Sprite Output Contract

- Source `.aseprite` or `.ase`
- Final sprite PNGs or spritesheet
- Palette notes or locked palette definition when applicable
- Frame size, sheet layout, and export dimensions

## UI, Icon, And HUD Lane

### UI Default Path

1. Generate visual direction in ComfyUI.
2. Build component states and layouts in Figma.
3. Use Inkscape when deterministic SVG cleanup or export is needed.
4. Export named target sizes for game integration.

### UI Output Contract

- SVG and PNG exports for required sizes
- State variants such as default, hover, selected, disabled, warning
- Stable component naming
- Layout or spacing notes when the asset is part of a larger HUD system

## 3D Asset Lane

### 3D Default Path

1. Generate reference sheet or material direction in ComfyUI.
2. Use Blender for modeling, fast iteration, scene assembly, baking, preview renders, and export.
3. Produce preview renders and exports.

### 3D Output Contract

- Concept or reference sheet
- Source scene file
- Preview renders
- Texture outputs or reference package when applicable
- Export target such as FBX, GLB, OBJ, or engine-ready mesh package

## Cross-Tool Handoff Rules

- Hand off files, not abstract intent. Every lane should leave a concrete artifact for the next tool.
- Prefer lossless intermediate outputs when a second tool will continue editing.
- Keep naming deterministic so later export or packaging steps can identify the chosen asset.
- Preserve style references, seeds, prompt variants, and approved direction notes where practical.

## What Is Deferred

- Krita remains deferred for the first release because it does not add enough beyond ComfyUI plus GIMP for the initial production lanes.
- Pixelle cloud mode remains deferred unless cloud execution or no-GPU operation becomes a first-class requirement.
- Affinity is excluded from this workflow.
- Rhino is excluded from this workflow because it requires a paid license.

## Output Requirements

When using this workflow:

1. Start from the asset lane, not from a favorite tool.
2. Use ComfyUI as the default generation entry point.
3. Ensure every lane produces a concrete output package, not just intermediate artwork.
4. Keep the 3D lane centered on Blender unless a new free tool is explicitly added later.
