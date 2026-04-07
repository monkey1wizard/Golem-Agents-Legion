# Godot C# External Knowledge

Use official Godot documentation as the source of truth for language behavior, runtime limitations, and API mapping.

## Core References

| Topic | Source |
| --- | --- |
| Godot command line workflow | [Godot Command Line Tutorial](https://docs.godotengine.org/en/stable/tutorials/editor/command_line_tutorial.html) |
| C# overview | [Godot C# / .NET Docs](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/index.html) |
| API differences from GDScript | [C# API Differences to GDScript](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_differences.html) |
| Exported properties | [C# Exported Properties](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_exports.html) |
| Signals | [C# Signals](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_signals.html) |
| Collections | [C# Collections](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_collections.html) |
| Variant support | [C# Variant](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_variant.html) |
| Style guide | [Godot C# Style Guide](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/c_sharp_style_guide.html) |

## What To Pull From Official Docs

- Runtime-supported C# version for the current Godot release
- `[Export]` and `PropertyHint` usage
- `[Signal]` delegates, generated events, and `ToSignal()` awaiting
- PascalCase API mappings from GDScript names
- `Godot.Collections` versus standard .NET collections
- Variant-compatible types and engine boundary behavior

## Secondary Sources

Use community MCP repositories for tooling capabilities, not for Godot language semantics:

- [Coding-Solo/godot-mcp](https://github.com/Coding-Solo/godot-mcp)
- [n24q02m/better-godot-mcp](https://github.com/n24q02m/better-godot-mcp)
- [MingHuiLiu/godot4-runtime-mcp](https://github.com/MingHuiLiu/godot4-runtime-mcp)

Do not use GDScript-only skill libraries as the primary knowledge base for C# gameplay code.
