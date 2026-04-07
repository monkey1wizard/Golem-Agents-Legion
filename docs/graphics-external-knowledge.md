# Graphics External Knowledge

This file tracks the official documentation sources GAL should trust first for the game-asset workflow.

## Authority Order

1. Official tool documentation
2. Official API or command-line references
3. MCP server documentation for tool capability boundaries
4. Community tutorials only when official docs are missing

## Core References

| Topic | Source |
| --- | --- |
| Blender command line | [Blender Manual - Command Line Arguments](https://docs.blender.org/manual/en/latest/advanced/command_line/arguments.html) |
| Blender Python API | [Blender Python API](https://docs.blender.org/api/current/) |
| GIMP scripting and plug-ins | [GIMP Documentation](https://docs.gimp.org/) |
| Inkscape CLI | [Inkscape Man Page](https://inkscape.org/doc/inkscape-man.html) |
| Figma developer docs | [Figma Developers](https://www.figma.com/developers) |
| Aseprite docs | [Aseprite Docs](https://www.aseprite.org/docs/) |
| ComfyUI project | [ComfyUI](https://github.com/comfyanonymous/ComfyUI) |

## Lane-Specific Guidance

### 2D Concept And Illustration

- Use ComfyUI references and workflow JSON as operational knowledge.
- Use GIMP official docs for cleanup, compositing, scripting, and export behavior.

### Sprite And Pixel Assets

- Use Aseprite docs for animation, timeline behavior, palette handling, and spritesheet semantics.
- Treat `pixel-mcp` docs as the operational wrapper, not the source of pixel-art semantics.

### UI And Vector Assets

- Use Figma official docs for design-system and component semantics.
- Use Inkscape docs and SVG standards for deterministic export and path behavior.

### 3D Assets

- Use Blender official docs for modeling, baking, materials, and CLI automation.

## ComfyUI Knowledge Boundary

ComfyUI is a workflow engine, not a single artistic methodology.

Rely on ComfyUI for:

- workflow orchestration
- parameter exposure
- batch variation
- image refinement
- seed and style consistency

Do not treat ComfyUI outputs as production-ready by default. They are inputs to the downstream lane tools unless the lane explicitly allows direct export.

## Deferred References

- Pixelle references are deferred until cloud mode or multimodal generation becomes part of the first-class workflow.
- Krita references remain secondary because Krita is not in the initial MCP surface. See [Krita Linux Command Line](https://docs.krita.org/en/reference_manual/linux_command_line.html) when needed.
- Rhino is excluded from this workflow because it requires a paid license.

## Output Requirements

When pulling external knowledge into GAL:

1. Prefer official docs over MCP README claims.
2. Use MCP repo docs only to understand wrapper capabilities and limits.
3. Keep the workflow AI-first: ComfyUI generates, lane tools finish and export.
