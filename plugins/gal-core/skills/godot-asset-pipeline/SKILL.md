---
name: godot-asset-pipeline
description: Asset import and resource management for Godot 4 projects. Use when importing art or audio, managing resources, editing shaders, tilemaps, navigation data, input maps, or choosing the right tool for Godot resource pipeline work.
mcpDependencies:
  optional:
    - godot-mcp
    - better-godot-mcp
license: Complete terms in LICENSE.txt
---

# Godot Asset Pipeline

Use this skill for resource import, resource configuration, and non-code gameplay data.

## Preferred Tool Order

1. Use Godot CLI `--import` for reproducible asset imports.
2. Use `better-godot-mcp` for resource-level edits such as shader, tilemap, audio, navigation, and input-map changes.
3. Use `godot-mcp` when asset work must be validated through the editor or a running project.

## Coverage

| Area | Preferred Path |
| --- | --- |
| Asset import | Godot CLI |
| Shader files | `better-godot-mcp` shader tools |
| TileSet and TileMap data | `better-godot-mcp` tilemap tools |
| Audio buses and effects | `better-godot-mcp` audio tools |
| Navigation regions and agents | `better-godot-mcp` navigation tools |
| Input actions and events | `better-godot-mcp` input map tools |

## Decision Tree

```text
Need deterministic import or reimport?
    -> Godot CLI --import

Need to edit structured resource data offline?
    -> better-godot-mcp

Need visual validation in the editor or game?
    -> godot-mcp
```

## Output Requirements

When using this skill:

1. Separate import work from gameplay-code changes when possible.
2. Prefer reproducible import commands over manual editor-only steps.
3. Validate resource paths and formats before wiring them into scenes or scripts.