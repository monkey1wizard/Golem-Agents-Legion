---
name: godot-scripting
description: C# scripting workflow for Godot 4 projects. Use when writing or reviewing Godot C# node scripts, signals, exported properties, lifecycle methods, input handling, physics logic, or runtime-safe patterns such as `partial class`, `_Ready()`, and `ToSignal()`.
mcpDependencies:
  optional:
    - better-godot-mcp
    - godot4-runtime-mcp
license: Complete terms in LICENSE.txt
---

# Godot Scripting

Use this skill for Godot gameplay code written in C#.

## First Rule

Read the Godot runtime section in `../../conventions/csharp.md` before changing gameplay scripts.

## Core Rules

- Godot node scripts must be `public partial class`
- Avoid constructors and primary constructors on node scripts
- Use `[Export]` for Inspector-driven configuration
- Use `[Signal]` delegates and generated events for signal-driven behavior
- Cache node references in `_Ready()`
- Use `_PhysicsProcess(double delta)` for movement and collision logic

## Preferred Patterns

| Problem | Pattern |
| --- | --- |
| Scene reference needed after load | `GetNode<T>()` inside `_Ready()` |
| Configurable speed, damage, or resource | `[Export] public float Speed { get; set; }` |
| Wait for a Godot signal | `await ToSignal(node, Node.SignalName.X)` |
| Per-frame movement | `_PhysicsProcess(double delta)` |
| Input query | `Input.IsActionPressed(...)` or `Input.GetVector(...)` |

## Decision Tree

```text
Need to write or refactor Godot C# code?
    -> Follow ../../conventions/csharp.md first

Need live confirmation that a signal, property, or method behaves correctly?
    -> Use godot4-runtime-mcp after the game is running

Need file-level script edits attached to scene work?
    -> Combine with godot-scene-authoring
```

## Output Requirements

When using this skill:

1. Keep runtime code compatible with the Godot-supported C# version.
2. Use Godot lifecycle hooks instead of constructors for scene wiring.
3. Prefer explicit, engine-friendly patterns over clever language features.