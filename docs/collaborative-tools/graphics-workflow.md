# Game Art Workflow

A complete guide to the AI-first game art pipeline. Covers the four pipeline routes, the MCP tool stack, and the official documentation index.

## Core Principles

1. Start from the asset pipeline, not from the tool.
2. ComfyUI is the default generation entry point for every pipeline.
3. Each pipeline must produce a concrete output package, not an intermediate object.
4. The 3D pipeline is Blender-centered unless another free tool is explicitly added.

## The Four Pipelines

### 2D Concept and Illustration

```text
ComfyUI → GIMP → final game asset
```

1. Generate directional references or controlled variants in ComfyUI.
2. Choose the preferred direction.
3. Clean up edges, alpha, layers, and color balance in GIMP.
4. Export the final game-ready image.

**Output contract**: final PNG or WebP, transparent or solid background, variant naming, prompt and seed metadata.

### Sprite and Pixel Assets

```text
ComfyUI → Aseprite → final sprite or spritesheet
```

1. Generate concept or silhouette directions in ComfyUI.
2. Reduce visual complexity.
3. Rebuild and clean up in Aseprite.
4. Export the sprite, animation, or spritesheet package.

**Output contract**: source `.aseprite` or `.ase`, final sprite PNG or spritesheet, palette definition (when applicable), frame size and sheet layout.

### UI, Icon, and HUD

```text
ComfyUI → Figma → Inkscape → final export
```

1. Generate visual direction in ComfyUI.
2. Build component states and layout in Figma.
3. Use Inkscape when deterministic SVG cleanup or export is needed.
4. Export at the target sizes for game integration.

**Output contract**: SVG and PNG exports, state variants (default / hover / selected / disabled / warning), stable component naming, layout or spacing notes.

### 3D Assets

```text
ComfyUI → Blender → preview render and export
```

1. Generate a reference sheet or material direction in ComfyUI.
2. Model, iterate quickly, assemble scenes, bake, preview-render, and export in Blender.
3. Produce the preview render and export.

**Output contract**: concept or reference sheet, source scene file, preview render, texture output or reference package, export target (FBX / GLB / OBJ or an engine-ready mesh package).

## MCP Tool Stack

| Pipeline | Tool | MCP / Runtime | Notes |
| --- | --- | --- | --- |
| Generation | ComfyUI | `joenorton/comfyui-mcp-server` | default local generation server |
| 2D cleanup | GIMP | `maorcc/gimp-mcp` | GIMP 3.0 API bridge |
| Pixel assets | Aseprite | `willibrandon/pixel-mcp` | sprite, animation, and spritesheet workflows |
| UI layout | Figma | `grab/cursor-talk-to-figma-mcp` | UI and component workflows |
| Vector cleanup | Inkscape | `grumpydevorg/inkscape-mcps` | CLI + DOM operations |
| 3D general | Blender | `ahujasid/blender-mcp` | default 3D workflow |

### Decision Tree

```text
Need to generate asset direction or controlled variants? → ComfyUI
Need raster cleanup or compositing?                       → GIMP MCP
Need pixel production or a spritesheet?                    → pixel-mcp
Need UI layout or HUD compositing?                         → Figma MCP
Need deterministic SVG cleanup?                            → Inkscape MCP
Need general 3D modeling and preview render?               → Blender MCP
```

## Cross-Tool Handoff Rules

- Hand off files, not abstract intent. Each pipeline should leave a concrete output file for the next tool.
- Prefer lossless intermediate output when a second tool will keep editing.
- Keep naming deterministic so later export or packaging steps can identify the chosen assets.
- When practical, preserve style references, seeds, prompt variants, and approved-direction notes.

## Official Documentation Index

| Topic | Source |
| --- | --- |
| Blender command line | [Blender Manual - Command Line Arguments](https://docs.blender.org/manual/en/latest/advanced/command_line/arguments.html) |
| Blender Python API | [Blender Python API](https://docs.blender.org/api/current/) |
| GIMP scripting and plugins | [GIMP Documentation](https://docs.gimp.org/) |
| Inkscape CLI | [Inkscape Man Page](https://inkscape.org/doc/inkscape-man.html) |
| Figma developer docs | [Figma Developers](https://www.figma.com/developers) |
| Aseprite docs | [Aseprite Docs](https://www.aseprite.org/docs/) |
| ComfyUI project | [ComfyUI](https://github.com/comfyanonymous/ComfyUI) |

### Authority Order

1. Official tool documentation
2. Official API or command-line reference
3. MCP server documentation (to understand the wrapper's capability boundary)
4. Community tutorials (only when official documentation is missing)

### ComfyUI Knowledge Boundary

ComfyUI is a workflow engine, not a single art methodology. Rely on ComfyUI for workflow orchestration, parameter exposure, batch variation, image refinement, and seed/style consistency. Do not assume ComfyUI output is production-ready — it is input for the downstream pipeline tools.

## Deferred Tools

- **Krita** — deferred, because ComfyUI + GIMP already cover early 2D needs.
- **Pixelle** — deferred, unless cloud mode or GPU-less operation becomes necessary.
