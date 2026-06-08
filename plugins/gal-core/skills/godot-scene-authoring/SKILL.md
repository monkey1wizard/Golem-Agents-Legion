---
name: godot-scene-authoring
description: Scene and node authoring for Godot 4 projects. Use when creating `.tscn` files, adding nodes, changing scene properties, deciding between online editor control and offline scene editing, or structuring Godot scene trees for AI-assisted game development.
mcpDependencies:
  optional:
    - godot-mcp
    - better-godot-mcp
license: Complete terms in LICENSE.txt
---

# Godot Scene Authoring

Use this skill when the task is primarily about scenes, nodes, resources, and scene-tree structure.

## Preferred Tool Order

1. Use `better-godot-mcp` for deterministic offline `.tscn` edits.
2. Use `godot-mcp` when editor feedback or run-loop validation matters.
3. Use direct file edits only when the MCP path is unavailable and the scene format is fully understood.
4. Treat this as an explicit MCP-first exception to generic CLI-first patterns.

## Common Patterns

- Create a root scene with the correct base node first.
- Add child nodes in small steps and set only the properties you can verify.
- Keep scene ownership and node paths stable so scripts can bind predictably.

## Node Selection Guidance

| Goal | Typical Node |
| --- | --- |
| 2D world root | `Node2D` |
| 3D world root | `Node3D` |
| 2D player movement | `CharacterBody2D` |
| 3D player movement | `CharacterBody3D` |
| Trigger volume | `Area2D` or `Area3D` |
| Physics object | `RigidBody2D` or `RigidBody3D` |
| Camera | `Camera2D` or `Camera3D` |
| Light | `PointLight2D`, `DirectionalLight3D`, or similar |

## Decision Tree

```text
Need to create or refactor scenes without opening Godot?
    -> better-godot-mcp

Need editor-driven validation or live run feedback?
    -> godot-mcp

Need runtime inspection of the scene tree?
    -> Switch to godot-runtime-debug
```

## No-Tool Behavior

If the required Godot MCP path is unavailable, do not guess at scene edits that depend on editor semantics or structured resource operations.

- Use direct file edits only when the `.tscn` change is deterministic and fully understood.
- If the task depends on MCP-owned operations and no safe direct-edit path exists, report the blocker explicitly.
- Do not claim a scene change was validated when no runnable editor or MCP path was available.

## Output Requirements

When using this skill:

1. Prefer small, verifiable scene edits.
2. Choose base nodes that match movement, physics, and camera needs.
3. Keep scene tree structure predictable for script lookup and automation.